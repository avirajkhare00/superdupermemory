mod tools;

use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use superdupermemory_core::extractor::{AnthropicExtractor, OpenAIExtractor};
use superdupermemory_embed::{Embedder, FastEmbedder, OpenAIEmbedder};
use superdupermemory_store::{MemoryStore, SqliteStore};
use tools::MemoryServer;

// ── CLI definition ─────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "superdupermemory", version, about = "Local-first memory layer for AI agents (MCP)")]
struct Cli {
    /// Path to the SQLite database file.
    #[arg(long, env = "SDM_DB_PATH", default_value = "~/.superdupermemory/memory.db")]
    db: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the MCP server over stdio (default when no subcommand given).
    Serve,

    /// List recent facts stored in memory.
    Inspect {
        /// Maximum number of facts to show.
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },

    /// Show database statistics.
    Stats,

    /// Back up the database to a file using SQLite's online backup API.
    Backup {
        /// Destination file path.
        dest: String,
    },

    /// Restore the database from a backup file.
    Restore {
        /// Source backup file path.
        src: String,
    },

    /// Run SQLite's integrity_check PRAGMA and report the result.
    Check,
}

// ── helpers ────────────────────────────────────────────────────────────────

fn resolve_db_path(raw: &str) -> String {
    if raw.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        raw.replacen('~', &home, 1)
    } else {
        raw.to_string()
    }
}

fn open_store(db_path: &str) -> anyhow::Result<SqliteStore> {
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating data dir {}", parent.display()))?;
    }
    SqliteStore::open(db_path)
}

// ── main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    let db_path = resolve_db_path(&cli.db);

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => run_serve(&db_path).await,
        Command::Inspect { limit } => run_inspect(&db_path, limit).await,
        Command::Stats => run_stats(&db_path),
        Command::Backup { dest } => run_backup(&db_path, &dest),
        Command::Restore { src } => run_restore(&db_path, &src),
        Command::Check => run_check(&db_path),
    }
}

// ── subcommand handlers ────────────────────────────────────────────────────

async fn run_serve(db_path: &str) -> anyhow::Result<()> {
    let extractor_kind = std::env::var("SDM_EXTRACTOR").unwrap_or_else(|_| "anthropic".into());

    let extractor: Arc<dyn superdupermemory_core::Extractor> = match extractor_kind.as_str() {
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY")
                .context("OPENAI_API_KEY must be set when SDM_EXTRACTOR=openai")?;
            let mut e = OpenAIExtractor::new(key);
            if let Ok(model) = std::env::var("SDM_EXTRACTOR_MODEL") {
                e = e.with_model(model);
            }
            Arc::new(e)
        }
        _ => {
            let key = std::env::var("ANTHROPIC_API_KEY")
                .context("ANTHROPIC_API_KEY must be set when SDM_EXTRACTOR=anthropic (default)")?;
            let mut e = AnthropicExtractor::new(key);
            if let Ok(model) = std::env::var("SDM_EXTRACTOR_MODEL") {
                e = e.with_model(model);
            }
            Arc::new(e)
        }
    };

    let embedder_kind = std::env::var("SDM_EMBEDDER").unwrap_or_else(|_| "local".into());

    let embedder: Arc<dyn Embedder> = match embedder_kind.as_str() {
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY")
                .context("OPENAI_API_KEY must be set when SDM_EMBEDDER=openai")?;
            let mut e = OpenAIEmbedder::new(key);
            if let Ok(model) = std::env::var("SDM_EMBEDDER_MODEL") {
                e = e.with_model(model);
            }
            Arc::new(e)
        }
        _ => {
            let e = tokio::task::spawn_blocking(|| FastEmbedder::new())
                .await
                .context("embedder init panicked")??;
            Arc::new(e)
        }
    };

    let server = MemoryServer::new(db_path, extractor, embedder)?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn run_inspect(db_path: &str, limit: usize) -> anyhow::Result<()> {
    let store = open_store(db_path)?;
    let facts = store.list(limit).await?;
    if facts.is_empty() {
        println!("No facts stored.");
        return Ok(());
    }
    println!("{:>4}  {:<30}  {:<20}  {}", "#", "subject", "updated_at", "body");
    println!("{}", "-".repeat(100));
    for (i, f) in facts.iter().enumerate() {
        let body_preview: String = f.body.chars().take(60).collect();
        let ellipsis = if f.body.len() > 60 { "…" } else { "" };
        println!(
            "{:>4}  {:<30}  {:<20}  {}{}",
            i + 1,
            &f.subject,
            f.updated_at.format("%Y-%m-%d %H:%M:%S"),
            body_preview,
            ellipsis
        );
    }
    Ok(())
}

fn run_stats(db_path: &str) -> anyhow::Result<()> {
    let store = open_store(db_path)?;
    let s = store.stats(Some(db_path))?;
    println!("superdupermemory database stats");
    println!("  schema version      : {}", s.schema_version);
    println!("  total facts         : {}", s.total_facts);
    println!("  facts with vectors  : {}", s.facts_with_embeddings);
    println!("  stale facts (>30d)  : {}", s.stale_facts);
    println!("  db size             : {} bytes ({:.1} KB)", s.db_size_bytes, s.db_size_bytes as f64 / 1024.0);
    Ok(())
}

fn run_backup(db_path: &str, dest: &str) -> anyhow::Result<()> {
    let store = open_store(db_path)?;
    let bytes = store.backup_to(dest)?;
    println!("Backup written to {} ({} bytes)", dest, bytes);
    Ok(())
}

fn run_restore(db_path: &str, src: &str) -> anyhow::Result<()> {
    let store = open_store(db_path)?;
    store.restore_from(src)?;
    println!("Restored from {}", src);
    Ok(())
}

fn run_check(db_path: &str) -> anyhow::Result<()> {
    let store = open_store(db_path)?;
    store.integrity_check()?;
    println!("Integrity check passed.");
    Ok(())
}
