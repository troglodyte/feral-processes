//! Work orders: the base staff pool, the queue, and the scheduler that
//! turns one into postings against the other.

use super::support::*;
use crate::*;

/// A scratch save path unique to this process and `tag`, so two tests in
/// the same run can't tread on each other's file.
fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_work_orders_{tag}_{}.bin",
        std::process::id()
    ))
}

#[test]
fn assigning_a_program_you_own_puts_it_on_the_base_staff() {
    let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.resource_mut::<resources::Party>().0.push(worker);

    game.assign_base_staff(worker).unwrap();

    assert!(
        game.world.get::<components::BaseStaff>(worker).is_some(),
        "the program must be marked as base staff"
    );
    assert!(
        !game
            .world
            .resource::<resources::Party>()
            .0
            .contains(&worker),
        "staff and party are disjoint sets"
    );
    assert_eq!(game.base_staff(), vec![worker]);
}

#[test]
fn assigning_a_program_you_do_not_own_is_refused() {
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    let err = game
        .assign_base_staff(wild)
        .expect_err("a wild program is nobody's to post");

    assert!(!err.is_empty());
    assert!(game.world.get::<components::BaseStaff>(wild).is_none());
    assert!(game.base_staff().is_empty());
}

#[test]
fn releasing_a_staff_member_clears_the_marker() {
    let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_base_staff(worker).unwrap();

    game.release_base_staff(worker).unwrap();

    assert!(game.world.get::<components::BaseStaff>(worker).is_none());
    assert!(game.base_staff().is_empty());
}

#[test]
fn the_base_staff_marker_survives_a_save_round_trip() {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_base_staff(worker).unwrap();

    let path = save_path("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.base_staff().len(),
        1,
        "the staff pool must come back with its one member"
    );
}

/// The load-path absorption rule. A base built before work orders existed
/// has its workers posted by hand and no `staff` flag on disk; standing
/// them all down on the first load after the feature ships would empty a
/// working base. Anything holding a `Task` comes back as staff.
#[test]
fn a_hand_posted_cronjob_loads_back_as_base_staff() {
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 1, 0);
    let node = spawn_mining_node(&mut game, 3, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
    stand_player_at_post(&mut game, node);
    game.assign_cronjob(worker, node).unwrap();
    // The saved file predates the flag: a hand-posted worker was never
    // staff, so the absorption has to work off the `Task` alone.
    assert!(
        game.world.get::<components::BaseStaff>(worker).is_none(),
        "precondition: posting by hand does not itself mark staff"
    );

    let path = save_path("absorb");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let staff = loaded.base_staff();
    assert_eq!(
        staff.len(),
        1,
        "the posted worker must be absorbed as staff"
    );
    assert!(
        loaded.world.get::<components::Task>(staff[0]).is_some(),
        "and must still be on its machine"
    );
}

// ---------------------------------------------------------------------
// Task 2: chain resolution and queue-time refusal
// ---------------------------------------------------------------------

/// The shipped three-deep line for a Routine Disk, laid out so each
/// machine is orthogonally adjacent to its feeder:
/// Mining Node (2,0) → Lathe (3,0) → Disk Press (4,0).
///
/// Returns the three entities in that order.
fn lay_disk_line(game: &mut Game) -> (Entity, Entity, Entity) {
    place_home(&mut *game, 0, 1);
    let mine = spawn_machine_at(game, "mining_node", 2, 0);
    let lathe = spawn_machine_at(game, "lathe", 3, 0);
    let press = spawn_machine_at(game, "disk_press", 4, 0);
    (mine, lathe, press)
}

#[test]
fn an_item_no_deployed_machine_makes_is_refused_by_name() {
    let mut game = Game::new(10, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    // Everything upstream of the press is standing; the press itself is not.
    spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 3, 0);

    let err = game
        .queue_work_order(ItemId::from("routine_disk"), 3)
        .expect_err("nothing deployed presses a disk");

    assert!(
        err.contains("Disk Press"),
        "the refusal must name the missing machine, got: {err}"
    );
    assert!(game.work_orders().is_empty());
}

