use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use superdupermemory_core::{Extractor, Fact};
use superdupermemory_embed::Embedder;
use superdupermemory_store::{Cipher, MemoryStore, SqliteStore};

// ── parameter structs ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RememberParams {
    /// The text to extract facts from and persist.
    pub text: String,
    /// Optional source label (e.g. "claude-code-session", "manual").
    pub source: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecallParams {
    /// Natural-language query to search stored facts.
    pub query: String,
    /// Maximum number of results to return (default: 10).
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ForgetParams {
    /// The id of the fact to delete, as returned by remember or recall.
    pub id: String,
}

// ── server ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MemoryServer {
    store: Arc<SqliteStore>,
    extractor: Arc<dyn Extractor>,
    embedder: Arc<dyn Embedder>,
}

impl MemoryServer {
    pub fn new(
        db_path: &str,
        extractor: Arc<dyn Extractor>,
        embedder: Arc<dyn Embedder>,
        cipher: Option<Cipher>,
    ) -> anyhow::Result<Self> {
        let store = SqliteStore::open_with_cipher(db_path, cipher)?;
        Ok(Self {
            store: Arc::new(store),
            extractor,
            embedder,
        })
    }
}

#[tool_router]
impl MemoryServer {
    /// Extract discrete facts from the provided text and store them with embeddings.
    /// Returns the IDs and subjects of all facts that were saved.
    #[tool(description = "Extract and store information as persistent memory facts")]
    async fn remember(&self, Parameters(params): Parameters<RememberParams>) -> String {
        let source = params.source.unwrap_or_else(|| "mcp-tool".to_string());

        // 1. Extract discrete facts from raw text.
        let facts = match self.extractor.extract(&params.text, &source).await {
            Ok(f) if !f.is_empty() => f,
            Ok(_) => {
                // Strip any "[Session date: ...]" header before storing as raw text
                // so bare date strings don't pollute recall results.
                let raw = params.text.trim();
                let body = if raw.starts_with("[Session date:") {
                    raw.lines().skip(1).collect::<Vec<_>>().join("\n")
                } else {
                    raw.to_string()
                };
                if body.trim().is_empty() {
                    return "ok: 0 fact(s) stored".to_string();
                }
                vec![Fact::new("raw.text", &body, &source)]
            }
            Err(e) => return format!("error: extraction failed — {e}"),
        };

        // 2. Embed and store each fact.
        let mut stored = Vec::with_capacity(facts.len());
        for fact in &facts {
            let embed_input = format!("{}: {}", fact.subject, fact.body);
            let embedding = match self.embedder.embed(&embed_input).await {
                Ok(v) => v,
                Err(e) => return format!("error: embedding failed — {e}"),
            };
            if let Err(e) = self.store.save(fact, Some(&embedding)).await {
                return format!("error: store failed — {e}");
            }
            stored.push(format!("{} ({})", fact.id, fact.subject));
        }

        format!("ok: {} fact(s) stored\n{}", stored.len(), stored.join("\n"))
    }

    /// Embed the query and return the closest stored memory facts by cosine similarity.
    #[tool(description = "Retrieve stored memory facts matching a query")]
    async fn recall(&self, Parameters(params): Parameters<RecallParams>) -> String {
        let limit = params.limit.unwrap_or(10);

        let embedding = match self.embedder.embed(&params.query).await {
            Ok(v) => v,
            Err(e) => return format!("error: embedding failed — {e}"),
        };

        match self.store.search_blended(&params.query, &embedding, limit).await {
            Ok(facts) if facts.is_empty() => "No matching facts found.".to_string(),
            Ok(facts) => facts
                .iter()
                .map(|f| format!("[{}] {}: {}", f.id, f.subject, f.body))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("error: {e}"),
        }
    }

    /// Delete a stored memory fact by its ID.
    #[tool(description = "Delete a stored memory fact by ID")]
    async fn forget(&self, Parameters(params): Parameters<ForgetParams>) -> String {
        match self.store.delete(&params.id).await {
            Ok(true) => format!("ok: deleted {}", params.id),
            Ok(false) => format!("not found: {}", params.id),
            Err(e) => format!("error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "superdupermemory",
                env!("CARGO_PKG_VERSION"),
            ))
    }
}
