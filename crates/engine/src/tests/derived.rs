//! `Game::recompute_derived`: the player's stats are the derivation of their
//! attributes plus gear and the perk receipt, and nothing else.

use super::support::*;
use crate::attributes::AttributeId;
use crate::components::{Attributes, Derived};
use crate::*;

fn set_attribute(game: &mut Game, id: &str, value: i32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Attributes>(player)
        .unwrap()
        .set(&AttributeId::from(id), value);
}

fn stats_of(game: &Game, entity: Entity) -> Stats {
    *game.world.get::<Stats>(entity).unwrap()
}

#[test]
fn a_fresh_player_derives_exactly_the_base_stats() {
    let game = Game::new(7001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert_eq!(stats_of(&game, player), crate::tuning::PLAYER_BASE_STATS);
    assert_eq!(game.max_power(player), crate::components::POWER_MAX);
    assert!(game.world.get::<Derived>(player).is_some());
}

#[test]
fn recompute_follows_the_attributes_with_gear_worn() {
    let mut game = Game::new(7002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(player, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    let gear_atk = game.gear_bonus(player).atk;
    assert!(gear_atk > 0, "the fixture must wear something that hits");

    set_attribute(&mut game, "parity", 60);
    set_attribute(&mut game, "analysis", 14);
    game.recompute_derived(player);

    let stats = stats_of(&game, player);
    assert_eq!(stats.max_hp, 90 + 10 * 6);
    assert_eq!(stats.atk, 6 + 4 + gear_atk, "gear is put back on top");
    assert_eq!(game.world.get::<Decompiler>(player).unwrap().skill, 4);

    // Unequipping takes exactly the gear off again: nothing was welded in.
    game.unequip(player, crate::EquipmentSlot::Weapon).unwrap();
    assert_eq!(stats_of(&game, player).atk, 6 + 4);
}

#[test]
fn recompute_does_not_double_count_the_perk_receipt() {
    let mut game = Game::new(7003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 20;
    game.unlock_perk(Perk::Attacker).unwrap();
    let receipt = *game.world.get::<BoughtStats>(player).unwrap();
    assert!(receipt.atk > 0, "the fixture perk must move attack");
    let after_purchase = stats_of(&game, player);
    assert_eq!(after_purchase.atk, 6 + receipt.atk);

    game.recompute_derived(player);
    game.recompute_derived(player);
    assert_eq!(
        stats_of(&game, player),
        after_purchase,
        "recompute after a purchase is idempotent"
    );
}

#[test]
fn a_perk_respec_leaves_the_derived_base_alone() {
    let mut game = Game::new(7004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_attribute(&mut game, "parity", 55);
    game.recompute_derived(player);
    game.world.get_mut::<Perks>(player).unwrap().points = 20;
    game.unlock_perk(Perk::Attacker).unwrap();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CREDITS), 1_000_000);
    game.respec_perks().unwrap();
    let stats = stats_of(&game, player);
    assert_eq!((stats.max_hp, stats.atk), (90 + 30, 6));
}

#[test]
fn recompute_clamps_hp_and_power_but_never_refills() {
    let mut game = Game::new(7005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 50;
    set_attribute(&mut game, "parity", 60);
    game.recompute_derived(player);
    let stats = stats_of(&game, player);
    assert_eq!((stats.hp, stats.max_hp), (50, 150), "a raise does not heal");

    set_attribute(&mut game, "parity", 40);
    set_attribute(&mut game, "bandwidth", 30);
    game.recompute_derived(player);
    let stats = stats_of(&game, player);
    assert_eq!(stats.max_hp, 30);
    assert_eq!(stats.hp, 30, "a cut trims hp down to the maximum");
    assert_eq!(game.max_power(player), 60.0);
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        60.0,
        "and the reserve with it"
    );

    set_attribute(&mut game, "bandwidth", 50);
    game.recompute_derived(player);
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        60.0,
        "raising the maximum does not refill the reserve"
    );
}

#[test]
fn recompute_is_a_no_op_on_a_body_without_derived() {
    let mut game = Game::new(7006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .to_string();
    let companion = game
        .spawn_wild_creature_scaled(&species, 60, 60, 1.0, false)
        .unwrap();
    let before = stats_of(&game, companion);
    game.recompute_derived(companion);
    assert_eq!(stats_of(&game, companion), before);
    assert!(game.world.get::<Derived>(companion).is_none());
    assert_eq!(game.max_power(companion), crate::components::POWER_MAX);
}

#[test]
fn emulation_keeps_its_own_attack_through_a_recompute() {
    let mut game = Game::new(7007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.world.entity_mut(player).insert(Emulation {
        species,
        rounds_left: 3,
    });
    let atk = game.effective_atk(player);
    set_attribute(&mut game, "analysis", 20);
    game.recompute_derived(player);
    assert_eq!(
        game.effective_atk(player),
        atk,
        "an emulated body's attack comes from the image, not from Analysis"
    );
}

fn bank(game: &mut Game, points: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::StatPoints>(player)
        .unwrap()
        .0 = points;
}

fn banked(game: &Game) -> u32 {
    game.world
        .get::<crate::components::StatPoints>(game.player_entity())
        .unwrap()
        .0
}

fn spend(pairs: &[(&str, u32)]) -> Vec<(AttributeId, u32)> {
    pairs
        .iter()
        .map(|(id, n)| (AttributeId::from(*id), *n))
        .collect()
}

#[test]
fn a_spend_raises_the_attributes_and_derives_the_stats() {
    let mut game = Game::new(7010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    bank(&mut game, 6);
    game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 4), ("analysis", 2)]))
        .unwrap();
    let player = game.player_entity();
    assert_eq!(stats_of(&game, player).max_hp, 90 + 24);
    assert_eq!(stats_of(&game, player).atk, 6 + 2);
    assert_eq!(game.world.get::<Decompiler>(player).unwrap().skill, 2);
    assert_eq!(banked(&game), 0);
}

#[test]
fn an_overspend_writes_nothing() {
    let mut game = Game::new(7011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    bank(&mut game, 3);
    let player = game.player_entity();
    let before = (
        stats_of(&game, player),
        game.world.get::<Attributes>(player).cloned().unwrap(),
    );
    assert_eq!(
        game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 2), ("analysis", 2)])),
        Err(SpendError::InsufficientPoints)
    );
    assert_eq!(banked(&game), 3);
    assert_eq!(stats_of(&game, player), before.0);
    assert_eq!(
        game.world.get::<Attributes>(player).cloned().unwrap(),
        before.1
    );
}

