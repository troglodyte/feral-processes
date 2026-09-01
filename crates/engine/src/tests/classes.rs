//! The player's chosen class: the affinity spread `ClassDb` resolves, the
//! kit it replaces, and the empty-directory property held at both ends —
//! see `classes.rs`'s own module doc comment.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::classes::ClassDb;
use crate::items::ids;
use crate::species::AffinityClass;
use crate::tuning::{AFFINITY_MAX, AFFINITY_NEUTRAL};
use crate::*;

fn save_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("feral_classes_{name}_{}.sav", std::process::id()))
}

/// Recursively copies `src` into `dst`, for the one test here that needs a
/// whole alternate `assets/` tree (`Game::load` takes a single `assets_dir`
/// and every other directory in it must still resolve) rather than a
/// single retuned file in isolation.
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_all(&entry.path(), &dst_path);
        } else {
            std::fs::copy(entry.path(), &dst_path).unwrap();
        }
    }
}

fn default_kit() -> Vec<(ItemId, u32)> {
    vec![
        (ids::ICE_BREAKER.into(), 3),
        (ids::POWER_CELL.into(), 3),
        (ids::CORE_FRAGMENT.into(), 5),
        (ids::OUTLET.into(), 2),
    ]
}

/// A Medic's `Heal` affinity sits above neutral and nothing else moves —
/// the spread affects one axis, not the whole player.
#[test]
fn a_class_moves_its_own_axis_and_nothing_else() {
    let choice = CharacterChoice {
        class: Some(AffinityClass::Medic),
        ..CharacterChoice::default()
    };
    let game = Game::new_with(1, DifficultyMode::Forgiving, &test_assets_dir(), &choice).unwrap();

    assert!(
        game.player_class_affinity(AffinityKind::Heal) > AFFINITY_NEUTRAL,
        "a Medic must be better at Heal than neutral"
    );
    for kind in [
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(
            game.player_class_affinity(kind),
            AFFINITY_NEUTRAL,
            "{kind:?} must stay exactly neutral — a class moves only its own axes"
        );
    }
}

/// A maxed `HealAffinity` perk on top of a Medic's own spread must still
/// clamp at `AFFINITY_MAX`, not exceed it — `ability_affinity`'s existing
/// ceiling has to hold with a class term added, not just without one.
#[test]
fn a_class_and_a_perk_stack_and_stay_clamped() {
    let choice = CharacterChoice {
        class: Some(AffinityClass::Medic),
        ..CharacterChoice::default()
    };
    let mut game =
        Game::new_with(2, DifficultyMode::Forgiving, &test_assets_dir(), &choice).unwrap();
    let player = game.player_entity();

    let cost = game
        .world
        .resource::<PerkDb>()
        .get(Perk::HealAffinity)
        .unwrap()
        .cost;
    // Chosen so the perk *alone* (on top of flat `AFFINITY_NEUTRAL`) stays
    // under the ceiling but the class spread *plus* the perk crosses it —
    // a level count that already clamps without the class term would let
    // this test pass whether or not the class is actually wired into
    // `ability_affinity`, proving nothing about the additive stacking.
    let levels = 15;
    let rate = AffinityKind::Heal.perk_bonus_per_level();
    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = levels * cost;
    }
    for _ in 0..levels {
        game.unlock_perk(Perk::HealAffinity).unwrap();
    }

    let class_only = game.player_class_affinity(AffinityKind::Heal);
    assert!(
        class_only > AFFINITY_NEUTRAL,
        "the fixture must actually start above neutral, or the clamp proves nothing"
    );
    assert!(
        AFFINITY_NEUTRAL + levels as f32 * rate < AFFINITY_MAX,
        "the perk alone must stay under the ceiling, or this test cannot tell the class \
         term apart from the perk term"
    );
    assert!(
        class_only + levels as f32 * rate > AFFINITY_MAX,
        "the class plus the perk must actually overshoot the ceiling, or the clamp below \
         proves nothing"
    );

    let effect = AbilityEffect::Heal {
        power: 8,
        spread: 0,
    };
    assert_eq!(
        game.ability_affinity(player, &effect),
        AFFINITY_MAX,
        "a Medic spread plus a maxed perk must not exceed the ceiling ability_affinity \
         already enforces"
    );
}

