# superdupermemory

Your AI agents forget everything the moment a conversation ends. superdupermemory fixes that.

It runs on your infra. Your data never leaves. One binary, no cloud dependency.

---

## What it does

Send it text. It extracts facts, embeds them, and stores them. Ask it a question later — it finds the right memories using hybrid semantic + keyword search.

```bash
# Store
curl -X POST http://localhost:3000/api/v1/memories \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{"user_id": "alice", "text": "Alice loves coffee and hates Monday meetings."}'

# Recall
curl "http://localhost:3000/api/v1/memories?user_id=alice&q=what+does+alice+hate"
```

That's it. Your AI now remembers Alice.

---

## Deploy in one line

**Docker:**
```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/docker-compose.yml -o docker-compose.yml
ANTHROPIC_API_KEY=sk-ant-... docker compose up -d
```

**Debian / Ubuntu:**
```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/install.sh | sudo bash
```

Open `http://localhost:3000`, create your org, ship.

---

## Two modes

**Personal** — plug into Claude Code, Cursor, or any MCP client. Your AI remembers things across sessions.

**Multi-tenant** — build AI products where every user gets their own persistent memory. Org → App → User → Memories.

---

## Why self-host?

mem0 and Supermemory are great. But they see your users' memories. Some customers — healthcare, legal, fintech — can't allow that. superdupermemory runs entirely on your AWS or GCP. We'll help you set it up.

---

## Built with

Rust · SQLite · fastembed (local embeddings, no API needed) · axum · React

---

## License

Apache 2.0 — free to use, self-host, and modify.
