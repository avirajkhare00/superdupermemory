# Quick Start

## Option 1 — Docker (fastest)

```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/docker-compose.yml -o docker-compose.yml
ANTHROPIC_API_KEY=sk-ant-... docker compose up -d
```

Open [http://localhost:3000](http://localhost:3000).

## Option 2 — Debian / Ubuntu VM

```bash
curl -fsSL https://raw.githubusercontent.com/avirajkhare00/superdupermemory/master/install.sh | sudo bash
```

Edit `/etc/superdupermemory/env` with your API key, then:

```bash
sudo systemctl restart superdupermemory
```

## First steps in the dashboard

1. **Create your org** — click "First-time setup", pick a name and slug. Save the admin token somewhere safe.
2. **Create an app** — give it a name (e.g. "My Chatbot"). Save the API key.
3. **Store a memory**:

```bash
curl -X POST http://localhost:3000/api/v1/memories \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "alice", "text": "Alice is a senior engineer who loves Rust and hates standup meetings."}'
```

4. **Recall it**:

```bash
curl "http://localhost:3000/api/v1/memories?user_id=alice&q=what+does+alice+like" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

## Using the TypeScript SDK

```bash
npm install superdupermemory
```

```ts
import { SupduperMemory } from 'superdupermemory'

const mem = new SupduperMemory({
  baseUrl: 'http://localhost:3000',
  apiKey: 'YOUR_API_KEY',
})

await mem.remember({ userId: 'alice', text: 'Alice loves Rust and hates standup meetings.' })

const facts = await mem.recall({ userId: 'alice', query: 'what does alice hate?' })
console.log(facts)
```
