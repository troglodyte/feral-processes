//! The gear inspect page — `Game::gear_detail` and `Game::routine_detail`.
//!
//! One derivation behind the `[I]` page, for the reason `Game::copy_bonus`
//! is one: four screens rebuilt the scaling chain by hand and all four
//! dropped the affix at once. The page adds a second axis of the same
//! hazard — a granted routine's magnitudes are scaled for their caster, so
//! a renderer reading `AbilityEffect::Damage`'s authored `power` would
//! quote the level-1 figure forever.

use super::support::*;
use crate::abilities::{AbilityId, AbilityTarget, PassiveTrigger};
use crate::items::GearCopy;
use crate::*;

/// The Crash Handler's whole point is the routine it grants, and until this
/// page existed none of that routine's mechanics reached any screen: the
/// player could read "Grants: Core Dump Single" and had no way to learn
/// when it fires, what it hits, or for how much.
#[test]
fn the_grant_block_carries_the_routines_mechanics() {
    let game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let copy = GearCopy::plain(ItemId::from("crash_handler"));

    let detail = game.gear_detail(&copy, game.player_entity());
    let grant = detail.grant.expect("the Crash Handler grants a routine");

    assert_eq!(grant.name, "Core Dump Single");
    assert!(
        grant.when.contains("Integrity"),
        "an AllyWounded passive says what drives it: {}",
        grant.when
    );
    assert_eq!(grant.target, AbilityTarget::OneEnemyGroupFront.phrase());
    assert!(
        grant.effect.contains("Damage"),
        "the effect line names what it does: {}",
        grant.effect
    );
    assert_eq!(grant.cooldown, 3);
    assert!(
        grant.rolls_to_hit,
        "Damage resolves through battle::resolve_attack and can miss"
    );
}

/// A routine's authored `power` is the level-1 figure. Quoting it directly
/// is the hand-rolled-chain bug in a new place, so the line goes through
/// the same `abilities::scaled_range` the cast does.
#[test]
fn a_damage_routines_band_is_scaled_for_its_caster() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let id = AbilityId::from("core_dump");

    let low = game.routine_detail(&id, player).expect("the routine ships");
    game.world.get_mut::<Experience>(player).unwrap().level = 12;
    let high = game.routine_detail(&id, player).expect("the routine ships");

    assert_ne!(
        low.effect, high.effect,
        "a level-12 caster hits for more than a level-1 one"
    );
}

/// Gear that grants nothing has no block to draw, exactly as `item_grant`
/// has no row.
#[test]
fn gear_that_grants_nothing_has_no_routine_block() {
    let game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let detail = game.gear_detail(
        &GearCopy::plain(ItemId::from("kinetic_edge")),
        game.player_entity(),
    );
    assert!(detail.grant.is_none());
    assert!(detail.worn.is_some(), "but it is still a wearable weapon");
}

