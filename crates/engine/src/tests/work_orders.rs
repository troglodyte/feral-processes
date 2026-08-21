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

/// The staff pool is derived, so a round trip has nothing to restore — it
/// rebuilds the party and the wield and the roles fall out. `CreatureSave`
/// still *writes* a `staff` flag, the way `Experience::xp_to_next` is still
/// written, which is why this asserts on the pool and not on the field.
#[test]
fn the_base_staff_pool_survives_a_save_round_trip() {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_tamed(&mut game, 10, 3);

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

/// The other half of "derived, never stored": a party member is not staff
/// after a round trip either, and that is decided by `Party` coming back
/// rather than by any flag on the creature.
#[test]
fn a_party_member_does_not_come_back_as_base_staff() {
    let mut game = Game::new(6, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let member = spawn_tamed(&mut game, 10, 3);
    game.add_companion(member).unwrap();

    let path = save_path("party_roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.base_staff().is_empty(),
        "a program that loaded back into the party must not also be staff"
    );
}

/// A base built before work orders existed had its workers posted by hand
/// and no `staff` flag on disk. `Game::load` used to rescue those off their
/// `Task`; now nothing needs rescuing, because everything the player owns
/// and is not fighting with is staff. The worker must still come back on
/// its machine either way.
#[test]
fn a_hand_posted_cronjob_loads_back_as_base_staff() {
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let node = spawn_mining_node(&mut game, 3, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
    stand_player_at_post(&mut game, node);
    game.assign_cronjob(worker, node).unwrap();

    let path = save_path("absorb");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let staff = loaded.base_staff();
    assert_eq!(staff.len(), 1, "the posted worker must come back as staff");
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
    place_home(&mut *game);
    let mine = spawn_machine_at(game, "mining_node", 2, 0);
    let lathe = spawn_machine_at(game, "lathe", 3, 0);
    let press = spawn_machine_at(game, "disk_press", 4, 0);
    (mine, lathe, press)
}

#[test]
fn an_item_no_deployed_machine_makes_is_refused_by_name() {
    let mut game = Game::new(10, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    // Everything upstream of the press is standing; the press itself is not.
    spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 3, 0);

    let err = game
        .queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    stand_in_base(&mut game);
    place_home(&mut game);
    spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 3, 0);
    // Deployed, but nowhere near the Lathe, so nothing can ever feed it.
    spawn_machine_at(&mut game, "disk_press", 9, 9);

    let err = game
        .queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    stand_in_base(&mut game);
    place_home(&mut game);
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
        .queue_work_order(WorkOrder::batch(unmakeable.clone(), 1))
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
    stand_in_base(&mut game);
    place_home(&mut game);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);

    assert_eq!(
        crate::game::base::work_orders::producers_of(&game, &ItemId::from("research_data")),
        vec![node],
        "precondition: the machine that gathers it is deployed and findable"
    );

    let err = game
        .queue_work_order(WorkOrder::batch(ItemId::from("research_data"), 5))
        .expect_err("a banked item reaches no output, so no base can hold a stock of it");

    assert!(!err.is_empty());
    assert!(game.work_orders().is_empty());
}

#[test]
fn a_whole_line_correctly_laid_out_is_accepted() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);

    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    stand_in_base(&mut game);
    let (mine, lathe, press) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("core_fragment"), 5))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("core_fragment"), 5))
        .unwrap();

    assert!(game.cancel_work_order(7).is_err());
    assert_eq!(game.work_orders().len(), 1, "the queue is untouched");
}

#[test]
fn work_orders_round_trip_through_a_save() {
    let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("core_fragment"), 9))
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    let order = WorkOrder::batch(ItemId::from("routine_disk"), 3);

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
    let order = WorkOrder::batch(ItemId::from("routine_disk"), 3);

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
    place_home(&mut game);
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

    let order = WorkOrder::batch(ItemId::from("test_widget"), 1);
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
    place_home(&mut game);
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
        // Nothing assigns: `spawn_tamed` puts a program on the roster, and
        // an owned program outside the party is staff by derivation.
        staff.push(spawn_tamed(game, 10, 3));
    }
    staff.sort();
    staff
}

#[test]
fn one_staff_member_is_posted_to_the_top_of_the_line() {
    let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, press) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    stand_in_base(&mut game);
    let (mine, lathe, press) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
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
    stand_in_base(&mut game);
    let (mine, lathe, press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    let staff = hire(&mut game, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
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
    stand_in_base(&mut game);
    let (mine, lathe, press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    let staff = hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
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

/// **Every machine that makes the ordered item is a want, not the first
/// one.** A base that builds a second Mining Node because the first one's
/// output is being eaten by the assembler beside it has doubled its own
/// capacity, and the order has to see the machine it just paid for.
#[test]
fn a_second_machine_making_the_ordered_item_is_staffed_too() {
    let mut game = Game::new(69, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let first = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let second = spawn_machine_at(&mut game, "mining_node", 2, 2);
    let staff = hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 60))
        .unwrap();

    game.tick();

    let mut posts: Vec<Entity> = staff.iter().filter_map(|&s| posted_at(&game, s)).collect();
    posts.sort();
    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(
        posts, expected,
        "both Mining Nodes make Core Fragments, so both want a body"
    );
}

/// The report is built by calling `wants`, so the screen has to name the
/// second machine as well — a player who cannot see the node they just
/// deployed on the order's line has no way to tell it was picked up.
#[test]
fn the_report_names_every_machine_making_the_ordered_item() {
    let mut game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let first = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let second = spawn_machine_at(&mut game, "mining_node", 2, 2);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 60))
        .unwrap();

    let report = game.work_order_report();

    let mut listed: Vec<Entity> = report[0].machines.iter().map(|m| m.entity).collect();
    listed.sort();
    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(listed, expected);
}

