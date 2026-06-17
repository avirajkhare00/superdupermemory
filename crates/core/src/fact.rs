use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    /// Short label for what this fact is about (e.g. "user.name", "project.goal")
    pub subject: String,
    /// The actual statement, written as a complete sentence
    pub body: String,
    /// Where this fact came from (conversation id, tool name, etc.)
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Previous body value before the last update, if any
    pub previous_body: Option<String>,
}

impl Fact {
    pub fn new(
        subject: impl Into<String>,
        body: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            subject: subject.into(),
            body: body.into(),
            source: source.into(),
            created_at: now,
            updated_at: now,
            previous_body: None,
        }
    }

    pub fn update_body(&mut self, new_body: impl Into<String>) {
        let new_body = new_body.into();
        self.previous_body = Some(std::mem::replace(&mut self.body, new_body));
        self.updated_at = Utc::now();
    }
}
