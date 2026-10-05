//! A headless instrument for the base: load a save, wait N ticks, and report
//! what the economy did.
//!
//! The shape follows `arena`, and the discipline is the same: sampling goes
//! only through public `Game` methods, so a report says what the screens
//! could have shown and `Game.world` stays private. The launcher's `bench`
//! bin turns a template name into a save, because only it can.

mod report;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use rand::SeedableRng;
use rand::rngs::StdRng;

pub use report::{
    BenchReport, EconomyReport, LabourBench, LineBench, MEASURES, MachineReport, fold_status,
};

use crate::duties::Duty;
use crate::resources::GameRng;
use crate::telemetry::Record;
use crate::*;

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub ticks: u64,
    pub seed: u64,
    /// Batch orders queued before the first tick, as `(item, qty)`. Only
    /// posted workers haul, and workers are posted only on machines an order
    /// wants, so a template with no standing demand has no drain; a
    /// quantity too big to reach keeps the demand alive for the whole run.
    pub orders: Vec<(String, u32)>,
}

/// Per-line tick tallies, accumulated as the run goes.
#[derive(Default)]
struct LineTally {
    status_ticks: BTreeMap<String, u64>,
    /// The tile of the line's end-of-line machine, as of the last tick seen.
    last_member: Option<(i32, i32)>,
}

