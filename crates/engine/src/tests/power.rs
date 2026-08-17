//! The base power ledger (`game::base::power::ledger`), unit-tested directly
//! against a bare `World` and a `StructureDb` built from scratch.
//!
//! `ledger` takes neither a `Game` nor any shipped asset, so these tests
//! don't lean on either: every shipped `power_draw`/`power_supply` is
//! authored `0` until Task 4 lands the real numbers (see
//! `docs/superpowers/specs/2026-08-17-base-power-grid-design.md`), so
//! reading them here would make every assertion vacuous. Each test builds
//! its own tiny `StructureDb` out of throwaway ids instead.

use bevy_ecs::prelude::*;

use super::support::scratch_assets_dir;
use crate::components::{Position, Structure};
use crate::game::base::power::ledger;
use crate::structures::StructureDb;

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
