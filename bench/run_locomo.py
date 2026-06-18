"""
LOCOMO-10 benchmark runner — superdupermemory vs mem0

Flow per conversation:
  1. Download locomo10.json (auto, from snap-research/locomo)
  2. Ingest all sessions turn-by-turn via the chosen backend
  3. For each QA pair: search → generate answer → LLM judge
  4. Compute accuracy by category and top-k cutoff

Usage:
  # Against superdupermemory (needs the binary on PATH or --sdm-binary):
  python run_locomo.py --backend sdm --project-name sdm-test

  # Against mem0 OSS (needs: cd path/to/mem0 && docker compose up -d):
  python run_locomo.py --backend mem0 --project-name mem0-test

  # Quick smoke test (1 conversation, 5 questions):
  python run_locomo.py --backend sdm --project-name smoke --conversations 0 --max-questions 5

  # Cheaper models:
  python run_locomo.py --backend sdm --project-name sdm-cheap \\
      --answerer-model gpt-4.1-mini --judge-model gpt-4.1-mini
"""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
import re
import statistics
import sys
import time
import uuid
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import aiohttp
import openai
import requests
from dotenv import load_dotenv
from tqdm import tqdm

from sdm_client import SdmClient

load_dotenv(override=True)

# ── dataset ────────────────────────────────────────────────────────────────────

DATASET_URL = "https://raw.githubusercontent.com/snap-research/locomo/main/data/locomo10.json"
CHUNK_SIZE = 1  # turns per ingestion call (matches mem0 benchmark default)

CATEGORY_NAMES = {
    1: "multi-hop",
    2: "temporal",
    3: "open-domain",
    4: "single-hop",
    5: "adversarial",
}
CATEGORIES_TO_EVALUATE = [1, 2, 3, 4]

# ── prompts (adapted from mem0ai/memory-benchmarks) ────────────────────────────

ANSWER_PROMPT = """\
You are answering a question using retrieved memories from past conversations.

## Memories (chronological, oldest first)
{memories}

Question: {question}

Instructions:
- Read ALL memories before answering. Important details are often near the end.
- Combine facts from multiple memories when needed.
- Give the most specific answer the memories support.
- For list/count questions include every distinct item found.
- For temporal questions: conversations took place around {reference_date}.
- Never say "not mentioned" if any memory contains relevant information.
- After your reasoning write your final answer after "ANSWER:".
"""

JUDGE_SYSTEM = "You are evaluating AI memory recall. Return JSON only."

JUDGE_PROMPT = """\
Label the generated answer as CORRECT or WRONG.

Rules:
1. Partial credit: if the generated answer includes AT LEAST ONE correct item from the gold list, mark CORRECT.
2. Paraphrases count: same concept in different words is CORRECT.
3. Extra detail is fine: more detail than the gold answer is CORRECT.
4. Date tolerance: dates within 14 days are CORRECT. Durations within 50% are CORRECT.
5. Semantic overlap: judge the underlying concept, not exact wording.
6. Only mark WRONG if the answer contains ZERO correct items OR addresses a completely different topic.

Question: {question}
Gold answer: {answer}
Generated answer: {response}

Return JSON: {{"reasoning": "<one sentence>", "label": "CORRECT" or "WRONG"}}
"""


