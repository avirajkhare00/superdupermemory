# TypeScript SDK

## Installation

```bash
npm install superdupermemory
```

Works in Node.js 18+ and modern browsers (uses the native `fetch` API).

## Setup

```ts
import { SupduperMemory } from 'superdupermemory'

const mem = new SupduperMemory({
  baseUrl: 'http://localhost:3000',  // your server URL
  apiKey: 'YOUR_API_KEY',
})
```

## First-time org setup

Run this once when provisioning a new server:

```ts
import { createOrg, SupduperMemoryAdmin } from 'superdupermemory'

const { org, adminToken } = await createOrg({
  baseUrl: 'http://localhost:3000',
  name: 'Acme Corp',
  slug: 'acme',
})
// Save adminToken — shown only once

const admin = new SupduperMemoryAdmin({
  baseUrl: 'http://localhost:3000',
  adminToken,
  orgId: org.id,
})

const { app, apiKey } = await admin.createApp('Support Bot')
// Save apiKey — shown only once
```
