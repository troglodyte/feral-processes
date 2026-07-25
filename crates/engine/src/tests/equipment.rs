//! Equipping, unequipping, and fusing gear.

use super::support::*;
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

    game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

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
            .all(|(i, _)| *i != ItemId::from(ids::OVERCLOCK_CORE)),
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

    game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

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

    game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
    assert_eq!(game.player_status().atk, atk_before + 3);

    // Equipping into an already-occupied slot swaps the old item back
    // to inventory and must not stack the bonus a second time.
    game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
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
            .find(|(i, _)| *i == ItemId::from(ids::OVERCLOCK_CORE))
            .map(|(_, q)| *q),
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
    game.equip(&ItemId::from(ids::FIREWALL_PLATING)).unwrap();
    assert_eq!(game.player_status().def, def_before + 3);

    game.unequip(EquipmentSlot::Armor).unwrap();

    let status = game.player_status();
    assert_eq!(status.def, def_before, "unequip should remove the bonus");
    assert_eq!(status.armor, None);
    assert_eq!(
        status
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::from(ids::FIREWALL_PLATING))
            .map(|(_, q)| *q),
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

    let result = game.equip(&ItemId::from(ids::OVERCLOCK_CORE));

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

#[test]
fn fuse_item_consumes_two_copies_and_raises_the_fusion_tier() {
    let mut game = Game::new(200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 3);

    game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

    assert_eq!(game.item_fusion_tier(&ItemId::from(ids::OVERCLOCK_CORE)), 1);
    assert_eq!(
        game.player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::from(ids::OVERCLOCK_CORE))
            .map(|(_, q)| *q),
        Some(1),
        "fusing should consume 2 of the 3 copies"
    );
}

#[test]
fn fuse_item_bonus_scales_the_equipped_stat_bonus() {
    let mut game = Game::new(201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Ablative Plating's base is +4 def, so a 10%/tier bonus is visible
    // (unlike a +3 item, where 10% rounds away to nothing at tier 1).
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ABLATIVE_PLATING), 6);

    let def_before = game.player_status().def;
    game.equip(&ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
    assert_eq!(
        game.player_status().def,
        def_before + 4,
        "unfused equip should grant the plain base bonus"
    );
    game.unequip(EquipmentSlot::Armor).unwrap();

    game.fuse_item(&ItemId::from(ids::ABLATIVE_PLATING))
        .unwrap();
    game.fuse_item(&ItemId::from(ids::ABLATIVE_PLATING))
        .unwrap();
    assert_eq!(
        game.item_fusion_tier(&ItemId::from(ids::ABLATIVE_PLATING)),
        2
    );

    game.equip(&ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
    assert_eq!(
        game.player_status().def,
        def_before + 5,
        "tier 2 is +20%: 4 * 1.2 = 4.8, rounds to 5"
    );
}

#[test]
fn fuse_item_rejects_non_equipment_and_insufficient_stock() {
    let mut game = Game::new(202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(
        game.fuse_item(&ItemId::from(ids::CORE_FRAGMENT)).is_err(),
        "plain resources aren't equipment and can't be fused"
    );

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    assert!(
        game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE)).is_err(),
        "fusing needs 2 copies, only 1 is available"
    );
    assert_eq!(
        game.player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::from(ids::OVERCLOCK_CORE))
            .map(|(_, q)| *q),
        Some(1),
        "a failed fuse should not consume the lone copy"
    );
}

#[test]
fn fusing_a_worn_item_counts_it_and_upgrades_the_worn_copy_live() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // One copy to wear, two spares.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 3);
    let base_def = game.player_status().def;

    game.equip(&armor).unwrap();
    assert_eq!(
        game.player_status().def,
        base_def + 4,
        "Ablative Plating's base is +4 def while worn, unfused"
    );

    let held = |g: &Game| {
        g.player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == armor)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    };
    assert_eq!(held(&game), 2, "equipping consumed one of the three copies");

    // The worn copy counts as one of the two a fusion needs, so a single
    // spare is enough.
    game.fuse_item(&armor).unwrap();
    assert_eq!(game.item_fusion_tier(&armor), 1);
    assert_eq!(
        held(&game),
        1,
        "only one spare consumed — the worn copy counted for the other"
    );

    // Second fuse reaches tier 2, where +20% is visible: 4 * 1.2 = 4.8 -> 5.
    game.fuse_item(&armor).unwrap();
    assert_eq!(game.item_fusion_tier(&armor), 2);
    assert_eq!(held(&game), 0);
    assert_eq!(
        game.player_status().def,
        base_def + 5,
        "the worn copy picks up the new tier live, without a re-equip"
    );
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
    game.equip(&armor).unwrap(); // now zero spares held
    let err = game.fuse_item(&armor).unwrap_err();
    assert_eq!(err, "Need 1 Ablative Plating to fuse (have 0).");
    assert_eq!(
        game.item_fusion_tier(&armor),
        0,
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
    game.equip(&worn).unwrap(); // Firewall Plating occupies the Armor slot
    // The worn armor is a different item, so it can't count toward fusing
    // Ablative Plating — that still needs two spares.
    let err = game.fuse_item(&target).unwrap_err();
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
    let msg = game.fuse_item(&core).unwrap();
    assert!(
        msg.contains("fuse") && msg.contains('%'),
        "a fuse must hand back a confirmation to surface, got: {msg}"
    );
}

#[test]
fn item_fusion_tier_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(203, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 2);
    game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_fusion_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.item_fusion_tier(&ItemId::from(ids::OVERCLOCK_CORE)),
        1
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

    game.erase_item(&ItemId::from(ids::NEURAL_AMPLIFIER), 3)
        .unwrap();
    assert!(
        game.player_status()
            .inventory
            .iter()
            .all(|(i, _)| *i != ItemId::from(ids::NEURAL_AMPLIFIER))
    );

    assert!(
        game.erase_item(&ItemId::from(ids::NEURAL_AMPLIFIER), 1)
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
    game.equip(&ItemId::from(ids::NEURAL_AMPLIFIER)).unwrap();
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
