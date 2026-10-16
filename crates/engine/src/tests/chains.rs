//! Adjacency-fed production chains: what a machine pulls, and from where.

use super::support::*;
use crate::*;

/// A modded assembler that builds Power Cells. Its recipe is *not* written
/// here — the machine runs `power_cell.ron`'s own `craftable.cost`, which is
/// the whole point of a machine listing an item rather than a recipe.
const TEST_ASSEMBLER: &str = r#"(
    id: "test_assembler",
    name: "Test Assembler",
    description: "A modded assembler, for tests.",
    glyph: 'A',
    color: Cyan,
    build_cost: [],
    work: None,
    capacity: 20,
    machine: Some((role: Assemble, recipes: [(produces: "power_cell", ticks_per_unit: 3)])),
)"#;

/// A modded assembler on a *two*-ingredient recipe. No shipped machine runs
/// one any more — every shipped `assembles` recipe is a single ingredient, so
/// that a line is a straight line (see
/// `every_shipped_assembler_recipe_is_a_single_ingredient`). The engine still
/// supports multi-input machines and mods may ship them, so the starve path
/// is exercised here rather than left uncovered.
///
/// The recipe is `entropy_damper`'s own — 2 Logic Wafers and 3 Charge Coils —
/// for the same reason `TEST_ASSEMBLER` uses `power_cell`'s: an assembler
/// runs the item's `craftable.cost`, never a recipe restated at the machine.
const TWO_INPUT_ASSEMBLER: &str = r#"(
    id: "two_input_assembler",
    name: "Two Input Assembler",
    description: "A modded two-ingredient assembler, for tests.",
    glyph: 'D',
    color: Magenta,
    build_cost: [],
    work: None,
    capacity: 20,
    machine: Some((role: Assemble, recipes: [(produces: "entropy_damper", ticks_per_unit: 3)])),
)"#;

/// A game whose asset set includes `test_assembler`. The caller drops the
/// scratch directory; `Game` has already read everything it needs.
fn game_with_assembler(tag: &str, seed: u32) -> Game {
    let dir = assets_dir_with_extra_structure(tag, "test_assembler.ron", TEST_ASSEMBLER);
    let game = Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    game
}

/// As above, for the two-ingredient machine.
fn game_with_two_input_assembler(tag: &str, seed: u32) -> Game {
    let dir = assets_dir_with_extra_structure(tag, "two_input_assembler.ron", TWO_INPUT_ASSEMBLER);
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
                kind: "extractor".to_string(),
            },
            crate::components::MachineRecipe(Some(ItemId::from(ids::CORE_FRAGMENT))),
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
            crate::components::MachineRecipe(Some(ItemId::from(ids::POWER_CELL))),
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

/// Posts a program of `species` to `machine` through the real assignment
/// path, so its `Task::required` is baked out of the machine's rate and the
/// species' `base_speed` by `Game::work_ticks_for` rather than hand-written.
///
/// `assign_cronjob` starts a program from the player's tile, so the player is
/// stood at the post first — otherwise the two machines under comparison
/// would differ by a walk as well as by a rate.
fn post_species(game: &mut Game, machine: Entity, species: &str) -> Entity {
    let worker = spawn_tamed(game, 10, 3);
    game.world.get_mut::<Creature>(worker).unwrap().species = species.to_string();
    stand_player_at_post(game, machine);
    game.assign_cronjob(worker, machine)
        .expect("an assembler takes a program like any other machine");
    worker
}

/// The phase's whole claim, at an assembler: two identical machines,
/// identically fed, differing only in who is posted. It is invisible until
/// `assembler_system` stops reading the def's `ticks_per_unit` and starts
/// reading the rate baked into `Task::required` at assignment.
///
/// Thirty ticks, deliberately: `test_assembler` is rated 3 ticks a unit, so
/// the Sprite (`base_speed: 14`) runs it at 2 and the Construct
/// (`base_speed: 6`) at 4, and neither machine reaches the 20 its `Stock`
/// holds. A longer run would clog the quick one and start measuring capacity
/// instead of pace.
#[test]
fn a_quicker_program_runs_the_same_assembler_harder() {
    let mut game = game_with_assembler("assembler_speed", 1000);
    // Deploying and posting are base actions; the party stands in the base.
    stand_in_base(&mut game);

    let quick = assembler_at(&mut game, 40, 40, false);
    feeder_at(&mut game, 41, 40, 200);
    post_species(&mut game, quick, "sprite");

    let slow = assembler_at(&mut game, 40, 50, false);
    feeder_at(&mut game, 41, 50, 200);
    post_species(&mut game, slow, "construct");

    for _ in 0..30 {
        game.tick();
    }

    let fast_out = output_of(&game, quick, ids::POWER_CELL);
    let slow_out = output_of(&game, slow, ids::POWER_CELL);
    assert!(
        fast_out > slow_out,
        "the sprite-run machine should be ahead — got {fast_out} against {slow_out}"
    );
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

/// Two modded programs alike in every way the base economy has ever read —
/// same stats, same lack of abilities — and differing only in Analysis.
/// Authored here rather than reusing shipped species so the assertion below
/// survives any later retune of the roster.
const SHARP_PROGRAM: &str = r#"(
    id: "sharpmon",
    name: "Sharpmon",
    glyph: 's',
    color: Cyan,
    base_hp: 40,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    attributes: {"analysis": 16},
    moves: [(name: "Poke", power: 3)],
    work_resource: None,
)"#;

