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
