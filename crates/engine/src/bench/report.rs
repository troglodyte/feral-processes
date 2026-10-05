//! The shapes a bench run reports, and the pure folds that build and read
//! them. Nothing here touches a `Game`, so the arithmetic is testable
//! without running a base.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Every prefix `BenchReport::measure` accepts, for error messages and for
/// an objective file's load-time check.
pub const MEASURES: &[&str] = &[
    "economy.running_share.<kind>",
    "economy.line_output_per_1000.<line key>",
    "economy.labour_unworked",
    "economy.items.<item id>",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BenchReport {
    pub ticks: u64,
    pub seed: u64,
    pub economy: EconomyReport,
    /// Typed in phase 2.
    pub staff: Option<()>,
    /// Typed in phase 3.
    pub memories: Option<()>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EconomyReport {
    /// Sorted by `pos`.
    pub machines: Vec<MachineReport>,
    pub lines: Vec<LineBench>,
    pub labour: LabourBench,
    /// Units made per item id.
    pub items: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MachineReport {
    pub pos: (i32, i32),
    pub kind: String,
    /// Ticks in each `MachineStatus::as_str`; sums to the run's `ticks`.
    pub status_ticks: BTreeMap<String, u64>,
    pub units: u64,
    pub running_share: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LineBench {
    /// The line's lowest member tile, `"x,y"`.
    pub key: String,
    /// Units the line's last member made, per 1000 ticks.
    pub output_per_1000: f32,
    pub status_ticks: BTreeMap<String, u64>,
}

/// Means over the run's ticks.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct LabourBench {
    pub mean_wanted: f32,
    pub mean_staffed: f32,
    /// By `Duty::name`.
    pub mean_unworked: BTreeMap<String, f32>,
    pub mean_unworked_total: f32,
}

/// Ticks spent in each status over a run of `ticks`.
///
/// `initial` holds from tick 0; each edge `(tick, status)` — ticks relative
/// to the run's start, in order — holds from its tick to the next edge. An
/// edge at or past `ticks` changes nothing the run saw.
pub fn fold_status(initial: &str, edges: &[(u64, &str)], ticks: u64) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let mut status = initial;
    let mut since = 0;
    for &(at, next) in edges {
        let at = at.min(ticks);
        *out.entry(status.to_string()).or_insert(0) += at.saturating_sub(since);
        since = at.max(since);
        status = next;
    }
    *out.entry(status.to_string()).or_insert(0) += ticks - since;
    out.retain(|_, n| *n > 0);
    out
}

impl BenchReport {
    /// A named scalar view of the report, which is what an objective file's
    /// targets read. An unknown name is an error rather than a zero, so a
    /// typo cannot pass as a target met.
    pub fn measure(&self, name: &str) -> Result<f64, String> {
        let economy = &self.economy;
        if name == "economy.labour_unworked" {
            return Ok(f64::from(economy.labour.mean_unworked_total));
        }
        if let Some(kind) = name.strip_prefix("economy.running_share.") {
            let shares: Vec<f64> = economy
                .machines
                .iter()
                .filter(|m| m.kind == kind)
                .map(|m| f64::from(m.running_share))
                .collect();
            if shares.is_empty() {
                return Err(format!("no machine of kind `{kind}` in this base"));
            }
            return Ok(shares.iter().sum::<f64>() / shares.len() as f64);
        }
        if let Some(key) = name.strip_prefix("economy.line_output_per_1000.") {
            return economy
                .lines
                .iter()
                .find(|l| l.key == key)
                .map(|l| f64::from(l.output_per_1000))
                .ok_or_else(|| format!("no line keyed `{key}` in this base"));
        }
        if let Some(item) = name.strip_prefix("economy.items.") {
            return Ok(economy.items.get(item).copied().unwrap_or(0) as f64);
        }
        Err(format!(
            "unknown measure `{name}`; known: {}",
            MEASURES.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
        pairs.iter().map(|&(s, n)| (s.to_string(), n)).collect()
    }

    #[test]
    fn edge_fold_counts_each_status_and_sums_to_ticks() {
        let folded = fold_status(
            "idle",
            &[(10, "running"), (30, "starved"), (35, "running")],
            100,
        );
        assert_eq!(
            folded,
            counts(&[("idle", 10), ("running", 20 + 65), ("starved", 5)])
        );
        assert_eq!(folded.values().sum::<u64>(), 100);
    }

    #[test]
    fn edge_fold_with_no_edges_is_all_initial() {
        assert_eq!(fold_status("running", &[], 50), counts(&[("running", 50)]));
    }

    #[test]
    fn edge_fold_ignores_edges_past_the_run() {
        let folded = fold_status("running", &[(40, "starved"), (60, "idle")], 40);
        assert_eq!(folded, counts(&[("running", 40)]));
    }

    fn sample() -> BenchReport {
        BenchReport {
            ticks: 100,
            seed: 1,
            economy: EconomyReport {
                machines: vec![
                    MachineReport {
                        pos: (0, 0),
                        kind: "extractor".into(),
                        status_ticks: counts(&[("running", 50), ("idle", 50)]),
                        units: 5,
                        running_share: 0.5,
                    },
                    MachineReport {
                        pos: (1, 0),
                        kind: "extractor".into(),
                        status_ticks: counts(&[("running", 100)]),
                        units: 9,
                        running_share: 1.0,
                    },
                ],
                lines: vec![LineBench {
                    key: "0,0".into(),
                    output_per_1000: 12.5,
                    status_ticks: counts(&[("running", 100)]),
                }],
                labour: LabourBench {
                    mean_unworked_total: 1.5,
                    ..LabourBench::default()
                },
                items: BTreeMap::from([("ore".to_string(), 14)]),
            },
            staff: None,
            memories: None,
        }
    }

    #[test]
    fn measure_reads_known_names() {
        let r = sample();
        assert_eq!(r.measure("economy.running_share.extractor"), Ok(0.75));
        assert_eq!(r.measure("economy.line_output_per_1000.0,0"), Ok(12.5));
        assert_eq!(r.measure("economy.labour_unworked"), Ok(1.5));
        assert_eq!(r.measure("economy.items.ore"), Ok(14.0));
        assert_eq!(r.measure("economy.items.wire"), Ok(0.0));
    }

    #[test]
    fn measure_rejects_unknown_names() {
        let r = sample();
        assert!(r.measure("economy.nope").is_err());
        assert!(r.measure("economy.running_share.assembler").is_err());
        assert!(r.measure("economy.line_output_per_1000.9,9").is_err());
    }
}