#[test]
fn a_bad_row_refuses_the_whole_spend() {
    let mut game = Game::new(7012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Every shipped attribute has effects, so a flavour-only one is authored
    // here for the NotBuyable row.
    let dir = scratch_assets_dir("flavour_attribute");
    std::fs::create_dir_all(&*dir).unwrap();
    for entry in std::fs::read_dir(test_assets_dir().join("attributes")).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
    }
    std::fs::write(
        dir.join("flavour.ron"),
        "(id: \"flavour\", name: \"Flavour\", legacy: \"x\", short: \"s\", \
         meaning: \"m\", base: 10, spread: 0)",
    )
    .unwrap();
    let (db, warnings) = crate::attributes::AttributeDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    game.world.insert_resource(db);
    bank(&mut game, 6);
    let player = game.player_entity();
    let before = game.world.get::<Attributes>(player).cloned().unwrap();
    assert_eq!(
        game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 1), ("flavour", 1)])),
        Err(SpendError::NotBuyable)
    );
    assert_eq!(
        game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 1), ("nonesuch", 1)])),
        Err(SpendError::NoSuchTarget)
    );
    assert_eq!(banked(&game), 6);
    assert_eq!(
        game.world.get::<Attributes>(player).cloned().unwrap(),
        before
    );
}

