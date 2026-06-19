# REST API Overview

Base path: `/api/v1`

## Authentication

| Header | When to use |
|--------|-------------|
| `X-Admin-Token: <token>` | Org-level operations (create apps, view stats) |
| `Authorization: Bearer <api_key>` | App-level memory operations |

Both tokens are returned once at creation time and not stored in recoverable form — save them when you first see them.

## Errors

All errors return JSON:

```json
{ "error": "invalid api key" }
```

HTTP status codes follow standard conventions: `200/201` success, `400` bad request, `401/403` auth failure, `500` server error.

## Data model

```
Org
 └── App  (has api_key)
       └── User  (identified by your external_user_id)
             └── Fact  (a discrete piece of memory)
```

A **Fact** looks like:

```json
{
  "id": "4012ddf4-...",
  "subject": "user.preference",
  "body": "Alice loves coffee.",
  "source": "api",
  "created_at": "2026-06-18T16:51:25Z",
  "updated_at": "2026-06-18T16:51:25Z",
  "previous_body": null
}
```
