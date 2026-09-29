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
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let companion = game.adopt_program(&species, pos.x, pos.y, 1.0).unwrap();
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
    let species = game.species_defs()[0].id.clone();
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