/// `producers_of` returning several is also what stops a *whole* line being
/// refused on the strength of an unfed twin. Two Disk Presses, only one of
/// them beside a Lathe: the base can make Routine Disks, so the order
/// stands.
#[test]
fn a_second_unfed_bench_does_not_refuse_a_line_that_is_whole() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (_mine, _lathe, press) = lay_disk_line(&mut game);
    // Lower in `(x, y)` order than the fed Press, so an arbitrary
    // first-producer pick lands on the one with nothing beside it.
    let orphan = spawn_machine_at(&mut game, "disk_press", 0, 4);
    assert_eq!(
        crate::game::base::work_orders::producers_of(&game, &ItemId::from("routine_disk")),
        vec![orphan, press],
        "precondition: the unfed twin is the one an arbitrary pick would take"
    );

    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .expect("one whole line is enough for the order to stand");

    assert_eq!(game.work_orders().len(), 1);
}

/// An order is a **target level, not a production run** — three already in
/// a Depot means the base has three, and the order is done before anyone
/// is sent anywhere.
#[test]
fn an_order_the_base_already_holds_completes_without_staffing_anything() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (_mine, _lathe, _press) = lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 6, 0);
    put_output(&mut game, depot, "routine_disk", 5);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    stand_in_base(&mut game);
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 6, 0);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
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
    stand_in_base(&mut game);
    let (mine, _lathe, press) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 30))
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
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
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
    place_home(&mut game);
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
    stand_in_base(&mut game);
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let node = spawn_machine_at(&mut game, "research_node", 2, 3);
    let staff = hire(&mut game, 1);
    game.set_standing_job(node, true, false).unwrap();
    game.tick();
    assert_eq!(posted_at(&game, staff[0]), Some(node), "precondition");

    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);
    let staff = hire(&mut game, 2);
    let radius = game.world.resource::<crate::base_grid::BaseGrid>().radius();
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
///
/// Asked from **inside the base**, which is where an owned program's tile
/// is a tile: a `Tamed` `Position` is a base-space cell, so neither view
/// answers for one from the open grid (`Game::stands_in_base_space`, and
/// `a_program_standing_in_the_base_is_not_drawn_on_the_zone_surface` for
/// the other half of that).
#[test]
fn an_idle_staff_member_is_drawn_and_can_be_named_but_a_companion_is_neither() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    // `spawn_tamed_on_map` rather than `spawn_tamed`: `view_entities` draws
    // from `Glyph`, and a program without one is invisible for reasons that
    // have nothing to do with this rule.
    let idle = spawn_tamed_on_map(&mut game, 5, 5);
    let companion = spawn_tamed_on_map(&mut game, 6, 5);
    game.add_companion(companion).unwrap();
    game.tick();
    stand_in_base(&mut game);

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

    // Line the idle program up east of the party's own cell and confirm `x`
    // names it. `place_home` stands the Home on base space's origin, which
    // is where `stand_in_base` puts the party, so the ray leaves from the
    // Home's tile and the first thing on it is the program.
    let here = game.base_pos().expect("the party is in base space");
    let mut pos = game.world.get_mut::<Position>(idle).unwrap();
    pos.x = here.0 + 2;
    pos.y = here.1;
    assert!(
        matches!(
            game.find_target_in_direction(1, 0, 8),
            Some(InspectTarget::Creature(e)) if e == idle
        ),
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
        place_home(&mut game);
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

// ---------------------------------------------------------------------
// Task 7: the status report
// ---------------------------------------------------------------------

/// The report's machine list is the same list the scheduler acts on, in
/// the same order, for the same world — asserted against `wants` itself
/// rather than a hardcoded expectation, so the two cannot drift. Per
/// `CLAUDE.md`, a claim that two places use one rule has to be a call and
/// not a comment.
#[test]
fn the_report_lists_exactly_what_the_scheduler_walks() {
    let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, _press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();

    let report = game.work_order_report();
    let order = WorkOrder::batch(ItemId::from("routine_disk"), 3);
    let walked: Vec<Entity> = wants(&game, &order).into_iter().map(|(e, _)| e).collect();

    assert_eq!(report.len(), 1);
    let listed: Vec<Entity> = report[0].machines.iter().map(|m| m.entity).collect();
    assert_eq!(listed, walked);
    assert_ne!(report[0].state, views::OrderState::Stalled);
    assert_eq!(report[0].target, 3);
}

#[test]
fn the_report_counts_what_the_base_holds_against_the_target() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 6, 0);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 5))
        .unwrap();
    put_output(&mut game, depot, "routine_disk", 2);

    let report = game.work_order_report();

    assert_eq!((report[0].have, report[0].target), (2, 5));
}

#[test]
fn a_stalled_order_says_so_and_names_the_machine_that_went_missing() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (_mine, _lathe, press) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();

    game.world.entity_mut(press).despawn();
    let report = game.work_order_report();

    assert_eq!(report[0].state, views::OrderState::Stalled);
    assert!(
        report[0]
            .blocked_by
            .as_deref()
            .is_some_and(|why| why.contains("Disk Press")),
        "the screen has to say which machine went missing, got: {:?}",
        report[0].blocked_by
    );
}

/// A base with nobody in it and a base with a broken line are different
/// errands, and the screen must not conflate them: the first reports its
/// orders normally, and the fact that nothing is happening is answered by
/// the empty staff pool rather than by the order.
#[test]
fn a_base_with_no_staff_reports_its_orders_normally_rather_than_stalled() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();

    let report = game.work_order_report();

    assert!(game.base_staff().is_empty(), "precondition");
    assert_eq!(
        report[0].state,
        views::OrderState::Queued,
        "a base with nobody in it has its orders queued, not stalled"
    );
    assert!(report[0].blocked_by.is_none());
    assert!(!report[0].machines.is_empty());
}

