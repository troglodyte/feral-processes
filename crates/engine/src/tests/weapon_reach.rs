//! A weapon whose swing lands on more than one body.
//!
//! The feature is one `#[serde(default)]` field on `ItemDef` naming an
//! enemy-facing plural `AbilityTarget`, one door that decides whether a
//! given swing is wide (`Game::swing_reach`), and one sweep per combat
//! model built out of the converter that model already has.
//!
//! The halves held apart here are what the loader refuses, what the charge
//! gates, and what each model's sweep lands on.

use super::support::*;
use crate::abilities::{AbilityDb, AbilityShape};
use crate::items_db::ItemDb;
use crate::tuning::TACTICAL_GROUP_RADIUS;

/// `gear_passives.rs`' loader fixture: the reach's refusals live in
/// `ItemDb::load_dir` beside `ungrantable_ability`, so they need a real
/// `AbilityDb` alongside the scratch item tree.
fn load_items(tag: &str, files: &[(&str, &str)]) -> (ItemDb, Vec<String>) {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(dir.join("items")).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join("items").join(name), body).unwrap();
    }
    let (abilities, _) = AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    ItemDb::load_dir(&dir.join("items"), &abilities).unwrap()
}

/// Armour that cleaves is an authoring mistake, and one that would
/// otherwise be silently inert: nothing but a weapon is ever asked for a
/// reach, so the field would sit in the file doing nothing.
#[test]
fn a_reach_on_a_non_weapon_is_refused_at_load() {
    let (db, warnings) = load_items(
        "reach_non_weapon",
        &[(
            "cleaving_vest.ron",
            r#"(id: "cleaving_vest", name: "Cleaving Vest", description: "d",
                equipment: Some((Armor, (mitigation: 1))),
                reach: Some((target: WholeEnemyGroup, recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("cleaving_vest").is_none(),
        "only a weapon swings, so a reach on anything else must not load"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// A swing is aimed at the other side. An ally-facing target would resolve
/// through `ability_recipients` to the player's own party.
#[test]
fn a_reach_aimed_at_an_ally_is_refused_at_load() {
    let (db, warnings) = load_items(
        "reach_ally_facing",
        &[(
            "friendly_lance.ron",
            r#"(id: "friendly_lance", name: "Friendly Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: WholeParty, recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("friendly_lance").is_none(),
        "a reach lands on the hostile side; an ally-facing one would swing at the party"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// The same fault in two vocabularies: a reach that reaches exactly one
/// body is not a reach. `OneEnemyGroupFront` says it in the group model's
/// words and `AbilityShape::Single` says it in the board's.
#[test]
fn a_reach_that_reaches_one_body_is_refused_at_load() {
    let (db, warnings) = load_items(
        "reach_single_target",
        &[(
            "narrow_lance.ron",
            r#"(id: "narrow_lance", name: "Narrow Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: OneEnemyGroupFront, recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("narrow_lance").is_none(),
        "a single-target reach reaches nobody past the body already being swung at"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");

    let (db, warnings) = load_items(
        "reach_single_shape",
        &[(
            "flat_lance.ron",
            r#"(id: "flat_lance", name: "Flat Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: WholeEnemyGroup, shape: Some(Single), recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("flat_lance").is_none(),
        "an authored Single shape is the same fault spelled on the board's side"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// The accepted case, and the one-door rule that comes with it: a shipped
/// weapon authors no `shape:`, so a reader taking `self.shape` directly
/// resolves every one of them to nothing.
#[test]
fn a_reach_weapon_loads_and_derives_its_shape() {
    let (db, warnings) = load_items(
        "reach_ok",
        &[(
            "wide_lance.ron",
            r#"(id: "wide_lance", name: "Wide Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: WholeEnemyGroup, recharge: 3)))"#,
        )],
    );
    assert!(warnings.is_empty(), "nothing to warn about: {warnings:?}");
    let reach = db
        .get("wide_lance")
        .expect("a well-formed reach weapon loads")
        .reach
        .expect("the field survives the round trip");
    assert_eq!(reach.recharge, 3);
    assert_eq!(
        reach.tactical_shape(),
        AbilityShape::Radius {
            radius: TACTICAL_GROUP_RADIUS
        },
        "an unauthored shape is derived from the target, `AbilityDef::tactical_shape`'s rule"
    );
}
