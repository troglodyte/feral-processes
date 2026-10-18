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
    let winding = place_machine(&mut game, "factory", "charge_coil", 4, 0).unwrap();
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
    let winding = place_machine(&mut game, "factory", "charge_coil", 4, 0).unwrap();
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

/// With a want standing (the shelf holds coils), the hauler still takes them
/// off the shelf and not off the nearer production buffer.
#[test]
fn a_shield_takes_the_shelf_not_the_nearer_production_output() {
    let mut game = base(9308);
    let shield = deploy(&mut game, "shield", 1, 0);
    let winding = place_machine(&mut game, "factory", "charge_coil", 3, 0).unwrap();
    let depot = deploy(&mut game, "depot", 6, 0);
    fill_output(&mut game, winding, ids::CHARGE_COIL, 20);
    fill_output(&mut game, depot, ids::CHARGE_COIL, 20);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    tick_until(&mut game, 400, |g| hopper(g, shield, ids::CHARGE_COIL) > 0);
    for _ in 0..40 {
        game.tick();
    }

    assert!(hopper(&game, shield, ids::CHARGE_COIL) > 0);
    assert_eq!(node_output(&game, winding, ids::CHARGE_COIL), 20);
}

/// A defence beside a coil maker is not its attached building: nothing pulls
/// from a neighbour into a supply-only input, so reserving the maker's buffer
/// for it would leave the coils where no hauler may take them.
#[test]
fn a_shield_beside_the_coil_maker_does_not_hold_its_buffer() {
    let mut game = base(9309);
    deploy(&mut game, "shield", 1, 0);
    let winding = place_machine(&mut game, "factory", "charge_coil", 2, 0).unwrap();
    let depot = deploy(&mut game, "depot", 5, 0);
    let worker = spawn_tamed(&mut game, 500, 3);
    game.assign_cronjob(worker, winding).unwrap();
    park_at_post(&mut game, worker, winding);
    fill_output(&mut game, winding, ids::CHARGE_COIL, 3);

    tick_until(&mut game, 60, |g| {
        node_output(g, depot, ids::CHARGE_COIL) > 0
    });

    assert!(
        node_output(&game, depot, ids::CHARGE_COIL) > 0,
        "the coils belong on a shelf the shield can be supplied from"
    );
}

/// A shield registered under a new id with its own authored ceiling.
fn capped_shield(game: &mut Game, id: &str, cap: Option<u32>) {
    let mut def = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get("shield")
        .cloned()
        .expect("shield ships");
    def.id = id.to_string();
    def.supply_capacity = cap;
    game.world
        .resource_mut::<crate::structures::StructureDb>()
        .insert(def);
}

#[test]
fn an_authored_supply_capacity_is_what_the_sheet_quotes() {
    let mut game = base(9310);
    capped_shield(&mut game, "shield_capped", Some(20));
    capped_shield(&mut game, "shield_plain", None);
    let capped = deploy(&mut game, "shield_capped", 1, 0);
    let plain = deploy(&mut game, "shield_plain", 3, 0);
    assert_eq!(
        game.structure_manifest(capped).unwrap().supply,
        Some(("Charge Coil".to_string(), 20))
    );
    assert_eq!(
        game.structure_manifest(plain).unwrap().supply,
        Some((
            "Charge Coil".to_string(),
            SUPPLY_BATCH * crate::tuning::INPUT_STOCK_BATCHES
        ))
    );
}

#[test]
fn a_crew_never_stocks_past_an_authored_capacity() {
    let mut game = base(9311);
    // Under the carry capacity (5), so only the ceiling can stop the trip at 4.
    capped_shield(&mut game, "shield_capped", Some(4));
    let shield = deploy(&mut game, "shield_capped", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    fill_output(&mut game, depot, ids::CHARGE_COIL, 60);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    let mut peak = 0;
    for _ in 0..1500 {
        game.tick();
        peak = peak.max(hopper(&game, shield, ids::CHARGE_COIL));
    }
    assert_eq!(
        peak, 4,
        "the crew fills to the authored ceiling, no further"
    );
}

fn give_cores(game: &mut Game, qty: u32) -> Vec<(ItemId, u32)> {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ANNEALED_CORE), qty);
    let basket = TransferBasket::items(&[], &[(ItemId::from(ids::ANNEALED_CORE), qty)]);
    game.transfer_items(&basket).1
}

fn carried(game: &Game, item: &str) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .map_or(0, |inv| inv.count(&ItemId::from(item)))
}

fn bench_beside_player(game: &mut Game) -> Entity {
    unlock_research_chain(game, "program_refactoring");
    deploy(game, "refit_bench", 1, 0)
}

