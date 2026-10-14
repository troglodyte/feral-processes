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
