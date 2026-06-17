use superdupermemory_core::Fact;

use crate::case::{Category, EvalCase};

pub fn all_cases() -> Vec<EvalCase> {
    let mut cases = vec![];
    cases.extend(basic_recall());
    cases.extend(semantic_recall());
    cases.extend(multi_fact());
    cases.extend(contradiction());
    cases.extend(forget());
    cases.extend(disambiguation());
    cases.extend(scale());
    cases
}

// ── basic recall ───────────────────────────────────────────────────────────

fn basic_recall() -> Vec<EvalCase> {
    vec![
        EvalCase::new("user name", Category::BasicRecall,
            vec![("user.name", "The user's name is Aviraj.")],
            "What is the user's name?", vec!["user.name"], 1),
        EvalCase::new("preferred language", Category::BasicRecall,
            vec![("user.language", "The user prefers Rust for systems programming.")],
            "What programming language does the user prefer?", vec!["user.language"], 1),
        EvalCase::new("project goal", Category::BasicRecall,
            vec![("project.goal", "The project goal is to build a local-first memory layer for AI agents.")],
            "What is the project goal?", vec!["project.goal"], 1),
        EvalCase::new("editor preference", Category::BasicRecall,
            vec![("preference.editor", "The user prefers Neovim as their code editor.")],
            "Which editor does the user use?", vec!["preference.editor"], 1),
        EvalCase::new("database choice", Category::BasicRecall,
            vec![("tech.database", "The project uses SQLite for local storage.")],
            "What database is used in the project?", vec!["tech.database"], 1),
    ]
}

// ── semantic recall ────────────────────────────────────────────────────────

fn semantic_recall() -> Vec<EvalCase> {
    vec![
        EvalCase::new("paraphrase: user name", Category::SemanticRecall,
            vec![("user.name", "The user's name is Aviraj.")],
            "Who am I talking to?", vec!["user.name"], 3),
        EvalCase::new("paraphrase: language preference", Category::SemanticRecall,
            vec![("user.language", "The user prefers Rust for systems programming.")],
            "What is their favourite tech stack?", vec!["user.language"], 3),
        EvalCase::new("paraphrase: project description", Category::SemanticRecall,
            vec![("project.goal", "The project goal is to build a local-first memory layer for AI agents.")],
            "Tell me about the software being built.", vec!["project.goal"], 3),
        EvalCase::new("paraphrase: interview status", Category::SemanticRecall,
            vec![("user.status", "The user is actively interviewing for senior engineering roles.")],
            "Is this person looking for a job?", vec!["user.status"], 3),
        EvalCase::new("paraphrase: OS preference", Category::SemanticRecall,
            vec![("preference.os", "The user runs macOS on Apple Silicon for development.")],
            "What machine do they develop on?", vec!["preference.os"], 3),
    ]
}

// ── multi-fact retrieval ───────────────────────────────────────────────────

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
        with_noise("find project goal among noise",
            ("project.goal", "The project goal is to build a local-first memory layer for AI agents."),
            "What is the main objective of the project?", &noise),
        with_noise("find language pref among noise",
            ("user.language", "The user prefers Rust for systems programming."),
            "What language does the user code in?", &noise),
        with_noise("find editor among noise",
            ("preference.editor", "The user prefers Neovim as their code editor."),
            "Which text editor is preferred?", &noise),
        with_noise("find database among noise",
            ("tech.database", "The project uses SQLite for local storage."),
            "What storage technology is being used?", &noise),
        with_noise("find interview status among noise",
            ("user.status", "The user is actively interviewing for senior engineering roles."),
            "Is the user job hunting?", &noise),
    ]
}

// ── contradiction / update ─────────────────────────────────────────────────

fn contradiction() -> Vec<EvalCase> {
    fn update_case(
        name: &'static str, subject: &'static str,
        old_body: &'static str, new_body: &'static str, query: &'static str,
    ) -> EvalCase {
        let old = Fact::new(subject, old_body, "eval");
        let mut new = Fact::new(subject, new_body, "eval");
        new.id = old.id.clone();
        new.previous_body = Some(old_body.to_string());
        EvalCase {
            name: name.to_string(),
            category: Category::Contradiction,
            setup: vec![old, new],
            delete_ids: vec![],
            query: query.to_string(),
            expected_subjects: vec![subject.to_string()],
            k: 1,
        }
    }

    vec![
        update_case("city updated",
            "user.city",
            "The user lives in Bangalore, India.",
            "The user has moved to Berlin, Germany.",
            "Where does the user live now?"),
        update_case("project status updated",
            "project.status",
            "The project is in early planning.",
            "The project is now in active development with Phase 1 complete.",
            "What stage is the project at?"),
        update_case("language preference updated",
            "user.language",
            "The user prefers Go for backend work.",
            "The user has switched to Rust as their primary language.",
            "What is the user's current primary language?"),
    ]
}