#[test]
fn a_spend_never_touches_the_perk_receipt_so_a_respec_keeps_it() {
    let mut game = Game::new(7013, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    bank(&mut game, 4);
    game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 4)]))
        .unwrap();
    assert_eq!(
        *game
            .world
            .get::<BoughtStats>(player)
            .unwrap_or(&BoughtStats::default()),
        BoughtStats::default()
    );
    game.world.get_mut::<Perks>(player).unwrap().points = 20;
    game.unlock_perk(Perk::Attacker).unwrap();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CREDITS), 1_000_000);
    game.respec_perks().unwrap();
    assert_eq!(stats_of(&game, player).max_hp, 90 + 24, "the spend stays");
}

#[test]
fn attention_flags_unspent_stat_points() {
    let mut game = Game::new(7014, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let has = |game: &mut Game| {
        game.attention()
            .into_iter()
            .find(|r| r.kind == AttentionKind::StatPoints)
    };
    assert!(has(&mut game).is_none());
    bank(&mut game, 6);
    let row = has(&mut game).expect("6 unspent points ask to be spent");
    assert_eq!(row.key, 'p');
    assert!(row.text.contains("6 stat points"), "{}", row.text);
    assert!(row.text.contains('S'), "{}", row.text);
    assert!(!row.threat);
}

#[test]
fn spending_raises_current_hp_by_what_the_maximum_rose_by() {
    let mut game = Game::new(7015, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    bank(&mut game, 4);
    game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 2)]))
        .unwrap();
    let stats = stats_of(&game, player);
    assert_eq!(
        (stats.hp, stats.max_hp),
        (102, 102),
        "a full player stays full"
    );

    game.world.get_mut::<Stats>(player).unwrap().hp = 50;
    game.spend_stat_points(StatOwner::Player, &spend(&[("parity", 2)]))
        .unwrap();
    let stats = stats_of(&game, player);
    assert_eq!(
        (stats.hp, stats.max_hp),
        (62, 114),
        "a wounded one gains only what the maximum rose by"
    );
}

