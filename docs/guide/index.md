# What is superdupermemory?

AI agents are stateless by default. Every conversation starts from scratch. superdupermemory gives your agents a persistent memory — facts extracted from conversations, stored locally, recalled with semantic search.

## How it works

1. **You send text** — a conversation turn, a user message, anything
2. **Facts are extracted** — an LLM reads the text and pulls out discrete facts (`user.name: Alice`, `user.preference: loves coffee`)
3. **Facts are embedded** — stored as vectors for semantic search (locally, using fastembed)
4. **You ask questions** — `"what does alice like?"` → returns relevant facts ranked by similarity + recency

## Two ways to use it

### Personal (MCP)

Plug it into Claude Code, Cursor, or any MCP-compatible client. Your AI assistant remembers things across sessions — project context, your preferences, past decisions.

### Multi-tenant (HTTP API)

Build AI products where every end-user gets their own persistent memory. You get an API key per app, your users are identified by any string (`user_id`), and memories are isolated between users.

## Why self-host?

mem0 and Supermemory are cloud services. They work great, but your users' memories leave your infra. For healthcare, legal, and fintech products — or any customer with strict data requirements — that's a dealbreaker.

superdupermemory runs on your AWS, GCP, or bare metal. We can help you set it up.
