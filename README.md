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
- **Encryption at rest** — AES-256-GCM field encryption of fact bodies, activated via `SDM_ENCRYPTION_KEY`
- **Audit log** — every `remember` and `forget` event is logged with subject, source, and timestamp
- **One-command install** — `superdupermemory install` writes MCP config for Claude Code, Cursor, and Codex CLI
- **Offline benchmark** — `superdupermemory bench` measures insert rate and recall latency with no network calls
- **Eval harness** — 28 deterministic, LLM-free test cases across 7 categories with hit@1/hit@k/MRR/p50/p95 metrics
- **Zero cloud dependency** — fully offline when using local embedder + local extractor

## Architecture

```
crates/
  core/    — Fact struct, Extractor trait, Anthropic + OpenAI extractors
  store/   — MemoryStore trait, SQLite implementation, AES-256-GCM cipher
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
SDM_EXTRACTOR_MODEL=gpt-5.4-mini        # default depends on provider
SDM_EMBEDDER=openai                     # default: local
SDM_DB_PATH=~/.superdupermemory/memory.db  # default

# Optional: encrypt fact bodies at rest with AES-256-GCM
SDM_ENCRYPTION_KEY=<64 hex characters>  # generate with: openssl rand -hex 32
```

### Add to your agent

Run the install command to auto-write config for all supported clients:

```sh
superdupermemory install
```

Or target a specific client:

```sh
superdupermemory install --claude-code   # ~/.claude/settings.json
superdupermemory install --cursor        # ~/.cursor/mcp.json
superdupermemory install --codex         # prints YAML snippet to stdout
```

To add manually to Claude Code:

```sh
claude mcp add superdupermemory -- /path/to/superdupermemory serve
```

## CLI

```
superdupermemory [--db <path>] <subcommand>

Subcommands:
  serve      Start the MCP server over stdio (default when no subcommand given)
  install    Write MCP config for Claude Code, Cursor, and/or Codex CLI
  inspect    List recent facts stored in memory
  stats      Show database statistics
  audit      Show the audit log (remember/forget events)
  backup     Online backup to a file (safe while server is running)
  restore    Restore from a backup file
  check      Run SQLite integrity check
  bench      Run an insert + recall benchmark using the local embedder
  prune      Delete facts not accessed or updated within the last N days
```

Examples:

```sh
# Auto-install for all supported clients
superdupermemory install

# Show what's stored (decrypts if SDM_ENCRYPTION_KEY is set)
superdupermemory inspect --limit 50

# Show audit history
superdupermemory audit --limit 100

# Database stats
superdupermemory stats

# Back up
superdupermemory backup ~/backups/memory-$(date +%Y%m%d).db

# Benchmark (offline, no API calls)
superdupermemory bench --facts 200 --queries 50

# Prune facts untouched for 90 days (dry-run first)
superdupermemory prune --days 90 --dry-run
superdupermemory prune --days 90

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

## Encryption at rest

Set `SDM_ENCRYPTION_KEY` to a 64-character hex string (32 bytes) to enable AES-256-GCM encryption of fact `body` and `previous_body` fields before they are written to SQLite.

```sh
# Generate a key
openssl rand -hex 32

# Add to .env
SDM_ENCRYPTION_KEY=a3f1...
```

Encrypted values are stored with a `$enc$` prefix. Data stored before the key was set is read as plaintext and re-encrypted on the next update — there is no forced migration. Removing the key from the environment leaves encrypted rows unreadable until it is restored.

## Audit log

Every `remember` and `forget` call is logged to an `audit_log` table with the event type (`remember_create`, `remember_update`, `forget`), fact ID, subject, source, and timestamp. View it with:

```sh
superdupermemory audit --limit 50
```

## How the AI decides what to remember

Superdupermemory does not store anything automatically. The agent only remembers when it explicitly calls the `remember` tool — which means you control what gets stored by instructing the agent in your `CLAUDE.md`.

The recommended pattern is a standing instruction at the end of your `CLAUDE.md`:

```markdown
At the end of every session, call superdupermemory remember() with any new facts
learned about the user, their preferences, project decisions, or technical choices.
At the start of every session, call superdupermemory recall() to load relevant context.
```

With this in place:
- The agent loads what it already knows at session start
- It stores new facts it learned at session end
- You stay in control of what counts as "memory-worthy" — the agent judges this, not an automated trigger

**Why not store everything automatically?** Storing every file path, error message, and intermediate thought would flood the database with noise. The extraction pipeline already distills raw text into discrete facts — you still need the agent to decide *what text* is worth feeding it.

See [`CLAUDE.example.md`](CLAUDE.example.md) for a ready-to-use template.

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
| `SDM_ENCRYPTION_KEY` | — | 64 hex chars — enables AES-256-GCM encryption at rest |
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
| 4 | Done | `install` subcommand — auto-configure Claude Code, Cursor, Codex CLI |
| 5 | Done | AES-256-GCM encryption at rest, audit log |
| 6 | Done | `bench` subcommand — offline insert/recall benchmark with p50/p95 |

## License

Apache-2.0 — see [LICENSE](LICENSE).
