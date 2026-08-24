//! Equipping, unequipping, and fusing gear.

use super::support::*;
use crate::tuning::{MAX_FUSIONS, QUALITY_DEFAULT, QUALITY_MAX, QUALITY_MIN};
use crate::*;

#[test]
fn equip_grants_stat_bonus_and_removes_item_from_inventory() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    let atk_before = game.player_status().atk;

    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();

    let status = game.player_status();
    assert_eq!(
        status.atk,
        atk_before + 3,
        "weapon should grant its Attack bonus"
    );
    assert_eq!(
        status.weapon,
        Some(EquippedItem {
            copy: gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
            level: 1
        })
    );
    assert!(
        status
            .inventory
            .iter()
            .all(|r| r.copy.item != ItemId::from(ids::OVERCLOCK_CORE)),
        "equipped item should leave the inventory stack"
    );
}

/// An equip and the unequip that undoes it must move `Stats` by the same
/// amount in both directions, at every zone.
///
/// `apply_equipment_delta` writes the bonus straight into `Stats` and
/// `unequip` subtracts a freshly *recomputed* one, so the two are only
/// symmetric while `scaled_for_level` returns the same figure at both ends.
/// Any change to the gear curve that leaves an old bonus baked into a saved
/// `Stats` breaks it in the direction nobody notices: the unequip subtracts
/// less than the equip added and welds the difference into the player's base
/// stats permanently, with no record of where it came from. That is exactly
/// the trap `EquippedItem::fusion_tier` carries a doc comment about.
#[test]
fn an_equip_and_its_unequip_cancel_exactly_at_every_zone() {
    for zone in 1..=5 {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.resource_mut::<ZoneLevel>().0 = zone;
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);

        let before = game.player_status();
        game.equip(player, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
            .unwrap();
        let worn = game.player_status();
        game.unequip(player, EquipmentSlot::Weapon).unwrap();
        let after = game.player_status();

        assert!(
            worn.atk > before.atk,
            "zone {zone}: the equip did nothing, so the symmetry is vacuous"
        );
        assert_eq!(
            (after.atk, after.mitigation),
            (before.atk, before.mitigation),
            "zone {zone}: {} ATK welded into base stats by an equip/unequip \
             round trip",
            after.atk - before.atk
        );
    }
}

#[test]
fn equipping_gear_in_a_deeper_zone_adds_100_percent_of_base_per_level() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    let atk_before = game.player_status().atk;

    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();

    let status = game.player_status();
    // Base +3 ATK, plus 100% of base per level above 1: level 3 = 3 * 3 = 9.
    // Linear, matching `ZoneLevel::stat_multiplier` — a geometric gear curve
    // against a linear zone curve inverts the bug it was matched to fix.
    assert_eq!(
        status.atk,
        atk_before + 9,
        "gear equipped at zone level 3 should be base * 3"
    );
    assert_eq!(
        status.weapon,
        Some(EquippedItem {
            copy: gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
            level: 3
        })
    );

    game.unequip(game.player_entity(), EquipmentSlot::Weapon)
        .unwrap();
    assert_eq!(
        game.player_status().atk,
        atk_before,
        "unequipping should remove exactly the level-scaled bonus that was granted"
    );
}

#[test]
fn equipping_the_same_slot_again_swaps_without_double_counting_the_bonus() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 2);
    let atk_before = game.player_status().atk;

    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();
    assert_eq!(game.player_status().atk, atk_before + 3);

    // Equipping into an already-occupied slot swaps the old item back
    // to inventory and must not stack the bonus a second time.
    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();
    let status = game.player_status();
    assert_eq!(
        status.atk,
        atk_before + 3,
        "re-equipping must not double the bonus"
    );
    assert_eq!(
        status
            .inventory
            .iter()
            .find(|r| r.copy.item == ItemId::from(ids::OVERCLOCK_CORE))
            .map(|r| r.qty),
        Some(1),
        "the swapped-out copy should return to inventory"
    );
}

#[test]
fn unequip_removes_bonus_and_returns_item_to_inventory() {
    let mut game = Game::new(10, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::FIREWALL_PLATING), 1);
    let def_before = game.player_status().mitigation;
    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::FIREWALL_PLATING), 0),
    )
    .unwrap();
    assert_eq!(game.player_status().mitigation, def_before + 9);

    game.unequip(game.player_entity(), EquipmentSlot::Armor)
        .unwrap();

    let status = game.player_status();
    assert_eq!(
        status.mitigation, def_before,
        "unequip should remove the bonus"
    );
    assert_eq!(status.armor, None);
    assert_eq!(
        status
            .inventory
            .iter()
            .find(|r| r.copy.item == ItemId::from(ids::FIREWALL_PLATING))
            .map(|r| r.qty),
        Some(1)
    );
}

#[test]
fn unequip_errors_on_an_empty_slot() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.unequip(game.player_entity(), EquipmentSlot::Weapon)
            .is_err()
    );
}

#[test]
fn unequipping_an_item_with_no_itemdb_entry_errors_instead_of_panicking() {
    // A save can restore an `EquippedItem` id that `ItemDb::load_dir`
    // has since warned-and-skipped (the mod's .ron was renamed, broken,
    // or removed) — `Game::load` doesn't validate equipment slots
    // against the item set, so `equipment_of` can no longer resolve
    // the id by the time the player tries to unequip it.
    let mut game = Game::new(712, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let broken = ItemId::from("a_removed_mod_item");
    game.world.get_mut::<Equipment>(player).unwrap().weapon = Some(EquippedItem {
        copy: gear(&broken.clone(), 0),
        level: 1,
    });
    let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();
    let stats_before = {
        let stats = game.world.get::<Stats>(player).unwrap();
        (stats.atk, stats.mitigation)
    };
    let decompiler_before = game.world.get::<Decompiler>(player).map(|d| d.skill);

    let result = game.unequip(game.player_entity(), EquipmentSlot::Weapon);

    assert!(
        result.is_err(),
        "unequipping an item absent from ItemDb should error, not panic"
    );
    assert_eq!(
        game.player_status().weapon.map(|eq| eq.copy.item),
        Some(broken),
        "a refused unequip must leave the item in its slot, not destroy it"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        inventory_before,
        "a refused unequip must not touch the inventory"
    );
    let stats_after = game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        (stats_after.atk, stats_after.mitigation),
        stats_before,
        "a refused unequip must not alter stats"
    );
    assert_eq!(
        game.world.get::<Decompiler>(player).map(|d| d.skill),
        decompiler_before,
        "a refused unequip must not alter decompiler skill"
    );
}

#[test]
fn equipping_over_a_slot_holding_an_item_with_no_itemdb_entry_errors_instead_of_panicking() {
    // Same failure mode as the unequip case above, but hit via the
    // swap-out path when equipping a new item into an already-occupied
    // slot whose old occupant's data is gone.
    let mut game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let broken = ItemId::from("a_removed_mod_item");
    game.world.get_mut::<Equipment>(player).unwrap().weapon = Some(EquippedItem {
        copy: gear(&broken.clone(), 0),
        level: 1,
    });
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();
    let stats_before = {
        let stats = game.world.get::<Stats>(player).unwrap();
        (stats.atk, stats.mitigation)
    };
    let decompiler_before = game.world.get::<Decompiler>(player).map(|d| d.skill);

    let result = game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    );

    assert!(
        result.is_err(),
        "equipping over a slot whose old item is absent from ItemDb should error, not panic"
    );
    assert_eq!(
        game.player_status().weapon.map(|eq| eq.copy.item),
        Some(broken),
        "a refused equip must leave the old item in its slot, not destroy it"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        inventory_before,
        "a refused equip must not consume the new item from inventory"
    );
    let stats_after = game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        (stats_after.atk, stats_after.mitigation),
        stats_before,
        "a refused equip must not alter stats"
    );
    assert_eq!(
        game.world.get::<Decompiler>(player).map(|d| d.skill),
        decompiler_before,
        "a refused equip must not alter decompiler skill"
    );
}

