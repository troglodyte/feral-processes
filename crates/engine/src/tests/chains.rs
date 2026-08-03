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
    let machine = assembler_at(&mut game, 40, 40, true);
    // Deliberately clogged, so the pull is observable without the work phase
    // spending what it just took. A clogged machine still pulls — bounded at
    // two batches, and it means the line restarts the instant the clog
    // clears rather than a cycle later.
    {
        let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
        let capacity = stock.capacity;
        stock
            .output
            .insert(ItemId::from(ids::ICE_BREAKER), capacity);
    }
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

fn output_of(game: &Game, machine: Entity, item: &str) -> u32 {
    game.world
        .get::<Stock>(machine)
        .and_then(|s| s.output.get(&ItemId::from(item)).copied())
        .unwrap_or(0)
}

fn status_of(game: &Game, machine: Entity) -> Option<MachineStatus> {
    game.world.get::<MachineStatus>(machine).copied()
}

fn log_hits(game: &Game, needle: &str) -> usize {
    game.message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.text.contains(needle))
        .count()
}

/// The two-machine line, end to end: an extractor feeding an assembler that
/// turns its output into something else. This is the smallest thing the
/// design has to make worth building.
#[test]
fn a_fed_and_staffed_machine_turns_a_batch_into_product() {
    let mut game = game_with_assembler("chain_build", 1006);
    let batch = per_batch(&game);
    let machine = assembler_at(&mut game, 40, 40, true);
    // Stocked far past what twenty ticks can consume, so the machine is
    // still Running at the end rather than having simply eaten the feeder.
    feeder_at(&mut game, 41, 40, batch * 50);

    for _ in 0..20 {
        game.tick();
    }

    assert!(
        output_of(&game, machine, ids::POWER_CELL) > 0,
        "a fed, staffed, roomy machine builds the item it assembles"
    );
    assert_eq!(status_of(&game, machine), Some(MachineStatus::Running));
}

/// Every machine needs an assigned program, assemblers included — that is
/// what makes roster capacity, not fragments, buy chain length.
#[test]
fn a_machine_with_no_program_advances_nothing_and_reports_idle() {
    let mut game = game_with_assembler("chain_noprog", 1007);
    let batch = per_batch(&game);
    let machine = assembler_at(&mut game, 40, 40, false);
    // Pre-stocked directly, since an unstaffed machine cannot pull either.
    game.world
        .get_mut::<Stock>(machine)
        .unwrap()
        .input
        .insert(ItemId::from(ids::CORE_FRAGMENT), batch * 2);

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(output_of(&game, machine, ids::POWER_CELL), 0);
    assert_eq!(status_of(&game, machine), Some(MachineStatus::Idle));
}

#[test]
fn a_starved_machine_advances_no_progress() {
    let mut game = game_with_assembler("chain_starved", 1008);
    let short = per_batch(&game) - 1;
    let machine = assembler_at(&mut game, 40, 40, true);
    // A feeder holding one unit less than a batch: adjacent, staffed, and
    // still short.
    feeder_at(&mut game, 41, 40, short);

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(output_of(&game, machine, ids::POWER_CELL), 0);
    assert_eq!(status_of(&game, machine), Some(MachineStatus::Starved));
}

/// Asserts the input is *still there*: consuming the batch and then
/// discarding the product is the plausible wrong implementation, and it
/// looks identical from the output side.
#[test]
fn a_clogged_machine_does_not_consume_its_input() {
    let mut game = game_with_assembler("chain_clog", 1009);
    let batch = per_batch(&game);
    let machine = assembler_at(&mut game, 40, 40, true);
    feeder_at(&mut game, 41, 40, batch * 4);
    {
        let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
        let capacity = stock.capacity;
        stock
            .output
            .insert(ItemId::from(ids::ICE_BREAKER), capacity);
    }

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(status_of(&game, machine), Some(MachineStatus::Clogged));
    assert_eq!(
        output_of(&game, machine, ids::POWER_CELL),
        0,
        "nothing was built"
    );
    assert!(
        input_of(&game, machine, ids::CORE_FRAGMENT) >= batch,
        "and the batch it could not deliver is still staged, not spent"
    );
}

/// A stalled base must not flood the log pane.
#[test]
fn a_machine_stalled_for_twenty_ticks_says_so_once() {
    let mut game = game_with_assembler("chain_once", 1010);
    let machine = assembler_at(&mut game, 40, 40, true);

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(status_of(&game, machine), Some(MachineStatus::Starved));
    assert_eq!(
        log_hits(&game, "is starved"),
        1,
        "entering the state is news; staying in it is not"
    );
}

/// The stall clears when its cause does, and the recovery is worth a line —
/// otherwise the player has no way to know their trip home fixed anything.
#[test]
fn feeding_a_starved_machine_resumes_it_and_says_so() {
    let mut game = game_with_assembler("chain_resume", 1011);
    let batch = per_batch(&game);
    let machine = assembler_at(&mut game, 40, 40, true);
    let feeder = feeder_at(&mut game, 41, 40, 0);

    for _ in 0..5 {
        game.tick();
    }
    assert_eq!(status_of(&game, machine), Some(MachineStatus::Starved));

    game.world
        .get_mut::<Stock>(feeder)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), batch * 4);
    for _ in 0..10 {
        game.tick();
    }

    assert_eq!(status_of(&game, machine), Some(MachineStatus::Running));
    assert_eq!(log_hits(&game, "resumes"), 1);
}

