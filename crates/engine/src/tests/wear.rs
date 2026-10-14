//! Gear wear: storage, save routes, and (further down) wearing, breaking and
//! the Refit Bench.

use super::support::*;
use crate::items::EquipmentSlot;
use crate::tuning::GEAR_DURABILITY;
use crate::*;

fn worn_copy(item: &str, wear: u8) -> GearCopy {
    GearCopy {
        wear,
        ..GearCopy::plain(ItemId::from(item))
    }
}

#[test]
fn condition_counts_down_from_the_default_durability() {
    let game = Game::new(5001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let fresh = worn_copy(ids::MONOFILAMENT_WHIP, 0);
    assert_eq!(
        game.condition_of(&fresh),
        (GEAR_DURABILITY, GEAR_DURABILITY)
    );
    let used = worn_copy(ids::MONOFILAMENT_WHIP, 4);
    assert_eq!(
        game.condition_of(&used),
        (GEAR_DURABILITY - 4, GEAR_DURABILITY)
    );
    let spent = worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY + 9);
    assert_eq!(game.condition_of(&spent).0, 0, "never underflows");
}

#[test]
fn an_items_authored_durability_overrides_the_default() {
    let assets = assets_dir_with_extra_item(
        "fragile_blade",
        "fragile_blade.ron",
        r#"(
    id: "fragile_blade",
    name: "Fragile Blade",
    description: "Breaks fast.",
    value: Some(3),
    equipment: Some((Weapon, (atk: 1))),
    durability: Some(3),
)"#,
    );
    let game = Game::new(5002, DifficultyMode::Forgiving, &assets).unwrap();
    let copy = worn_copy("fragile_blade", 1);
    assert_eq!(game.condition_of(&copy), (2, 3));
}

#[test]
fn copies_of_differing_wear_do_not_stack() {
    let mut game = Game::new(5003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let fresh = worn_copy(ids::MONOFILAMENT_WHIP, 0);
    let used = worn_copy(ids::MONOFILAMENT_WHIP, 2);
    assert!(fresh.is_plain());
    assert!(!used.is_plain(), "worn copies leave the plain store");
    game.add_copies(&fresh, 1);
    game.add_copies(&used, 1);
    game.add_copies(&used, 1);
    assert_eq!(game.count_copies(&fresh), 1);
    assert_eq!(game.count_copies(&used), 2);
    assert_eq!(game.take_copies(&used, 2), 2);
    assert_eq!(game.count_copies(&fresh), 1, "the fresh copy is untouched");
}

/// Wear travels four ways: the player's flat slot fields, a companion's
/// `EquippedItemSave`, a carried `GearCopySave`, and a trader's buyback
/// shelf. A RON round trip cannot see a route that skips the field, so this
/// goes through `Game::save` and `Game::load`.
fn a_game_with_worn_gear_on_every_route(seed: u32) -> (Game, GearCopy, GearCopy, GearCopy) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let worn = worn_copy(ids::MONOFILAMENT_WHIP, 4);
    game.add_copies(&worn, 2);
    game.equip(game.player_entity(), &worn).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.equip(companion, &worn).unwrap();
    // Fused, so it is still non-plain once its wear is stripped away and a
    // legacy save's row lands in the store `count_copies` reads.
    let carried = GearCopy {
        tier: 1,
        ..worn_copy(ids::ABLATIVE_PLATING, 7)
    };
    game.add_copies(&carried, 2);
    let sold = worn_copy(ids::FIREWALL_PLATING, 9);
    let market = spawn_market(&mut game);
    game.add_copies(&sold, 1);
    game.sell_item(market, sold.clone(), 1).unwrap();
    (game, worn, carried, sold)
}

