//! Gear wear: storage, save routes, and (further down) wearing, breaking and
//! the Refit Bench.

use super::support::*;
use crate::items::EquipmentSlot;
use crate::resources::{BattleState, Sorties};
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

// ------------------------------------------------------------ wearing

fn player_weapon(game: &Game) -> GearCopy {
    game.worn(game.player_entity(), EquipmentSlot::Weapon)
        .expect("the player wears a weapon")
        .copy
}

fn wear_the_whip(game: &mut Game, wearer: Entity, wear: u8) -> GearCopy {
    let copy = worn_copy(ids::MONOFILAMENT_WHIP, wear);
    game.add_copies(&copy, 1);
    game.equip(wearer, &copy).unwrap();
    copy
}

/// One abstract fight against a lone hostile, then torn down.
fn lose_an_abstract_fight(game: &mut Game) {
    battle_with_a_pack_of(game, 1, 10);
    let player = game.player_entity();
    game.end_battle(player, None);
}

fn win_an_abstract_fight(game: &mut Game) {
    battle_with_a_pack_of(game, 1, 10);
    game.world.resource_mut::<BattleState>().groups.clear();
    let player = game.player_entity();
    game.end_battle(player, None);
}

#[test]
fn an_abstract_fight_wears_the_player_and_the_party_win_or_lose() {
    let mut game = Game::new(5030, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    wear_the_whip(&mut game, player, 0);
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    wear_the_whip(&mut game, companion, 2);

    lose_an_abstract_fight(&mut game);
    assert_eq!(player_weapon(&game).wear, 1, "a lost fight still wears");
    assert_eq!(
        game.worn(companion, EquipmentSlot::Weapon)
            .unwrap()
            .copy
            .wear,
        3
    );

    win_an_abstract_fight(&mut game);
    assert_eq!(player_weapon(&game).wear, 2, "a won fight wears too");
}

#[test]
fn a_tactical_fight_wears_the_player() {
    let mut game = Game::new(5031, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    wear_the_whip(&mut game, player, 0);
    super::tactical::win_a_lone_tactical_fight(&mut game);
    assert_eq!(player_weapon(&game).wear, 1);
}

#[test]
fn a_sortie_battle_wears_its_squad() {
    let (mut game, squad) = super::sorties::a_dispatched_sortie(5032, DifficultyMode::Forgiving);
    for &member in &squad {
        wear_the_whip(&mut game, member, 0);
        // Sturdy enough that nobody falls and the arithmetic is plain.
        let mut stats = game.world.get_mut::<Stats>(member).unwrap();
        stats.hp = 5_000;
        stats.max_hp = 5_000;
    }
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed =
        game.world.resource::<Sorties>().0[0].ticks_total / 2;
    game.run_sorties();

    let done = game.world.resource::<Sorties>().0[0].battles_done;
    assert!(done > 0, "a battle must have fired");
    for &member in &squad {
        assert_eq!(
            game.worn(member, EquipmentSlot::Weapon).unwrap().copy.wear,
            done as u8
        );
    }
}

/// **Wear comes before `bench_or_dissolve`**, which strips a fallen
/// member's gear into the pack. Wear taken afterwards would miss the copy
/// that just took the hits and hand it back fresh.
#[test]
fn a_sortie_casualtys_gear_is_worn_before_it_is_returned() {
    let (mut game, squad) = super::sorties::a_dispatched_sortie(5033, DifficultyMode::Forgiving);
    let fallen = squad[0];
    wear_the_whip(&mut game, fallen, 0);
    for &member in &squad {
        game.world.get_mut::<Stats>(member).unwrap().hp = 1;
    }
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed =
        game.world.resource::<Sorties>().0[0].ticks_total / 2;
    game.run_sorties();
    assert!(
        game.world.resource::<Sorties>().0[0].aborted,
        "someone fell"
    );

    assert_eq!(
        game.count_copies(&worn_copy(ids::MONOFILAMENT_WHIP, 1)),
        1,
        "the casualty's whip came home worn once, not fresh"
    );
}

#[test]
fn a_party_member_killed_in_the_fight_has_its_gear_worn_and_returned() {
    let mut game = Game::new(5034, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    wear_the_whip(&mut game, companion, 0);
    battle_with_a_pack_of(&mut game, 1, 10);
    game.world.get_mut::<Stats>(companion).unwrap().hp = 0;
    let player = game.player_entity();
    game.end_battle(player, None);

    assert_eq!(
        game.count_copies(&worn_copy(ids::MONOFILAMENT_WHIP, 1)),
        1,
        "the fallen program's gear is returned, worn once"
    );
}

#[test]
fn reinforcements_and_forks_do_not_wear() {
    let mut game = Game::new(5035, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let trooper = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, trooper);
    wear_the_whip(&mut game, trooper, 0);
    game.world
        .entity_mut(trooper)
        .insert(crate::components::Reinforcement { reorienting: false });
    lose_an_abstract_fight(&mut game);
    assert_eq!(
        game.worn(trooper, EquipmentSlot::Weapon).unwrap().copy.wear,
        0
    );
}

#[test]
fn a_broken_copy_gives_no_bonus_and_keeps_stats_exact() {
    let mut game = Game::new(5036, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let bare = *game.world.get::<Stats>(player).unwrap();
    // Armour, because a weapon's atk lives in a damage band `Stats` does
    // not hold; mitigation is baked in and so shows the delta exactly.
    let plating = worn_copy(ids::FIREWALL_PLATING, GEAR_DURABILITY - 1);
    game.add_copies(&plating, 1);
    game.equip(player, &plating).unwrap();
    let armoured = game.world.get::<Stats>(player).unwrap().mitigation;
    assert!(
        armoured > bare.mitigation,
        "the plating has to give something"
    );

    lose_an_abstract_fight(&mut game);
    let worn = game.worn(player, EquipmentSlot::Armor).unwrap().copy;
    assert_eq!(game.condition_of(&worn).0, 0);
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().mitigation,
        bare.mitigation,
        "a broken copy is inert"
    );
    assert!(
        game.message_log(crate::MESSAGE_LOG_CAP)
            .iter()
            .any(|l| l.text.contains("broke")),
        "the break is logged"
    );

    game.unequip(player, EquipmentSlot::Armor).unwrap();
    assert_eq!(
        *game.world.get::<Stats>(player).unwrap(),
        bare,
        "Stats must be exact after equip, wear, break and unequip"
    );
}

#[test]
fn a_broken_copy_takes_no_further_wear() {
    let mut game = Game::new(5037, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    wear_the_whip(&mut game, player, GEAR_DURABILITY);
    lose_an_abstract_fight(&mut game);
    assert_eq!(player_weapon(&game).wear, GEAR_DURABILITY);
}

#[test]
fn the_wear_alert_fires_once_on_crossing_the_threshold() {
    let mut game = Game::new(5038, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // 20% of 15 is 3: the step that leaves 2 is the first under it.
    let player = game.player_entity();
    wear_the_whip(&mut game, player, GEAR_DURABILITY - 4);
    let worn_alerts = |g: &Game| {
        g.alerts()
            .iter()
            .filter(|a| a.kind == crate::alerts::AlertKind::GearWorn)
            .count()
    };
    lose_an_abstract_fight(&mut game);
    assert_eq!(worn_alerts(&game), 0, "3 of 15 left is not yet under 20%");
    lose_an_abstract_fight(&mut game);
    assert_eq!(worn_alerts(&game), 1, "2 of 15 left is");
    let count_after_first = game.alerts()[0].count;
    lose_an_abstract_fight(&mut game);
    assert_eq!(
        game.alerts()[0].count,
        count_after_first,
        "the next step is not a new crossing"
    );
}

#[test]
fn fusing_never_launders_wear_into_a_repair() {
    let mut game = Game::new(5039, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worn = worn_copy(ids::MONOFILAMENT_WHIP, 9);
    game.add_copies(&worn, 2);
    game.fuse_item(&worn).unwrap();
    let fused = game
        .world
        .get::<GearCopies>(game.player_entity())
        .unwrap()
        .copies
        .iter()
        .map(|(c, _)| c.clone())
        .find(|c| c.tier == 1)
        .expect("a fused copy");
    assert_eq!(fused.wear, 9);
}

/// A fusion averages the two copies' wear, so of two otherwise equal
/// partners the less worn one yields the better result — whatever order
/// they were picked up in.
#[test]
fn fusion_prefers_the_least_worn_partner_when_all_else_is_equal() {
    let mut game = Game::new(5041, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let survivor = worn_copy(ids::MONOFILAMENT_WHIP, 0);
    let ragged = worn_copy(ids::MONOFILAMENT_WHIP, 8);
    let crisp = worn_copy(ids::MONOFILAMENT_WHIP, 2);
    game.add_copies(&survivor, 1);
    game.add_copies(&ragged, 1);
    game.add_copies(&crisp, 1);

    game.fuse_item(&survivor).unwrap();

    assert_eq!(game.count_copies(&ragged), 1, "the worn spare is kept");
    assert_eq!(
        game.count_copies(&crisp),
        0,
        "the crisp one was the partner"
    );
    let fused = game
        .world
        .get::<GearCopies>(game.player_entity())
        .unwrap()
        .copies
        .iter()
        .map(|(c, _)| c.clone())
        .find(|c| c.tier == 1)
        .expect("a fused copy");
    assert_eq!(fused.wear, 1);
}

// -------------------------------------------------------------- refit

fn refit_game(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "refit_bench", 3, 3);
    game
}

fn pack(game: &Game, item: &str) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(item))
}

#[test]
fn a_craftable_copys_refit_is_half_its_recipe_rounded_up_plus_a_coil() {
    let game = refit_game(5100);
    let edge = worn_copy("kinetic_edge", 5);
    assert_eq!(
        game.refit_cost(&edge),
        vec![
            (ItemId::from(ids::CHARGE_COIL), 1),
            (ItemId::from(ids::CORE_FRAGMENT), 4),
        ],
        "half of 7 core fragments, rounded up"
    );
}

#[test]
fn a_found_copys_refit_scales_with_its_rarity_rank() {
    let game = refit_game(5101);
    let cost = |rarity| {
        game.refit_cost(&GearCopy {
            rarity,
            ..worn_copy("broadcast_storm", 3)
        })
    };
    let fragments = |bill: &[(ItemId, u32)]| {
        bill.iter()
            .find(|(i, _)| i.as_str() == ids::CORE_FRAGMENT)
            .map(|(_, q)| *q)
            .unwrap()
    };
    let per = crate::tuning::REFIT_DROP_FRAGMENTS_PER_RANK;
    assert_eq!(fragments(&cost(Rarity::Ordinary)), per);
    assert_eq!(fragments(&cost(Rarity::Gold)), per * 3);
    assert_eq!(
        cost(Rarity::Gold)
            .iter()
            .find(|(i, _)| i.as_str() == ids::CHARGE_COIL)
            .map(|(_, q)| *q),
        Some(crate::tuning::REFIT_COILS)
    );
}

#[test]
fn refitting_a_carried_copy_restores_it_and_charges_the_share() {
    let mut game = refit_game(5102);
    let edge = worn_copy("kinetic_edge", 5);
    game.add_copies(&edge, 1);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 10);
    give(&mut game, &ItemId::from(ids::CHARGE_COIL), 2);
    let (fragments, coils) = (
        pack(&game, ids::CORE_FRAGMENT),
        pack(&game, ids::CHARGE_COIL),
    );

    let (renewed, _) = game.refit_copy(&edge).unwrap();

    assert_eq!(renewed.wear, 0);
    assert!(renewed.is_plain(), "a mended copy is plain again");
    assert_eq!(game.count_copies(&edge), 0, "the worn key is gone");
    assert_eq!(game.count_copies(&renewed), 1, "the new key is findable");
    assert_eq!(pack(&game, ids::CORE_FRAGMENT), fragments - 4);
    assert_eq!(pack(&game, ids::CHARGE_COIL), coils - 1);
}

#[test]
fn refitting_a_broken_worn_copy_gives_its_bonus_back_exactly() {
    let mut game = refit_game(5103);
    let player = game.player_entity();
    let bare = *game.world.get::<Stats>(player).unwrap();
    let plating = worn_copy(ids::FIREWALL_PLATING, GEAR_DURABILITY);
    game.add_copies(&plating, 1);
    game.equip(player, &plating).unwrap();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().mitigation,
        bare.mitigation,
        "broken gear is inert"
    );
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 50);
    give(&mut game, &ItemId::from(ids::CHARGE_COIL), 1);

    let (renewed, _) = game.refit_copy(&plating).unwrap();

    assert_eq!(
        game.worn(player, EquipmentSlot::Armor).unwrap().copy,
        renewed
    );
    assert!(game.world.get::<Stats>(player).unwrap().mitigation > bare.mitigation);
    game.unequip(player, EquipmentSlot::Armor).unwrap();
    assert_eq!(*game.world.get::<Stats>(player).unwrap(), bare);
}

#[test]
fn a_short_payer_is_refused_with_nothing_spent() {
    let mut game = refit_game(5104);
    let edge = worn_copy("kinetic_edge", 5);
    game.add_copies(&edge, 1);
    // Plenty of fragments, no coil: the bill is all-or-nothing.
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 10);
    let fragments = pack(&game, ids::CORE_FRAGMENT);

    let err = game.refit_copy(&edge).unwrap_err();

    assert!(err.contains("Charge Coil"), "{err}");
    assert_eq!(
        pack(&game, ids::CORE_FRAGMENT),
        fragments,
        "nothing was spent"
    );
    assert_eq!(game.count_copies(&edge), 1, "and the copy is unchanged");
}

