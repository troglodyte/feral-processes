//! The shapes a bench run reports, and the pure folds that build and read
//! them. Nothing here touches a `Game`, so the arithmetic is testable
//! without running a base.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::Grievance;
use crate::telemetry::Record;

/// Every prefix `BenchReport::measure` accepts, for error messages and for
/// an objective file's load-time check.
pub const MEASURES: &[&str] = &[
    "economy.running_share.<kind>",
    "economy.line_output_per_1000.<line key>",
    "economy.labour_unworked",
    "economy.items.<item id>",
    "staff.on_shift_share",
    "staff.morale_mean",
    "staff.morale_min",
    "staff.sulking_share",
    "staff.need_strain_mean",
    "staff.need_mean.<need>",
    "staff.need_critical_share.<need>",
    "staff.rung_share.<rung|none>",
    "staff.tantrums_per_1000",
    "staff.frays_per_1000",
];

/// The grievance rungs a staff member can stand on, by `Grievance::as_str`;
/// `"none"` is the absence of one.
fn rungs() -> Vec<&'static str> {
    std::iter::once("none")
        .chain(Grievance::ALL.map(Grievance::as_str))
        .collect()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BenchReport {
    /// Ticks actually run; every rate and status count is over these.
    pub ticks: u64,
    /// The tick the run ended early at, relative to its start, because a
    /// battle opened or the game ended. `None` for a run that played out.
    #[serde(default)]
    pub stopped_at: Option<u64>,
    pub seed: u64,
    pub economy: EconomyReport,
    /// Never absent. A base with no staff reads zero for every scalar
    /// (`on_shift_share`, `morale_*`, `sulking_share`, the rung shares, the
    /// rates). The per-need measures are the exception: the report learns
    /// the needs from the staff it sampled, so with none a
    /// `need_mean.<id>` or `need_critical_share.<id>` is an error, the same
    /// as an unknown id, rather than a zero a typo could hide behind.
    pub staff: StaffReport,
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

/// Staff over the run, sampled per staff member per tick, so every share is
/// over `staff_ticks` and staff who join or leave weigh by the ticks they
/// were there.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct StaffReport {
    /// Sum over ticks of how many staff there were.
    pub staff_ticks: u64,
    pub on_shift_share: f32,
    pub morale_mean: f32,
    pub morale_min: f32,
    /// Staff-ticks with morale at or below `MORALE_SULKS_AT`: any rung reached.
    pub sulking_share: f32,
    pub need_strain_mean: f32,
    pub needs: BTreeMap<String, NeedBench>,
    /// `"none"` plus every rung, over `staff_ticks`.
    pub rung_share: BTreeMap<String, f32>,
    pub tantrums: u64,
    pub frays: u64,
    /// Ticks from the run's start to the first fray.
    pub first_fray: Option<u64>,
    /// Each staff member's morale at the last tick, ascending.
    pub end_morale: Vec<f32>,
}

/// One reserve, over the staff-ticks that member had it.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NeedBench {
    pub mean: f32,
    pub min: f32,
    /// Share of those ticks with the level below the def's `critical`.
    pub critical_share: f32,
}

/// One staff member at one tick, as the public `Game` reads give it.
pub struct StaffSample<'a> {
    pub on_shift: bool,
    pub morale: f32,
    pub strain: f32,
    /// `(id, level, critical)` per reserve.
    pub needs: Vec<(&'a str, f32, f32)>,
    /// `Game::grievance`.
    pub rung: Option<&'a str>,
}

#[derive(Default)]
struct NeedTally {
    sum: f64,
    min: f32,
    critical: u64,
    n: u64,
}

/// Running sums for `StaffReport`, so a run keeps no per-tick samples.
#[derive(Default)]
pub struct StaffTally {
    staff_ticks: u64,
    on_shift: u64,
    sulking: u64,
    morale_sum: f64,
    morale_min: Option<f32>,
    strain_sum: f64,
    needs: BTreeMap<String, NeedTally>,
    rungs: BTreeMap<String, u64>,
    last_morale: Vec<f32>,
}

impl StaffTally {
    /// Start a tick: the morale list is whoever is there at the last one.
    pub fn begin_tick(&mut self) {
        self.last_morale.clear();
    }

    pub fn add(&mut self, s: &StaffSample) {
        self.staff_ticks += 1;
        self.on_shift += u64::from(s.on_shift);
        self.sulking += u64::from(crate::game::base::morale::reached(s.morale).is_some());
        self.morale_sum += f64::from(s.morale);
        self.morale_min = Some(self.morale_min.map_or(s.morale, |m| m.min(s.morale)));
        self.strain_sum += f64::from(s.strain);
        self.last_morale.push(s.morale);
        *self
            .rungs
            .entry(s.rung.unwrap_or("none").to_string())
            .or_insert(0) += 1;
        for &(id, level, critical) in &s.needs {
            let t = self
                .needs
                .entry(id.to_string())
                .or_insert_with(|| NeedTally {
                    min: level,
                    ..NeedTally::default()
                });
            t.sum += f64::from(level);
            t.min = t.min.min(level);
            t.critical += u64::from(level < critical);
            t.n += 1;
        }
    }

