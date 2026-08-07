//! Equipping, unequipping, and fusing gear.

use super::support::*;
use crate::tuning::MAX_FUSIONS;
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

    game.equip(&ItemId::from(ids::OVERCLOCK_CORE), 0).unwrap();

    let status = game.player_status();
    assert_eq!(
        status.atk,
        atk_before + 3,
        "weapon should grant its Attack bonus"
    );
    assert_eq!(
        status.weapon,
        Some(EquippedItem {
            item: ItemId::from(ids::OVERCLOCK_CORE),
            level: 1,
            fusion_tier: 0
        })
    );
    assert!(
        status
            .inventory
            .iter()
            .all(|r| r.item != ItemId::from(ids::OVERCLOCK_CORE)),
        "equipped item should leave the inventory stack"
    );
}

#[test]
fn equipping_gear_in_a_deeper_zone_scales_its_bonus_100_percent_per_level() {
    let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    let atk_before = game.player_status().atk;

    game.equip(&ItemId::from(ids::OVERCLOCK_CORE), 0).unwrap();

    let status = game.player_status();
    // Base +3 ATK, scaled 2x per level above 1: level 3 = 3 * 2^2 = 12.
    assert_eq!(
        status.atk,
        atk_before + 12,
        "gear equipped at zone level 3 should be scaled 2x per level"
    );
    assert_eq!(
        status.weapon,
        Some(EquippedItem {
            item: ItemId::from(ids::OVERCLOCK_CORE),
            level: 3,
            fusion_tier: 0
        })
    );

    game.unequip(EquipmentSlot::Weapon).unwrap();
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

    game.equip(&ItemId::from(ids::OVERCLOCK_CORE), 0).unwrap();
    assert_eq!(game.player_status().atk, atk_before + 3);

    // Equipping into an already-occupied slot swaps the old item back
    // to inventory and must not stack the bonus a second time.
    game.equip(&ItemId::from(ids::OVERCLOCK_CORE), 0).unwrap();
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
            .find(|r| r.item == ItemId::from(ids::OVERCLOCK_CORE))
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
    let def_before = game.player_status().def;
    game.equip(&ItemId::from(ids::FIREWALL_PLATING), 0).unwrap();
    assert_eq!(game.player_status().def, def_before + 3);

    game.unequip(EquipmentSlot::Armor).unwrap();

    let status = game.player_status();
    assert_eq!(status.def, def_before, "unequip should remove the bonus");
    assert_eq!(status.armor, None);
    assert_eq!(
        status
            .inventory
            .iter()
            .find(|r| r.item == ItemId::from(ids::FIREWALL_PLATING))
            .map(|r| r.qty),
        Some(1)
    );
}

#[test]
fn unequip_errors_on_an_empty_slot() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.unequip(EquipmentSlot::Weapon).is_err());
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
        item: broken.clone(),
        level: 1,
        fusion_tier: 0,
    });
    let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();
    let stats_before = {
        let stats = game.world.get::<Stats>(player).unwrap();
        (stats.atk, stats.def)
    };
    let decompiler_before = game.world.get::<Decompiler>(player).map(|d| d.skill);

    let result = game.unequip(EquipmentSlot::Weapon);

    assert!(
        result.is_err(),
        "unequipping an item absent from ItemDb should error, not panic"
    );
    assert_eq!(
        game.player_status().weapon.map(|eq| eq.item),
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
        (stats_after.atk, stats_after.def),
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
        item: broken.clone(),
        level: 1,
        fusion_tier: 0,
    });
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();
    let stats_before = {
        let stats = game.world.get::<Stats>(player).unwrap();
        (stats.atk, stats.def)
    };
    let decompiler_before = game.world.get::<Decompiler>(player).map(|d| d.skill);

    let result = game.equip(&ItemId::from(ids::OVERCLOCK_CORE), 0);

    assert!(
        result.is_err(),
        "equipping over a slot whose old item is absent from ItemDb should error, not panic"
    );
    assert_eq!(
        game.player_status().weapon.map(|eq| eq.item),
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
        (stats_after.atk, stats_after.def),
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

    game.fuse_item(&armor, 0).unwrap();

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
            game.fuse_item(&armor, tier).unwrap();
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

    let err = game.fuse_item(&armor, MAX_FUSIONS).unwrap_err();
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
    // Ablative Plating's base is +4 def. At this magnitude the percentage
    // is the smaller of the two terms — 4 * 1.2 = 4.8 -> 5 against a floor
    // of 4 + 2 — so what this pins is that the two combine by taking the
    // better, not that the percentage alone is doing the work.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 6);

    let def_before = game.player_status().def;
    game.equip(&armor, 0).unwrap();
    assert_eq!(
        game.player_status().def,
        def_before + 4,
        "unfused equip should grant the plain base bonus"
    );
    game.unequip(EquipmentSlot::Armor).unwrap();

    // Four base copies for one T2: two T1s, then those two.
    game.fuse_item(&armor, 0).unwrap();
    game.fuse_item(&armor, 0).unwrap();
    game.fuse_item(&armor, 1).unwrap();
    assert_eq!(held_at(&game, &armor, 2), 1);

    game.equip(&armor, 2).unwrap();
    assert_eq!(
        game.player_status().def,
        def_before + 6,
        "tier 2: the +1/tier floor (4 + 2 = 6) beats the +20% (4.8 -> 5)"
    );
}

