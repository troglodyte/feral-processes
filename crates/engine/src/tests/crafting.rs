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

/// The outlet is what `Game::rest` will spend (Task 2) — this task only
/// needs it craftable, starter (no bench), at the price the spec fixes.
/// Asserted against a fresh game, which unlocks no perks, so
/// `LeanCompiler`'s discount can't shave the 5 down and make this brittle.
#[test]
fn the_outlet_is_craftable_from_five_core_fragments_with_no_perks() {
    let game = Game::new(98, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == ItemId::from(ids::OUTLET))
        .expect("outlet.ron should declare a craftable recipe");
    assert_eq!(recipe.cost, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(
        game.craft_cost(&ItemId::from(ids::OUTLET)),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)],
        "with no perks unlocked, craft_cost should match the raw recipe"
    );
}

#[test]
fn every_compilable_item_either_has_a_blurb_or_declares_nothing_blurb_worthy() {
    let game = Game::new(97, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for recipe in game.craft_recipes() {
        // Skip only when the def sets none of the three fields `item_blurb`
        // (`game/catalog.rs`) actually reads — read directly off the def
        // rather than through `ItemDef::category`, which checks a different
        // set (`routine`, `equipment`, `consume`, `role`) and is why this
        // guard used to silently drop the ICE Breaker: it sets only
        // `taming_potency`, which `category` never looks at, so it fell
        // through to `Material` and got skipped even though `item_blurb`
        // has "taming catalyst" to say about it. The outlet is the one
        // shipped recipe that genuinely sets none of the three and reads
        // fine as itself, the same as a bare currency would.
        let def = game
            .item_def(&recipe.result)
            .expect("a craftable recipe's result should have a definition");
        if def.equipment.is_none() && def.consume.is_none() && def.taming_potency.is_none() {
            continue;
        }
        let blurb = game.item_blurb(&recipe.result);
        assert!(
            blurb.is_some(),
            "{} is compilable but the compile menu would say nothing about it",
            recipe.result
        );
    }
}

/// A modded recipe priced in a fusable item, so the one thing keeping
/// fused gear out of the production chain can actually be walked.
const PLATING_RECIPE: &str = r#"(
    id: "plating_probe",
    name: "Plating Probe",
    description: "A test recipe priced in armour.",
    value: Some(1),
    craftable: Some((cost: [("ablative_plating", 2)])),
)"#;

/// `Inventory` is by definition the tier-0 store, and every recipe reads it
/// — so a fused copy is not an ingredient, however many base copies went
/// into it. No shipped recipe is priced in equipment, so the path is walked
/// with a modded item rather than left unasserted; the machine half needs no
/// test of its own, since `Stock` is the only thing an assembler pulls from
/// and nothing in the game puts a player's copy into one.
#[test]
fn a_fused_copy_is_not_a_recipe_ingredient() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let probe = ItemId::from("plating_probe");
    let dir = modded_assets_dir(
        "fused_recipe",
        &[],
        &[("plating_probe.ron", PLATING_RECIPE)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(21, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 2);

    assert_eq!(game.max_craftable(&probe), 1, "two spares buy one probe");

    game.fuse_item(&armor, 0).unwrap();

    assert_eq!(
        game.max_craftable(&probe),
        0,
        "the copies went into a fused one, which no recipe can reach"
    );
    assert!(game.craft(&probe, 1).is_err());
    assert_eq!(
        held_at(&game, &armor, 1),
        1,
        "and the refusal left the fused copy alone"
    );
}