#[test]
fn a_machine_with_no_feeder_beside_it_is_refused_by_link() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 3, 0);
    // Deployed, but nowhere near the Lathe, so nothing can ever feed it.
    spawn_machine_at(&mut game, "disk_press", 9, 9);

    let err = game
        .queue_work_order(ItemId::from("routine_disk"), 3)
        .expect_err("a press with no substrate beside it can never run");

    assert!(
        err.contains("Blank Substrate"),
        "the refusal must name the missing link, got: {err}"
    );
    assert!(game.work_orders().is_empty());
}

#[test]
fn an_item_nothing_declares_as_a_product_is_refused_as_unmakeable() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let unmakeable = game
        .item_defs()
        .into_iter()
        .find(|d| {
            game.structure_defs()
                .iter()
                .all(|s| systems::produced_item(s) != Some(&d.id))
        })
        .expect("some shipped item is not made by any structure")
        .id;

    let err = game
        .queue_work_order(unmakeable.clone(), 1)
        .expect_err("nothing in the base can make it");

    assert!(!err.is_empty());
    assert!(game.work_orders().is_empty());
}

/// The banked exclusion, and the assertion is deliberately about *why*.
/// `research_data` is refused because a banked payout never reaches an
/// `output` — nothing can hold a stock of it and nothing can be fed from
/// it — not because the id is special-cased. A test that passed against
/// `if item == "research_data"` would be testing the wrong thing, so this
/// stands the Research Node up first: the machine is there, and the refusal
/// still has to come from the item.
#[test]
fn a_banked_item_is_refused_even_with_its_machine_standing() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);

    assert_eq!(
        crate::game::base::work_orders::producer_of(&game, &ItemId::from("research_data")),
        Some(node),
        "precondition: the machine that gathers it is deployed and findable"
    );

    let err = game
        .queue_work_order(ItemId::from("research_data"), 5)
        .expect_err("a banked item reaches no output, so no base can hold a stock of it");

    assert!(!err.is_empty());
    assert!(game.work_orders().is_empty());
}

#[test]
fn a_whole_line_correctly_laid_out_is_accepted() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    lay_disk_line(&mut game);

    game.queue_work_order(ItemId::from("routine_disk"), 3)
        .expect("a three-deep line stood up end to end must be orderable");

    assert_eq!(game.work_orders().len(), 1);
    assert_eq!(game.work_orders()[0].item, ItemId::from("routine_disk"));
    assert_eq!(game.work_orders()[0].qty, 3);
}

/// Cancelling unwinds nothing, because nothing was wound — there are no
/// per-machine targets to roll back and no reserved stock to release. That
/// absence is asserted so a later reader does not add an unwind path for a
/// state that never existed.
#[test]
fn cancelling_an_order_shifts_the_queue_up_and_unwinds_nothing() {
    let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, press) = lay_disk_line(&mut game);
    game.queue_work_order(ItemId::from("core_fragment"), 5)
        .unwrap();
    game.queue_work_order(ItemId::from("routine_disk"), 3)
        .unwrap();

    game.cancel_work_order(0).unwrap();

    assert_eq!(game.work_orders().len(), 1);
    assert_eq!(
        game.work_orders()[0].item,
        ItemId::from("routine_disk"),
        "the order behind it shifts up"
    );
    for machine in [mine, lathe, press] {
        assert!(
            game.world.get::<components::Task>(machine).is_none(),
            "no machine ever carried a target to unwind"
        );
    }
}

#[test]
fn cancelling_an_out_of_range_order_is_refused_rather_than_panicking() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    lay_disk_line(&mut game);
    game.queue_work_order(ItemId::from("core_fragment"), 5)
        .unwrap();

    assert!(game.cancel_work_order(7).is_err());
    assert_eq!(game.work_orders().len(), 1, "the queue is untouched");
}