/// The reported bug, and the reason this feature exists. Fusing used to
/// upgrade the item *type* — the ledger was keyed by `ItemId` — so every
/// spare and every copy picked up afterwards equipped at the fused tier.
/// It reads as a display bug in the inventory screen and is not one.
#[test]
fn fusing_leaves_the_spares_ordinary() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 6);

    game.fuse_item(&gear(&armor, 0)).unwrap();

    assert_eq!(held_at(&game, &armor, 1), 1, "one stronger copy comes out");
    assert_eq!(
        held_at(&game, &armor, 0),
        4,
        "the spares stay ordinary — they are not what was fused"
    );
}

/// The ladder's real price in base copies: 2 for a T1, 4 for a T2, 8 for a
/// T3, because each rung is two copies of the rung below. The last
/// assertion pins the ceiling as the refusal, with the stock still in hand.
#[test]
fn the_fusion_ladder_doubles_its_cost_at_every_rung() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(2001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 8);

    for tier in 0..MAX_FUSIONS {
        while held_at(&game, &armor, tier) >= crate::tuning::ITEM_FUSION_COST {
            game.fuse_item(&gear(&armor, tier)).unwrap();
        }
    }

    assert_eq!(
        held_at(&game, &armor, MAX_FUSIONS),
        1,
        "eight base copies buy exactly one T{MAX_FUSIONS}"
    );
    for tier in 0..MAX_FUSIONS {
        assert_eq!(
            held_at(&game, &armor, tier),
            0,
            "nothing left over at T{tier}"
        );
    }

    let err = game.fuse_item(&gear(&armor, MAX_FUSIONS)).unwrap_err();
    assert!(
        err.contains("can't be fused again"),
        "a maxed copy should say so, got: {err}"
    );
}

#[test]
fn fuse_item_bonus_scales_the_equipped_stat_bonus() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Ablative Plating's base is +12 mitigation, and at that magnitude the
    // *percentage* is now the larger of the two terms: 12 * 1.4 = 17 against
    // a floor of 12 + 2 = 14. That is an inversion — the floor used to win
    // here, when the piece granted 4 points of subtractive DEF — and it is a
    // consequence of mitigation being percentage points, which tripled every
    // armour number while `ITEM_FUSION_MIN_BONUS_PER_TIER` stayed at 1. The
    // floor still does the work on the flat axes, where gear grants 1-4;
    // `the_fusion_floor_beats_the_percentage_at_the_magnitudes_gear_ships_at`
    // in `items.rs` is what holds that, since no shipped armour can show it
    // any more.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 6);

    let def_before = game.player_status().mitigation;
    game.equip(game.player_entity(), &gear(&armor, 0)).unwrap();
    assert_eq!(
        game.player_status().mitigation,
        def_before + 12,
        "unfused equip should grant the plain base bonus"
    );
    game.unequip(game.player_entity(), EquipmentSlot::Armor)
        .unwrap();

    // Four base copies for one T2: two T1s, then those two.
    game.fuse_item(&gear(&armor, 0)).unwrap();
    game.fuse_item(&gear(&armor, 0)).unwrap();
    game.fuse_item(&gear(&armor, 1)).unwrap();
    assert_eq!(held_at(&game, &armor, 2), 1);

    game.equip(game.player_entity(), &gear(&armor, 2)).unwrap();
    assert_eq!(
        game.player_status().mitigation,
        def_before + 17,
        "tier 2: the +20% (12 * 1.4 -> 17) beats the +1/tier floor (12 + 2)"
    );
}

#[test]
fn fuse_item_rejects_non_equipment_and_insufficient_stock() {
    let mut game = Game::new(202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(
        game.fuse_item(&gear(&ItemId::from(ids::CORE_FRAGMENT), 0))
            .is_err(),
        "plain resources aren't equipment and can't be fused"
    );

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    assert!(
        game.fuse_item(&gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
            .is_err(),
        "fusing needs 2 copies, only 1 is available"
    );
    assert_eq!(
        held_at(&game, &ItemId::from(ids::OVERCLOCK_CORE), 0),
        1,
        "a failed fuse should not consume the lone copy"
    );
}

/// Both refusals sit above the first `take_copies`, so neither store is
/// touched — the ordering `install_routine` and `use_symlink` also keep.
#[test]
fn a_refused_fusion_spends_nothing_from_either_store() {
    let core = ItemId::from(ids::OVERCLOCK_CORE);
    let mut game = Game::new(203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(core.clone(), 3);
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&core.clone(), MAX_FUSIONS), 2);

    // Refused by the ceiling, with the stock for it plainly in hand.
    let err = game.fuse_item(&gear(&core, MAX_FUSIONS)).unwrap_err();
    assert!(err.contains("can't be fused again"), "got: {err}");
    // Refused by the stock, one rung down where there is none.
    assert!(game.fuse_item(&gear(&core, MAX_FUSIONS - 1)).is_err());

    assert_eq!(held_at(&game, &core, MAX_FUSIONS), 2);
    assert_eq!(held_at(&game, &core, 0), 3);
}

#[test]
fn fusing_a_worn_item_counts_it_and_upgrades_the_worn_copy_live() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // One copy to wear, three spares.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 4);
    let base_mitigation = game.player_status().mitigation;

    game.equip(game.player_entity(), &gear(&armor, 0)).unwrap();
    assert_eq!(
        game.player_status().mitigation,
        base_mitigation + 12,
        "Ablative Plating's base is +12 mitigation while worn, unfused"
    );
    assert_eq!(
        held_at(&game, &armor, 0),
        3,
        "equipping consumed one of the four copies"
    );

    // The worn copy counts as one of the two a fusion needs, so a single
    // spare is enough — and it is the worn copy that comes out stronger.
    game.fuse_item(&gear(&armor, 0)).unwrap();
    assert_eq!(
        held_at(&game, &armor, 0),
        2,
        "only one spare consumed — the worn copy counted for the other"
    );
    assert_eq!(
        game.player_status().armor.map(|e| e.copy.tier),
        Some(1),
        "the worn copy is the survivor"
    );

    // The worn copy is a T1 now, so the next rung needs a T1 spare —
    // which the two remaining ordinary copies make.
    game.fuse_item(&gear(&armor, 0)).unwrap();
    assert_eq!(held_at(&game, &armor, 1), 1);
    game.fuse_item(&gear(&armor, 1)).unwrap();
    assert_eq!(held_at(&game, &armor, 1), 0);
    assert_eq!(
        game.player_status().mitigation,
        base_mitigation + 17,
        "the worn copy picks up the new tier live, without a re-equip"
    );
}

/// A worn copy pays for a fusion of its *own* tier and no other. Without
/// the tier half of that match, wearing an ordinary copy would discount
/// every rung of the ladder above it.
#[test]
fn a_worn_copy_at_another_tier_does_not_pay_for_the_fusion() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(7041, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 1);
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&armor.clone(), 1), 1);
    game.equip(game.player_entity(), &gear(&armor, 0)).unwrap();

    let err = game.fuse_item(&gear(&armor, 1)).unwrap_err();
    assert_eq!(err, "Need 2 Ablative Plating to fuse (have 1).");
}