#[test]
fn a_refit_pays_the_base_first_then_the_pack() {
    let mut game = refit_game(5105);
    let depot = spawn_structure_at(&mut game, "depot", 4, 3);
    game.world.entity_mut(depot).insert(Stock {
        output: [(ItemId::from(ids::CORE_FRAGMENT), 3)]
            .into_iter()
            .collect(),
        capacity: 50,
        ..Default::default()
    });
    let edge = worn_copy("kinetic_edge", 5);
    game.add_copies(&edge, 1);
    give(&mut game, &ItemId::from(ids::CHARGE_COIL), 1);
    let fragments = pack(&game, ids::CORE_FRAGMENT);

    game.refit_copy(&edge).unwrap();

    assert_eq!(
        game.world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0),
        0,
        "the shelf is emptied first"
    );
    assert_eq!(
        pack(&game, ids::CORE_FRAGMENT),
        fragments - 1,
        "the pack covers the one the shelf could not"
    );
    assert_eq!(pack(&game, ids::CHARGE_COIL), 0);
}

#[test]
fn a_module_can_be_refitted() {
    let mut game = refit_game(5106);
    let module = worn_copy("redundant_bank", 4);
    game.add_copies(&module, 1);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 60);
    give(&mut game, &ItemId::from(ids::CHARGE_COIL), 1);
    let (renewed, _) = game.refit_copy(&module).unwrap();
    assert_eq!(renewed.wear, 0);
}