#[test]
fn the_report_names_who_is_posted_on_each_machine() {
    let mut game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();
    game.tick();

    let report = game.work_order_report();
    let row = report[0]
        .machines
        .iter()
        .find(|m| m.entity == mine)
        .expect("the Mining Node is in the walk");

    assert!(
        row.worker.is_some(),
        "a machine with a body on it says whose"
    );
}

/// A finished order is the one piece of base news that is unambiguously
/// good, and the log's colour table is the only thing that says so — the
/// text reads the same as a filing or a cancellation. `MessageKind::Info`
/// draws it dim beside every routine payout line, which is where it was
/// getting lost.
#[test]
fn a_completed_order_is_announced_as_a_completion() {
    let mut game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    // Already satisfied when the scheduler next looks, so the order settles
    // on the first tick without anything having to be produced.
    game.world
        .get_mut::<Stock>(mine)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 5);

    game.tick();

    let line = game
        .message_log(30)
        .into_iter()
        .find(|l| l.text.starts_with("Work order complete"))
        .expect("the order should have settled");
    assert_eq!(
        line.kind,
        MessageKind::Complete,
        "a finished order needs its own kind — no existing one is both green \
         and about a job being done"
    );
    assert_eq!(
        line.source,
        MessageSource::Base,
        "and it stays base news, which is the axis that keeps it off the \
         battle pane"
    );
}

/// The scheduler frees a worker it no longer wants with
/// `.remove::<Task>().remove::<Carrying>()` — and a load has already been
/// taken *out* of its machine's stock by the time it exists, so dropping the
/// component destroys goods rather than releasing them.
///
/// Rare while a worker only ever set off from a clogged machine; routine now
/// that one sets off every cycle. The rule is the scheduler's own, one case
/// wider: it does not take a body off a post unless it has somewhere to put
/// it, and a body mid-delivery has somewhere to be.
#[test]
fn a_worker_mid_delivery_is_not_stood_down_with_its_load() {
    let mut game = Game::new(66, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "depot", 3, 0);
    // Somewhere else for the scheduler to want a body, so the pass that
    // frees one is actually reached: with every want already filled it
    // returns before touching anybody. Placed *after* the mine in `(x, y)`
    // order, since `wants` sorts by tile at equal depth and is truncated to
    // the one staff member — the order has to land on the machine beside
    // the depot.
    let spare = spawn_machine_at(&mut game, "mining_node", 3, 3);
    game.set_standing_job(spare, true, false).unwrap();
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 60))
        .unwrap();
    game.tick();

    let worker = game.base_staff()[0];
    assert_eq!(
        game.world.get::<components::Task>(worker).map(|t| t.target),
        Some(mine),
        "precondition: the order's want is what the body is on"
    );
    // Nothing consumes Core Fragments beside the mine, so the worker sets
    // off with the first cycle's payout of its own accord.
    for _ in 0..60 {
        if game.world.get::<Carrying>(worker).is_some() {
            break;
        }
        game.tick();
    }
    let load = game
        .world
        .get::<Carrying>(worker)
        .cloned()
        .expect("precondition: the worker has to be holding a load");

    // The player changes their mind mid-walk. The order's want disappears
    // and the standing job is all that is left to fill.
    game.cancel_work_order(0).unwrap();
    game.tick();

    let still = game
        .world
        .get::<Carrying>(worker)
        .expect("the load must not be deleted out from under a walking worker");
    assert_eq!(still.item, load.item);
    assert_eq!(still.qty, load.qty);
}

/// **Look in the depot before making it by hand.** `wants` is sorted
/// deepest-first, so a feeder outranks the bench it feeds and a single body
/// goes upstream to make more of something the base already has in store.
/// With a batch on the shelf the feeder stops being wanted at all, and the
/// bench — which can now run — takes the body.
#[test]
fn a_bench_fed_from_the_depot_does_not_staff_its_feeder() {
    let mut game = Game::new(67, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, press) = lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 3, 1);
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from("blank_substrate"), 10);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    game.tick();

    let worker = game.base_staff()[0];
    assert_eq!(
        game.world.get::<components::Task>(worker).map(|t| t.target),
        Some(press),
        "the body belongs on the machine that can run off store, not \
         upstream making more"
    );
    let walked: Vec<Entity> = game
        .work_order_report()
        .remove(0)
        .machines
        .into_iter()
        .map(|m| m.entity)
        .collect();
    assert!(
        !walked.contains(&lathe) && !walked.contains(&mine),
        "and nothing upstream of it is wanted at all: {walked:?}"
    );
}

/// The other half of "while stock lasts". A shelf too thin for a batch is
/// no answer, and neither is an empty one — the line has to come back.
#[test]
fn the_feeder_is_wanted_again_once_the_shelf_will_not_cover_a_batch() {
    let mut game = Game::new(68, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 3, 1);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    // One on the shelf against a Disk Press batch of two. Deliberately not
    // an *empty* shelf: a `> 0` skip would pass that and still strand the
    // order here, having taken the feeder off the list on the strength of
    // stock that cannot run a single cycle — leaving nobody working
    // anything.
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from("blank_substrate"), 1);
    game.tick();

    let walked: Vec<Entity> = game
        .work_order_report()
        .remove(0)
        .machines
        .into_iter()
        .map(|m| m.entity)
        .collect();
    assert!(
        walked.contains(&mine),
        "a shelf too thin for a batch is no answer — the base has to make \
         its own: {walked:?}"
    );
}

// ---------------------------------------------------------------------
// The depot route: a machine fed by haulage rather than by a neighbour
// ---------------------------------------------------------------------

