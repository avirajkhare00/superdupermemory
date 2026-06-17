use superdupermemory_core::Fact;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Category {
    BasicRecall,
    SemanticRecall,
    MultiFact,
    Contradiction,
    Forget,
    /// Multiple similar facts — must pick the right one at k=1.
    Disambiguation,
    /// Large fact pool (20 / 50 / 100 noise facts) — tests ranking under scale.
    Scale,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Self::BasicRecall => "basic_recall",
            Self::SemanticRecall => "semantic_recall",
            Self::MultiFact => "multi_fact",
            Self::Contradiction => "contradiction",
            Self::Forget => "forget",
            Self::Disambiguation => "disambiguation",
            Self::Scale => "scale",
        }
    }
}

pub struct EvalCase {
    pub name: String,
    pub category: Category,
    pub setup: Vec<Fact>,
    pub delete_ids: Vec<String>,
    pub query: String,
    /// Subjects that must appear in top-k for this case to pass.
    pub expected_subjects: Vec<String>,
    pub k: usize,
}

impl EvalCase {
    pub fn new(
        name: impl Into<String>,
        category: Category,
        setup: Vec<(&str, &str)>,
        query: impl Into<String>,
        expected_subjects: Vec<&str>,
        k: usize,
    ) -> Self {
        let facts = setup
            .into_iter()
            .map(|(s, b)| Fact::new(s, b, "eval"))
            .collect();
        Self {
            name: name.into(),
            category,
            setup: facts,
            delete_ids: vec![],
            query: query.into(),
            expected_subjects: expected_subjects.into_iter().map(str::to_string).collect(),
            k,
        }
    }
}
