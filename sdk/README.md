# superdupermemory

TypeScript SDK for [superdupermemory](https://github.com/avirajkhare00/superdupermemory) — a self-hosted, local-first memory layer for AI agents.

## Installation

```bash
npm install superdupermemory
```

## Quick start

```ts
import { SupduperMemory } from 'superdupermemory'

const memory = new SupduperMemory({
  baseUrl: 'http://localhost:3000',
  apiKey: 'your-app-api-key',
})

// Store a memory for a user
await memory.remember({
  userId: 'alice',
  text: 'Alice is a software engineer who prefers TypeScript over JavaScript.',
})

// Recall memories with semantic search
const facts = await memory.recall({
  userId: 'alice',
  query: 'what does alice prefer?',
})

console.log(facts[0].body)
// → "Alice prefers TypeScript over JavaScript."

// Delete a specific memory
await memory.forget({ userId: 'alice', factId: facts[0].id })
```

## Self-hosting

Run the server with Docker:

```bash
docker run -p 3000:3000 -v $(pwd)/data:/data \
  -e ANTHROPIC_API_KEY=sk-ant-... \
  ghcr.io/avirajkhare00/superdupermemory:latest
```

Or with Docker Compose — see the [self-hosting guide](https://avirajkhare00.github.io/superdupermemory/guide/self-hosting).

## API

### `new SupduperMemory(opts)`

Memory operations for a single app. Use one instance per app.

| Option | Type | Description |
|--------|------|-------------|
| `baseUrl` | `string` | URL of your superdupermemory server |
| `apiKey` | `string` | App API key from the dashboard |

#### `.remember(opts)` → `Promise<Fact[]>`

Extracts facts from free-form text and stores them for a user. Returns the list of facts saved.

```ts
const facts = await memory.remember({
  userId: 'alice',           // any string — email, UUID, username
  text: 'Alice loves hiking and her favourite food is pasta.',
  source: 'chat',            // optional label
})
```

#### `.recall(opts)` → `Promise<Fact[]>`

Semantic search over a user's memories. Omit `query` to list the most recent memories.

```ts
const facts = await memory.recall({
  userId: 'alice',
  query: "alice's hobbies",  // natural-language query
  limit: 5,                  // default 10, max 100
})
```

#### `.forget(opts)` → `Promise<boolean>`

Delete a specific memory by ID.

```ts
await memory.forget({ userId: 'alice', factId: 'fact-uuid' })
```

#### `.users(appId)` → `Promise<UserWithCount[]>`

List all users for an app along with their memory counts.

```ts
const users = await memory.users('app-uuid')
```

---

### `new SupduperMemoryAdmin(opts)`

Org and app management. Use the admin token returned when you created your org.

| Option | Type | Description |
|--------|------|-------------|
| `baseUrl` | `string` | URL of your superdupermemory server |
| `adminToken` | `string` | Admin token from org creation |
| `orgId` | `string` | Your org ID |

```ts
import { SupduperMemoryAdmin } from 'superdupermemory'

const admin = new SupduperMemoryAdmin({
  baseUrl: 'http://localhost:3000',
  adminToken: 'your-admin-token',
  orgId: 'your-org-id',
})

// Create an app — the apiKey is shown only once
const { app, apiKey } = await admin.createApp('my-chatbot')

// List all apps
const apps = await admin.listApps()

// Org-wide memory stats
const stats = await admin.stats()
```

---

### `createOrg(opts)` → `Promise<{ org, adminToken }>`

One-time setup. Creates a new organization and returns the admin token. **Store the admin token securely — it is shown only once.**

```ts
import { createOrg } from 'superdupermemory'

const { org, adminToken } = await createOrg({
  baseUrl: 'http://localhost:3000',
  name: 'Acme Corp',
  slug: 'acme',
})
```

## Types

```ts
interface Fact {
  id: string
  subject: string        // e.g. "user.preference.food"
  body: string           // e.g. "Alice's favourite food is pasta."
  source: string
  created_at: string
  updated_at: string
  previous_body: string | null
}
```

## License

Apache 2.0