#[test]
fn work_orders_round_trip_through_a_save() {
    let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    lay_disk_line(&mut game);
    game.queue_work_order(ItemId::from("routine_disk"), 3)
        .unwrap();
    game.queue_work_order(ItemId::from("core_fragment"), 9)
        .unwrap();

    let path = save_path("orders");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let orders = loaded.work_orders();
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].item, ItemId::from("routine_disk"));
    assert_eq!(orders[0].qty, 3);
    assert_eq!(orders[1].item, ItemId::from("core_fragment"));
    assert_eq!(orders[1].qty, 9);
}

/// A real file with the key stripped back out, rather than a hand-written
/// string — the same thing `a_save_written_before_contracts_existed_still_
/// loads` does, and for the same reason: a hand-written fixture asserts
/// what its author believed the format was.
#[test]
fn a_save_written_before_work_orders_existed_still_loads() {
    // No order queued, so the field serialises as a single `[]` line and
    // stripping it leaves a file a build without the field would have
    // written — which is the file under test. An order would spread the key
    // over a block and stripping only its first line would leave a dangling
    // `[`, which is a torn file rather than an older one.
    let mut game = Game::new(18, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    lay_disk_line(&mut game);

    let path = save_path("legacy");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("work_orders:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(stripped, text, "precondition: the key was there to remove");
    std::fs::write(&path, stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.work_orders().is_empty(),
        "a file written before the feature loads with no orders, not a parse error"
    );
}

// ---------------------------------------------------------------------
// Task 3: can_progress and wants
// ---------------------------------------------------------------------

use crate::game::base::work_orders::{base_holding, can_progress, wants};

/// Fills `machine`'s output to its capacity with `item`, which is what
/// clogging is.
fn clog_with(game: &mut Game, machine: Entity, item: &str) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    let room = stock.output_room();
    *stock.output.entry(ItemId::from(item)).or_default() += room;
}

fn clog(game: &mut Game, machine: Entity) {
    clog_with(game, machine, ids::CORE_FRAGMENT);
}

fn put_output(game: &mut Game, machine: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    *stock.output.entry(ItemId::from(item)).or_default() += qty;
}

fn put_input(game: &mut Game, machine: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    *stock.input.entry(ItemId::from(item)).or_default() += qty;
}

#[test]
fn a_clogged_machine_cannot_progress() {
    let mut game = Game::new(20, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);

    assert!(
        can_progress(&game, mine),
        "precondition: an empty extractor has room"
    );
    clog(&mut game, mine);

    assert!(
        !can_progress(&game, mine),
        "a machine with no output room is what releases its worker"
    );
}

#[test]
fn an_assembler_with_nothing_beside_it_holding_its_ingredient_cannot_progress() {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let lathe = spawn_machine_at(&mut game, "lathe", 3, 0);
    let _ = mine;

    assert!(
        !can_progress(&game, lathe),
        "an empty input and an empty feeder is a starved machine"
    );
}

/// The case that decides the whole feature: an assembler with an empty
/// input whose *feeder* holds the ingredient can progress, because
/// staffing it is what makes it pull. `assembler_system`'s pull phase sits
/// behind the "is anyone posted here" gate, so a Lathe with nobody on it
/// never fills its own input — meaning "input is empty" is not the same
/// question as "this machine has nothing to do".
#[test]
fn an_assembler_whose_feeder_holds_the_ingredient_can_progress() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let lathe = spawn_machine_at(&mut game, "lathe", 3, 0);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);

    assert!(
        can_progress(&game, lathe),
        "a full feeder beside it is work waiting to be done"
    );
}

#[test]
fn an_assembler_with_a_stocked_input_can_progress_with_no_feeder_at_all() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let lathe = spawn_machine_at(&mut game, "lathe", 3, 0);
    put_input(&mut game, lathe, ids::CORE_FRAGMENT, 8);

    assert!(can_progress(&game, lathe));
}