/// The layout a real save turned up: three machines in a row, spaced two
/// tiles apart so none of them touches, with a Depot on the slab.
///
/// Mining Node (2,0), Lathe (0,0), Depot (-2,0). Nothing is orthogonally
/// adjacent to anything, which is the whole point — the only path from the
/// fragments to the Lathe is a worker walking to the Depot, which is exactly
/// the path `Errand::Collect` already takes and `batch_within_reach` already
/// counts through `depot_holding`.
fn lay_depot_route(game: &mut Game) -> (Entity, Entity, Entity) {
    place_home(&mut *game);
    let mine = spawn_machine_at(game, "mining_node", 2, 0);
    let lathe = spawn_machine_at(game, "lathe", 0, 0);
    let depot = spawn_machine_at(game, "depot", -2, 0);
    (mine, lathe, depot)
}

fn stock_output(game: &mut Game, structure: Entity, item: &str, qty: u32) {
    game.world
        .get_mut::<Stock>(structure)
        .unwrap()
        .output
        .insert(ItemId::from(item), qty);
}

/// The reproducer. `can_progress` already says the Lathe can run off the
/// Depot's fragments, so a picker that hides the item is refusing an order
/// the base would have filled.
#[test]
fn a_machine_fed_from_a_depot_is_orderable_without_a_neighbour() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (_mine, lathe, depot) = lay_depot_route(&mut game);
    stock_output(&mut game, depot, ids::CORE_FRAGMENT, 12);

    assert!(
        crate::game::base::work_orders::can_progress(&game, lathe),
        "precondition: the runtime already reaches the depot's fragments"
    );

    game.queue_work_order(WorkOrder::batch(ItemId::from("blank_substrate"), 3))
        .expect("a lathe a hauler can feed from the depot must be orderable");

    assert!(
        game.orderable_items()
            .iter()
            .any(|(id, _)| id.as_str() == "blank_substrate"),
        "and the picker must list what the queue accepts"
    );
}

/// The other half: the order has to keep moving once the Depot runs dry.
/// `walk_feeders` skips a feeder while the shelf holds a batch — the shelf
/// comes before the bench — so the case that stalls is a *thin* shelf with
/// no neighbour to fall back on. The walk has to reach the producer behind
/// the depot, or nobody is ever posted and the order sits forever.
#[test]
fn the_producer_behind_a_depot_route_is_staffed_when_the_shelf_runs_thin() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, _lathe, _depot) = lay_depot_route(&mut game);

    let wanted = crate::game::base::work_orders::wants(
        &game,
        &WorkOrder::batch(ItemId::from("blank_substrate"), 3),
    );
    let posts: Vec<Entity> = wanted.into_iter().map(|(e, _)| e).collect();

    assert!(
        posts.contains(&mine),
        "an empty shelf must send a body to the mining node behind it: {posts:?}"
    );
}

/// And the shelf-before-bench rule survives the new reach: with a batch
/// already in store there is nothing for the upstream to make.
#[test]
fn a_stocked_shelf_still_keeps_the_body_off_the_producer_behind_it() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mine, lathe, depot) = lay_depot_route(&mut game);
    stock_output(&mut game, depot, ids::CORE_FRAGMENT, 12);

    let wanted = crate::game::base::work_orders::wants(
        &game,
        &WorkOrder::batch(ItemId::from("blank_substrate"), 3),
    );
    let posts: Vec<Entity> = wanted.into_iter().map(|(e, _)| e).collect();

    assert!(
        posts.contains(&lathe),
        "the lathe has fragments to work: {posts:?}"
    );
    assert!(
        !posts.contains(&mine),
        "but the shelf comes before the bench: {posts:?}"
    );
}

/// The refusal that must survive: with no Depot standing there is no route
/// at all, and the sentence still has to name the missing link.
#[test]
fn a_depot_less_base_still_refuses_a_machine_with_no_neighbour() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 0, 0);

    let err = game
        .queue_work_order(WorkOrder::batch(ItemId::from("blank_substrate"), 3))
        .expect_err("no neighbour and no depot is no route");

    assert!(
        err.contains("Core Fragment"),
        "the refusal must still name the missing link, got: {err}"
    );
}

/// **A posting does not move the body that takes it.** A program already
/// standing in the base was teleported onto the player's tile the instant
/// the scheduler gave it a job, and walked in from wherever the player
/// happened to be. Idle staff loiter on a real tile now, so the walk starts
/// from that tile.
///
/// The bound is two tiles rather than zero because a tick does two things
/// to an idle body before this can be read: `park_idle_staff` may step it
/// one along its ring, and `haul_step_system` takes the first step of the
/// walk in the same tick.
///
/// `stand_player_at` is a decoy here, not a distance: the base is its own
/// coordinate space now, so the player's tile has nothing to do with where
/// a worker's walk field is. It exists to give a regression that reads the
/// player's `Position` instead of the worker's own something to be caught
/// jumping to — a value nowhere near the ring `park_idle_staff` parks
/// bodies on around Home.
#[test]
fn a_posted_staff_member_sets_off_from_its_own_tile_and_not_the_player_s() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, _, _) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    // Nothing to do for a few ticks, so the program is standing on a
    // parking tile of its own rather than wherever it was spawned.
    for _ in 0..3 {
        game.tick();
    }
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();
    // The decoy the doc comment above explains.
    stand_player_at(&mut game, 5, 5);
    let before = *game.world.get::<Position>(staff[0]).unwrap();

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "precondition: the body took the posting"
    );
    let pos = *game.world.get::<Position>(staff[0]).unwrap();
    assert!(
        (pos.x - before.x).abs().max((pos.y - before.y).abs()) <= 2,
        "a posting moves a body one step, not across the map: {before:?} -> {pos:?}"
    );
}

