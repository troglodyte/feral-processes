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
