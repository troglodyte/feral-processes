//! The search: CEM over the knobs, scored by how far the bench's measures
//! fall outside their target ranges.
//!
//! The output is a proposal, never an edit: nothing here writes `assets/`.

use super::knob::{Knob, patch, read_back};
use super::objective::{Objective, range_error};
use crate::cem::{CemConfig, optimise};
use crate::scratch_assets::ScratchAssets;
use feral_processes_engine::bench::{self, RunOptions};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

const ELITE_FRACTION: f32 = 0.25;
/// In unit space, where a knob's whole range is 1.0.
const INITIAL_STD: f32 = 0.25;
const STD_FLOOR: f32 = 0.02;

/// How the objective's targets scored on a set of seeds.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    /// Mean over seeds of the summed `range_error` of every target.
    pub error: f64,
    /// Mean over seeds of each target's measure, in target order.
    pub values: Vec<f64>,
    /// How many of the seeds' runs ended early (a battle opened or the game
    /// ended), so their rates cover fewer ticks than asked for.
    pub stopped: usize,
}

/// The shipped numbers are the search's baseline, so a stopped run in them
/// leaves nothing to compare against; the objective's `ticks` outlasts
/// something in the base, and the fix is a shorter run.
fn require_whole_runs(score: &Score, ticks: u64) -> Result<(), String> {
    if score.stopped == 0 {
        return Ok(());
    }
    Err(format!(
        "{} run(s) of the shipped set stopped early on the search seeds (a battle opened or the \
         game ended before {ticks} ticks); use fewer `ticks` in the objective",
        score.stopped
    ))
}