const DULL_PROGRAM: &str = r#"(
    id: "dullmon",
    name: "Dullmon",
    glyph: 'd',
    color: Brown,
    base_hp: 40,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    attributes: {"analysis": 4},
    moves: [(name: "Poke", power: 3)],
    work_resource: None,
)"#;

/// Two more of the same, differing only in the affinity axis they raise —
/// which is the whole of what names a class. Same Analysis and the same
/// (defaulted) `base_speed`, so a difference in what lands in the buffer is
/// the class and can be nothing else.
const DRAIN_PROGRAM: &str = r#"(
    id: "leechmon",
    name: "Leechmon",
    glyph: 'l',
    color: Magenta,
    base_hp: 40,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    work_resource: None,
    affinities: (buff: 0.85, drain: 1.3),
)"#;

const BURST_PROGRAM: &str = r#"(
    id: "strikermon",
    name: "Strikermon",
    glyph: 'k',
    color: Red,
    base_hp: 40,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    work_resource: None,
    affinities: (heal: 0.85, damage: 1.3),
)"#;

/// The seeds both mining comparisons sweep. Five is enough that one tied
/// run cannot decide the answer, and few enough that the pair of sweeps
/// stays under a second.
const MINING_SAMPLE_SEEDS: [u32; 5] = [4181, 991, 20_313, 77, 60_509];

/// Posts one program of `species` to a fresh Mining Node, runs it for
/// `ticks`, and returns what landed in the node's buffer.
///
/// The horizon is a parameter and matters, because of
/// `DEFAULT_OUTPUT_CAPACITY`: the node's def sets `ticks_per_unit: 10` and
/// its buffer holds 20, so a run long enough for the better worker to clog
/// reports the same capped number for both and quietly stops measuring
/// anything. 250 ticks is the ceiling for a comparison of *reliability*
/// (one unit a cycle either way); a comparison of *yield* has to stop well
/// short of it, since the Leech is taking two.
///
/// **The seed is a parameter because one run is not a measurement.** Bevy's
/// query iteration order is not stable between processes, so which draw a
/// mining roll takes is not fixed by the seed alone, and a single pair of
/// runs can land on a tie — this comparison failed once at 8 against 8 on a
/// strict `>`. Both callers sweep `MINING_SAMPLE_SEEDS` and compare totals,
/// which is what makes the assertion about the class rather than about a
/// lucky stream position.
fn units_mined_by(tag: &str, species: &str, ticks: u32, seed: u32) -> u32 {
    units_mined_with(tag, species, None, ticks, seed)
}

/// `units_mined_by`, with the worker given its own Analysis attribute as a
/// seated program has one: it must override the species' catalogue figure.
fn units_mined_with(
    tag: &str,
    species: &str,
    own_analysis: Option<i32>,
    ticks: u32,
    seed: u32,
) -> u32 {
    let dir = modded_assets_dir(
        tag,
        &[],
        &[],
        &[
            ("sharpmon.ron", SHARP_PROGRAM),
            ("dullmon.ron", DULL_PROGRAM),
            ("leechmon.ron", DRAIN_PROGRAM),
            ("strikermon.ron", BURST_PROGRAM),
        ],
        &[],
        &[],
    );
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap();
    // Deploying and posting are base actions; the party stands in the base.
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    let worker = spawn_tamed(&mut game, 40, 4);
    // A fixture that fails to parse is *skipped* with a warning, and the
    // lookups in `task_progress_system` then quietly fall back to the
    // baseline — so a typo in one of the files above reads as "the feature
    // does nothing" rather than as a broken test. It cost a debugging round
    // once; this is that round, spent.
    assert!(
        game.species_defs().iter().any(|d| d.id == species),
        "{species} did not load — check the fixture parses"
    );
    game.world.get_mut::<Creature>(worker).unwrap().species = species.to_string();
    // The fixture was minted as the generic species; swapping the species
    // leaves those attributes behind, so strip them and let the catalogue
    // fallback speak unless the caller supplies the program's own.
    game.world
        .entity_mut(worker)
        .remove::<crate::components::Attributes>();
    if let Some(value) = own_analysis {
        let mut attributes = crate::components::Attributes::default();
        attributes.set(&crate::attributes::AttributeId::from("analysis"), value);
        game.world.entity_mut(worker).insert(attributes);
    }
    stand_player_at_post(&mut game, node);
    game.assign_cronjob(worker, node)
        .expect("a Mining Node takes a posted program");
    park_at_post(&mut game, worker, node);
    for _ in 0..ticks {
        game.tick();
    }
    let mined = node_output(&game, node, ids::CORE_FRAGMENT);
    let _ = std::fs::remove_dir_all(&dir);
    mined
}

