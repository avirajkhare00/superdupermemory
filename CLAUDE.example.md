# CLAUDE.md — superdupermemory example

Copy this file to your project root as `CLAUDE.md` (or merge it into an existing one)
and adjust to fit your workflow.

## Memory

This project uses [superdupermemory](https://github.com/avirajkhare00/superdupermemory)
for persistent memory across sessions.

**At the start of every session**, call recall() before doing any work:

```
recall(query: "user preferences and ongoing project context", limit: 10)
```

**During the session**, note any preferences, decisions, or technical choices the user shares.

**At the end of every session**, call remember() with new facts learned. Good candidates:

- User preferences ("prefers Rust", "uses Neovim", "avoids classes in Python")
- Project decisions ("switched from Postgres to SQLite", "targeting macOS ARM64 only")
- Technical context ("the auth module lives in crates/auth", "deploy via Fly.io")
- Corrections ("user clarified that X is not Y")
- Goals and constraints ("must stay offline", "no external APIs in prod")

Example call:

```
remember(
  text: "Aviraj decided to use SQLite over Postgres for local-first simplicity.
         He prefers short, direct responses with no summaries at the end.",
  source: "claude-code-session"
)
```

Do not store:
- Raw error messages or stack traces
- Intermediate reasoning steps
- File contents or code snippets
- Anything the user explicitly said to forget

## Behaviour guidelines

- Load memory at session start, store at session end — do not call remember() after every single message
- When in doubt about whether something is worth remembering, store it; facts are cheap, lost context is expensive
- If the user says "remember this" or "don't forget", call remember() immediately
- If the user says "forget that", call forget() with the relevant fact ID from the last recall result
