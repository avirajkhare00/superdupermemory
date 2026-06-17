use async_trait::async_trait;

use crate::Fact;

#[async_trait]
pub trait Extractor: Send + Sync {
    /// Turn raw conversational text into discrete, attributable facts.
    async fn extract(&self, text: &str, source: &str) -> anyhow::Result<Vec<Fact>>;
}

/// Calls the Anthropic Messages API to extract facts from text.
pub struct AnthropicExtractor {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicExtractor {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-haiku-4-5-20251001".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

const SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Given a block of text, extract discrete facts about the user, their projects, preferences, or decisions.

Return a JSON array of objects. Each object must have:
- "subject": a short dot-separated label (e.g. "user.name", "project.goal", "preference.editor")
- "body": a complete sentence stating the fact (e.g. "The user's name is Aviraj.")

Return ONLY the JSON array, no explanation."#;

#[async_trait]
impl Extractor for AnthropicExtractor {
    async fn extract(&self, text: &str, source: &str) -> anyhow::Result<Vec<Fact>> {
        let payload = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": SYSTEM_PROMPT,
            "messages": [
                { "role": "user", "content": text }
            ]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let content = response["content"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("unexpected Anthropic response shape"))?;

        let items: Vec<serde_json::Value> = serde_json::from_str(content)?;
        let facts = items
            .iter()
            .filter_map(|item| {
                let subject = item["subject"].as_str()?;
                let body = item["body"].as_str()?;
                Some(Fact::new(subject, body, source))
            })
            .collect();

        Ok(facts)
    }
}
