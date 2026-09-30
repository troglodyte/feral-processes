//! The affix database: what a file has to say to be loadable, and the
//! census over the affixes the game actually ships.
//!
//! Nothing here names a shipped affix. `affixes.rs` is fully data-driven —
//! a mod's affix is in the roll on exactly the same terms as ours — so a
//! test that reached for an id by hand would be asserting about one file
//! rather than about the rule, and would start passing vacuously the day
//! that file was renamed.

use crate::DifficultyMode;
use crate::Game;
use crate::affixes::AffixDb;
use crate::components::{Decompiler, Rarity, Stats};
use crate::items::{EquipmentSlot, EquipmentStats, GearCopy, ItemId};
use crate::tests::support::{
    ScratchAssets, modded_assets_dir, scratch_assets_dir, test_assets_dir,
};
use bevy_ecs::prelude::Entity;

/// A scratch affix directory holding `files` as `(filename, body)`. Built
/// on the `ScratchAssets` guard for the reason `ScratchAssets`
/// records: `Drop` runs on an unwind, a manual cleanup call does not.
fn affix_dir(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

/// `AffixDb::load_dir` over `dir`, checked against the shipped items — the
/// ids a research block names are validated there.
fn load_affixes(dir: &std::path::Path) -> (AffixDb, Vec<String>) {
    let assets = test_assets_dir();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
    AffixDb::load_dir(dir, &items).unwrap()
}

/// Every axis an affix may charge on, each paired with the most it may be
/// worth — so a test can speak about "the axis this affix charges on"
/// without naming it, and a new axis cannot be added without a ceiling.
///
/// **One ceiling per axis, not one across all of them.** They are no longer
/// the same currency: `mitigation` is percentage points where `atk` is flat
/// damage, so the +3 that makes an attack affix generous is nearly nothing
/// as a percentage. The ceilings are calibrated against what the shipped
/// gear grants on each axis — see `assets/affixes/README.md`.
///
/// Not `tuning.rs` constants: these bound the *content*, are checked against
/// the content, and an affix past one stops being a bonus on an item and
/// starts being the item.
fn axes(stats: EquipmentStats) -> [(&'static str, i32, i32); 7] {
    [
        ("ATK", stats.atk, 3),
        ("MIT", stats.mitigation, 9),
        ("DECOMP", stats.decompiler, 3),
        ("ACC", stats.accuracy, 3),
        ("EVA", stats.evasion, 5),
        // A damage affix widens a band, and the high end is what bounds it.
        ("DMG", stats.damage.max, 3),
        // Flat per-hit reduction, unscaled and capped per wearer at
        // `tuning::DEFLECTION_MAX` (6): +4 leaves room for a second piece.
        ("DEFL", stats.deflection, 4),
    ]
}

// ---------------------------------------------------------------------------
// What a file has to say.

/// An affix that only charges is refused. The bonus-with-a-drawback shape
/// is supported deliberately, which is exactly what makes this reachable:
/// before penalties were a legal thing to author, `fault`'s "grants no
/// stats" arm caught every unusable file by itself. It no longer does — a
/// file granting nothing but `-2 DEF` passes that arm, and a player has no
/// reason to ever equip the copy that rolled it.
#[test]
fn an_affix_that_only_charges_is_refused() {
    let dir = affix_dir(
        "affix_pure_penalty",
        &[(
            "cursed.ron",
            r#"(id: "cursed", prefix: Some("Cursed"), stats: (def: -2))"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0, "the affix must be skipped");
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(
        warnings[0].contains("cursed.ron"),
        "the warning must name the file: {}",
        warnings[0]
    );
}

/// The other side of the same arm: a penalty *beside* a bonus is a
/// well-formed affix, not a malformed one.
#[test]
fn an_affix_that_pays_before_it_charges_loads() {
    let dir = affix_dir(
        "affix_trade_off",
        &[(
            "volatile.ron",
            r#"(id: "volatile", prefix: Some("Volatile"), stats: (atk: 2, def: -1))"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.all().count(), 1);
}

/// An affix paying on an axis that is not ATK, mitigation or decompiler
/// loads. Both of `fault`'s emptiness arms enumerated three of the six
/// fields, so an accuracy-only or evasion-only affix was refused as though
/// it granted nothing — the accuracy axis could only ever ride along on an
/// ATK affix, which is part of why it stayed on three weapons.
#[test]
fn an_affix_paying_only_on_a_newer_axis_loads() {
    let dir = affix_dir(
        "affix_newer_axes",
        &[
            (
                "zeroed.ron",
                r#"(id: "zeroed", prefix: Some("Zeroed"), stats: (accuracy: 3))"#,
            ),
            (
                "slippery.ron",
                r#"(id: "slippery", prefix: Some("Slippery"), stats: (evasion: 3))"#,
            ),
            (
                "keen.ron",
                r#"(id: "keen", prefix: Some("Keen"), stats: (damage: (min: 1, max: 3)))"#,
            ),
        ],
    );
    let (db, warnings) = load_affixes(&dir);

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.all().count(), 3, "all three axes have to be authorable");
}

// ---------------------------------------------------------------------------
// The census over what ships.

/// Every shipped affix pays something, and none pays past the calibration
/// the README states. The ceiling is what stops an affix out-weighing the
/// item it lands on; the floor is the rule the load-time arm above enforces
/// for a mod, asserted here over our own files so a retune cannot ship one
/// the engine would have refused from anybody else.
#[test]
fn every_shipped_affix_pays_and_none_pays_past_the_calibration() {
    let game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for affix in game.affix_defs() {
        let axes = axes(affix.stats);
        assert!(
            axes.iter().any(|&(_, value, _)| value > 0),
            "{} grants no positive stat, so nothing weighs its cost",
            affix.id.as_str()
        );
        for (name, value, ceiling) in axes {
            assert!(
                value <= ceiling,
                "{} grants {value} {name}, past that axis's calibration ceiling of \
                 +{ceiling} — see assets/affixes/README.md before widening this",
                affix.id.as_str()
            );
        }
    }
}

/// No slot may be left with nothing to roll. A slot with an empty pool is
/// one where `Game::roll_affix` finds nothing whatever it draws, so every
/// drop for that slot is as interchangeable as it was before affixes
/// existed — the exact complaint the feature answers, surviving in one
/// corner of it.
#[test]
fn every_slot_has_something_to_roll() {
    let game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs = game.affix_defs();

    for slot in EquipmentSlot::ALL {
        let pool = defs.iter().filter(|d| d.fits(slot)).count();
        assert!(pool > 0, "{slot:?} has no eligible affix at all");
    }
}

// ---------------------------------------------------------------------------
// A drawback is a trade, and it is a trade at every point in a run.

/// The shipped set carries at least one affix that charges for what it
/// grants. Asserted separately from the behaviour below so that removing
/// the last one fails *here*, saying so, rather than turning the two tests
/// that follow into vacuous passes over a lucky pick.
#[test]
fn the_shipped_set_offers_a_trade_off() {
    let game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.affix_defs()
            .iter()
            .any(|d| axes(d.stats).iter().any(|&(_, value, _)| value < 0)),
        "no shipped affix carries a drawback"
    );
}

/// A shipped drawback affix, with an equippable item it may land on, and
/// which of `axes` it charges on.
///
/// Both picks are made off a *sorted* list. `affix_defs` and `item_defs`
/// both come out of a `HashMap`, so an unsorted `find` would silently test
/// a different affix on a different item from one run to the next — which
/// is the flake this repo has already paid for twice in seeded spawn tests.
fn a_trade_off(game: &Game) -> (crate::affixes::AffixDef, ItemId, usize) {
    let mut defs = game.affix_defs();
    defs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let affix = defs
        .into_iter()
        .find(|d| axes(d.stats).iter().any(|&(_, value, _)| value < 0))
        .expect("the_shipped_set_offers_a_trade_off says there is one");
    let charged = axes(affix.stats)
        .iter()
        .position(|&(_, value, _)| value < 0)
        .expect("it was found by having one");

    let mut items: Vec<ItemId> = game
        .item_defs()
        .into_iter()
        .filter(|d| {
            game.equipment_of(&d.id)
                .is_some_and(|(slot, _)| affix.fits(slot))
        })
        .map(|d| d.id)
        .collect();
    items.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let item = items
        .into_iter()
        .next()
        .expect("every slot has equippable gear");
    (affix, item, charged)
}

/// The trade holds in both directions and grows with the run: the affixed
/// copy is worth strictly less on the axis it charges and strictly more on
/// the axis it pays, at zone 1 and by a wider margin later.
///
/// The margin widening is the whole reason an affix is folded into the base
/// *before* `copy_bonus`'s three scaling axes. Added afterwards, a drawback
/// would quietly stop costing anything after a breach or two — which reads
/// as a free upgrade rather than as the choice it was authored to be.
#[test]
fn a_drawback_is_a_trade_at_zone_one_and_a_bigger_one_later() {
    let game = Game::new(4104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (affix, item, charged) = a_trade_off(&game);
    let paid = axes(affix.stats)
        .iter()
        .position(|&(_, value, _)| value > 0)
        .expect("the census says every affix pays something");

    let plain = GearCopy::plain(item.clone());
    let dressed = GearCopy {
        rarity: Rarity::Ordinary,
        tier: 0,
        affixes: vec![affix.id.clone()],
        ..GearCopy::plain(item)
    };
    // Just the values: the ceilings are the census's business, not this
    // test's.
    let bonus = |copy: &GearCopy, zone: u32| {
        axes(game.copy_bonus(copy, zone).unwrap()).map(|(_, value, _)| value)
    };

    let mut previous_cost = 0;
    for zone in [1, 5] {
        let bare = bonus(&plain, zone);
        let worn = bonus(&dressed, zone);
        assert!(
            worn[charged] < bare[charged],
            "at zone {zone} the drawback cost nothing: {worn:?} against {bare:?}"
        );
        assert!(
            worn[paid] > bare[paid],
            "at zone {zone} the bonus was worth nothing: {worn:?} against {bare:?}"
        );

        let cost = bare[charged] - worn[charged];
        assert!(
            cost > previous_cost,
            "the drawback stopped growing with the run ({previous_cost} -> {cost} by zone {zone})"
        );
        previous_cost = cost;
    }
}

/// Wearing and removing a copy that carries a drawback leaves the wearer
/// exactly where it found them, on every axis.
///
/// This is `EquippedItem::fusion_tier`'s documented trap reached from the
/// other side. `apply_equipment_delta` writes a bonus straight into `Stats`,
/// so an unequip that computed a different figure from its equip would weld
/// the difference into the wearer's base stats with no record of where it
/// came from — and with a negative component in the delta, the welded
/// difference is a permanent *loss* the player can never account for.
#[test]
fn wearing_and_removing_a_drawback_copy_leaves_no_dent() {
    let mut game = Game::new(4105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (affix, item, _) = a_trade_off(&game);
    let slot = game.equipment_of(&item).expect("it is equippable").0;

    // DECOMP is a component of its own rather than a `Stats` field, and
    // `apply_equipment_delta` writes both — so all three have to be read
    // back, or the one axis living somewhere else is the one that dents.
    let read = |game: &Game, who: Entity| {
        let stats = game.world.get::<Stats>(who).unwrap();
        let skill = game.world.get::<Decompiler>(who).map_or(0, |d| d.skill);
        (stats.atk, stats.mitigation, skill)
    };

    let before = read(&game, player);
    let copy = GearCopy {
        rarity: Rarity::Ordinary,
        tier: 0,
        affixes: vec![affix.id.clone()],
        ..GearCopy::plain(item)
    };

    game.add_copies(&copy, 1);
    game.equip(player, &copy).unwrap();
    assert_ne!(
        read(&game, player),
        before,
        "the copy changed nothing worn, so this proves nothing about taking it off"
    );

    game.unequip(player, slot).unwrap();
    assert_eq!(
        read(&game, player),
        before,
        "a drawback copy left a permanent dent in the wearer's base stats"
    );
}

// ---------------------------------------------------------------------------
// The loader's other refusals.

/// Its bonus would have no visible source on the item's name.
#[test]
fn an_affix_with_neither_prefix_nor_suffix_is_refused() {
    let dir = affix_dir(
        "affix_no_prefix_or_suffix",
        &[("bare.ron", r#"(id: "bare", stats: (atk: 1))"#)],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("bare.ron"));
}

/// No screen has room for a name carrying both.
#[test]
fn an_affix_with_both_prefix_and_suffix_is_refused() {
    let dir = affix_dir(
        "affix_both_prefix_and_suffix",
        &[(
            "both.ron",
            r#"(id: "both", prefix: Some("Honed"), suffix: Some("of Static"), stats: (atk: 1))"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("both.ron"));
}

/// An affix with zero weight is unusable because it would never roll.
#[test]
fn an_affix_with_zero_weight_is_refused() {
    let dir = affix_dir(
        "affix_zero_weight",
        &[(
            "never.ron",
            r#"(id: "never", prefix: Some("Never"), weight: 0, stats: (atk: 1))"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("never.ron"));
}

/// It would rename an item and change nothing else.
#[test]
fn an_affix_granting_no_stats_is_refused() {
    let dir = affix_dir(
        "affix_no_stats",
        &[(
            "hollow.ron",
            r#"(id: "hollow", prefix: Some("Hollow"), stats: ())"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("hollow.ron"));
    // `has_upside` would refuse this file too; the reason given is what tells the two arms apart.
    assert!(
        warnings[0].contains("grants no stats"),
        "wrong reason: {}",
        warnings[0]
    );
}

/// It allows no slots, so it could never be rolled.
#[test]
fn an_affix_with_an_empty_slot_list_is_refused() {
    let dir = affix_dir(
        "affix_empty_slots",
        &[(
            "nowhere.ron",
            r#"(id: "nowhere", prefix: Some("Nowhere"), slots: Some([]), stats: (atk: 1))"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("nowhere.ron"));
}

/// An unparseable affix file is skipped with a warning.
#[test]
fn an_unparseable_affix_file_is_skipped_with_a_warning() {
    let dir = affix_dir("affix_broken_ron", &[("broken.ron", r#"(id: "#)]);
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("broken.ron"));
}

/// Loading from a missing directory yields an empty database and no warnings.
#[test]
fn a_missing_affix_directory_yields_an_empty_database() {
    let dir = scratch_assets_dir("affix_missing");
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 0);
    assert!(warnings.is_empty());
}

/// A notes.txt file beside a valid affix is ignored without warnings.
#[test]
fn a_notes_txt_file_is_ignored_without_warnings() {
    let dir = affix_dir(
        "affix_with_notes",
        &[
            (
                "ok.ron",
                r#"(id: "ok", prefix: Some("Ok"), stats: (atk: 1))"#,
            ),
            ("notes.txt", "This is a note."),
        ],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 1);
    assert!(warnings.is_empty());
}

/// The calibration reaches Deflection, so a shipped or modded-in affix past
/// +4 DEFL is caught by the census rather than slipping through unmeasured.
#[test]
fn the_calibration_census_measures_deflection() {
    let stats = EquipmentStats {
        deflection: 5,
        ..EquipmentStats::default()
    };

    assert!(
        axes(stats)
            .iter()
            .any(|&(name, value, ceiling)| name == "DEFL" && value > ceiling),
        "a +5 DEFL affix must read as past its ceiling"
    );
}

// ---------------------------------------------------------------------------
// Deflection: a flat per-hit reduction, read live and never scaled.

const DEFLECTING: &str = r#"(id: "deflecting", prefix: Some("Deflecting"), stats: (deflection: 2), slots: Some([Armor]))"#;

/// A game whose affix pool also holds `deflecting` (+2 DEFL, armour only).
fn deflecting_game(seed: u32, tag: &str) -> Game {
    let dir = modded_assets_dir(tag, &[], &[], &[], &[], &[]);
    std::fs::write(dir.join("affixes").join("deflecting.ron"), DEFLECTING).unwrap();
    Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap()
}

fn deflecting_armour(count: usize) -> GearCopy {
    GearCopy {
        affixes: vec!["deflecting".into(); count],
        ..GearCopy::plain(ItemId::from("firewall_plating"))
    }
}

/// Wears `copy` on the player and zeroes the wearer's percentage
/// mitigation to `percent`, so the flat term is read on its own.
fn wear_with_mitigation(game: &mut Game, copy: &GearCopy, percent: i32) -> Entity {
    let player = game.player_entity();
    game.add_copies(copy, 1);
    game.equip(player, copy).unwrap();
    game.world.get_mut::<Stats>(player).unwrap().mitigation = percent;
    player
}

#[test]
fn an_affix_granting_only_deflection_loads() {
    let dir = affix_dir("affix_deflection_only", &[("deflecting.ron", DEFLECTING)]);
    let (db, warnings) = load_affixes(&dir);

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.all().count(), 1);
}

#[test]
fn deflection_is_subtracted_even_with_no_percentage_mitigation() {
    let mut game = deflecting_game(4110, "deflection_zero_mitigation");
    let player = wear_with_mitigation(&mut game, &deflecting_armour(1), 0);
    assert_eq!(
        game.effective_mitigation(player),
        0,
        "the test's precondition"
    );

    assert_eq!(game.mitigate_incoming_damage(player, 20), 18);
}

#[test]
fn deflection_comes_off_after_the_percentage_cut_and_the_floor_is_one() {
    let mut game = deflecting_game(4111, "deflection_order");
    let player = wear_with_mitigation(&mut game, &deflecting_armour(1), 50);
    assert_eq!(
        game.effective_mitigation(player),
        50,
        "the test's precondition"
    );

    // 20 -> 10 by the percentage, then -2.
    assert_eq!(game.mitigate_incoming_damage(player, 20), 8);
    // 2 -> 1 by the percentage, then -2 would be -1: a landed hit stays one.
    assert_eq!(game.mitigate_incoming_damage(player, 2), 1);
    // A miss is not raised to a hit.
    assert_eq!(game.mitigate_incoming_damage(player, 0), 0);
}

#[test]
fn a_wearers_total_deflection_is_capped() {
    let mut game = deflecting_game(4112, "deflection_cap");
    // Four of the +2 affix is 8 before the cap.
    let player = wear_with_mitigation(&mut game, &deflecting_armour(4), 0);

    assert_eq!(
        game.mitigate_incoming_damage(player, 20),
        20 - crate::tuning::DEFLECTION_MAX
    );
}

/// What one scaling axis does to a copy's deflection, against a control
/// stat on the same copy so the axis is known to have acted at all.
fn deflection_and_control(game: &Game, copy: &GearCopy, level: u32) -> (i32, i32) {
    let bonus = game.copy_bonus(copy, level).unwrap();
    (bonus.deflection, bonus.mitigation)
}

#[test]
fn deflection_is_not_scaled_by_gear_level() {
    let game = deflecting_game(4113, "deflection_level");
    let copy = deflecting_armour(1);
    let (base, base_control) = deflection_and_control(&game, &copy, 1);
    let (high, high_control) = deflection_and_control(&game, &copy, 10);

    assert!(high_control > base_control, "level scaled nothing");
    assert_eq!((base, high), (2, 2));
}

#[test]
fn deflection_is_not_scaled_by_fusion_tier() {
    let game = deflecting_game(4114, "deflection_tier");
    let fused = GearCopy {
        tier: 3,
        ..deflecting_armour(1)
    };
    let (deflection, control) = deflection_and_control(&game, &fused, 1);

    assert!(control > 9, "the tier scaled nothing");
    assert_eq!(deflection, 2);
}

#[test]
fn deflection_is_not_scaled_by_rarity() {
    let game = deflecting_game(4115, "deflection_rarity");
    let rare = GearCopy {
        rarity: Rarity::Prismatic,
        ..deflecting_armour(1)
    };
    let (deflection, control) = deflection_and_control(&game, &rare, 1);

    assert!(control > 9, "the rarity scaled nothing");
    assert_eq!(deflection, 2);
}

#[test]
fn deflection_is_not_scaled_by_quality() {
    let game = deflecting_game(4116, "deflection_quality");
    let fine = GearCopy {
        quality: crate::tuning::QUALITY_MAX,
        ..deflecting_armour(1)
    };
    let (deflection, control) = deflection_and_control(&game, &fine, 1);

    assert!(control > 9, "the quality scaled nothing");
    assert_eq!(deflection, 2);
}

#[test]
fn stat_summary_names_deflection() {
    let game = Game::new(4117, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mods = EquipmentStats {
        deflection: 2,
        ..EquipmentStats::default()
    };

    assert_eq!(game.stat_summary(mods), "+2 DEFL");
}

// ---------------------------------------------------------------------------
// Research-only affixes: never rolled, and refused unless they can be paid for.

/// A research-only armour affix `id`, optionally with `requires`.
fn research_affix(id: &str, extra: &str) -> String {
    format!(
        r#"(id: "{id}", prefix: Some("Deflecting"), stats: (deflection: 2), slots: Some([Armor]),
            research: Some((cost: 20, apply_cost: [("core_fragment", 3)]{extra})))"#
    )
}

fn ids(pool: Vec<&crate::affixes::AffixDef>) -> Vec<&str> {
    pool.iter().map(|a| a.id.as_str()).collect()
}

#[test]
fn a_research_only_affix_is_never_offered_to_a_roll() {
    let dir = affix_dir(
        "affix_research_pool",
        &[
            (
                "plain.ron",
                r#"(id: "plain", prefix: Some("Plain"), stats: (mitigation: 2))"#,
            ),
            ("gated.ron", &research_affix("gated", "")),
        ],
    );
    let (db, warnings) = load_affixes(&dir);

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.all().count(), 2, "it still loads, for the research tree");
    assert_eq!(ids(db.pool_for(EquipmentSlot::Armor)), vec!["plain"]);
}

#[test]
fn the_shipped_roll_pool_is_every_shipped_affix_that_fits() {
    let game = Game::new(4120, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs = game.affix_defs();
    let (db, _) = load_affixes(&test_assets_dir().join("affixes"));

    for slot in EquipmentSlot::ALL {
        let mut expected: Vec<&str> = defs
            .iter()
            .filter(|d| d.fits(slot))
            .map(|d| d.id.as_str())
            .collect();
        expected.sort();
        assert_eq!(ids(db.pool_for(slot)), expected, "{slot:?}");
    }
}

/// The caravan rolls from the same pool as a drop. With only a research-only
/// affix installed, a row that is *certain* to carry an affix (`bonus`) still
/// carries none.
#[test]
fn a_caravan_row_never_carries_a_research_only_affix() {
    let dir = modded_assets_dir("affix_research_caravan", &[], &[], &[], &[], &[]);
    let affixes = dir.join("affixes");
    std::fs::remove_dir_all(&affixes).unwrap();
    std::fs::create_dir_all(&affixes).unwrap();
    std::fs::write(affixes.join("gated.ron"), research_affix("gated", "")).unwrap();
    let game = Game::new(4121, DifficultyMode::Forgiving, &dir).unwrap();
    assert_eq!(
        game.affix_defs().len(),
        1,
        "the precondition: only `gated` loaded"
    );

    let mut rng = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(7);
    for _ in 0..20 {
        let copy = game.roll_shelf_copy(ItemId::from("firewall_plating"), &mut rng, true);
        assert!(copy.affixes.is_empty(), "rolled {:?}", copy.affixes);
    }
}

fn refusal(tag: &str, files: &[(&str, &str)], names: &str) {
    let dir = affix_dir(tag, files);
    let (db, warnings) = load_affixes(&dir);
    assert_eq!(db.all().count(), 0, "it must be skipped");
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(warnings[0].contains(names), "warning was: {}", warnings[0]);
}

#[test]
fn an_empty_apply_cost_is_refused() {
    refusal(
        "affix_empty_apply_cost",
        &[(
            "free.ron",
            r#"(id: "free", prefix: Some("Free"), stats: (deflection: 2),
                research: Some((cost: 5, apply_cost: [])))"#,
        )],
        "apply_cost",
    );
}

#[test]
fn an_unknown_item_in_apply_cost_is_refused() {
    refusal(
        "affix_unknown_apply_item",
        &[(
            "odd.ron",
            r#"(id: "odd", prefix: Some("Odd"), stats: (deflection: 2),
                research: Some((cost: 5, apply_cost: [("no_such_item", 1)])))"#,
        )],
        "no_such_item",
    );
}

#[test]
fn an_unknown_item_in_materials_is_refused() {
    refusal(
        "affix_unknown_material",
        &[(
            "odd.ron",
            r#"(id: "odd", prefix: Some("Odd"), stats: (deflection: 2),
                research: Some((cost: 5, materials: [("no_such_item", 1)],
                                apply_cost: [("core_fragment", 1)])))"#,
        )],
        "no_such_item",
    );
}

#[test]
fn a_requirement_on_an_unknown_affix_is_refused() {
    refusal(
        "affix_unknown_requirement",
        &[(
            "child.ron",
            &research_affix("child", r#", requires: ["ghost"]"#),
        )],
        "ghost",
    );
}

#[test]
fn a_requirement_on_a_droppable_affix_is_refused() {
    let dir = affix_dir(
        "affix_droppable_requirement",
        &[
            (
                "plain.ron",
                r#"(id: "plain", prefix: Some("Plain"), stats: (mitigation: 2))"#,
            ),
            (
                "child.ron",
                &research_affix("child", r#", requires: ["plain"]"#),
            ),
        ],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(ids(db.pool_for(EquipmentSlot::Armor)), vec!["plain"]);
    assert_eq!(db.all().count(), 1, "only `plain` survives");
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
}

/// Dropping a prerequisite drops what requires it, however deep.
#[test]
fn a_refused_prerequisite_takes_its_dependants_with_it() {
    let dir = affix_dir(
        "affix_requirement_cascade",
        &[
            (
                "root.ron",
                r#"(id: "root", prefix: Some("Root"), stats: (deflection: 2),
                    research: Some((cost: 5, apply_cost: [])))"#,
            ),
            ("mid.ron", &research_affix("mid", r#", requires: ["root"]"#)),
            (
                "leaf.ron",
                &research_affix("leaf", r#", requires: ["mid"]"#),
            ),
            ("free.ron", &research_affix("free", "")),
        ],
    );
    let (db, warnings) = load_affixes(&dir);

    let mut left: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
    left.sort();
    assert_eq!(left, vec!["free"]);
    assert_eq!(warnings.len(), 3, "warnings were {warnings:?}");
}

#[test]
fn a_chain_of_research_only_affixes_loads_intact() {
    let dir = affix_dir(
        "affix_requirement_chain",
        &[
            ("root.ron", &research_affix("root", "")),
            (
                "leaf.ron",
                &research_affix("leaf", r#", requires: ["root"]"#),
            ),
        ],
    );
    let (db, warnings) = load_affixes(&dir);

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.all().count(), 2);
}

/// Weight is moot on a research-only affix, so it is warned about and kept.
#[test]
fn a_weight_on_a_research_only_affix_warns_but_loads() {
    let dir = affix_dir(
        "affix_research_weight",
        &[(
            "heavy.ron",
            r#"(id: "heavy", prefix: Some("Heavy"), stats: (deflection: 2), weight: 9,
                research: Some((cost: 5, apply_cost: [("core_fragment", 1)])))"#,
        )],
    );
    let (db, warnings) = load_affixes(&dir);

    assert_eq!(db.all().count(), 1);
    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(warnings[0].contains("weight"), "{}", warnings[0]);
}
