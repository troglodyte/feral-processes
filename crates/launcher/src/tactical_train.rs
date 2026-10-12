//! Tuning a tactical behaviour profile's free weights so its hostiles land
//! the difficulty the `default` profile already has.
//!
//! The target is a recorded baseline, not a maximum: the objective is the
//! squared distance from `default`'s numbers on the same fixed fights, so a
//! personality changes how a fight is lost and not how often.
//!
//! This file is the pure half — which weights are free, how a candidate
//! becomes a profile, what a result set costs. `bin/train.rs` supplies the
//! arena and the optimiser.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use feral_processes_engine::arena::{self, Scenario};
use feral_processes_engine::behaviours::{BehaviourDb, BehaviourDef, Sign, write_file};
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::cem::{CemConfig, CemProgress, optimise};
use crate::scratch_assets::ScratchAssets;
use feral_processes_engine::tactical::policy::TacticalFeature;

/// Objective term weights, from the spec.
const HP_TERM: f32 = 1.0;
const WIN_TERM: f32 = 0.5;
const ROUNDS_TERM: f32 = 0.1;

/// The band a trained profile must land inside, relative to baseline.
pub const BAND: f32 = 0.10;

/// What one scenario's reps came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub enemy_win: f32,
    /// Fraction of the player's Integrity lost, 1 - the arena's HP left.
    pub hp_lost: f32,
    pub rounds: f32,
}

/// Every scenario of the set, in filename order.
pub type SetResult = Vec<(String, Stats)>;

/// The features training may move: named in `weights`, known to this build,
/// and not pinned. In `TacticalFeature::ALL` order so a candidate vector's
/// slots mean the same thing every run.
pub fn free_features(def: &BehaviourDef) -> Vec<TacticalFeature> {
    TacticalFeature::ALL
        .into_iter()
        .filter(|f| def.weights.contains_key(f.name()) && !def.pinned.contains_key(f.name()))
        .collect()
}

/// `def` with its free weights replaced by `candidate`, one slot per
/// `free_features` entry. A weight whose feature has a sign is clamped to
/// that side of zero; everything else in `def` is returned untouched.
pub fn weights_from_candidate(def: &BehaviourDef, candidate: &[f32]) -> BehaviourDef {
    let mut out = def.clone();
    for (f, &value) in free_features(def).into_iter().zip(candidate) {
        let clamped = match def.signs.get(f.name()) {
            Some(Sign::Positive) => value.max(0.0),
            Some(Sign::Negative) => value.min(0.0),
            None => value,
        };
        out.weights.insert(f.name().to_string(), clamped);
    }
    out
}

/// Cost of `set` against `base`, summed over scenarios; 0 at baseline.
pub fn objective(set: &SetResult, base: &SetResult) -> f32 {
    set.iter()
        .zip(base)
        .map(|((_, s), (_, b))| {
            let rounds_rel = if b.rounds > 0.0 {
                s.rounds / b.rounds
            } else {
                1.0
            };
            (s.hp_lost - b.hp_lost).powi(2) * HP_TERM
                + (s.enemy_win - b.enemy_win).powi(2) * WIN_TERM
                + (rounds_rel - 1.0).powi(2) * ROUNDS_TERM
        })
        .sum()
}

/// Set-wide means of (HP lost, enemy win rate).
pub fn means(set: &SetResult) -> (f32, f32) {
    let n = set.len().max(1) as f32;
    (
        set.iter().map(|(_, s)| s.hp_lost).sum::<f32>() / n,
        set.iter().map(|(_, s)| s.enemy_win).sum::<f32>() / n,
    )
}

/// Whether both set-wide means sit within `BAND` of baseline, relative.
pub fn in_band(set: &SetResult, base: &SetResult) -> bool {
    let ((hp, win), (bhp, bwin)) = (means(set), means(base));
    let within = |v: f32, b: f32| (v - b).abs() <= BAND * b.abs();
    within(hp, bhp) && within(win, bwin)
}