/// A created Medic's `Inventory` is the Medic file's own kit; a choice with
/// no class still gets the four hardcoded items — the fallback
/// `apply_kit`'s doc comment describes.
#[test]
fn the_class_kit_replaces_the_default_kit() {
    let assets = test_assets_dir();
    let (db, warnings) = ClassDb::load_dir(&assets.join("classes")).unwrap();
    assert!(
        warnings.is_empty(),
        "the shipped classes must load clean: {warnings:?}"
    );
    let medic_kit = db.get(AffinityClass::Medic).unwrap().kit.clone();
    assert!(
        !medic_kit.is_empty(),
        "the fixture must ship a real kit, or this proves nothing"
    );

    let with_class = CharacterChoice {
        class: Some(AffinityClass::Medic),
        ..CharacterChoice::default()
    };
    let game = Game::new_with(3, DifficultyMode::Forgiving, &assets, &with_class).unwrap();
    let inventory = game.world.get::<Inventory>(game.player_entity()).unwrap();
    assert_eq!(inventory.items, medic_kit);

    let no_class = Game::new_with(
        3,
        DifficultyMode::Forgiving,
        &assets,
        &CharacterChoice::default(),
    )
    .unwrap();
    let inventory = no_class
        .world
        .get::<Inventory>(no_class.player_entity())
        .unwrap();
    assert_eq!(inventory.items, default_kit());
}

/// The supported-install property, held at **both** ends: with an empty
/// `ClassDb` (standing in for a deleted `assets/classes/`), every axis
/// resolves neutral *and* `apply_kit` falls back to the hardcoded kit, even
/// for a choice that still names a class. A test that only checked one end
/// would pass against a resolver that falls back but a kit that doesn't
/// (or the reverse).
#[test]
fn an_empty_class_directory_plays_as_todays_game() {
    let dir = scratch_assets_dir("classes_empty");
    std::fs::create_dir_all(&*dir).unwrap();
    let (empty_db, warnings) = ClassDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        empty_db.iter().count(),
        0,
        "an empty directory loads no classes"
    );

    let choice = CharacterChoice {
        class: Some(AffinityClass::Medic),
        ..CharacterChoice::default()
    };
    let mut game =
        Game::new_with(4, DifficultyMode::Forgiving, &test_assets_dir(), &choice).unwrap();
    *game.world.resource_mut::<ClassDb>() = empty_db;

    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(
            game.player_class_affinity(kind),
            AFFINITY_NEUTRAL,
            "{kind:?} must resolve neutral once nothing backs the chosen class"
        );
    }

    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();
    crate::classes::apply_kit(&mut game, Some(AffinityClass::Medic));
    let inventory = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inventory.items,
        default_kit(),
        "apply_kit must fall back even though a class is still chosen"
    );
}

/// A broken file costs the game that one class and nothing else — same
/// contract as every other `*Db::load_dir` in this crate.
#[test]
fn a_malformed_class_file_is_skipped_with_a_warning() {
    let dir = scratch_assets_dir("classes_malformed");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(&*dir.join("broken.ron"), "( class: Medic").unwrap();

    let (db, warnings) = ClassDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 1, "one bad file, one warning: {warnings:?}");
    assert_eq!(db.iter().count(), 0);
}

/// The player stores the class, not the spread, so a class file retuned
/// between a save and a load reaches the run already in progress —
/// `classes.rs`'s module doc comment states this as the deliberate
/// opposite of `ActiveContract`.
#[test]
fn a_retuned_class_file_reaches_a_loaded_save() {
    let choice = CharacterChoice {
        class: Some(AffinityClass::Medic),
        ..CharacterChoice::default()
    };
    let mut game =
        Game::new_with(5, DifficultyMode::Forgiving, &test_assets_dir(), &choice).unwrap();
    assert_eq!(game.player_class_affinity(AffinityKind::Heal), 1.3);

    let path = save_path("retune");
    game.save(&path).unwrap();

    let retuned_assets = scratch_assets_dir("classes_retuned");
    copy_dir_all(&test_assets_dir(), &retuned_assets);
    std::fs::write(
        retuned_assets.join("classes").join("medic.ron"),
        "(class: Medic, name: \"Medic\", description: \"d\", affinities: (heal: 1.6, damage: 1.0), kit: [])",
    )
    .unwrap();

    let loaded = Game::load(&path, &retuned_assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.player_class_affinity(AffinityKind::Heal),
        1.6,
        "the resolved spread must come from the db at load time, not a stored value"
    );
}
