//! Trains the enemy battle policy against the arena harness and writes the
//! weights out as an asset.
//!
//! ```sh
//! cargo run --release --bin train -- --out assets/policies/enemy_battle.ron \
//!     --scenarios dev-training/ --iters 30 --pop 40 --reps 200 --seed 1
//! ```
//!
//! The optimiser is `feral_processes::cem`; the environment and the reward
//! are `arena::run`, which already knows the two non-obvious parts of
//! reading a fight's outcome. This file is the glue between them, and it is
//! deliberately thin — everything worth a unit test lives on one side or
//! the other.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use feral_processes::cem::{CemConfig, CemProgress, optimise};
use feral_processes_engine::arena::{self, Scenario};
use feral_processes_engine::policy::{self, Feature, PolicyWeights};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Weight on "how much of the party's HP did we take" in the fitness. Small,
/// because it is a tiebreak and not the objective — but not zero, because at
/// low enemy win rates a pure win/loss signal has no gradient at all and the
/// first several generations would be a random walk.
const TIEBREAK: f32 = 0.1;

/// Standard deviation the first generation is drawn from, in units of the
/// score a feature contributes. The features are all normalised to roughly
/// `[0, 1]` and the aggro prior spans `ln(1)..ln(7)`, so a weight of 1 is
/// already a strong opinion and 1.5 is a wide net without being absurd.
const INITIAL_STD: f32 = 1.5;
const STD_FLOOR: f32 = 0.05;
const ELITE_FRACTION: f32 = 0.25;