/// The refusal counts the whole eligible **group**, worn copy included, not
/// exact matches: with quality and affixes free of the match, an exact count
/// would read "have 1" to a player looking at four copies of the thing. So
/// wearing one and holding no spares is "need 2, have 1", not "need 1, have
/// 0".
#[test]
fn fusing_a_worn_item_still_needs_one_spare() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 1);
    game.equip(game.player_entity(), &gear(&armor, 0)).unwrap(); // now zero spares held
    let err = game.fuse_item(&gear(&armor, 0)).unwrap_err();
    assert_eq!(err, "Need 2 Ablative Plating to fuse (have 1).");
    assert_eq!(
        game.player_status().armor.map(|e| e.copy.tier),
        Some(0),
        "a refused fuse changes nothing"
    );
}

#[test]
fn fusing_needs_two_spares_when_a_different_item_is_worn() {
    let worn = ItemId::from(ids::FIREWALL_PLATING); // armor
    let target = ItemId::from(ids::ABLATIVE_PLATING); // also armor, different item
    let mut game = Game::new(706, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(worn.clone(), 1);
        inv.add(target.clone(), 1);
    }
    game.equip(game.player_entity(), &gear(&worn, 0)).unwrap(); // Firewall Plating occupies the Armor slot
    // The worn armor is a different item, so it can't count toward fusing
    // Ablative Plating — that still needs two spares.
    let err = game.fuse_item(&gear(&target, 0)).unwrap_err();
    assert_eq!(err, "Need 2 Ablative Plating to fuse (have 1).");
}

#[test]
fn a_successful_fuse_returns_its_confirmation_line() {
    let core = ItemId::from(ids::OVERCLOCK_CORE);
    let mut game = Game::new(707, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(core.clone(), 2);
    let msg = game.fuse_item(&gear(&core, 0)).unwrap();
    assert!(
        msg.contains("fuse") && msg.contains('%'),
        "a fuse must hand back a confirmation to surface, got: {msg}"
    );
}

/// Unequipping puts the copy back where its tier belongs. Returning a
/// fused copy to `Inventory` would launder it into the tier-0 store the
/// production chain reads, which is the one thing `GearCopies` exists to
/// prevent.
#[test]
fn unequipping_a_fused_copy_returns_it_to_the_fused_store() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(7071, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&armor.clone(), 2), 1);

    game.equip(game.player_entity(), &gear(&armor, 2)).unwrap();
    game.unequip(game.player_entity(), EquipmentSlot::Armor)
        .unwrap();

    assert_eq!(
        held_at(&game, &armor, 2),
        1,
        "the copy comes back at its tier"
    );
    assert_eq!(
        held_at(&game, &armor, 0),
        0,
        "and not into the ordinary stack"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&armor),
        0,
        "a fused copy must never reach the store recipes read"
    );
}

#[test]
fn a_fused_copy_survives_save_and_load() {
    let assets = test_assets_dir();
    let core = ItemId::from(ids::OVERCLOCK_CORE);
    let mut game = Game::new(203, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(core.clone(), 3);
    game.fuse_item(&gear(&core, 0)).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_fusion_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(held_at(&loaded, &core, 1), 1);
    assert_eq!(held_at(&loaded, &core, 0), 1);
}

/// Gear fusion was uncapped before `MAX_FUSIONS` applied to it, so a save
/// can hold a copy above the ceiling. Carried copies are clamped on load —
/// but the *worn* copy is deliberately left alone, and that is what the
/// last two assertions pin. `apply_equipment_delta` writes straight into
/// `Stats` and the load path restores those numbers verbatim, so lowering
/// the worn tier would make unequipping subtract a smaller bonus than was
/// added and weld the difference into the player's base stats — an
/// invisible buff from a change whose whole purpose is a nerf.
#[test]
fn loading_a_legacy_over_ceiling_tier_clamps_the_ledger_not_the_worn_copy() {
    let assets = test_assets_dir();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let legacy = MAX_FUSIONS + 2;
    let mut game = Game::new(204, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    // Written directly: `fuse_item` now refuses this, which is the point.
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&armor, legacy), 2);
    game.equip(game.player_entity(), &gear(&armor, legacy))
        .unwrap();
    let def_before = game.player_status().mitigation;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_legacy_fusion_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        held_at(&loaded, &armor, MAX_FUSIONS),
        1,
        "the carried copy governs future equips and fusions, so it takes the cap"
    );
    assert_eq!(held_at(&loaded, &armor, legacy), 0);
    assert_eq!(
        loaded
            .world
            .get::<Equipment>(loaded.player_entity())
            .and_then(|e| e.get(EquipmentSlot::Armor))
            .map(|e| e.copy.tier),
        Some(legacy),
        "the worn copy keeps the tier its bonus was actually applied at"
    );
    assert_eq!(
        loaded.player_status().mitigation,
        def_before,
        "clamping must not silently restate what is already in Stats"
    );
}

#[test]
fn erase_item_removes_the_full_stack() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::NEURAL_AMPLIFIER), 3);

    game.erase_item(&gear(&ItemId::from(ids::NEURAL_AMPLIFIER), 0), 3)
        .unwrap();
    assert!(
        game.player_status()
            .inventory
            .iter()
            .all(|r| r.copy.item != ItemId::from(ids::NEURAL_AMPLIFIER))
    );

    assert!(
        game.erase_item(&gear(&ItemId::from(ids::NEURAL_AMPLIFIER), 0), 1)
            .is_err(),
        "erasing from an empty stack should error"
    );
}

#[test]
fn equipped_gear_and_its_bonus_survive_save_and_load() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::NEURAL_AMPLIFIER), 1);
    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::NEURAL_AMPLIFIER), 0),
    )
    .unwrap();
    let decompiler_after_equip = game.player_status().decompiler;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_equipment_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let status = loaded.player_status();
    assert_eq!(
        status.module,
        Some(EquippedItem {
            copy: gear(&ItemId::from(ids::NEURAL_AMPLIFIER), 0),
            level: 1
        })
    );
    assert_eq!(status.decompiler, decompiler_after_equip);
}

#[test]
fn fusing_from_inventory_alone_still_leaves_you_holding_one_copy() {
    let mut game = Game::new(9310, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    // Exactly the fusion cost, none worn — the case a player hits first,
    // and the one where the two paths through `fuse_item` used to diverge:
    // wearing a copy left you holding one afterwards, fusing purely from
    // cargo left you holding nothing at all.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), crate::tuning::ITEM_FUSION_COST);

    game.fuse_item(&gear(&armor, 0)).unwrap();

    assert_eq!(
        held_at(&game, &armor, 1),
        1,
        "a fusion yields one stronger copy; it must not consume the result too"
    );
    assert_eq!(held_at(&game, &armor, 0), 0);
}

#[test]
fn fusing_costs_the_same_whether_or_not_a_copy_is_worn() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let held_after = |wear: bool| {
        let mut game = Game::new(9311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(armor.clone(), 4);
        if wear {
            game.equip(game.player_entity(), &gear(&armor, 0)).unwrap();
        }
        game.fuse_item(&gear(&armor, 0)).unwrap();
        // Every tier, because a fusion moves a copy between the two stores.
        // Equipping moves one out of cargo entirely, so count it back in to
        // compare total copies owned rather than cargo alone.
        let in_cargo: u32 = game
            .player_status()
            .inventory
            .iter()
            .filter(|r| r.copy.item == armor)
            .map(|r| r.qty)
            .sum();
        in_cargo + u32::from(wear)
    };

    assert_eq!(
        held_after(false),
        held_after(true),
        "whether a copy happens to be worn must not change what a fusion costs"
    );
}

#[test]
fn a_fusion_tier_is_worth_at_least_one_point_on_every_stat_it_touches() {
    // Every shipped equipment stat is in the 1..=4 range, where a flat 10%
    // rounds away to nothing: 4 -> 4.4 -> 4. Without a floor the whole
    // mechanic is invisible on real content, which is what made fusing feel
    // like it only destroyed items.
    let base = EquipmentStats {
        atk: 4,
        mitigation: 1,
        decompiler: 0,
        ..EquipmentStats::default()
    };

    let t1 = base.fused_for_tier(1);
    assert_eq!(
        t1.atk, 5,
        "4 at tier 1 must gain a point, not round back to 4"
    );
    assert_eq!(t1.mitigation, 2, "the floor applies to every non-zero stat");
    assert_eq!(
        t1.decompiler, 0,
        "a stat the item does not have stays absent — a floor is not a grant"
    );

    let t3 = base.fused_for_tier(3);
    assert!(
        t3.atk >= base.atk + 3,
        "each tier is worth at least a point: {} should be at least {}",
        t3.atk,
        base.atk + 3
    );
}

// --- Companion equipment -------------------------------------------------

fn stats_of(game: &Game, entity: Entity) -> Stats {
    *game.world.get::<Stats>(entity).unwrap()
}

#[test]
fn a_companion_wears_a_weapon_and_gives_the_bonus_back_on_unequip() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);
    let before = stats_of(&game, companion);

    game.equip(companion, &gear(&weapon, 0)).unwrap();

    assert_eq!(
        stats_of(&game, companion).atk,
        before.atk + 3,
        "a companion should gain the weapon's level-scaled Attack bonus"
    );

    game.unequip(companion, EquipmentSlot::Weapon).unwrap();

    assert_eq!(
        stats_of(&game, companion).atk,
        before.atk,
        "unequipping should return the program to exactly its own numbers"
    );
    assert_eq!(held(&game, &weapon), 1, "the copy goes back to the cargo");
}

