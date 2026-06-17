pub mod sqlite;

use async_trait::async_trait;
use superdupermemory_core::Fact;

#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Persist a fact. If a fact with the same subject already exists, update it.
    async fn save(&self, fact: &Fact) -> anyhow::Result<()>;

    /// Return all facts whose embedding is closest to `embedding`, up to `limit` results.
    /// Falls back to returning all facts ordered by recency when no embeddings are stored.
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
