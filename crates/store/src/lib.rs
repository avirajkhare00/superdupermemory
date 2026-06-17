pub mod sqlite;

use async_trait::async_trait;
use superdupermemory_core::Fact;

#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Persist a fact with an optional embedding vector.
    /// If a fact with the same id already exists, it is updated in place.
    async fn save(&self, fact: &Fact, embedding: Option<&[f32]>) -> anyhow::Result<()>;

    /// Return facts whose stored embedding is closest to `embedding` (cosine similarity),
    /// up to `limit` results. Falls back to recency order when no embeddings are stored.
    async fn search_by_embedding(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<Fact>>;

    /// Full-text keyword search across subject + body, ordered by recency.
    async fn search_by_text(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>>;

    /// Delete a fact by its id. Returns true if a row was removed.
    async fn delete(&self, id: &str) -> anyhow::Result<bool>;

    /// Retrieve a single fact by id.
    async fn get(&self, id: &str) -> anyhow::Result<Option<Fact>>;

    /// List all facts ordered by updated_at descending.
    async fn list(&self, limit: usize) -> anyhow::Result<Vec<Fact>>;
}

pub use sqlite::SqliteStore;
