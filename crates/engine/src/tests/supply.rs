//! `StructureDef::supply`: an item a structure spends by its own action and
//! that haulers keep in its input buffer, sharing the hauling path with a
//! burner's fuel. The defences' use of it is in `tests::defences`.

use super::support::*;
use crate::tuning::SUPPLY_BATCH;
use crate::*;

fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "fortification");
    unlock_research_chain(&mut game, "power_grid");
    game
}

fn deploy(game: &mut Game, kind: &str, dx: i32, dy: i32) -> Entity {
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    place_now(game, kind, dx, dy).unwrap();
    let (x, y) = (px + dx, py + dy);
    let mut query = game.world.query::<(Entity, &Position, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, p, _)| p.x == x && p.y == y)
        .map(|(e, ..)| e)
        .expect("the structure was just deployed")
}

fn fill_output(game: &mut Game, structure: Entity, item: &str, qty: u32) {
    game.world
        .get_mut::<Stock>(structure)
        .unwrap()
        .output
        .insert(ItemId::from(item), qty);
}

fn hopper(game: &Game, structure: Entity, item: &str) -> u32 {
    game.world
        .get::<Stock>(structure)
        .and_then(|s| s.input.get(&ItemId::from(item)).copied())
        .unwrap_or(0)
}

fn tick_until(game: &mut Game, limit: u32, done: impl Fn(&Game) -> bool) {
    for _ in 0..limit {
        if done(game) {
            return;
        }
        game.tick();
    }
}

#[test]
fn a_hauler_fills_a_shields_input_from_a_store() {
    let mut game = base(9301);
    let shield = deploy(&mut game, "shield", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    fill_output(&mut game, depot, ids::CHARGE_COIL, 20);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    tick_until(&mut game, 400, |g| hopper(g, shield, ids::CHARGE_COIL) > 0);
    // Let any in-flight trip land before reading the hopper.
    for _ in 0..40 {
        game.tick();
    }

    let held = hopper(&game, shield, ids::CHARGE_COIL);
    assert!(
        (SUPPLY_BATCH..=2 * SUPPLY_BATCH).contains(&held),
        "a hauler brings a shield at least one batch and no more than its \
         two-batch input room, got {held}"
    );
    assert!(
        node_output(&game, depot, ids::CHARGE_COIL) < 20,
        "and takes them off the shelf"
    );
}

#[test]
fn a_fuel_burner_outranks_a_defence_when_both_want() {
    let mut game = base(9302);
    let shield = deploy(&mut game, "shield", 0, 1);
    let recharger = deploy(&mut game, "recharger_node", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    fill_output(&mut game, depot, ids::CHARGE_COIL, 20);
    fill_output(&mut game, depot, ids::POWER_CELL, 20);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    game.tick();

    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(recharger),
        "with one body and both wants standing, the fuel want comes first"
    );
}

#[test]
fn every_shipped_supply_names_a_real_item() {
    let game = Game::new(9303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let mut checked = 0;
    for def in db.all() {
        let Some(item) = &def.supply else { continue };
        assert!(
            items.get(item.as_str()).is_some(),
            "{} declares supply {item:?}, which no ItemDb entry resolves",
            def.id
        );
        checked += 1;
    }
    assert!(
        checked >= 3,
        "the shield, turret and repair bay all author one"
    );
}

/// A supply spender reads only its own input (`defence_is_armed`,
/// `spend_supply`), unlike a burner, which `burn_grid_upkeep` feeds from its
/// neighbours — so stock standing beside one is not stock within reach.
#[test]
fn a_shield_beside_a_stocked_depot_is_still_hauled_to() {
    let mut game = base(9304);
    let shield = deploy(&mut game, "shield", 1, 0);
    let depot = deploy(&mut game, "depot", 2, 0);
    fill_output(&mut game, depot, ids::CHARGE_COIL, SUPPLY_BATCH);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    tick_until(&mut game, 400, |g| hopper(g, shield, ids::CHARGE_COIL) > 0);

    assert!(
        hopper(&game, shield, ids::CHARGE_COIL) > 0,
        "coils beside the shield are not in its input"
    );
}

#[test]
fn a_repair_bay_beside_stocked_patches_is_still_hauled_to() {
    let mut game = base(9305);
    let bay = deploy(&mut game, "repair_bay", 1, 0);
    let depot = deploy(&mut game, "depot", 2, 0);
    fill_output(&mut game, depot, ids::PATCH_ROUTINE, SUPPLY_BATCH);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, bay);

    tick_until(&mut game, 400, |g| hopper(g, bay, ids::PATCH_ROUTINE) > 0);

    assert!(
        hopper(&game, bay, ids::PATCH_ROUTINE) > 0,
        "patches beside the bay are not in its input"
    );
}

/// The Winding Node's output feeds the Assembly Bay's `patch_routine` line;
/// a defence's supply is drawn off a store, never off a production buffer.
#[test]
fn a_shield_is_not_supplied_from_a_production_output() {
    let mut game = base(9306);
    let shield = deploy(&mut game, "shield", 1, 0);
    let winding = deploy(&mut game, "winding_node", 4, 0);
    fill_output(&mut game, winding, ids::CHARGE_COIL, 20);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    for _ in 0..200 {
        game.tick();
    }

    assert_eq!(hopper(&game, shield, ids::CHARGE_COIL), 0);
    assert_eq!(node_output(&game, winding, ids::CHARGE_COIL), 20);
}

#[test]
fn an_unstocked_shield_does_not_crew_the_coil_maker() {
    let mut game = base(9307);
    let shield = deploy(&mut game, "shield", 1, 0);
    let winding = deploy(&mut game, "winding_node", 4, 0);
    game.world
        .get_mut::<Stock>(winding)
        .unwrap()
        .input
        .insert(ItemId::from(ids::POWER_CELL), 10);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    for _ in 0..3 {
        game.tick();
    }

    assert_ne!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(winding),
        "a defence's supply is no reason to put anyone on the coil maker"
    );
}

/// `Uncharged` is shared by every supply structure, so its wording cannot
/// name the one item a Shield happens to spend.
#[test]
fn the_uncharged_reason_does_not_name_an_item() {
    assert!(!MachineStatus::Uncharged.reason().contains("Coil"));
}
