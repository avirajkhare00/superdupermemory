mod case;
mod fixtures;
mod metrics;
mod runner;

use std::sync::Arc;

use superdupermemory_embed::FastEmbedder;

// ── CLI ────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Args {
    verbose: bool,
    category: Option<String>,
    json: bool,
    save: bool,
    compare: bool,
}

impl Args {
    fn parse() -> Self {
        let argv: Vec<String> = std::env::args().collect();
        let mut a = Self::default();
        let mut i = 1;
        while i < argv.len() {
            match argv[i].as_str() {
                "--verbose" | "-v" => a.verbose = true,
                "--json"           => a.json = true,
                "--save"           => a.save = true,
                "--compare"        => a.compare = true,
                "--category" | "-c" => {
                    i += 1;
                    a.category = argv.get(i).cloned();
                }
                "--help" | "-h" => {
                    println!("Usage: sdm-eval [OPTIONS]");
                    println!();
                    println!("  -v, --verbose          Print per-case rank and latency");
                    println!("  -c, --category <name>  Run only this category");
                    println!("      --json             Print full results as JSON");
                    println!("      --save             Save results as baseline");
                    println!("      --compare          Compare with saved baseline");
                    println!();
                    println!("Categories: basic_recall  semantic_recall  multi_fact");
                    println!("            contradiction  forget  disambiguation  scale");
                    std::process::exit(0);
                }
                _ => {}
            }
            i += 1;
        }
        a
    }
}

fn baseline_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".superdupermemory").join("eval-baseline.json")
}

// ── main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !args.json {
        println!("superdupermemory eval harness");
        println!("Initialising embedder (downloads model on first run)…");
    }

    let embedder: Arc<dyn superdupermemory_embed::Embedder> = {
        let e = tokio::task::spawn_blocking(|| FastEmbedder::new())
            .await
            .expect("embedder init panicked")?;
        Arc::new(e)
    };

    let mut cases = fixtures::all_cases();
    if let Some(ref cat) = args.category {
        cases.retain(|c| c.category.label() == cat.as_str());
        if cases.is_empty() {
            eprintln!("No cases found for category '{cat}'. Run with --help to list categories.");
            std::process::exit(1);
        }
    }

    if !args.json {
        println!("Running {} eval cases…\n", cases.len());
    }

    let mut results = Vec::with_capacity(cases.len());
    for case in &cases {
        let result = runner::run_case(case, &embedder).await?;
        if !args.json {
            let status = if result.passed { "PASS" } else { "FAIL" };
            println!("[{status}] [{:<15}] {}", case.category.label(), case.name);
        }
        results.push(result);
    }

    // ── output ─────────────────────────────────────────────────────────────
    if args.json {
        let summaries = metrics::summarise(&results);
        let baseline = metrics::to_baseline(&summaries);
        println!("{}", serde_json::to_string_pretty(&baseline)?);
    } else {
        metrics::print_report(&results, args.verbose);
    }

    // ── compare with saved baseline ────────────────────────────────────────
    if args.compare {
        let path = baseline_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let old: metrics::Baseline = serde_json::from_str(&content)?;
                let summaries = metrics::summarise(&results);
                metrics::compare_baseline(&old, &summaries);
            }
            Err(_) => eprintln!("No baseline found at {}. Run with --save first.", path.display()),
        }
    }

    // ── save baseline ──────────────────────────────────────────────────────
    if args.save {
        let path = baseline_path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        let summaries = metrics::summarise(&results);
        let baseline = metrics::to_baseline(&summaries);
        std::fs::write(&path, serde_json::to_string_pretty(&baseline)?)?;
        if !args.json {
            println!("Baseline saved to {}", path.display());
        }
    }

    // ── exit code ──────────────────────────────────────────────────────────
    let failures = results.iter().filter(|r| !r.passed).count();
    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}