/// On a base with every buffer empty, the top of the line is the only thing
/// with anything to do — a body on the Lathe would stand there pulling from
/// an empty Mining Node — so that is the only machine that wants one.
#[test]
fn on_an_empty_base_only_the_top_of_the_line_wants_a_body() {
    let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let order = crate::game::base::work_orders::WorkOrder {
        item: ItemId::from("routine_disk"),
        qty: 3,
    };

    let order_of: Vec<Entity> = wants(&game, &order).into_iter().map(|(e, _)| e).collect();

    assert_eq!(order_of, vec![mine]);
}

/// Once each stage has something to work on, the whole line wants bodies —
/// **deepest first**, which is what makes a lone body work upstream and a
/// full roster spread down the chain rather than crowding its far end.
#[test]
fn wants_orders_a_running_line_upstream_first() {
    let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 10);
    let order = crate::game::base::work_orders::WorkOrder {
        item: ItemId::from("routine_disk"),
        qty: 3,
    };

    let order_of: Vec<Entity> = wants(&game, &order).into_iter().map(|(e, _)| e).collect();

    assert_eq!(
        order_of,
        vec![mine, lathe, press],
        "the deepest requirement is what needs a body first"
    );
}

/// A modded two-ingredient item, and the machine that builds it. No shipped
/// `assembles` recipe has more than one ingredient — that is a deliberate
/// property of the shipped items, so a chain is a straight line — but the
/// engine's multi-input support is real and mods may ship one, which is the
/// same reason `chains::a_machine_short_one_of_its_two_ingredients_stays_
/// starved` exists.
///
/// Its two ingredients put one Mining Node down two branches of very
/// different lengths — straight in as a Core Fragment, and three links away
/// through the Routine Disk line. The **order** of the two is load-bearing
/// to the test rather than to the game: the long branch is walked first, so
/// an implementation that kept the *last* depth it saw rather than the
/// deepest records 1 instead of 3 and the assertion catches it. Swap them
/// and the mutation becomes invisible.
const SHARED_FEEDER_ITEM: &str = r#"(
    id: "test_widget",
    name: "Test Widget",
    description: "A modded two-ingredient item, for tests.",
    value: Some(20),
    craftable: Some((cost: [("routine_disk", 1), ("core_fragment", 1)], requires_structure: Some("widget_bench"))),
)"#;

const SHARED_FEEDER_ASSEMBLER: &str = r#"(
    id: "widget_bench",
    name: "Widget Bench",
    description: "A modded two-ingredient assembler, for tests.",
    glyph: 'W',
    color: Magenta,
    build_cost: [],
    work: None,
    capacity: 20,
    assembles: Some((item: "test_widget", ticks_per_unit: 12)),
)"#;

