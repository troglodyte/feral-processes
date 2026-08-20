//! The base power grid, in two halves.
//!
//! **The ledger** (`game::base::power::ledger`), unit-tested directly against
//! a bare `World` and a `StructureDb` built from scratch. `ledger` takes
//! neither a `Game` nor any shipped asset, so those tests don't lean on
//! either: every shipped `power_draw`/`power_supply` is authored `0` until
//! Task 4 lands the real numbers (see
//! `docs/superpowers/specs/2026-08-17-base-power-grid-design.md`), so reading
//! them here would make every assertion vacuous. Each test builds its own
//! tiny `StructureDb` out of throwaway ids instead.
//!
//! **The systems** (`systems::power_grid_system`, the `Unpowered` writer in
//! `idle_machine_system`, and the three guards — cronjob, assembler and the
//! player's own hands), driven through a real `Game` and a real tick. Those
//! fixtures install their own structure defs on top of a scratch copy of the
//! shipped assets — see `grid_assets` for why the numbers are absurd rather
//! than realistic.

use bevy_ecs::prelude::*;

use super::support::{
    ScratchAssets, copy_shipped_assets, find_structure_by_kind, node_output, park_at_post,
    scratch_assets_dir, spawn_machine_at, spawn_structure_at, spawn_tamed, stand_in_base,
    stand_in_base_at, stand_player_at_post,
};
use crate::components::{MachineStatus, Position, PowerReserve, Stock, Structure, Task};
use crate::game::base::power::ledger;
use crate::items::{ItemId, ids};
use crate::structures::StructureDb;
use crate::{DifficultyMode, Game};

/// A structure that never runs a job: no `work`, no `assembles`. `power_draw`
/// is deliberately settable and non-zero on some fixtures even though no
/// shipped structure would ever author one there — that is what makes
/// `draw_sums_only_over_structures_that_run_a_job` a real test of the
/// `runs_a_job()` filter, rather than one that would pass just as well
/// summing every structure's draw.
fn passive_ron(id: &str, power_supply: u32, power_draw: u32) -> String {
    format!(
        r#"(
    id: "{id}",
    name: "{id}",
    glyph: 'x',
    color: White,
    build_cost: [],
    work: None,
    power_supply: {power_supply},
    power_draw: {power_draw},
)"#
    )
}

/// A structure that runs a job — `work` is set, which is all `runs_a_job()`
/// checks for. `produces` names an item that need not exist anywhere:
/// `StructureDb::load_dir` never looks it up, and neither does `ledger`.
fn machine_ron(id: &str, power_draw: u32) -> String {
    format!(
        r#"(
    id: "{id}",
    name: "{id}",
    glyph: 'x',
    color: White,
    build_cost: [],
    work: Some((
        produces: "test_item",
        ticks_per_unit: 1,
    )),
    power_draw: {power_draw},
)"#
    )
}

/// Loads a `StructureDb` from exactly the defs given. The scratch directory
/// behind it is gone before this returns — `ledger` only ever needs the
/// `StructureDb` itself, never the files it was read from.
fn structure_db(defs: &[String]) -> StructureDb {
    let dir = scratch_assets_dir("power_ledger");
    std::fs::create_dir_all(&dir).unwrap();
    for (i, body) in defs.iter().enumerate() {
        std::fs::write(dir.join(format!("s{i}.ron")), body).unwrap();
    }
    let (db, warnings) = StructureDb::load_dir(&dir).unwrap();
    assert!(
        warnings.is_empty(),
        "bad fixture structure ron: {warnings:?}"
    );
    db
}

/// Spawns the one component pair `ledger` reads: `Structure` for the kind
/// lookup, `Position` for the `(x, y)` cut order.
fn spawn(world: &mut World, kind: &str, x: i32, y: i32) -> Entity {
    world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
        ))
        .id()
}