#[test]
fn gear_on_one_wearer_leaves_the_other_wearer_alone() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 2);
    let player_before = stats_of(&game, player);
    let companion_before = stats_of(&game, companion);

    game.equip(companion, &gear(&weapon, 0)).unwrap();

    assert_eq!(
        stats_of(&game, player).atk,
        player_before.atk,
        "gear worn by a program must not touch the player's Stats"
    );

    game.equip(player, &gear(&weapon, 0)).unwrap();

    assert_eq!(
        stats_of(&game, companion).atk,
        companion_before.atk + 3,
        "gear worn by the player must not touch the program's Stats"
    );
    assert_eq!(stats_of(&game, player).atk, player_before.atk + 3);
}

#[test]
fn a_copy_taken_off_the_player_goes_onto_a_companion() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);
    let companion_before = stats_of(&game, companion);

    game.equip(player, &gear(&weapon, 0)).unwrap();
    game.unequip(player, EquipmentSlot::Weapon).unwrap();
    game.equip(companion, &gear(&weapon, 0)).unwrap();

    assert_eq!(
        stats_of(&game, companion).atk,
        companion_before.atk + 3,
        "one copy is interchangeable: what comes off the player goes on a program"
    );
    assert_eq!(
        stats_of(&game, player).atk,
        game.player_status().atk,
        "the player should be carrying no leftover bonus"
    );
}

#[test]
fn a_wild_program_cannot_be_geared() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);
    let before = stats_of(&game, wild);

    let result = game.equip(wild, &gear(&weapon, 0));

    assert!(
        result.is_err(),
        "gear only goes on programs the player owns"
    );
    assert_eq!(
        stats_of(&game, wild).atk,
        before.atk,
        "a refused equip must move nothing"
    );
    assert_eq!(
        held(&game, &weapon),
        1,
        "a refused equip must not spend the copy"
    );
}

#[test]
fn a_decompiler_module_on_a_companion_changes_none_of_its_stats() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    // Neural Amplifier's only stat is `decompiler`, and programs never
    // attempt a capture — worn, the bonus is dead.
    let module = ItemId::from(ids::NEURAL_AMPLIFIER);
    give(&mut game, &module, 1);
    let before = stats_of(&game, companion);

    game.equip(companion, &gear(&module, 0)).unwrap();

    let after = stats_of(&game, companion);
    assert_eq!(after.atk, before.atk);
    assert_eq!(after.mitigation, before.mitigation);
    assert_eq!(after.max_hp, before.max_hp);
    assert_eq!(
        game.world
            .get::<Equipment>(companion)
            .and_then(|e| e.get(EquipmentSlot::Module))
            .map(|worn| worn.copy.item),
        Some(module),
        "the module is worn even though its bonus does nothing"
    );
}

/// A program that has never worn anything must survive the strip the
/// destruction paths run unconditionally.
#[test]
fn stripping_a_program_wearing_nothing_is_a_no_op() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    let before = stats_of(&game, companion);

    game.strip_gear(companion);

    let after = stats_of(&game, companion);
    assert_eq!(
        (after.atk, after.mitigation, after.max_hp),
        (before.atk, before.mitigation, before.max_hp)
    );
    assert!(
        game.world.get::<Equipment>(companion).is_none(),
        "a strip must not grow the component it found absent"
    );
}

/// The gear on a program that dies fighting is the player's property and
/// comes back — the reap in `end_battle` runs through
/// `dissolve_tamed_program`, which is where the strip lives.
#[test]
fn a_companion_killed_in_battle_returns_its_gear_to_cargo() {
    let mut game = Game::new(514, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);
    game.equip(companion, &gear(&weapon, 0)).unwrap();
    assert_eq!(held(&game, &weapon), 0, "the copy is on the program");

    let enemy = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(enemy).unwrap().hp = 1;
    insert_battle(&mut game, player, vec![enemy]);
    // `apply_damage` is the only path that lowers HP.
    game.apply_damage(companion, 999);
    assert!(!game.creature_alive(companion));

    // Forced: the reap runs at teardown, and the fight only ends if the
    // strike lands on the 1-HP enemy.
    force_the_next_attack_to_land(&mut game);
    player_attacks(&mut game);

    assert!(
        game.world.get::<Stats>(companion).is_none(),
        "the dead program should have been reaped"
    );
    assert_eq!(
        held(&game, &weapon),
        1,
        "its gear is the player's property and comes back"
    );
}

/// Extraction destroys the program too, and goes through the same dissolve.
#[test]
fn extracting_a_routine_from_a_geared_program_returns_its_gear() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5);
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let armor = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &armor, 1);
    game.equip(medic, &gear(&armor, 0)).unwrap();
    assert_eq!(held(&game, &armor), 0);

    game.extract_routine(medic, 0).unwrap();

    assert!(
        game.world.get::<Stats>(medic).is_none(),
        "the program is spent"
    );
    assert_eq!(held(&game, &armor), 1, "its gear comes back to cargo");
}

/// A scratch save path unique to the calling test, cleaned up by the caller.
fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_companion_gear_{tag}_{}.bin",
        std::process::id()
    ))
}

#[test]
fn a_geared_companion_survives_save_and_load() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    let armor = ItemId::from(ids::FIREWALL_PLATING);
    give(&mut game, &armor, 2);
    game.fuse_item(&gear(&armor, 0)).unwrap(); // one tier-1 copy
    game.equip(companion, &gear(&armor, 1)).unwrap();
    let geared = stats_of(&game, companion);

    let path = save_path("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded.world.resource::<Party>().0[0];
    assert_eq!(
        loaded
            .world
            .get::<Equipment>(restored)
            .and_then(|e| e.get(EquipmentSlot::Armor)),
        Some(EquippedItem {
            copy: gear(&armor, 1),
            level: 1
        }),
        "the slot, its gear level and the tier of the copy all survive"
    );
    // The load path restores stats verbatim, so the numbers are the assertion
    // that matters — a restored slot with unrestored stats reads as a working
    // save right up until the first unequip subtracts a bonus never added.
    let after = stats_of(&loaded, restored);
    assert_eq!(
        (after.mitigation, after.atk),
        (geared.mitigation, geared.atk)
    );
}

