//! Startup validation of the shipped and modded asset directories.

use super::support::*;
use crate::*;

#[test]
fn game_new_aborts_startup_when_the_item_set_is_missing_the_currency_role() {
    // The economy can't run without a Currency-role item — see
    // `ItemDb::missing_roles` — so `Game::new` must abort before the
    // world is built rather than let play reach `Game::currency()`'s
    // `.expect("validated at startup")` deep in gameplay.
    let dir = assets_dir_missing_currency_item();
    let result = Game::new(900, DifficultyMode::Forgiving, &dir);
    let _ = std::fs::remove_dir_all(&dir);

    // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
    // use `Result::expect_err` / `unwrap_err`.
    let Err(err) = result else {
        panic!("startup should abort rather than run with no item holding the Currency role");
    };
    assert!(
        err.to_string().contains("Currency"),
        "error should name the missing role: {err}"
    );
}

#[test]
fn game_load_aborts_when_the_item_set_is_missing_the_currency_role() {
    // Resuming a save is the other door into the same world, and it
    // reaches the same `Game::currency()` `.expect("validated at
    // startup")` — so an item set that lost its Currency-role holder
    // between saving and loading has to be refused here too, not only
    // in `Game::new`.
    let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_missing_currency_load_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let dir = assets_dir_missing_currency_item();
    let result = Game::load(&path, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&path);

    // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
    // use `Result::expect_err` / `unwrap_err`.
    let Err(err) = result else {
        panic!("loading should abort rather than resume with no item holding the Currency role");
    };
    assert!(
        err.to_string().contains("Currency"),
        "error should name the missing role: {err}"
    );
}

#[test]
fn every_shipped_asset_file_loads_without_a_warning() {
    // A malformed shipped asset is warn-and-skipped like a mod's would
    // be, so it costs the player content silently instead of failing the
    // build. This is the only thing that catches it — a serde attribute
    // missing from `ItemId` once made every asset load fail this way.
    let game = Game::new(901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let skipped: Vec<String> = game
        .message_log(usize::MAX)
        .into_iter()
        .map(|(_, text)| text)
        .filter(|text| text.contains("skipped invalid"))
        .collect();

    assert!(
        skipped.is_empty(),
        "shipped assets must all parse: {skipped:#?}"
    );
}

#[test]
fn the_shipped_species_kits_reference_only_real_abilities() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();
    let mut declared = 0;
    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                db.get(&ability.id).is_some(),
                "species {:?} names unknown ability {:?}",
                species.id,
                ability.id
            );
            assert!(
                ability.level >= 1 && ability.level <= crate::tuning::CREATURE_MAX_LEVEL,
                "species {:?}: ability {:?} unlocks at level {}, outside 1..={}",
                species.id,
                ability.id,
                ability.level,
                crate::tuning::CREATURE_MAX_LEVEL
            );
            declared += 1;
        }
    }
    assert!(
        declared >= 10,
        "the shipped roster should actually use the ability system, found {declared}"
    );
}

/// Authored text replaced a derivation that could not go blank. The only
/// thing left to guard mechanically is that nothing shipped is *missing*
/// text — a wrong number in an authored line is a review problem, not a
/// test one.
#[test]
fn every_shipped_item_and_structure_has_description_text() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.item_defs() {
        assert!(
            !def.description.trim().is_empty(),
            "item {} ships with no description",
            def.id.as_str()
        );
    }
    for def in game.structure_defs() {
        assert!(
            !def.description.trim().is_empty(),
            "structure {} ships with no description",
            def.id
        );
    }
}