#[test]
fn supply_sums_across_home_and_every_recharger() {
    let db = structure_db(&[
        passive_ron("test_home", 4, 0),
        passive_ron("test_recharger", 4, 0),
    ]);
    let mut world = World::new();
    spawn(&mut world, "test_home", 0, 0);
    spawn(&mut world, "test_recharger", 5, 5);
    spawn(&mut world, "test_recharger", 7, 7);

    let result = ledger(&world, &db);

    assert_eq!(
        result.supply, 12,
        "a Home and two Rechargers should report the authored total, 4 + 4 + 4"
    );
    assert_eq!(result.draw, 0);
    assert!(result.dark.is_empty());
}

#[test]
fn draw_sums_only_over_structures_that_run_a_job() {
    let db = structure_db(&[
        // Plenty of supply so nothing here is a cut question — this test is
        // about which structures count toward `draw`, not the budget.
        passive_ron("test_home", 100, 0),
        // A Depot and a Shield stand-in: passive, but deliberately given a
        // non-zero draw so the test actually exercises the `runs_a_job()`
        // filter rather than passing because every non-machine happens to
        // draw 0.
        passive_ron("test_depot", 0, 9),
        passive_ron("test_shield", 0, 9),
        machine_ron("test_machine_a", 2),
        machine_ron("test_machine_b", 3),
    ]);
    let mut world = World::new();
    spawn(&mut world, "test_home", 0, 0);
    spawn(&mut world, "test_depot", 1, 0);
    spawn(&mut world, "test_shield", 2, 0);
    spawn(&mut world, "test_machine_a", 3, 0);
    spawn(&mut world, "test_machine_b", 4, 0);

    let result = ledger(&world, &db);

    assert_eq!(
        result.draw, 5,
        "draw counts the two machines alone, not the Depot or Shield beside them"
    );
}

#[test]
fn a_base_exactly_at_capacity_has_nothing_dark() {
    // Written at the exact boundary — supply equals draw — rather than one
    // under it, since a `<` implementation instead of `<=` only fails here.
    let db = structure_db(&[
        passive_ron("test_home", 5, 0),
        machine_ron("test_machine", 5),
    ]);
    let mut world = World::new();
    spawn(&mut world, "test_home", 0, 0);
    spawn(&mut world, "test_machine", 1, 0);

    let result = ledger(&world, &db);

    assert_eq!(result.supply, 5);
    assert_eq!(result.draw, 5);
    assert!(
        result.dark.is_empty(),
        "a base exactly at capacity should have nothing dark"
    );
}

#[test]
fn the_cut_order_is_by_position_not_spawn_order() {
    let db = structure_db(&[
        passive_ron("test_home", 2, 0),
        machine_ron("test_machine", 2),
    ]);
    let mut world = World::new();
    spawn(&mut world, "test_home", 0, 0);
    // `east` is spawned *first* deliberately — spawning in position order
    // would let this pass on bevy's iteration order alone, which is the
    // exact bug the `(x, y)` sort exists to prevent. Two machines each draw
    // the base's entire 2-unit supply, so only one of them can run.
    let east = spawn(&mut world, "test_machine", 42, 40);
    let west = spawn(&mut world, "test_machine", 40, 40);

    let result = ledger(&world, &db);

    assert!(
        !result.dark.contains(&west),
        "the machine at the lower x is cut last, so it should still run"
    );
    assert!(
        result.dark.contains(&east),
        "and the one behind it in sort order loses the cut"
    );
}

#[test]
fn a_machine_too_big_for_the_budget_does_not_darken_the_one_behind_it() {
    // Three machines, sorted `big`, `small`, `tail` by position, against a
    // 2-unit budget. `small` fits what `big` left untouched — the scenario
    // the spec names. `tail` is what makes the test actually prove the loop
    // *kept going* rather than stopping at `big`'s failure: `tail` doesn't
    // fit what's left after `small` runs, so it must come back dark too. A
    // `break` at the first failure would skip `small` and `tail` both,
    // leaving `tail` wrongly *not* dark — indistinguishable from "it ran"
    // if this test only checked `small`.
    let db = structure_db(&[
        passive_ron("test_home", 2, 0),
        machine_ron("test_big", 3),
        machine_ron("test_small", 1),
        machine_ron("test_tail", 2),
    ]);
    let mut world = World::new();
    spawn(&mut world, "test_home", 0, 0);
    // Sorted first by position; its 3-draw doesn't fit the 2-unit budget.
    let big = spawn(&mut world, "test_big", 0, 1);
    // Sorted second; its 1-draw fits what `big` left untouched, dropping
    // the budget to 1.
    let small = spawn(&mut world, "test_small", 1, 1);
    // Sorted third; its 2-draw no longer fits the 1 unit left.
    let tail = spawn(&mut world, "test_tail", 2, 1);

    let result = ledger(&world, &db);

    assert!(
        result.dark.contains(&big),
        "a 3-draw machine that doesn't fit a 2-unit budget goes dark"
    );
    assert!(
        !result.dark.contains(&small),
        "the loop must not stop at the first failure — the 1-draw machine \
         behind it still fits and should keep running"
    );
    assert!(
        result.dark.contains(&tail),
        "and the loop must keep evaluating past a machine that ran too — \
         `tail` no longer fits the budget `small` left behind, so it must \
         come back dark rather than being skipped by an early break"
    );
}

