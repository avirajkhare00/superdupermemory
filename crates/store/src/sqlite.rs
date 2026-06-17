use std::sync::{Arc, Mutex};

use anyhow::Context;
use async_trait::async_trait;
use rusqlite::{Connection, params};
use superdupermemory_core::Fact;

use crate::MemoryStore;

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening SQLite database at {path}"))?;
        let store = Self { conn: Arc::new(Mutex::new(conn)) };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn: Arc::new(Mutex::new(conn)) };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS facts (
                id            TEXT PRIMARY KEY,
                subject       TEXT NOT NULL,
                body          TEXT NOT NULL,
                source        TEXT NOT NULL,
                previous_body TEXT,
                embedding     BLOB,
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS facts_subject    ON facts(subject);
            CREATE INDEX IF NOT EXISTS facts_updated_at ON facts(updated_at DESC);",
        )?;
        Ok(())
    }

    fn row_to_fact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Fact> {
        let created_at: String = row.get(5)?;
        let updated_at: String = row.get(6)?;
        Ok(Fact {
            id: row.get(0)?,
            subject: row.get(1)?,
            body: row.get(2)?,
            source: row.get(3)?,
            previous_body: row.get(4)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_default(),
            updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_default(),
        })
    }

    /// Run a synchronous closure on a blocking thread with exclusive DB access.
    async fn run<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&Connection) -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || f(&conn.lock().unwrap()))
            .await
            .context("db thread panicked")?
    }
}

// ── embedding helpers ──────────────────────────────────────────────────────

fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for &f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn bytes_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

// ── MemoryStore impl ───────────────────────────────────────────────────────

#[async_trait]
impl MemoryStore for SqliteStore {
    async fn save(&self, fact: &Fact, embedding: Option<&[f32]>) -> anyhow::Result<()> {
        let fact = fact.clone();
        let blob = embedding.map(f32_to_bytes);
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO facts (id, subject, body, source, previous_body, embedding, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                   body          = excluded.body,
                   source        = excluded.source,
                   previous_body = excluded.previous_body,
                   embedding     = COALESCE(excluded.embedding, facts.embedding),
                   updated_at    = excluded.updated_at",
                params![
                    fact.id,
                    fact.subject,
                    fact.body,
                    fact.source,
                    fact.previous_body,
                    blob,
                    fact.created_at.to_rfc3339(),
                    fact.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
        .await
    }

    async fn search_by_embedding(&self, embedding: &[f32], limit: usize) -> anyhow::Result<Vec<Fact>> {
        let query_vec = embedding.to_vec();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at, embedding
                 FROM facts WHERE embedding IS NOT NULL",
            )?;

            let mut rows: Vec<(Fact, Vec<f32>)> = stmt
                .query_map([], |row| {
                    let fact = SqliteStore::row_to_fact(row)?;
                    let blob: Vec<u8> = row.get(7)?;
                    Ok((fact, bytes_to_f32(&blob)))
                })?
                .collect::<Result<_, _>>()?;

            if rows.is_empty() {
                // No embeddings yet — fall back to recency order.
                let mut stmt2 = conn.prepare(
                    "SELECT id, subject, body, source, previous_body, created_at, updated_at
                     FROM facts ORDER BY updated_at DESC LIMIT ?1",
                )?;
                let facts = stmt2
                    .query_map(params![limit as i64], SqliteStore::row_to_fact)?
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(facts);
            }

            rows.sort_by(|(_, a), (_, b)| {
                let sa = cosine_similarity(&query_vec, a);
                let sb = cosine_similarity(&query_vec, b);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(rows.into_iter().take(limit).map(|(f, _)| f).collect())
        })
        .await
    }

    async fn search_by_text(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>> {
        let pattern = format!("%{query}%");
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts
                 WHERE subject LIKE ?1 OR body LIKE ?1
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )?;
            let facts = stmt
                .query_map(params![pattern, limit as i64], SqliteStore::row_to_fact)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(facts)
        })
        .await
    }

    async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let id = id.to_string();
        self.run(move |conn| {
            let rows = conn.execute("DELETE FROM facts WHERE id = ?1", params![id])?;
            Ok(rows > 0)
        })
        .await
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<Fact>> {
        let id = id.to_string();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map(params![id], SqliteStore::row_to_fact)?;
            Ok(rows.next().transpose()?)
        })
        .await
    }

    async fn list(&self, limit: usize) -> anyhow::Result<Vec<Fact>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let facts = stmt
                .query_map(params![limit as i64], SqliteStore::row_to_fact)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(facts)
        })
        .await
    }
}
