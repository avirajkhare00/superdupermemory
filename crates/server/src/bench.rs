use std::sync::Arc;
use std::time::Instant;

use superdupermemory_core::Fact;
use superdupermemory_embed::{Embedder, FastEmbedder};
use superdupermemory_store::{MemoryStore, SqliteStore};

static FACT_TEMPLATES: &[(&str, &str)] = &[
    ("tech.language",   "The user primarily programs in {}."),
    ("tech.editor",     "The user's preferred code editor is {}."),
    ("person.name",     "The person's name is {}."),
    ("person.city",     "The person lives in {}."),
    ("project.name",    "The active project is called {}."),
    ("person.hobby",    "The person's main hobby is {}."),
    ("tech.database",   "The preferred database technology is {}."),
    ("person.role",     "The person works as a {} engineer."),
    ("project.goal",    "The project {} aims to simplify local-first AI workflows."),
    ("tech.framework",  "The team uses {} as their primary web framework."),
    ("person.os",       "The user runs {} as their development operating system."),
    ("tech.cloud",      "The infrastructure is hosted on {}."),
];

static FILL_VALUES: &[&str] = &[
    "Rust", "Python", "TypeScript", "Go", "Java", "Kotlin", "Swift", "C++", "Zig", "Elixir",
    "Neovim", "VS Code", "Emacs", "IntelliJ", "Helix", "Zed",
    "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Hiroshi", "Ingrid",
    "Tokyo", "Berlin", "San Francisco", "London", "Bangalore", "Singapore", "Amsterdam",
    "SQLite", "PostgreSQL", "MySQL", "MongoDB", "Redis", "DynamoDB", "ClickHouse",
    "Axum", "Actix", "Rocket", "FastAPI", "Express", "Rails", "Phoenix",
    "macOS", "Arch Linux", "NixOS", "Ubuntu", "Fedora",
    "AWS", "GCP", "Fly.io", "Hetzner", "Render",
];

static QUERIES: &[&str] = &[
    "What programming language does the user prefer?",
    "Which code editor does the developer use?",
    "Where does this person live?",
    "What is the user's name?",
    "What database technology is being used?",
    "What is the main project goal?",
    "What web framework do they use?",
    "What is the person's role at work?",
    "What hobby does the user have?",
    "What cloud provider does the team use?",
    "What operating system does the developer run?",
    "What is the name of the active project?",
];

fn percentile(v: &mut [u128], p: usize) -> u128 {
    if v.is_empty() { return 0; }
    let idx = (p * v.len()).saturating_sub(1) / 100;
    v[idx]
}

pub async fn run(n_facts: usize, n_queries: usize) -> anyhow::Result<()> {
    println!("superdupermemory bench");
    println!("  facts: {}  queries: {}", n_facts, n_queries);
    println!();
    println!("Initialising local embedder (downloads model on first run)…");

    let embedder: Arc<dyn Embedder> = {
        let e = tokio::task::spawn_blocking(|| FastEmbedder::new())
            .await
            .expect("embedder thread panicked")?;
        Arc::new(e)
    };

    let store = SqliteStore::open_in_memory()?;

    // ── insert phase ────────────────────────────────────────────────────────
    println!("Inserting {} facts…", n_facts);
    let t_insert = Instant::now();
    let mut insert_latencies: Vec<u128> = Vec::with_capacity(n_facts);

    for i in 0..n_facts {
        let (subject_tmpl, body_tmpl) = FACT_TEMPLATES[i % FACT_TEMPLATES.len()];
        let fill = FILL_VALUES[i % FILL_VALUES.len()];
        let subject = format!("{subject_tmpl}.{i}");
        let body = body_tmpl.replace("{}", fill);
        let fact = Fact::new(&subject, &body, "bench");
        let embed_text = format!("{subject}: {body}");

        let t0 = Instant::now();
        let embedding = embedder.embed(&embed_text).await?;
        store.save(&fact, Some(&embedding)).await?;
        insert_latencies.push(t0.elapsed().as_millis());
    }

    let total_insert_ms = t_insert.elapsed().as_millis();
    let insert_rate = n_facts as f64 / (total_insert_ms as f64 / 1000.0);
    insert_latencies.sort_unstable();
    let insert_p50 = percentile(&mut insert_latencies, 50);
    let insert_p95 = percentile(&mut insert_latencies, 95);

    // ── query phase ─────────────────────────────────────────────────────────
    println!("Running {} recall queries…", n_queries);
    let mut query_latencies: Vec<u128> = Vec::with_capacity(n_queries);

    for i in 0..n_queries {
        let query = QUERIES[i % QUERIES.len()];
        let t0 = Instant::now();
        let emb = embedder.embed(query).await?;
        let _ = store.search_by_embedding(&emb, 5).await?;
        query_latencies.push(t0.elapsed().as_millis());
    }

    query_latencies.sort_unstable();
    let query_p50 = percentile(&mut query_latencies, 50);
    let query_p95 = percentile(&mut query_latencies, 95);
    let query_avg: f64 =
        query_latencies.iter().sum::<u128>() as f64 / query_latencies.len() as f64;

    // ── report ──────────────────────────────────────────────────────────────
    println!();
    println!("─── results ──────────────────────────────────────────────");
    println!("  insert  total  : {}ms  ({:.0} facts/sec)", total_insert_ms, insert_rate);
    println!("  insert  p50/p95: {}ms / {}ms", insert_p50, insert_p95);
    println!("  recall  queries: {}", n_queries);
    println!("  recall  avg    : {:.1}ms", query_avg);
    println!("  recall  p50    : {}ms", query_p50);
    println!("  recall  p95    : {}ms", query_p95);
    println!("  embedder       : AllMiniLM-L6-v2 (local, 384-dim)");
    println!("──────────────────────────────────────────────────────────");

    Ok(())
}
