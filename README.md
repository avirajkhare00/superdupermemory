# superdupermemory

Local-first, Rust-native persistent memory for AI agents, exposed over [MCP](https://modelcontextprotocol.io/).

Agents are stateless between sessions. Every conversation re-establishes context the user has already provided. Superdupermemory fixes that: it extracts discrete facts from conversations, stores them in a local SQLite database, and makes them instantly searchable — so any MCP-compatible agent (Claude Code, Cursor, Codex CLI, etc.) can remember and recall what matters across sessions and process restarts.

## Features

- **Three MCP tools** — `remember`, `recall`, `forget`
- **Extraction pipeline** — LLM call turns raw conversational text into discrete, attributable facts (Anthropic or OpenAI, configurable)
- **Semantic search** — local embedding model (AllMiniLM-L6-v2 via fastembed) or OpenAI `text-embedding-3-small`, cosine similarity ranked
- **Update semantics** — same-subject facts are upserted; previous value is preserved in `previous_body`
- **Crash-safe storage** — SQLite with WAL mode, schema versioning, online backup/restore
- **Access tracking** — `access_count` and `last_accessed_at` updated on every recall, foundation for future decay scoring
- **Eval harness** — 28 deterministic, LLM-free test cases across 7 categories (basic recall, semantic recall, multi-fact, contradiction, forget, disambiguation, scale), with hit@1/hit@k/MRR/p50/p95 metrics and baseline comparison
- **Zero cloud dependency** — fully offline when using local embedder + local extractor

## Architecture

```
crates/
  core/    — Fact struct, Extractor trait, Anthropic + OpenAI extractors
  store/   — MemoryStore trait, SQLite implementation
  embed/   — Embedder trait, FastEmbedder (local) + OpenAIEmbedder
  server/  — MCP server binary with CLI subcommands
  eval/    — Deterministic eval harness
```

## Quick start

### Prerequisites

- Rust 1.78+
- An Anthropic or OpenAI API key (for fact extraction)

### Build

```sh
git clone https://github.com/avirajkhare00/superdupermemory
cd superdupermemory
cargo build --release
```

The binary is at `target/release/superdupermemory`.

### Configure

Create a `.env` file (or export these variables):

```sh
# Required for extraction (pick one)
ANTHROPIC_API_KEY=sk-ant-...
# or
OPENAI_API_KEY=sk-...
SDM_EXTRACTOR=openai          # default: anthropic

# Optional overrides
SDM_EXTRACTOR_MODEL=gpt-5.4-mini   # default depends on provider
SDM_EMBEDDER=openai                # default: local
SDM_DB_PATH=~/.superdupermemory/memory.db  # default
```

### Add to Claude Code

```sh
claude mcp add superdupermemory -- /path/to/superdupermemory serve
```

Or edit `~/.claude/settings.json` directly:

```json
{
  "mcpServers": {
    "superdupermemory": {
      "command": "/path/to/superdupermemory",
      "args": ["serve"]
    }
  }
}
```

## CLI

```
superdupermemory [--db <path>] <subcommand>

Subcommands:
  serve      Start the MCP server over stdio (default when no subcommand given)
  inspect    List recent facts stored in memory
  stats      Show database statistics
  backup     Online backup to a file (safe while server is running)
  restore    Restore from a backup file
  check      Run SQLite integrity check
```

Examples:

```sh
# Show what's stored
superdupermemory inspect --limit 50

# Database stats
superdupermemory stats

# Back up
superdupermemory backup ~/backups/memory-$(date +%Y%m%d).db

# Check integrity
superdupermemory check
```

## MCP tools

### `remember`

Extracts facts from free text and stores them.

```
remember(text: "I prefer Rust for systems work and Neovim as my editor")
```

### `recall`

Semantic search over stored facts.

```
recall(query: "what editor does the user prefer?", limit: 5)
```

### `forget`

Delete a fact by ID.

```
forget(id: "uuid-of-the-fact")
```

## Eval harness

Run the full eval suite (downloads the local embedding model on first run):

```sh
cargo run --bin sdm-eval

# Specific category
cargo run --bin sdm-eval -- --category disambiguation

# Save a baseline, then compare after changes
cargo run --bin sdm-eval -- --save
# ... make changes ...
cargo run --bin sdm-eval -- --compare

# JSON output
cargo run --bin sdm-eval -- --json
```

Categories: `basic_recall`, `semantic_recall`, `multi_fact`, `contradiction`, `forget`, `disambiguation`, `scale`

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `SDM_DB_PATH` | `~/.superdupermemory/memory.db` | SQLite database path |
| `SDM_EXTRACTOR` | `anthropic` | Extraction provider: `anthropic` or `openai` |
| `SDM_EXTRACTOR_MODEL` | provider default | Override the extraction model |
| `SDM_EMBEDDER` | `local` | Embedding provider: `local` or `openai` |
| `SDM_EMBEDDER_MODEL` | provider default | Override the embedding model |
| `ANTHROPIC_API_KEY` | — | Required when `SDM_EXTRACTOR=anthropic` |
| `OPENAI_API_KEY` | — | Required when `SDM_EXTRACTOR=openai` or `SDM_EMBEDDER=openai` |

> **Note:** Switching embedders on an existing database requires re-indexing. The local model (AllMiniLM-L6-v2) produces 384-dimensional vectors; OpenAI `text-embedding-3-small` produces 1536-dimensional vectors — they are not compatible.

## Roadmap

| Phase | Status | Focus |
|---|---|---|
| 0 | Done | Workspace scaffold, traits, MCP server compiles |
| 1 | Done | `remember` / `recall` / `forget` end-to-end pipeline |
| 2 | Done | Eval harness — 28 cases, metrics, baseline comparison |
| 3 | Done | Crash-safe storage, access tracking, CLI subcommands |
| 4 | Planned | Client integrations (Claude Code, Cursor, Codex CLI config) |
| 5 | Planned | Security hardening — encryption at rest, deletion audit, OSS release |
| 6 | Planned | Operational proof — latency/footprint/cost numbers, external validation |

## License

Apache-2.0 — see [LICENSE](LICENSE).
