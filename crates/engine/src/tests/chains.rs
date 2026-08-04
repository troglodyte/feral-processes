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

/// The armour chain end to end. Same spine as the Patch Routine chain above
/// but ending in equipment, which is what makes the base the way gear happens
/// rather than a source of consumables beside it.
///
/// ```text
///   $ B %      $ mining_node   B refinery     % armory
///       W      W winding_node  + power_conduit
///       +
/// ```
#[test]
fn the_armoury_chain_produces_a_hardened_shell() {
    let mut game = Game::new(1103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let armory = staffed(&mut game, "armory", 42, 40);
    let refinery = staffed(&mut game, "refinery", 41, 40);
    let winding = staffed(&mut game, "winding_node", 42, 41);
    // Pre-stocked rather than worked, so this measures the chain and not
    // `mining_success_chance`'s roll.
    stocked(&mut game, "mining_node", 40, 40, ids::CORE_FRAGMENT, 200);
    stocked(&mut game, "power_conduit", 42, 42, ids::POWER_CELL, 200);

    for _ in 0..300 {
        game.tick();
    }

    assert!(
        output_of(&game, refinery, ids::BYTECODE_BLOCK) > 0
            || input_of(&game, armory, ids::BYTECODE_BLOCK) > 0,
        "the refinery turned fragments into blocks"
    );
    assert!(
        output_of(&game, winding, ids::CHARGE_COIL) > 0
            || input_of(&game, armory, ids::CHARGE_COIL) > 0,
        "and the winding node turned cells into coils"
    );
    assert!(
        output_of(&game, armory, "hardened_shell") > 0,
        "and the armoury built wearable gear out of both"
    );
}

/// The module chain, which is the one that proves the two gear classes draw
/// on *different* taps: this one runs off the Log Scraper's Raw Trace through
/// the Transcriber, and never touches a Mining Node. Nothing walked that path
/// before — the Disk Press chain shares it but has no end-to-end test.
///
/// ```text
///   T S *      T log_scraper   S transcriber  * fabricator
///       W      W winding_node  + power_conduit
///       +
/// ```
#[test]
fn the_fabricator_chain_produces_a_trace_sniffer_off_the_trace_tap() {
    let mut game = Game::new(1104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let fabricator = staffed(&mut game, "fabricator", 42, 40);
    let transcriber = staffed(&mut game, "transcriber", 41, 40);
    staffed(&mut game, "winding_node", 42, 41);
    stocked(&mut game, "log_scraper", 40, 40, "raw_trace", 200);
    stocked(&mut game, "power_conduit", 42, 42, ids::POWER_CELL, 200);

    for _ in 0..400 {
        game.tick();
    }

    assert!(
        output_of(&game, transcriber, "logic_wafer") > 0
            || input_of(&game, fabricator, "logic_wafer") > 0,
        "the transcriber turned trace into wafers"
    );
    assert!(
        output_of(&game, fabricator, "trace_sniffer") > 0,
        "and the fabricator built a module out of wafers and coils"
    );
}

/// Armour, modules and Patch Routines all want Charge Coils, so the Winding
/// Node is the first shipped feeder three machines can pull on at once. With
/// one coil to give, the `(x, y)` sort decides who eats.
///
/// The three are spawned in the reverse of their positions on purpose: in
/// position order this would pass on bevy's iteration order alone, which is
/// the exact bug the sort exists to prevent.
///
/// ```text
///     %        % armory (42, 40)
///   Y W *      Y assembly_bay (41, 41)  W winding_node (42, 41)
///              * fabricator (43, 41)
/// ```
#[test]
fn three_machines_competing_for_one_coil_resolve_in_position_order() {
    let mut game = Game::new(1105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let fabricator = staffed(&mut game, "fabricator", 43, 41);
    let armory = staffed(&mut game, "armory", 42, 40);
    let bay = staffed(&mut game, "assembly_bay", 41, 41);
    stocked(&mut game, "winding_node", 42, 41, ids::CHARGE_COIL, 1);

    game.tick();

    assert_eq!(
        input_of(&game, bay, ids::CHARGE_COIL),
        1,
        "the machine at the lower x is visited first and takes the only coil"
    );
    assert_eq!(
        input_of(&game, armory, ids::CHARGE_COIL),
        0,
        "the armoury is behind it in sort order"
    );
    assert_eq!(
        input_of(&game, fabricator, ids::CHARGE_COIL),
        0,
        "and the fabricator is behind that"
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

/// What the map draws its wiring links from. A link means "this neighbour
/// makes something my recipe wants" — the property that stays true while a
/// healthy chain drains its feeder every other tick.
#[test]
fn a_machine_reports_an_edge_toward_a_neighbour_that_makes_what_it_needs() {
    let mut game = Game::new(1200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let refinery = deployed(&mut game, "refinery", 41, 40);
    let mine = deployed(&mut game, "mining_node", 40, 40);

    assert_eq!(
        edges_of(&mut game, refinery),
        vec![(-1, 0)],
        "the Mining Node to the west makes the Core Fragments a Refinery wants"
    );
    // Symmetric, though the feeding relation is not: a Mining Node has no
    // recipe and names nobody, but the map has to drop *both* halves of the
    // wall they share or the one line left reads as a rendering fault.
    assert_eq!(
        edges_of(&mut game, mine),
        vec![(1, 0)],
        "the feeder reports the join back, so both walls come down together"
    );
}

#[test]
fn a_diagonal_neighbour_is_not_wired() {
    let mut game = Game::new(1201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let refinery = deployed(&mut game, "refinery", 41, 40);
    deployed(&mut game, "mining_node", 40, 39);

    assert!(edges_of(&mut game, refinery).is_empty());
}

/// Touching is not the same as feeding. A Research Node beside a Refinery
/// makes Research Data, which no recipe of the Refinery's wants.
#[test]
fn a_neighbour_making_something_the_recipe_does_not_want_is_not_wired() {
    let mut game = Game::new(1202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let refinery = deployed(&mut game, "refinery", 41, 40);
    deployed(&mut game, "research_node", 40, 40);

    assert!(edges_of(&mut game, refinery).is_empty());
}

/// The failure a player actually hits: a Bay built against one feeder
/// because the other did not fit. It shows one link where it needs two, so
/// the mistake is visible on the map without opening a menu.
#[test]
fn a_mislaid_assembly_bay_reports_one_edge_rather_than_two() {
    let mut game = Game::new(1203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bay = deployed(&mut game, "assembly_bay", 42, 40);
    deployed(&mut game, "refinery", 41, 40);
    // One tile too far south — touching nothing.
    deployed(&mut game, "winding_node", 42, 42);

    assert_eq!(edges_of(&mut game, bay), vec![(-1, 0)]);

    // Move it into place and the second join appears.
    let mut game = Game::new(1203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bay = deployed(&mut game, "assembly_bay", 42, 40);
    deployed(&mut game, "refinery", 41, 40);
    deployed(&mut game, "winding_node", 42, 41);

    assert_eq!(edges_of(&mut game, bay), vec![(-1, 0), (0, 1)]);
}

/// A Home assembles nothing and runs no job, so it has neither half of the
/// map's machine vocabulary — no links and no status outline.
#[test]
fn a_home_reports_no_edges_and_no_machine_status() {
    let mut game = Game::new(1204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let home = find_home(&mut game).unwrap();

    assert!(edges_of(&mut game, home).is_empty());
    assert_eq!(game.world.get::<MachineStatus>(home).copied(), None);
}

/// The wiring the map draws and the pull the system performs read the same
/// recipe and walk the same four tiles, so a link can never point somewhere
/// the pull phase would refuse to take from.
#[test]
fn a_joined_edge_is_an_edge_the_pull_phase_actually_uses() {
    let mut game = Game::new(1205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let refinery = staffed(&mut game, "refinery", 41, 40);
    stocked(&mut game, "mining_node", 40, 40, ids::CORE_FRAGMENT, 100);

    assert_eq!(edges_of(&mut game, refinery), vec![(-1, 0)]);
    game.tick();

    assert!(
        input_of(&game, refinery, ids::CORE_FRAGMENT) > 0,
        "the link points at a feeder the pull phase really drew from"
    );
}

fn edges_of(game: &mut Game, structure: Entity) -> Vec<(i32, i32)> {
    let mut edges = game
        .linked_edges_by_structure()
        .remove(&structure)
        .unwrap_or_default();
    edges.sort();
    edges
}

/// Through the real `place_structure`, not a hand-built fixture. Every test
/// above stands its machines by spawning the components directly, so none of
/// them could catch a deploy path that forgets one — and the deploy path did
/// forget `MachineStatus` for assemblers, which declare `assembles` and no
/// `work` block. A machine with no status silently reports nothing: no stall
/// line in the log, no state on the roster, no outline on the map.
#[test]
fn a_deployed_assembler_gets_a_machine_status_like_any_other_machine() {
    let mut game = Game::new(1210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    game.place_structure("refinery", 1, 0)
        .expect("a Refinery is buildable from the start");

    let refinery = find_structure_by_kind(&mut game, "refinery").unwrap();
    // `Idle`, not `Running`: `place_structure` ticks, and a machine nobody
    // is posted to correctly reports having no program. The point of the
    // assertion is that it reports *at all* — without a `MachineStatus` it
    // would read as `None`, which every consumer treats as "not a machine".
    assert_eq!(
        game.world.get::<MachineStatus>(refinery).copied(),
        Some(MachineStatus::Idle),
        "an assembler is a machine and has a state like an extractor does"
    );
    assert!(
        game.world.get::<Stock>(refinery).is_some(),
        "and it has the buffers it pulls into"
    );
}

/// A modded structure that assembles a self-referential item, plus the item
/// itself. A recipe that costs itself is what a mod typo looks like, and the
/// chain walk has to survive it rather than recursing until the stack goes.
const LOOP_STRUCTURE: &str = r#"(
    id: "loop_maker",
    name: "Loop Maker",
    description: "A modded assembler with a cyclic recipe, for tests.",
    glyph: 'L',
    color: Red,
    build_cost: [],
    work: None,
    capacity: 20,
    assembles: Some((item: "loop_item", ticks_per_unit: 3)),
)"#;

const LOOP_ITEM: &str = r#"(
    id: "loop_item",
    name: "Loop Item",
    description: "An item whose recipe names itself.",
    craftable: Some((cost: [("loop_item", 1)], requires_structure: Some("loop_maker"))),
)"#;

/// The one chain in the shipped game with real depth, and the reason the
/// screen exists: what to build, and in what order, to automate a Patch
/// Routine.
#[test]
fn the_patch_routine_chain_runs_from_fragments_up_through_the_assembly_bay() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let chain = chains
        .iter()
        .find(|c| c.product == "Patch Routine")
        .expect("the Assembly Bay assembles one");

    let shape: Vec<(Vec<(&str, u32)>, Option<&str>, &str)> = chain
        .steps
        .iter()
        .map(|s| {
            (
                s.inputs
                    .iter()
                    .map(|(n, q)| (n.as_str(), *q))
                    .collect::<Vec<_>>(),
                s.maker.as_deref(),
                s.output.as_str(),
            )
        })
        .collect();

    assert_eq!(
        shape,
        vec![
            (
                vec![("Core Fragment", 4)],
                Some("Refinery"),
                "Bytecode Block"
            ),
            (vec![("Core Fragment", 2)], None, "Power Cell"),
            (vec![("Power Cell", 3)], Some("Winding Node"), "Charge Coil"),
            (
                vec![("Bytecode Block", 1), ("Charge Coil", 1)],
                Some("Assembly Bay"),
                "Patch Routine"
            ),
        ],
        "every dependency is listed before the step that consumes it"
    );
}

#[test]
fn an_extractors_chain_is_a_single_step_that_consumes_nothing() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let chain = chains
        .iter()
        .find(|c| c.product == "Core Fragment")
        .expect("the Mining Node produces them");

    assert_eq!(chain.steps.len(), 1, "a tap has no upstream");
    assert!(
        chain.steps[0].inputs.is_empty(),
        "an extractor draws from nothing, not from a recipe"
    );
    assert_eq!(chain.steps[0].maker.as_deref(), Some("Mining Node"));
}

/// Power Cell is the shipped item that is craftable with no
/// `requires_structure`, so it is what proves the bench case is reachable
/// rather than every step naming a machine.
#[test]
fn a_step_the_player_compiles_by_hand_names_no_maker() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let coil = chains
        .iter()
        .find(|c| c.product == "Charge Coil")
        .expect("the Winding Node assembles them");
    let power_cell = coil
        .steps
        .iter()
        .find(|s| s.output == "Power Cell")
        .expect("the coil's recipe is priced in them");

    assert_eq!(
        power_cell.maker, None,
        "power_cell.ron sets no requires_structure"
    );
}

/// Shortest chains first, so the screen opens on the taps and reads down
/// into the things that need a base to make.
#[test]
fn chains_are_listed_shallowest_first() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let depths: Vec<usize> = chains.iter().map(|c| c.steps.len()).collect();
    let mut sorted = depths.clone();
    sorted.sort();
    assert_eq!(depths, sorted);
    // Three products tie at the deepest tier now that gear is assembled, so
    // naming one of them would pin whichever way the tie happens to break.
    // What the screen actually promises is that everything needing a full base
    // reads last, and the set is what is worth holding.
    let deepest = depths
        .last()
        .copied()
        .expect("the shipped assets declare chains");
    let bottom: Vec<&str> = chains
        .iter()
        .filter(|c| c.steps.len() == deepest)
        .map(|c| c.product.as_str())
        .collect();
    assert_eq!(
        bottom,
        ["Hardened Shell", "Patch Routine", "Trace Sniffer"],
        "the deepest things in the game sit at the bottom"
    );
}

/// A mod whose recipe names itself must not take the process down with it —
/// the same contract as a malformed `.ron` being skipped rather than
/// panicking at startup.
#[test]
fn a_self_referential_recipe_does_not_recurse_forever() {
    let dir = assets_dir_with_extra_structure("recipe_cycle", "loop_maker.ron", LOOP_STRUCTURE);
    std::fs::write(dir.join("items").join("loop_item.ron"), LOOP_ITEM).unwrap();
    let game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();

    let chains = game.recipe_chains();
    let chain = chains
        .iter()
        .find(|c| c.product == "Loop Item")
        .expect("the modded assembler makes one");

    assert_eq!(
        chain.steps.len(),
        1,
        "the cycle is cut at the repeat, leaving the machine's own step"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Compiler is the shipped instance of that two-machine line: it does not
/// print catalysts from nothing any more, it compiles them out of Core
/// Fragments pulled from whatever is touching it — a Mining Node, in
/// practice, which is where fragments come from.
///
/// Written against the real assets rather than `test_assembler` because what
/// is under test is `compiler.ron` declaring `assembles` at all. A modded
/// fixture would keep passing with the shipped file reverted to `work`.
///
/// The feeder is stocked rather than mined so the assertion is about the
/// chain and not about `mining_success_chance`'s roll.
#[test]
fn the_shipped_compiler_compiles_catalysts_out_of_an_adjacent_mining_node() {
    let mut game = Game::new(1012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let batch = game
        .world
        .resource::<ItemDb>()
        .get(ids::ICE_BREAKER)
        .and_then(|d| d.craftable.as_ref())
        .expect("ice_breaker ships with a recipe")
        .cost
        .iter()
        .find(|(i, _)| i.as_str() == ids::CORE_FRAGMENT)
        .map(|(_, n)| *n)
        .expect("its recipe is priced in core fragments");

    let compiler = game
        .world
        .spawn((
            Structure {
                kind: "compiler".to_string(),
            },
            Position { x: 40, y: 40 },
            Stock::new(20),
            MachineStatus::default(),
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: compiler,
        progress: 0,
        required: 1,
    });
    let stocked = batch * 50;
    let feeder = feeder_at(&mut game, 41, 40, stocked);

    for _ in 0..20 {
        game.tick();
    }

    assert!(
        output_of(&game, compiler, ids::ICE_BREAKER) > 0,
        "a staffed Compiler beside a stocked Mining Node compiles catalysts"
    );
    assert!(
        output_of(&game, feeder, ids::CORE_FRAGMENT) < stocked,
        "and pays for them out of the node's fragments rather than out of nothing"
    );
}
