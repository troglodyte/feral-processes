//! Recipes, crafting costs, and the blurbs describing what an item does.

use super::support::*;
use crate::*;

#[test]
fn craft_consumes_cost_and_grants_the_result() {
    let mut game = Game::new(20, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST);
    }

    game.craft(&ItemId::from(ids::ICE_BREAKER), 1).unwrap();

    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
        0,
        "cost should be fully consumed"
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::ICE_BREAKER)),
        1,
        "the recipe's result should be granted"
    );
}

#[test]
fn craft_multiple_scales_cost_and_result() {
    let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST * 3);
    }

    game.craft(&ItemId::from(ids::ICE_BREAKER), 3).unwrap();

    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
        0,
        "cost should scale with quantity"
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::ICE_BREAKER)),
        3,
        "quantity units should be granted"
    );
}

#[test]
fn max_craftable_floors_to_the_cheapest_affordable_whole_unit() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        // ICE_BREAKER_CORE_COST per unit; 7 fragments afford 2 whole
        // units with 1 left over, not 3.
        inv.add(
            ItemId::from(ids::CORE_FRAGMENT),
            ICE_BREAKER_CORE_COST * 2 + 1,
        );
    }

    assert_eq!(game.max_craftable(&ItemId::from(ids::ICE_BREAKER)), 2);
}

#[test]
fn max_craftable_is_zero_with_no_recipe_or_no_resources() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();

    assert_eq!(
        game.max_craftable(&ItemId::from(ids::ICE_BREAKER)),
        0,
        "no resources at all"
    );
    assert_eq!(
        game.max_craftable(&ItemId::from(ids::CORE_FRAGMENT)),
        0,
        "no recipe exists for this item"
    );
}

#[test]
fn craft_fails_without_enough_of_the_cost() {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
    }

    assert!(game.craft(&ItemId::from(ids::ICE_BREAKER), 1).is_err());
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER)),
        0
    );
}

#[test]
fn craft_rejects_a_result_with_no_recipe() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.craft(&ItemId::from(ids::CORE_FRAGMENT), 1).is_err());
}

#[test]
fn item_blurbs_gloss_what_a_shipped_item_actually_does() {
    let game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.item_blurb(&ItemId::from(ids::POWER_CELL)).as_deref(),
        Some("+25 power"),
        "a consumable should quote what it restores"
    );
    assert_eq!(
        game.item_blurb(&ItemId::from("arc_lance")).as_deref(),
        Some("+3 atk"),
        "equipment should quote the stats it grants"
    );
    assert_eq!(
        game.item_blurb(&ItemId::from("black_ice_pick")).as_deref(),
        Some("+3 atk +2 decomp"),
        "an item granting several stats should list each"
    );
    assert_eq!(
        game.item_blurb(&ItemId::from(ids::CORE_FRAGMENT)),
        None,
        "a plain currency has nothing to gloss and reads fine as itself"
    );
}

#[test]
fn every_compilable_item_either_has_a_blurb_or_is_plain_currency() {
    let game = Game::new(97, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for recipe in game.craft_recipes() {
        let blurb = game.item_blurb(&recipe.result);
        assert!(
            blurb.is_some(),
            "{} is compilable but the compile menu would say nothing about it",
            recipe.result
        );
    }
}