struct Args {
    out: PathBuf,
    scenarios: PathBuf,
    assets: PathBuf,
    iters: usize,
    pop: usize,
    reps: Option<u32>,
    seed: u64,
    report: Option<PathBuf>,
    /// Features held at zero for the whole run, by name.
    ///
    /// Not a tuning knob — a design boundary. The first real run learned
    /// `target_is_player: +7.4` and `target_bracing: -3.3`, which together
    /// mean "kill the player, ignore everyone else, and especially ignore
    /// whoever braced". That is optimal play and it deletes soft ranks,
    /// party positioning and Defend in one go. Pinning a feature says the
    /// aggro table decides that question and the policy may not reopen it.
    pin: Vec<Feature>,
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("train: {e}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let scenarios = load_scenarios(&args.scenarios, args.reps)?;
    println!(
        "training on {} scenarios from {}",
        scenarios.len(),
        args.scenarios.display()
    );

    // One assets tree per worker thread. A candidate is installed by writing
    // its weights into the tree it is being evaluated in, so concurrent
    // candidates need separate trees — and the real `assets/` is never
    // written to, which matters when the file being trained is the one the
    // running game would read.
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(args.pop.max(1));
    let arena_pool = AssetPool::build(&args.assets, workers)?;
    println!("{workers} worker asset trees under {}", arena_pool.root());

    // Every candidate in a generation is scored on the same seeds, so a
    // comparison between them is signal rather than luck; the offset moves
    // between generations so the winner is not one that overfitted to a
    // particular set of fights. Read by many threads, written only between
    // generations, from `on_progress`.
    let seed_offset = AtomicU64::new(args.seed.wrapping_mul(1_000_003));

    if !args.pin.is_empty() {
        let names: Vec<&str> = args.pin.iter().map(|f| f.name()).collect();
        println!("pinned to zero: {}", names.join(", "));
    }

    let baseline = evaluate_set(
        &scenarios,
        &arena_pool,
        &PolicyWeights::default(),
        args.seed,
    )?;
    println!(
        "baseline (all-zero weights): enemy win rate {:.3}, fitness {:.4}",
        baseline.enemy_win_rate, baseline.fitness
    );

    let cfg = CemConfig {
        dims: policy::FEATURE_COUNT,
        population: args.pop,
        elite_fraction: ELITE_FRACTION,
        iterations: args.iters,
        initial_std: INITIAL_STD,
        std_floor: STD_FLOOR,
    };
    let mut rng = StdRng::seed_from_u64(args.seed);

    let fitness = |candidate: &[f32]| -> f32 {
        let weights = weights_from_slice(candidate, &args.pin);
        let offset = seed_offset.load(Ordering::Relaxed);
        match evaluate_set(&scenarios, &arena_pool, &weights, offset) {
            Ok(result) => result.fitness,
            // A scenario that cannot be staged is a broken scenario file,
            // not a bad candidate — but every candidate hits it equally, so
            // failing the whole run here would be the right call and
            // scoring it worst is the wrong one. It is checked up front
            // instead, by the baseline evaluation above.
            Err(e) => {
                eprintln!("train: scenario failed mid-run: {e}");
                f32::NEG_INFINITY
            }
        }
    };

    let start = std::time::Instant::now();
    let trained = optimise(&cfg, &mut rng, fitness, |p: CemProgress| {
        seed_offset.fetch_add(7_919, Ordering::Relaxed);
        println!(
            "gen {:>3}/{}  best {:.4}  mean {:.4}  [{:.0}s]",
            p.iteration + 1,
            cfg.iterations,
            p.best_fitness,
            p.mean_fitness,
            start.elapsed().as_secs_f32(),
        );
    });

    let weights = weights_from_slice(&trained, &args.pin);
    let final_result = evaluate_set(&scenarios, &arena_pool, &weights, args.seed)?;
    println!(
        "trained: enemy win rate {:.3} (baseline {:.3}), fitness {:.4} (baseline {:.4})",
        final_result.enemy_win_rate,
        baseline.enemy_win_rate,
        final_result.fitness,
        baseline.fitness
    );
    println!("\nlearned weights, largest first:");
    for (name, value) in by_magnitude(&weights) {
        println!("  {value:>8.3}  {name}");
    }

    policy::write_file(&args.out, &weights).map_err(|e| format!("{}: {e}", args.out.display()))?;
    println!("\nwrote {}", args.out.display());

    if let Some(path) = &args.report {
        let text = report_ron(&args, &scenarios, &baseline, &final_result, &weights)?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// What one weight vector scored over the whole scenario set.
struct SetResult {
    /// The enemy's, which is `1.0 - Summary::win_rate` — that field is the
    /// *player's*.
    enemy_win_rate: f32,
    mean_player_hp_fraction: f32,
    fitness: f32,
    per_scenario: Vec<(String, f32, f32)>,
}

fn evaluate_set(
    scenarios: &[(String, Scenario)],
    pool: &AssetPool,
    weights: &PolicyWeights,
    seed_offset: u64,
) -> Result<SetResult, String> {
    let lease = pool.take();
    policy::write_file(&lease.policy_path(), weights).map_err(|e| e.to_string())?;

    let mut per_scenario = Vec::new();
    let (mut enemy_wins, mut hp) = (0.0f32, 0.0f32);
    for (name, scenario) in scenarios {
        let mut scenario = scenario.clone();
        scenario.seed = scenario.seed.wrapping_add(seed_offset);
        let summary = arena::run(&scenario, lease.assets(), arena::RunOptions::default())?
            .0
            .summary();
        let enemy = 1.0 - summary.win_rate;
        enemy_wins += enemy;
        hp += summary.mean_player_hp_fraction;
        per_scenario.push((name.clone(), enemy, summary.mean_player_hp_fraction));
    }
    let n = scenarios.len().max(1) as f32;
    let enemy_win_rate = enemy_wins / n;
    let mean_player_hp_fraction = hp / n;
    Ok(SetResult {
        enemy_win_rate,
        mean_player_hp_fraction,
        fitness: enemy_win_rate + TIEBREAK * (1.0 - mean_player_hp_fraction),
        per_scenario,
    })
}

/// The one place a CEM candidate becomes a weight vector, and therefore the
/// one place a pin has to be applied — masking here means the fitness
/// function never sees a pinned feature's value, so the search cannot learn
/// to exploit it and the written file cannot carry a stray one.
fn weights_from_slice(values: &[f32], pin: &[Feature]) -> PolicyWeights {
    let pairs: Vec<(String, f32)> = Feature::ALL
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            let value = if pin.contains(&f) {
                0.0
            } else {
                values.get(i).copied().unwrap_or(0.0)
            };
            (f.name().to_string(), value)
        })
        .collect();
    PolicyWeights::from_pairs(&pairs)
        .expect("CEM samples are finite")
        .0
}

fn by_magnitude(w: &PolicyWeights) -> Vec<(String, f32)> {
    let mut pairs = w.to_pairs();
    pairs.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    pairs
}

/// A set of complete asset trees, one per worker, each with its own
/// `policies/enemy_battle.ron` for the candidate being scored in it.
///
/// A free list rather than a tree per candidate: a tree is ~200 files and
/// there are `iters * pop` candidates, so building one each would cost more
/// than the fights do.
struct AssetPool {
    root: PathBuf,
    free: Mutex<Vec<PathBuf>>,
}

struct Lease<'a> {
    pool: &'a AssetPool,
    dir: Option<PathBuf>,
}

impl AssetPool {
    fn build(assets: &Path, workers: usize) -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!("feral_train_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut free = Vec::new();
        for i in 0..workers.max(1) {
            let dir = root.join(i.to_string());
            copy_tree(assets, &dir).map_err(|e| format!("copying {}: {e}", assets.display()))?;
            free.push(dir);
        }
        Ok(AssetPool {
            root,
            free: Mutex::new(free),
        })
    }

    fn root(&self) -> String {
        self.root.display().to_string()
    }

    /// Blocks only in the sense of spinning on an empty pool, which cannot
    /// happen: there are as many trees as there are worker threads.
    fn take(&self) -> Lease<'_> {
        let dir = self
            .free
            .lock()
            .expect("asset pool")
            .pop()
            .expect("one tree per worker, so one is always free");
        Lease {
            pool: self,
            dir: Some(dir),
        }
    }
}

