use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::case::Category;

#[derive(Debug)]
pub struct CaseResult {
    pub name: String,
    pub category: Category,
    pub passed: bool,
    /// 1-based position of first expected subject in results, or None.
    pub rank: Option<usize>,
    pub latency_ms: u128,
    pub k: usize,
}

impl CaseResult {
    pub fn hit_at_1(&self) -> bool {
        self.rank == Some(1)
    }

    pub fn reciprocal_rank(&self) -> f64 {
        self.rank.map(|r| 1.0 / r as f64).unwrap_or(0.0)
    }
}

// ── per-category summary ───────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CategorySummary {
    pub label: String,
    pub total: usize,
    pub hit_at_1: usize,
    pub hit_at_k: usize,
    pub mrr_sum: f64,
    pub latencies: Vec<u128>,
}

impl CategorySummary {
    pub fn hit_at_1_pct(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.hit_at_1 as f64 / self.total as f64 * 100.0 }
    }
    pub fn hit_at_k_pct(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.hit_at_k as f64 / self.total as f64 * 100.0 }
    }
    pub fn mrr(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.mrr_sum / self.total as f64 }
    }
    pub fn p50_ms(&self) -> u128 {
        percentile(&self.latencies, 0.50)
    }
    pub fn p95_ms(&self) -> u128 {
        percentile(&self.latencies, 0.95)
    }
}

fn percentile(sorted_or_not: &[u128], p: f64) -> u128 {
    if sorted_or_not.is_empty() { return 0; }
    let mut v = sorted_or_not.to_vec();
    v.sort_unstable();
    let idx = ((v.len() as f64 * p) as usize).min(v.len() - 1);
    v[idx]
}