#[test]
fn a_player_beside_a_bench_loads_it_with_cores_up_to_its_capacity() {
    let mut game = base(9320);
    let bench = bench_beside_player(&mut game);
    assert!(
        game.transfer_offer()
            .iter()
            .all(|r| r.item.0 != ids::ANNEALED_CORE),
        "no cores carried, so no row yet"
    );

    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::ANNEALED_CORE), 25);
    let row = game
        .transfer_offer()
        .into_iter()
        .find(|r| r.item.0 == ids::ANNEALED_CORE)
        .expect("the bench's row is offered");
    assert_eq!(row.can_put, 20, "the bench's capacity is the permission");
    assert_eq!(game.transfer_room(), Some(20));

    let given = game
        .transfer_items(&TransferBasket::items(
            &[],
            &[(ItemId::from(ids::ANNEALED_CORE), 25)],
        ))
        .1;
    assert_eq!(given, vec![(ItemId::from(ids::ANNEALED_CORE), 20)]);
    assert_eq!(hopper(&game, bench, ids::ANNEALED_CORE), 20);
    assert_eq!(
        carried(&game, ids::ANNEALED_CORE),
        5,
        "the rest stays packed"
    );

    assert!(give_cores(&mut game, 1).is_empty(), "the 21st is refused");
    assert_eq!(hopper(&game, bench, ids::ANNEALED_CORE), 20);
    assert_eq!(carried(&game, ids::ANNEALED_CORE), 6);
}

#[test]
fn two_rows_of_one_item_cannot_overfill_a_bench() {
    let mut game = base(9321);
    let bench = bench_beside_player(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::ANNEALED_CORE), 30);

    let core = ItemId::from(ids::ANNEALED_CORE);
    let given = game.give_to_adjacent(&[(core.clone(), 15), (core.clone(), 15)]);

    assert_eq!(given, vec![(core, 20)]);
    assert_eq!(hopper(&game, bench, ids::ANNEALED_CORE), 20);
    assert_eq!(carried(&game, ids::ANNEALED_CORE), 10);
}

#[test]
fn a_bench_takes_only_its_supply_item_by_hand() {
    let mut game = base(9321);
    let bench = bench_beside_player(&mut game);
    let before = carried(&game, ids::CORE_FRAGMENT);
    let given = game
        .transfer_items(&TransferBasket::items(
            &[],
            &[(ItemId::from(ids::CORE_FRAGMENT), 5)],
        ))
        .1;
    assert!(given.is_empty());
    assert_eq!(carried(&game, ids::CORE_FRAGMENT), before);
    assert_eq!(hopper(&game, bench, ids::CORE_FRAGMENT), 0);
}

#[test]
fn a_crew_tops_an_authored_capacity_bench_up_in_full_trips() {
    let mut game = base(9322);
    capped_shield(&mut game, "shield_capped", Some(20));
    let shield = deploy(&mut game, "shield_capped", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    fill_output(&mut game, depot, ids::CHARGE_COIL, 25);
    let worker = spawn_tamed(&mut game, 500, 3);
    park_at_post(&mut game, worker, shield);

    tick_until(&mut game, 3000, |g| {
        hopper(g, shield, ids::CHARGE_COIL) == 20
    });
    for _ in 0..300 {
        game.tick();
    }
    assert_eq!(hopper(&game, shield, ids::CHARGE_COIL), 20);
    let left = game.world.get::<Stock>(depot).unwrap().output[&ItemId::from(ids::CHARGE_COIL)];
    assert_eq!(
        left, 5,
        "a full trip fits no more, so the depot keeps the rest"
    );
}

#[test]
fn a_turret_without_an_authored_capacity_is_not_loaded_by_hand() {
    let mut game = base(9323);
    let turret = deploy(&mut game, "turret", 1, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CHARGE_COIL), 10);

    let given = game.give_to_adjacent(&[(ItemId::from(ids::CHARGE_COIL), 10)]);

    assert!(given.is_empty());
    assert_eq!(hopper(&game, turret, ids::CHARGE_COIL), 0);
    assert_eq!(carried(&game, ids::CHARGE_COIL), 10);
}

#[test]
fn beside_a_bench_with_no_depot_only_its_supply_is_puttable() {
    let mut game = base(9324);
    bench_beside_player(&mut game);
    let inv = game.world.get_mut::<Inventory>(game.player_entity());
    let mut inv = inv.unwrap();
    inv.add(ItemId::from(ids::ANNEALED_CORE), 5);
    inv.add(ItemId::from(ids::BYTECODE_BLOCK), 3);

    let rows: Vec<_> = game
        .transfer_offer()
        .into_iter()
        .map(|r| (r.item.0, r.can_put))
        .collect();
    assert_eq!(
        rows,
        vec![(ids::ANNEALED_CORE.to_string(), 5)],
        "a pack item nothing here takes gets no row at all"
    );
}
