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

#[test]
fn every_loaded_ability_gets_a_routine_item_carrying_its_own_text() {
    let game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for ability in game.ability_defs() {
        let item = crate::abilities::routine_item_id(&ability.id);
        let def = game
            .item_def(&item)
            .unwrap_or_else(|| panic!("{} should have a synthesized routine item", ability.id));
        assert_eq!(def.routine.as_deref(), Some(ability.id.as_str()));
        assert_eq!(
            def.description, ability.description,
            "a routine item reads its text from the ability, never a copy"
        );
        assert!(def.name.ends_with(" Routine"), "{}", def.name);
    }
}

#[test]
fn install_then_uninstall_returns_the_same_item() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 4); // two slots, one of them free
    let item = crate::abilities::routine_item_id("sandbox");
    set_inventory(&mut game, &[(item.as_str(), 1)]);

    game.install_routine(pet, &item).unwrap();
    assert_eq!(
        count_item(&game, item.as_str()),
        0,
        "installing spends the item"
    );
    assert!(
        game.routine_view(pet)
            .iter()
            .any(|s| s.ability.as_deref() == Some("sandbox")),
        "the routine should occupy a slot"
    );

    let slot = game
        .routine_view(pet)
        .into_iter()
        .position(|s| s.ability.as_deref() == Some("sandbox"))
        .unwrap();
    game.uninstall_routine(pet, slot).unwrap();
    assert_eq!(
        count_item(&game, item.as_str()),
        1,
        "uninstalling gives it back"
    );
}

#[test]
fn install_is_refused_with_no_free_slot_without_the_item_and_during_battle() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3); // level 1: exactly one slot, already full
    let item = crate::abilities::routine_item_id("sandbox");

    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("don't have"), "no copy held: {err}");

    set_inventory(&mut game, &[(item.as_str(), 1)]);
    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("no free routine slot"), "slots full: {err}");

    set_level(&mut game, pet, 4);
    start_battle_with_a_wild_program(&mut game);
    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("right now"), "mid-battle: {err}");
}

#[test]
fn an_innate_routine_can_be_popped_out_and_plugged_into_another_program() {
    let (mut game, medic) = game_with_two_ability_companion();
    let popped = game.routine_view(medic)[0].ability.clone().unwrap();
    game.uninstall_routine(medic, 0).unwrap();
    assert!(
        game.routine_view(medic).iter().all(|s| s.ability.is_none()),
        "the innate slot should now be empty"
    );

    let host = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, host, 4);
    let item = crate::abilities::routine_item_id(&popped);
    game.install_routine(host, &item).unwrap();
    assert!(
        game.routine_view(host)
            .iter()
            .any(|s| s.ability.as_deref() == Some(popped.as_str())),
        "a foreign species' routine should install fine"
    );
}

#[test]
fn extraction_needs_a_bench_built_somewhere_but_not_nearby() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.can_extract_routines(), "no bench, no extraction");

    let pet = spawn_tamed(&mut game, 10, 3);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(
        err.contains("Compiler"),
        "the refusal should name the bench: {err}"
    );

    spawn_structure_at(&mut game, "compiler", 30, 30);
    assert!(
        game.can_extract_routines(),
        "a bench 30 tiles away still counts — extraction has no proximity rule"
    );
}

#[test]
fn extracting_yields_the_picked_routine_destroys_the_program_and_loses_the_rest() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5); // both of its unlocks installed
    spawn_structure_at(&mut game, "compiler", 30, 30);

    let offered = game.extractable_routines(medic);
    assert_eq!(offered.len(), 2, "both installed routines are on offer");
    let kept = offered[1].id.clone();
    let lost = offered[0].id.clone();

    game.extract_routine(medic, 1).unwrap();

    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&kept).as_str()),
        1,
        "the picked routine lands in inventory"
    );
    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&lost).as_str()),
        0,
        "everything else on the program is lost with it"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != medic),
        "the program is consumed"
    );
}

#[test]
fn extraction_is_refused_for_a_program_you_dont_own_and_during_battle() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let wild = spawn_wild_on_player_tile(&mut game);
    let err = game.extract_routine(wild, 0).unwrap_err();
    assert!(err.contains("control"), "{err}");

    let pet = spawn_tamed(&mut game, 10, 3);
    start_battle_with_a_wild_program(&mut game);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("right now"), "{err}");
}
