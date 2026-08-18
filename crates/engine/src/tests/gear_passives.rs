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
use crate::*;

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

// --------------------------------------------------------- gear grants fire

/// The same passive as above, granted by three wearable items rather than
/// installed. Two of them name it deliberately: one routine granted twice
/// is the case that must *not* pay twice.
const A_GRANTING_WEAPON: &str = r#"(
    id: "test_grant_weapon", name: "Test Grant Weapon", description: "d",
    equipment: Some((Weapon, (atk: 1))), grants: Some("test_round_start"),
)"#;

const A_GRANTING_MODULE: &str = r#"(
    id: "test_grant_module", name: "Test Grant Module", description: "d",
    equipment: Some((Module, (def: 1))), grants: Some("test_round_start"),
)"#;

/// Wearable, grants nothing, and stat-for-stat identical to its granting
/// twin — the control for "the gear did it" against "wearing anything at
/// all did it". A stat line that differed by a point would move the
/// passive's own damage, since a Damage effect scales with its caster.
const A_PLAIN_MODULE: &str = r#"(
    id: "test_plain_module", name: "Test Plain Module", description: "d",
    equipment: Some((Module, (def: 1))),
)"#;

const A_PLAIN_WEAPON: &str = r#"(
    id: "test_plain_weapon", name: "Test Plain Weapon", description: "d",
    equipment: Some((Weapon, (atk: 1))),
)"#;

fn assets_with_granting_gear(tag: &str) -> ScratchAssets {
    modded_assets_dir(
        tag,
        &[],
        &[
            ("test_grant_weapon.ron", A_GRANTING_WEAPON),
            ("test_grant_module.ron", A_GRANTING_MODULE),
            ("test_plain_module.ron", A_PLAIN_MODULE),
            ("test_plain_weapon.ron", A_PLAIN_WEAPON),
        ],
        &[],
        &[],
        &[("test_round_start.ron", A_ROUND_START_PASSIVE)],
    )
}

/// Damage the enemy took in one round where every party member defended —
/// so the passive is the only thing in the round that can move it.
fn damage_in_one_defended_round(game: &mut Game) -> i32 {
    let before = total_enemy_hp(game);
    resolve_a_planned_round(game);
    before - total_enemy_hp(game)
}

/// The feature, and its own control. The stripped half is what stops this
/// passing with the `Equipment` source deleted: worn and unworn differ only
/// in which module is on, so nothing but the grant can explain a gap.
#[test]
fn a_worn_grant_fires_and_a_plain_item_in_the_same_slot_does_not() {
    let dir = assets_with_granting_gear("worn_or_not");

    let mut worn = battle_with_a_passive_holder_prepared(&dir, 9201, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_grant_module");
    });
    let mut bare = battle_with_a_passive_holder_prepared(&dir, 9201, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_plain_module");
    });

    assert!(
        damage_in_one_defended_round(&mut worn) > damage_in_one_defended_round(&mut bare),
        "the grant is the only difference between these two battles"
    );
}

/// A routine fires once per source. Spending a slot on what your gear
/// already gives you is meant to pay, and deduping across the two sources
/// would silently delete that — while the cooldown, keyed on the id, is
/// still one entry.
#[test]
fn a_grant_and_an_installed_copy_both_fire_and_share_one_cooldown() {
    let dir = assets_with_granting_gear("two_sources");

    let mut both =
        battle_with_a_passive_holder_prepared(&dir, 9202, Some("test_round_start"), |g| {
            let player = g.player_entity();
            wear(g, player, "test_grant_module");
        });
    let mut installed_only = battle_with_a_passive_holder_in(&dir, 9202, Some("test_round_start"));

    let twice = damage_in_one_defended_round(&mut both);
    let once = damage_in_one_defended_round(&mut installed_only);
    assert!(
        twice > once,
        "two sources should land twice: {twice} against {once}"
    );

    let player = both.player_entity();
    let cooling = both
        .world
        .get::<AbilityCooldowns>(player)
        .map(|c| c.0.len())
        .unwrap_or(0);
    assert_eq!(cooling, 1, "the cooldown is per id, not per source");
}

