# SupduperMemoryAdmin

Admin client for managing orgs and apps. Requires an admin token.

```ts
import { SupduperMemoryAdmin } from 'superdupermemory'

const admin = new SupduperMemoryAdmin({
  baseUrl: 'http://localhost:3000',
  adminToken: 'YOUR_ADMIN_TOKEN',
  orgId: 'YOUR_ORG_ID',
})
```

---

## `listApps()`

```ts
const apps = await admin.listApps()
// → App[]
```

---

## `createApp(name)`

Creates a new app. The returned `apiKey` is shown only once.

```ts
const { app, apiKey } = await admin.createApp('Support Bot')
// Save apiKey!
```

---

## `stats()`

```ts
const stats = await admin.stats()
// → { total_apps: 2, total_users: 847, total_memories: 12043 }
```
