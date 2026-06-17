use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use superdupermemory_core::Fact;
use superdupermemory_store::{MemoryStore, SqliteStore};

// ── parameter structs ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RememberParams {
    /// The text to store as a memory fact.
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
    store: std::sync::Arc<SqliteStore>,
}

impl MemoryServer {
    pub fn new(db_path: &str) -> anyhow::Result<Self> {
        let store = SqliteStore::open(db_path)?;
        Ok(Self {
            store: std::sync::Arc::new(store),
        })
    }
}

#[tool_router]
impl MemoryServer {
    /// Store a piece of information as a persistent memory fact.
    /// Returns the fact ID so the caller can reference it later.
    #[tool(description = "Store information as a persistent memory fact")]
    async fn remember(&self, Parameters(params): Parameters<RememberParams>) -> String {
        let source = params.source.unwrap_or_else(|| "mcp-tool".to_string());
        // Phase 0: store raw text as a single fact.
        // Phase 1 will route through AnthropicExtractor to split into discrete facts.
        let fact = Fact::new("raw.text", &params.text, &source);
        let id = fact.id.clone();
        match self.store.save(&fact).await {
            Ok(()) => format!("ok: stored {id}"),
            Err(e) => format!("error: {e}"),
        }
    }

    /// Search stored memory facts and return the best matches for a query.
    #[tool(description = "Retrieve stored memory facts matching a query")]
    async fn recall(&self, Parameters(params): Parameters<RecallParams>) -> String {
        let limit = params.limit.unwrap_or(10);
        match self.store.search_by_text(&params.query, limit).await {
            Ok(facts) if facts.is_empty() => "No matching facts found.".to_string(),
            Ok(facts) => facts
                .iter()
                .map(|f| {
                    format!(
                        "[{}] {} — {} (updated {})",
                        f.id, f.subject, f.body, f.updated_at
                    )
                })
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