/// The formula tests prove the arithmetic; this proves it reaches the base.
///
/// Before `base_int` the only creature-side input to the entire base economy
/// was one `Stats::def` read mitigating sweep damage — every program was an
/// interchangeable pair of hands, and swapping who was posted to a node
/// changed nothing at all. This is the assertion that says that is no longer
/// true, and it is deliberately about *output* rather than about a chance,
/// because a roll nobody's buffer ever sees is not a feature.
/// The Leech base job, measured where the player meets it: in the buffer.
///
/// The formula test in `systems.rs` proves a cycle pays a unit more; this
/// proves the class reaches `task_progress_system`'s call at all, which is
/// the wiring a unit test cannot see. The two programs differ in nothing
/// else the economy reads, so the gap is the drain affinity and can be
/// nothing else.
#[test]
fn a_leech_fills_a_node_buffer_faster_than_a_striker() {
    let mut leech = 0;
    let mut striker = 0;
    for seed in MINING_SAMPLE_SEEDS {
        leech += units_mined_by("class_leech", "leechmon", 100, seed);
        striker += units_mined_by("class_striker", "strikermon", 100, seed);
    }
    assert!(
        leech > striker,
        "the drain class has to draw more out of the same tap \
         (leech took {leech}, striker took {striker} across \
         {} seeds)",
        MINING_SAMPLE_SEEDS.len()
    );
}

#[test]
fn a_sharper_program_mines_more_from_the_same_node() {
    // The tick budget is bounded on both sides and neither bound is slack:
    // too few and the two programs have not diverged, too many and the
    // faster one fills the buffer and the comparison flattens. It was 250
    // until per-chunk population shifted the RNG stream and the sharp run
    // landed exactly on the cap.
    let mut sharp = 0;
    let mut dull = 0;
    for seed in MINING_SAMPLE_SEEDS {
        let run = units_mined_by("int_sharp", "sharpmon", 180, seed);
        // The clog bound is per run, not on the total: a sweep that sums
        // past the cap is fine, a single run that reaches it is the case
        // where the comparison has stopped measuring anything.
        assert!(
            run < 20,
            "every run must stay under DEFAULT_OUTPUT_CAPACITY or the node \
             clogs and the comparison stops measuring anything (seed {seed} \
             mined {run})"
        );
        sharp += run;
        dull += units_mined_by("int_dull", "dullmon", 180, seed);
    }
    assert!(
        sharp > dull,
        "who you post to a node has to change what it produces \
         (sharp mined {sharp}, dull mined {dull} across {} seeds)",
        MINING_SAMPLE_SEEDS.len()
    );
}

/// A posted program's roll follows its *own* Analysis, not its species'
/// catalogue figure: the same sharp species, given an absurdly low and an
/// absurdly high attribute, mines nothing and mines something. The
/// extremes saturate the clamp, so the low end is exactly zero whatever the
/// stream and the high end is positive across the sample; the exact count
/// is not asserted because it is bounded by the node's own cycle length.
#[test]
fn a_posted_programs_own_analysis_overrides_its_species() {
    let mut dull = 0;
    let mut keen = 0;
    for seed in MINING_SAMPLE_SEEDS {
        dull += units_mined_with("own_dull", "sharpmon", Some(-1000), 100, seed);
        keen += units_mined_with("own_keen", "sharpmon", Some(1000), 100, seed);
    }
    assert_eq!(dull, 0, "an Analysis of -1000 can never land a cycle");
    assert!(
        keen > 0,
        "an Analysis of 1000 saturates the roll, so it mines"
    );
}