/// A candidate whose run stopped early scores worst, as an unreadable one
/// does. Its measures cover a shorter run, so a base that dies at tick 3
/// could look like it sits on every target; and `optimise` already ranks a
/// `NEG_INFINITY` candidate last, so the search just steers away from it.
fn fitness_of(score: &Score) -> f32 {
    if score.stopped > 0 {
        f32::NEG_INFINITY
    } else {
        -(score.error as f32)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    pub file: String,
    pub field: String,
    pub old: f64,
    pub new: f64,
}

#[derive(Debug)]
pub struct Proposal {
    pub changes: Vec<Change>,
    pub search_before: Score,
    pub search_after: Score,
    pub holdout_before: Score,
    pub holdout_after: Score,
    /// The patched text of every file a knob touches, by path under assets.
    pub files: BTreeMap<String, String>,
}

impl Proposal {
    /// Whether the proposal beat the shipped numbers on seeds the search
    /// never saw; a win on the training seeds alone is overfitting. Numbers
    /// that already sit on every target cannot improve, so a hold-out that
    /// stays at zero error holds up.
    pub fn holds_up(&self) -> bool {
        let (before, after) = (self.holdout_before.error, self.holdout_after.error);
        self.holdout_after.stopped == 0 && (after < before || (before == 0.0 && after == 0.0))
    }
}

/// Maps a unit-interval position to a knob's range, snapped to what the knob
/// stores.
fn knob_value(knob: &Knob, unit: f64) -> f64 {
    knob.snap(knob.min + unit.clamp(0.0, 1.0) * (knob.max - knob.min))
}

fn unit_of(knob: &Knob, value: f64) -> f64 {
    ((value - knob.min) / (knob.max - knob.min)).clamp(0.0, 1.0)
}

/// Writes every knob into `scratch` from the pristine file text and reads
/// each back through the engine's own types. Returns the patched texts.
fn apply(
    scratch: &Path,
    pristine: &BTreeMap<String, String>,
    knobs: &[Knob],
    values: &[f64],
) -> Result<BTreeMap<String, String>, String> {
    let files = apply_to_text(pristine, knobs, values)?;
    for (file, text) in &files {
        let path = scratch.join(file);
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    for (knob, &value) in knobs.iter().zip(values) {
        let got = read_back(scratch, knob)?;
        if (got - value).abs() > 1e-6 * value.abs().max(1.0) {
            return Err(format!(
                "{} `{}`: wrote {value}, read back {got}",
                knob.file, knob.field
            ));
        }
    }
    Ok(files)
}

/// Runs the bench on every seed in `seeds` and scores the targets.
fn score(obj: &Objective, save: &Path, assets: &Path, seeds: &[u64]) -> Result<Score, String> {
    let mut error = 0.0;
    let mut values = vec![0.0; obj.targets.len()];
    let mut stopped = 0;
    for &seed in seeds {
        let report = bench::run(
            save,
            assets,
            RunOptions {
                ticks: obj.ticks,
                seed,
                orders: obj.orders.clone(),
            },
        )?;
        stopped += usize::from(report.stopped_at.is_some());
        for (target, total) in obj.targets.iter().zip(&mut values) {
            let value = report.measure(&target.measure)?;
            *total += value;
            error += range_error(value, target.min, target.max);
        }
    }
    let n = seeds.len() as f64;
    Ok(Score {
        error: error / n,
        values: values.into_iter().map(|v| v / n).collect(),
        stopped,
    })
}

/// Trees leased one per worker thread; `cem::optimise` runs at most
/// `population` threads at once, so one is always free.
struct Pool {
    free: Mutex<Vec<ScratchAssets>>,
}

impl Pool {
    fn build(assets: &Path, trees: usize) -> Result<Self, String> {
        let free = (0..trees.max(1))
            .map(|_| ScratchAssets::new(assets, "bench"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Pool {
            free: Mutex::new(free),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&Path) -> T) -> T {
        let tree = self
            .free
            .lock()
            .expect("pool")
            .pop()
            .expect("one tree per worker");
        let out = f(tree.dir());
        self.free.lock().expect("pool").push(tree);
        out
    }
}

pub fn search(
    obj: &Objective,
    assets: &Path,
    save: &Path,
    log: &mut dyn FnMut(&str),
) -> Result<Proposal, String> {
    let read = |knob: &Knob| {
        std::fs::read_to_string(assets.join(&knob.file))
            .map_err(|e| format!("cannot read {}: {e}", knob.file))
    };
    let mut pristine = BTreeMap::new();
    for knob in &obj.knobs {
        if !pristine.contains_key(&knob.file) {
            pristine.insert(knob.file.clone(), read(knob)?);
        }
    }
    // Before any tick: a knob that names nothing real fails here, not an
    // hour in.
    let old: Vec<f64> = obj
        .knobs
        .iter()
        .map(|k| read_back(assets, k))
        .collect::<Result<_, _>>()?;
    let start: Vec<f64> = obj
        .knobs
        .iter()
        .zip(&old)
        .map(|(k, &v)| unit_of(k, v))
        .collect();
    for (knob, &v) in obj.knobs.iter().zip(&old) {
        if v < knob.min || v > knob.max {
            log(&format!(
                "note: {} `{}` ships at {v}, outside its range {}..{}",
                knob.file, knob.field, knob.min, knob.max
            ));
        }
    }

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(obj.population);
    let pool = Pool::build(assets, workers)?;

    // The CEM mean starts at zero, so the vector is an offset from where the
    // shipped numbers sit rather than an absolute position: a search that
    // began at every knob's minimum would not be a search around the game.
    let values_of = |x: &[f32]| -> Vec<f64> {
        obj.knobs
            .iter()
            .zip(&start)
            .zip(x)
            .map(|((k, &s), &d)| knob_value(k, s + f64::from(d)))
            .collect()
    };
    let evaluate = |values: &[f64], seeds: &[u64]| -> Result<Score, String> {
        pool.with(|scratch| {
            apply(scratch, &pristine, &obj.knobs, values)?;
            score(obj, save, scratch, seeds)
        })
    };

    let shipped: Vec<f64> = old
        .iter()
        .zip(&obj.knobs)
        .map(|(v, k)| k.snap(*v))
        .collect();
    let search_before = score(obj, save, assets, &obj.seeds)?;
    require_whole_runs(&search_before, obj.ticks)?;
    let holdout_before = score(obj, save, assets, &obj.holdout_seeds)?;
    log(&format!(
        "shipped: error {:.4} on the search seeds",
        search_before.error
    ));

    let cfg = CemConfig {
        dims: obj.knobs.len(),
        population: obj.population,
        elite_fraction: ELITE_FRACTION,
        iterations: obj.iterations,
        initial_std: INITIAL_STD,
        std_floor: STD_FLOOR,
    };
    let mut rng = StdRng::seed_from_u64(obj.search_seed);
    let first_error: Mutex<Option<String>> = Mutex::new(None);
    let fitness = |x: &[f32]| -> f32 {
        match evaluate(&values_of(x), &obj.seeds) {
            Ok(s) => fitness_of(&s),
            // A candidate whose measure cannot be read (a kind that no
            // longer exists) scores worst rather than ending the search;
            // the first reason is kept so the log can say why.
            Err(e) => {
                first_error.lock().expect("first error").get_or_insert(e);
                f32::NEG_INFINITY
            }
        }
    };
    let mut reported = false;
    let best = optimise(&cfg, &mut rng, fitness, |p| {
        if !reported && let Some(e) = first_error.lock().expect("first error").as_ref() {
            log(&format!("note: a candidate failed and scored worst: {e}"));
            reported = true;
        }
        log(&format!(
            "gen {}/{}  best error {:.4}  mean {:.4}",
            p.iteration + 1,
            obj.iterations,
            -p.best_fitness,
            -p.mean_fitness
        ));
    });

    let proposed = values_of(&best);
    let search_after = evaluate(&proposed, &obj.seeds)?;
    let holdout_after = evaluate(&proposed, &obj.holdout_seeds)?;
    let files = apply_to_text(&pristine, &obj.knobs, &proposed)?;
    let changes = obj
        .knobs
        .iter()
        .zip(shipped.iter().zip(&proposed))
        .map(|(k, (&old, &new))| Change {
            file: k.file.clone(),
            field: k.field.clone(),
            old,
            new,
        })
        .collect();
    Ok(Proposal {
        changes,
        search_before,
        search_after,
        holdout_before,
        holdout_after,
        files,
    })
}

/// Writes the proposal under `out_dir`: `proposal.ron`, `report.md`, and the
/// patched files at their asset-relative paths so `diff -r` against
/// `assets/` shows the change. Never touches `assets/` itself.
pub fn write_proposal(out_dir: &Path, obj: &Objective, proposal: &Proposal) -> Result<(), String> {
    let write = |path: std::path::PathBuf, text: &str| -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    };
    for (file, text) in &proposal.files {
        write(out_dir.join(file), text)?;
    }
    let ron = ron::ser::to_string_pretty(&proposal.changes, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    write(out_dir.join("proposal.ron"), &ron)?;
    write(out_dir.join("report.md"), &report(obj, proposal))
}

fn report(obj: &Objective, proposal: &Proposal) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("# Economy proposal\n\n");
    let _ = writeln!(
        out,
        "Template `{}`, {} ticks per run, {} iterations x {} candidates.\n",
        obj.template, obj.ticks, obj.iterations, obj.population
    );
    let _ = writeln!(
        out,
        "| seeds | error before | error after |\n|---|---|---|\n\
         | search {:?} | {:.4} | {:.4} |\n| hold-out {:?} | {:.4} | {:.4} |\n",
        obj.seeds,
        proposal.search_before.error,
        proposal.search_after.error,
        obj.holdout_seeds,
        proposal.holdout_before.error,
        proposal.holdout_after.error,
    );
    let stops = [
        ("search before", proposal.search_before.stopped),
        ("search after", proposal.search_after.stopped),
        ("hold-out before", proposal.holdout_before.stopped),
        ("hold-out after", proposal.holdout_after.stopped),
    ];
    for (which, n) in stops.into_iter().filter(|&(_, n)| n > 0) {
        let _ = writeln!(
            out,
            "**{which}: {n} run(s) stopped early** (a battle opened or the game ended); \
             their rates cover fewer ticks than asked.\n"
        );
    }
    let _ = writeln!(
        out,
        "Hold-out: **{}**\n",
        if proposal.holds_up() {
            "holds up"
        } else {
            "DOES NOT HOLD UP; do not apply"
        }
    );
    let _ = writeln!(out, "## Targets, mean over the hold-out seeds\n");
    let _ = writeln!(
        out,
        "| measure | want | shipped | proposed |\n|---|---|---|---|"
    );
    for (i, t) in obj.targets.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {}..{} | {:.3} | {:.3} |",
            t.measure,
            t.min,
            t.max,
            proposal.holdout_before.values[i],
            proposal.holdout_after.values[i]
        );
    }
    let _ = writeln!(
        out,
        "\n## Knobs\n\n| file | field | shipped | proposed |\n|---|---|---|---|"
    );
    for c in &proposal.changes {
        let _ = writeln!(out, "| {} | {} | {} | {} |", c.file, c.field, c.old, c.new);
    }
    out
}

fn apply_to_text(
    pristine: &BTreeMap<String, String>,
    knobs: &[Knob],
    values: &[f64],
) -> Result<BTreeMap<String, String>, String> {
    let mut files = pristine.clone();
    for (knob, &value) in knobs.iter().zip(values) {
        let text = files.get_mut(&knob.file).ok_or("knob file not loaded")?;
        *text = patch(text, &knob.field, value, knob.is_integer())?;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::super::chains_save;
    use super::*;
    use crate::dev_template::assets_dir;

    fn objective() -> Objective {
        Objective::from_ron(
            r#"(template: "chains", ticks: 200, seeds: [1], holdout_seeds: [2],
            iterations: 2, population: 2, search_seed: 7,
            targets: [(measure: "economy.labour_unworked", min: 0.0, max: 0.5)],
            knobs: [(file: "structures/assembly_bay.ron", field: "assembles.ticks_per_unit",
                     min: 5.0, max: 60.0)])"#,
        )
        .unwrap()
    }

    fn proposal_with_holdout(before: f64, after: f64) -> Proposal {
        let score = |error| Score {
            error,
            values: vec![],
            stopped: 0,
        };
        Proposal {
            changes: vec![],
            search_before: score(0.0),
            search_after: score(0.0),
            holdout_before: score(before),
            holdout_after: score(after),
            files: BTreeMap::new(),
        }
    }

    #[test]
    fn a_stopped_run_scores_worst_whatever_its_error() {
        let score = |error, stopped| Score {
            error,
            values: vec![],
            stopped,
        };
        assert_eq!(fitness_of(&score(0.0, 1)), f32::NEG_INFINITY);
        assert_eq!(fitness_of(&score(2.0, 0)), -2.0);
    }

    #[test]
    fn a_stopped_baseline_is_refused_with_the_ticks_and_a_remedy() {
        let stopped = |stopped| Score {
            error: 0.0,
            values: vec![],
            stopped,
        };
        assert!(require_whole_runs(&stopped(0), 5000).is_ok());
        let err = require_whole_runs(&stopped(2), 5000).unwrap_err();
        assert!(
            err.contains("5000") && err.contains("fewer `ticks`"),
            "{err}"
        );
    }

    #[test]
    fn a_stopped_holdout_run_never_holds_up() {
        let mut p = proposal_with_holdout(2.0, 1.0);
        p.holdout_after.stopped = 1;
        assert!(!p.holds_up());
    }

    #[test]
    fn holds_up_when_better_or_already_perfect_but_not_when_worse() {
        assert!(proposal_with_holdout(2.0, 1.0).holds_up());
        assert!(proposal_with_holdout(0.0, 0.0).holds_up());
        assert!(!proposal_with_holdout(1.0, 1.0).holds_up());
        assert!(!proposal_with_holdout(1.0, 2.0).holds_up());
        assert!(!proposal_with_holdout(0.0, 0.5).holds_up());
    }

    #[test]
    fn knob_values_map_linearly_clamp_and_round() {
        let k = Knob {
            file: "f".into(),
            field: "capacity".into(),
            min: 10.0,
            max: 20.0,
        };
        assert_eq!(knob_value(&k, 0.0), 10.0);
        assert_eq!(knob_value(&k, 0.26), 13.0);
        assert_eq!(knob_value(&k, 5.0), 20.0);
        assert_eq!(knob_value(&k, -5.0), 10.0);
        assert!((unit_of(&k, 15.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_float_knob_keeps_its_fraction() {
        let k = Knob {
            file: "needs/slack.ron".into(),
            field: "drain_per_tick".into(),
            min: 0.01,
            max: 0.03,
        };
        assert_eq!(knob_value(&k, 0.5), 0.02);
    }

    #[test]
    fn a_smoke_search_proposes_values_inside_the_bounds_and_leaves_assets_alone() {
        let obj = objective();
        let before = std::fs::read_to_string(assets_dir().join(&obj.knobs[0].file)).unwrap();
        let proposal = search(&obj, &assets_dir(), &chains_save(), &mut |_| {}).unwrap();
        let [change] = proposal.changes.as_slice() else {
            panic!("one knob, one change");
        };
        assert!((5.0..=60.0).contains(&change.new), "{change:?}");
        let patched = &proposal.files[&obj.knobs[0].file];
        assert!(
            patched.contains(&format!("ticks_per_unit: {}", change.new)),
            "{patched}"
        );
        assert_eq!(
            std::fs::read_to_string(assets_dir().join(&obj.knobs[0].file)).unwrap(),
            before
        );

        let out = std::env::temp_dir().join(format!("feral_bench_out_{}", std::process::id()));
        write_proposal(&out, &obj, &proposal).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join(&obj.knobs[0].file)).unwrap(),
            *patched
        );
        assert!(
            std::fs::read_to_string(out.join("proposal.ron"))
                .unwrap()
                .contains("old")
        );
        assert!(
            std::fs::read_to_string(out.join("report.md"))
                .unwrap()
                .contains("hold-out")
        );
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn an_unreadable_knob_fails_before_any_tick() {
        let mut obj = objective();
        obj.knobs[0].field = "glyph".into();
        let e = search(&obj, &assets_dir(), &chains_save(), &mut |_| {}).unwrap_err();
        assert!(e.contains("glyph"), "{e}");
    }
}