// ── overall snapshot (saved to disk) ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Baseline {
    pub timestamp: String,
    pub categories: Vec<CategorySnapshot>,
    pub overall: OverallSnapshot,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CategorySnapshot {
    pub label: String,
    pub total: usize,
    pub hit_at_1_pct: f64,
    pub hit_at_k_pct: f64,
    pub mrr: f64,
    pub p50_ms: u128,
    pub p95_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OverallSnapshot {
    pub total: usize,
    pub hit_at_1_pct: f64,
    pub hit_at_k_pct: f64,
    pub mrr: f64,
    pub p50_ms: u128,
    pub p95_ms: u128,
}

// ── aggregation ────────────────────────────────────────────────────────────

pub fn summarise(results: &[CaseResult]) -> Vec<CategorySummary> {
    let mut map: HashMap<String, CategorySummary> = HashMap::new();
    for r in results {
        let entry = map.entry(r.category.label().to_string()).or_insert_with(|| CategorySummary {
            label: r.category.label().to_string(),
            ..Default::default()
        });
        entry.total += 1;
        if r.hit_at_1() { entry.hit_at_1 += 1; }
        if r.passed { entry.hit_at_k += 1; }
        entry.mrr_sum += r.reciprocal_rank();
        entry.latencies.push(r.latency_ms);
    }

    let order = ["basic_recall", "semantic_recall", "multi_fact", "contradiction",
                 "forget", "disambiguation", "scale"];
    order.iter().filter_map(|&k| map.remove(k)).collect()
}

pub fn to_baseline(summaries: &[CategorySummary]) -> Baseline {
    let timestamp = chrono::Utc::now().to_rfc3339();

    let all_latencies: Vec<u128> = summaries.iter().flat_map(|s| s.latencies.iter().copied()).collect();
    let total: usize = summaries.iter().map(|s| s.total).sum();
    let hit1: usize = summaries.iter().map(|s| s.hit_at_1).sum();
    let hitk: usize = summaries.iter().map(|s| s.hit_at_k).sum();
    let mrr_sum: f64 = summaries.iter().map(|s| s.mrr_sum).sum();

    let categories = summaries.iter().map(|s| CategorySnapshot {
        label: s.label.clone(),
        total: s.total,
        hit_at_1_pct: s.hit_at_1_pct(),
        hit_at_k_pct: s.hit_at_k_pct(),
        mrr: s.mrr(),
        p50_ms: s.p50_ms(),
        p95_ms: s.p95_ms(),
    }).collect();

    Baseline {
        timestamp,
        categories,
        overall: OverallSnapshot {
            total,
            hit_at_1_pct: if total == 0 { 0.0 } else { hit1 as f64 / total as f64 * 100.0 },
            hit_at_k_pct: if total == 0 { 0.0 } else { hitk as f64 / total as f64 * 100.0 },
            mrr: if total == 0 { 0.0 } else { mrr_sum / total as f64 },
            p50_ms: percentile(&all_latencies, 0.50),
            p95_ms: percentile(&all_latencies, 0.95),
        },
    }
}

// ── printing ───────────────────────────────────────────────────────────────

pub fn print_report(results: &[CaseResult], verbose: bool) {
    if verbose {
        println!("\n{:<35} {:<16} {:>6} {:>5}", "case", "category", "rank", "ms");
        println!("{}", "─".repeat(68));
        for r in results {
            let rank_str = r.rank.map(|x| x.to_string()).unwrap_or_else(|| "—".to_string());
            let status = if r.passed { "✓" } else { "✗" };
            println!("{} {:<34} {:<16} {:>6} {:>4}ms",
                status, r.name, r.category.label(), rank_str, r.latency_ms);
        }
    }

    let summaries = summarise(results);
    let all_latencies: Vec<u128> = summaries.iter().flat_map(|s| s.latencies.iter().copied()).collect();
    let total: usize = summaries.iter().map(|s| s.total).sum();
    let hit1: usize = summaries.iter().map(|s| s.hit_at_1).sum();
    let hitk: usize = summaries.iter().map(|s| s.hit_at_k).sum();
    let mrr_sum: f64 = summaries.iter().map(|s| s.mrr_sum).sum();

    println!("\n{}", "─".repeat(78));
    println!("{:<20} {:>5} {:>8} {:>8} {:>7} {:>7} {:>7}",
        "category", "n", "hit@1", "hit@k", "MRR", "p50ms", "p95ms");
    println!("{}", "─".repeat(78));

    for s in &summaries {
        println!("{:<20} {:>5} {:>7.0}% {:>7.0}% {:>7.3} {:>6}ms {:>6}ms",
            s.label, s.total,
            s.hit_at_1_pct(), s.hit_at_k_pct(),
            s.mrr(), s.p50_ms(), s.p95_ms());
    }

    println!("{}", "─".repeat(78));
    println!("{:<20} {:>5} {:>7.0}% {:>7.0}% {:>7.3} {:>6}ms {:>6}ms",
        "TOTAL", total,
        if total==0 {0.0} else {hit1 as f64/total as f64*100.0},
        if total==0 {0.0} else {hitk as f64/total as f64*100.0},
        if total==0 {0.0} else {mrr_sum/total as f64},
        percentile(&all_latencies, 0.50),
        percentile(&all_latencies, 0.95));
    println!("{}\n", "─".repeat(78));

    let failures: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    if failures.is_empty() {
        println!("All cases passed.");
    } else {
        println!("Failures ({}):", failures.len());
        for f in failures {
            println!("  ✗ [{}] {} (rank={:?})", f.category.label(), f.name, f.rank);
        }
    }
    println!();
}

pub fn compare_baseline(old: &Baseline, new: &[CategorySummary]) {
    println!("\nBaseline comparison (saved: {}):", old.timestamp);
    println!("{}", "─".repeat(70));
    println!("{:<20} {:>12} {:>12} {:>8}", "category", "old hit@k%", "new hit@k%", "delta");
    println!("{}", "─".repeat(70));

    let mut regressions = 0;
    for new_cat in new {
        if let Some(old_cat) = old.categories.iter().find(|c| c.label == new_cat.label) {
            let delta = new_cat.hit_at_k_pct() - old_cat.hit_at_k_pct;
            let flag = if delta < -5.0 { regressions += 1; " ← REGRESSION" } else { "" };
            println!("{:<20} {:>11.0}% {:>11.0}% {:>+7.1}%{}",
                new_cat.label, old_cat.hit_at_k_pct, new_cat.hit_at_k_pct(), delta, flag);
        } else {
            println!("{:<20} {:>12} {:>11.0}%  (new)", new_cat.label, "—", new_cat.hit_at_k_pct());
        }
    }
    println!("{}", "─".repeat(70));
    if regressions == 0 {
        println!("No regressions detected.");
    } else {
        println!("{} regression(s) detected (hit@k dropped >5%).", regressions);
    }
    println!();
}