#[test]
fn fuse_item_rejects_non_equipment_and_insufficient_stock() {
    let mut game = Game::new(202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(
        game.fuse_item(&ItemId::from(ids::CORE_FRAGMENT), 0)
            .is_err(),
        "plain resources aren't equipment and can't be fused"
    );

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    assert!(
        game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE), 0)
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
        .get_mut::<FusedGear>(player)
        .unwrap()
        .add(core.clone(), MAX_FUSIONS, 2);

    // Refused by the ceiling, with the stock for it plainly in hand.
    let err = game.fuse_item(&core, MAX_FUSIONS).unwrap_err();
    assert!(err.contains("can't be fused again"), "got: {err}");
    // Refused by the stock, one rung down where there is none.
    assert!(game.fuse_item(&core, MAX_FUSIONS - 1).is_err());

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
    let base_def = game.player_status().def;

    game.equip(&armor, 0).unwrap();
    assert_eq!(
        game.player_status().def,
        base_def + 4,
        "Ablative Plating's base is +4 def while worn, unfused"
    );
    assert_eq!(
        held_at(&game, &armor, 0),
        3,
        "equipping consumed one of the four copies"
    );

    // The worn copy counts as one of the two a fusion needs, so a single
    // spare is enough — and it is the worn copy that comes out stronger.
    game.fuse_item(&armor, 0).unwrap();
    assert_eq!(
        held_at(&game, &armor, 0),
        2,
        "only one spare consumed — the worn copy counted for the other"
    );
    assert_eq!(
        game.player_status().armor.map(|e| e.fusion_tier),
        Some(1),
        "the worn copy is the survivor"
    );

    // The worn copy is a T1 now, so the next rung needs a T1 spare —
    // which the two remaining ordinary copies make.
    game.fuse_item(&armor, 0).unwrap();
    assert_eq!(held_at(&game, &armor, 1), 1);
    game.fuse_item(&armor, 1).unwrap();
    assert_eq!(held_at(&game, &armor, 1), 0);
    assert_eq!(
        game.player_status().def,
        base_def + 6,
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
        .get_mut::<FusedGear>(player)
        .unwrap()
        .add(armor.clone(), 1, 1);
    game.equip(&armor, 0).unwrap();

    let err = game.fuse_item(&armor, 1).unwrap_err();
    assert_eq!(err, "Need 2 Ablative Plating to fuse (have 1).");
}

#[test]
fn fusing_a_worn_item_still_needs_one_spare() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 1);
    game.equip(&armor, 0).unwrap(); // now zero spares held
    let err = game.fuse_item(&armor, 0).unwrap_err();
    assert_eq!(err, "Need 1 Ablative Plating to fuse (have 0).");
    assert_eq!(
        game.player_status().armor.map(|e| e.fusion_tier),
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
    game.equip(&worn, 0).unwrap(); // Firewall Plating occupies the Armor slot
    // The worn armor is a different item, so it can't count toward fusing
    // Ablative Plating — that still needs two spares.
    let err = game.fuse_item(&target, 0).unwrap_err();
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
    let msg = game.fuse_item(&core, 0).unwrap();
    assert!(
        msg.contains("fuse") && msg.contains('%'),
        "a fuse must hand back a confirmation to surface, got: {msg}"
    );
}

/// Unequipping puts the copy back where its tier belongs. Returning a
/// fused copy to `Inventory` would launder it into the tier-0 store the
/// production chain reads, which is the one thing `FusedGear` exists to
/// prevent.
#[test]
fn unequipping_a_fused_copy_returns_it_to_the_fused_store() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(7071, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<FusedGear>(player)
        .unwrap()
        .add(armor.clone(), 2, 1);

    game.equip(&armor, 2).unwrap();
    game.unequip(EquipmentSlot::Armor).unwrap();

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
    game.fuse_item(&core, 0).unwrap();

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
        .get_mut::<FusedGear>(player)
        .unwrap()
        .add(armor.clone(), legacy, 2);
    game.equip(&armor, legacy).unwrap();
    let def_before = game.player_status().def;

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
            .map(|e| e.fusion_tier),
        Some(legacy),
        "the worn copy keeps the tier its bonus was actually applied at"
    );
    assert_eq!(
        loaded.player_status().def,
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

    game.erase_item(&ItemId::from(ids::NEURAL_AMPLIFIER), 0, 3)
        .unwrap();
    assert!(
        game.player_status()
            .inventory
            .iter()
            .all(|r| r.item != ItemId::from(ids::NEURAL_AMPLIFIER))
    );

    assert!(
        game.erase_item(&ItemId::from(ids::NEURAL_AMPLIFIER), 0, 1)
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
    game.equip(&ItemId::from(ids::NEURAL_AMPLIFIER), 0).unwrap();
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
            item: ItemId::from(ids::NEURAL_AMPLIFIER),
            level: 1,
            fusion_tier: 0
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

    game.fuse_item(&armor, 0).unwrap();

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
            game.equip(&armor, 0).unwrap();
        }
        game.fuse_item(&armor, 0).unwrap();
        // Every tier, because a fusion moves a copy between the two stores.
        // Equipping moves one out of cargo entirely, so count it back in to
        // compare total copies owned rather than cargo alone.
        let in_cargo: u32 = game
            .player_status()
            .inventory
            .iter()
            .filter(|r| r.item == armor)
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
        def: 1,
        decompiler: 0,
    };

    let t1 = base.fused_for_tier(1);
    assert_eq!(
        t1.atk, 5,
        "4 at tier 1 must gain a point, not round back to 4"
    );
    assert_eq!(t1.def, 2, "the floor applies to every non-zero stat");
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