#[test]
fn an_unstaffed_machine_still_draws() {
    // No `Task` is spawned anywhere in this world — nothing is staffing the
    // machine. Pinned so nobody "fixes" the ledger into a staffing-dependent
    // one later: the building draws whether or not anyone is posted to it.
    let db = structure_db(&[
        passive_ron("test_home", 10, 0),
        machine_ron("test_machine", 3),
    ]);
    let mut world = World::new();
    spawn(&mut world, "test_home", 0, 0);
    spawn(&mut world, "test_machine", 1, 0);

    let result = ledger(&world, &db);

    assert_eq!(
        result.draw, 3,
        "an unstaffed machine still draws its full power_draw"
    );
    assert!(result.dark.is_empty());
}

// ---------------------------------------------------------------------------
// The systems: the ledger wired into the tick, the one writer, three guards.
// ---------------------------------------------------------------------------

/// A working extractor whose draw is far past anything a base could ever
/// supply. Deliberately absurd rather than realistic: Task 4 authors the real
/// `power_supply` onto Home and the Recharger Node *after* this task, and a
/// fixture machine drawing a plausible 4 would silently stop being dark the
/// day those numbers land. A draw of a million is dark under any supply this
/// game will ever ship, which is what keeps these tests reading the guard
/// rather than reading the asset numbers of the day.
const GREEDY_NODE: &str = r#"(
    id: "test_greedy_node",
    name: "Greedy Node",
    glyph: 'g',
    color: Brown,
    build_cost: [],
    work: Some((produces: "core_fragment", ticks_per_unit: 40)),
    capacity: 100,
    power_draw: 1000000,
)"#;

/// The same, for the *other* guard: `assembler_system` visits only structures
/// declaring `assembles`, so the extractor above is invisible to it and one
/// fixture cannot cover both. `blank_substrate` is the shipped item whose
/// `craftable.cost` becomes the recipe (see `systems::assembly_recipe`).
const GREEDY_LATHE: &str = r#"(
    id: "test_greedy_lathe",
    name: "Greedy Lathe",
    glyph: 'l',
    color: Brown,
    build_cost: [],
    work: None,
    capacity: 100,
    assembles: Some((item: "blank_substrate", ticks_per_unit: 40)),
    power_draw: 1000000,
)"#;

/// The same extractor on a one-tick cycle, for the hand-work guard. The short
/// cycle is the point: `hand_working_a_dark_machine_yields_nothing` has to be
/// a test about a machine that really would have *paid out*, not one that
/// would merely have inched along, and `GREEDY_NODE`'s 40-tick cycle cannot
/// deliver that inside a short fixture.
///
/// No `level` on the `work`, deliberately, exactly as `GREEDY_NODE` has none:
/// `resolve_gather_cycle` rolls its fizzle check only for a node with a
/// `level`, so this node's payout is deterministic and the test cannot flake
/// on a bad roll. It also means these fixtures make no `GameRng` draw at all,
/// so nothing here can shift the stream either way.
const GREEDY_QUICK_NODE: &str = r#"(
    id: "test_greedy_quick_node",
    name: "Greedy Quick Node",
    glyph: 'q',
    color: Brown,
    build_cost: [],
    work: Some((produces: "core_fragment", ticks_per_unit: 1)),
    capacity: 100,
    power_draw: 1000000,
)"#;