#[test]
fn a_geared_companion_survives_the_savetools_ron_round_trip() {
    let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);
    game.equip(companion, &gear(&weapon, 0)).unwrap();

    let path = save_path("ron");
    game.save(&path).unwrap();
    let data = crate::save::load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let text = crate::save::to_ron(&data).unwrap();
    let parsed = crate::save::from_ron(&text).unwrap();

    let before = bincode::serde::encode_to_vec(&data, bincode::config::standard()).unwrap();
    let after = bincode::serde::encode_to_vec(&parsed, bincode::config::standard()).unwrap();
    assert_eq!(
        before, after,
        "dump-then-pack must not change a byte of a save holding a geared program"
    );
    assert!(
        parsed
            .creatures
            .iter()
            .any(|c| c.equipment.iter().any(|(_, worn)| worn.item == weapon)),
        "and the program is still wearing it on the other side"
    );
}

/// A v27 dump carries no `equipment` key at all. `savetool pack` is the
/// migration path, so it has to default rather than refuse.
#[test]
fn a_save_dump_without_the_equipment_key_still_packs() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    let path = save_path("v27");
    game.save(&path).unwrap();
    let data = crate::save::load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let text = crate::save::to_ron(&data).unwrap();
    assert!(
        text.contains("equipment: ["),
        "the v28 dump should carry the key at all"
    );
    let v27: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("equipment: ["))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !v27.contains("equipment: ["),
        "the older shape is what is being parsed"
    );

    let parsed = crate::save::from_ron(&v27).expect("a v27-shaped dump must still parse");
    assert!(
        parsed.creatures.iter().all(|c| c.equipment.is_empty()),
        "a program in an older dump wears nothing"
    );
}

/// **The trap this whole feature is built around.** `apply_equipment_delta`
/// writes a gear bonus straight into `Stats`, so an unequip must subtract
/// precisely what the equip added — with rarity as a third scaling axis, a
/// path that computed one side without it would leave the difference welded
/// permanently into the player's base stats, invisibly, because a stat
/// carries no record of where it came from.
///
/// Walks every rung rather than checking one, since the failure is
/// proportional to the tier and a check at Silver would barely move.
#[test]
fn unequipping_a_rare_copy_leaves_no_bonus_behind() {
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    let base = *game.world.get::<Stats>(player).unwrap();

    for rarity in Rarity::ALL {
        let copy = GearCopy {
            rarity,
            tier: 0,
            affixes: Vec::new(),
            ..GearCopy::plain(weapon.clone())
        };
        game.add_copies(&copy, 1);
        game.equip(player, &copy).unwrap();

        let worn = *game.world.get::<Stats>(player).unwrap();
        if rarity != Rarity::Ordinary {
            assert!(
                worn.atk > base.atk,
                "{rarity:?} should be worth something while worn"
            );
        }

        game.unequip(player, EquipmentSlot::Weapon).unwrap();
        let after = *game.world.get::<Stats>(player).unwrap();
        assert_eq!(
            (after.atk, after.mitigation),
            (base.atk, base.mitigation),
            "{rarity:?} left a bonus welded into base stats after the unequip"
        );
        // And the copy came back as itself rather than laundering its tier.
        assert_eq!(game.count_copies(&copy), 1, "{rarity:?} did not come back");
        game.take_copies(&copy, 1);
    }
}

/// A rare copy is strictly better worn than a plain one of the same item, at
/// the same level and fusion tier. Without this the whole axis could be
/// wired up correctly and still be worth nothing, because `for_rarity` is
/// the only thing that makes a tier mean a number.
#[test]
fn a_rare_copy_is_worth_more_worn_than_a_plain_one() {
    let mut game = Game::new(6, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);

    let atk_at = |game: &mut Game, rarity: Rarity| {
        let copy = GearCopy {
            rarity,
            tier: 0,
            affixes: Vec::new(),
            ..GearCopy::plain(weapon.clone())
        };
        game.add_copies(&copy, 1);
        game.equip(player, &copy).unwrap();
        let atk = game.world.get::<Stats>(player).unwrap().atk;
        game.unequip(player, EquipmentSlot::Weapon).unwrap();
        game.take_copies(&copy, 1);
        atk
    };

    let plain = atk_at(&mut game, Rarity::Ordinary);
    for pair in Rarity::ALL.windows(2) {
        assert!(
            atk_at(&mut game, pair[1]) > atk_at(&mut game, pair[0]),
            "{:?} must beat {:?} on the wearer's ATK",
            pair[1],
            pair[0]
        );
    }
    assert!(atk_at(&mut game, Rarity::Prismatic) > plain);
}

/// Fusion matches on the whole copy, so two copies that differ only in rare
/// tier are not two of a thing — see `Game::fuse_item`. The alternative
/// launders a tier into or out of the result depending on which parent won.
#[test]
fn a_rare_copy_will_not_fuse_with_a_plain_one() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let plain = GearCopy::plain(armor.clone());
    let rare = GearCopy {
        rarity: Rarity::Gold,
        tier: 0,
        affixes: Vec::new(),
        ..GearCopy::plain(armor)
    };
    game.add_copies(&plain, 1);
    game.add_copies(&rare, 1);

    assert!(
        game.fuse_item(&rare).is_err(),
        "one Overclocked copy plus one ordinary copy is not two of a thing"
    );
    assert_eq!(
        game.count_copies(&rare),
        1,
        "the refusal must spend nothing"
    );
    assert_eq!(game.count_copies(&plain), 1);

    // Two of the same copy do fuse, and the result keeps the tier.
    game.add_copies(&rare, 1);
    game.fuse_item(&rare).unwrap();
    let fused = GearCopy {
        tier: 1,
        ..rare.clone()
    };
    assert_eq!(
        game.count_copies(&fused),
        1,
        "the fused copy must still be Overclocked"
    );
}

/// **One pass, not a cascade.** Four copies come out as two T1s and stop —
/// the T1s are not fused again into a T2. That falls out of the snapshot
/// `fuse_all_items` iterates rather than being a rule enforced on top: a
/// row it creates is not a row it was handed. The odd copy in the second
/// stack is what pins that a stack is drained in pairs and the remainder
/// is left alone.
#[test]
fn fusing_all_pairs_promotes_every_stack_once() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let core = ItemId::from(ids::OVERCLOCK_CORE);
    let mut game = Game::new(9110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(armor.clone(), 4);
        inv.add(core.clone(), 5);
    }

    game.fuse_all_items().unwrap();

    assert_eq!(held_at(&game, &armor, 1), 2, "four copies buy two T1s");
    assert_eq!(
        held_at(&game, &armor, 2),
        0,
        "and are not fused on into a T2"
    );
    assert_eq!(held_at(&game, &armor, 0), 0, "with nothing left over");
    assert_eq!(held_at(&game, &core, 1), 2, "five copies buy two T1s");
    assert_eq!(
        held_at(&game, &core, 0),
        1,
        "and leave the odd one ordinary"
    );
}

/// The whole reason the key exists: the same fusions pressed one at a time
/// charge a turn each, and a convenience key must not charge the player
/// need decay, sweep pressure and spawn rolls for typing less.
#[test]
fn fusing_all_pairs_costs_one_tick() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(9111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 6);

    let before = game.current_tick();
    game.fuse_all_items().unwrap();

    assert_eq!(held_at(&game, &armor, 1), 3, "three fusions happened");
    assert_eq!(
        game.current_tick() - before,
        1,
        "three fusions in one press cost one turn, not three"
    );
}