/// A machine reached down two paths is kept once, at its deepest position
/// — otherwise a shared feeder is staffed second on behalf of the short
/// branch while the long one still needs it first, and a lone body works
/// the wrong end of the line.
///
/// The layout, with the Widget Bench as the ordered item's producer:
///
/// ```text
///   Mining ── Lathe          Mining feeds the Bench directly (depth 1)
///      │        │            and the Lathe, three links round (depth 3)
///   Bench ── Disk Press
/// ```
///
/// Note the geometry is not free: two orthogonally adjacent tiles share no
/// common orthogonal neighbour, so a feeder can never sit beside both a
/// bench and the machine that bench feeds. A shared feeder at *differing*
/// depths therefore needs the long way round, which is what this is.
#[test]
fn wants_keeps_a_shared_feeder_once_at_its_deepest_position() {
    let dir = assets_dir_with_extra_machine(
        "wo_shared_feeder",
        ("test_widget.ron", SHARED_FEEDER_ITEM),
        ("widget_bench.ron", SHARED_FEEDER_ASSEMBLER),
    );
    let mut game = Game::new(25, DifficultyMode::Forgiving, &dir).unwrap();
    place_home(&mut game, 4, 4);
    let bench = spawn_machine_at(&mut game, "widget_bench", 0, 0);
    let mine = spawn_machine_at(&mut game, "mining_node", 0, 1);
    let press = spawn_machine_at(&mut game, "disk_press", 1, 0);
    let lathe = spawn_machine_at(&mut game, "lathe", 1, 1);
    // Every stage stocked, so every machine has work and the whole line is
    // in the answer — with empty buffers only the feeder would be, and
    // "kept once at its deepest" would be vacuously true.
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    put_output(&mut game, press, "routine_disk", 4);

    let order = crate::game::base::work_orders::WorkOrder {
        item: ItemId::from("test_widget"),
        qty: 1,
    };
    let list = wants(&game, &order);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        list.iter().filter(|(e, _)| *e == mine).count(),
        1,
        "the shared feeder appears once, not once per branch"
    );
    let depth_of = |e: Entity| list.iter().find(|(m, _)| *m == e).unwrap().1;
    assert_eq!(depth_of(bench), 0);
    assert_eq!(depth_of(press), 1);
    assert_eq!(depth_of(lathe), 2);
    assert_eq!(
        depth_of(mine),
        3,
        "the feeder is kept at the deepest position it was reached at, not the last"
    );
    assert_eq!(
        list[0].0, mine,
        "so it is what a lone body is sent to first"
    );
}

#[test]
fn base_holding_counts_depots_and_machine_outputs_but_not_your_pockets() {
    let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let depot = spawn_machine_at(&mut game, "depot", 4, 0);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 7);
    put_output(&mut game, depot, ids::CORE_FRAGMENT, 5);
    let carried = game
        .world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .items
        .iter()
        .find(|(i, _)| i.as_str() == ids::CORE_FRAGMENT)
        .map(|(_, q)| *q)
        .unwrap_or(0);
    assert!(carried > 0, "precondition: the player starts holding some");

    assert_eq!(
        base_holding(&game, &ItemId::from(ids::CORE_FRAGMENT)),
        12,
        "what the base holds, not what you are carrying"
    );
}

// ---------------------------------------------------------------------
// Task 4: the scheduler
// ---------------------------------------------------------------------

/// Where `worker` is currently posted, or `None` if it is idle.
fn posted_at(game: &Game, worker: Entity) -> Option<Entity> {
    game.world.get::<Task>(worker).map(|t| t.target)
}

/// `n` programs on the base staff, in the order `base_staff` will return
/// them.
fn hire(game: &mut Game, n: usize) -> Vec<Entity> {
    let mut staff = Vec::new();
    for _ in 0..n {
        let worker = spawn_tamed(game, 10, 3);
        game.assign_base_staff(worker).unwrap();
        staff.push(worker);
    }
    staff.sort();
    staff
}

#[test]
fn one_staff_member_is_posted_to_the_top_of_the_line() {
    let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, press) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(ItemId::from("routine_disk"), 3)
        .unwrap();

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "nothing else in the line has anything to do yet"
    );
    let _ = (lathe, press);
}

/// The whole of "work the deepest requirement until it is made, then move
/// on", and it is not sequenced by the scheduler — it falls out of
/// `can_progress` being false for a clogged machine.
#[test]
fn a_lone_body_walks_the_line_downstream_as_each_machine_stops_being_useful() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, press) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(ItemId::from("routine_disk"), 30)
        .unwrap();
    game.tick();
    assert_eq!(posted_at(&game, staff[0]), Some(mine), "precondition");

    // The Mining Node fills its buffer and can do no more; the Lathe now
    // has a feeder full of fragments.
    clog(&mut game, mine);
    game.tick();
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(lathe),
        "a clogged machine stops wanting a body, so the body moves downstream"
    );

    // The Lathe fills its own buffer in turn, and the Mining Node fills the
    // room the Lathe's pull just made in its. Both upstream machines are
    // clogged and only the Press has room, so the Press is the one thing
    // left in the base that a body can move.
    //
    // Re-clogging the Mining Node is not fussiness: a staffed Lathe drains
    // its feeder, which is exactly what gives that feeder something to do
    // again — a body cycles back upstream rather than marching one way down
    // the line, and that is the behaviour, not a wrinkle in the fixture.
    clog(&mut game, mine);
    clog_with(&mut game, lathe, "blank_substrate");
    game.tick();
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(press),
        "and again, once the Lathe has filled up behind it"
    );
}

