use std::collections::HashMap;

use crate::case::Category;

#[derive(Debug)]
pub struct CaseResult {
    pub name: String,
    pub category: Category,
    pub passed: bool,
    /// Position (1-based) of the first expected subject in results, or None.
    pub rank: Option<usize>,
    pub latency_ms: u128,
}

impl CaseResult {
    pub fn reciprocal_rank(&self) -> f64 {
        self.rank.map(|r| 1.0 / r as f64).unwrap_or(0.0)
    }
}

#[derive(Debug, Default)]
pub struct CategorySummary {
    pub label: String,
    pub total: usize,
    pub hits: usize,
    pub mrr_sum: f64,
    pub latency_sum: u128,
}

impl CategorySummary {
    pub fn hit_rate(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.hits as f64 / self.total as f64 }
    }
    pub fn mrr(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.mrr_sum / self.total as f64 }
    }
    pub fn avg_latency_ms(&self) -> u128 {
        if self.total == 0 { 0 } else { self.latency_sum / self.total as u128 }
    }
}

pub fn summarise(results: &[CaseResult]) -> Vec<CategorySummary> {
    let mut map: HashMap<String, CategorySummary> = HashMap::new();

    for r in results {
        let entry = map.entry(r.category.label().to_string()).or_insert_with(|| CategorySummary {
            label: r.category.label().to_string(),
            ..Default::default()
        });
        entry.total += 1;
        if r.passed { entry.hits += 1; }
        entry.mrr_sum += r.reciprocal_rank();
        entry.latency_sum += r.latency_ms;
    }

    // Return in a stable order matching fixture categories.
    let order = ["basic_recall", "semantic_recall", "multi_fact", "contradiction", "forget"];
    order.iter()
        .filter_map(|&k| map.remove(k))
        .collect()
}

pub fn print_report(results: &[CaseResult]) {
    let summaries = summarise(results);

    println!("\n{}", "─".repeat(70));
    println!("{:<20} {:>6} {:>8} {:>8} {:>12}", "category", "cases", "hit@k", "MRR", "avg ms");
    println!("{}", "─".repeat(70));

    let (mut total, mut hits, mut mrr_sum, mut lat_sum) = (0, 0, 0.0f64, 0u128);
    for s in &summaries {
        println!(
            "{:<20} {:>6} {:>7.0}% {:>8.3} {:>11}ms",
            s.label,
            s.total,
            s.hit_rate() * 100.0,
            s.mrr(),
            s.avg_latency_ms(),
        );
        total += s.total;
        hits += s.hits;
        mrr_sum += s.mrr_sum;
        lat_sum += s.latency_sum;
    }

    println!("{}", "─".repeat(70));
    let overall_lat = if total > 0 { lat_sum / total as u128 } else { 0 };
    println!(
        "{:<20} {:>6} {:>7.0}% {:>8.3} {:>11}ms",
        "TOTAL",
        total,
        if total > 0 { hits as f64 / total as f64 * 100.0 } else { 0.0 },
        if total > 0 { mrr_sum / total as f64 } else { 0.0 },
        overall_lat,
    );
    println!("{}\n", "─".repeat(70));

    // Print failures for debugging
    let failures: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    if failures.is_empty() {
        println!("All cases passed.");
    } else {
        println!("Failures ({}):", failures.len());
        for f in failures {
            println!("  [{}] {} — rank={:?}", f.category.label(), f.name, f.rank);
        }
    }
    println!();
}