#[test]
fn save_and_load_keep_wear_on_every_route() {
    let (mut game, worn, carried, sold) = a_game_with_worn_gear_on_every_route(5010);
    let path =
        std::env::temp_dir().join(format!("feral_processes_wear_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let player = loaded.player_entity();
    assert_eq!(
        loaded.worn(player, EquipmentSlot::Weapon).unwrap().copy,
        worn,
        "the player's flat slot fields dropped wear"
    );
    let companion_worn: Vec<_> = loaded
        .world
        .iter_entities()
        .filter(|e| e.contains::<Tamed>())
        .filter_map(|e| e.get::<Equipment>())
        .filter_map(|eq| eq.weapon.clone())
        .collect();
    assert_eq!(companion_worn.len(), 1);
    assert_eq!(
        companion_worn[0].copy, worn,
        "EquippedItemSave dropped wear"
    );
    assert_eq!(
        loaded.count_copies(&carried),
        2,
        "GearCopySave dropped wear"
    );
    let shelved: Vec<GearCopy> = loaded
        .world
        .resource::<crate::resources::BuybackLedger>()
        .0
        .values()
        .flatten()
        .map(|(c, _)| c.clone())
        .collect();
    assert_eq!(shelved, vec![sold], "the buyback shelf dropped wear");
}

#[test]
fn a_save_without_the_wear_field_loads_every_copy_at_full_condition() {
    let (mut game, worn, carried, sold) = a_game_with_worn_gear_on_every_route(5011);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_pre_wear_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    // The player's slots spell it `weapon_wear:` and the rest `wear:`, so
    // match on the tail of the key — stripping only the bare form leaves the
    // player's own slot reading a field that is still there.
    let stripped: String = text
        .lines()
        .filter(|l| {
            !l.trim_start()
                .split(':')
                .next()
                .unwrap_or_default()
                .ends_with("wear")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.len() < text.len(),
        "the save carried no wear field"
    );
    assert!(!stripped.contains("wear:"), "every route's field is gone");
    std::fs::write(&path, &stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).expect("an older save still loads");
    let _ = std::fs::remove_file(&path);

    let player = loaded.player_entity();
    let full = |c: &GearCopy| GearCopy {
        wear: 0,
        ..c.clone()
    };
    assert_eq!(
        loaded.worn(player, EquipmentSlot::Weapon).unwrap().copy,
        full(&worn)
    );
    assert_eq!(loaded.count_copies(&full(&carried)), 2);
    let shelved: Vec<GearCopy> = loaded
        .world
        .resource::<crate::resources::BuybackLedger>()
        .0
        .values()
        .flatten()
        .map(|(c, _)| c.clone())
        .collect();
    assert_eq!(shelved, vec![full(&sold)]);
}

/// A mod re-keys the copy, and the new key has to keep the condition the old
/// one had, or a Mod Bench would double as a free repair.
#[test]
fn fitting_an_affix_keeps_the_copys_wear_carried_or_worn() {
    let dir = modded_assets_dir("wear_mods", &[], &[], &[], &[], &[]);
    std::fs::write(
        dir.join("affixes").join("t_plate.ron"),
        r#"(id: "t_plate", prefix: Some("Plated"), stats: (mitigation: 6),
    slots: Some([Armor]),
    research: Some((cost: 1, apply_cost: [("core_fragment", 2)])))"#,
    )
    .unwrap();
    let mut game = Game::new(5020, DifficultyMode::Forgiving, &dir).unwrap();
    spawn_structure_at(&mut game, "mod_bench", 3, 3);
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .insert("affix:t_plate".to_string());
    give(&mut game, &ItemId::from("core_fragment"), 20);
    let affix_id = crate::affixes::AffixId::from("t_plate");

    let carried = GearCopy {
        tier: 1,
        ..worn_copy(ids::FIREWALL_PLATING, 6)
    };
    game.add_copies(&carried, 1);
    let (modded, _) = game.apply_affix(&carried, &affix_id).unwrap();
    assert_eq!(modded.wear, 6, "a carried copy lost its wear to the mod");

    game.equip(game.player_entity(), &modded).unwrap();
    let (stripped, _) = game.remove_affix(&modded, &affix_id).unwrap();
    assert_eq!(stripped.wear, 6, "a worn copy lost its wear to the strip");
    let worn = game
        .worn(game.player_entity(), EquipmentSlot::Armor)
        .unwrap();
    assert_eq!(worn.copy, stripped);
}
