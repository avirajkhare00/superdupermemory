# Memories

All memory endpoints require `Authorization: Bearer <api_key>`.

Users are auto-created on first use — no need to register them upfront.

## Store a memory

`POST /api/v1/memories`

Sends text through the LLM extractor, creates facts, stores them with embeddings.

**Request**
```json
{
  "user_id": "alice@example.com",
  "text": "Alice is the head of engineering at Acme. She loves coffee and hates meetings before 10am.",
  "source": "chat"
}
```

`source` is optional — a label for where this text came from (e.g. `"chat"`, `"email"`, `"notes"`).

**Response** `201`
```json
{
  "facts": [
    { "id": "...", "subject": "user.name",       "body": "The user's name is Alice." },
    { "id": "...", "subject": "user.role",        "body": "Alice is the head of engineering at Acme." },
    { "id": "...", "subject": "user.preference",  "body": "Alice loves coffee." },
    { "id": "...", "subject": "user.preference",  "body": "Alice hates meetings before 10am." }
  ]
}
```

---

## Recall memories

`GET /api/v1/memories?user_id=&q=&limit=`

Searches a user's memories using hybrid semantic + keyword search.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `user_id` | yes | Your user's identifier |
| `q` | no | Natural-language search query. Omit to list most recent. |
| `limit` | no | Max results (default `10`, max `100`) |

**Example**
```bash
curl "http://localhost:3000/api/v1/memories?user_id=alice@example.com&q=what+does+alice+hate&limit=5" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

**Response** `200`
```json
{
  "facts": [
    { "id": "...", "subject": "user.preference", "body": "Alice hates meetings before 10am." },
    { "id": "...", "subject": "user.preference", "body": "Alice loves coffee." }
  ]
}
```

---

## Delete a memory

`DELETE /api/v1/memories/:id?user_id=`

| Parameter | Required | Description |
|-----------|----------|-------------|
| `user_id` | yes | Must match the user who owns this fact |

**Response** `200`
```json
{ "deleted": true }
```
