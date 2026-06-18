# superdupermemory

Local-first memory layer for AI agents, exposed over MCP and HTTP.

Built in Rust. Uses SQLite for storage and fastembed for local embeddings — no external services required except an LLM for fact extraction (Anthropic or OpenAI).

---

## Two modes

| Mode | Use case |
|------|----------|
| **MCP server** (stdio) | Personal memory for Claude Code, Cursor, Codex CLI |
| **HTTP server** | Multi-tenant memory API for your AI products |

---

## Quick start — self-hosted (one line)

### Docker Compose

```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/docker-compose.yml -o docker-compose.yml
ANTHROPIC_API_KEY=sk-ant-... docker compose up -d
```

Open `http://localhost:3000` — create your org, get an API key, start storing memories.

### Debian / Ubuntu VM

```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/install.sh | sudo bash
```

Then edit `/etc/superdupermemory/env` with your API key and restart:

```bash
sudo systemctl restart superdupermemory
```

---

## HTTP API

The web server (`serve-web`) exposes a REST API under `/api/v1`.

### Authentication

| Header | Used for |
|--------|----------|
| `X-Admin-Token: <token>` | Org management (create apps, view stats) |
| `Authorization: Bearer <api_key>` | App-level memory operations |

### Multi-tenant model

```
Organization  ←  your company
  └── App      ←  your AI product (gets an API key)
        └── User  ←  your end user (identified by any string ID)
              └── Memories
```

### Endpoints

```
GET  /api/v1/health

POST /api/v1/orgs                          Create org → returns admin_token (once)
GET  /api/v1/orgs/:id/apps                 List apps
POST /api/v1/orgs/:id/apps                 Create app → returns api_key (once)
GET  /api/v1/orgs/:id/stats                Org-level stats

GET  /api/v1/apps/:id/users                List users + memory counts

POST /api/v1/memories                      Store memory for a user
GET  /api/v1/memories?user_id=&q=&limit=   Recall memories (semantic search)
DELETE /api/v1/memories/:id?user_id=       Delete a memory
```

### Store a memory

```bash
curl -X POST http://localhost:3000/api/v1/memories \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "alice@example.com", "text": "Alice is the head of engineering. She loves coffee."}'
```

Response:
```json
{
  "facts": [
    { "id": "...", "subject": "user.role", "body": "Alice is the head of engineering." },
    { "id": "...", "subject": "user.preference", "body": "Alice loves coffee." }
  ]
}
```

### Recall memories

```bash
curl "http://localhost:3000/api/v1/memories?user_id=alice@example.com&q=what+does+alice+like&limit=5" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

## MCP server (personal mode)

Add to your MCP client config:

```json
{
  "mcpServers": {
    "superdupermemory": {
      "command": "superdupermemory",
      "env": {
        "ANTHROPIC_API_KEY": "sk-ant-..."
      }
    }
  }
}
```

Or auto-install for Claude Code / Cursor:

```bash
superdupermemory install --claude-code
superdupermemory install --cursor
```

---

## Configuration

| Env var | Default | Description |
|---------|---------|-------------|
| `SDM_DB_PATH` | `~/.superdupermemory/memory.db` | SQLite database path |
| `SDM_HTTP_PORT` | `3000` | HTTP server port |
| `SDM_EXTRACTOR` | `anthropic` | Fact extractor: `anthropic` or `openai` |
| `SDM_EMBEDDER` | `local` | Embedder: `local` (fastembed) or `openai` |
| `ANTHROPIC_API_KEY` | — | Required when extractor=anthropic |
| `OPENAI_API_KEY` | — | Required when extractor/embedder=openai |
| `SDM_ENCRYPTION_KEY` | — | Optional AES-256-GCM key (hex) for at-rest encryption |

---

## Build from source

```bash
# Build the webapp first
cd webapp && npm install && npm run build && cd ..

# Build the binary (webapp is embedded)
cargo build --release --bin superdupermemory

# Run the HTTP server
ANTHROPIC_API_KEY=sk-ant-... ./target/release/superdupermemory serve-web
```

---

## CLI commands

```
superdupermemory                  Start MCP server (stdio)
superdupermemory serve-web        Start HTTP server + dashboard
superdupermemory install          Install MCP config for Claude Code / Cursor
superdupermemory inspect          List stored facts
superdupermemory stats            Database statistics
superdupermemory audit            Recent memory events
superdupermemory backup <dest>    Backup database
superdupermemory restore <src>    Restore database
superdupermemory prune --days 90  Delete stale facts
```

---

## Architecture

```
superdupermemory (single binary)
├── MCP server (stdio)       ← Claude Code, Cursor, Codex CLI
├── HTTP server (axum)       ← REST API + embedded React dashboard
│   ├── /api/v1/*            ← Multi-tenant memory API
│   └── /*                   ← React SPA (embedded at compile time)
├── crates/core              ← Fact extraction (Anthropic / OpenAI)
├── crates/store             ← SQLite storage + hybrid search (semantic + BM25)
├── crates/embed             ← Embeddings (fastembed local / OpenAI)
└── webapp/                  ← React + Vite + Tailwind dashboard
```

---

## License

Apache 2.0