#[test]
fn a_refit_needs_the_bench_and_a_worn_copy() {
    let mut game = Game::new(5107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let edge = worn_copy("kinetic_edge", 5);
    game.add_copies(&edge, 1);
    assert!(game.refit_copy(&edge).unwrap_err().contains("Refit Bench"));

    spawn_structure_at(&mut game, "refit_bench", 3, 3);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 10);
    give(&mut game, &ItemId::from(ids::CHARGE_COIL), 1);
    let fragments = pack(&game, ids::CORE_FRAGMENT);
    let fresh = worn_copy("kinetic_edge", 0);
    game.add_copies(&fresh, 1);
    assert!(
        game.refit_copy(&fresh)
            .unwrap_err()
            .contains("full condition")
    );
    let absent = worn_copy("kinetic_edge", 9);
    assert!(game.refit_copy(&absent).unwrap_err().contains("don't have"));
    assert_eq!(
        pack(&game, ids::CORE_FRAGMENT),
        fragments,
        "refusals spend nothing"
    );
}

// -------------------------------------------------------- naming

/// A full-condition copy names exactly as it did before wear shipped, so no
/// existing row moves; a worn one reads its fights left, a spent one says so.
#[test]
fn copy_name_reads_the_condition_only_when_worn() {
    let game = Game::new(5030, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let item = ids::MONOFILAMENT_WHIP;
    let plain = game.copy_name(&worn_copy(item, 0));
    let (_, max) = game.condition_of(&worn_copy(item, 0));

    assert_eq!(
        game.copy_name(&worn_copy(item, 2)),
        format!("{plain} {}/{max}", max - 2)
    );
    assert_eq!(
        game.copy_name(&worn_copy(item, max)),
        format!("{plain} 0/{max}!")
    );
    assert!(
        !game.item_name(&ItemId::from(item)).contains('/'),
        "log lines name items with item_name and must not pick the figure up"
    );
}

// ------------------------------------------------------------ alerts

/// Alerts are saved, so a subject built from the wearer's `Entity` would
/// differ after a reload and the same slot's break would post a second row
/// instead of collapsing onto the first.
#[test]
fn the_wear_alert_subject_survives_a_save_and_load() {
    let mut game = Game::new(5040, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    wear_the_whip(&mut game, player, GEAR_DURABILITY - 3);
    lose_an_abstract_fight(&mut game);
    let first = game.alerts();
    assert_eq!(first.len(), 1, "2 of 15 left crosses the threshold");

    let path = std::env::temp_dir().join(format!(
        "feral_processes_wear_alert_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    lose_an_abstract_fight(&mut loaded);
    lose_an_abstract_fight(&mut loaded);
    assert_eq!(
        loaded.condition_of(&player_weapon(&loaded)).0,
        0,
        "the weapon broke after the reload"
    );
    let alerts = loaded.alerts();
    assert_eq!(alerts.len(), 1, "the break collapsed onto the saved row");
    assert_eq!(alerts[0].count, 2);
}

/// The refit's whole bill is a sink, the pack's share as much as the
/// shelves': `spend_from_stores` once logged only the base half.
#[test]
fn a_refit_logs_its_whole_bill_to_the_ledger() {
    let mut game = refit_game(5110);
    let depot = spawn_structure_at(&mut game, "depot", 4, 3);
    game.world.entity_mut(depot).insert(Stock {
        output: [(ItemId::from(ids::CORE_FRAGMENT), 1)]
            .into_iter()
            .collect(),
        capacity: 50,
        ..Default::default()
    });
    let edge = worn_copy("kinetic_edge", 5);
    game.add_copies(&edge, 1);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 10);
    give(&mut game, &ItemId::from(ids::CHARGE_COIL), 1);
    let ledger = |game: &Game, item: &str| {
        game.world
            .resource::<crate::base_ledger::BaseLedger>()
            .lifetime
            .get(&ItemId::from(item))
            .map_or(0, |t| t.consumed)
    };
    let (fragments, coils) = (
        ledger(&game, ids::CORE_FRAGMENT),
        ledger(&game, ids::CHARGE_COIL),
    );

    game.refit_copy(&edge).unwrap();

    assert_eq!(ledger(&game, ids::CORE_FRAGMENT) - fragments, 4);
    assert_eq!(ledger(&game, ids::CHARGE_COIL) - coils, 1, "pack-only item");
}