#[test]
fn three_staff_spread_across_a_running_line_without_doubling_up() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    let staff = hire(&mut game, 3);
    game.queue_work_order(ItemId::from("routine_disk"), 30)
        .unwrap();

    game.tick();

    let mut posts: Vec<Entity> = staff.iter().filter_map(|&s| posted_at(&game, s)).collect();
    posts.sort();
    let mut expected = vec![mine, lathe, press];
    expected.sort();
    assert_eq!(posts, expected, "one body per machine, no machine twice");
}

#[test]
fn two_staff_take_the_two_deepest_machines() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    let staff = hire(&mut game, 2);
    game.queue_work_order(ItemId::from("routine_disk"), 30)
        .unwrap();

    game.tick();

    let mut posts: Vec<Entity> = staff.iter().filter_map(|&s| posted_at(&game, s)).collect();
    posts.sort();
    let mut expected = vec![mine, lathe];
    expected.sort();
    assert_eq!(
        posts, expected,
        "scarce bodies go upstream first; the Disk Press waits"
    );
    assert!(posts.iter().all(|&p| p != press));
}

/// An order is a **target level, not a production run** — three already in
/// a Depot means the base has three, and the order is done before anyone
/// is sent anywhere.
#[test]
fn an_order_the_base_already_holds_completes_without_staffing_anything() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_mine, _lathe, _press) = lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 6, 0);
    put_output(&mut game, depot, "routine_disk", 5);
    let staff = hire(&mut game, 1);
    game.queue_work_order(ItemId::from("routine_disk"), 3)
        .unwrap();

    game.tick();

    assert!(game.work_orders().is_empty(), "the order is popped");
    assert_eq!(
        posted_at(&game, staff[0]),
        None,
        "and nobody was sent anywhere to fill an order the base already met"
    );
}

/// Only *idle* staff are ever assigned and a program already posted where
/// a want still exists is never moved. Without that rule the scheduler
/// would walk the whole roster across the base whenever a buffer changed
/// by one unit.
#[test]
fn a_posted_worker_is_not_moved_when_an_unrelated_buffer_changes() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 6, 0);
    let staff = hire(&mut game, 1);
    game.queue_work_order(ItemId::from("routine_disk"), 30)
        .unwrap();
    game.tick();
    assert_eq!(posted_at(&game, staff[0]), Some(mine), "precondition");
    let progress_before = game.world.get::<Task>(staff[0]).unwrap().progress;

    put_output(&mut game, depot, ids::POWER_CELL, 4);
    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "an unrelated buffer moving is not a reason to walk the roster"
    );
    assert!(
        game.world.get::<Task>(staff[0]).unwrap().progress >= progress_before,
        "and the cronjob was not restarted from zero"
    );
}

/// Queue-time refusal catches a broken line when the order is placed, but
/// a machine can be demolished or swept to destruction after that. One
/// dead order must not freeze a base that could still work the ones
/// behind it.
#[test]
fn a_stalled_front_order_does_not_block_the_queue() {
    let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, _lathe, press) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(ItemId::from("routine_disk"), 30)
        .unwrap();
    game.queue_work_order(ItemId::from(ids::CORE_FRAGMENT), 30)
        .unwrap();

    game.world.entity_mut(press).despawn();
    game.tick();

    assert_eq!(
        game.work_orders().len(),
        2,
        "the stalled order stays listed rather than being silently dropped"
    );
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "and the order behind it is worked"
    );
}

