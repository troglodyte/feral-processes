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
use crate::abilities::{AbilityDb, PassiveTrigger};
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

/// A grant on `AllyWounded`, on a module so the wearer's own stat line is
/// untouched by which of the two fixtures they are in.
const A_WOUNDING_PASSIVE: &str = r#"(
    id: "test_wounded",
    name: "Test Wound Single",
    description: "Fires when the holder is driven low",
    target: OneEnemyGroupFront,
    effect: Damage(power: 9),
    cooldown: 2,
    power_cost: 0.0,
    triggers: Some(AllyWounded),
)"#;

const A_WOUNDING_MODULE: &str = r#"(
    id: "test_wound_module", name: "Test Wound Module", description: "d",
    equipment: Some((Module, (def: 1))), grants: Some("test_wounded"),
)"#;

fn assets_with_a_wounding_passive(tag: &str) -> ScratchAssets {
    modded_assets_dir(
        tag,
        &[],
        &[("test_wound_module.ron", A_WOUNDING_MODULE)],
        &[],
        &[],
        &[("test_wounded.ron", A_WOUNDING_PASSIVE)],
    )
}

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
        granting >= 8,
        "the shipped set should carry the gear passives, found {granting}"
    );
}

/// Which triggers gear reaches, pinned as a *decision* rather than as
/// coverage. The first version of this test asserted every
/// `PassiveTrigger` variant had gear on it, which turned a deliberate hole
/// into an omission and is what put two items on `AllyDropped` — a trigger
/// whose event is a companion being dissolved and despawned, with no
/// revive at any difficulty. A player never wants that to fire, so nothing
/// they choose to wear should be built around it.
///
/// `deadman` is the exception the list names: an exclusive last-stand
/// routine is supposed to be the thing you never want to see, and Deadman
/// Relay is a Wintermute curio rather than a build.
#[test]
fn gear_reaches_the_triggers_a_player_can_want_and_only_deadman_reaches_the_other() {
    let game = Game::new(3404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abilities = game.world.resource::<AbilityDb>();
    let granted: Vec<(&str, PassiveTrigger)> = game
        .item_defs()
        .iter()
        .filter_map(|def| def.grants.as_ref())
        .filter_map(|id| abilities.get(id))
        .filter_map(|def| def.triggers.map(|t| (def.id.as_str(), t)))
        .collect();

    for trigger in [
        PassiveTrigger::RoundStart,
        PassiveTrigger::AllyWounded,
        PassiveTrigger::Afflicted,
    ] {
        assert!(
            granted.iter().any(|(_, t)| *t == trigger),
            "no shipped item grants anything that fires on {trigger:?}"
        );
    }

    let on_a_death: Vec<&str> = granted
        .iter()
        .filter(|(_, t)| *t == PassiveTrigger::AllyDropped)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(
        on_a_death,
        vec!["deadman"],
        "gear built around losing a program is a design mistake, not a gap to fill"
    );
}

/// One shipped item, all the way through a real round, on the real assets
/// — everything above this runs against scratch content, so a shipped file
/// with a target the passive path cannot resolve would go unnoticed.
/// `ragged_edge` is the one that lands a *condition* rather than damage,
/// which is the effect with the most between authoring it and seeing it.
#[test]
fn a_shipped_granting_weapon_lands_its_condition_in_a_real_round() {
    let mut game = battle_with_a_passive_holder_prepared(&test_assets_dir(), 9301, None, |g| {
        let player = g.player_entity();
        wear(g, player, "ragged_edge");
    });

    resolve_a_planned_round(&mut game);

    let bleeding = game
        .world
        .resource::<BattleState>()
        .groups
        .iter()
        .flat_map(|grp| grp.members.iter())
        .filter_map(|&e| game.world.get::<StatusEffects>(e))
        .filter_map(|s| s.active.as_ref())
        .any(|status| status.kind == StatusKind::Bleed);
    assert!(
        bleeding,
        "the round opened and nothing on the other side is bleeding"
    );
}

/// The describe page is a read-only screen, so the row it shows has to be
/// derived here rather than in the renderer — and it is derived off the
/// *ability*, because the item's own prose is mod-controlled free text that
/// nothing keeps in step with `grants`.
#[test]
fn item_grant_reports_the_routines_own_name_and_prose() {
    let game = Game::new(3402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ability = game
        .world
        .resource::<AbilityDb>()
        .get("interrupt_request")
        .expect("the routine ships");

    assert_eq!(
        game.item_grant(&ItemId("interrupt_coil".into())),
        Some((ability.name.as_str(), ability.description.as_str()))
    );
    assert_eq!(
        game.item_grant(&ItemId("kinetic_edge".into())),
        None,
        "gear that grants nothing has no row to draw"
    );
}

/// A grant is read off the item id, so every axis a *copy* carries —
/// rarity, fusion tier, an affix — is orthogonal to it. Worth pinning
/// rather than reasoning about: `Game::copy_bonus` folds all three into the
/// wearer's stats, and a Damage passive scales with its caster's ATK, so
/// the two systems do meet — just not at the lookup.
#[test]
fn a_rare_fused_affixed_copy_grants_the_same_routine() {
    let game = Game::new(3403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let plain = crate::items::GearCopy::plain(ItemId("interrupt_coil".into()));
    let decorated = crate::items::GearCopy {
        rarity: Rarity::Gold,
        tier: 2,
        affix: Some("honed".into()),
        ..plain.clone()
    };

    assert_eq!(
        game.item_grant(&decorated.item),
        game.item_grant(&plain.item),
        "the grant hangs off the item, not off what the copy rolled"
    );
    assert!(
        game.copy_bonus(&decorated, 12).unwrap().atk > game.copy_bonus(&plain, 12).unwrap().atk,
        "the affix is doing something, or this test proves nothing"
    );
}

/// And the whole way through, not just at the lookup: an affixed copy fires
/// its grant in a real round, and hits harder for the affix — the passive
/// casts as its wearer, so the ATK the copy added is ATK the routine swings
/// with.
#[test]
fn an_affixed_copy_fires_its_grant_and_the_affix_reaches_the_damage() {
    let dir = assets_with_granting_gear("affixed");

    let mut honed = battle_with_a_passive_holder_prepared(&dir, 9207, None, |g| {
        let player = g.player_entity();
        let copy = crate::items::GearCopy {
            affix: Some("honed".into()),
            ..crate::items::GearCopy::plain(ItemId("test_grant_weapon".into()))
        };
        g.add_copies(&copy, 1);
        g.equip(player, &copy).unwrap();
    });
    let mut plain = battle_with_a_passive_holder_prepared(&dir, 9207, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_grant_weapon");
    });

    let with_affix = damage_in_one_defended_round(&mut honed);
    let without = damage_in_one_defended_round(&mut plain);
    assert!(without > 0, "the plain copy never fired: {without}");
    assert!(
        with_affix > without,
        "the affix should reach the passive's swing: {with_affix} against {without}"
    );
}

// ------------------------------------------------------ the AllyWounded trigger

/// Puts `entity` one point of Integrity above the wounded line, so the
/// hostile's own sweep is what carries them across it. Derived from
/// `WOUNDED_INTEGRITY_FRACTION` rather than written as a number: a fixture
/// that hardcoded 0.35 would stop testing a crossing the moment the
/// threshold moved, and would read as the trigger not firing.
fn park_just_above_the_wounded_line(game: &mut Game, entity: Entity) {
    let mut stats = game.world.get_mut::<Stats>(entity).unwrap();
    let line = (stats.max_hp as f32 * tuning::WOUNDED_INTEGRITY_FRACTION).floor() as i32;
    stats.hp = (line + 1).clamp(1, stats.max_hp);
}

/// Drives `entity` to `fraction` of its maximum Integrity directly.
fn set_integrity(game: &mut Game, entity: Entity, fraction: f32) {
    let mut stats = game.world.get_mut::<Stats>(entity).unwrap();
    stats.hp = ((stats.max_hp as f32 * fraction).round() as i32).max(1);
}

/// A grant on `AllyWounded`, and the control that says the wound is what
/// did it: the same battle where the wearer stays healthy fires nothing.
#[test]
fn a_wounded_wearer_fires_its_grant_and_a_healthy_one_does_not() {
    let dir = assets_with_a_wounding_passive("wounded");

    let mut hurt = battle_with_a_passive_holder_prepared(&dir, 9401, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_wound_module");
    });
    let player = hurt.player_entity();
    park_just_above_the_wounded_line(&mut hurt, player);
    let wounded_damage = damage_in_one_defended_round(&mut hurt);

    let mut healthy = battle_with_a_passive_holder_prepared(&dir, 9401, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_wound_module");
    });
    let healthy_damage = damage_in_one_defended_round(&mut healthy);

    assert!(
        wounded_damage > healthy_damage,
        "crossing the line is what fires it: {wounded_damage} against {healthy_damage}"
    );
}

