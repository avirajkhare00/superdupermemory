# SupduperMemory

The main client for memory operations. Requires an app API key.

```ts
import { SupduperMemory } from 'superdupermemory'

const mem = new SupduperMemory({
  baseUrl: 'http://localhost:3000',
  apiKey: 'YOUR_API_KEY',
})
```

---

## `remember(opts)`

Extracts facts from text and stores them for a user.

```ts
const facts = await mem.remember({
  userId: 'alice@example.com',
  text: 'Alice is a senior engineer who loves Rust. She is working on the auth service.',
  source: 'chat',  // optional
})
// → Fact[]
```

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `userId` | `string` | yes | Your user's identifier |
| `text` | `string` | yes | Free-form text to extract facts from |
| `source` | `string` | no | Label for the text source |

---

## `recall(opts)`

Searches a user's memories using semantic + keyword search.

```ts
const facts = await mem.recall({
  userId: 'alice@example.com',
  query: 'what is alice working on?',
  limit: 5,
})
// → Fact[]
```

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `userId` | `string` | yes | Your user's identifier |
| `query` | `string` | no | Search query — omit to list most recent memories |
| `limit` | `number` | no | Max results (default 10, max 100) |

---

## `forget(opts)`

Deletes a specific memory.

```ts
const deleted = await mem.forget({
  userId: 'alice@example.com',
  factId: '4012ddf4-3901-4bc4-87f6-088db17aee24',
})
// → boolean
```

---

## `users(appId)`

Lists all users for an app with their memory counts. Useful for the dashboard.

```ts
const users = await mem.users('ebc53f79-...')
// → UserWithCount[]
```

---

## The `Fact` type

```ts
interface Fact {
  id: string
  subject: string       // e.g. "user.preference", "user.role"
  body: string          // e.g. "Alice loves coffee."
  source: string
  created_at: string
  updated_at: string
  previous_body: string | null
}
```
