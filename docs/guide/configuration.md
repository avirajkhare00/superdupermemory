# Configuration

All configuration is via environment variables.

| Variable | Default | Description |
|----------|---------|-------------|
| `SDM_DB_PATH` | `~/.superdupermemory/memory.db` | Path to the SQLite database file |
| `SDM_HTTP_PORT` | `3000` | Port for the HTTP server (`serve-web`) |
| `SDM_EXTRACTOR` | `anthropic` | LLM for fact extraction: `anthropic` or `openai` |
| `SDM_EXTRACTOR_MODEL` | _(provider default)_ | Override the model used for extraction |
| `SDM_EMBEDDER` | `local` | Embedding backend: `local` (fastembed) or `openai` |
| `SDM_EMBEDDER_MODEL` | _(provider default)_ | Override the OpenAI embedding model |
| `ANTHROPIC_API_KEY` | — | Required when `SDM_EXTRACTOR=anthropic` |
| `OPENAI_API_KEY` | — | Required when `SDM_EXTRACTOR=openai` or `SDM_EMBEDDER=openai` |
| `SDM_ENCRYPTION_KEY` | — | AES-256-GCM key (64 hex chars) for at-rest encryption of fact bodies |

## Using OpenAI instead of Anthropic

```bash
SDM_EXTRACTOR=openai
SDM_EMBEDDER=openai   # optional — local embeddings work well without this
OPENAI_API_KEY=sk-...
```

## At-rest encryption

Generate a key and store it somewhere safe:

```bash
openssl rand -hex 32
# → 4a7f3b9c2d1e8a5f...
```

Set `SDM_ENCRYPTION_KEY=4a7f3b9c2d1e8a5f...`. All fact bodies will be encrypted in SQLite. Losing this key means losing access to all memories.

## Local embeddings

By default, superdupermemory uses [fastembed](https://github.com/Anush008/fastembed-rs) with the `all-MiniLM-L6-v2` model running locally. The model downloads automatically on first run (~22 MB). No API key needed.

This is the recommended setup — it keeps all data local and avoids per-token embedding costs.