/// The stat block is `Game::copy_bonus`, not the item's bare `equipment`
/// block — the four properties a copy carries are exactly what four earlier
/// screens each forgot.
#[test]
fn the_worn_block_prices_the_whole_copy() {
    let game = Game::new(4104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let plain = GearCopy::plain(ItemId::from("kinetic_edge"));
    let decorated = GearCopy {
        rarity: components::Rarity::Gold,
        tier: 2,
        affix: Some("honed".into()),
        ..plain.clone()
    };

    let worn = |copy: &GearCopy| {
        game.gear_detail(copy, game.player_entity())
            .worn
            .expect("a weapon is wearable")
    };
    let (a, b) = (worn(&plain), worn(&decorated));

    let priced = game.copy_bonus(&decorated, b.level).unwrap();
    assert_eq!(
        (b.stats.atk, b.stats.damage.min, b.stats.damage.max),
        (priced.atk, priced.damage.min, priced.damage.max),
        "the block is copy_bonus, whole copy and all"
    );
    assert!(
        b.stats.atk > a.stats.atk,
        "or the test proves nothing about the axes"
    );
}

/// The hit-chance line is the *shared* formula against the game's own
/// definition of a typical program — a copy of either would be free to
/// drift from what a swing actually rolls.
#[test]
fn the_hit_chance_is_the_shared_formula_against_the_median_species() {
    let game = Game::new(4105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worn = game
        .gear_detail(
            &GearCopy::plain(ItemId::from("kinetic_edge")),
            game.player_entity(),
        )
        .worn
        .expect("a weapon is wearable");

    let median = crate::balance_sim::median_ordinary_species(game.world.resource::<SpeciesDb>());
    assert_eq!(
        worn.nominal.evasion,
        battle::evasion_of(median.base_speed, worn.nominal.zone, 0),
        "the nominal hostile is the median species at the zone level, ungeared"
    );
    assert_eq!(
        worn.hit_chance,
        battle::hit_chance(worn.accuracy, worn.nominal.evasion)
    );
}

/// Accuracy is read with the candidate *in its slot*, which is the whole
/// question the swap picker asks. Measured off the worn gear minus what the
/// slot already holds, so inspecting the copy you are wearing reports the
/// accuracy you actually have.
#[test]
fn accuracy_is_measured_with_the_candidate_in_its_slot() {
    let mut game = Game::new(4106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let sighted = ItemId::from("kinetic_edge");

    let bare = game
        .gear_detail(&GearCopy::plain(sighted.clone()), player)
        .worn
        .unwrap()
        .accuracy;

    // Wearing the same copy must report the same accuracy: the slot's own
    // contribution is taken back off before the candidate's is added.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(sighted.clone(), 1);
    game.equip(player, &GearCopy::plain(sighted.clone()))
        .unwrap();
    let worn = game
        .gear_detail(&GearCopy::plain(sighted), player)
        .worn
        .unwrap()
        .accuracy;

    assert_eq!(bare, worn, "double-counting the slot is the trap here");
}

/// Both taxonomies are drawn from an exhaustive match, the rule
/// `render/stack.rs::cell_mark` records: as a `_ =>` arm a new variant
/// ships invisible.
#[test]
fn every_trigger_and_target_has_its_own_phrase() {
    let triggers = [
        PassiveTrigger::AllyDropped,
        PassiveTrigger::AllyWounded,
        PassiveTrigger::Afflicted,
        PassiveTrigger::RoundStart,
    ];
    let targets = [
        AbilityTarget::OneAlly,
        AbilityTarget::WholeParty,
        AbilityTarget::OneEnemyGroupFront,
        AbilityTarget::WholeEnemyGroup,
        AbilityTarget::AllEnemies,
    ];
    let phrases: Vec<String> = triggers
        .iter()
        .map(|t| t.phrase())
        .chain(targets.iter().map(|t| t.phrase().to_string()))
        .collect();

    assert!(phrases.iter().all(|p| !p.is_empty()));
    let unique: std::collections::BTreeSet<&String> = phrases.iter().collect();
    assert_eq!(
        unique.len(),
        phrases.len(),
        "two variants reading the same is a variant nobody can tell apart"
    );
}

/// The page draws the grant in full, so the one-line `Grants:` row would be
/// the same fact twice. Split rather than trimmed off the finished list:
/// `item_effects` stays the one derivation and this is a shorter length of
/// it, exactly as `item_effects` is a shorter length of `item_grant`.
#[test]
fn the_effects_list_leaves_the_grant_to_its_own_block() {
    let game = Game::new(4107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let id = ItemId::from("crash_handler");

    let full = game.item_effects(&id);
    let rest = game.item_effects_besides_grant(&id);

    assert!(
        full.first().is_some_and(|l| l.starts_with("Grants:")),
        "the listing line still leads with the grant: {full:?}"
    );
    assert_eq!(rest, full[1..], "and the page's list is the remainder");
}
