//! Startup validation of the shipped and modded asset directories.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::tuning::{AFFINITY_MAX, AFFINITY_NEUTRAL};
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

/// The twenty hunt-only routines are reachable exactly one way: off a wild
/// carrier. A species or research file naming one would quietly restore the
/// "just target the species" loop this set exists to break.
#[test]
fn no_species_or_research_file_grants_a_wild_only_ability() {
    let game = Game::new(3301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild_only: Vec<String> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.clone())
        .collect();
    assert_eq!(wild_only.len(), 20, "twenty routines are hunt-only");

    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                !wild_only.contains(&ability.id),
                "species {:?} grants {:?}, which is meant to be findable only in the field",
                species.id,
                ability.id
            );
        }
    }
    for node in game.world.resource::<crate::research::ResearchDb>().all() {
        for id in &node.unlocks_abilities {
            assert!(
                !wild_only.contains(id),
                "research node {:?} unlocks {:?}, which is meant to be findable only in the field",
                node.id,
                id
            );
        }
    }
}

/// A cooldown of 0 means a hostile carrier fires the routine every single
/// round (see `Game::wild_retaliate`). `decompile` is the one exception: it
/// is the player's capture mechanism, hostiles never use it, and a cooldown
/// on a failed capture roll would change the core loop.
#[test]
fn every_shipped_ability_but_decompile_has_a_cooldown() {
    let game = Game::new(3302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if def.id == crate::abilities::DECOMPILE_ABILITY_ID {
            assert_eq!(def.cooldown, 0, "decompile stays spammable, deliberately");
            continue;
        }
        assert!(
            def.cooldown >= 1,
            "ability {:?} has no cooldown, so a wild carrier would fire it every round",
            def.id
        );
    }
}

const AFFINITY_SPECIES: &str = r#"(
    id: "test_healer",
    name: "Test Healer",
    glyph: 'h',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (heal: 1.5, damage: 0.8),
)"#;

#[test]
fn a_species_declares_affinities_and_omitted_ones_stay_neutral() {
    let dir = super::support::modded_assets_dir(
        "affinity_species",
        &[],
        &[],
        &[("test_healer.ron", AFFINITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), 1.5);
    assert_eq!(aff.get(AffinityKind::Damage), 0.8);
    // The three the file never named must default individually, not leave
    // the whole struct at its all-neutral fallback.
    assert_eq!(aff.get(AffinityKind::Buff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Debuff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Drain), AFFINITY_NEUTRAL);
}

#[test]
fn a_species_file_with_no_affinities_field_still_loads_neutral() {
    // The #[serde(default)] contract that keeps every shipped file and
    // every third-party mod parsing untouched.
    let dir = super::support::modded_assets_dir(
        "affinity_absent",
        &[],
        &[],
        &[("test_plain.ron", super::support::TWO_ABILITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_medic").unwrap();
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(aff.get(kind), AFFINITY_NEUTRAL);
    }
}

#[test]
fn an_out_of_range_affinity_is_clamped_at_load() {
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: 99.0");
    let dir = super::support::modded_assets_dir(
        "affinity_clamped",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), AFFINITY_MAX);
}

#[test]
fn a_nan_affinity_disqualifies_the_file_and_the_rest_still_load() {
    // NaN specifically, not just inf: f32::clamp returns NaN for a NaN
    // input, so the clamp alone would pass this straight through into
    // every magnitude the species ever casts.
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: NaN");
    let dir = super::support::modded_assets_dir(
        "affinity_nan",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(
        game.species_affinities("test_healer").is_none(),
        "a species with a non-finite affinity should not have loaded"
    );
    // A single bad mod file must not take the shipped roster down with it.
    assert!(game.species_affinities("drone").is_some());
}
