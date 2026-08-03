//! Adjacency-fed production chains: what a machine pulls, and from where.

use super::support::*;
use crate::*;

/// A modded assembler that builds Power Cells. Its recipe is *not* written
/// here — the machine runs `power_cell.ron`'s own `craftable.cost`, which is
/// the whole point of the `assembles` field.
const TEST_ASSEMBLER: &str = r#"(
    id: "test_assembler",
    name: "Test Assembler",
    description: "A modded assembler, for tests.",
    glyph: 'A',
    color: Cyan,
    build_cost: [],
    work: None,
    capacity: 20,
    assembles: Some((item: "power_cell", ticks_per_unit: 3)),
)"#;

/// A game whose asset set includes `test_assembler`. The caller drops the
/// scratch directory; `Game` has already read everything it needs.
fn game_with_assembler(tag: &str, seed: u32) -> Game {
    let dir = assets_dir_with_extra_structure(tag, "test_assembler.ron", TEST_ASSEMBLER);
    let game = Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    game
}

/// How many Core Fragments one batch of the assembler's recipe costs — read
/// off the shipped item file rather than restated, so this can't drift from
/// what the machine actually runs.
fn per_batch(game: &Game) -> u32 {
    game.world
        .resource::<ItemDb>()
        .get(ids::POWER_CELL)
        .and_then(|d| d.craftable.as_ref())
        .expect("power_cell ships with a recipe")
        .cost
        .iter()
        .find(|(i, _)| i.as_str() == ids::CORE_FRAGMENT)
        .map(|(_, n)| *n)
        .expect("its recipe is priced in core fragments")
}

/// A feeder holding `output` Core Fragments at an absolute tile.
fn feeder_at(game: &mut Game, x: i32, y: i32, output: u32) -> Entity {
    let mut stock = Stock::new(1000);
    stock
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), output);
    game.world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x, y },
            stock,
        ))
        .id()
}

/// An assembler at an absolute tile, staffed unless `staffed` is false.
fn assembler_at(game: &mut Game, x: i32, y: i32, staffed: bool) -> Entity {
    let machine = game
        .world
        .spawn((
            Structure {
                kind: "test_assembler".to_string(),
            },
            Position { x, y },
            Stock::new(20),
            MachineStatus::default(),
        ))
        .id();
    if staffed {
        let worker = spawn_tamed(game, 10, 3);
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: machine,
            progress: 0,
            required: 3,
        });
    }
    machine
}

fn input_of(game: &Game, machine: Entity, item: &str) -> u32 {
    game.world
        .get::<Stock>(machine)
        .and_then(|s| s.input.get(&ItemId::from(item)).copied())
        .unwrap_or(0)
}

#[test]
fn an_orthogonal_neighbour_feeds_the_machine() {
    let mut game = game_with_assembler("chain_ortho", 1000);
    let machine = assembler_at(&mut game, 40, 40, true);
    feeder_at(&mut game, 41, 40, 10);

    game.tick();

    assert!(
        input_of(&game, machine, ids::CORE_FRAGMENT) > 0,
        "a machine takes from the output of the structure touching it"
    );
}

/// Diagonals feed nothing. Without this a base is a blob rather than a line,
/// and layout stops being a decision.
#[test]
fn a_diagonal_neighbour_feeds_nothing() {
    let mut game = game_with_assembler("chain_diag", 1001);
    let machine = assembler_at(&mut game, 40, 40, true);
    let feeder = feeder_at(&mut game, 41, 41, 10);

    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(input_of(&game, machine, ids::CORE_FRAGMENT), 0);
    assert_eq!(
        game.world
            .get::<Stock>(feeder)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied(),
        Some(10),
        "and the feeder keeps everything it had"
    );
}

/// A greedy machine must not drain a feeder several machines share. Two
/// batches staged is enough to keep working while the next one arrives.
#[test]
fn input_stops_at_two_batches_and_leaves_the_rest_in_the_feeder() {
    let mut game = game_with_assembler("chain_cap", 1002);
    let batch = per_batch(&game);
    let stocked = batch * 10;
    // Unstaffed, so the pull is observable without the work phase spending
    // what it just took.
    let machine = assembler_at(&mut game, 40, 40, false);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 10_000,
    });
    let feeder = feeder_at(&mut game, 41, 40, stocked);

    for _ in 0..10 {
        game.tick();
    }

    let cap = batch * crate::tuning::INPUT_STOCK_BATCHES;
    assert_eq!(
        input_of(&game, machine, ids::CORE_FRAGMENT),
        cap,
        "a machine stages two batches and stops asking"
    );
    assert_eq!(
        game.world
            .get::<Stock>(feeder)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied(),
        Some(stocked - cap),
        "the rest stays in the feeder for whoever else is on it"
    );
}

/// Asserted by *which* machine won, not merely that someone did — an
/// order-independent assertion would pass under exactly the bug this test
/// exists to catch. Machines are visited in `(x, y)` order, so the one at
/// the lower x takes first.
#[test]
fn two_machines_competing_for_one_scarce_feeder_resolve_in_position_order() {
    let mut game = game_with_assembler("chain_order", 1003);
    let batch = per_batch(&game);

    // The feeder at (41, 40) touches both: west neighbour at (40, 40) and
    // east neighbour at (42, 40). One batch between them.
    //
    // `east` is spawned *first* deliberately. Spawning in position order
    // would let this pass on bevy's iteration order alone, which is the
    // exact bug the sort exists to prevent.
    let east = assembler_at(&mut game, 42, 40, true);
    let west = assembler_at(&mut game, 40, 40, true);
    feeder_at(&mut game, 41, 40, batch);

    game.tick();

    assert_eq!(
        input_of(&game, west, ids::CORE_FRAGMENT),
        batch,
        "the machine at the lower x is visited first and takes what there is"
    );
    assert_eq!(
        input_of(&game, east, ids::CORE_FRAGMENT),
        0,
        "and the one behind it in sort order gets nothing this tick"
    );
}

#[test]
fn a_machine_adjacent_to_nothing_pulls_nothing_and_does_not_panic() {
    let mut game = game_with_assembler("chain_alone", 1004);
    let machine = assembler_at(&mut game, 40, 40, true);

    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(input_of(&game, machine, ids::CORE_FRAGMENT), 0);
}

/// An unstaffed machine hoarding from a shared feeder would starve the line
/// beside it while producing nothing itself.
#[test]
fn a_machine_with_no_program_pulls_nothing() {
    let mut game = game_with_assembler("chain_idle", 1005);
    let machine = assembler_at(&mut game, 40, 40, false);
    feeder_at(&mut game, 41, 40, 10);

    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(input_of(&game, machine, ids::CORE_FRAGMENT), 0);
}