/// Enough supply to light either of the two above, for the half of a test
/// that is about coming *back*.
const GRID_SOURCE: &str = r#"(
    id: "test_grid_source",
    name: "Grid Source",
    glyph: 'G',
    color: Orange,
    build_cost: [],
    work: None,
    power_supply: 2000000,
)"#;

/// A scratch install of the shipped assets carrying the three fixture
/// structures above. The shipped set has to come along whole — `Game::new`
/// checks for the structures that fill required roles — so this is
/// `copy_shipped_assets` plus three files, not a hand-built install.
fn grid_assets(tag: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    for (name, body) in [
        ("test_greedy_node.ron", GREEDY_NODE),
        ("test_greedy_quick_node.ron", GREEDY_QUICK_NODE),
        ("test_greedy_lathe.ron", GREEDY_LATHE),
        ("test_grid_source.ron", GRID_SOURCE),
    ] {
        std::fs::write(dir.join("structures").join(name), body).unwrap();
    }
    dir
}

/// A game on an install that has the fixture structures but no supply
/// standing anywhere — so any fixture machine deployed into it is dark from
/// the first tick.
fn game_on_a_short_grid(tag: &str, seed: u32) -> Game {
    let dir = grid_assets(tag);
    Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap()
}

fn status_of(game: &Game, machine: Entity) -> Option<MachineStatus> {
    game.world.get::<MachineStatus>(machine).copied()
}

/// How many logged lines contain `needle`. Reads the whole log rather than a
/// recent window: the counts these tests assert on are *totals across the
/// run*, which is the whole point of "exactly once".
fn log_hits(game: &Game, needle: &str) -> usize {
    game.message_log(usize::MAX)
        .into_iter()
        .filter(|line| line.text.contains(needle))
        .count()
}

/// Posts `worker` to `machine` and leaves it standing at its post, so the
/// machine is one a working tick really would advance.
///
/// This is the fixture the vacuous-test trap turns on: a machine with nobody
/// posted to it makes no progress anyway, so a "no progress" assertion
/// against an unstaffed machine passes with the entire feature deleted.
fn post_a_worker_at(game: &mut Game, machine: Entity) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    stand_player_at_post(game, machine);
    game.assign_cronjob(worker, machine).unwrap();
    park_at_post(game, worker, machine);
    worker
}

#[test]
fn a_dark_machine_makes_no_progress_on_a_cronjob() {
    let mut game = game_on_a_short_grid("power_dark_cronjob", 4001);
    // Posting and hand-working are base actions; the party stands in the base.
    stand_in_base(&mut game);
    let node = spawn_machine_at(&mut game, "test_greedy_node", 3, 4);
    let worker = post_a_worker_at(&mut game, node);

    for _ in 0..3 {
        game.tick();
    }

    assert_eq!(
        status_of(&game, node),
        Some(MachineStatus::Unpowered),
        "a machine drawing more than the base supplies must report Unpowered"
    );
    // `ticks_per_unit` is 40 on the fixture, so an unguarded run cannot have
    // completed a cycle and reset `progress` to 0 behind our backs — a
    // non-zero count is the only thing four ticks of work could look like.
    assert_eq!(
        game.world.get::<Task>(worker).unwrap().progress,
        0,
        "the posted program is standing at its post and would otherwise be \
         advancing — a dark machine must make no progress at all"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        0,
        "and nothing reaches its output buffer"
    );
}

