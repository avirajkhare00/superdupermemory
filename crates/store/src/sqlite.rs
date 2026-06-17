pub const SCHEMA_VERSION: u32 = 2;

use std::sync::{Arc, Mutex};

use anyhow::Context;
use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use superdupermemory_core::Fact;

use crate::{Cipher, MemoryStore};

// ── public types ───────────────────────────────────────────────────────────

pub struct AuditEntry {
    pub row_id: i64,
    pub event: String,
    pub fact_id: String,
    pub fact_subject: String,
    pub source: String,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

// ── store ──────────────────────────────────────────────────────────────────

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
    cipher: Option<Arc<Cipher>>,
}

impl SqliteStore {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        Self::open_with_cipher(path, None)
    }

    pub fn open_with_cipher(path: &str, cipher: Option<Cipher>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening SQLite database at {path}"))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            cipher: cipher.map(Arc::new),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::open_in_memory_with_cipher(None)
    }

    pub fn open_in_memory_with_cipher(cipher: Option<Cipher>) -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            cipher: cipher.map(Arc::new),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;

        // Base schema (v0).
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
            CREATE INDEX IF NOT EXISTS facts_updated_at ON facts(updated_at DESC);
            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '0');",
        )?;

        let version: u32 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        // v1 — access tracking.
        if version < 1 {
            let _ = conn.execute(
                "ALTER TABLE facts ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0",
                [],
            );
            let _ = conn.execute(
                "ALTER TABLE facts ADD COLUMN last_accessed_at TEXT",
                [],
            );
            conn.execute(
                "UPDATE meta SET value = '1' WHERE key = 'schema_version'",
                [],
            )?;
        }

        // v2 — audit log.
        if version < 2 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS audit_log (
                    rowid        INTEGER PRIMARY KEY AUTOINCREMENT,
                    event        TEXT NOT NULL,
                    fact_id      TEXT NOT NULL,
                    fact_subject TEXT NOT NULL,
                    source       TEXT NOT NULL,
                    occurred_at  TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS audit_occurred_at ON audit_log(occurred_at DESC);",
            )?;
            conn.execute(
                "UPDATE meta SET value = '2' WHERE key = 'schema_version'",
                [],
            )?;
        }

        Ok(())
    }

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

fn decrypt_fact(fact: Fact, cipher: Option<&Cipher>) -> anyhow::Result<Fact> {
    match cipher {
        None => Ok(fact),
        Some(c) => Ok(Fact {
            body: c.decrypt(&fact.body)?,
            previous_body: fact.previous_body.map(|pb| c.decrypt(&pb)).transpose()?,
            ..fact
        }),
    }
}

fn audit(conn: &Connection, event: &str, fact_id: &str, fact_subject: &str, source: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "INSERT INTO audit_log (event, fact_id, fact_subject, source, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event, fact_id, fact_subject, source, now],
    );
}

// ── durability helpers (sync, for CLI) ────────────────────────────────────

pub struct DbStats {
    pub total_facts: i64,
    pub facts_with_embeddings: i64,
    pub stale_facts: i64,
    pub schema_version: u32,
    pub audit_events: i64,
    pub db_size_bytes: u64,
}

impl SqliteStore {
    pub fn integrity_check(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let result: String =
            conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        anyhow::ensure!(result == "ok", "integrity check failed: {result}");
        Ok(())
    }

    pub fn backup_to(&self, dest_path: &str) -> anyhow::Result<u64> {
        let mut dest = Connection::open(dest_path)
            .with_context(|| format!("creating backup at {dest_path}"))?;
        let conn = self.conn.lock().unwrap();
        {
            let backup = rusqlite::backup::Backup::new(&*conn, &mut dest)
                .context("starting backup")?;
            backup
                .run_to_completion(500, std::time::Duration::from_millis(250), None)
                .context("running backup")?;
        }
        Ok(std::fs::metadata(dest_path)?.len())
    }

    pub fn restore_from(&self, src_path: &str) -> anyhow::Result<()> {
        let src = Connection::open(src_path)
            .with_context(|| format!("opening backup at {src_path}"))?;
        let mut conn = self.conn.lock().unwrap();
        let backup = rusqlite::backup::Backup::new(&src, &mut *conn)
            .context("starting restore")?;
        backup
            .run_to_completion(500, std::time::Duration::from_millis(250), None)
            .context("running restore")?;
        Ok(())
    }

    pub fn stats(&self, db_path: Option<&str>) -> anyhow::Result<DbStats> {
        let conn = self.conn.lock().unwrap();
        let total_facts: i64 =
            conn.query_row("SELECT COUNT(*) FROM facts", [], |r| r.get(0))?;
        let facts_with_embeddings: i64 = conn.query_row(
            "SELECT COUNT(*) FROM facts WHERE embedding IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        let stale_facts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM facts
             WHERE access_count = 0
               AND CAST(julianday('now') - julianday(created_at) AS INTEGER) > 30",
            [],
            |r| r.get(0),
        )?;
        let schema_version: u32 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let audit_events: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
            .unwrap_or(0);
        let db_size_bytes = db_path
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(DbStats {
            total_facts,
            facts_with_embeddings,
            stale_facts,
            schema_version,
            audit_events,
            db_size_bytes,
        })
    }

