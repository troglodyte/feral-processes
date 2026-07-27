//! Routines: the slots abilities occupy, and how they get there.

use super::support::*;
use crate::components::Routines;
use crate::*;

/// The generic test species declares no abilities, so its kit is the
/// fallback — which must be a real installed routine, not an empty list
/// resolved at read time.
#[test]
fn a_tamed_program_with_no_species_kit_starts_with_the_fallback_installed() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    let installed = game
        .world
        .get::<Routines>(pet)
        .expect("a tamed program has routines");
    assert_eq!(
        installed.0,
        vec![crate::abilities::FALLBACK_ABILITY_ID.to_string()],
        "a species declaring no abilities implicitly installs the fallback"
    );
}

#[test]
fn a_species_kit_is_installed_at_tame_time_in_declared_order() {
    let (game, medic) = game_with_two_ability_companion();
    let installed = &game.world.get::<Routines>(medic).unwrap().0;
    assert_eq!(
        installed.len(),
        1,
        "only the level-1 unlock is installed on a level-1 program: {installed:?}"
    );
}

#[test]
fn a_level_up_that_reaches_an_unlock_installs_it_into_a_free_slot() {
    let (mut game, medic) = game_with_two_ability_companion();
    let before = game.world.get::<Routines>(medic).unwrap().0.len();
    // `TWO_ABILITY_SPECIES` gates `sandbox` at level 5, and level 5 is worth
    // two slots, so the unlock has somewhere to land.
    set_level(&mut game, medic, 5);
    let after = &game.world.get::<Routines>(medic).unwrap().0;
    assert_eq!(
        after.len(),
        before + 1,
        "reaching the second unlock should install it: {after:?}"
    );
}

#[test]
fn slot_count_follows_the_tuning_curve_for_both_sides() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    for level in 1..=12u32 {
        set_level(&mut game, pet, level);
        assert_eq!(
            game.routine_slots(pet),
            crate::abilities::companion_routine_slots(level),
            "companion slots at level {level}"
        );
    }
    for level in [1u32, 9, 10, 25, 50] {
        set_level(&mut game, player, level);
        assert_eq!(
            game.routine_slots(player),
            crate::abilities::player_routine_slots(level),
            "player slots at level {level}"
        );
    }
}

#[test]
fn installed_routines_survive_a_save_load_round_trip() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    let before = game.world.get::<Routines>(pet).unwrap().0.clone();
    let path = std::env::temp_dir().join(format!(
        "feral_routines_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .owned_pets()
        .first()
        .map(|p| loaded.world.get::<Routines>(p.entity).unwrap().0.clone())
        .expect("the tamed program should come back");
    assert_eq!(restored, before, "a save must carry installed routines");
}
