pub mod fast;
pub mod openai;

use async_trait::async_trait;

#[async_trait]
pub trait Embedder: Send + Sync {
    /// Embed a piece of text, returning a dense float vector.
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// Embed multiple texts in one batch call.
    async fn embed_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;
}

pub use fast::FastEmbedder;
pub use openai::OpenAIEmbedder;
