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

use report::share;
pub use report::{
    BenchReport, EconomyReport, LabourBench, LineBench, MEASURES, MachineReport, MemoryReport,
    MemoryTally, NeedBench, StaffReport, StaffSample, StaffTally, fold_status,
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
    /// Whether the siege clock runs. Off keeps a long run from stopping at
    /// the first siege (`stopped_at`), for measuring what a siege hides.
    pub sieges: bool,
}

/// Per-line tick tallies, accumulated as the run goes.
#[derive(Default)]
struct LineTally {
    status_ticks: BTreeMap<String, u64>,
    /// The tile of the line's end-of-line machine, as of the last tick seen.
    last_member: Option<(i32, i32)>,
}

pub fn run(save: &Path, assets_dir: &Path, opts: RunOptions) -> Result<BenchReport, String> {
    let game = Game::load(save, assets_dir).map_err(|e| format!("{}: {e}", save.display()))?;
    play(game, opts)
}

fn play(game: Game, opts: RunOptions) -> Result<BenchReport, String> {
    play_with(game, opts, |_, _| {})
}

/// `play`, with `before_tick(game, n)` run ahead of tick `n`, so a test can
/// stop the clock partway through, which no public call does.
fn play_with(
    mut game: Game,
    opts: RunOptions,
    mut before_tick: impl FnMut(&mut Game, u64),
) -> Result<BenchReport, String> {
    // Installed after the load, as `arena` does, so one save under several
    // seeds is several different runs and any one replays alone.
    game.world
        .insert_resource(GameRng(StdRng::seed_from_u64(opts.seed)));
    for (item, qty) in &opts.orders {
        game.queue_work_order(WorkOrder::batch(ItemId::from(item.as_str()), *qty))
            .map_err(|e| format!("order {item} x{qty} refused: {e}"))?;
    }
    game.dev_set_sieges(opts.sieges);
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
    let mut staff = StaffTally::default();
    // The game's own catalogue, so the seeded ids are exactly the ones
    // `remember` can resolve.
    let mut memories = MemoryTally::new(
        game.world
            .resource::<crate::memories::MemoryDb>()
            .all()
            .map(|d| d.id.as_str()),
    );
    let mut ticks = 0;
    let mut stopped_at = None;
    for _ in 0..opts.ticks {
        before_tick(&mut game, ticks);
        if !advance(&mut game) {
            stopped_at = Some(ticks);
            break;
        }
        ticks += 1;
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
        sample_staff(&game, &mut staff, &mut memories);
        let demand = game.labour_demand();
        wanted += demand.wanted as u64;
        staffed += demand.staff as u64;
        for (&duty, &n) in &demand.unworked {
            *unworked.entry(duty).or_insert(0) += n as u64;
        }
    }

    sample_bonds(&game, &mut memories);
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
                ticks,
            );
            let running = status_ticks.get("running").copied().unwrap_or(0);
            MachineReport {
                pos,
                kind,
                running_share: share(running, ticks),
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
                output_per_1000: if ticks == 0 {
                    0.0
                } else {
                    made as f32 * 1000.0 / ticks as f32
                },
                status_ticks: tally.status_ticks,
            }
        })
        .collect();
    let mean_unworked: BTreeMap<String, f32> = unworked
        .iter()
        .map(|(duty, &n)| (duty.name().to_string(), share(n, ticks)))
        .collect();
    let labour = LabourBench {
        mean_wanted: share(wanted, ticks),
        mean_staffed: share(staffed, ticks),
        mean_unworked_total: mean_unworked.values().fold(0.0, |a, v| a + v),
        mean_unworked,
    };

    Ok(BenchReport {
        ticks,
        stopped_at,
        seed: opts.seed,
        economy: EconomyReport {
            machines,
            lines,
            labour,
            items,
        },
        staff: staff.finish(t0, &records),
        memories: memories.finish(&records),
    })
}

/// One `wait`; false when it did nothing, which happens while a battle is
/// open or the game is over. The run ends there rather than counting on: a
/// bench that kept going would report a stopped clock as a quiet base.
fn advance(game: &mut Game) -> bool {
    let before = game.current_tick();
    game.wait();
    game.current_tick() != before
}

/// One tick's staff, re-read each tick because staff can join or leave.
fn sample_staff(game: &Game, tally: &mut StaffTally, memories: &mut MemoryTally) {
    tally.begin_tick();
    for who in game.base_staff() {
        let needs = game.need_levels(who);
        let grievance = game.grievance(who);
        let morale = game.morale(who);
        memories.add_morale(morale);
        tally.add(&StaffSample {
            on_shift: game.on_shift(who),
            morale,
            strain: game.need_strain(who),
            needs: needs
                .iter()
                .map(|n| (n.id.as_str(), n.level, n.critical))
                .collect(),
            rung: grievance,
        });
    }
}

/// Bonds are slow state, so the end of the run is what a target reads;
/// sampling `social` every tick would cost a lot and add nothing. Live
/// relationships only: a departed program's bond is a memory of grief, which
/// the formation records already count.
fn sample_bonds(game: &Game, memories: &mut MemoryTally) {
    for who in game.base_staff() {
        let Some(social) = game.social(who) else {
            continue;
        };
        for row in social.relationships.iter().filter(|r| !r.gone) {
            memories.add_bond(row.bond);
        }
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
                sieges: true,
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
    fn a_stopped_clock_ends_the_run_early_and_says_where() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource_mut::<crate::resources::GameOver>()
            .reason = Some("test".into());
        let report = play(
            game,
            RunOptions {
                ticks: 50,
                seed: 1,
                orders: vec![],
                sieges: true,
            },
        )
        .unwrap();
        assert_eq!(report.stopped_at, Some(0));
        assert_eq!(report.ticks, 0);
    }

    #[test]
    fn a_run_stopped_midway_is_folded_and_rated_over_the_ticks_it_ran() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.spawn((
            Structure {
                kind: "bay".to_string(),
            },
            Position { x: 3, y: 3 },
            MachineStatus::Running,
        ));
        let report = play_with(
            game,
            RunOptions {
                ticks: 50,
                seed: 1,
                orders: vec![],
                sieges: true,
            },
            |game, n| {
                if n == 20 {
                    game.world
                        .resource_mut::<crate::resources::GameOver>()
                        .reason = Some("test".into());
                }
            },
        )
        .unwrap();
        assert_eq!(report.stopped_at, Some(20));
        assert_eq!(report.ticks, 20);
        let machines = &report.economy.machines;
        assert!(!machines.is_empty(), "the fixture machine reports a status");
        for m in machines {
            assert_eq!(m.status_ticks.values().sum::<u64>(), 20, "{m:?}");
            let running = m.status_ticks.get("running").copied().unwrap_or(0);
            assert_eq!(m.running_share, running as f32 / 20.0);
        }
    }

    #[test]
    fn a_run_that_plays_out_has_no_stop() {
        let path = blank_save("no_stop");
        let report = run_blank(&path, 1);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(report.stopped_at, None);
        assert_eq!(report.ticks, 50);
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
                sieges: true,
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
                sieges: true,
            },
        )
        .unwrap_err();
        assert!(err.contains("feral_bench.bin"), "{err}");
    }
}