// ── forget ─────────────────────────────────────────────────────────────────

fn forget() -> Vec<EvalCase> {
    vec![
        {
            let fact = Fact::new("user.secret", "The user's secret project is called Nova.", "eval");
            let id = fact.id.clone();
            EvalCase {
                name: "deleted fact not recalled".to_string(),
                category: Category::Forget,
                setup: vec![fact],
                delete_ids: vec![id],
                query: "What is the user's secret project?".to_string(),
                expected_subjects: vec![],
                k: 5,
            }
        },
        {
            let keep   = Fact::new("user.city",   "The user lives in Bangalore.", "eval");
            let delete = Fact::new("user.secret", "The secret API key is abc123.", "eval");
            let del_id = delete.id.clone();
            EvalCase {
                name: "neighbour fact still recalled after delete".to_string(),
                category: Category::Forget,
                setup: vec![keep, delete],
                delete_ids: vec![del_id],
                query: "Where does the user live?".to_string(),
                expected_subjects: vec!["user.city".to_string()],
                k: 3,
            }
        },
    ]
}

// ── disambiguation ─────────────────────────────────────────────────────────
// Multiple facts of the same shape — must return the right one at k=1.

fn disambiguation() -> Vec<EvalCase> {
    vec![
        EvalCase::new("identify Carol among team members", Category::Disambiguation,
            vec![
                ("person.alice.role", "Alice is a frontend engineer who builds React dashboards."),
                ("person.bob.role",   "Bob is a backend engineer who writes Go microservices."),
                ("person.carol.role", "Carol is a data scientist who builds ML pipelines in Python."),
                ("person.dave.role",  "Dave is a DevOps engineer who manages Kubernetes clusters."),
                ("person.eve.role",   "Eve is a security engineer who runs penetration tests."),
            ],
            "What does Carol do at work?", vec!["person.carol.role"], 1),

        EvalCase::new("identify AI memory project among similar", Category::Disambiguation,
            vec![
                ("project.alpha", "Project Alpha is a real-time analytics dashboard for e-commerce."),
                ("project.beta",  "Project Beta is a fraud detection pipeline using gradient boosting."),
                ("project.gamma", "Project Gamma is a local-first memory layer for AI coding agents."),
                ("project.delta", "Project Delta is a mobile app for personal expense tracking."),
                ("project.epsilon", "Project Epsilon is a search engine for internal company documents."),
            ],
            "Which project is building memory for AI agents?", vec!["project.gamma"], 1),

        EvalCase::new("find product release deadline not beta", Category::Disambiguation,
            vec![
                ("deadline.alpha",   "The alpha testing phase ends on January 20th."),
                ("deadline.beta",    "The public beta launches on February 1st."),
                ("deadline.release", "The full product release is scheduled for March 15th."),
                ("deadline.eol",     "End-of-life for version 1 is planned for December 2027."),
            ],
            "When is the product going to be released to the public?", vec!["deadline.release"], 1),

        EvalCase::new("find backend tech not frontend", Category::Disambiguation,
            vec![
                ("tech.frontend", "The frontend uses React 18 with TypeScript and Tailwind CSS."),
                ("tech.backend",  "The backend API is written in Rust using the Axum framework."),
                ("tech.database", "The database is PostgreSQL 16 with the pgvector extension."),
                ("tech.cache",    "Redis is used for session storage and rate limiting."),
                ("tech.infra",    "Infrastructure runs on AWS ECS with Terraform for provisioning."),
            ],
            "What language and framework is the server-side API built with?",
            vec!["tech.backend"], 1),

        EvalCase::new("distinguish meeting pref from coding pref", Category::Disambiguation,
            vec![
                ("pref.work_hours", "The user prefers starting work at 7am to beat distractions."),
                ("pref.meetings",   "The user prefers to schedule all meetings between 2pm and 4pm."),
                ("pref.coding",     "The user prefers uninterrupted 4-hour deep work blocks in the morning."),
                ("pref.breaks",     "The user takes a 15-minute walk break every 90 minutes."),
            ],
            "When does the user like to hold meetings?", vec!["pref.meetings"], 1),
    ]
}

// ── scale ──────────────────────────────────────────────────────────────────
// One target fact buried in N noise facts. Tests ranking stability at scale.

