use std::sync::Mutex;

use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::Embedder;

/// Local embedder using all-MiniLM-L6-v2 via fastembed.
/// Model weights are downloaded on first use and cached in ~/.cache/huggingface.
pub struct FastEmbedder {
    inner: Mutex<TextEmbedding>,
}

impl FastEmbedder {
    pub fn new() -> anyhow::Result<Self> {
        let inner = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }
}

#[async_trait]
impl Embedder for FastEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut results = self.embed_batch(&[text]).await?;
        results
            .pop()
            .ok_or_else(|| anyhow::anyhow!("empty embedding result"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        // fastembed is CPU-bound; run on a blocking thread to avoid starving the tokio runtime.
        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let embeddings = tokio::task::block_in_place(|| {
            self.inner.lock().unwrap().embed(owned, None)
        })?;
        Ok(embeddings)
    }
}
