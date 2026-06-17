use superdupermemory_core::Fact;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Category {
    BasicRecall,
    SemanticRecall,
    MultiFact,
    Contradiction,
    Forget,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Self::BasicRecall => "basic_recall",
            Self::SemanticRecall => "semantic_recall",
            Self::MultiFact => "multi_fact",
            Self::Contradiction => "contradiction",
            Self::Forget => "forget",
        }
    }
}

/// A single eval case.
pub struct EvalCase {
    pub name: &'static str,
    pub category: Category,
    /// Facts to insert before querying.
    pub setup: Vec<Fact>,
    /// IDs of facts to delete after setup (for the Forget category).
    pub delete_ids: Vec<String>,
    /// The recall query.
    pub query: &'static str,
    /// Subjects that must appear in the top-k results for this case to pass.
    pub expected_subjects: Vec<&'static str>,
    pub k: usize,
}

impl EvalCase {
    pub fn new(
        name: &'static str,
        category: Category,
        setup: Vec<(&'static str, &'static str)>,
        query: &'static str,
        expected_subjects: Vec<&'static str>,
        k: usize,
    ) -> Self {
        let facts = setup
            .into_iter()
            .map(|(subj, body)| Fact::new(subj, body, "eval"))
            .collect();
        Self {
            name,
            category,
            setup: facts,
            delete_ids: vec![],
            query,
            expected_subjects,
            k,
        }
    }
}