def _to_human_date(iso_str: str) -> str:
    for fmt in ("%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%dT%H:%M:%S.%f%z", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d"):
        try:
            return datetime.strptime(iso_str[:26].rstrip("Z"), fmt.replace("%z", "")).strftime("%B %d, %Y")
        except ValueError:
            continue
    return iso_str[:10]


def build_answer_prompt(question: str, results: list[dict], reference_date: str | None) -> str:
    if not results:
        memories_text = "(No memories retrieved)"
    else:
        sorted_r = sorted(results, key=lambda x: x.get("created_at", ""))
        lines = []
        for r in sorted_r:
            mem = r.get("memory", "")
            ca = r.get("created_at", "")
            date_str = _to_human_date(ca) if ca else "unknown date"
            lines.append(f"({date_str}) {mem}")
        memories_text = "\n".join(lines)
    return ANSWER_PROMPT.format(
        memories=memories_text,
        question=question,
        reference_date=reference_date or "2023",
    )


# ── LLM client ─────────────────────────────────────────────────────────────────

class LLMClient:
    def __init__(self, model: str, rpm: int = 200):
        self.model = model
        self._client = openai.AsyncOpenAI()
        self._sem = asyncio.Semaphore(max(1, rpm // 6))  # rough concurrency cap

    async def generate(self, system: str, user: str) -> str:
        async with self._sem:
            for attempt in range(5):
                try:
                    msgs = []
                    if system:
                        msgs.append({"role": "system", "content": system})
                    msgs.append({"role": "user", "content": user})
                    resp = await self._client.chat.completions.create(
                        model=self.model, messages=msgs, temperature=0.0,
                    )
                    return resp.choices[0].message.content or ""
                except openai.RateLimitError:
                    await asyncio.sleep(10 * (attempt + 1))
                except Exception as exc:
                    if attempt == 4:
                        raise
                    await asyncio.sleep(3 * (attempt + 1))
        return ""

    async def generate_json(self, system: str, user: str) -> dict:
        raw = await self.generate(system, user)
        # Strip markdown fences if present
        text = raw.strip().lstrip("```json").lstrip("```").rstrip("```").strip()
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return {"label": "WRONG", "reasoning": "parse error"}


# ── dataset helpers ─────────────────────────────────────────────────────────────

def download_dataset(dest_dir: str) -> str:
    path = os.path.join(dest_dir, "locomo10.json")
    if os.path.exists(path):
        return path
    os.makedirs(dest_dir, exist_ok=True)
    print("Downloading LOCOMO-10 dataset...")
    r = requests.get(DATASET_URL, stream=True, timeout=60)
    r.raise_for_status()
    with open(path, "wb") as f:
        for chunk in r.iter_content(chunk_size=8192):
            f.write(chunk)
    data = json.loads(Path(path).read_text())
    assert len(data) == 10, f"Expected 10 conversations, got {len(data)}"
    print(f"Downloaded: {path}")
    return path


def parse_locomo_date(date_str: str) -> datetime | None:
    for fmt in ("%I:%M %p on %d %B, %Y", "%I:%M %p on %d %b, %Y"):
        try:
            return datetime.strptime(date_str, fmt)
        except (ValueError, TypeError):
            continue
    return None


def locomo_date_to_epoch(date_str: str) -> int | None:
    parsed = parse_locomo_date(date_str)
    return int(parsed.replace(tzinfo=timezone.utc).timestamp()) if parsed else None


def get_sorted_sessions(conversation: dict) -> list[tuple[str, str, list[dict]]]:
    keys = [k for k in conversation if re.match(r"^session_\d+$", k)]
    paired = [(k, conversation.get(f"{k}_date_time", ""), conversation[k]) for k in keys]

    def sort_key(item):
        parsed = parse_locomo_date(item[1])
        if parsed:
            return (0, parsed)
        num = int(re.search(r"\d+", item[0]).group())
        return (1, datetime(2000, 1, num))

    return sorted(paired, key=sort_key)


def session_to_chunks(turns: list[dict], speaker_a: str, speaker_b: str) -> list[list[dict]]:
    messages = []
    for turn in turns:
        speaker = turn.get("speaker", "")
        text = turn.get("text", "")
        blip = turn.get("blip_caption", "")
        query = turn.get("query", "")
        if query and blip:
            text = f"{text} [Image: {query}; shows: {blip}]" if text else f"[Image: {query}; shows: {blip}]"
        elif blip:
            text = f"{text} [Image shows: {blip}]" if text else f"[Image shows: {blip}]"
        if not text:
            continue
        role = "user" if speaker == speaker_a else "assistant"
        messages.append({"role": role, "content": f"{speaker}: {text}"})
    return [messages[i:i + CHUNK_SIZE] for i in range(0, len(messages), CHUNK_SIZE) if messages[i:i + CHUNK_SIZE]]


# ── mem0 client (OSS-only, minimal) ────────────────────────────────────────────

class Mem0Client:
    """Thin async wrapper for the mem0 OSS server."""

    def __init__(self, host: str = "http://localhost:8888", max_retries: int = 5):
        self.host = host.rstrip("/")
        self.max_retries = max_retries
        self._session: aiohttp.ClientSession | None = None

    async def _sess(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(
                headers={"Content-Type": "application/json"},
                timeout=aiohttp.ClientTimeout(total=300),
            )
        return self._session

    async def add(self, messages, user_id, timestamp=None, **kwargs):
        payload: dict[str, Any] = {"messages": messages, "user_id": user_id}
        if timestamp is not None:
            payload["timestamp"] = timestamp
        sess = await self._sess()
        for attempt in range(self.max_retries):
            try:
                async with sess.post(f"{self.host}/memories", json=payload) as r:
                    r.raise_for_status()
                    data = await r.json()
                return data if isinstance(data, dict) else {"results": data}
            except Exception as exc:
                if attempt == self.max_retries - 1:
                    return None
                await asyncio.sleep(5 * (attempt + 1))

    async def search(self, query, user_id, top_k=200, **kwargs):
        payload = {"query": query, "user_id": user_id, "limit": top_k}
        sess = await self._sess()
        for attempt in range(self.max_retries):
            try:
                async with sess.post(f"{self.host}/search", json=payload) as r:
                    r.raise_for_status()
                    data = await r.json()
                results = data.get("results", data) if isinstance(data, dict) else data
                return [
                    {
                        "memory": r.get("memory", r.get("data", "")),
                        "score": r.get("score", 0),
                        "id": r.get("id", ""),
                        "created_at": r.get("created_at", ""),
                        "updated_at": r.get("updated_at", ""),
                    }
                    for r in (results if isinstance(results, list) else [])
                ]
            except Exception as exc:
                if attempt == self.max_retries - 1:
                    return []
                await asyncio.sleep(5 * (attempt + 1))

    async def delete_user(self, user_id):
        sess = await self._sess()
        try:
            async with sess.delete(f"{self.host}/memories", params={"user_id": user_id}) as r:
                r.raise_for_status()
            return True
        except Exception:
            return False

    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc):
        await self.close()


# ── cutoff helpers ──────────────────────────────────────────────────────────────

def cutoff_label(k: int) -> str:
    return f"top_{k}"


# ── core pipeline ───────────────────────────────────────────────────────────────

async def ingest_conversation(
    conv_idx: int,
    entry: dict,
    backend: Any,
    run_id: str,
    logger: logging.Logger,
) -> str:
    """Ingest all sessions of one LOCOMO conversation. Returns user_id."""
    conversation = entry["conversation"]
    speaker_a = conversation["speaker_a"]
    speaker_b = conversation["speaker_b"]
    user_id = f"locomo_{conv_idx}_{run_id}"

    sorted_sessions = get_sorted_sessions(conversation)
    total_chunks = sum(len(session_to_chunks(s, speaker_a, speaker_b)) for _, _, s in sorted_sessions)
    pbar = tqdm(total=total_chunks, desc=f"Ingest conv {conv_idx}", leave=False)
    failed = 0

    for session_key, date_str, turns in sorted_sessions:
        chunks = session_to_chunks(turns, speaker_a, speaker_b)
        epoch = locomo_date_to_epoch(date_str)
        for messages in chunks:
            if any(not m.get("content", "").strip() for m in messages):
                pbar.update(1)
                continue
            result = await backend.add(messages, user_id, timestamp=epoch)
            if result is None:
                failed += 1
                logger.warning("Ingest failed: conv %d %s", conv_idx, session_key)
            pbar.update(1)

    pbar.close()
    if failed:
        logger.warning("Conversation %d: %d chunks failed", conv_idx, failed)
    return user_id


async def process_question(
    qa: dict,
    qa_idx: int,
    conv_idx: int,
    user_id: str,
    backend: Any,
    answerer: LLMClient,
    judge: LLMClient,
    cutoffs: list[int],
    top_k: int,
    reference_date: str | None,
) -> dict:
    """Search → generate answer → judge at each cutoff."""
    question = qa["question"]
    category = qa.get("category")
    ground_truth = str(qa["answer"])

    # Preprocess open-domain answers (category 3)
    expected = ground_truth.split(";")[0].strip() if category == 3 and ";" in ground_truth else ground_truth

    t0 = time.monotonic()
    results = await backend.search(question, user_id, top_k=top_k)
    search_ms = (time.monotonic() - t0) * 1000

    record: dict[str, Any] = {
        "question_id": f"conv{conv_idx}_q{qa_idx}",
        "conversation_idx": conv_idx,
        "category": category,
        "category_name": CATEGORY_NAMES.get(category, "unknown"),
        "question": question,
        "ground_truth_answer": ground_truth,
        "user_id": user_id,
        "reference_date": reference_date,
        "retrieval": {
            "search_query": question,
            "search_results": results,
            "search_latency_ms": round(search_ms, 1),
            "total_results": len(results),
        },
        "cutoff_results": {},
    }

    for c in cutoffs:
        sliced = results[:c]
        label = cutoff_label(c)

        answer_prompt = build_answer_prompt(question, sliced, reference_date)
        generated = await answerer.generate(system="", user=answer_prompt)
        if "ANSWER:" in generated:
            generated = generated.rsplit("ANSWER:", 1)[-1].strip()

        judge_prompt = JUDGE_PROMPT.format(
            question=question, answer=expected, response=generated,
        )
        verdict = await judge.generate_json(system=JUDGE_SYSTEM, user=judge_prompt)
        correct = verdict.get("label", "").upper() == "CORRECT"

        record["cutoff_results"][label] = {
            "judgment": "CORRECT" if correct else "WRONG",
            "score": 1.0 if correct else 0.0,
            "generated_answer": generated,
            "memories_evaluated": len(sliced),
            "reason": verdict.get("reasoning", ""),
        }

    return record


async def run_benchmark(
    dataset: list[dict],
    backend: Any,
    answerer: LLMClient,
    judge: LLMClient,
    conv_indices: list[int],
    cutoffs: list[int],
    top_k: int,
    max_questions: int | None,
    max_workers: int,
    run_id: str,
    output_dir: str,
    categories: list[int],
    logger: logging.Logger,
) -> list[dict]:
    os.makedirs(output_dir, exist_ok=True)
    all_results: list[dict] = []
    sem = asyncio.Semaphore(max_workers)

    async def handle_conversation(conv_idx: int) -> None:
        async with sem:
            if conv_idx >= len(dataset):
                return
            entry = dataset[conv_idx]
            conversation = entry["conversation"]
            sorted_sessions = get_sorted_sessions(conversation)
            ref_date = sorted_sessions[-1][1] if sorted_sessions else None

            user_id = await ingest_conversation(conv_idx, entry, backend, run_id, logger)

            questions = entry.get("qa", entry.get("qa_pairs", []))
            in_scope = [(qi, qa) for qi, qa in enumerate(questions) if qa.get("category") in categories]
            if max_questions is not None:
                in_scope = in_scope[:max_questions]

            pbar = tqdm(in_scope, desc=f"Questions conv {conv_idx}", leave=False)
            for qi, qa in pbar:
                result = await process_question(
                    qa=qa, qa_idx=qi, conv_idx=conv_idx,
                    user_id=user_id, backend=backend,
                    answerer=answerer, judge=judge,
                    cutoffs=cutoffs, top_k=top_k,
                    reference_date=ref_date,
                )
                path = Path(output_dir) / f"conv{conv_idx}_q{qi}.json"
                path.write_text(json.dumps(result, indent=2))
                all_results.append(result)

    await asyncio.gather(*[handle_conversation(i) for i in conv_indices])
    return all_results


# ── metrics ─────────────────────────────────────────────────────────────────────

def compute_metrics(evaluations: list[dict], cutoffs: list[int]) -> dict:
    metrics: dict[str, dict] = {}
    for c in cutoffs:
        label = cutoff_label(c)
        scores = [e.get("cutoff_results", {}).get(label, {}).get("score", 0.0) for e in evaluations]
        by_cat: dict[str, list] = defaultdict(list)
        for e in evaluations:
            cat = e.get("category_name", "unknown")
            by_cat[cat].append(e.get("cutoff_results", {}).get(label, {}).get("score", 0.0))
        cat_metrics = {
            cat: {
                "total": len(ss),
                "correct": sum(1 for s in ss if s >= 0.5),
                "accuracy": sum(1 for s in ss if s >= 0.5) / len(ss) * 100 if ss else 0.0,
            }
            for cat, ss in sorted(by_cat.items())
        }
        total = len(scores)
        correct = sum(1 for s in scores if s >= 0.5)
        metrics[label] = {
            "overall": {
                "total": total,
                "correct": correct,
                "accuracy": correct / total * 100 if total else 0.0,
                "avg_score": statistics.mean(scores) * 100 if scores else 0.0,
            },
            "by_category": cat_metrics,
        }
    return metrics


def display_metrics(metrics: dict, cutoffs: list[int], backend_name: str) -> None:
    print(f"\n{'=' * 60}")
    print(f"  Results — {backend_name}")
    print(f"{'=' * 60}")
    for c in cutoffs:
        label = cutoff_label(c)
        m = metrics.get(label, {})
        overall = m.get("overall", {})
        print(f"\n  [{label}]  {overall.get('correct', 0)}/{overall.get('total', 0)}"
              f"  accuracy={overall.get('accuracy', 0):.1f}%"
              f"  avg={overall.get('avg_score', 0):.1f}%")
        for cat, cm in m.get("by_category", {}).items():
            print(f"    {cat:<18} {cm['correct']:>3}/{cm['total']:<3}  ({cm['accuracy']:.1f}%)")


# ── CLI ──────────────────────────────────────────────────────────────────────────

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="LOCOMO-10 benchmark: superdupermemory vs mem0")
    p.add_argument("--backend", default="sdm", choices=["sdm", "mem0"],
                   help="Memory backend to benchmark (default: sdm)")
    p.add_argument("--project-name", required=True, help="Label for this run (used in output paths)")
    p.add_argument("--sdm-binary", default="superdupermemory",
                   help="Path to superdupermemory binary (default: superdupermemory on PATH)")
    p.add_argument("--sdm-db-dir", default="/tmp/sdm_bench",
                   help="Directory for per-user SDM databases")
    p.add_argument("--mem0-host", default="http://localhost:8888",
                   help="mem0 OSS server URL (default: http://localhost:8888)")
    p.add_argument("--answerer-model", default="gpt-4.1-mini",
                   help="OpenAI model for answer generation")
    p.add_argument("--judge-model", default="gpt-4.1-mini",
                   help="OpenAI model for judging")
    p.add_argument("--conversations", default="0,1,2,3,4,5,6,7,8,9",
                   help="Comma-separated conversation indices (default: all 10)")
    p.add_argument("--categories", default="1,2,3,4",
                   help="Comma-separated LOCOMO categories (default: 1,2,3,4)")
    p.add_argument("--top-k", type=int, default=200,
                   help="Max memories to retrieve per query")
    p.add_argument("--top-k-cutoffs", default="10,20,50,200",
                   help="Comma-separated cutoffs for evaluation")
    p.add_argument("--max-questions", type=int, default=None,
                   help="Max questions per conversation (for quick testing)")
    p.add_argument("--max-workers", type=int, default=3,
                   help="Max concurrent conversations")
    p.add_argument("--rpm", type=int, default=200,
                   help="Requests-per-minute cap for LLM calls")
    p.add_argument("--output-dir", default="results/locomo",
                   help="Directory for per-question JSON results")
    p.add_argument("--dataset-dir", default="datasets/locomo",
                   help="Directory to cache the dataset")
    p.add_argument("--debug", action="store_true")
    return p.parse_args()


async def async_main() -> None:
    args = parse_args()

    logging.basicConfig(
        level=logging.DEBUG if args.debug else logging.INFO,
        format="%(levelname)s %(name)s: %(message)s",
    )
    for noisy in ("httpx", "httpcore", "urllib3", "aiohttp", "openai"):
        logging.getLogger(noisy).setLevel(logging.WARNING)
    logger = logging.getLogger("locomo")

    cutoffs = [int(c) for c in args.top_k_cutoffs.split(",")]
    categories = [int(c) for c in args.categories.split(",")]
    conv_indices = [int(c) for c in args.conversations.split(",")]
    run_id = uuid.uuid4().hex[:8]
    output_dir = os.path.join(args.output_dir, f"{args.backend}_{args.project_name}_{run_id}")

    print(f"\nLOCOMO-10 Benchmark")
    print(f"  backend       : {args.backend}")
    print(f"  project       : {args.project_name}  run_id={run_id}")
    print(f"  answerer      : {args.answerer_model}")
    print(f"  judge         : {args.judge_model}")
    print(f"  conversations : {args.conversations}")
    print(f"  cutoffs       : {args.top_k_cutoffs}")
    print(f"  output        : {output_dir}")

    dataset_path = download_dataset(args.dataset_dir)
    dataset = json.loads(Path(dataset_path).read_text())

    answerer = LLMClient(model=args.answerer_model, rpm=args.rpm)
    judge = LLMClient(model=args.judge_model, rpm=args.rpm)

    if args.backend == "sdm":
        backend: Any = SdmClient(binary=args.sdm_binary, db_dir=args.sdm_db_dir)
    else:
        backend = Mem0Client(host=args.mem0_host)

    t_start = time.monotonic()
    async with backend:
        results = await run_benchmark(
            dataset=dataset,
            backend=backend,
            answerer=answerer,
            judge=judge,
            conv_indices=conv_indices,
            cutoffs=cutoffs,
            top_k=args.top_k,
            max_questions=args.max_questions,
            max_workers=args.max_workers,
            run_id=run_id,
            output_dir=output_dir,
            categories=categories,
            logger=logger,
        )
    elapsed = time.monotonic() - t_start

    if not results:
        print("No results produced.")
        return

    metrics = compute_metrics(results, cutoffs)
    display_metrics(metrics, cutoffs, args.backend)

    # Save unified result
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    result_path = os.path.join(args.output_dir, f"{args.backend}_{args.project_name}_{timestamp}.json")
    os.makedirs(args.output_dir, exist_ok=True)
    Path(result_path).write_text(json.dumps({
        "metadata": {
            "benchmark": "locomo",
            "backend": args.backend,
            "project_name": args.project_name,
            "run_id": run_id,
            "timestamp": timestamp,
            "answerer_model": args.answerer_model,
            "judge_model": args.judge_model,
            "top_k": args.top_k,
            "top_k_cutoffs": [cutoff_label(c) for c in cutoffs],
            "conversations": conv_indices,
            "categories": categories,
            "total_questions": len(results),
            "elapsed_seconds": round(elapsed, 1),
        },
        "metrics_by_cutoff": metrics,
        "evaluations": results,
    }, indent=2))

    print(f"\n  Elapsed : {elapsed:.0f}s")
    print(f"  Results : {result_path}")
    print(f"  Total   : {len(results)} questions\n")


def main() -> None:
    asyncio.run(async_main())


if __name__ == "__main__":
    main()
