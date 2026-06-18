pub const SCHEMA_VERSION: u32 = 4;

use std::sync::{Arc, Mutex};

use anyhow::Context;
use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use superdupermemory_core::Fact;

use crate::{
    tenant::{App, AppUser, Org, OrgStats, UserWithCount},
    Cipher, MemoryStore,
};

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

        // v3 — FTS5 full-text index for BM25 retrieval.
        if version < 3 {
            conn.execute_batch(
                // External-content FTS5 table backed by facts.subject + facts.body.
                // Works correctly only on unencrypted databases; silently degrades
                // to semantic-only on encrypted ones (BM25 scores will be 0).
                "CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
                    subject, body,
                    content='facts',
                    content_rowid='rowid'
                );
                -- Keep FTS index in sync with the facts table.
                CREATE TRIGGER IF NOT EXISTS facts_fts_ai AFTER INSERT ON facts BEGIN
                    INSERT INTO facts_fts(rowid, subject, body)
                    VALUES (new.rowid, new.subject, new.body);
                END;
                CREATE TRIGGER IF NOT EXISTS facts_fts_ad AFTER DELETE ON facts BEGIN
                    INSERT INTO facts_fts(facts_fts, rowid, subject, body)
                    VALUES ('delete', old.rowid, old.subject, old.body);
                END;
                CREATE TRIGGER IF NOT EXISTS facts_fts_au AFTER UPDATE ON facts BEGIN
                    INSERT INTO facts_fts(facts_fts, rowid, subject, body)
                    VALUES ('delete', old.rowid, old.subject, old.body);
                    INSERT INTO facts_fts(rowid, subject, body)
                    VALUES (new.rowid, new.subject, new.body);
                END;
                -- Backfill existing facts into the FTS index.
                INSERT INTO facts_fts(facts_fts) VALUES ('rebuild');",
            )?;
            conn.execute(
                "UPDATE meta SET value = '3' WHERE key = 'schema_version'",
                [],
            )?;
        }

        // v4 — multi-tenancy: orgs, apps, app_users; scope facts to app_user.
        if version < 4 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS orgs (
                    id          TEXT PRIMARY KEY,
                    name        TEXT NOT NULL,
                    slug        TEXT NOT NULL UNIQUE,
                    admin_token TEXT NOT NULL UNIQUE,
                    created_at  TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS apps (
                    id        TEXT PRIMARY KEY,
                    org_id    TEXT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
                    name      TEXT NOT NULL,
                    api_key   TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS apps_org_id ON apps(org_id);
                CREATE TABLE IF NOT EXISTS app_users (
                    id               TEXT PRIMARY KEY,
                    app_id           TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
                    external_user_id TEXT NOT NULL,
                    created_at       TEXT NOT NULL,
                    UNIQUE(app_id, external_user_id)
                );
                CREATE INDEX IF NOT EXISTS app_users_app_id ON app_users(app_id);",
            )?;
            // ALTER TABLE cannot be done inside execute_batch on older SQLite
            let _ = conn.execute(
                "ALTER TABLE facts ADD COLUMN app_user_id TEXT REFERENCES app_users(id) ON DELETE CASCADE",
                [],
            );
            let _ = conn.execute(
                "CREATE INDEX IF NOT EXISTS facts_app_user_id ON facts(app_user_id)",
                [],
            );
            conn.execute(
                "UPDATE meta SET value = '4' WHERE key = 'schema_version'",
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

    /// Delete facts whose last relevant timestamp (last_accessed_at ?? updated_at) is older than
    /// `days` days. Returns the number of facts deleted.
    pub async fn purge_stale(&self, days: u64) -> anyhow::Result<usize> {
        self.run(move |conn| {
            let n = conn.execute(
                "DELETE FROM facts
                 WHERE CAST(julianday('now') - julianday(
                     COALESCE(last_accessed_at, updated_at)
                 ) AS INTEGER) > ?1",
                params![days as i64],
            )?;
            Ok(n)
        })
        .await
    }

    /// Hybrid search: 70 % cosine similarity + 20 % BM25 (FTS5) + 10 % recency.
    /// Falls back gracefully to semantic-only if the FTS5 index is unavailable.
    pub async fn search_blended(
        &self,
        query: &str,
        embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<Fact>> {
        let query_text = query.to_string();
        let query_vec = embedding.to_vec();
        let cipher = self.cipher.clone();

        self.run(move |conn| {
            let c = cipher.as_deref();

            // ── BM25 via FTS5 ──────────────────────────────────────────────
            let clean: String = query_text
                .chars()
                .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
                .collect();
            let words: Vec<String> = clean
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .map(|w| format!("\"{}\"", w))
                .collect();

            let mut bm25_map: std::collections::HashMap<i64, f32> =
                std::collections::HashMap::new();

            if !words.is_empty() {
                let fts_query = words.join(" OR ");
                // Best-effort: silently skip if FTS5 table is missing.
                let _ = (|| -> anyhow::Result<()> {
                    let mut stmt = conn.prepare(
                        "SELECT rowid, bm25(facts_fts) FROM facts_fts
                         WHERE facts_fts MATCH ?1 LIMIT ?2",
                    )?;
                    let pairs = stmt
                        .query_map(params![fts_query, (limit * 10) as i64], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    for (rowid, raw) in pairs {
                        // raw is negative; more negative = better match.
                        let abs = (-raw).max(0.0) as f32;
                        bm25_map.insert(rowid, abs / (abs + 1.0));
                    }
                    Ok(())
                })();
            }

            // ── Semantic: load all embedded facts ─────────────────────────
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body,
                        created_at, updated_at, embedding, last_accessed_at, rowid
                 FROM facts WHERE embedding IS NOT NULL AND app_user_id IS NULL",
            )?;

            let mut rows: Vec<(Fact, Vec<f32>, String, i64)> = stmt
                .query_map([], |row| {
                    let fact = SqliteStore::row_to_fact(row)?;
                    let blob: Vec<u8> = row.get(7)?;
                    let last_accessed: Option<String> = row.get(8)?;
                    let recency_date =
                        last_accessed.unwrap_or_else(|| fact.updated_at.to_rfc3339());
                    let rowid: i64 = row.get(9)?;
                    Ok((fact, bytes_to_f32(&blob), recency_date, rowid))
                })?
                .collect::<Result<_, _>>()?;

            if rows.is_empty() {
                let mut stmt2 = conn.prepare(
                    "SELECT id, subject, body, source, previous_body, created_at, updated_at
                     FROM facts WHERE app_user_id IS NULL ORDER BY updated_at DESC LIMIT ?1",
                )?;
                let fallback: Vec<Fact> = stmt2
                    .query_map(params![limit as i64], SqliteStore::row_to_fact)?
                    .collect::<Result<_, _>>()?;
                return fallback
                    .into_iter()
                    .map(|f| decrypt_fact(f, c))
                    .collect::<anyhow::Result<_>>();
            }

            // ── Blended sort: 70% cosine + 20% BM25 + 10% recency ─────────
            rows.sort_by(|(_, emb_a, rec_a, rid_a), (_, emb_b, rec_b, rid_b)| {
                let cos_a = cosine_similarity(&query_vec, emb_a);
                let cos_b = cosine_similarity(&query_vec, emb_b);
                let bm25_a = bm25_map.get(rid_a).copied().unwrap_or(0.0);
                let bm25_b = bm25_map.get(rid_b).copied().unwrap_or(0.0);
                let score_a = 0.70 * cos_a + 0.20 * bm25_a + 0.10 * recency_score(rec_a);
                let score_b = 0.70 * cos_b + 0.20 * bm25_b + 0.10 * recency_score(rec_b);
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let top: Vec<Fact> = rows
                .into_iter()
                .take(limit)
                .map(|(f, _, _, _)| decrypt_fact(f, c))
                .collect::<anyhow::Result<_>>()?;

            let now = chrono::Utc::now().to_rfc3339();
            for fact in &top {
                let _ = conn.execute(
                    "UPDATE facts SET access_count = access_count + 1,
                     last_accessed_at = ?1 WHERE id = ?2",
                    params![now, fact.id],
                );
            }

            Ok(top)
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

// Exponential recency decay: exp(-0.005 * days) → half-life ~139 days.
fn recency_score(date_str: &str) -> f32 {
    let parsed = chrono::DateTime::parse_from_rfc3339(date_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_default();
    let days = (chrono::Utc::now() - parsed).num_days().max(0) as f32;
    (-0.005_f32 * days).exp()
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
                "SELECT id, subject, body, source, previous_body, created_at, updated_at,
                        embedding, last_accessed_at
                 FROM facts WHERE embedding IS NOT NULL AND app_user_id IS NULL",
            )?;
            // (fact, embedding_vec, recency_date) — recency_date is last_accessed_at ?? updated_at
            let mut rows: Vec<(Fact, Vec<f32>, String)> = stmt
                .query_map([], |row| {
                    let fact = SqliteStore::row_to_fact(row)?;
                    let blob: Vec<u8> = row.get(7)?;
                    let last_accessed: Option<String> = row.get(8)?;
                    let recency_date = last_accessed
                        .unwrap_or_else(|| fact.updated_at.to_rfc3339());
                    Ok((fact, bytes_to_f32(&blob), recency_date))
                })?
                .collect::<Result<_, _>>()?;

            if rows.is_empty() {
                let mut stmt2 = conn.prepare(
                    "SELECT id, subject, body, source, previous_body, created_at, updated_at
                     FROM facts WHERE app_user_id IS NULL ORDER BY updated_at DESC LIMIT ?1",
                )?;
                let fallback: Vec<Fact> = stmt2
                    .query_map(params![limit as i64], SqliteStore::row_to_fact)?
                    .collect::<Result<_, _>>()?;
                return fallback
                    .into_iter()
                    .map(|f| decrypt_fact(f, c))
                    .collect::<anyhow::Result<_>>();
            }

            // Blended score: 85% cosine similarity + 15% recency decay.
            rows.sort_by(|(_, a, rec_a), (_, b, rec_b)| {
                let score_a = 0.85 * cosine_similarity(&query_vec, a) + 0.15 * recency_score(rec_a);
                let score_b = 0.85 * cosine_similarity(&query_vec, b) + 0.15 * recency_score(rec_b);
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            });

            let top: Vec<Fact> = rows
                .into_iter()
                .take(limit)
                .map(|(f, _, _)| decrypt_fact(f, c))
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
                 WHERE (subject LIKE ?1 OR body LIKE ?1) AND app_user_id IS NULL
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
                    "SELECT subject, source FROM facts WHERE id = ?1 AND app_user_id IS NULL",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;

            let deleted = conn.execute(
                "DELETE FROM facts WHERE id = ?1 AND app_user_id IS NULL",
                params![id],
            )? > 0;

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
                 FROM facts WHERE id = ?1 AND app_user_id IS NULL",
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
                 FROM facts WHERE app_user_id IS NULL ORDER BY updated_at DESC LIMIT ?1",
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

// ── token generation ───────────────────────────────────────────────────────

fn generate_token() -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use rand::Rng;
    let bytes: [u8; 24] = rand::thread_rng().gen();
    URL_SAFE_NO_PAD.encode(bytes)
}

// ── multi-tenant methods ───────────────────────────────────────────────────

impl SqliteStore {
    pub async fn create_org(&self, name: &str, slug: &str) -> anyhow::Result<(Org, String)> {
        let id = uuid::Uuid::new_v4().to_string();
        let admin_token = generate_token();
        let name = name.to_string();
        let slug = slug.to_string();
        let token_clone = admin_token.clone();
        let now = chrono::Utc::now().to_rfc3339();
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO orgs (id, name, slug, admin_token, created_at) VALUES (?1,?2,?3,?4,?5)",
                params![id, name, slug, token_clone, now],
            )?;
            Ok(())
        }).await?;
        let org = self.get_org_by_token(&admin_token).await?.unwrap();
        Ok((org, admin_token))
    }

    pub async fn get_org_by_token(&self, token: &str) -> anyhow::Result<Option<Org>> {
        let token = token.to_string();
        self.run(move |conn| {
            let row = conn.query_row(
                "SELECT id, name, slug, created_at FROM orgs WHERE admin_token = ?1",
                params![token],
                |r| {
                    let created_at: String = r.get(3)?;
                    Ok(Org {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        slug: r.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    })
                },
            ).optional()?;
            Ok(row)
        }).await
    }

    pub async fn get_org_by_id(&self, org_id: &str) -> anyhow::Result<Option<Org>> {
        let org_id = org_id.to_string();
        self.run(move |conn| {
            let row = conn.query_row(
                "SELECT id, name, slug, created_at FROM orgs WHERE id = ?1",
                params![org_id],
                |r| {
                    let created_at: String = r.get(3)?;
                    Ok(Org {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        slug: r.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    })
                },
            ).optional()?;
            Ok(row)
        }).await
    }

    pub async fn create_app(&self, org_id: &str, name: &str) -> anyhow::Result<(App, String)> {
        let id = uuid::Uuid::new_v4().to_string();
        let api_key = generate_token();
        let org_id = org_id.to_string();
        let name = name.to_string();
        let key_clone = api_key.clone();
        let now = chrono::Utc::now().to_rfc3339();
        let id_clone = id.clone();
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO apps (id, org_id, name, api_key, created_at) VALUES (?1,?2,?3,?4,?5)",
                params![id, org_id, name, key_clone, now],
            )?;
            Ok(())
        }).await?;
        let app = self.get_app_by_id(&id_clone).await?.unwrap();
        Ok((app, api_key))
    }

    pub async fn get_app_by_key(&self, api_key: &str) -> anyhow::Result<Option<App>> {
        let api_key = api_key.to_string();
        self.run(move |conn| {
            let row = conn.query_row(
                "SELECT id, org_id, name, created_at FROM apps WHERE api_key = ?1",
                params![api_key],
                |r| {
                    let created_at: String = r.get(3)?;
                    Ok(App {
                        id: r.get(0)?,
                        org_id: r.get(1)?,
                        name: r.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    })
                },
            ).optional()?;
            Ok(row)
        }).await
    }

    pub async fn get_app_by_id(&self, app_id: &str) -> anyhow::Result<Option<App>> {
        let app_id = app_id.to_string();
        self.run(move |conn| {
            let row = conn.query_row(
                "SELECT id, org_id, name, created_at FROM apps WHERE id = ?1",
                params![app_id],
                |r| {
                    let created_at: String = r.get(3)?;
                    Ok(App {
                        id: r.get(0)?,
                        org_id: r.get(1)?,
                        name: r.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    })
                },
            ).optional()?;
            Ok(row)
        }).await
    }

    pub async fn list_apps_for_org(&self, org_id: &str) -> anyhow::Result<Vec<App>> {
        let org_id = org_id.to_string();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, org_id, name, created_at FROM apps WHERE org_id = ?1 ORDER BY created_at",
            )?;
            let rows: Vec<App> = stmt.query_map(params![org_id], |r| {
                let created_at: String = r.get(3)?;
                Ok(App {
                    id: r.get(0)?,
                    org_id: r.get(1)?,
                    name: r.get(2)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_default(),
                })
            })?.collect::<Result<_, _>>()?;
            Ok(rows)
        }).await
    }

    pub async fn get_or_create_app_user(
        &self,
        app_id: &str,
        external_user_id: &str,
    ) -> anyhow::Result<AppUser> {
        let app_id = app_id.to_string();
        let external_user_id = external_user_id.to_string();
        self.run(move |conn| {
            let now = chrono::Utc::now().to_rfc3339();
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT OR IGNORE INTO app_users (id, app_id, external_user_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, app_id, external_user_id, now],
            )?;
            let row = conn.query_row(
                "SELECT id, app_id, external_user_id, created_at FROM app_users
                 WHERE app_id = ?1 AND external_user_id = ?2",
                params![app_id, external_user_id],
                |r| {
                    let created_at: String = r.get(3)?;
                    Ok(AppUser {
                        id: r.get(0)?,
                        app_id: r.get(1)?,
                        external_user_id: r.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    })
                },
            )?;
            Ok(row)
        }).await
    }

    pub async fn save_for_user(
        &self,
        fact: &superdupermemory_core::Fact,
        embedding: Option<&[f32]>,
        app_user_id: &str,
    ) -> anyhow::Result<()> {
        let fact = fact.clone();
        let blob = embedding.map(f32_to_bytes);
        let cipher = self.cipher.clone();
        let app_user_id = app_user_id.to_string();
        self.run(move |conn| {
            let c = cipher.as_deref();
            let body = Cipher::maybe_encrypt(c, &fact.body)?;
            let previous_body = fact
                .previous_body
                .as_deref()
                .map(|pb| Cipher::maybe_encrypt(c, pb))
                .transpose()?;

            conn.execute(
                "INSERT INTO facts
                    (id, subject, body, source, previous_body, embedding, created_at, updated_at, app_user_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   body          = excluded.body,
                   source        = excluded.source,
                   previous_body = excluded.previous_body,
                   embedding     = COALESCE(excluded.embedding, facts.embedding),
                   updated_at    = excluded.updated_at",
                params![
                    fact.id, fact.subject, body, fact.source, previous_body, blob,
                    fact.created_at.to_rfc3339(), fact.updated_at.to_rfc3339(), app_user_id,
                ],
            )?;
            Ok(())
        }).await
    }

    pub async fn search_blended_for_user(
        &self,
        query: &str,
        embedding: &[f32],
        limit: usize,
        app_user_id: &str,
    ) -> anyhow::Result<Vec<superdupermemory_core::Fact>> {
        let query_text = query.to_string();
        let query_vec = embedding.to_vec();
        let cipher = self.cipher.clone();
        let app_user_id = app_user_id.to_string();

        self.run(move |conn| {
            let c = cipher.as_deref();

            let clean: String = query_text
                .chars()
                .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
                .collect();
            let words: Vec<String> = clean
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .map(|w| format!("\"{}\"", w))
                .collect();

            let mut bm25_map: std::collections::HashMap<i64, f32> =
                std::collections::HashMap::new();

            if !words.is_empty() {
                let fts_query = words.join(" OR ");
                let _ = (|| -> anyhow::Result<()> {
                    let mut stmt = conn.prepare(
                        "SELECT f.rowid, bm25(facts_fts) FROM facts_fts
                         JOIN facts f ON facts_fts.rowid = f.rowid
                         WHERE facts_fts MATCH ?1 AND f.app_user_id = ?2 LIMIT ?3",
                    )?;
                    let pairs = stmt
                        .query_map(params![fts_query, app_user_id, (limit * 10) as i64], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    for (rowid, raw) in pairs {
                        let abs = (-raw).max(0.0) as f32;
                        bm25_map.insert(rowid, abs / (abs + 1.0));
                    }
                    Ok(())
                })();
            }

            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body,
                        created_at, updated_at, embedding, last_accessed_at, rowid
                 FROM facts WHERE embedding IS NOT NULL AND app_user_id = ?1",
            )?;

            let mut rows: Vec<(superdupermemory_core::Fact, Vec<f32>, String, i64)> = stmt
                .query_map(params![app_user_id], |row| {
                    let fact = SqliteStore::row_to_fact(row)?;
                    let blob: Vec<u8> = row.get(7)?;
                    let last_accessed: Option<String> = row.get(8)?;
                    let recency_date =
                        last_accessed.unwrap_or_else(|| fact.updated_at.to_rfc3339());
                    let rowid: i64 = row.get(9)?;
                    Ok((fact, bytes_to_f32(&blob), recency_date, rowid))
                })?
                .collect::<Result<_, _>>()?;

            if rows.is_empty() {
                let mut stmt2 = conn.prepare(
                    "SELECT id, subject, body, source, previous_body, created_at, updated_at
                     FROM facts WHERE app_user_id = ?1 ORDER BY updated_at DESC LIMIT ?2",
                )?;
                let fallback: Vec<superdupermemory_core::Fact> = stmt2
                    .query_map(params![app_user_id, limit as i64], SqliteStore::row_to_fact)?
                    .collect::<Result<_, _>>()?;
                return fallback
                    .into_iter()
                    .map(|f| decrypt_fact(f, c))
                    .collect::<anyhow::Result<_>>();
            }

            rows.sort_by(|(_, emb_a, rec_a, rid_a), (_, emb_b, rec_b, rid_b)| {
                let cos_a = cosine_similarity(&query_vec, emb_a);
                let cos_b = cosine_similarity(&query_vec, emb_b);
                let bm25_a = bm25_map.get(rid_a).copied().unwrap_or(0.0);
                let bm25_b = bm25_map.get(rid_b).copied().unwrap_or(0.0);
                let score_a = 0.70 * cos_a + 0.20 * bm25_a + 0.10 * recency_score(rec_a);
                let score_b = 0.70 * cos_b + 0.20 * bm25_b + 0.10 * recency_score(rec_b);
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            });

            let top: Vec<superdupermemory_core::Fact> = rows
                .into_iter()
                .take(limit)
                .map(|(f, _, _, _)| decrypt_fact(f, c))
                .collect::<anyhow::Result<_>>()?;

            let now = chrono::Utc::now().to_rfc3339();
            for fact in &top {
                let _ = conn.execute(
                    "UPDATE facts SET access_count = access_count + 1,
                     last_accessed_at = ?1 WHERE id = ?2",
                    params![now, fact.id],
                );
            }

            Ok(top)
        }).await
    }

    pub async fn list_for_user(
        &self,
        limit: usize,
        app_user_id: &str,
    ) -> anyhow::Result<Vec<superdupermemory_core::Fact>> {
        let app_user_id = app_user_id.to_string();
        let cipher = self.cipher.clone();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, body, source, previous_body, created_at, updated_at
                 FROM facts WHERE app_user_id = ?1 ORDER BY updated_at DESC LIMIT ?2",
            )?;
            let rows: Vec<superdupermemory_core::Fact> = stmt
                .query_map(params![app_user_id, limit as i64], SqliteStore::row_to_fact)?
                .collect::<Result<_, _>>()?;
            rows.into_iter()
                .map(|f| decrypt_fact(f, cipher.as_deref()))
                .collect::<anyhow::Result<_>>()
        }).await
    }

    pub async fn delete_for_user(&self, id: &str, app_user_id: &str) -> anyhow::Result<bool> {
        let id = id.to_string();
        let app_user_id = app_user_id.to_string();
        self.run(move |conn| {
            let deleted = conn.execute(
                "DELETE FROM facts WHERE id = ?1 AND app_user_id = ?2",
                params![id, app_user_id],
            )? > 0;
            Ok(deleted)
        }).await
    }

    pub async fn count_facts_for_user(&self, app_user_id: &str) -> anyhow::Result<i64> {
        let app_user_id = app_user_id.to_string();
        self.run(move |conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM facts WHERE app_user_id = ?1",
                params![app_user_id],
                |r| r.get(0),
            )?;
            Ok(n)
        }).await
    }

    pub async fn list_users_for_app(
        &self,
        app_id: &str,
    ) -> anyhow::Result<Vec<UserWithCount>> {
        let app_id = app_id.to_string();
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT u.id, u.app_id, u.external_user_id, u.created_at,
                        COUNT(f.id) as memory_count
                 FROM app_users u
                 LEFT JOIN facts f ON f.app_user_id = u.id
                 WHERE u.app_id = ?1
                 GROUP BY u.id
                 ORDER BY memory_count DESC",
            )?;
            let rows = stmt.query_map(params![app_id], |r| {
                let created_at: String = r.get(3)?;
                Ok(UserWithCount {
                    user: AppUser {
                        id: r.get(0)?,
                        app_id: r.get(1)?,
                        external_user_id: r.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                    },
                    memory_count: r.get(4)?,
                })
            })?.collect::<Result<_, _>>()?;
            Ok(rows)
        }).await
    }

    pub async fn org_stats(&self, org_id: &str) -> anyhow::Result<OrgStats> {
        let org_id = org_id.to_string();
        self.run(move |conn| {
            let total_apps: i64 = conn.query_row(
                "SELECT COUNT(*) FROM apps WHERE org_id = ?1",
                params![org_id],
                |r| r.get(0),
            )?;
            let total_users: i64 = conn.query_row(
                "SELECT COUNT(*) FROM app_users u JOIN apps a ON u.app_id = a.id WHERE a.org_id = ?1",
                params![org_id],
                |r| r.get(0),
            )?;
            let total_memories: i64 = conn.query_row(
                "SELECT COUNT(*) FROM facts f
                 JOIN app_users u ON f.app_user_id = u.id
                 JOIN apps a ON u.app_id = a.id
                 WHERE a.org_id = ?1",
                params![org_id],
                |r| r.get(0),
            )?;
            Ok(OrgStats { total_apps, total_users, total_memories })
        }).await
    }
}