/// The other half of the same root cause: `post_reach` was asked from the
/// player's tile too, so walking away from your own base stopped the
/// scheduler filling a single machine — the pool stood idle beside the
/// order it was hired to work.
///
/// "Far from the base" no longer fits inside one coordinate space to be
/// measured in tiles — the honest analogue is the party not being in base
/// space at all: back on the surface, wherever the anchor happens to sit,
/// while the base keeps running unattended. `stand_player_at` still sets a
/// decoy well outside where the base's own tiles live, so a regression that
/// reads the player's `Position` as a body's walk origin fails this the
/// same way it would fail the sibling test above.
#[test]
fn the_scheduler_still_posts_staff_while_the_player_is_far_from_the_base() {
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, _, _) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    for _ in 0..3 {
        game.tick();
    }
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();
    // Not just far within the base — out of base space entirely, back on
    // the surface at the decoy tile the sibling test above explains.
    game.world.insert_resource(Locale::Surface);
    stand_player_at(&mut game, 40, 40);

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "the base runs itself whether or not you are stood in it"
    );
}

// ---------------------------------------------------------------------
// Work order queue, phase 1: orders worked concurrently
// ---------------------------------------------------------------------

/// A second, independent line: Log Scraper (2,2) → Transcriber (3,2),
/// which makes Logic Wafers and shares no machine with `lay_disk_line`.
fn lay_wafer_line(game: &mut Game) -> (Entity, Entity) {
    let scraper = spawn_machine_at(game, "log_scraper", 2, 2);
    let transcriber = spawn_machine_at(game, "transcriber", 3, 2);
    (scraper, transcriber)
}

/// Every posted body, whoever they belong to, standing at `machine`.
fn bodies_at(game: &mut Game, machine: Entity) -> usize {
    let mut query = game.world.query::<&Task>();
    query
        .iter(&game.world)
        .filter(|t| t.target == machine)
        .count()
}

/// The queue is a production policy, not a to-do list: a base with more
/// staff than the front order can use works the one behind it too rather
/// than parking the spare bodies.
#[test]
fn spare_staff_are_put_on_the_second_order() {
    let mut game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let (scraper, _transcriber) = lay_wafer_line(&mut game);
    let staff = hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("logic_wafer"), 3))
        .unwrap();

    // **The preconditions are the test.** Without them this passes against
    // the serial scheduler on any tick where the front order happened to be
    // satisfied — the spare body would reach the second order by the front
    // one being popped, not by the two being worked together.
    let front = game.work_orders()[0].clone();
    assert!(
        base_holding(&game, &front.item) < front.qty,
        "the front order has to still be unsatisfied for this to mean anything"
    );
    let front_wants = wants(&game, &front);
    assert!(
        front_wants.len() < staff.len(),
        "and it has to want fewer machines than the base has bodies"
    );

    game.tick();

    let posts: Vec<Option<Entity>> = staff.iter().map(|&s| posted_at(&game, s)).collect();
    assert!(
        posts.contains(&Some(mine)),
        "the front order still gets its body first"
    );
    assert!(
        posts.contains(&Some(scraper)),
        "and the spare body works the order behind it rather than parking"
    );
}

/// A machine both orders need is one post, not two.
///
/// Counted twice, the duplicate eats a staff slot against a job that is
/// already filled — `post_worker` displaces the body already standing
/// there, so the base ends up with an idle program and the want the
/// truncation dropped for it, here the second order's own bench, unstaffed.
/// The body count at the shared feeder stays 1 either way, which is why the
/// assertion that discriminates is the one below it.
#[test]
fn a_machine_two_orders_want_is_posted_once() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, _press) = lay_disk_line(&mut game);
    let annealer = spawn_machine_at(&mut game, "annealing_node", 2, 1);
    // Both lines run off the Mining Node's output, so both orders reach it.
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    let staff = hire(&mut game, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("annealed_core"), 30))
        .unwrap();
    let orders = game.work_orders().to_vec();
    assert!(
        wants(&game, &orders[0]).iter().any(|&(e, _)| e == mine)
            && wants(&game, &orders[1]).iter().any(|&(e, _)| e == mine),
        "precondition: the Mining Node is a want of both orders"
    );

    game.tick();

    assert_eq!(
        bodies_at(&mut game, mine),
        1,
        "one machine is one post however many orders want it"
    );
    let posts: Vec<Option<Entity>> = staff.iter().map(|&s| posted_at(&game, s)).collect();
    assert!(
        posts.iter().all(|p| p.is_some()),
        "a duplicated want left a body with nowhere to stand"
    );
    assert!(
        posts.contains(&Some(lathe)) && posts.contains(&Some(annealer)),
        "and the slot the duplicate would have eaten still reaches the second order's bench"
    );
}

/// Concurrency is not fairness. The accumulated list is in queue order and
/// `truncate(staff.len())` cuts from the end, so a scarce body goes to the
/// front order and the one behind it waits.
#[test]
fn the_front_order_still_fills_first_when_staff_are_scarce() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, _press) = lay_disk_line(&mut game);
    let (scraper, transcriber) = lay_wafer_line(&mut game);
    // Stocked, so the front order wants two machines and can use both
    // bodies by itself.
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    let staff = hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("logic_wafer"), 30))
        .unwrap();
    let front = game.work_orders()[0].clone();
    assert_eq!(
        wants(&game, &front).len(),
        staff.len(),
        "precondition: the front order wants exactly what the base has"
    );

    game.tick();

    let mut posts: Vec<Entity> = staff.iter().filter_map(|&s| posted_at(&game, s)).collect();
    posts.sort();
    let mut expected = vec![mine, lathe];
    expected.sort();
    assert_eq!(posts, expected, "both bodies belong to the front order");
    assert_eq!(bodies_at(&mut game, scraper), 0);
    assert_eq!(bodies_at(&mut game, transcriber), 0);
}