    /// The report for a run that began at tick `t0`, with the
    /// tantrum and fray counts and the first fray read off `records`.
    pub fn finish(mut self, t0: u64, records: &[Record]) -> StaffReport {
        let n = self.staff_ticks;
        let mean = |sum: f64, n: u64| if n == 0 { 0.0 } else { (sum / n as f64) as f32 };
        let (mut tantrums, mut frays, mut first_fray) = (0, 0, None::<u64>);
        for record in records {
            match record {
                Record::Tantrum { .. } => tantrums += 1,
                Record::Fray { tick, .. } => {
                    frays += 1;
                    let at = tick.saturating_sub(t0);
                    first_fray = Some(first_fray.map_or(at, |f| f.min(at)));
                }
                _ => {}
            }
        }
        self.last_morale.sort_by(f32::total_cmp);
        StaffReport {
            staff_ticks: n,
            on_shift_share: share(self.on_shift, n),
            morale_mean: mean(self.morale_sum, n),
            morale_min: self.morale_min.unwrap_or(0.0),
            sulking_share: share(self.sulking, n),
            need_strain_mean: mean(self.strain_sum, n),
            needs: self
                .needs
                .into_iter()
                .map(|(id, t)| {
                    let bench = NeedBench {
                        mean: mean(t.sum, t.n),
                        min: t.min,
                        critical_share: share(t.critical, t.n),
                    };
                    (id, bench)
                })
                .collect(),
            rung_share: rungs()
                .into_iter()
                .map(|r| {
                    (
                        r.to_string(),
                        share(self.rungs.get(r).copied().unwrap_or(0), n),
                    )
                })
                .collect(),
            tantrums,
            frays,
            first_fray,
            end_morale: self.last_morale,
        }
    }
}

