mod tools;

use anyhow::Context;
use rmcp::{ServiceExt, transport::stdio};
use tools::MemoryServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_path = std::env::var("SDM_DB_PATH").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.superdupermemory/memory.db")
    });

    // Ensure the data directory exists.
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating data dir {}", parent.display()))?;
    }

    let server = MemoryServer::new(&db_path)?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