/// The rule the cooldown alone would not give: a party pinned under the
/// line is one crisis, not one per round. Asserted with the *cooldown
/// already expired*, or this passes against a passive that simply hasn't
/// come off cooldown yet.
#[test]
fn a_member_already_under_the_line_is_not_newly_wounded() {
    let dir = assets_with_a_wounding_passive("already_low");

    let mut game = battle_with_a_passive_holder_prepared(&dir, 9402, None, |g| {
        let player = g.player_entity();
        wear(g, player, "test_wound_module");
    });
    let player = game.player_entity();
    // Well under the threshold before the round opens, so nothing that
    // happens in it can be a crossing.
    set_integrity(&mut game, player, 0.10);

    let mut landed = 0;
    for _ in 0..6 {
        if damage_in_one_defended_round(&mut game) > 0 {
            landed += 1;
        }
    }
    assert_eq!(
        landed, 0,
        "a member held low is one crisis; six rounds under the line fired {landed} times"
    );
}

/// A dropped member belongs to `AllyDropped` and must not also be reported
/// as wounded — a round that costs a player a program should not pay them
/// twice for it.
#[test]
fn a_member_who_died_is_not_reported_as_wounded() {
    let dir = assets_with_a_wounding_passive("died");
    let mut game = battle_with_a_passive_holder_in(&dir, 9403, None);

    let ally = game.world.resource::<Party>().0[0];
    let before = game.party_integrity();
    assert!(
        before.iter().any(|(e, _)| *e == ally),
        "the fixture's companion should be in the snapshot"
    );
    {
        let mut stats = game.world.get_mut::<Stats>(ally).unwrap();
        stats.hp = 0;
    }

    assert!(
        !game.newly_wounded_party(&before).contains(&ally),
        "a dead member is AllyDropped's, not AllyWounded's"
    );
}