/// A program can actually be posted to an assembler through the same
/// cronjob assignment an extractor uses — there is no second concept, and
/// the menu and the assignment agree about what is assignable.
#[test]
fn a_program_can_be_posted_to_an_assembler() {
    let mut game = game_with_assembler("chain_assign", 1012);
    let machine = assembler_at(&mut game, 40, 40, false);
    let worker = spawn_tamed(&mut game, 10, 3);

    game.assign_cronjob(worker, machine)
        .expect("an assembler takes a program like any other machine");

    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(machine)
    );
}

/// The shipped chain, walked end to end from real assets rather than from a
/// fixture: two extractors feed two refiners, which feed one assembler, and
/// the terminal item comes out. This is the test that would have caught a
/// content slice that loaded fine and could never actually run.
///
/// The layout is the design's whole point — the Assembly Bay needs *both*
/// feeders orthogonally touching it, so it wants a corner:
///
/// ```text
///   $ B Y      $ mining_node   B refinery    Y assembly_bay
///       W      W winding_node  + power_conduit
///       +
/// ```
#[test]
fn the_shipped_three_stage_chain_produces_its_terminal_item() {
    let mut game = Game::new(1100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let bay = staffed(&mut game, "assembly_bay", 42, 40);
    let refinery = staffed(&mut game, "refinery", 41, 40);
    let winding = staffed(&mut game, "winding_node", 42, 41);
    // The two extractors are pre-stocked rather than worked, so this test
    // measures the chain rather than `mining_success_chance`'s roll.
    let mine = stocked(&mut game, "mining_node", 40, 40, ids::CORE_FRAGMENT, 200);
    let conduit = stocked(&mut game, "power_conduit", 42, 42, ids::POWER_CELL, 200);

    for _ in 0..200 {
        game.tick();
    }

    assert!(
        output_of(&game, refinery, ids::BYTECODE_BLOCK) > 0
            || input_of(&game, bay, ids::BYTECODE_BLOCK) > 0,
        "stage two ran: the refinery turned fragments into blocks"
    );
    assert!(
        output_of(&game, winding, ids::CHARGE_COIL) > 0
            || input_of(&game, bay, ids::CHARGE_COIL) > 0,
        "and the winding node turned cells into coils"
    );
    assert!(
        output_of(&game, bay, ids::PATCH_ROUTINE) > 0,
        "and the assembly bay built the terminal item out of both"
    );
    assert!(
        game.world
            .get::<Stock>(mine)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0)
            < 200,
        "the chain really drew from the extractors rather than conjuring input"
    );
    let _ = conduit;
}

/// A machine short one of its two ingredients is starved, not half-running.
/// This is the failure the player will actually hit — a bay built against
/// one feeder because the other did not fit.
#[test]
fn an_assembly_bay_with_only_one_feeder_adjacent_stays_starved() {
    let mut game = Game::new(1101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bay = staffed(&mut game, "assembly_bay", 42, 40);
    stocked(&mut game, "refinery", 41, 40, ids::BYTECODE_BLOCK, 50);

    for _ in 0..100 {
        game.tick();
    }

    assert!(input_of(&game, bay, ids::BYTECODE_BLOCK) > 0, "it is fed");
    assert_eq!(
        output_of(&game, bay, ids::PATCH_ROUTINE),
        0,
        "but half a recipe builds nothing"
    );
    assert_eq!(status_of(&game, bay), Some(MachineStatus::Starved));
}

/// The first stage's product is what buys the last stage. Without this the
/// two-machine line a starting roster can afford has no payoff of its own,
/// and the spec's "the intermediate needs standalone value" goes unmet — the
/// Market's flat sell rate cannot express it.
#[test]
fn the_assembly_bay_is_built_out_of_what_the_refinery_makes() {
    let game = Game::new(1102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let cost = &game
        .world
        .resource::<crate::structures::StructureDb>()
        .get("assembly_bay")
        .expect("assembly_bay ships")
        .build_cost;
    assert!(
        cost.iter()
            .any(|(i, n)| i.as_str() == ids::BYTECODE_BLOCK && *n > 0),
        "the Assembly Bay costs Bytecode Blocks: {cost:?}"
    );
}

/// A staffed structure of `kind` at an absolute tile.
fn staffed(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    let machine = deployed(game, kind, x, y);
    let worker = spawn_tamed(game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 1,
    });
    machine
}

/// A structure of `kind` at an absolute tile with `qty` of `item` already in
/// its output buffer, standing in for an extractor that has been running.
fn stocked(game: &mut Game, kind: &str, x: i32, y: i32, item: &str, qty: u32) -> Entity {
    let e = deployed(game, kind, x, y);
    game.world
        .get_mut::<Stock>(e)
        .unwrap()
        .output
        .insert(ItemId::from(item), qty);
    e
}

/// Spawns `kind` with the same components `place_structure` gives it,
/// bypassing the Home, cost and distance rules — these tests are about what
/// a standing chain does, not about the build rules.
fn deployed(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    let capacity = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get(kind)
        .unwrap_or_else(|| panic!("{kind} ships with the game"))
        .capacity;
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            Stock::new(capacity),
            MachineStatus::default(),
        ))
        .id()
}