/// A refusal spends nothing, the turn included — the same ordering every
/// other refused action keeps.
#[test]
fn fusing_all_with_nothing_to_fuse_spends_no_tick() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(9112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // A lone copy of a piece of gear, plus whatever resources a new game
    // starts with — none of it a matching pair.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 1);

    let before = game.current_tick();
    let err = game.fuse_all_items().unwrap_err();

    assert!(err.contains("matching pair"), "got: {err}");
    assert_eq!(game.current_tick(), before, "a refusal costs no turn");
    assert_eq!(held_at(&game, &armor, 0), 1, "and spends nothing");
}

#[test]
fn fusing_all_is_refused_during_a_battle() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(9113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 4);
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    assert!(game.fuse_all_items().is_err());
    assert_eq!(held_at(&game, &armor, 0), 4, "a refusal spends nothing");
}

/// **Quality is the fourth thing that makes a copy special**, so it joins
/// `is_plain`'s `&&` and with it the choice of cargo store. A copy that
/// compiled off spec is not interchangeable with one that did, which is
/// exactly the question `Inventory`-or-`GearCopies` asks.
#[test]
fn an_off_spec_copy_is_not_plain() {
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let plain = GearCopy::plain(whip);
    assert!(plain.is_plain(), "a copy compiled to spec still stacks");
    assert_eq!(plain.quality, QUALITY_DEFAULT);

    let off_spec = GearCopy {
        quality: QUALITY_DEFAULT - 5,
        ..plain.clone()
    };
    assert!(!off_spec.is_plain());

    let over_spec = GearCopy {
        quality: QUALITY_DEFAULT + 5,
        ..plain
    };
    assert!(!over_spec.is_plain(), "better than spec is special too");
}

/// The store split follows `is_plain` and nothing else, so an off-spec copy
/// lands in the ledger that can tell two copies apart rather than stacking
/// namelessly in `Inventory`.
#[test]
fn an_off_spec_copy_lands_in_the_gear_ledger() {
    let assets = test_assets_dir();
    let mut game = Game::new(211, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let off_spec = GearCopy {
        quality: QUALITY_DEFAULT + 10,
        ..GearCopy::plain(whip.clone())
    };

    game.add_copies(&off_spec, 1);

    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&whip),
        0,
        "an off-spec copy must not stack in the plain-copy store"
    );
    assert_eq!(game.count_copies(&off_spec), 1);
}

/// A worn copy's quality is four flat save fields rather than a nested
/// `GearCopy`, which is the shape `EquippedItemSave`'s doc argues for — so
/// it is also four places the field can be forgotten. This is the test that
/// notices.
#[test]
fn a_worn_off_spec_copy_survives_save_and_load() {
    let assets = test_assets_dir();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let mut game = Game::new(212, DifficultyMode::Forgiving, &assets).unwrap();
    let copy = GearCopy {
        quality: QUALITY_DEFAULT + 15,
        ..GearCopy::plain(whip.clone())
    };
    game.add_copies(&copy, 1);
    game.equip(game.player_entity(), &copy).unwrap();
    let atk_before = game.player_status().atk;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_quality_worn_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let worn = loaded
        .worn(loaded.player_entity(), crate::items::EquipmentSlot::Weapon)
        .expect("the whip is still on");
    assert_eq!(worn.copy.quality, QUALITY_DEFAULT + 15);
    assert_eq!(
        loaded.player_status().atk,
        atk_before,
        "reloading must not change what the worn copy is worth"
    );
}

/// **A save written before this field loads its gear at 100.** That claim
/// is the whole reason the field costs no `SAVE_FORMAT_VERSION` bump, and
/// it is a claim about a file, so this edits one: a real save with every
/// `quality:` line stripped is byte-for-byte what the previous release
/// wrote.
#[test]
fn a_pre_quality_save_loads_its_gear_as_designed() {
    let assets = test_assets_dir();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let plating = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(213, DifficultyMode::Forgiving, &assets).unwrap();
    // One worn (flat save fields) and one carried (serde'd `GearCopy`),
    // because they travel by different routes.
    let worn = GearCopy::plain(whip.clone());
    game.add_copies(&worn, 2);
    game.equip(game.player_entity(), &worn).unwrap();
    // A companion's loadout is the third route: `EquippedItemSave`, with
    // its own `serde` default to get wrong.
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    game.equip(companion, &worn).unwrap();
    let carried = GearCopy {
        tier: 1,
        ..GearCopy::plain(plating.clone())
    };
    game.add_copies(&carried, 2);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_pre_quality_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    // The three player slots spell the field `weapon_quality:` and the rest
    // spell it `quality:`, so the filter matches on the tail of the field
    // name — stripping only the bare form leaves the player's own worn copy
    // reading a field that is still there, which is a vacuous test.
    let stripped: String = text
        .lines()
        .filter(|line| {
            !line
                .trim_start()
                .split(':')
                .next()
                .unwrap_or_default()
                .ends_with("quality")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !stripped.contains("quality"),
        "every route's field has to be gone, or this proves nothing about that route"
    );
    assert!(
        stripped.len() < text.len(),
        "the save has to have carried the field, or this proves nothing"
    );
    std::fs::write(&path, &stripped).unwrap();

    let mut loaded = Game::load(&path, &assets).expect("an older save still loads");
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded
            .worn(loaded.player_entity(), crate::items::EquipmentSlot::Weapon)
            .expect("still wearing it")
            .copy
            .quality,
        QUALITY_DEFAULT,
    );
    let player = loaded.player_entity();
    let companion_worn: Vec<_> = loaded
        .world
        .query::<(Entity, &Equipment)>()
        .iter(&loaded.world)
        .filter(|(e, _)| *e != player)
        .filter_map(|(_, eq)| eq.weapon.clone())
        .collect();
    assert_eq!(companion_worn.len(), 1);
    assert_eq!(companion_worn[0].copy.quality, QUALITY_DEFAULT);
    assert_eq!(
        loaded.count_copies(&carried),
        2,
        "the carried copy is unchanged"
    );
}

/// **A rare tier's floor is guaranteed against a copy of equal quality**,
/// not globally — which is the honest form of the guarantee and the reason
/// the two floored axes stay last in the chain. Swept over the whole band
/// and both ends of the level range.
#[test]
fn a_rarer_copy_beats_an_ordinary_one_of_equal_quality() {
    let assets = test_assets_dir();
    let game = Game::new(214, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);

    for quality in [70u8, 85, QUALITY_DEFAULT, 115, 130] {
        for level in [1u32, 10] {
            let ordinary = GearCopy {
                quality,
                ..GearCopy::plain(whip.clone())
            };
            let silver = GearCopy {
                rarity: Rarity::Silver,
                ..ordinary.clone()
            };
            let plain_stats = game.copy_bonus(&ordinary, level).unwrap();
            let rare_stats = game.copy_bonus(&silver, level).unwrap();
            assert!(
                rare_stats.atk > plain_stats.atk,
                "Silver at {quality}% and level {level} must beat Ordinary at the same quality"
            );
            assert!(rare_stats.damage.max >= plain_stats.damage.max);
        }
    }
}