/// The mirror case: nothing was spent on the second slot, so there is
/// nothing to pay out for it.
#[test]
fn two_slots_granting_one_routine_fire_it_once() {
    let dir = assets_with_granting_gear("two_slots");

    let mut two = battle_with_a_passive_holder_prepared(&dir, 9203, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_grant_weapon");
        wear(g, player, "test_grant_module");
    });
    let mut one = battle_with_a_passive_holder_prepared(&dir, 9203, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_plain_weapon");
        wear(g, player, "test_grant_module");
    });

    assert_eq!(
        damage_in_one_defended_round(&mut two),
        damage_in_one_defended_round(&mut one),
        "a second slot naming the same routine buys nothing"
    );
}

/// Gear is wearable by any owned program, so a granted passive is a
/// companion's as readily as the player's — and that case is what the
/// feature adds for free.
#[test]
fn a_companions_worn_grant_fires() {
    let dir = assets_with_granting_gear("companion");

    let mut armed = battle_with_a_passive_holder_prepared(&dir, 9204, None, |g| {
        let ally = g.world.resource::<Party>().0[0];
        wear(g, ally, "test_grant_module");
    });
    let mut bare = battle_with_a_passive_holder_prepared(&dir, 9204, None, |g| {
        let ally = g.world.resource::<Party>().0[0];
        wear(g, ally, "test_plain_module");
    });

    assert!(
        damage_in_one_defended_round(&mut armed) > damage_in_one_defended_round(&mut bare),
        "a companion wearing the grant should fire it"
    );
}

/// The cooldown is the whole of what keeps a `RoundStart` passive from
/// being a free extra action every single round.
#[test]
fn a_fired_grant_does_not_fire_again_next_round() {
    let dir = assets_with_granting_gear("cooldown");

    let mut game = battle_with_a_passive_holder_prepared(&dir, 9205, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_grant_module");
    });

    let first = damage_in_one_defended_round(&mut game);
    let second = damage_in_one_defended_round(&mut game);
    assert!(first > 0, "the fixture never fired: {first}");
    assert_eq!(
        second, 0,
        "a four-round cooldown should hold the next round"
    );
}

/// Nothing about a grant reaches the save — it is derived off the worn item
/// every time the trigger comes round. A RON round trip cannot see that;
/// only a real save and load can.
#[test]
fn a_grant_survives_a_save_and_load_with_no_field_of_its_own() {
    let dir = assets_with_granting_gear("roundtrip");
    let path = std::env::temp_dir().join(format!("feral_gear_passive_{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let mut game = battle_with_a_passive_holder_prepared(&dir, 9206, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_grant_module");
        g.save(&path).unwrap();
    });
    let live = damage_in_one_defended_round(&mut game);

    let mut loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);
    let wild = spawn_wild_on_player_tile(&mut loaded);
    {
        let mut stats = loaded.world.get_mut::<Stats>(wild).unwrap();
        stats.max_hp = 40_000;
        stats.hp = 40_000;
    }
    loaded.start_battle(vec![wild]);
    let after = damage_in_one_defended_round(&mut loaded);

    assert!(live > 0, "the fixture never fired before the save: {live}");
    assert_eq!(after, live, "the grant is read off the item, not the save");
}

// ------------------------------------------------------------- the content

/// A census rather than a list of three ids: it is what stops a later edit
/// orphaning a grant, and it puts any item a mod adds under the same rule
/// the shipped three are held to. The loader already refuses a bad one, so
/// what this really asserts is that the shipped set *has* some — a rename
/// that emptied it would leave every test above passing against fixtures.
#[test]
fn every_shipped_grant_names_a_real_battle_passive() {
    let game = Game::new(3401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abilities = game.world.resource::<AbilityDb>();
    let mut granting = 0;
    for def in game.item_defs() {
        let Some(id) = &def.grants else { continue };
        let ability = abilities
            .get(id)
            .unwrap_or_else(|| panic!("{} grants {id:?}, which is not an ability", def.id));
        assert!(
            ability.is_passive() && !ability.effect.field_only(),
            "{} grants {id:?}, which can never fire",
            def.id
        );
        assert!(
            def.equipment.is_some(),
            "{} grants {id:?} but cannot be worn, so nothing will ever read it",
            def.id
        );
        granting += 1;
    }
    assert!(
        granting >= 3,
        "the shipped set should carry the three gear passives, found {granting}"
    );
}
