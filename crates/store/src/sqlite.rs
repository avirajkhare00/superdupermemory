use std::sync::Mutex;

use anyhow::Context;
use async_trait::async_trait;
use rusqlite::{Connection, params};
use superdupermemory_core::Fact;

use crate::MemoryStore;

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening SQLite database at {path}"))?;
        let store = Self { conn: Mutex::new(conn) };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn: Mutex::new(conn) };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS facts (
                id           TEXT PRIMARY KEY,
                subject      TEXT NOT NULL,
                body         TEXT NOT NULL,
                source       TEXT NOT NULL,
                previous_body TEXT,
                embedding    BLOB,
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS facts_subject ON facts(subject);
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
}

#[async_trait]
impl MemoryStore for SqliteStore {
    async fn save(&self, fact: &Fact) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        // Upsert: if a fact with the same subject exists, update it; otherwise insert.
        conn.execute(
            "INSERT INTO facts (id, subject, body, source, previous_body, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               body          = excluded.body,
               source        = excluded.source,
               previous_body = excluded.previous_body,
               updated_at    = excluded.updated_at",
            params![
                fact.id,
                fact.subject,
                fact.body,
                fact.source,
                fact.previous_body,
                fact.created_at.to_rfc3339(),
                fact.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    async fn search_by_embedding(
        &self,
        _embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<Fact>> {
        // Vector search not yet wired — fall back to recency order.
        self.list(limit).await
    }

    async fn search_by_text(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{query}%");
        let mut stmt = conn.prepare(
            "SELECT id, subject, body, source, previous_body, created_at, updated_at
             FROM facts
             WHERE subject LIKE ?1 OR body LIKE ?1
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let facts = stmt
            .query_map(params![pattern, limit as i64], Self::row_to_fact)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(facts)
    }

    async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM facts WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<Fact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, subject, body, source, previous_body, created_at, updated_at
             FROM facts WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], Self::row_to_fact)?;
        Ok(rows.next().transpose()?)
    }

    async fn list(&self, limit: usize) -> anyhow::Result<Vec<Fact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, subject, body, source, previous_body, created_at, updated_at
             FROM facts ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let facts = stmt
            .query_map(params![limit as i64], Self::row_to_fact)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(facts)
    }
}
