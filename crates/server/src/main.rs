mod tools;

use std::sync::Arc;

use anyhow::Context;
use rmcp::{ServiceExt, transport::stdio};
use superdupermemory_core::extractor::{AnthropicExtractor, OpenAIExtractor};
use superdupermemory_embed::{Embedder, FastEmbedder, OpenAIEmbedder};
use tools::MemoryServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_path = std::env::var("SDM_DB_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.superdupermemory/memory.db")
    });

    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating data dir {}", parent.display()))?;
    }

    // ── extractor ────────────────────────────────────────────────────────────
    // SDM_EXTRACTOR=anthropic|openai  (default: anthropic)
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

    // ── embedder ─────────────────────────────────────────────────────────────
    // SDM_EMBEDDER=local|openai  (default: local)
    // WARNING: switching embedders on an existing DB requires re-indexing —
    // all-MiniLM-L6-v2 is 384-dim, OpenAI text-embedding-3-small is 1536-dim.
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
            // FastEmbedder loads an ONNX model on first call — do it on a blocking thread.
            let e = tokio::task::spawn_blocking(|| FastEmbedder::new())
                .await
                .context("embedder init panicked")??;
            Arc::new(e)
        }
    };

    let server = MemoryServer::new(&db_path, extractor, embedder)?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