#[test]
fn a_dark_assembler_makes_no_progress() {
    let mut game = game_on_a_short_grid("power_dark_assembler", 4002);
    // Posting and hand-working are base actions; the party stands in the base.
    stand_in_base(&mut game);
    let lathe = spawn_machine_at(&mut game, "test_greedy_lathe", 3, 4);
    // Fed and roomy before the worker is posted, because those are
    // `assembler_system`'s other two stalls: a test about the power guard
    // must not be able to pass because the machine was starved or clogged.
    game.world
        .get_mut::<Stock>(lathe)
        .unwrap()
        .input
        .insert(ItemId::from(ids::CORE_FRAGMENT), 40);
    let worker = post_a_worker_at(&mut game, lathe);

    for _ in 0..3 {
        game.tick();
    }

    assert_eq!(
        status_of(&game, lathe),
        Some(MachineStatus::Unpowered),
        "a dark assembler reports Unpowered, not Starved or Running"
    );
    assert_eq!(
        game.world.get::<Task>(worker).unwrap().progress,
        0,
        "a fed, roomy, staffed assembler that is dark must make no progress"
    );
    assert!(
        game.world.get::<Stock>(lathe).unwrap().output.is_empty(),
        "and it must assemble nothing"
    );
}

#[test]
fn hand_working_a_dark_machine_yields_nothing() {
    // The third guard, in `player_gather_system`. The player is the one
    // cranking the handle here, not a posted program, and this path is the
    // largest hole in "nothing the player can do makes a dark machine run":
    // it calls `deliver_payout`, and on its payout tick it also writes
    // `Running` over the `Unpowered` that `idle_machine_system` set earlier in
    // the same tick.
    //
    // `test_greedy_quick_node` runs a one-tick cycle with a deterministic
    // payout, so with the guard gone this machine does not merely inch along —
    // it pays out, repeatedly, inside the window below.
    let mut game = game_on_a_short_grid("power_hand_work", 4007);
    // Posting and hand-working are base actions; the party stands in the base.
    stand_in_base(&mut game);
    let node = spawn_machine_at(&mut game, "test_greedy_quick_node", 3, 4);
    // `work_structure` refuses a node the player is not at the station of, so
    // this stand is what makes the hand-work legal — and it ticks once itself.
    stand_player_at_post(&mut game, node);
    game.work_structure(node).unwrap();
    for _ in 0..5 {
        game.tick();
    }

    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Task>(player).unwrap().progress,
        0,
        "the player is mid-gather at the node and would otherwise be \
         advancing — a dark machine must make no progress under a hand, \
         either"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        0,
        "and nothing may be hand-cranked out of a machine the ledger has \
         already declared dark"
    );
    assert_eq!(
        status_of(&game, node),
        Some(MachineStatus::Unpowered),
        "the status stays Unpowered too: this system must not write Running \
         over it on a payout tick, which is the twice-per-tick transition \
         logging the design refused"
    );
    assert_eq!(
        log_hits(&game, "is dark — the grid can't power it."),
        1,
        "and going dark is still news exactly once across all six ticks"
    );
}

#[test]
fn a_dark_and_unstaffed_machine_reports_unpowered_not_idle() {
    // Both facts are true of this machine at once: nobody is posted to it
    // *and* it is dark. An implementation that writes `Idle` after the dark
    // check — or that checks `worked` first and skips a dark machine because
    // nobody is on it — reports `Idle` here, which is the precedence this
    // test exists to pin.
    let mut game = game_on_a_short_grid("power_precedence", 4003);
    let node = spawn_machine_at(&mut game, "test_greedy_node", 3, 4);

    game.tick();

    assert_eq!(
        status_of(&game, node),
        Some(MachineStatus::Unpowered),
        "Unpowered outranks Idle: nothing the player does to an idle machine \
         makes a dark one run, so the dark reading is the one to show"
    );
}

#[test]
fn going_dark_and_coming_back_each_log_exactly_once() {
    // The regression guarding the refused ordering. A power system running
    // *last* would have `idle_machine_system` write `Idle` and the power
    // system overwrite it with `Unpowered` on every tick, and
    // `set_machine_status` logs on every transition — so the dark line would
    // land four times below, not once.
    let mut game = game_on_a_short_grid("power_log_once", 4004);
    spawn_machine_at(&mut game, "test_greedy_node", 3, 4);

    for _ in 0..4 {
        game.tick();
    }

    assert_eq!(
        log_hits(&game, "is dark — the grid can't power it."),
        1,
        "going dark is news once, not once per tick"
    );

    spawn_structure_at(&mut game, "test_grid_source", 5, 4);
    for _ in 0..4 {
        game.tick();
    }

    assert_eq!(
        log_hits(&game, "is dark — the grid can't power it."),
        1,
        "and standing up supply must not re-announce the state it left"
    );
    assert_eq!(
        log_hits(&game, "sits idle — no program is assigned."),
        1,
        "coming back is news exactly once too — the machine is unstaffed, so \
         it lands on Idle and says so a single time"
    );
}