/// Quality lands *after* level scaling, which is what gives it a number
/// with enough resolution to bite. Applied last instead, a 4-point stat at
/// level 1 would round the whole band flat.
#[test]
fn quality_moves_a_copys_bonus_at_every_level() {
    let assets = test_assets_dir();
    let game = Game::new(215, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let spec = GearCopy::plain(whip.clone());
    let good = GearCopy {
        quality: 130,
        ..spec.clone()
    };
    let poor = GearCopy {
        quality: 70,
        ..spec.clone()
    };

    for level in [1u32, 5, 10] {
        let at_spec = game.copy_bonus(&spec, level).unwrap().atk;
        assert!(game.copy_bonus(&good, level).unwrap().atk > at_spec);
        assert!(game.copy_bonus(&poor, level).unwrap().atk < at_spec);
    }
}

/// The *companion* half of the worn route. A program's loadout rides
/// `EquippedItemSave` rather than `PlayerSave`'s flat fields, so it is a
/// second place a worn copy's quality can be dropped — and neither
/// `worn_to_save` nor that struct's `serde` default is exercised by the
/// player's own gear, which `Game::save` writes field by field.
#[test]
fn a_companions_off_spec_copy_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(216, DifficultyMode::Forgiving, &assets).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    let copy = GearCopy {
        quality: QUALITY_DEFAULT + 15,
        ..GearCopy::plain(ItemId::from(ids::OVERCLOCK_CORE))
    };
    game.add_copies(&copy, 1);
    game.equip(companion, &copy).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_quality_companion_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let player = loaded.player_entity();
    let worn: Vec<_> = loaded
        .world
        .query::<(Entity, &Equipment)>()
        .iter(&loaded.world)
        .filter(|(e, _)| *e != player)
        .filter_map(|(_, eq)| eq.weapon.clone())
        .collect();
    assert_eq!(
        worn.len(),
        1,
        "the companion is still wearing exactly one weapon"
    );
    assert_eq!(worn[0].copy.quality, QUALITY_DEFAULT + 15);
}

/// The inversion `EquipmentStats::for_quality` argues from, pinned. With
/// quality applied *last* a Silver copy at the bottom of the band prices
/// below an Ordinary one at the top, because the rare ladder's floor is
/// added and then multiplied away — and a row colour that says "better"
/// about the worse copy is the failure. Quality third keeps the ladder
/// standing across the whole band.
#[test]
fn the_rare_ladder_survives_the_whole_quality_band() {
    use crate::tuning::{QUALITY_MAX, QUALITY_MIN};
    let assets = test_assets_dir();
    let game = Game::new(217, DifficultyMode::Forgiving, &assets).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let poor_silver = GearCopy {
        rarity: Rarity::Silver,
        quality: QUALITY_MIN,
        ..GearCopy::plain(whip.clone())
    };
    let good_ordinary = GearCopy {
        quality: QUALITY_MAX,
        ..GearCopy::plain(whip.clone())
    };

    assert!(
        game.copy_bonus(&poor_silver, 1).unwrap().atk
            >= game.copy_bonus(&good_ordinary, 1).unwrap().atk,
        "a Silver copy must never price below an Ordinary one at level 1"
    );
}

/// The affix list replaced an `Option`, and this task's whole claim is that
/// nothing moved for the copies that already exist. A one-affix copy built
/// through the new constructor must price and name exactly as the old
/// single-affix copy did — the same `copy_bonus` and the same string.
///
/// The figures are not hardcoded: what is asserted is that one affix's
/// contribution over the plain copy is exactly its authored `stats`, put
/// through the same scaling chain. A hardcoded number would move with any
/// asset edit and say nothing about the flip.
#[test]
fn one_affix_prices_and_names_exactly_as_it_did_before_the_list() {
    let game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let affix = game
        .affix_defs()
        .into_iter()
        .find(|a| a.stats.atk > 0 && a.prefix.is_some())
        .expect("the shipped set has an ATK prefix");
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);

    let plain = GearCopy::plain(weapon.clone());
    let one = GearCopy::with_affixes(
        weapon.clone(),
        Rarity::Ordinary,
        0,
        vec![affix.id.clone()],
        QUALITY_DEFAULT,
    );

    let plain_bonus = game.copy_bonus(&plain, 1).expect("equippable");
    let one_bonus = game.copy_bonus(&one, 1).expect("equippable");
    assert!(
        one_bonus.atk > plain_bonus.atk,
        "the affix must still be worth something: {plain_bonus:?} -> {one_bonus:?}"
    );

    // And the name is the affix's own decoration of the item name, which is
    // what every existing save's affixed copy is already called.
    assert_eq!(
        game.copy_name(&one),
        affix.decorate(game.item_name(&weapon)),
        "a one-affix copy's name moved"
    );
}

/// `[A, B]` and `[B, A]` are the same copy to a player, so they must be the
/// same copy to `Eq` — `GearCopy` is the key of the `GearCopies` ledger and
/// two rows for one thing makes `count_copies` under-report and strands the
/// copies in the row it did not find.
///
/// `with_affixes` sorting on the way in is what holds this, which is why the
/// two lists here are handed over in opposite orders.
#[test]
fn an_affix_list_is_one_row_whatever_order_it_is_built_in() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut ids: Vec<_> = game.affix_defs().into_iter().map(|a| a.id).collect();
    ids.sort();
    let (a, b) = (ids[0].clone(), ids[1].clone());
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);

    let forward = GearCopy::with_affixes(
        weapon.clone(),
        Rarity::Ordinary,
        0,
        vec![a.clone(), b.clone()],
        QUALITY_DEFAULT,
    );
    let backward = GearCopy::with_affixes(weapon, Rarity::Ordinary, 0, vec![b, a], QUALITY_DEFAULT);
    assert_eq!(
        forward, backward,
        "the order a list is built in must not key"
    );

    game.add_copies(&forward, 1);
    game.add_copies(&backward, 1);
    assert_eq!(
        game.count_copies(&forward),
        2,
        "both copies must land in one row"
    );
}

/// A RON round trip cannot catch a `#[serde(skip)]`, so the shim needs a
/// real `Game::save`/`Game::load` pair: a two-affix copy in cargo and a
/// two-affix copy on the player's back both have to come back with both.
#[test]
fn a_two_affix_copy_survives_save_and_load_worn_and_carried() {
    let assets = test_assets_dir();
    let mut game = Game::new(4103, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let mut ids: Vec<_> = game.affix_defs().into_iter().map(|a| a.id).collect();
    ids.sort();
    let pair = vec![ids[0].clone(), ids[1].clone()];

    let carried = GearCopy::with_affixes(
        ItemId::from(ids::ABLATIVE_PLATING),
        Rarity::Gold,
        1,
        pair.clone(),
        QUALITY_DEFAULT,
    );
    let worn = GearCopy::with_affixes(
        ItemId::from(ids::OVERCLOCK_CORE),
        Rarity::Ordinary,
        0,
        pair.clone(),
        QUALITY_DEFAULT,
    );
    game.add_copies(&carried, 2);
    game.add_copies(&worn, 1);
    game.equip(player, &worn).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_affix_list_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.count_copies(&carried),
        2,
        "a carried two-affix copy lost an affix and keyed a different row"
    );
    let back = loaded
        .world
        .get::<Equipment>(loaded.player_entity())
        .and_then(|e| e.weapon.clone())
        .expect("the weapon is still worn");
    assert_eq!(back.copy.affixes, pair, "the worn copy lost its affixes");
}

/// Quality is thirteen buckets, so two field-found copies of one item almost
/// never matched as whole values and fusion stopped firing for anything not
/// crafted or bought. It now goes free, and the result's quality is the two
/// averaged and snapped to a `QUALITY_STEP`.
///
/// Ties go **down**, so no fusion ever buys quality. The table drives both
/// tie cases through it — `(75, 90)` lands exactly between two steps and
/// `(85, 85)` is its own average.
#[test]
fn two_copies_differing_only_in_quality_fuse_to_their_average() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    for (a, b, want) in [
        (75u8, 90u8, 80u8),
        (85, 85, 85),
        (90, 95, 90),
        (70, 130, 100),
    ] {
        let mut game = Game::new(9200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let left = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), a);
        let right = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), b);
        game.add_copies(&left, 1);
        game.add_copies(&right, 1);

        game.fuse_item(&left)
            .unwrap_or_else(|e| panic!("{a}% and {b}% must fuse: {e}"));

        let fused = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 1, Vec::new(), want);
        assert_eq!(
            game.count_copies(&fused),
            1,
            "{a}% + {b}% should land at {want}%, and did not"
        );
    }
}