/// Per-generation search cost knob: reps per scenario while searching. The
/// baseline and the final comparison use the scenario files' own reps.
const DEFAULT_SEARCH_REPS: u32 = 50;
const INITIAL_STD: f32 = 1.0;
const STD_FLOOR: f32 = 0.05;
const ELITE_FRACTION: f32 = 0.25;

struct Args {
    profile: String,
    scenarios: PathBuf,
    assets: PathBuf,
    out: Option<PathBuf>,
    iters: usize,
    pop: usize,
    search_reps: u32,
    reps: Option<u32>,
    seed: u64,
    eval: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        profile: String::new(),
        scenarios: PathBuf::from("dev-training/tactical"),
        assets: PathBuf::from("assets"),
        out: None,
        iters: 20,
        pop: 24,
        search_reps: DEFAULT_SEARCH_REPS,
        reps: None,
        seed: 1,
        eval: false,
    };
    let mut it = args;
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        let num = |v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("{flag}: {v:?} is not a number"))
        };
        match flag.as_str() {
            "--tactical" => {}
            "--profile" => a.profile = value()?,
            "--scenarios" => a.scenarios = value()?.into(),
            "--assets" => a.assets = value()?.into(),
            "--out" => a.out = Some(value()?.into()),
            "--iters" => a.iters = num(value()?)? as usize,
            "--pop" => a.pop = num(value()?)? as usize,
            "--search-reps" => a.search_reps = num(value()?)? as u32,
            "--reps" => a.reps = Some(num(value()?)? as u32),
            "--seed" => a.seed = num(value()?)?,
            "--eval" => a.eval = true,
            other => return Err(format!("unknown flag {other} (tactical mode)")),
        }
    }
    if a.profile.is_empty() {
        return Err("--tactical needs --profile <id>".into());
    }
    Ok(a)
}

/// One asset tree per worker: a candidate is installed by writing its
/// profile file into the tree it is scored in.
struct Pool(Mutex<Vec<ScratchAssets>>);

fn evaluate(
    scenarios: &[(String, Scenario)],
    pool: &Pool,
    profile: &BehaviourDef,
    reps: Option<u32>,
    seed_offset: u64,
) -> Result<SetResult, String> {
    let tree = pool.0.lock().expect("pool").pop().expect("tree per worker");
    let result = (|| {
        let path = tree
            .dir()
            .join("behaviours")
            .join(format!("{}.ron", profile.id));
        write_file(&path, profile).map_err(|e| format!("{}: {e}", path.display()))?;
        scenarios
            .iter()
            .map(|(name, scenario)| {
                let mut scenario = scenario.clone();
                scenario.seed = scenario.seed.wrapping_add(seed_offset);
                scenario.behaviour = Some(profile.id.clone());
                if let Some(reps) = reps {
                    scenario.reps = reps;
                }
                let (report, _) = arena::run(&scenario, tree.dir(), arena::RunOptions::default())?;
                let s = report.summary();
                Ok((
                    name.clone(),
                    Stats {
                        enemy_win: 1.0 - s.win_rate,
                        hp_lost: 1.0 - s.mean_player_hp_fraction,
                        rounds: s.mean_rounds,
                    },
                ))
            })
            .collect()
    })();
    pool.0.lock().expect("pool").push(tree);
    result
}

fn load_scenarios(dir: &Path) -> Result<Vec<(String, Scenario)>, String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no .ron scenarios in {}", dir.display()));
    }
    paths
        .into_iter()
        .map(|p| {
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            Scenario::load(&p).map(|s| (name, s))
        })
        .collect()
}

