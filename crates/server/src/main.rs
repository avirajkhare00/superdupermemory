mod tools;

use std::sync::Arc;

use anyhow::Context;
use rmcp::{ServiceExt, transport::stdio};
use superdupermemory_core::extractor::AnthropicExtractor;
use superdupermemory_embed::FastEmbedder;
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

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY must be set for fact extraction")?;

    // FastEmbedder loads an ONNX model on first call — do it on a blocking thread.
    let embedder = tokio::task::spawn_blocking(|| FastEmbedder::new())
        .await
        .context("embedder init panicked")??;

    let extractor = AnthropicExtractor::new(api_key);

    let server = MemoryServer::new(&db_path, Arc::new(extractor), Arc::new(embedder))?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