    pub async fn recent_audit(&self, limit: usize) -> anyhow::Result<Vec<AuditEntry>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT rowid, event, fact_id, fact_subject, source, occurred_at
                 FROM audit_log ORDER BY occurred_at DESC LIMIT ?1",
            )?;
            let entries: Vec<AuditEntry> = stmt
                .query_map(params![limit as i64], |row| {
                    let occurred_at: String = row.get(5)?;
                    Ok(AuditEntry {
                        row_id: row.get(0)?,
                        event: row.get(1)?,
                        fact_id: row.get(2)?,
                        fact_subject: row.get(3)?,
                        source: row.get(4)?,
                        occurred_at: chrono::DateTime::parse_from_rfc3339(&occurred_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    })
                })?
                .collect::<Result<_, _>>()?;
            Ok(entries)
        })
        .await
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
        let cipher = self.cipher.clone();
        self.run(move |conn| {
            let c = cipher.as_deref();
            let body = Cipher::maybe_encrypt(c, &fact.body)?;
            let previous_body = fact
                .previous_body
                .as_deref()
                .map(|pb| Cipher::maybe_encrypt(c, pb))
                .transpose()?;

            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM facts WHERE id = ?1)",
                    params![fact.id],
                    |r| r.get(0),
                )
                .unwrap_or(false);

            conn.execute(
                "INSERT INTO facts
                    (id, subject, body, source, previous_body, embedding, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                   body          = excluded.body,
                   source        = excluded.source,
                   previous_body = excluded.previous_body,
                   embedding     = COALESCE(excluded.embedding, facts.embedding),
                   updated_at    = excluded.updated_at",
                params![
                    fact.id, fact.subject, body, fact.source, previous_body, blob,
                    fact.created_at.to_rfc3339(), fact.updated_at.to_rfc3339(),
                ],
            )?;

            let event = if exists { "remember_update" } else { "remember_create" };
            audit(conn, event, &fact.id, &fact.subject, &fact.source);
            Ok(())
        })
        .await
    }

    async fn search_by_embedding(&self, embedding: &[f32], limit: usize) -> anyhow::Result<Vec<Fact>> {
        let query_vec = embedding.to_vec();
        let cipher = self.cipher.clone();
        self.run(move |conn| {
            let c = cipher.as_deref();
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
                let mut stmt2 = conn.prepare(
                    "SELECT id, subject, body, source, previous_body, created_at, updated_at
                     FROM facts ORDER BY updated_at DESC LIMIT ?1",
                )?;
                let fallback: Vec<Fact> = stmt2
                    .query_map(params![limit as i64], SqliteStore::row_to_fact)?
                    .collect::<Result<_, _>>()?;
                return fallback
                    .into_iter()
                    .map(|f| decrypt_fact(f, c))
                    .collect::<anyhow::Result<_>>();
            }

            rows.sort_by(|(_, a), (_, b)| {
                cosine_similarity(&query_vec, b)
                    .partial_cmp(&cosine_similarity(&query_vec, a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let top: Vec<Fact> = rows
                .into_iter()
                .take(limit)
                .map(|(f, _)| decrypt_fact(f, c))
                .collect::<anyhow::Result<_>>()?;

            let now = chrono::Utc::now().to_rfc3339();
            for fact in &top {
                let _ = conn.execute(
                    "UPDATE facts
                     SET access_count = access_count + 1, last_accessed_at = ?1
                     WHERE id = ?2",
                    params![now, fact.id],
                );
            }

            Ok(top)
        })
        .await
    }

    async fn search_by_text(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>> {
        let pattern = format!("%{query}%");
        let cipher = self.cipher.clone();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts
                 WHERE subject LIKE ?1 OR body LIKE ?1
                 ORDER BY updated_at DESC LIMIT ?2",
            )?;
            let rows: Vec<Fact> = stmt
                .query_map(params![pattern, limit as i64], SqliteStore::row_to_fact)?
                .collect::<Result<_, _>>()?;
            rows.into_iter()
                .map(|f| decrypt_fact(f, cipher.as_deref()))
                .collect::<anyhow::Result<_>>()
        })
        .await
    }

    async fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let id = id.to_string();
        self.run(move |conn| {
            let info: Option<(String, String)> = conn
                .query_row(
                    "SELECT subject, source FROM facts WHERE id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;

            let deleted = conn.execute("DELETE FROM facts WHERE id = ?1", params![id])? > 0;

            if deleted {
                if let Some((subject, source)) = info {
                    audit(conn, "forget", &id, &subject, &source);
                }
            }
            Ok(deleted)
        })
        .await
    }

    async fn get(&self, id: &str) -> anyhow::Result<Option<Fact>> {
        let id = id.to_string();
        let cipher = self.cipher.clone();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts WHERE id = ?1",
            )?;
            let row = stmt
                .query_map(params![id], SqliteStore::row_to_fact)?
                .next()
                .transpose()?;
            row.map(|f| decrypt_fact(f, cipher.as_deref())).transpose()
        })
        .await
    }

    async fn list(&self, limit: usize) -> anyhow::Result<Vec<Fact>> {
        let cipher = self.cipher.clone();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let rows: Vec<Fact> = stmt
                .query_map(params![limit as i64], SqliteStore::row_to_fact)?
                .collect::<Result<_, _>>()?;
            rows.into_iter()
                .map(|f| decrypt_fact(f, cipher.as_deref()))
                .collect::<anyhow::Result<_>>()
        })
        .await
    }
}
