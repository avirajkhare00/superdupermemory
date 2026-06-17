mod case;
mod fixtures;
mod metrics;
mod runner;

use std::sync::Arc;

use superdupermemory_embed::FastEmbedder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("superdupermemory eval harness");
    println!("Initialising embedder (downloads model on first run)…");

    let embedder: Arc<dyn superdupermemory_embed::Embedder> = {
        let e = tokio::task::spawn_blocking(|| FastEmbedder::new())
            .await
            .expect("embedder init panicked")?;
        Arc::new(e)
    };

    let cases = fixtures::all_cases();
    println!("Running {} eval cases…\n", cases.len());

    let mut results = Vec::with_capacity(cases.len());
    for case in &cases {
        let result = runner::run_case(case, &embedder).await?;
        let status = if result.passed { "PASS" } else { "FAIL" };
        println!(
            "[{}] [{:<15}] {}",
            status,
            case.category.label(),
            case.name
        );
        results.push(result);
    }

    metrics::print_report(&results);

    let failures = results.iter().filter(|r| !r.passed).count();
    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}