/// `n / total`, with nothing to divide over reading as zero rather than NaN.
pub fn share(n: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        n as f32 / total as f32
    }
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
    /// The `staff.` measures; `None` when `name` is not one of them.
    fn staff_measure(&self, name: &str) -> Option<Result<f64, String>> {
        let staff = &self.staff;
        let per_1000 = |n: u64| {
            if self.ticks == 0 {
                0.0
            } else {
                n as f64 * 1000.0 / self.ticks as f64
            }
        };
        let rest = name.strip_prefix("staff.")?;
        let ok = |v: f32| Some(Ok(f64::from(v)));
        match rest {
            "on_shift_share" => return ok(staff.on_shift_share),
            "morale_mean" => return ok(staff.morale_mean),
            "morale_min" => return ok(staff.morale_min),
            "sulking_share" => return ok(staff.sulking_share),
            "need_strain_mean" => return ok(staff.need_strain_mean),
            "tantrums_per_1000" => return Some(Ok(per_1000(staff.tantrums))),
            "frays_per_1000" => return Some(Ok(per_1000(staff.frays))),
            _ => {}
        }
        let need = |id: &str| {
            staff
                .needs
                .get(id)
                .ok_or_else(|| format!("no need `{id}` among this base's staff"))
        };
        if let Some(id) = rest.strip_prefix("need_mean.") {
            return Some(need(id).map(|n| f64::from(n.mean)));
        }
        if let Some(id) = rest.strip_prefix("need_critical_share.") {
            return Some(need(id).map(|n| f64::from(n.critical_share)));
        }
        if let Some(rung) = rest.strip_prefix("rung_share.") {
            return Some(
                staff
                    .rung_share
                    .get(rung)
                    .map(|&v| f64::from(v))
                    .ok_or_else(|| format!("no rung `{rung}`; known: {}", rungs().join(", "))),
            );
        }
        None
    }

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
        if let Some(v) = self.staff_measure(name) {
            return v;
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
            stopped_at: None,
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
            staff: StaffReport::default(),
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

    fn sample_of(morale: f32, on_shift: bool, rung: Option<&'static str>) -> StaffSample<'static> {
        StaffSample {
            on_shift,
            morale,
            strain: morale.abs(),
            needs: vec![("coherence", morale + 10.0, 5.0)],
            rung,
        }
    }

    /// The sim's own entry test is `<=`, so a program sitting exactly on the
    /// line is sulking and the report has to say so.
    #[test]
    fn a_program_exactly_on_the_sulking_line_counts_as_sulking() {
        let mut t = StaffTally::default();
        t.add(&sample_of(crate::tuning::MORALE_SULKS_AT, true, None));
        assert_eq!(t.finish(0, &[]).sulking_share, 1.0);
    }

    fn folded() -> StaffReport {
        let mut t = StaffTally::default();
        for samples in [
            vec![
                sample_of(0.0, true, None),
                sample_of(-10.0, false, Some("sulking")),
            ],
            vec![
                sample_of(-12.0, true, None),
                sample_of(-20.0, true, Some("downed_tools")),
            ],
        ] {
            t.begin_tick();
            samples.iter().for_each(|s| t.add(s));
        }
        let records = [
            Record::Tantrum {
                tick: 1005,
                who: "a".into(),
            },
            Record::Fray {
                tick: 1030,
                who: "a".into(),
                need: "n".into(),
                unreachable: false,
            },
            Record::Fray {
                tick: 1012,
                who: "b".into(),
                need: "n".into(),
                unreachable: true,
            },
        ];
        t.finish(1000, &records)
    }

    #[test]
    fn staff_fold_shares_means_and_mins_are_over_staff_ticks() {
        let r = folded();
        assert_eq!(r.staff_ticks, 4);
        assert_eq!(r.on_shift_share, 0.75);
        assert_eq!(r.morale_mean, -10.5);
        assert_eq!(r.morale_min, -20.0);
        assert_eq!(r.sulking_share, 0.75);
        assert_eq!(r.need_strain_mean, 10.5);
        assert_eq!(r.rung_share["none"], 0.5);
        assert_eq!(r.rung_share["sulking"], 0.25);
        assert_eq!(r.rung_share["downed_tools"], 0.25);
        assert_eq!(r.rung_share["lashing_out"], 0.0);
        let need = &r.needs["coherence"];
        assert_eq!(
            (need.mean, need.min, need.critical_share),
            (-0.5, -10.0, 0.75)
        );
        assert_eq!(r.end_morale, vec![-20.0, -12.0]);
    }

    #[test]
    fn staff_fold_counts_records_and_dates_the_first_fray_from_run_start() {
        let r = folded();
        assert_eq!((r.tantrums, r.frays), (1, 2));
        assert_eq!(r.first_fray, Some(12));
    }

    #[test]
    fn staff_fold_with_no_staff_is_zeros_not_nan() {
        let r = StaffTally::default().finish(0, &[]);
        assert_eq!(r.staff_ticks, 0);
        assert_eq!(r.on_shift_share, 0.0);
        assert_eq!(r.morale_mean, 0.0);
        assert_eq!(r.morale_min, 0.0);
        assert_eq!(r.first_fray, None);
        assert!(r.end_morale.is_empty());
        assert!(r.rung_share.values().all(|&v| v == 0.0));
        assert_eq!(r.rung_share.len(), rungs().len());
    }

    #[test]
    fn an_empty_base_reads_zero_scalars_but_errors_on_per_need_measures() {
        let mut r = sample();
        r.staff = StaffTally::default().finish(0, &[]);
        for name in [
            "staff.morale_min",
            "staff.on_shift_share",
            "staff.rung_share.none",
        ] {
            assert_eq!(r.measure(name), Ok(0.0), "{name}");
        }
        assert!(r.measure("staff.need_mean.coherence").is_err());
        assert!(r.measure("staff.need_critical_share.coherence").is_err());
    }

    fn staffed() -> BenchReport {
        BenchReport {
            ticks: 500,
            staff: folded(),
            ..sample()
        }
    }

    #[test]
    fn staff_measures_read_known_names() {
        let r = staffed();
        let m = |n: &str| r.measure(n).unwrap();
        assert_eq!(m("staff.on_shift_share"), 0.75);
        assert_eq!(m("staff.morale_mean"), -10.5);
        assert_eq!(m("staff.morale_min"), -20.0);
        assert_eq!(m("staff.sulking_share"), 0.75);
        assert_eq!(m("staff.need_strain_mean"), 10.5);
        assert_eq!(m("staff.need_mean.coherence"), -0.5);
        assert_eq!(m("staff.need_critical_share.coherence"), 0.75);
        assert_eq!(m("staff.rung_share.none"), 0.5);
        assert_eq!(m("staff.rung_share.lashing_out"), 0.0);
        assert_eq!(m("staff.tantrums_per_1000"), 2.0);
        assert_eq!(m("staff.frays_per_1000"), 4.0);
    }

    #[test]
    fn every_grievance_rung_is_a_known_measure() {
        let r = staffed();
        for g in Grievance::ALL {
            let name = format!("staff.rung_share.{}", g.as_str());
            assert!(r.measure(&name).is_ok(), "{name}");
        }
    }

    #[test]
    fn staff_measures_reject_unknown_names() {
        let r = staffed();
        assert!(r.measure("staff.need_mean.warmth").is_err());
        assert!(r.measure("staff.need_critical_share.warmth").is_err());
        assert!(r.measure("staff.rung_share.exploding").is_err());
        assert!(r.measure("staff.nope").is_err());
    }

    #[test]
    fn staff_rates_over_a_zero_tick_run_are_zero() {
        let r = BenchReport {
            ticks: 0,
            ..staffed()
        };
        assert_eq!(r.measure("staff.frays_per_1000"), Ok(0.0));
    }
}
