use std::sync::Arc;
use std::time::Instant;

use superdupermemory_embed::Embedder;
use superdupermemory_store::{MemoryStore, SqliteStore};

use crate::case::EvalCase;
use crate::metrics::CaseResult;

pub async fn run_case(
    case: &EvalCase,
    embedder: &Arc<dyn Embedder>,
) -> anyhow::Result<CaseResult> {
    // Each case gets its own isolated in-memory store.
    let store = SqliteStore::open_in_memory()?;

    // 1. Setup: embed and store each fact.
    for fact in &case.setup {
        let embed_input = format!("{}: {}", fact.subject, fact.body);
        let embedding = embedder.embed(&embed_input).await?;
        store.save(fact, Some(&embedding)).await?;
    }

    // 2. Apply deletes (for the Forget category).
    for id in &case.delete_ids {
        store.delete(id).await?;
    }

    // 3. Recall: embed the query, search, measure latency.
    let t0 = Instant::now();
    let query_embedding = embedder.embed(case.query).await?;
    let results = store.search_by_embedding(&query_embedding, case.k + 5).await?;
    let latency_ms = t0.elapsed().as_millis();

    // 4. Score.
    if case.expected_subjects.is_empty() {
        // Forget case: pass if nothing relevant is returned.
        let passed = results.is_empty()
            || !results.iter().any(|f| {
                // If any deleted fact subject shows up — fail.
                case.delete_ids.iter().any(|id| &f.id == id)
            });
        return Ok(CaseResult {
            name: case.name.to_string(),
            category: case.category.clone(),
            passed,
            rank: None,
            latency_ms,
        });
    }

    // Find the best (lowest) rank at which any expected subject appears.
    let rank = results
        .iter()
        .position(|f| case.expected_subjects.contains(&f.subject.as_str()))
        .map(|pos| pos + 1); // 1-based

    let passed = rank.map(|r| r <= case.k).unwrap_or(false);

    Ok(CaseResult {
        name: case.name.to_string(),
        category: case.category.clone(),
        passed,
        rank,
        latency_ms,
    })
}
