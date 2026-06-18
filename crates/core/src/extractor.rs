use async_trait::async_trait;

use crate::Fact;

#[async_trait]
pub trait Extractor: Send + Sync {
    /// Turn raw conversational text into discrete, attributable facts.
    async fn extract(&self, text: &str, source: &str) -> anyhow::Result<Vec<Fact>>;
}

// ── shared ─────────────────────────────────────────────────────────────────

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Given a block of text, extract discrete facts about the user, their projects, preferences, or decisions.

If the text begins with a "[Session date: Month DD, YYYY]" header, use that date to resolve any relative time references (e.g. "yesterday", "last Friday", "last week", "next Monday") to their actual calendar dates before extracting facts. Include the resolved absolute date in the fact body when the timing is relevant.

Return a JSON array of objects. Each object must have:
- "subject": a short dot-separated label (e.g. "user.name", "project.goal", "preference.editor", "user.event")
- "body": a complete sentence stating the fact, with any relative dates resolved to absolute dates (e.g. "The user's name is Aviraj.", "Caroline went to an LGBTQ support group on May 7, 2023.")

Return ONLY the JSON array, no explanation."#;

fn parse_facts(json_text: &str, source: &str) -> anyhow::Result<Vec<Fact>> {
    let items: Vec<serde_json::Value> = serde_json::from_str(json_text)?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let subject = item["subject"].as_str()?;
            let body = item["body"].as_str()?;
            Some(Fact::new(subject, body, source))
        })
        .collect())
}

// ── Anthropic ──────────────────────────────────────────────────────────────

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

#[async_trait]
impl Extractor for AnthropicExtractor {
    async fn extract(&self, text: &str, source: &str) -> anyhow::Result<Vec<Fact>> {
        let payload = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": EXTRACTION_SYSTEM_PROMPT,
            "messages": [{ "role": "user", "content": text }]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let text = response["content"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("unexpected Anthropic response shape"))?;

        parse_facts(text, source)
    }
}

// ── OpenAI ─────────────────────────────────────────────────────────────────

pub struct OpenAIExtractor {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAIExtractor {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gpt-5.4-mini".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl Extractor for OpenAIExtractor {
    async fn extract(&self, text: &str, source: &str) -> anyhow::Result<Vec<Fact>> {
        // No response_format constraint — the system prompt asks for a bare JSON array,
        // and without json_object mode the model actually returns one.
        let payload = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": EXTRACTION_SYSTEM_PROMPT },
                { "role": "user",   "content": text }
            ]
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let raw = response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("unexpected OpenAI response shape: {response}"))?;

        // Strip markdown fences if the model wrapped the array.
        let stripped = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        parse_facts(stripped, source)
    }
}