#[test]
fn a_base_with_no_staff_queues_and_reports_without_posting_or_panicking() {
    let mut game = Game::new(37, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    lay_disk_line(&mut game);
    game.queue_work_order(ItemId::from("routine_disk"), 3)
        .unwrap();

    game.tick();
    game.tick();

    assert_eq!(game.work_orders().len(), 1);
    assert!(game.base_staff().is_empty());
}

// ---------------------------------------------------------------------
// Task 5: standing jobs
// ---------------------------------------------------------------------

/// A standing job is what keeps a machine running with no order behind it
/// — the Research Node is the case it exists for, since a banked payout
/// can never be ordered against at all.
#[test]
fn a_standing_work_job_is_filled_when_no_order_needs_the_body() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);
    let staff = hire(&mut game, 1);
    game.set_standing_job(node, true, false).unwrap();

    game.tick();

    assert_eq!(posted_at(&game, staff[0]), Some(node));
}

/// Standing jobs sit at the **lowest** priority, after whatever order is
/// being worked, so a spare body fills one and a needed body does not.
#[test]
fn a_standing_job_yields_the_body_to_an_order_and_takes_it_back_after() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let node = spawn_machine_at(&mut game, "research_node", 2, 3);
    let staff = hire(&mut game, 1);
    game.set_standing_job(node, true, false).unwrap();
    game.tick();
    assert_eq!(posted_at(&game, staff[0]), Some(node), "precondition");

    game.queue_work_order(ItemId::from("routine_disk"), 30)
        .unwrap();
    game.tick();
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "an order outranks a standing job for a scarce body"
    );

    game.cancel_work_order(0).unwrap();
    game.tick();
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(node),
        "and the standing job takes it back once the order is gone"
    );
}

/// A guard produces nothing, so no `can_progress` walk can ever ask for
/// one — a standing guard is the only way a post survives the sweep that
/// makes it worth having.
#[test]
fn a_standing_guard_is_filled_and_refilled_after_a_sweep() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let shield = spawn_machine_at(&mut game, "shield", 2, 0);
    let staff = hire(&mut game, 1);
    game.set_standing_job(shield, false, true).unwrap();
    game.tick();
    assert_eq!(
        game.world.get::<Task>(staff[0]).map(|t| t.kind),
        Some(TaskKind::Guard),
        "the post is a guard post, not a cronjob"
    );

    // The guard is stood down by hand, as a sweep's aftermath might.
    game.world.entity_mut(staff[0]).remove::<Task>();
    game.tick();

    assert_eq!(posted_at(&game, staff[0]), Some(shield), "and it re-fills");
}

#[test]
fn a_guard_job_on_an_unraidable_structure_is_refused() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let home = find_home(&mut game).expect("the Home is standing");

    let err = game
        .set_standing_job(home, false, true)
        .expect_err("a Home cannot be swept, so it does not need a guard");

    assert!(!err.is_empty());
    assert_eq!(game.standing_job(home), None);
}

/// The flags live on the structure entity, deliberately the opposite of
/// `BuybackLedger`: a shelf outlives its building on purpose, a job order
/// must not — a Shield rebuilt on the footprint of a demolished one should
/// not inherit a standing guard nobody asked for.
#[test]
fn standing_jobs_survive_a_save_but_not_a_demolition() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);
    game.set_standing_job(node, true, false).unwrap();

    let path = save_path("standing");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .world
        .iter_entities()
        .find(|e| {
            e.get::<Structure>()
                .is_some_and(|s| s.kind == "research_node")
        })
        .map(|e| e.id())
        .expect("the node comes back");
    assert_eq!(loaded.standing_job(restored), Some((true, false)));

    // Rebuilt on the same tile, the replacement carries no job order.
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let first = spawn_machine_at(&mut game, "research_node", 2, 0);
    game.set_standing_job(first, true, false).unwrap();
    game.world.entity_mut(first).despawn();
    let second = spawn_machine_at(&mut game, "research_node", 2, 0);
    assert_eq!(game.standing_job(second), None);
}