/// Cleanup is a guard, not a line at the end of `run`. Each tree is a full
/// copy of `assets/` — around two hundred files — and a run that ends on an
/// error or a Ctrl-C would otherwise leave every one of them behind. The
/// engine's test fixtures learned this the expensive way: 5,437 stale
/// installs exhausted the filesystem's *inode* table on a tmpfs that was
/// 15% full by bytes, which fails builds machine-wide with an error naming
/// none of it.
impl Drop for AssetPool {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Lease<'_> {
    fn assets(&self) -> &Path {
        self.dir.as_ref().expect("held for the lease's lifetime")
    }

    fn policy_path(&self) -> PathBuf {
        self.assets().join("policies/enemy_battle.ron")
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if let Some(dir) = self.dir.take() {
            self.pool.free.lock().expect("asset pool").push(dir);
        }
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Every `.ron` in `dir`, in filename order so a run is reproducible and a
/// report's rows line up between runs.
fn load_scenarios(dir: &Path, reps: Option<u32>) -> Result<Vec<(String, Scenario)>, String> {
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
        .map(|path| {
            let mut scenario = Scenario::load(&path)?;
            if let Some(reps) = reps {
                scenario.reps = reps;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            Ok((name, scenario))
        })
        .collect()
}

fn report_ron(
    args: &Args,
    scenarios: &[(String, Scenario)],
    baseline: &SetResult,
    trained: &SetResult,
    weights: &PolicyWeights,
) -> Result<String, String> {
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(s, "(").unwrap();
    writeln!(
        s,
        "    budget: (iters: {}, pop: {}, reps: {}, scenarios: {}, seed: {}),",
        args.iters,
        args.pop,
        scenarios.first().map(|(_, s)| s.reps).unwrap_or(0),
        scenarios.len(),
        args.seed
    )
    .unwrap();
    writeln!(
        s,
        "    baseline: (enemy_win_rate: {:.4}, mean_player_hp_fraction: {:.4}, fitness: {:.4}),",
        baseline.enemy_win_rate, baseline.mean_player_hp_fraction, baseline.fitness
    )
    .unwrap();
    writeln!(
        s,
        "    trained: (enemy_win_rate: {:.4}, mean_player_hp_fraction: {:.4}, fitness: {:.4}),",
        trained.enemy_win_rate, trained.mean_player_hp_fraction, trained.fitness
    )
    .unwrap();
    let pinned: Vec<&str> = args.pin.iter().map(|f| f.name()).collect();
    writeln!(s, "    pinned_to_zero: {pinned:?},").unwrap();
    writeln!(s, "    per_scenario: [").unwrap();
    for ((name, base_enemy, base_hp), (_, trained_enemy, trained_hp)) in
        baseline.per_scenario.iter().zip(&trained.per_scenario)
    {
        writeln!(
            s,
            "        (name: {name:?}, baseline_enemy_win: {base_enemy:.4}, \
             trained_enemy_win: {trained_enemy:.4}, baseline_player_hp: {base_hp:.4}, \
             trained_player_hp: {trained_hp:.4}),"
        )
        .unwrap();
    }
    writeln!(s, "    ],").unwrap();
    writeln!(s, "    weights_by_magnitude: [").unwrap();
    for (name, value) in by_magnitude(weights) {
        writeln!(s, "        ({name:?}, {value:.4}),").unwrap();
    }
    writeln!(s, "    ],").unwrap();
    writeln!(s, ")").unwrap();
    Ok(s)
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        out: PathBuf::from("assets/policies/enemy_battle.ron"),
        scenarios: PathBuf::from("dev-training"),
        assets: PathBuf::from("assets"),
        iters: 30,
        pop: 40,
        reps: None,
        seed: 1,
        report: None,
        pin: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| format!("{flag} needs a value"))
                .map(|v| (flag.clone(), v))
        };
        match flag.as_str() {
            "--out" => args.out = value()?.1.into(),
            "--scenarios" => args.scenarios = value()?.1.into(),
            "--assets" => args.assets = value()?.1.into(),
            "--report" => args.report = Some(value()?.1.into()),
            "--iters" => args.iters = parse(value()?)?,
            "--pop" => args.pop = parse(value()?)?,
            "--reps" => args.reps = Some(parse(value()?)?),
            "--seed" => args.seed = parse(value()?)?,
            "--pin" => {
                for name in value()?.1.split(',').filter(|s| !s.is_empty()) {
                    let feature = Feature::from_name(name)
                        .ok_or_else(|| format!("--pin: {name:?} is not a feature name"))?;
                    args.pin.push(feature);
                }
            }
            "-h" | "--help" => {
                println!(
                    "train --out <path> --scenarios <dir> [--assets <dir>] [--iters 30] \
                     [--pop 40] [--reps N] [--seed 1] [--report <path>] \
                     [--pin feature,feature]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(args)
}

fn parse<T: std::str::FromStr>((flag, value): (String, String)) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag}: {value:?} is not a number"))
}
