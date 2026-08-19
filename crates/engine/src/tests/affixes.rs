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
use crate::tests::support::{ScratchAssets, scratch_assets_dir, test_assets_dir};
use bevy_ecs::prelude::Entity;

/// A scratch affix directory holding `files` as `(filename, body)`. Built
/// on the `ScratchAssets` guard for the reason `sectors::sector_dir`
/// records: `Drop` runs on an unwind, a manual cleanup call does not.
fn affix_dir(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
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
fn axes(stats: EquipmentStats) -> [(&'static str, i32, i32); 6] {
    [
        ("ATK", stats.atk, 3),
        ("MIT", stats.mitigation, 9),
        ("DECOMP", stats.decompiler, 3),
        ("ACC", stats.accuracy, 3),
        ("EVA", stats.evasion, 5),
        // A damage affix widens a band, and the high end is what bounds it.
        ("DMG", stats.damage.max, 3),
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
    let (db, warnings) = AffixDb::load_dir(&dir).unwrap();

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
    let (db, warnings) = AffixDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.all().count(), 1);
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
        item,
        rarity: Rarity::Ordinary,
        tier: 0,
        affix: Some(affix.id.clone()),
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
        item,
        rarity: Rarity::Ordinary,
        tier: 0,
        affix: Some(affix.id.clone()),
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