/// Orders now occupy more of the list, so the append sites below them are
/// worth re-asserting: a standing job and a dig site are still filled by a
/// body no order needs, and never before one.
#[test]
fn standing_jobs_and_digs_still_come_after_every_order() {
    let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, _lathe, _press) = lay_disk_line(&mut game);
    let node = spawn_machine_at(&mut game, "research_node", 2, 3);
    game.set_standing_job(node, true, false).unwrap();
    let wall = (crate::tuning::STARTING_POCKET_RADIUS + 1, 0);
    game.toggle_mark_box(wall, wall);
    let site = game
        .dig_site_at(wall.0, wall.1)
        .expect("a marked wall has a dig site");
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "the one body works the order"
    );
    assert_eq!(bodies_at(&mut game, node), 0, "the standing job waits");
    assert_eq!(bodies_at(&mut game, site), 0, "and so does the dig site");
}

// ---------------------------------------------------------------------
// Work order queue, phase 2: standing orders
// ---------------------------------------------------------------------

/// An order is a target level, and a level that deletes itself the moment
/// it is reached is not a level. A standing order sleeps where a one-shot
/// one is completed and removed.
#[test]
fn a_satisfied_standing_order_stays_in_the_queue() {
    let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 5);
    // **The precondition is the test.** Without it this passes against an
    // order that was never satisfied at all, which is every order on a base
    // with nothing on its shelves.
    assert!(
        base_holding(&game, &ItemId::from(ids::CORE_FRAGMENT)) >= 5,
        "precondition: the shelf has to actually hold what was asked for"
    );

    game.tick();

    assert_eq!(
        game.work_orders().len(),
        1,
        "a standing order is a level the base holds, not a batch it makes once"
    );
    assert!(
        game.message_log(30)
            .iter()
            .all(|l| !l.text.starts_with("Work order complete")),
        "and 'complete' is a lie about something that is not complete"
    );
}

/// Skipped, not returned — the one correctness point in the phase.
///
/// A dormant standing order contributes no wants, and returning those
/// straight out of `settle_orders` would starve every order behind it for
/// as long as the shelf stayed full.
#[test]
fn an_order_below_a_satisfied_standing_order_is_worked() {
    let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    let (scraper, _transcriber) = lay_wafer_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 5);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("logic_wafer"), 3))
        .unwrap();

    game.tick();

    assert_eq!(
        game.work_orders().len(),
        2,
        "the dormant standing order is still holding its place in the queue"
    );
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(scraper),
        "and the order behind it is worked rather than starved"
    );
}

/// No hysteresis, and none needed: `collect_adjacent` empties the whole
/// output buffer, so the drain is a burst rather than a trickle and there
/// is nothing for a re-arm threshold to oscillate around.
#[test]
fn a_standing_order_re_arms_after_the_shelf_drains() {
    let mut game = Game::new(76, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 5);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();

    game.tick();
    assert_eq!(
        bodies_at(&mut game, mine),
        0,
        "precondition: a satisfied standing order asks for nobody"
    );

    // Beside the machine rather than on the exit cell: collecting reads the
    // party's *base* cell and takes from its orthogonal neighbours.
    game.world.insert_resource(Locale::Base { x: 1, y: 0 });
    assert!(
        !game.collect_adjacent().is_empty(),
        "precondition: the shelf actually drained"
    );
    game.tick();

    assert_eq!(
        bodies_at(&mut game, mine),
        1,
        "a standing order wakes when the level it holds is broken"
    );
}

/// The other half: a one-shot order is still a batch, still announces
/// itself finished, and still leaves the queue.
#[test]
fn a_one_shot_order_still_completes_and_is_removed() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 5);

    game.tick();

    assert!(
        game.work_orders().is_empty(),
        "a batch that has been made is done with"
    );
    assert!(
        game.message_log(30)
            .iter()
            .any(|l| l.text.starts_with("Work order complete")),
        "and says so"
    );
}

/// A save written before the field existed loads every order as one-shot,
/// which is the whole of the compatibility story for a `#[serde(default)]`
/// field on a file-named RON save.
///
/// The field is *stripped from the file* rather than round-tripped: a plain
/// round trip asserts the field survives, which is a different claim and
/// passes against a default nobody ever exercises. The strip is asserted to
/// have applied, or a rename would quietly turn this into a round trip.
#[test]
fn an_order_saved_before_standing_orders_loads_as_one_shot() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();

    let path = save_path("standing_default");
    game.save(&path).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    let older = written.replace("standing: true,", "");
    assert_ne!(
        older, written,
        "the field has to be in the file for removing it to mean anything"
    );
    std::fs::write(&path, &older).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let orders = loaded.work_orders();
    assert_eq!(orders.len(), 1, "the order itself still loads");
    assert!(
        !orders[0].standing,
        "and an order filed before standing ones existed is a batch"
    );
}

// ---------------------------------------------------------------------
// Work order queue, phase 3: priority bands
// ---------------------------------------------------------------------

/// What the queue is holding, in the order the scheduler will walk it.
fn queued_items(game: &Game) -> Vec<String> {
    game.work_orders()
        .iter()
        .map(|o| o.item.to_string())
        .collect()
}

/// Priority is an **insert position**, not a second sort — so a High order
/// is above a Normal one in the Vec itself, which is the only ordering
/// `settle_orders`, `cancel_work_order` and the screen all read.
#[test]
fn a_high_order_files_above_a_normal_one() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    game.queue_work_order(
        WorkOrder::batch(ItemId::from("routine_disk"), 3).with_priority(OrderPriority::High),
    )
    .unwrap();

    assert_eq!(
        queued_items(&game),
        vec!["routine_disk".to_string(), ids::CORE_FRAGMENT.to_string()],
        "the High order jumps the Normal one already standing"
    );
}

