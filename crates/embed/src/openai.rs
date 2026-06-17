use async_trait::async_trait;

use crate::Embedder;

/// Calls the OpenAI Embeddings API.
/// Default model: text-embedding-3-small (1536 dims).
///
/// NOTE: dimensions differ from FastEmbedder (384 dims). If you switch embedders
/// on an existing database, re-index all facts — mixed vectors will give wrong results.
pub struct OpenAIEmbedder {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAIEmbedder {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "text-embedding-3-small".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut results = self.embed_batch(&[text]).await?;
        results
            .pop()
            .ok_or_else(|| anyhow::anyhow!("empty embedding result"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let payload = serde_json::json!({
            "model": self.model,
            "input": texts,
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let data = response["data"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("unexpected OpenAI embeddings response shape"))?;

        // OpenAI returns objects sorted by their original index.
        let embeddings = data
            .iter()
            .map(|item| {
                item["embedding"]
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("missing embedding array"))?
                    .iter()
                    .map(|v| {
                        v.as_f64()
                            .map(|f| f as f32)
                            .ok_or_else(|| anyhow::anyhow!("non-numeric value in embedding"))
                    })
                    .collect::<anyhow::Result<Vec<f32>>>()
            })
            .collect::<anyhow::Result<Vec<Vec<f32>>>>()?;

        Ok(embeddings)
    }
}
