"""
superdupermemory async client — drop-in for Mem0Client in benchmarks.

Each user_id gets an isolated SQLite database and dedicated subprocess so
conversations don't bleed into each other.
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
import re
from typing import Any

logger = logging.getLogger(__name__)


class SdmProcess:
    """One superdupermemory MCP server subprocess, isolated per user."""

    def __init__(self, binary: str, db_path: str):
        self.binary = binary
        self.db_path = db_path
        self._proc: asyncio.subprocess.Process | None = None
        self._req_id = 0
        self._lock = asyncio.Lock()

    async def start(self) -> None:
        os.makedirs(os.path.dirname(os.path.abspath(self.db_path)), exist_ok=True)
        env = {**os.environ, "SDM_DB_PATH": self.db_path}
        self._proc = await asyncio.create_subprocess_exec(
            self.binary, "serve",
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
            env=env,
        )
        await self._initialize()

    async def _initialize(self) -> None:
        await self._send({
            "jsonrpc": "2.0",
            "id": self._next_id(),
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "sdm-bench", "version": "1.0"},
            },
        })
        await self._recv()  # InitializeResult
        # Send initialized notification (no response expected)
        await self._send({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        })

    def _next_id(self) -> int:
        self._req_id += 1
        return self._req_id

    async def _send(self, msg: dict) -> None:
        data = (json.dumps(msg) + "\n").encode()
        self._proc.stdin.write(data)
        await self._proc.stdin.drain()

    async def _recv(self) -> dict:
        line = await asyncio.wait_for(self._proc.stdout.readline(), timeout=300.0)
        return json.loads(line.decode())

    async def call_tool(self, tool: str, arguments: dict) -> Any:
        async with self._lock:
            req_id = self._next_id()
            await self._send({
                "jsonrpc": "2.0",
                "id": req_id,
                "method": "tools/call",
                "params": {"name": tool, "arguments": arguments},
            })
            # Drain notifications until we get our response
            while True:
                resp = await self._recv()
                if "id" not in resp:
                    continue  # notification, skip
                if resp.get("id") != req_id:
                    continue
                break

            if "error" in resp:
                raise RuntimeError(f"MCP error from {tool}: {resp['error']}")

            content = resp.get("result", {}).get("content", [])
            for item in content:
                if item.get("type") == "text":
                    try:
                        return json.loads(item["text"])
                    except (json.JSONDecodeError, ValueError):
                        return item.get("text")
            return None

    async def close(self) -> None:
        if self._proc:
            try:
                self._proc.stdin.close()
                await asyncio.wait_for(self._proc.wait(), timeout=5.0)
            except (asyncio.TimeoutError, Exception):
                self._proc.kill()
            self._proc = None


class SdmClient:
    """
    Async superdupermemory client matching the Mem0Client interface.

    Args:
        binary: Path to the superdupermemory binary.
        db_dir: Directory for per-user SQLite databases.
        max_retries: Retry count for tool calls.
    """

    def __init__(
        self,
        binary: str = "superdupermemory",
        db_dir: str = "/tmp/sdm_bench",
        max_retries: int = 3,
    ):
        self.binary = binary
        self.db_dir = db_dir
        self.max_retries = max_retries
        self._processes: dict[str, SdmProcess] = {}

    def _db_path(self, user_id: str) -> str:
        safe = user_id.replace("/", "_").replace(":", "_").replace(" ", "_")
        return os.path.join(self.db_dir, f"{safe}.db")

    async def _get_proc(self, user_id: str) -> SdmProcess:
        if user_id not in self._processes:
            proc = SdmProcess(self.binary, self._db_path(user_id))
            await proc.start()
            self._processes[user_id] = proc
            logger.debug("Started SDM process for user=%s db=%s", user_id, self._db_path(user_id))
        return self._processes[user_id]

    async def _kill_proc(self, user_id: str) -> None:
        if user_id in self._processes:
            await self._processes.pop(user_id).close()

    async def add(
        self,
        messages: list[dict[str, str]],
        user_id: str,
        observation_date: str | None = None,
        timestamp: int | None = None,
        **kwargs: Any,
    ) -> dict | None:
        """Ingest a message chunk into superdupermemory for user_id."""
        text = "\n".join(
            f"{m.get('role', 'user')}: {m.get('content', '')}"
            for m in messages
            if m.get("content", "").strip()
        )
        if not text.strip():
            return {"results": []}

        for attempt in range(self.max_retries):
            try:
                proc = await self._get_proc(user_id)
                result = await proc.call_tool("remember", {
                    "text": text,
                    "source": user_id,
                })
                facts = result if isinstance(result, list) else []
                return {
                    "results": [
                        {"memory": f.get("body", ""), "id": f.get("id", ""), "event": "ADD"}
                        for f in facts
                    ]
                }
            except Exception as exc:
                logger.warning("SDM add attempt %d/%d failed (user=%s): %s",
                               attempt + 1, self.max_retries, user_id, str(exc)[:200])
                await self._kill_proc(user_id)
                if attempt == self.max_retries - 1:
                    logger.error("SDM add failed after %d attempts for user=%s", self.max_retries, user_id)
                    return None
                await asyncio.sleep(2.0 * (attempt + 1))

    @staticmethod
    def _parse_recall_text(text: str) -> list[dict]:
        """Parse plain-text recall output ('[uuid] subject: body\\n...') into fact dicts."""
        if not text or "No matching" in text or text.startswith("error:"):
            return []
        facts = []
        for line in text.strip().split("\n"):
            m = re.match(r'\[([^\]]+)\]\s+(\S[^:]*?):\s+(.+)', line)
            if m:
                facts.append({
                    "id": m.group(1).strip(),
                    "subject": m.group(2).strip(),
                    "body": m.group(3).strip(),
                    "created_at": "",
                    "updated_at": "",
                })
        return facts

    async def search(
        self,
        query: str,
        user_id: str,
        top_k: int = 200,
        **kwargs: Any,
    ) -> list[dict]:
        """Search superdupermemory for user_id. Returns mem0-compatible result list."""
        for attempt in range(self.max_retries):
            try:
                proc = await self._get_proc(user_id)
                result = await proc.call_tool("recall", {
                    "query": query,
                    "limit": top_k,
                })
                if isinstance(result, list):
                    facts = result
                elif isinstance(result, str):
                    facts = self._parse_recall_text(result)
                else:
                    facts = []
                return [
                    {
                        "memory": f.get("body", ""),
                        "score": 1.0,  # SDM returns facts ordered by blended score
                        "id": f.get("id", ""),
                        "created_at": f.get("created_at", ""),
                        "updated_at": f.get("updated_at", ""),
                    }
                    for f in facts
                ]
            except Exception as exc:
                logger.warning("SDM search attempt %d/%d failed (user=%s): %s",
                               attempt + 1, self.max_retries, user_id, str(exc)[:200])
                await self._kill_proc(user_id)
                if attempt == self.max_retries - 1:
                    return []
                await asyncio.sleep(2.0 * (attempt + 1))
        return []

    async def delete_user(self, user_id: str) -> bool:
        """Terminate the user's process and delete their database."""
        await self._kill_proc(user_id)
        db = self._db_path(user_id)
        for path in [db, db + "-wal", db + "-shm"]:
            try:
                os.remove(path)
            except FileNotFoundError:
                pass
        return True

    async def close(self) -> None:
        for proc in list(self._processes.values()):
            await proc.close()
        self._processes.clear()

    async def __aenter__(self) -> "SdmClient":
        return self

    async def __aexit__(self, *exc: Any) -> None:
        await self.close()