/// A program can actually be posted to an assembler through the same
/// cronjob assignment an extractor uses — there is no second concept, and
/// the menu and the assignment agree about what is assignable.
#[test]
fn a_program_can_be_posted_to_an_assembler() {
    let mut game = game_with_assembler("chain_assign", 1012);
    // Deploying and posting are base actions; the party stands in the base.
    stand_in_base(&mut game);
    let machine = assembler_at(&mut game, 40, 40, false);
    let worker = spawn_tamed(&mut game, 10, 3);
    // A program is posted from wherever the player stands, and the machine
    // is 37 tiles from the spawn point — this test is about whether an
    // assembler is assignable at all, not about the walk to it.
    stand_player_at_post(&mut game, machine);

    game.assign_cronjob(worker, machine)
        .expect("an assembler takes a program like any other machine");

    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(machine)
    );
}

/// The shipped chain, walked end to end from real assets rather than from a
/// fixture: an extractor feeds a refiner, which feeds an assembler, and the
/// terminal item comes out. This is the test that would have caught a content
/// slice that loaded fine and could never actually run.
///
/// The layout is the design's whole point — a line is a *line*, so every
/// machine wants exactly one feeder touching it and a straight run of three
/// tiles will do:
///
/// ```text
///   + W Y      + cell_extractor  W coil_factory  Y patch_printer
/// ```
#[test]
fn the_shipped_three_stage_chain_produces_its_terminal_item() {
    let mut game = Game::new(1100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let bay = staffed(&mut game, "printer", Some("patch_routine"), 42, 40);
    let winding = staffed(&mut game, "factory", Some("charge_coil"), 41, 40);
    // The extractor is pre-stocked rather than worked, so this test measures
    // the chain rather than the production roll.
    let conduit = stocked(&mut game, "extractor", 40, 40, ids::POWER_CELL, 400);

    for _ in 0..300 {
        game.tick();
    }

    assert!(
        output_of(&game, winding, ids::CHARGE_COIL) > 0
            || input_of(&game, bay, ids::CHARGE_COIL) > 0,
        "stage two ran: the winding node turned cells into coils"
    );
    assert!(
        output_of(&game, bay, ids::PATCH_ROUTINE) > 0,
        "and the assembly bay built the terminal item out of them"
    );
    assert!(
        game.world
            .get::<Stock>(conduit)
            .unwrap()
            .output
            .get(&ItemId::from(ids::POWER_CELL))
            .copied()
            .unwrap_or(0)
            < 400,
        "the chain really drew from the extractor rather than conjuring input"
    );
}

/// A machine short one of its two ingredients is starved, not half-running.
///
/// No shipped machine can hit this any more — flattening the chains left
/// every shipped recipe on one ingredient. The engine still supports
/// multi-input assemblers and a mod may ship one, so the path is walked with
/// a modded machine rather than dropped along with the content that used it.
#[test]
fn a_machine_short_one_of_its_two_ingredients_stays_starved() {
    let mut game = game_with_two_input_assembler("chain_starve", 1101);
    let machine = staffed(
        &mut game,
        "two_input_assembler",
        Some("entropy_damper"),
        42,
        40,
    );
    stocked(&mut game, "factory", 41, 40, "logic_wafer", 50);

    for _ in 0..100 {
        game.tick();
    }

    assert!(input_of(&game, machine, "logic_wafer") > 0, "it is fed");
    assert_eq!(
        output_of(&game, machine, "entropy_damper"),
        0,
        "but half a recipe builds nothing"
    );
    assert_eq!(status_of(&game, machine), Some(MachineStatus::Starved));
}

/// The armour chain end to end. Same spine as the Patch Routine chain above
/// but ending in equipment, which is what makes the base the way gear happens
/// rather than a source of consumables beside it.
///
/// ```text
///   $ B %      $ extractor  B block_factory  % shell_assembler
/// ```
#[test]
fn the_armoury_chain_produces_a_hardened_shell() {
    let mut game = Game::new(1103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let shell_assembler = staffed(&mut game, "assembler", Some("hardened_shell"), 42, 40);
    let block_factory = staffed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    // Pre-stocked rather than worked, so this measures the chain and not
    // `mining_success_chance`'s roll.
    stocked(&mut game, "extractor", 40, 40, ids::CORE_FRAGMENT, 400);

    for _ in 0..400 {
        game.tick();
    }

    assert!(
        output_of(&game, block_factory, ids::BYTECODE_BLOCK) > 0
            || input_of(&game, shell_assembler, ids::BYTECODE_BLOCK) > 0,
        "the block_factory turned fragments into blocks"
    );
    assert!(
        output_of(&game, shell_assembler, "hardened_shell") > 0,
        "and the armoury built wearable gear out of them"
    );
}

/// The refactor chain end to end. The census tests hold its *shape* — one
/// ingredient per recipe, the bench built out of its own feeder's product,
/// nobody assembling anyone else's — and none of them runs a tick, so none
/// of them would notice a chain that is shaped right and produces nothing.
///
/// ```text
///   $ A X      $ extractor  A core_factory  X kernel_assembler
/// ```
#[test]
fn the_refactor_chain_produces_a_recompile_kernel() {
    let mut game = Game::new(1108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let bench = staffed(&mut game, "assembler", Some("recompile_kernel"), 42, 40);
    let annealer = staffed(&mut game, "factory", Some("annealed_core"), 41, 40);
    // Pre-stocked rather than mined, for the reason the armoury chain above
    // is: this measures the chain, not `mining_success_chance`'s roll.
    stocked(&mut game, "extractor", 40, 40, ids::CORE_FRAGMENT, 400);

    for _ in 0..400 {
        game.tick();
    }

    assert!(
        output_of(&game, annealer, "annealed_core") > 0
            || input_of(&game, bench, "annealed_core") > 0,
        "the annealing node turned fragments into cores"
    );
    assert!(
        output_of(&game, bench, "recompile_kernel") > 0,
        "and the bench built kernels out of them"
    );
}

/// The module chain, which is the one that proves the two gear classes draw
/// on *different* taps: this one runs off the Log Scraper's Raw Trace through
/// the Transcriber, and never touches a Mining Node.
///
/// ```text
///   T S *      T trace_extractor  S wafer_factory  * sniffer_assembler
/// ```
#[test]
fn the_fabricator_chain_produces_a_trace_sniffer_off_the_trace_tap() {
    let mut game = Game::new(1104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let sniffer_assembler = staffed(&mut game, "assembler", Some("trace_sniffer"), 42, 40);
    let wafer_factory = staffed(&mut game, "factory", Some("logic_wafer"), 41, 40);
    stocked(&mut game, "extractor", 40, 40, "raw_trace", 600);

    for _ in 0..600 {
        game.tick();
    }

    assert!(
        output_of(&game, wafer_factory, "logic_wafer") > 0
            || input_of(&game, sniffer_assembler, "logic_wafer") > 0,
        "the wafer_factory turned trace into wafers"
    );
    assert!(
        output_of(&game, sniffer_assembler, "trace_sniffer") > 0,
        "and the sniffer_assembler built a module out of them"
    );
}

/// Flattening the chains gave every intermediate exactly one bench, so no
/// shipped feeder has two *different* consumers any more — but a player may
/// still stand two of the same bench on one feeder, which is the real
/// contest. With one coil to give, the `(x, y)` sort decides who eats.
///
/// The two are spawned in the reverse of their positions on purpose: in
/// position order this would pass on bevy's iteration order alone, which is
/// the exact bug the sort exists to prevent.
///
/// ```text
///     Y        Y patch_printer (42, 40)
///   Y W        Y patch_printer (41, 41)  W coil_factory (42, 41)
/// ```
#[test]
fn two_benches_competing_for_one_coil_resolve_in_position_order() {
    let mut game = Game::new(1105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let above = staffed(&mut game, "printer", Some("patch_routine"), 42, 40);
    let left = staffed(&mut game, "printer", Some("patch_routine"), 41, 41);
    stocked(&mut game, "factory", 42, 41, ids::CHARGE_COIL, 1);

    game.tick();

    assert_eq!(
        input_of(&game, left, ids::CHARGE_COIL),
        1,
        "the machine at the lower x is visited first and takes the only coil"
    );
    assert_eq!(
        input_of(&game, above, ids::CHARGE_COIL),
        0,
        "the bay above it is behind in sort order"
    );
}

/// A staffed structure of `kind` at an absolute tile.
///
/// The task carries the machine's own `ticks_per_unit`, because
/// `assembler_system` reads `Task::required` — these tests are about which
/// machine pulls what, and a worker that finished a batch on tick 1 would
/// eat the very input they assert on.
fn staffed(game: &mut Game, kind: &str, recipe: Option<&str>, x: i32, y: i32) -> Entity {
    let machine = deployed(game, kind, recipe, x, y);
    let def = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get(kind)
        .cloned();
    let required = def
        .and_then(|d| match (&d.machine, recipe) {
            (Some(m), Some(r)) => m
                .recipes
                .iter()
                .find(|c| c.produces.as_str() == r)
                .map(|c| c.ticks_per_unit),
            _ => None,
        })
        .map_or(1, |t| t.max(1));
    let worker = spawn_tamed(game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required,
    });
    machine
}

/// A structure of `kind` at an absolute tile with `qty` of `item` already in
/// its output buffer, standing in for an extractor that has been running.
fn stocked(game: &mut Game, kind: &str, x: i32, y: i32, item: &str, qty: u32) -> Entity {
    let e = deployed(game, kind, Some(item), x, y);
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
///
/// `stand_ample_grid_supply` is what keeps that bypass from also skipping
/// the base's power supply: every chain here draws power now that Task 4
/// authored real numbers onto it, and none of these fixtures ever deploys a
/// Home.
///
/// A configurable machine is set to `recipe` through the real path; a
/// structure with nothing to set (a Research Node, a modded assembler) takes
/// `None`.
fn deployed(game: &mut Game, kind: &str, recipe: Option<&str>, x: i32, y: i32) -> Entity {
    stand_ample_grid_supply(game);
    if let Some(recipe) = recipe {
        return spawn_machine_with_recipe_at(game, kind, recipe, x, y);
    }
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
    let block_factory = deployed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    let mine = deployed(&mut game, "extractor", Some("core_fragment"), 40, 40);

    assert_eq!(
        edges_of(&mut game, block_factory),
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
    let block_factory = deployed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    deployed(&mut game, "extractor", Some("core_fragment"), 40, 39);

    assert!(edges_of(&mut game, block_factory).is_empty());
}

/// Touching is not the same as feeding. A Research Node beside a Refinery
/// makes Research Data, which no recipe of the Refinery's wants.
#[test]
fn a_neighbour_making_something_the_recipe_does_not_want_is_not_wired() {
    let mut game = Game::new(1202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let block_factory = deployed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    deployed(&mut game, "research_node", None, 40, 40);

    assert!(edges_of(&mut game, block_factory).is_empty());
}

/// The failure a player actually hits, now that a bench takes one ingredient
/// rather than two: a Bay built against the wrong machine. A Refinery touches
/// it and makes something real, but nothing the Bay's recipe wants — so it
/// shows no link at all, and the mistake is visible on the map without
/// opening a menu.
#[test]
fn a_bench_beside_the_wrong_feeder_reports_no_edge() {
    let mut game = Game::new(1203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bay = deployed(&mut game, "printer", Some("patch_routine"), 42, 40);
    deployed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    // The feeder it actually wants, one tile too far south — touching nothing.
    deployed(&mut game, "factory", Some("charge_coil"), 42, 42);

    assert!(
        edges_of(&mut game, bay).is_empty(),
        "a Refinery feeds an Armoury, not a Bay"
    );

    // Move it into place and the join appears.
    let mut game = Game::new(1203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bay = deployed(&mut game, "printer", Some("patch_routine"), 42, 40);
    deployed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    deployed(&mut game, "factory", Some("charge_coil"), 42, 41);

    assert_eq!(edges_of(&mut game, bay), vec![(0, 1)]);
}

/// A Home assembles nothing and runs no job, so it has neither half of the
/// map's machine vocabulary — no links and no status outline.
#[test]
fn a_home_reports_no_edges_and_no_machine_status() {
    let mut game = Game::new(1204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
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
    let block_factory = staffed(&mut game, "factory", Some("bytecode_block"), 41, 40);
    stocked(&mut game, "extractor", 40, 40, ids::CORE_FRAGMENT, 100);

    assert_eq!(edges_of(&mut game, block_factory), vec![(-1, 0)]);
    game.tick();

    assert!(
        input_of(&game, block_factory, ids::CORE_FRAGMENT) > 0,
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
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_machine(&mut game, "factory", "bytecode_block", 1, 0)
        .expect("a Factory is buildable from the start");

    let block_factory = find_structure_by_kind(&mut game, "factory").unwrap();
    // Setting the recipe leaves the status as it was until the next tick.
    game.tick();
    // `Idle`, not `Running`: `place_structure` ticks, and a machine nobody
    // is posted to correctly reports having no program. The point of the
    // assertion is that it reports *at all* — without a `MachineStatus` it
    // would read as `None`, which every consumer treats as "not a machine".
    assert_eq!(
        game.world.get::<MachineStatus>(block_factory).copied(),
        Some(MachineStatus::Idle),
        "an assembler is a machine and has a state like an extractor does"
    );
    assert!(
        game.world.get::<Stock>(block_factory).is_some(),
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
    machine: Some((role: Assemble, recipes: [(produces: "loop_item", ticks_per_unit: 3)])),
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
        .expect("the Printer assembles one");

    /// One step flattened for comparison: its inputs, its tool, its output.
    type StepShape<'a> = (Vec<(&'a str, u32)>, Option<&'a str>, &'a str);

    let shape: Vec<StepShape> = chain
        .steps
        .iter()
        .map(|s| {
            (
                s.inputs
                    .iter()
                    .map(|i| (i.item.as_str(), i.qty))
                    .collect::<Vec<_>>(),
                s.maker.as_deref(),
                s.output.as_str(),
            )
        })
        .collect();

    assert_eq!(
        shape,
        vec![
            (vec![("Core Fragment", 2)], None, "Power Cell"),
            (vec![("Power Cell", 3)], Some("Workbench"), "Charge Coil"),
            (vec![("Charge Coil", 3)], Some("Printer"), "Patch Routine"),
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
        .expect("the Extractor produces them");

    assert_eq!(chain.steps.len(), 1, "a tap has no upstream");
    assert!(
        chain.steps[0].inputs.is_empty(),
        "an extractor draws from nothing, not from a recipe"
    );
    assert_eq!(chain.steps[0].maker.as_deref(), Some("Extractor"));
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
        .expect("a Factory assembles them");
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
    // Flattening the chains broke the three-way tie that used to sit at the
    // bottom: gear is two steps off its tap now, and only the Patch Routine
    // still runs three deep, because Power Cell has a recipe of its own under
    // the Charge Coil. One product at the bottom is a stronger statement than
    // a set, so it is named.
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
        ["Patch Routine"],
        "the deepest thing in the game sits at the bottom"
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
    stand_ample_grid_supply(&mut game);
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

    let compiler = spawn_machine_with_recipe_at(&mut game, "printer", ids::ICE_BREAKER, 40, 40);
    let worker = spawn_tamed(&mut game, 10, 3);
    // The Printer's own `ticks_per_unit` for the recipe, for the reason
    // `staffed` carries it: `assembler_system` reads `Task::required`, and a
    // hand-written `1` would have this machine finish a batch every tick.
    let required = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get("printer")
        .and_then(|d| d.machine.as_ref())
        .and_then(|m| {
            m.recipes
                .iter()
                .find(|r| r.produces.as_str() == ids::ICE_BREAKER)
        })
        .map(|r| r.ticks_per_unit.max(1))
        .expect("the Printer ships an ice_breaker recipe");
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: compiler,
        progress: 0,
        required,
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

/// A modded assembler for an item priced entirely in a drop, so the "input
/// that nothing at all produces" case is reachable — no shipped structure
/// assembles one of the Portal Fragment recipes.
const ROUTER_STRUCTURE: &str = r#"(
    id: "router_press",
    name: "Router Press",
    description: "A modded assembler for an item made from salvage, for tests.",
    glyph: 'R',
    color: Red,
    build_cost: [],
    work: None,
    capacity: 20,
    machine: Some((role: Assemble, recipes: [(produces: "plasma_router", ticks_per_unit: 3)])),
)"#;

/// The Recipes screen's whole job is telling the player what to build, so an
/// ingredient that no recipe makes has to name the tap that does. Core
/// Fragment and Raw Trace are the two the shipped game bottoms out in, and
/// before this the screen simply started at them with no hint a Mining Node
/// was involved.
#[test]
fn an_input_no_recipe_makes_names_the_tap_that_produces_it() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let sourced = |product: &str| -> Vec<(String, Option<String>)> {
        chains
            .iter()
            .find(|c| c.product == product)
            .unwrap_or_else(|| panic!("{product} is assembled by a shipped bench"))
            .steps
            .iter()
            .flat_map(|s| &s.inputs)
            .map(|i| (i.item.clone(), i.source.clone()))
            .collect()
    };

    // Both taps are checked, on the two chains that now bottom out in them.
    // Flattening split them apart — the Routine Disk used to draw on both,
    // and asserting only there would have quietly stopped covering the Log
    // Scraper the moment its leg moved to the Fabricator.
    assert_eq!(
        sourced("Routine Disk"),
        vec![
            ("Core Fragment".into(), Some("Extractor".into())),
            ("Blank Substrate".into(), None),
        ],
        "the raw input names its tap; the intermediate is made by an earlier \
         step of this same chain and names none"
    );
    assert_eq!(
        sourced("Trace Sniffer"),
        vec![
            ("Raw Trace".into(), Some("Extractor".into())),
            ("Logic Wafer".into(), None),
        ],
        "and the module chain names the other tap"
    );
}

/// Power Cell is produced by a Power Conduit *and* craftable by hand, and the
/// chain deliberately reports the recipe rather than the tap: the hand step is
/// already on screen one line up, so naming the Conduit too would claim two
/// sources for one item in a single chain. Substitution is for inputs no step
/// of the chain produces.
#[test]
fn an_input_with_its_own_recipe_names_no_tap_even_when_a_structure_makes_it() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let coil = chains
        .iter()
        .find(|c| c.product == "Charge Coil")
        .expect("a Factory assembles them");
    let step = coil
        .steps
        .iter()
        .find(|s| s.output == "Charge Coil")
        .expect("the chain ends in its product");

    assert_eq!(
        step.inputs
            .iter()
            .map(|i| (i.item.as_str(), i.source.as_deref()))
            .collect::<Vec<_>>(),
        vec![("Power Cell", None)],
        "a Power Conduit produces Power Cells, but the chain already shows \
         the bench step that makes these ones"
    );
}

/// An ingredient that is neither craftable nor produced by any structure is a
/// drop, and the screen has nothing to offer beyond its name.
#[test]
fn an_input_nothing_produces_names_no_tap() {
    let dir = assets_dir_with_extra_structure("router_press", "router_press.ron", ROUTER_STRUCTURE);
    let game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();

    let chains = game.recipe_chains();
    let router = chains
        .iter()
        .find(|c| c.product == "Plasma Router")
        .expect("the modded press assembles one");

    assert_eq!(
        router
            .steps
            .iter()
            .flat_map(|s| &s.inputs)
            .map(|i| (i.item.as_str(), i.source.as_deref()))
            .collect::<Vec<_>>(),
        vec![("Portal Fragment", None)],
        "portal fragments are scavenged, not made"
    );
}

/// A recipe is one batch in, one unit out, so the screen can say so. A tap
/// cannot: `systems::node_payout` scales its yield by upgrade tier and zone
/// depth, and quoting `x1` there would be a number the game never honours.
#[test]
fn a_recipe_step_yields_one_unit_and_a_tap_declines_to_say() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let disk = chains
        .iter()
        .find(|c| c.product == "Routine Disk")
        .expect("the Disk Press assembles one");
    assert!(
        disk.steps.iter().all(|s| s.output_qty == Some(1)),
        "every step of a crafted chain is a single-unit batch"
    );

    let fragment = chains
        .iter()
        .find(|c| c.product == "Core Fragment")
        .expect("the Extractor produces them");
    assert_eq!(
        fragment.steps[0].output_qty, None,
        "a node's payout is not fixed at one"
    );
}

/// The chains say how to make a thing and never why you would. The product's
/// own authored prose is what answers that, and it is carried as the string
/// rather than the id for the reason `RecipeStep`'s names are: the renderer
/// holds no `ItemDb` to resolve one with.
#[test]
fn a_chain_carries_its_products_authored_description() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let chains = game.recipe_chains();

    let cell = chains
        .iter()
        .find(|c| c.product == "Power Cell")
        .expect("the Power Conduit produces them");
    assert_eq!(
        cell.description.as_deref(),
        game.item_description(&ItemId::from(ids::POWER_CELL)),
        "the chain quotes power_cell.ron's own prose, not a derived gloss"
    );

    let missing: Vec<&str> = chains
        .iter()
        .filter(|c| c.description.is_none())
        .map(|c| c.product.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "every shipped item carries description text, so every chain shows one: {missing:?}"
    );
}

/// The assembler's instrumentation seam. Consumption and production are one
/// event because the input drain and the `output` write happen in the same
/// scope — split across two emissions they could be counted in different
/// ticks, and the ledger would stop balancing.
#[test]
fn a_completed_assembly_reaches_the_ledger_with_its_inputs() {
    let mut game = game_with_assembler("chain_ledger", 1100);
    let machine = assembler_at(&mut game, 40, 40, true);
    feeder_at(&mut game, 41, 40, 100);
    let cost = per_batch(&game);

    for _ in 0..40 {
        game.tick();
        let made = game
            .world
            .get::<Stock>(machine)
            .and_then(|s| s.output.get(&ItemId::from(ids::POWER_CELL)).copied())
            .unwrap_or(0);
        if made > 0 {
            break;
        }
    }

    let ledger = game.world.resource::<crate::base_ledger::BaseLedger>();
    let product = ledger.lifetime[&ItemId::from(ids::POWER_CELL)];
    assert_eq!(
        product.compiled, 1,
        "the completed unit must reach the ledger as machine work"
    );
    assert_eq!(product.hand, 0, "and not as the player's own");
    assert_eq!(
        ledger.lifetime[&ItemId::from(ids::CORE_FRAGMENT)].consumed,
        cost,
        "the drained inputs ride the same event, priced off the item's own recipe"
    );
}