#[test]
fn a_level_up_banks_points_into_the_component_and_grants_no_decompiler() {
    let mut game = Game::new(7016, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let two_levels = crate::progression::xp_for_level(1) + crate::progression::xp_for_level(2);
    game.award_player_xp(player, two_levels);
    assert_eq!(game.world.get::<Experience>(player).unwrap().level, 3);
    assert_eq!(banked(&game), 2 * crate::tuning::STAT_POINTS_PER_LEVEL);
    assert_eq!(game.world.get::<Decompiler>(player).unwrap().skill, 0);
    assert_eq!(stats_of(&game, player), crate::tuning::PLAYER_BASE_STATS);
}

#[test]
fn a_loaded_player_is_rederived_rather_than_trusted() {
    let dir = scratch_assets_dir("derived_load");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(7017, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    bank(&mut game, 6);
    game.spend_stat_points(
        StatOwner::Player,
        &spend(&[("parity", 4), ("bandwidth", 2)]),
    )
    .unwrap();
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    data.player.hp = 5_000;
    data.player.power = 5_000.0;
    crate::save::save_to_file(&path, &data).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let player = loaded.player_entity();
    let stats = stats_of(&loaded, player);
    assert_eq!((stats.max_hp, stats.atk), (90 + 24, 6));
    assert_eq!(stats.hp, stats.max_hp, "an out-of-range hp is clamped");
    assert_eq!(loaded.max_power(player), 104.0);
    assert_eq!(
        loaded.world.get::<PowerReserve>(player).unwrap().get(),
        104.0,
        "so is power"
    );
}

#[test]
fn rest_fills_the_reserve_to_the_derived_maximum() {
    let mut game = Game::new(7010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_attribute(&mut game, "bandwidth", 60);
    game.recompute_derived(player);
    assert_eq!(game.max_power(player), 120.0, "max Power follows Bandwidth");
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(70.0);

    game.rest().unwrap();

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        120.0,
        "a rest fills to the entity's own maximum, past the old constant"
    );
}

#[test]
fn a_power_item_restores_only_up_to_the_derived_maximum() {
    let mut game = Game::new(7011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let cell = ItemId::from("power_cell");
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(cell.clone(), 2);
    set_attribute(&mut game, "bandwidth", 30);
    game.recompute_derived(player);
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(10.0);

    assert!(game.consume_item(player, &cell));

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        60.0,
        "restores stop at the narrowed maximum"
    );
}

/// The Points screen previews `derive` plus this, so it must be exactly
/// what gear and the perk receipt hold on top of the attributes.
#[test]
fn the_stat_bonus_is_what_gear_and_perks_add_to_the_derivation() {
    let mut game = Game::new(7013, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(player, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    game.world.get_mut::<Perks>(player).unwrap().points = 20;
    game.unlock_perk(Perk::Attacker).unwrap();
    set_attribute(&mut game, "analysis", 14);
    game.recompute_derived(player);

    let bonus = game.stat_bonus(game.player_entity());
    let derived = game.derived_stats(player);
    let stats = stats_of(&game, player);
    assert!(bonus.atk > 0, "the fixture must add Atk: {bonus:?}");
    assert_eq!(derived.atk + bonus.atk, stats.atk);
    assert_eq!(derived.max_hp + bonus.max_hp, stats.max_hp);
    assert_eq!(derived.mitigation + bonus.mitigation, stats.mitigation);
    assert_eq!(
        derived.decompiler + bonus.decompiler,
        game.world.get::<Decompiler>(player).unwrap().skill
    );
}

#[test]
fn a_save_keeps_attributes_points_and_derived_values_with_gear_and_a_perk() {
    use crate::components::StatPoints;
    let assets = test_assets_dir();
    let mut game = Game::new(7012, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(player, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    game.world.get_mut::<Perks>(player).unwrap().points = 20;
    game.unlock_perk(Perk::Attacker).unwrap();
    set_attribute(&mut game, "parity", 60);
    set_attribute(&mut game, "analysis", 14);
    set_attribute(&mut game, "bandwidth", 30);
    game.recompute_derived(player);
    game.world.get_mut::<StatPoints>(player).unwrap().0 = 3;
    game.world.get_mut::<Stats>(player).unwrap().hp -= 7;
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(10.0);

    let stats = stats_of(&game, player);
    let derived = *game.world.get::<Derived>(player).unwrap();
    let attrs = game.world.get::<Attributes>(player).unwrap().clone();
    let power = game.world.get::<PowerReserve>(player).unwrap().get();
    let receipt = *game.world.get::<BoughtStats>(player).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_derived_save_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let lp = loaded.player_entity();
    assert_eq!(
        loaded
            .world
            .get::<Attributes>(lp)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        attrs.iter().collect::<Vec<_>>()
    );
    assert_eq!(loaded.world.get::<StatPoints>(lp).unwrap().0, 3);
    assert_eq!(stats_of(&loaded, lp), stats, "hp and every derived stat");
    assert_eq!(*loaded.world.get::<Derived>(lp).unwrap(), derived);
    assert_eq!(loaded.world.get::<PowerReserve>(lp).unwrap().get(), power);
    assert_eq!(*loaded.world.get::<BoughtStats>(lp).unwrap(), receipt);
}

#[test]
fn the_players_entropy_sets_their_attack_bands_and_a_wild_programs_stay_flat() {
    use crate::tuning::{CRIT_CHANCE, FUMBLE_CHANCE};
    let mut game = Game::new(7013, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Ten points above the catalogue base of 40.
    set_attribute(&mut game, "entropy", 50);
    game.recompute_derived(player);

    let mine = game.combatant_profile(player, crate::battle::Swing::default());
    assert!(
        (mine.crit - (CRIT_CHANCE + 0.02)).abs() < 1e-6,
        "{}",
        mine.crit
    );
    assert!(
        (mine.fumble - (FUMBLE_CHANCE + 0.01)).abs() < 1e-6,
        "{}",
        mine.fumble
    );

    let wild = spawn_wild_on_player_tile(&mut game);
    let theirs = game.combatant_profile(wild, crate::battle::Swing::default());
    assert_eq!((theirs.crit, theirs.fumble), (CRIT_CHANCE, FUMBLE_CHANCE));
}
