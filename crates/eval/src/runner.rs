use std::sync::Arc;
use std::time::Instant;

use superdupermemory_embed::Embedder;
use superdupermemory_store::{MemoryStore, SqliteStore};

use crate::case::EvalCase;
use crate::metrics::CaseResult;

pub async fn run_case(case: &EvalCase, embedder: &Arc<dyn Embedder>) -> anyhow::Result<CaseResult> {
    let store = SqliteStore::open_in_memory()?;

    for fact in &case.setup {
        let text = format!("{}: {}", fact.subject, fact.body);
        let embedding = embedder.embed(&text).await?;
        store.save(fact, Some(&embedding)).await?;
    }

    for id in &case.delete_ids {
        store.delete(id).await?;
    }

    let t0 = Instant::now();
    let query_embedding = embedder.embed(&case.query).await?;
    let results = store.search_by_embedding(&query_embedding, case.k + 5).await?;
    let latency_ms = t0.elapsed().as_millis();

    // Forget cases: pass if none of the deleted IDs appear in results.
    if case.expected_subjects.is_empty() {
        let leaked = results.iter().any(|f| case.delete_ids.contains(&f.id));
        return Ok(CaseResult {
            name: case.name.clone(),
            category: case.category.clone(),
            passed: !leaked,
            rank: None,
            latency_ms,
            k: case.k,
        });
    }

    let rank = results
        .iter()
        .position(|f| case.expected_subjects.contains(&f.subject))
        .map(|p| p + 1);

    let passed = rank.map(|r| r <= case.k).unwrap_or(false);

    Ok(CaseResult {
        name: case.name.clone(),
        category: case.category.clone(),
        passed,
        rank,
        latency_ms,
        k: case.k,
    })
}