#[test]
fn power_regen_still_refills_the_party_on_a_dark_base() {
    // The two Powers do not touch. `power_regen_system` restores the
    // player's `PowerReserve` from a Recharger's `power_regen`, which is a
    // different resource from the base grid entirely, and a base short of
    // grid capacity must not stop it.
    //
    // `stand_in_base_at`, not `stand_player_at`: `power_regen_system` reads
    // `Locale::Base`'s own cell, never the player's `Position`, now that a
    // `Structure` can only ever stand in base space.
    let mut game = game_on_a_short_grid("power_regen_dark", 4005);
    let node = spawn_machine_at(&mut game, "test_greedy_node", 3, 4);
    spawn_structure_at(&mut game, "recharger_node", 3, 6);
    stand_in_base_at(&mut game, 4, 6);
    let player = game.player_entity();
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(40.0);
    let before = game.world.get::<PowerReserve>(player).unwrap().get();

    game.tick();

    let after = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        after > before,
        "the Recharger's regen ({before} -> {after}) must survive the base \
         being over its grid capacity"
    );
    assert_eq!(
        status_of(&game, node),
        Some(MachineStatus::Unpowered),
        "and the base really is short while it does — otherwise this test \
         proves nothing about a dark base"
    );
}

#[test]
fn a_base_over_capacity_survives_a_save_round_trip() {
    // The proof of the "no `SAVE_FORMAT_VERSION` bump" claim. `MachineStatus`
    // is not saved at all, so what has to survive is the *base*: a save
    // written while a machine was dark must load without erroring, and the
    // first tick after the load must decide it dark again off the standing
    // structures alone.
    let dir = grid_assets("power_save_roundtrip");
    let mut game = Game::new(4006, DifficultyMode::Forgiving, &dir).unwrap();
    spawn_machine_at(&mut game, "test_greedy_node", 3, 4);
    game.tick();
    let node = find_structure_by_kind(&mut game, "test_greedy_node").unwrap();
    assert_eq!(
        status_of(&game, node),
        Some(MachineStatus::Unpowered),
        "the base is over capacity before it is saved"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_power_over_capacity_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);

    let reloaded = find_structure_by_kind(&mut loaded, "test_greedy_node")
        .expect("the machine must come back off the save");
    assert_eq!(
        status_of(&loaded, reloaded),
        Some(MachineStatus::default()),
        "MachineStatus is not saved — it comes back at its default and the \
         first tick is what decides"
    );
    loaded.tick();
    assert_eq!(
        status_of(&loaded, reloaded),
        Some(MachineStatus::Unpowered),
        "and that first tick must find it dark again, from the standing \
         structures alone"
    );
}

// ---------------------------------------------------------------------------
// The view: `Game::base_power`, which the base pane's `Grid` header reads.
// ---------------------------------------------------------------------------

#[test]
fn base_power_reports_draw_and_supply_before_the_first_tick() {
    // No `game.tick()` anywhere in this test — that omission is the whole
    // point. `Game::new` leaves `resources::PowerGrid` at its `Default`
    // (supply 0, draw 0) until `systems::power_grid_system` first runs, so
    // an implementation of `base_power` that reads the resource instead of
    // calling `ledger` directly would report `(0, 0)` here rather than the
    // numbers the standing structures actually add up to.
    let mut game = game_on_a_short_grid("power_view_before_tick", 4008);
    spawn_machine_at(&mut game, "test_greedy_node", 3, 4);
    spawn_structure_at(&mut game, "test_grid_source", 5, 4);

    let (draw, supply) = game.base_power();

    assert_eq!(
        draw, 1_000_000,
        "the one deployed machine's full, unstaffed draw"
    );
    assert_eq!(
        supply, 2_000_000,
        "the one deployed source's supply — `Game::new` deploys no Home or \
         any other structure on its own, so this is the total"
    );
}