fn print_comparison(profile: &str, base: &SetResult, got: &SetResult) {
    println!(
        "{:<24} {:>14} {:>14} {:>16}",
        "scenario", "HP lost b/t", "enemy win b/t", "rounds b/t"
    );
    for ((name, b), (_, g)) in base.iter().zip(got) {
        println!(
            "{name:<24} {:>6.3}/{:<7.3} {:>6.3}/{:<7.3} {:>7.1}/{:<8.1}",
            b.hp_lost, g.hp_lost, b.enemy_win, g.enemy_win, b.rounds, g.rounds
        );
    }
    let ((bhp, bwin), (hp, win)) = (means(base), means(got));
    println!(
        "set mean HP lost   baseline {bhp:.4}  {profile} {hp:.4}  ({:+.1}%)",
        100.0 * (hp / bhp - 1.0)
    );
    println!(
        "set mean enemy win baseline {bwin:.4}  {profile} {win:.4}  ({:+.1}%)",
        100.0 * (win / bwin - 1.0)
    );
    println!(
        "within +/-{:.0}% of baseline on both: {}",
        BAND * 100.0,
        in_band(got, base)
    );
}

/// `train --tactical --profile <id> ...`: scores (`--eval`) or trains a
/// profile against `default`'s numbers on the fixed fights.
pub fn run_cli(args: impl Iterator<Item = String>) -> Result<(), String> {
    let args = parse_args(args)?;
    let scenarios = load_scenarios(&args.scenarios)?;
    let (db, _) = BehaviourDb::load_dir(&args.assets.join("behaviours"))
        .map_err(|e| format!("behaviours: {e}"))?;
    let hand = db
        .get(&args.profile)
        .ok_or_else(|| format!("no behaviour profile {:?}", args.profile))?
        .clone();
    let default = db.default_profile().clone();

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let pool = Pool(Mutex::new(
        (0..workers)
            .map(|_| ScratchAssets::new(&args.assets, "tactical_train"))
            .collect::<Result<_, _>>()?,
    ));
    let base = evaluate(&scenarios, &pool, &default, args.reps, args.seed)?;

    if args.eval {
        let got = evaluate(&scenarios, &pool, &hand, args.reps, args.seed)?;
        print_comparison(&args.profile, &base, &got);
        return Ok(());
    }

    let free = free_features(&hand);
    if free.is_empty() {
        return Err(format!("{:?} has no free weights to train", args.profile));
    }
    // The search moves a delta from the authored weights, so generation 0 is
    // the hand-set profile and the character it was written with.
    let start: Vec<f32> = free.iter().map(|f| hand.weights[f.name()]).collect();
    let candidate = |delta: &[f32]| -> BehaviourDef {
        let absolute: Vec<f32> = start.iter().zip(delta).map(|(s, d)| s + d).collect();
        weights_from_candidate(&hand, &absolute)
    };
    let cfg = CemConfig {
        dims: free.len(),
        population: args.pop,
        elite_fraction: ELITE_FRACTION,
        iterations: args.iters,
        initial_std: INITIAL_STD,
        std_floor: STD_FLOOR,
    };
    let offset = AtomicU64::new(args.seed.wrapping_mul(1_000_003));
    let mut rng = StdRng::seed_from_u64(args.seed);
    let fitness = |delta: &[f32]| -> f32 {
        let off = offset.load(Ordering::Relaxed);
        match evaluate(
            &scenarios,
            &pool,
            &candidate(delta),
            Some(args.search_reps),
            off,
        ) {
            Ok(set) => -objective(&set, &base),
            Err(e) => {
                eprintln!("train: scenario failed mid-run: {e}");
                f32::NEG_INFINITY
            }
        }
    };
    let begin = std::time::Instant::now();
    let trained = optimise(&cfg, &mut rng, fitness, |p: CemProgress| {
        offset.fetch_add(7_919, Ordering::Relaxed);
        println!(
            "gen {:>3}/{}  best {:.5}  mean {:.5}  [{:.0}s]",
            p.iteration + 1,
            cfg.iterations,
            p.best_fitness,
            p.mean_fitness,
            begin.elapsed().as_secs_f32()
        );
    });
    let result = candidate(&trained);
    // Scored on the baseline's own seeds, the comparison the band is read on.
    let got = evaluate(&scenarios, &pool, &result, args.reps, args.seed)?;
    print_comparison(&args.profile, &base, &got);
    println!(
        "free weights: hand {:?} -> trained {:?}",
        free.iter()
            .map(|f| (f.name(), hand.weights[f.name()]))
            .collect::<Vec<_>>(),
        free.iter()
            .map(|f| (f.name(), result.weights[f.name()]))
            .collect::<Vec<_>>()
    );
    if !in_band(&got, &base) {
        println!("NOT in band: weights not written");
        return Ok(());
    }
    let out = args.out.unwrap_or_else(|| {
        args.assets
            .join("behaviours")
            .join(format!("{}.ron", args.profile))
    });
    write_file(&out, &result).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn def() -> BehaviourDef {
        BehaviourDef {
            id: "t".into(),
            pinned: BTreeMap::from([("target_distance".to_string(), -8.0)]),
            signs: BTreeMap::from([
                ("target_hp_frac".to_string(), Sign::Negative),
                ("would_kill".to_string(), Sign::Positive),
            ]),
            weights: BTreeMap::from([
                ("target_hp_frac".to_string(), -1.0),
                ("would_kill".to_string(), 1.0),
                ("est_damage_frac".to_string(), 0.5),
                // Pinned too: pinned wins, so it is not free.
                ("target_distance".to_string(), 3.0),
                ("no_such_feature".to_string(), 3.0),
            ]),
            ..Default::default()
        }
    }

    fn set(hp: f32, win: f32, rounds: f32) -> SetResult {
        vec![
            (
                "a".into(),
                Stats {
                    enemy_win: win,
                    hp_lost: hp,
                    rounds,
                },
            ),
            (
                "b".into(),
                Stats {
                    enemy_win: win,
                    hp_lost: hp,
                    rounds,
                },
            ),
        ]
    }

    #[test]
    fn pinned_features_never_change_across_candidates() {
        let d = def();
        assert!(!free_features(&d).contains(&TacticalFeature::TargetDistance));
        for v in [-9.0, 0.0, 9.0] {
            let out = weights_from_candidate(&d, &[v, v, v]);
            assert_eq!(out.pinned, d.pinned);
            assert_eq!(out.weights["target_distance"], 3.0);
            assert_eq!(out.weights["no_such_feature"], 3.0);
        }
    }

    #[test]
    fn a_signed_feature_never_crosses_zero() {
        let d = def();
        let free = free_features(&d);
        for v in [-9.0, 0.0, 9.0] {
            let out = weights_from_candidate(&d, &vec![v; free.len()]);
            assert!(out.weights["target_hp_frac"] <= 0.0);
            assert!(out.weights["would_kill"] >= 0.0);
        }
        // An unsigned free weight takes either side.
        let out = weights_from_candidate(&d, &vec![-9.0; free.len()]);
        assert_eq!(out.weights["est_damage_frac"], -9.0);
    }

    #[test]
    fn objective_is_zero_at_baseline_and_grows_on_each_term() {
        let base = set(0.4, 0.3, 10.0);
        assert_eq!(objective(&base, &base), 0.0);
        let hp = objective(&set(0.5, 0.3, 10.0), &base);
        let win = objective(&set(0.4, 0.4, 10.0), &base);
        let rounds = objective(&set(0.4, 0.3, 12.0), &base);
        assert!(hp > 0.0 && win > 0.0 && rounds > 0.0);
        assert!(objective(&set(0.6, 0.3, 10.0), &base) > hp);
        assert!(objective(&set(0.4, 0.5, 10.0), &base) > win);
        assert!(objective(&set(0.4, 0.3, 15.0), &base) > rounds);
    }

    #[test]
    fn the_band_is_ten_percent_relative_on_both_means() {
        let base = set(0.40, 0.30, 10.0);
        assert!(in_band(&set(0.43, 0.32, 10.0), &base));
        assert!(!in_band(&set(0.45, 0.30, 10.0), &base));
        assert!(!in_band(&set(0.40, 0.34, 10.0), &base));
    }
}