// ---------------------------------------------------------------------
// Task 6: idle staff on the map
// ---------------------------------------------------------------------

use crate::game::base::work_orders::park_tile;

#[test]
fn park_tile_is_a_pure_function_of_its_arguments() {
    let home = Position { x: 4, y: 7 };

    assert_eq!(park_tile(home, 0, 12), park_tile(home, 0, 12));
    assert_eq!(park_tile(home, 3, 99), park_tile(home, 3, 99));
}

#[test]
fn two_staff_park_on_different_tiles_at_the_same_tick() {
    let home = Position { x: 0, y: 0 };

    assert_ne!(park_tile(home, 0, 5), park_tile(home, 1, 5));
}

#[test]
fn a_parked_staff_member_stands_inside_the_base_and_off_its_structures() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);
    let staff = hire(&mut game, 2);
    let radius = game.build_radius();
    let home_entity = find_home(&mut game).unwrap();
    let home = *game.world.get::<Position>(home_entity).unwrap();

    for _ in 0..5 {
        game.tick();
    }

    let node_pos = *game.world.get::<Position>(node).unwrap();
    for worker in staff {
        let pos = *game.world.get::<Position>(worker).unwrap();
        assert!(
            (pos.x - home.x).abs().max((pos.y - home.y).abs()) <= radius,
            "an idle program loiters inside the base, not off in the wild"
        );
        assert_ne!(
            (pos.x, pos.y),
            (node_pos.x, node_pos.y),
            "and never on a tile a structure stands on"
        );
    }
}

/// The map and the inspector must stay the same set — that is the whole
/// reason `drawn_on_surface_map` is one function called by both.
#[test]
fn an_idle_staff_member_is_drawn_and_can_be_named_but_a_companion_is_neither() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 1);
    // `spawn_tamed_on_map` rather than `spawn_tamed`: `view_entities` draws
    // from `Glyph`, and a program without one is invisible for reasons that
    // have nothing to do with this rule.
    let idle = spawn_tamed_on_map(&mut game, 5, 5);
    game.assign_base_staff(idle).unwrap();
    let companion = spawn_tamed_on_map(&mut game, 6, 5);
    game.add_companion(companion).unwrap();
    game.tick();

    let drawn: Vec<Entity> = game
        .view_entities(40, 40)
        .into_iter()
        .filter(|e| views::drawn_on_surface_map(e.is_tamed, e.position_is_honest))
        .map(|e| e.entity)
        .collect();

    assert!(drawn.contains(&idle), "idle staff are on the map");
    assert!(
        !drawn.contains(&companion),
        "a party companion has no honest tile of its own, so it is not"
    );

    // Line the idle program up east of the player and confirm `x` names it.
    let player = game.player_entity();
    let here = *game.world.get::<Position>(player).unwrap();
    let mut pos = game.world.get_mut::<Position>(idle).unwrap();
    pos.x = here.x + 2;
    pos.y = here.y;
    assert!(
        game.find_target_in_direction(1, 0, 8).is_some(),
        "what the map draws is what the inspector can name"
    );
}

/// The scheduler draws **no** RNG — not `GameRng`, not a local `StdRng`.
/// `CLAUDE.md` records three occasions where a shifted stream silently
/// rewrote a seeded test in an unrelated file, and idle staff milling
/// every tick would shift it harder than anything currently in the game.
#[test]
fn idle_staff_take_no_rng_draws() {
    let sample = |staff_count: usize| {
        let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, 0, 1);
        hire(&mut game, staff_count);
        for _ in 0..20 {
            game.tick();
        }
        // Any draw off the shared stream lands here, and its value is a
        // pure function of how many draws came before it.
        let mut rng = game.world.resource_mut::<resources::GameRng>();
        rand::RngExt::random_range(&mut rng.0, 0..1_000_000u32)
    };

    assert_eq!(
        sample(0),
        sample(3),
        "three idle programs milling for twenty ticks must not move the stream"
    );
}