/// The other half of the feature: a copy carries a list, so a fusion of two
/// affixed copies has somewhere to put the second. The result holds the
/// union, sorted — sorted because the list is the key of the ledger.
#[test]
fn two_copies_with_different_affixes_fuse_and_keep_both() {
    let mut game = Game::new(9201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut ids: Vec<_> = game.affix_defs().into_iter().map(|a| a.id).collect();
    ids.sort();
    let (a, b) = (ids[0].clone(), ids[1].clone());

    let left = GearCopy::with_affixes(
        armor.clone(),
        Rarity::Ordinary,
        0,
        vec![a.clone()],
        QUALITY_DEFAULT,
    );
    let right = GearCopy::with_affixes(
        armor.clone(),
        Rarity::Ordinary,
        0,
        vec![b.clone()],
        QUALITY_DEFAULT,
    );
    game.add_copies(&left, 1);
    game.add_copies(&right, 1);

    game.fuse_item(&left).expect("two affixed copies must fuse");

    let fused = GearCopy::with_affixes(armor, Rarity::Ordinary, 1, vec![a, b], QUALITY_DEFAULT);
    assert_eq!(
        game.count_copies(&fused),
        1,
        "the result must carry both affixes"
    );
}

/// The duplicate case, and the one the feature was asked for: two copies
/// carrying the *same* affix fuse into one carrying it **twice**. Deduping
/// here would make the second copy's affix worth nothing.
#[test]
fn two_copies_with_the_same_affix_fuse_into_one_carrying_it_twice() {
    let mut game = Game::new(9202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let affix = game
        .affix_defs()
        .into_iter()
        .find(|a| a.fits(EquipmentSlot::Armor) && a.stats.mitigation > 0)
        .expect("the shipped set has an armour affix worth mitigation");

    let one = GearCopy::with_affixes(
        armor.clone(),
        Rarity::Ordinary,
        0,
        vec![affix.id.clone()],
        QUALITY_DEFAULT,
    );
    game.add_copies(&one, 2);
    game.fuse_item(&one)
        .expect("two identical copies must fuse");

    let twice = GearCopy::with_affixes(
        armor.clone(),
        Rarity::Ordinary,
        1,
        vec![affix.id.clone(), affix.id.clone()],
        QUALITY_DEFAULT,
    );
    assert_eq!(
        game.count_copies(&twice),
        1,
        "the affix must be carried twice, not deduped"
    );

    // And it is worth twice as much, which is what makes the duplicate a
    // feature rather than a row on a screen.
    let once_t1 = GearCopy::with_affixes(
        armor,
        Rarity::Ordinary,
        1,
        vec![affix.id.clone()],
        QUALITY_DEFAULT,
    );
    let doubled = game.copy_bonus(&twice, 1).expect("equippable");
    let single = game.copy_bonus(&once_t1, 1).expect("equippable");
    assert!(
        doubled.mitigation > single.mitigation,
        "a duplicated affix must count twice: {single:?} against {doubled:?}"
    );
}

/// The partner is picked automatically, so which one it picks is the whole
/// of what a player gets for their spare. It is the **highest quality**
/// eligible copy.
///
/// Three spares at different qualities, because with only one other copy in
/// cargo "it picked the only candidate" passes and proves nothing.
#[test]
fn the_partner_chosen_is_the_highest_quality_spare() {
    let mut game = Game::new(9203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let survivor = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), 100);
    let copy_at = |q| GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), q);

    game.add_copies(&survivor, 1);
    for q in [75u8, 130, 90] {
        game.add_copies(&copy_at(q), 1);
    }

    game.fuse_item(&survivor).expect("a partner is available");

    // 100 and 130 average to 115. Anything else means a lesser spare was
    // taken.
    let fused = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 1, Vec::new(), 115);
    assert_eq!(
        game.count_copies(&fused),
        1,
        "the 130% spare should have been consumed"
    );
    assert_eq!(game.count_copies(&copy_at(130)), 0, "and be gone");
    assert_eq!(game.count_copies(&copy_at(75)), 1, "the 75% spare survives");
    assert_eq!(game.count_copies(&copy_at(90)), 1, "so does the 90% one");
}

/// A worn copy that is eligible but not *equal* is folded in — the survivor
/// is the copy `[U]` was pressed on, and the result takes the slot so the
/// player is never left bare.
///
/// Asserted on `Equipment` rather than on the log line: what matters is what
/// is on the player's back, and a line saying so proves nothing about it.
#[test]
fn an_eligible_worn_copy_is_folded_in_and_the_result_takes_the_slot() {
    let mut game = Game::new(9204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);

    let worn = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), 90);
    game.add_copies(&worn, 1);
    game.equip(player, &worn).unwrap();

    let spare = GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), 130);
    game.add_copies(&spare, 1);

    game.fuse_item(&spare)
        .expect("the worn copy is an eligible partner");

    let back = game
        .world
        .get::<Equipment>(player)
        .and_then(|e| e.armor.clone())
        .expect("the player must not be left bare");
    assert_eq!(back.copy.tier, 1, "the result takes the slot: {back:?}");
    assert_eq!(back.copy.quality, 110, "90% and 130% average to 110%");
    assert_eq!(
        game.count_copies(&spare),
        0,
        "the survivor was drawn from cargo"
    );
    assert_eq!(
        game.count_copies(&GearCopy::with_affixes(
            armor,
            Rarity::Ordinary,
            1,
            Vec::new(),
            110
        )),
        0,
        "and the result is worn, not carried"
    );
}

/// `fuse_all_items` needs no rule of its own — it iterates a snapshot taken
/// before any fusing starts, so it pairs greedily inside a group and still
/// cannot cascade. Four tier-0 copies at four different qualities come out
/// as two tier-1s, not one tier-2.
#[test]
fn fusing_all_pairs_crosses_quality_in_one_pass() {
    let mut game = Game::new(9205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    for q in [70u8, 85, 100, 130] {
        game.add_copies(
            &GearCopy::with_affixes(armor.clone(), Rarity::Ordinary, 0, Vec::new(), q),
            1,
        );
    }

    game.fuse_all_items()
        .expect("four mismatched copies are two pairs now");

    let tier_1: u32 = (QUALITY_MIN..=QUALITY_MAX)
        .step_by(crate::tuning::QUALITY_STEP as usize)
        .map(|q| {
            game.count_copies(&GearCopy::with_affixes(
                armor.clone(),
                Rarity::Ordinary,
                1,
                Vec::new(),
                q,
            ))
        })
        .sum();
    assert_eq!(tier_1, 2, "four copies buy two T1s");

    let tier_2: u32 = (QUALITY_MIN..=QUALITY_MAX)
        .step_by(crate::tuning::QUALITY_STEP as usize)
        .map(|q| {
            game.count_copies(&GearCopy::with_affixes(
                armor.clone(),
                Rarity::Ordinary,
                2,
                Vec::new(),
                q,
            ))
        })
        .sum();
    assert_eq!(tier_2, 0, "and are not fused on into a T2");

    let tier_0: u32 = (QUALITY_MIN..=QUALITY_MAX)
        .step_by(crate::tuning::QUALITY_STEP as usize)
        .map(|q| {
            game.count_copies(&GearCopy::with_affixes(
                armor.clone(),
                Rarity::Ordinary,
                0,
                Vec::new(),
                q,
            ))
        })
        .sum();
    assert_eq!(tier_0, 0, "with nothing left over");
}
