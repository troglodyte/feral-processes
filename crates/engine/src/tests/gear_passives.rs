//! Gear that grants a passive routine.
//!
//! The feature is one line of vocabulary — `ItemDef::grants` names an
//! ability — and one extra source in `Game::ready_passives`. Nothing is
//! written into `Routines` and nothing new reaches the save, so the whole
//! of "unequipping ends it" is that the grant is derived at fire time.
//!
//! The tests here are the two halves that need holding apart: what the
//! loader refuses (an item naming an ability that could never fire), and
//! what the battle does with one it accepted.

use super::support::*;
use crate::abilities::AbilityDb;
use crate::items_db::ItemDb;

/// Loads the shipped abilities, then `files` as items on top of a scratch
/// dir. The cross-database check `grants` needs cannot run inside
/// `ItemDef`, so the refusal lives in `ItemDb::load_dir` and needs a real
/// `AbilityDb` beside it — the same shape `SpeciesDb::load_dir` takes.
fn load_items_against_shipped_abilities(
    tag: &str,
    files: &[(&str, &str)],
) -> (ItemDb, Vec<String>) {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(dir.join("items")).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join("items").join(name), body).unwrap();
    }
    let (abilities, _) = AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    ItemDb::load_dir(&dir.join("items"), &abilities).unwrap()
}

/// A shipped passive that is not field-only, so the valid case is asserted
/// against real content rather than against a fixture that could drift out
/// of what the loader accepts.
fn a_shipped_passive(abilities: &AbilityDb) -> String {
    abilities
        .all()
        .find(|d| d.is_passive() && !d.effect.field_only())
        .expect("some shipped ability is a battle passive")
        .id
        .clone()
}

#[test]
fn an_item_granting_an_unknown_ability_is_skipped() {
    let (db, warnings) = load_items_against_shipped_abilities(
        "grants_unknown",
        &[(
            "phantom.ron",
            r#"(id: "phantom", name: "Phantom", description: "d",
                equipment: Some((Module, (def: 1))), grants: Some("no_such_routine"))"#,
        )],
    );
    assert!(
        db.get("phantom").is_none(),
        "an item naming an ability that does not exist grants nothing and must not load"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

#[test]
fn an_item_granting_a_chosen_routine_is_skipped() {
    // `segfault_v1` is picked on a turn, not fired by an event — worn, it
    // would sit in `ready_passives`' filter forever and never run.
    let (db, warnings) = load_items_against_shipped_abilities(
        "grants_active",
        &[(
            "active.ron",
            r#"(id: "active", name: "Active", description: "d",
                equipment: Some((Weapon, (atk: 1))), grants: Some("segfault_v1"))"#,
        )],
    );
    assert!(
        db.get("active").is_none(),
        "only a passive can be granted; a chosen routine has no trigger to fire on"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// Refused by the *same* check as an active routine, and there is
/// deliberately no third branch for it: `AbilityDef::passive_field_mismatch`
/// already refuses a `triggers` on a field-only effect at ability load, so
/// nothing field-only can be passive by the time an item names it. The case
/// is worth a test and not worth a branch.
#[test]
fn an_item_granting_a_field_only_routine_is_skipped() {
    let (db, warnings) = load_items_against_shipped_abilities(
        "grants_field",
        &[(
            "fieldy.ron",
            r#"(id: "fieldy", name: "Fieldy", description: "d",
                equipment: Some((Module, (def: 1))), grants: Some("wild_jump"))"#,
        )],
    );
    assert!(
        db.get("fieldy").is_none(),
        "a field-only routine has no battle moment to fire in"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

#[test]
fn an_item_granting_a_real_passive_loads() {
    let (abilities, _) = AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    let passive = a_shipped_passive(&abilities);
    let (db, warnings) = load_items_against_shipped_abilities(
        "grants_ok",
        &[(
            "good.ron",
            &format!(
                r#"(id: "good", name: "Good", description: "d",
                    equipment: Some((Module, (def: 1))), grants: Some("{passive}"))"#
            ),
        )],
    );
    let def = db.get("good").expect("a valid grant loads");
    assert_eq!(def.grants.as_deref(), Some(passive.as_str()));
    assert!(warnings.is_empty(), "nothing to warn about: {warnings:?}");
}

#[test]
fn an_item_granting_nothing_still_loads() {
    let (db, warnings) = load_items_against_shipped_abilities(
        "grants_absent",
        &[(
            "plain.ron",
            r#"(id: "plain", name: "Plain", description: "d")"#,
        )],
    );
    assert!(
        db.get("plain").is_some(),
        "the field is optional; every item shipped before it existed omits it"
    );
    assert!(warnings.is_empty(), "{warnings:?}");
}

// ------------------------------------------------------- the RoundStart trigger

/// A `RoundStart` passive as a scratch ability file, so the trigger's call
/// site is under test before any shipped content depends on it. Its damage
/// is the observable — an enemy's Integrity is the only thing in the fixture
/// that a defending party can move.
///
/// **Single scope on purpose.** Every shipped passive before this one
/// reached the whole party or the whole field, which is the only reason
/// `fire_passives`' hardcoded `SpecialTarget` went unnoticed; a Single-scope
/// passive is what the gear in this feature carries, and it lands on nobody
/// unless `Game::passive_target` resolves one.
const A_ROUND_START_PASSIVE: &str = r#"(
    id: "test_round_start",
    name: "Test Tick Single",
    description: "Fires at the top of every round",
    target: OneEnemyGroupFront,
    effect: Damage(power: 9),
    cooldown: 4,
    power_cost: 0.0,
    triggers: Some(RoundStart),
)"#;

fn assets_with_a_round_start_passive(tag: &str) -> ScratchAssets {
    modded_assets_dir(
        tag,
        &[],
        &[],
        &[],
        &[],
        &[("test_round_start.ron", A_ROUND_START_PASSIVE)],
    )
}

/// A round opening is a fact about the round, not about anything that
/// happened in it, so this must fire in a round where nothing else does —
/// which is also what makes the control half meaningful. Without the second
/// battle, this passes against a passive that never fires at all, because a
/// defending party still trades no damage either way.
#[test]
fn a_round_start_passive_fires_on_a_round_where_nothing_happens() {
    let dir = assets_with_a_round_start_passive("round_start");

    let mut armed = battle_with_a_passive_holder_in(&dir, 9101, Some("test_round_start"));
    let armed_before = total_enemy_hp(&armed);
    resolve_a_planned_round(&mut armed);
    let armed_damage = armed_before - total_enemy_hp(&armed);

    let mut quiet = battle_with_a_passive_holder_in(&dir, 9101, None);
    let quiet_before = total_enemy_hp(&quiet);
    resolve_a_planned_round(&mut quiet);
    let quiet_damage = quiet_before - total_enemy_hp(&quiet);

    assert!(
        armed_damage > quiet_damage,
        "the passive should open the round: {armed_damage} with it, {quiet_damage} without"
    );
}