/// Ties break by insertion order, which is what inserting *after* the last
/// order of equal priority buys rather than before the first.
#[test]
fn two_orders_of_one_band_keep_their_insertion_order() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    for item in [ids::CORE_FRAGMENT, "blank_substrate", "routine_disk"] {
        game.queue_work_order(WorkOrder::batch(ItemId::from(item), 3))
            .unwrap();
    }

    assert_eq!(
        queued_items(&game),
        vec![
            ids::CORE_FRAGMENT.to_string(),
            "blank_substrate".to_string(),
            "routine_disk".to_string()
        ],
        "one band is still a queue"
    );
}

/// The other end of the same rule.
#[test]
fn a_low_order_files_below_everything() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(
        WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5).with_priority(OrderPriority::Low),
    )
    .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();

    assert_eq!(
        queued_items(&game),
        vec!["routine_disk".to_string(), ids::CORE_FRAGMENT.to_string()],
        "a Normal order filed later still outranks a Low one filed first"
    );
}

/// `cancel_work_order` takes a raw Vec index and the screen indexes
/// straight into `work_order_report`, so the two must keep naming the same
/// row after a band has inserted one mid-queue. This is the whole reason
/// the band is an insert position rather than a sort at scheduling time.
#[test]
fn cancelling_still_drops_the_row_the_screen_names() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("blank_substrate"), 3))
        .unwrap();
    game.queue_work_order(
        WorkOrder::batch(ItemId::from("routine_disk"), 3).with_priority(OrderPriority::High),
    )
    .unwrap();

    let report = game.work_order_report();
    let second = report[1].item.clone();
    game.cancel_work_order(1).unwrap();

    assert!(
        !game.work_orders().iter().any(|o| o.item == second),
        "the index the screen showed named the row it dropped"
    );
    assert_eq!(
        queued_items(&game),
        vec!["routine_disk".to_string(), "blank_substrate".to_string()],
        "and nothing else moved"
    );
}

/// A save written before the field existed loads every order as Normal.
/// Stripped from the file rather than round-tripped, for the reason the
/// standing flag's twin test gives.
#[test]
fn an_order_saved_before_priority_loads_as_normal() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(
        WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5).with_priority(OrderPriority::High),
    )
    .unwrap();

    let path = save_path("priority_default");
    game.save(&path).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    let older = written.replace("priority: High,", "");
    assert_ne!(
        older, written,
        "the field has to be in the file for removing it to mean anything"
    );
    std::fs::write(&path, &older).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let orders = loaded.work_orders();
    assert_eq!(orders.len(), 1, "the order itself still loads");
    assert_eq!(
        orders[0].priority,
        OrderPriority::Normal,
        "an order filed before bands existed is an ordinary one"
    );
}

// ---------------------------------------------------------------------
// Work order queue, phase 4: four states on the screen
// ---------------------------------------------------------------------

/// The state of the order at `index`, which is what the queue screen puts
/// beside the row.
fn state_of(game: &Game, index: usize) -> views::OrderState {
    game.work_order_report()[index].state
}

/// An order with a body on its chain is the one the base is actually
/// spending itself on, and the screen's whole job in this phase is to say
/// which one that is.
#[test]
fn an_order_the_base_is_staffing_reports_working() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    let staff = hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "precondition: the base actually put somebody on this order"
    );
    assert_eq!(state_of(&game, 0), views::OrderState::Working);
}

/// The order behind it, with the body already spent. Not stalled — its
/// line is whole and it wants machines; there is simply nobody left to
/// stand on them, which is a different errand for the player and now says
/// so.
#[test]
fn an_order_with_no_body_left_for_it_reports_queued() {
    let mut game = Game::new(85, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    lay_wafer_line(&mut game);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("logic_wafer"), 30))
        .unwrap();

    game.tick();

    assert_eq!(
        state_of(&game, 0),
        views::OrderState::Working,
        "precondition: the one body went to the front order"
    );
    assert_eq!(state_of(&game, 1), views::OrderState::Queued);
}

/// A standing order at its level, which is the normal healthy state of a
/// standing order rather than a fault.
#[test]
fn a_standing_order_at_its_level_reports_dormant() {
    let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 5);

    game.tick();

    assert_eq!(state_of(&game, 0), views::OrderState::Dormant);
}

/// A line that broke after the order was placed.
#[test]
fn an_order_whose_chain_broke_reports_stalled() {
    let mut game = Game::new(87, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (_mine, _lathe, press) = lay_disk_line(&mut game);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    game.world.entity_mut(press).despawn();
    game.tick();

    assert_eq!(state_of(&game, 0), views::OrderState::Stalled);
}

/// **The regression the enum exists to prevent.** A dormant standing order
/// and a stalled one are indistinguishable from the outside: both ask for
/// nobody, both sit in the queue doing nothing. One is the feature working
/// and the other is a base that needs rebuilding, and a screen that words
/// them the same sends the player hunting for a machine that never went
/// missing.
#[test]
fn a_dormant_standing_order_is_not_reported_stalled() {
    let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, ..) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 5))
        .unwrap();
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 5);

    game.tick();

    assert_eq!(
        bodies_at(&mut game, mine),
        0,
        "precondition: it asks for nobody, exactly as a stalled order does"
    );
    let report = game.work_order_report();
    assert_eq!(report[0].state, views::OrderState::Dormant);
    assert!(
        report[0].blocked_by.is_none(),
        "and nothing went missing, so there is nothing to name"
    );
}

// ---------------------------------------------------------------------
// Work order queue, phase 5: announce a stall once
// ---------------------------------------------------------------------