static NOISE_POOL: &[(&str, &str)] = &[
    ("noise.hobby.01",    "The user enjoys rock climbing at an indoor gym on Saturdays."),
    ("noise.hobby.02",    "The user plays classical guitar and practises every evening."),
    ("noise.hobby.03",    "The user brews craft beer at home during the weekends."),
    ("noise.hobby.04",    "The user participates in local chess tournaments monthly."),
    ("noise.hobby.05",    "The user reads science fiction novels before going to sleep."),
    ("noise.food.01",     "The user's favourite cuisine is South Indian vegetarian food."),
    ("noise.food.02",     "The user orders pizza every Friday evening as a tradition."),
    ("noise.food.03",     "The user avoids dairy and follows a plant-based diet."),
    ("noise.food.04",     "The user's go-to comfort food is ramen with a soft-boiled egg."),
    ("noise.food.05",     "The user grows their own herbs: basil, mint, and rosemary."),
    ("noise.travel.01",   "The user's favourite travel destination is Kyoto, Japan."),
    ("noise.travel.02",   "The user visited Iceland last summer and loved the geysers."),
    ("noise.travel.03",   "The user has a goal to visit all seven continents by 2030."),
    ("noise.travel.04",   "The user prefers trains over planes for domestic travel."),
    ("noise.travel.05",   "The user always books window seats on flights for the view."),
    ("noise.health.01",   "The user runs 5km three times a week before work."),
    ("noise.health.02",   "The user does a 20-minute yoga routine every morning."),
    ("noise.health.03",   "The user tracks sleep with a smartwatch and targets 8 hours."),
    ("noise.health.04",   "The user drinks 3 litres of water per day."),
    ("noise.health.05",   "The user has a standing desk and alternates sitting/standing hourly."),
    ("noise.finance.01",  "The user invests 20% of their income into index funds monthly."),
    ("noise.finance.02",  "The user uses a zero-based budgeting approach."),
    ("noise.finance.03",  "The user has an emergency fund covering 6 months of expenses."),
    ("noise.finance.04",  "The user contributes the maximum allowed to their provident fund."),
    ("noise.finance.05",  "The user tracks all expenses in a spreadsheet every Sunday."),
    ("noise.tool.01",     "The user uses tmux to manage multiple terminal sessions."),
    ("noise.tool.02",     "The user's preferred terminal emulator is Alacritty."),
    ("noise.tool.03",     "The user manages dotfiles with a bare git repository."),
    ("noise.tool.04",     "The user uses Raycast instead of Spotlight on macOS."),
    ("noise.tool.05",     "The user writes all notes in Obsidian with a Zettelkasten structure."),
    ("noise.social.01",   "The user mentors two junior engineers from their previous company."),
    ("noise.social.02",   "The user organises a monthly tech reading group in their city."),
    ("noise.social.03",   "The user gives a talk at a local Rust meetup twice a year."),
    ("noise.social.04",   "The user contributes to open source on Sunday mornings."),
    ("noise.social.05",   "The user maintains a technical blog with fortnightly posts."),
    ("noise.career.01",   "The user has 8 years of professional software engineering experience."),
    ("noise.career.02",   "The user holds a bachelor's degree in computer science."),
    ("noise.career.03",   "The user previously worked at two Y-Combinator backed startups."),
    ("noise.career.04",   "The user was promoted to senior engineer after 18 months at their last job."),
    ("noise.career.05",   "The user is aiming for a staff engineering role within two years."),
    ("noise.pet.01",      "The user has a golden retriever named Charlie."),
    ("noise.pet.02",      "The user takes Charlie to the dog park every Sunday morning."),
    ("noise.media.01",    "The user's favourite podcast is Lex Fridman's show."),
    ("noise.media.02",    "The user watches Formula 1 races live even if they're at 1am."),
    ("noise.media.03",    "The user listens to lo-fi hip-hop while coding."),
    ("noise.media.04",    "The user recently finished re-reading The Pragmatic Programmer."),
    ("noise.media.05",    "The user follows Hacker News daily and saves interesting threads."),
    ("noise.env.01",      "The user keeps their desk completely clean except for their laptop."),
    ("noise.env.02",      "The user uses a 4K monitor positioned at arm's length."),
    ("noise.env.03",      "The user works from a home office with a mechanical keyboard."),
];

fn scale_case(
    name: &'static str,
    noise_count: usize,
    target: (&'static str, &'static str),
    query: &'static str,
    k: usize,
) -> EvalCase {
    let mut setup: Vec<(&str, &str)> = NOISE_POOL[..noise_count.min(NOISE_POOL.len())].to_vec();
    setup.push(target);
    EvalCase::new(name, Category::Scale, setup, query, vec![target.0], k)
}

fn scale() -> Vec<EvalCase> {
    vec![
        scale_case(
            "needle in 20 facts",
            20,
            ("project.goal", "The project goal is to build a local-first memory layer for AI agents."),
            "What is the main project the user is building?",
            3,
        ),
        scale_case(
            "needle in 50 facts",
            50,
            ("user.language", "The user's primary programming language is Rust."),
            "What programming language does the user primarily use?",
            3,
        ),
        scale_case(
            "needle in all facts",
            NOISE_POOL.len(),
            ("user.mcp_server", "The user is building an MCP server for persistent agent memory."),
            "What kind of server is the user building?",
            5,
        ),
    ]
}
