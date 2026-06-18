pub mod crypto;
pub mod sqlite;
pub mod tenant;

use async_trait::async_trait;
use superdupermemory_core::Fact;

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn save(&self, fact: &Fact, embedding: Option<&[f32]>) -> anyhow::Result<()>;
    async fn search_by_embedding(&self, embedding: &[f32], limit: usize) -> anyhow::Result<Vec<Fact>>;
    async fn search_by_text(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>>;
    async fn delete(&self, id: &str) -> anyhow::Result<bool>;
    async fn get(&self, id: &str) -> anyhow::Result<Option<Fact>>;
    async fn list(&self, limit: usize) -> anyhow::Result<Vec<Fact>>;
}

pub use crypto::Cipher;
pub use sqlite::{AuditEntry, SqliteStore};
pub use tenant::{App, AppUser, Org, OrgStats, UserWithCount};
