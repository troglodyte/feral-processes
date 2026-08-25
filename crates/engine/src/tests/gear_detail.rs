//! The gear inspect page — `Game::gear_detail` and `Game::routine_detail`.
//!
//! One derivation behind the `[I]` page, for the reason `Game::copy_bonus`
//! is one: four screens rebuilt the scaling chain by hand and all four
//! dropped the affix at once. The page adds a second axis of the same
//! hazard — a granted routine's magnitudes are scaled for their invoker, so
//! a renderer reading `AbilityEffect::Damage`'s authored `power` would
//! quote the level-1 figure forever.

use super::support::*;
use crate::abilities::{AbilityId, AbilityTarget, PassiveTrigger};
use crate::items::GearCopy;
use crate::tuning::QUALITY_DEFAULT;
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
/// the same `abilities::scaled_range` the invocation does.
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
        "a level-12 invoker hits for more than a level-1 one"
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
        affixes: vec!["honed".into()],
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

/// **The inspect page is where two copies get compared**, so it states the
/// quality outright rather than leaving the player to read it off the name
/// — including at spec, where the name says nothing. A figure missing from
/// a detail page reads as *unknown*, not as 100.
///
/// It rides `WornDetailView` and so is absent for a consumable or a
/// currency, which is honest: only equipment rolls quality.
#[test]
fn the_inspect_page_states_what_a_copy_compiled_at() {
    let game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    let at_spec = GearCopy::plain(ItemId::from("kinetic_edge"));
    assert_eq!(
        game.gear_detail(&at_spec, player)
            .worn
            .expect("a weapon is worn")
            .quality,
        QUALITY_DEFAULT
    );

    let off_spec = GearCopy {
        quality: 115,
        ..at_spec
    };
    assert_eq!(
        game.gear_detail(&off_spec, player)
            .worn
            .expect("a weapon is worn")
            .quality,
        115
    );

    let material = GearCopy::plain(ItemId::from("core_fragment"));
    assert!(
        game.gear_detail(&material, player).worn.is_none(),
        "a material has no slot and so no quality to state"
    );
}

/// A shipped affix that charges for what it pays, and one that does not —
/// with the **trade-off's id sorting after** the ordinary one's.
///
/// That ordering is the whole point of the fixture. A copy's affix list is
/// sorted by `GearCopy::with_affixes`, so handing the two over in either
/// order produces the same list; picking a trade-off that already sorts
/// first would make an ordering test pass against no rule at all.
///
/// Both picks come off a sorted list, so the fixture cannot name a
/// different affix from one run to the next.
fn trade_off_and_ordinary(game: &Game) -> (crate::affixes::AffixDef, crate::affixes::AffixDef) {
    let defs = game.affix_defs();
    let trade_off = defs
        .iter()
        .rfind(|a| charges_for_itself(a))
        .expect("the shipped set carries a drawback affix")
        .clone();
    let ordinary = defs
        .iter()
        .find(|a| !charges_for_itself(a) && a.id < trade_off.id)
        .expect("and one that only pays, sorting ahead of it")
        .clone();
    (trade_off, ordinary)
}

/// The census's own copy of the predicate `Game::affix_lines` sorts on. Kept
/// here rather than shared, because a test asserting an ordering against the
/// production function that decides it would pass whatever that function
/// said.
fn charges_for_itself(a: &crate::affixes::AffixDef) -> bool {
    [
        a.stats.atk,
        a.stats.mitigation,
        a.stats.decompiler,
        a.stats.accuracy,
        a.stats.evasion,
        a.stats.damage.min,
        a.stats.damage.max,
    ]
    .iter()
    .any(|v| *v < 0)
}

/// Fusion stacks affixes and keeps duplicates, so eight drawn from a pool of
/// nine is usually three or four distinct ones. The page folds them: one
/// entry per distinct affix, carrying how many of it the copy holds.
#[test]
fn the_gear_page_folds_a_repeated_affix_into_one_entry() {
    let game = Game::new(4310, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, ordinary) = trade_off_and_ordinary(&game);
    let copy = GearCopy::with_affixes(
        ItemId::from("kinetic_edge"),
        Rarity::Ordinary,
        0,
        vec![ordinary.id.clone(); 3],
        QUALITY_DEFAULT,
    );

    let detail = game.gear_detail(&copy, game.player_entity());
    assert_eq!(
        detail.affixes.len(),
        1,
        "three of one affix is one entry: {:?}",
        detail.affixes
    );
    assert!(
        detail.affixes[0].contains("×3"),
        "the entry must say how many: {:?}",
        detail.affixes
    );
}

/// An affix may charge for what it pays, and the page is the only screen
/// that can tell the player so. The block is capped by what fits, so a
/// drawback sorting last could be the line that falls off — which is
/// exactly the hidden cost this page exists to end. Trade-offs sort first.
#[test]
fn the_gear_page_lists_a_trade_off_affix_first() {
    let game = Game::new(4311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (trade_off, ordinary) = trade_off_and_ordinary(&game);
    let word = |a: &crate::affixes::AffixDef| {
        a.prefix
            .clone()
            .or_else(|| a.suffix.clone())
            .expect("a loaded affix has one or the other")
    };

    // The fixture guarantees the drawback's id sorts *after* the ordinary
    // one's, so the copy's own sorted order would put it second. Only the
    // trade-off-first rule can bring it to the front, which is what makes
    // this test able to fail.
    assert!(
        ordinary.id < trade_off.id,
        "the fixture must hand over a pair the plain id order would reverse"
    );
    let copy = GearCopy::with_affixes(
        ItemId::from("kinetic_edge"),
        Rarity::Ordinary,
        0,
        vec![ordinary.id.clone(), trade_off.id.clone()],
        QUALITY_DEFAULT,
    );
    let detail = game.gear_detail(&copy, game.player_entity());
    assert_eq!(detail.affixes.len(), 2, "{:?}", detail.affixes);
    assert!(
        detail.affixes[0].contains(&word(&trade_off)),
        "the drawback must lead: {:?}",
        detail.affixes
    );
    assert!(
        detail.affixes[1].contains(&word(&ordinary)),
        "{:?}",
        detail.affixes
    );
}

/// The empty-catalogue property, at this seam: a save naming an affix the
/// build no longer has reads as a copy with one fewer effect, so the page
/// skips what it cannot resolve and still draws the rest.
#[test]
fn the_gear_page_skips_an_affix_the_build_does_not_know() {
    let game = Game::new(4312, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, ordinary) = trade_off_and_ordinary(&game);
    let copy = GearCopy::with_affixes(
        ItemId::from("kinetic_edge"),
        Rarity::Ordinary,
        0,
        vec![
            ordinary.id.clone(),
            crate::affixes::AffixId::from("a_mod_that_was_uninstalled"),
        ],
        QUALITY_DEFAULT,
    );

    let detail = game.gear_detail(&copy, game.player_entity());
    assert_eq!(
        detail.affixes.len(),
        1,
        "the unresolvable id contributes nothing, and the rest still draws: {:?}",
        detail.affixes
    );
    // And it contributes nothing to the *count* either. Resolving an unknown
    // id to some placeholder would leave one entry too, reading `×2` — so
    // asserting the length alone passes against that.
    let word = ordinary
        .prefix
        .clone()
        .or_else(|| ordinary.suffix.clone())
        .expect("a loaded affix has one or the other");
    assert!(
        detail.affixes[0].contains(&word) && !detail.affixes[0].contains('×'),
        "the entry must be the known affix, held once: {:?}",
        detail.affixes
    );
}
