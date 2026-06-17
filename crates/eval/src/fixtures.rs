use superdupermemory_core::Fact;

use crate::case::{Category, EvalCase};

pub fn all_cases() -> Vec<EvalCase> {
    let mut cases = vec![];
    cases.extend(basic_recall());
    cases.extend(semantic_recall());
    cases.extend(multi_fact());
    cases.extend(contradiction());
    cases.extend(forget());
    cases
}

// ── basic recall ───────────────────────────────────────────────────────────
// Store a fact, query with nearly identical text — expect hit@1.

fn basic_recall() -> Vec<EvalCase> {
    vec![
        EvalCase::new(
            "user name",
            Category::BasicRecall,
            vec![("user.name", "The user's name is Aviraj.")],
            "What is the user's name?",
            vec!["user.name"],
            1,
        ),
        EvalCase::new(
            "preferred language",
            Category::BasicRecall,
            vec![("user.language", "The user prefers Rust for systems programming.")],
            "What programming language does the user prefer?",
            vec!["user.language"],
            1,
        ),
        EvalCase::new(
            "project goal",
            Category::BasicRecall,
            vec![("project.goal", "The project goal is to build a local-first memory layer for AI agents.")],
            "What is the project goal?",
            vec!["project.goal"],
            1,
        ),
        EvalCase::new(
            "editor preference",
            Category::BasicRecall,
            vec![("preference.editor", "The user prefers Neovim as their code editor.")],
            "Which editor does the user use?",
            vec!["preference.editor"],
            1,
        ),
        EvalCase::new(
            "database choice",
            Category::BasicRecall,
            vec![("tech.database", "The project uses SQLite for local storage.")],
            "What database is used in the project?",
            vec!["tech.database"],
            1,
        ),
    ]
}

// ── semantic recall ────────────────────────────────────────────────────────
// Query with a paraphrase — tests embedding quality, not keyword match.

fn semantic_recall() -> Vec<EvalCase> {
    vec![
        EvalCase::new(
            "paraphrase: user name",
            Category::SemanticRecall,
            vec![("user.name", "The user's name is Aviraj.")],
            "Who am I talking to?",
            vec!["user.name"],
            3,
        ),
        EvalCase::new(
            "paraphrase: language preference",
            Category::SemanticRecall,
            vec![("user.language", "The user prefers Rust for systems programming.")],
            "What is their favourite tech stack?",
            vec!["user.language"],
            3,
        ),
        EvalCase::new(
            "paraphrase: project description",
            Category::SemanticRecall,
            vec![("project.goal", "The project goal is to build a local-first memory layer for AI agents.")],
            "Tell me about the software being built.",
            vec!["project.goal"],
            3,
        ),
        EvalCase::new(
            "paraphrase: interview status",
            Category::SemanticRecall,
            vec![("user.status", "The user is actively interviewing for senior engineering roles.")],
            "Is this person looking for a job?",
            vec!["user.status"],
            3,
        ),
        EvalCase::new(
            "paraphrase: OS preference",
            Category::SemanticRecall,
            vec![("preference.os", "The user runs macOS on Apple Silicon for development.")],
            "What machine do they develop on?",
            vec!["preference.os"],
            3,
        ),
    ]
}

// ── multi-fact retrieval ───────────────────────────────────────────────────
// Store several unrelated facts; query for one — expect it in top-3.

fn multi_fact() -> Vec<EvalCase> {
    let noise: Vec<(&str, &str)> = vec![
        ("user.hobby",     "The user enjoys rock climbing on weekends."),
        ("user.city",      "The user lives in Bangalore, India."),
        ("preference.os",  "The user runs macOS on Apple Silicon."),
        ("project.status", "The project is in active development."),
    ];

    fn with_noise(
        name: &'static str,
        target: (&'static str, &'static str),
        query: &'static str,
        noise: &[(&'static str, &'static str)],
    ) -> EvalCase {
        let mut setup: Vec<(&str, &str)> = noise.to_vec();
        setup.push(target);
        EvalCase::new(name, Category::MultiFact, setup, query, vec![target.0], 3)
    }

    vec![
        with_noise(
            "find project goal among noise",
            ("project.goal", "The project goal is to build a local-first memory layer for AI agents."),
            "What is the main objective of the project?",
            &noise,
        ),
        with_noise(
            "find language pref among noise",
            ("user.language", "The user prefers Rust for systems programming."),
            "What language does the user code in?",
            &noise,
        ),
        with_noise(
            "find editor among noise",
            ("preference.editor", "The user prefers Neovim as their code editor."),
            "Which text editor is preferred?",
            &noise,
        ),
        with_noise(
            "find database among noise",
            ("tech.database", "The project uses SQLite for local storage."),
            "What storage technology is being used?",
            &noise,
        ),
        with_noise(
            "find interview status among noise",
            ("user.status", "The user is actively interviewing for senior engineering roles."),
            "Is the user job hunting?",
            &noise,
        ),
    ]
}

// ── contradiction / update ─────────────────────────────────────────────────
// Store fact A; update with fact B on the same subject; expect B in results.

fn contradiction() -> Vec<EvalCase> {
    fn update_case(
        name: &'static str,
        subject: &'static str,
        old_body: &'static str,
        new_body: &'static str,
        query: &'static str,
    ) -> EvalCase {
        // Build two facts for the same subject; the second has the same ID so it
        // overwrites the first in the store.
        let old = Fact::new(subject, old_body, "eval");
        let mut new = Fact::new(subject, new_body, "eval");
        new.id = old.id.clone(); // same ID → upsert replaces old body
        new.previous_body = Some(old_body.to_string());

        EvalCase {
            name,
            category: Category::Contradiction,
            setup: vec![old, new],
            delete_ids: vec![],
            query,
            expected_subjects: vec![subject],
            k: 1,
        }
    }

    vec![
        update_case(
            "city updated",
            "user.city",
            "The user lives in Bangalore, India.",
            "The user has moved to Berlin, Germany.",
            "Where does the user live now?",
        ),
        update_case(
            "project status updated",
            "project.status",
            "The project is in early planning.",
            "The project is now in active development with Phase 1 complete.",
            "What stage is the project at?",
        ),
        update_case(
            "language preference updated",
            "user.language",
            "The user prefers Go for backend work.",
            "The user has switched to Rust as their primary language.",
            "What is the user's current primary language?",
        ),
    ]
}

// ── forget ─────────────────────────────────────────────────────────────────
// Store a fact, delete it, expect zero results for a targeted query.

fn forget() -> Vec<EvalCase> {
    vec![
        {
            let fact = Fact::new("user.secret", "The user's secret project is called Nova.", "eval");
            let id = fact.id.clone();
            EvalCase {
                name: "deleted fact not recalled",
                category: Category::Forget,
                setup: vec![fact],
                delete_ids: vec![id],
                query: "What is the user's secret project?",
                expected_subjects: vec![], // expect nothing — fact was deleted
                k: 5,
            }
        },
        {
            let keep  = Fact::new("user.city",   "The user lives in Bangalore.", "eval");
            let delete = Fact::new("user.secret", "The secret API key is abc123.", "eval");
            let del_id = delete.id.clone();
            EvalCase {
                name: "neighbour fact still recalled after delete",
                category: Category::Forget,
                setup: vec![keep, delete],
                delete_ids: vec![del_id],
                query: "Where does the user live?",
                expected_subjects: vec!["user.city"],
                k: 3,
            }
        },
    ]
}