/// Every stall the log has announced, oldest first.
///
/// Read off `Game::message_log` rather than the pane, because `condense`
/// folds adjacent repeats into a `×N` row — a per-tick announcement would
/// draw as one line there and this is the test that has to see it.
fn stall_lines(game: &Game) -> Vec<String> {
    game.message_log(MESSAGE_LOG_CAP)
        .into_iter()
        .filter(|l| l.text.starts_with("Work order stalled"))
        .map(|l| l.text)
        .collect()
}

/// A base whose Disk Press has been swept to destruction, with the order
/// that needed it still queued and already announced once.
fn stall_a_disk_order(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (_mine, _lathe, press) = lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();
    game.world.entity_mut(press).despawn();
    game.tick();
    game
}

/// `set_machine_status`' rule one subsystem over: entering a state is news,
/// staying in it is not. A stalled order is skipped by `settle_orders` on
/// every tick for the rest of the run, so an unlatched announcement is the
/// same sentence forever.
#[test]
fn a_stall_is_announced_once_rather_than_every_tick() {
    let mut game = stall_a_disk_order(89);
    game.tick();
    game.tick();

    assert_eq!(
        state_of(&game, 0),
        views::OrderState::Stalled,
        "precondition: the order really is stalled, not merely unstaffed"
    );
    let lines = stall_lines(&game);
    assert_eq!(
        lines.len(),
        1,
        "one announcement, not one a tick: {lines:?}"
    );
    assert!(
        lines[0].contains("30 x Routine Disk"),
        "and it names the order that stopped, got: {}",
        lines[0]
    );
}

/// The half of `DigSite::announced_stuck` that gets forgotten. Without the
/// clear, the second break — and every one after it — is silent for the
/// rest of the run, which is worse than announcing every tick: the player
/// is told once about a machine they then rebuild, and never again about
/// the one they knock down next.
#[test]
fn a_stall_that_resolves_and_recurs_is_announced_again() {
    let mut game = stall_a_disk_order(90);
    assert_eq!(
        stall_lines(&game).len(),
        1,
        "precondition: the first break was announced"
    );

    let rebuilt = spawn_machine_at(&mut game, "disk_press", 4, 0);
    game.tick();
    assert_ne!(
        state_of(&game, 0),
        views::OrderState::Stalled,
        "precondition: rebuilding the press put the order back to work"
    );

    game.world.entity_mut(rebuilt).despawn();
    game.tick();

    let lines = stall_lines(&game);
    assert_eq!(lines.len(), 2, "a second break is news again: {lines:?}");
}

/// **`#[serde(skip)]`, not a default.** The run that was told the line broke
/// is over; a player loading back in has no reason to remember it, and the
/// order will otherwise sit stalled forever without ever saying so.
///
/// Its own save-then-load assertion rather than the RON round trip, which
/// cannot see this field at all — a round trip alone passes just as well
/// against a latch that was never skipped.
#[test]
fn a_reloaded_stall_is_announced_again() {
    let mut game = stall_a_disk_order(91);
    assert_eq!(
        stall_lines(&game).len(),
        1,
        "precondition: this run has already been told"
    );

    let path = save_path("stall_latch");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        state_of(&loaded, 0),
        views::OrderState::Stalled,
        "precondition: the break survived the save, so there is something to say"
    );
    assert!(
        stall_lines(&loaded).is_empty(),
        "precondition: the log itself is not saved"
    );

    loaded.tick();

    assert_eq!(
        stall_lines(&loaded).len(),
        1,
        "a reloaded run is told about the break it walked back into"
    );
}

// ---------------------------------------------------------------------
// Work order queue, phase 6: how short of bodies the base is
// ---------------------------------------------------------------------

/// What the scheduler asked for last tick against who it had to give.
fn demand(game: &Game) -> resources::LabourDemand {
    game.labour_demand()
}

/// The healthy case, and the reason the screen says nothing in it: with a
/// body for every post there is no shortfall to report, and a header that
/// always shows is a header nobody reads.
#[test]
fn a_base_with_bodies_to_spare_is_short_of_none() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, _press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    hire(&mut game, 4);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    game.tick();

    let demand = demand(&game);
    assert_eq!(
        demand.wanted, 3,
        "the whole line is running and wants a body"
    );
    assert_eq!(demand.staff, 4);
    assert_eq!(demand.shortfall(), 0);
}

/// The case the header exists for. `schedule_base_labour` cuts its want
/// list to `staff.len()` and the posts that fall off the end vanish
/// silently — the screen says "no one" per machine but never says how many
/// bodies short the base actually is.
#[test]
fn a_base_short_of_bodies_reports_the_difference() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let (mine, lathe, _press) = lay_disk_line(&mut game);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, "blank_substrate", 6);
    hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    game.tick();

    let demand = demand(&game);
    assert_eq!(
        demand.wanted, 3,
        "all three machines want a body — the figure is what was asked for, \
         not what was filled"
    );
    assert_eq!(demand.staff, 2);
    assert_eq!(demand.shortfall(), 1, "the Disk Press goes unstaffed");
}

/// The quiet state a player is most likely to have the screen open on, and
/// the one `schedule_base_labour` early-returns out of before it ever
/// reaches the cut: an empty roster reports the queue's wants against zero
/// rather than reading as no wants at all.
#[test]
fn a_base_with_nobody_in_it_reports_its_wants_against_zero() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    lay_disk_line(&mut game);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3))
        .unwrap();

    game.tick();

    assert!(
        game.base_staff().is_empty(),
        "precondition: nobody is on the roster"
    );
    let demand = demand(&game);
    assert_eq!(
        demand.wanted, 1,
        "the top of the line wants a body on an empty base"
    );
    assert_eq!(demand.staff, 0);
    assert_eq!(demand.shortfall(), 1);
}
