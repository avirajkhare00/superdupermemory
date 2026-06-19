# Orgs & Apps

## Create an org

`POST /api/v1/orgs`

No authentication required. Call this once during setup.

**Request**
```json
{ "name": "Acme Corp", "slug": "acme" }
```

**Response** `201`
```json
{
  "org": { "id": "...", "name": "Acme Corp", "slug": "acme", "created_at": "..." },
  "admin_token": "dIQ_nFeyTf..."
}
```

::: warning
`admin_token` is shown only once. Store it securely.
:::

---

## List apps

`GET /api/v1/orgs/:org_id/apps`

Requires `X-Admin-Token`.

**Response** `200`
```json
{ "apps": [{ "id": "...", "org_id": "...", "name": "Support Bot", "created_at": "..." }] }
```

---

## Create an app

`POST /api/v1/orgs/:org_id/apps`

Requires `X-Admin-Token`.

**Request**
```json
{ "name": "Support Bot" }
```

**Response** `201`
```json
{
  "app": { "id": "...", "org_id": "...", "name": "Support Bot", "created_at": "..." },
  "api_key": "9zuDO-Fnyf..."
}
```

::: warning
`api_key` is shown only once. Store it securely.
:::

---

## Org stats

`GET /api/v1/orgs/:org_id/stats`

Requires `X-Admin-Token`.

**Response** `200`
```json
{ "stats": { "total_apps": 2, "total_users": 847, "total_memories": 12043 } }
```

---

## List app users

`GET /api/v1/apps/:app_id/users`

Requires `Authorization: Bearer <api_key>`.

**Response** `200`
```json
{
  "users": [
    {
      "user": { "id": "...", "app_id": "...", "external_user_id": "alice@acme.com", "created_at": "..." },
      "memory_count": 4
    }
  ]
}
```