pub fn run(save: &Path, assets_dir: &Path, opts: RunOptions) -> Result<BenchReport, String> {
    let mut game = Game::load(save, assets_dir).map_err(|e| format!("{}: {e}", save.display()))?;
    // Installed after the load, as `arena` does, so one save under several
    // seeds is several different runs and any one replays alone.
    game.world
        .insert_resource(GameRng(StdRng::seed_from_u64(opts.seed)));
    for (item, qty) in &opts.orders {
        game.queue_work_order(WorkOrder::batch(ItemId::from(item.as_str()), *qty))
            .map_err(|e| format!("order {item} x{qty} refused: {e}"))?;
    }
    game.enable_telemetry();

    let start = game.structure_report();
    let tile_of: HashMap<Entity, (i32, i32)> = start.iter().map(|s| (s.entity, s.pos)).collect();
    let machines: BTreeMap<(i32, i32), (String, String)> = start
        .iter()
        .filter_map(|s| Some((s.pos, (s.kind.clone(), s.status?.as_str().to_string()))))
        .collect();
    let t0 = game.current_tick();

    let mut lines: BTreeMap<String, LineTally> = BTreeMap::new();
    let (mut wanted, mut staffed) = (0u64, 0u64);
    let mut unworked: BTreeMap<Duty, u64> = BTreeMap::new();
    for _ in 0..opts.ticks {
        advance(&mut game)?;
        for line in game.line_reports() {
            let tally = lines
                .entry(format!("{},{}", line.key.0.0, line.key.0.1))
                .or_default();
            *tally
                .status_ticks
                .entry(line.status.as_str().to_string())
                .or_insert(0) += 1;
            tally.last_member = line.members.last().and_then(|m| tile_of.get(m)).copied();
        }
        let demand = game.labour_demand();
        wanted += demand.wanted as u64;
        staffed += demand.staff as u64;
        for (&duty, &n) in &demand.unworked {
            *unworked.entry(duty).or_insert(0) += n as u64;
        }
    }

    let records = game.take_telemetry();
    let mut edges: HashMap<(i32, i32), Vec<(u64, &str)>> = HashMap::new();
    let mut units: HashMap<(i32, i32), u64> = HashMap::new();
    let mut items: BTreeMap<String, u64> = BTreeMap::new();
    for record in &records {
        match record {
            Record::MachineStall {
                tick,
                machine,
                status,
                ..
            } => edges
                .entry(*machine)
                .or_default()
                .push((tick.saturating_sub(t0), status)),
            Record::Extract {
                machine,
                item,
                landed,
                ok: true,
                ..
            } => {
                *units.entry(*machine).or_insert(0) += u64::from(*landed);
                *items.entry(item.clone()).or_insert(0) += u64::from(*landed);
            }
            Record::Assemble { machine, item, .. } => {
                *units.entry(*machine).or_insert(0) += 1;
                *items.entry(item.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let machines = machines
        .into_iter()
        .map(|(pos, (kind, initial))| {
            let status_ticks = fold_status(
                &initial,
                edges.get(&pos).map_or(&[][..], Vec::as_slice),
                opts.ticks,
            );
            let running = status_ticks.get("running").copied().unwrap_or(0);
            MachineReport {
                pos,
                kind,
                running_share: share(running, opts.ticks),
                units: units.get(&pos).copied().unwrap_or(0),
                status_ticks,
            }
        })
        .collect();
    let lines = lines
        .into_iter()
        .map(|(key, tally)| {
            let made = tally
                .last_member
                .and_then(|pos| units.get(&pos))
                .copied()
                .unwrap_or(0);
            LineBench {
                key,
                output_per_1000: if opts.ticks == 0 {
                    0.0
                } else {
                    made as f32 * 1000.0 / opts.ticks as f32
                },
                status_ticks: tally.status_ticks,
            }
        })
        .collect();
    let mean_unworked: BTreeMap<String, f32> = unworked
        .iter()
        .map(|(duty, &n)| (duty.name().to_string(), share(n, opts.ticks)))
        .collect();
    let labour = LabourBench {
        mean_wanted: share(wanted, opts.ticks),
        mean_staffed: share(staffed, opts.ticks),
        mean_unworked_total: mean_unworked.values().fold(0.0, |a, v| a + v),
        mean_unworked,
    };

    Ok(BenchReport {
        ticks: opts.ticks,
        seed: opts.seed,
        economy: EconomyReport {
            machines,
            lines,
            labour,
            items,
        },
        staff: None,
        memories: None,
    })
}

/// One `wait`, which does nothing while a battle is open or the game is
/// over: a bench that kept counting would report a stopped clock as a quiet
/// base.
fn advance(game: &mut Game) -> Result<(), String> {
    let before = game.current_tick();
    game.wait();
    if game.current_tick() == before {
        return Err(format!(
            "clock stopped at tick {before} (battle open or game over)"
        ));
    }
    Ok(())
}

/// `n / ticks`, with an empty run reading as zero rather than NaN.
fn share(n: u64, ticks: u64) -> f32 {
    if ticks == 0 {
        0.0
    } else {
        n as f32 / ticks as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::test_assets_dir;

    /// A `Game::new` save at a path unique to the caller. It has no machines,
    /// so these runs prove the plumbing and determinism; the real `chains`
    /// layout is checked from the launcher, where templates resolve.
    fn blank_save(tag: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("feral_bench_{tag}_{}.bin", std::process::id()));
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.save(&path).unwrap();
        path
    }

    fn run_blank(path: &Path, seed: u64) -> BenchReport {
        run(
            path,
            &test_assets_dir(),
            RunOptions {
                ticks: 50,
                seed,
                orders: vec![],
            },
        )
        .unwrap()
    }

    #[test]
    fn the_same_seed_twice_is_the_same_report() {
        let path = blank_save("determinism");
        let a = run_blank(&path, 1);
        let b = run_blank(&path, 1);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(a, b);
    }

    /// An empty `f32` sum is `-0.0`, which prints as "-0.00" and compares
    /// equal to zero only by accident of `==`.
    #[test]
    fn a_base_nobody_waits_on_reads_positive_zero_unworked() {
        let path = blank_save("unworked_zero");
        let report = run_blank(&path, 1);
        std::fs::remove_file(&path).unwrap();
        assert!(report.economy.labour.mean_unworked_total.is_sign_positive());
    }

    #[test]
    fn a_stopped_clock_is_an_error_not_a_quiet_base() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        advance(&mut game).unwrap();
        game.world
            .resource_mut::<crate::resources::GameOver>()
            .reason = Some("test".into());
        let err = advance(&mut game).unwrap_err();
        assert!(err.contains("clock stopped at tick"), "{err}");
    }

    #[test]
    fn a_refused_order_is_an_error_naming_the_item_and_the_reason() {
        let path = blank_save("refused_order");
        let err = run(
            &path,
            &test_assets_dir(),
            RunOptions {
                ticks: 1,
                seed: 0,
                orders: vec![("no_such_item".into(), 5)],
            },
        )
        .unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(err.contains("no_such_item"), "{err}");
        assert!(err.contains("refused"), "{err}");
    }

    #[test]
    fn a_missing_save_is_an_error_naming_the_path() {
        let err = run(
            Path::new("/nonexistent/feral_bench.bin"),
            &test_assets_dir(),
            RunOptions {
                ticks: 1,
                seed: 0,
                orders: vec![],
            },
        )
        .unwrap_err();
        assert!(err.contains("feral_bench.bin"), "{err}");
    }
}
