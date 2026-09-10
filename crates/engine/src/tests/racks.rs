//! The Quarantine Rack: `components::Racked`, its tier-derived ceiling, and
//! the carrier half of the `c` transfer basket. See
//! `docs/superpowers/specs/2026-09-10-quarantine-rack-design.md`.

use super::support::*;
use crate::components::Racked;
use crate::items::DownedProgram;
use crate::*;

fn program(level: u32) -> DownedProgram {
    DownedProgram {
        species: "scrapper".to_string(),
        level,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    }
}

#[test]
fn a_built_rack_carries_a_shelf_whose_ceiling_is_its_tier() {
    let mut game = Game::new(4200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let rack = spawn_machine_at(&mut game, "quarantine_rack", 3, 3);

    assert!(
        game.world.get::<Racked>(rack).is_some(),
        "a structure that racks should carry the shelf"
    );
    assert_eq!(game.rack_slots(rack), 8, "a missing tier reads as tier 1");
    assert_eq!(game.rack_room(rack), 8);

    game.world
        .entity_mut(rack)
        .insert(crate::components::StructureTier(3));
    assert_eq!(game.rack_slots(rack), 24, "tier multiplies the slots");

    game.world
        .get_mut::<Racked>(rack)
        .unwrap()
        .0
        .push(program(4));
    assert_eq!(game.rack_room(rack), 23);
}

/// A RON round trip cannot see a `#[serde(skip)]`, so the shelf's
/// persistence is asserted through a real save and load.
#[test]
fn a_stocked_rack_survives_save_and_load() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let rack = spawn_machine_at(&mut game, "quarantine_rack", 3, 3);
    let stock = vec![program(4), program(9), program(2)];
    game.world.get_mut::<Racked>(rack).unwrap().0 = stock.clone();

    let path =
        std::env::temp_dir().join(format!("feral_rack_roundtrip_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let racked = loaded
        .world
        .iter_entities()
        .find_map(|e| e.get::<Racked>())
        .expect("the rack should still stand, carrying its shelf");
    assert_eq!(racked.0, stock, "same carriers, same order");
}

/// The def decides whether a rack stands here, not the save —
/// `durability`'s rule, the arm above `strips` in `lifecycle.rs`.
#[test]
fn a_structure_that_racks_nothing_gets_no_shelf_and_drops_a_stored_one() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 3, 3);
    assert!(game.world.get::<Racked>(depot).is_none());
    assert_eq!(game.rack_slots(depot), 0, "a def with no racks holds none");

    let path = std::env::temp_dir().join(format!("feral_rack_dropped_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    for s in data.structures.iter_mut() {
        s.racked.push(program(4));
    }
    crate::save::save_to_file(&path, &data).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded
            .world
            .iter_entities()
            .all(|e| e.get::<Racked>().is_none()),
        "a stored shelf on a structure whose def racks nothing is dropped"
    );
}

// ---------------------------------------------------------------------------
// The carrier half of the basket: `Game::rack_offer` and the one commit door.
// ---------------------------------------------------------------------------

/// The player standing at (3,4) with a rack at (3,3), `held` carriers in the
/// pack and `racked` on the shelf, each level-numbered so a test can say
/// which one it means.
fn player_beside_a_rack(held: u32, racked: u32) -> (Game, Entity) {
    let mut game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let rack = spawn_machine_at(&mut game, "quarantine_rack", 3, 3);
    let player = game.player_entity();
    for level in 1..=held {
        game.world
            .get_mut::<crate::components::DownedPrograms>(player)
            .unwrap()
            .0
            .push(program(level));
    }
    for level in 1..=racked {
        game.world
            .get_mut::<Racked>(rack)
            .unwrap()
            .0
            .push(program(100 + level));
    }
    (game, rack)
}

fn pack_of(game: &Game) -> Vec<u32> {
    let player = game.player_entity();
    game.world
        .get::<crate::components::DownedPrograms>(player)
        .unwrap()
        .0
        .iter()
        .map(|p| p.level)
        .collect()
}

fn shelf_of(game: &Game, rack: Entity) -> Vec<u32> {
    game.world
        .get::<Racked>(rack)
        .unwrap()
        .0
        .iter()
        .map(|p| p.level)
        .collect()
}

#[test]
fn the_offer_lists_the_pack_then_the_shelves() {
    let (game, _) = player_beside_a_rack(2, 2);
    let offer = game.rack_offer();
    assert_eq!(offer.len(), 4);
    assert!(!offer[0].racked && !offer[1].racked, "the pack comes first");
    assert!(offer[2].racked && offer[3].racked);
    assert!(
        offer[0].label.contains("level 1"),
        "the label is the log line's own: {}",
        offer[0].label
    );
}

#[test]
fn a_carrier_crosses_to_the_side_it_is_not_on() {
    let (mut game, rack) = player_beside_a_rack(1, 1);
    // Row 0 is the pack's, row 1 the shelf's — one of each, both ways at once.
    game.transfer_items(&TransferBasket::carriers(&[0, 1]));
    assert_eq!(pack_of(&game), vec![101]);
    assert_eq!(shelf_of(&game, rack), vec![1]);
}

/// `commit_caravan_basket`'s rule, asserted per refusal: one test over one
/// path passes against every path that never spends anyway.
#[test]
fn a_take_past_the_pack_ceiling_is_refused_and_nothing_moves() {
    let (mut game, rack) = player_beside_a_rack(crate::tuning::MAX_DOWNED_PROGRAMS as u32, 1);
    let before = pack_of(&game);
    let row = game.rack_offer().len() - 1;
    game.transfer_items(&TransferBasket::carriers(&[row]));
    assert_eq!(pack_of(&game), before, "the pack is untouched");
    assert_eq!(shelf_of(&game, rack), vec![101], "and so is the shelf");
}

#[test]
fn a_put_with_no_room_on_any_rack_is_refused_and_nothing_moves() {
    let (mut game, rack) = player_beside_a_rack(1, 8);
    let before = shelf_of(&game, rack);
    game.transfer_items(&TransferBasket::carriers(&[0]));
    assert_eq!(pack_of(&game), vec![1], "the pack is untouched");
    assert_eq!(shelf_of(&game, rack), before, "and so is the shelf");
}

/// One basket, one commit: a refused carrier half takes the item half with
/// it rather than leaving a half-moved basket behind.
#[test]
fn a_refused_carrier_half_takes_the_item_half_with_it() {
    let (mut game, rack) = player_beside_a_rack(1, 8);
    let item = ItemId::from(crate::items::ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 3);
    let before = game.world.get::<Inventory>(player).unwrap().count(&item);
    let basket = TransferBasket {
        take: Vec::new(),
        give: vec![(item.clone(), 3)],
        carriers: vec![0],
    };
    game.transfer_items(&basket);
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&item),
        before,
        "the item half is refused with the carrier half"
    );
    assert_eq!(shelf_of(&game, rack).len(), 8);
}

/// `plan_adjacent_take`'s ordering discipline, not a second rule.
#[test]
fn a_put_lands_in_the_first_rack_with_room_in_tile_order() {
    let mut game = Game::new(4211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let west = spawn_machine_at(&mut game, "quarantine_rack", 2, 4);
    let east = spawn_machine_at(&mut game, "quarantine_rack", 4, 4);
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::DownedPrograms>(player)
        .unwrap()
        .0
        .push(program(7));
    assert_eq!(game.adjacent_racks(), vec![west, east], "sorted by tile");

    game.transfer_items(&TransferBasket::carriers(&[0]));
    assert_eq!(shelf_of(&game, west), vec![7]);
    assert!(shelf_of(&game, east).is_empty());
}

// ---------------------------------------------------------------------------
// The crew fetch: a posted body walks to a rack when the rig it works runs
// dry, and the two rules that guard the trip.
// ---------------------------------------------------------------------------

use crate::components::{CarryingProgram, Hopper, Task, TaskKind};

/// A staffed, lit Teardown Rig at (3,3) with a Quarantine Rack at (3,2)
/// holding `racked` carriers, and the rig hand-loaded once so it has a
/// standing tool. Returns `(game, rig, rack, worker)`.
fn a_rig_beside_a_stocked_rack(racked: u32) -> (Game, Entity, Entity, Entity) {
    let mut game = Game::new(4220, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let rig = spawn_machine_at(&mut game, "teardown_rig", 3, 3);
    let rack = spawn_machine_at(&mut game, "quarantine_rack", 3, 2);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: rig,
        progress: 0,
        required: 1,
    });
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::DownedPrograms>(player)
        .unwrap()
        .0
        .push(program(5));
    // Fitting the tool is what sets the standing tool, and it is the only
    // thing that does — the rig's yield is a property of the rig.
    game.world.get_mut::<Inventory>(player).unwrap().add(
        ItemId::tool(&crate::tools::ToolId("salvage_clamp".to_string())),
        1,
    );
    game.install_rig_tool(rig, &crate::tools::ToolId("salvage_clamp".to_string()))
        .unwrap();
    for level in 1..=racked {
        game.world
            .get_mut::<Racked>(rack)
            .unwrap()
            .0
            .push(program(200 + level));
    }
    stand_ample_grid_supply(&mut game);
    (game, rig, rack, worker)
}

#[test]
fn a_posted_body_fetches_one_carrier_from_a_rack_in_reach() {
    let (mut game, rig, rack, worker) = a_rig_beside_a_stocked_rack(2);

    game.tick();
    assert!(
        game.world.get::<CarryingProgram>(worker).is_some(),
        "the body picks one up"
    );
    assert_eq!(shelf_of(&game, rack).len(), 1, "one trip, one carrier");

    game.tick();
    let hopper = game.world.get::<Hopper>(rig).unwrap();
    assert_eq!(hopper.queue.len(), 1, "and it lands in the hopper");
    assert_eq!(hopper.queue[0].program.level, 201);
    assert!(
        game.world.get::<CarryingProgram>(worker).is_none(),
        "the trip is over"
    );
    assert_eq!(
        hopper.queue[0].tool.0, "salvage_clamp",
        "stripped with the rig's standing tool"
    );
}

/// **An unreachable rack is not a want** — the body stays on its post and
/// nothing is announced, `build_wants`' deadlock rule.
#[test]
fn a_rack_out_of_reach_produces_no_fetch() {
    let (mut game, _rig, rack, worker) = a_rig_beside_a_stocked_rack(1);
    // Off the laid floor entirely, so no walk reaches it.
    game.world.get_mut::<Position>(rack).unwrap().x = 900;
    game.world.get_mut::<Position>(rack).unwrap().y = 900;

    game.tick();
    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert_eq!(shelf_of(&game, rack).len(), 1);
}

/// A rig nobody has hand-loaded has no tool to strip with, so it does not
/// fetch — and falls through to the `Starved` an empty hopper already
/// writes, on the transition `set_machine_status` speaks on.
#[test]
fn a_rig_with_no_standing_tool_does_not_fetch() {
    let (mut game, rig, rack, worker) = a_rig_beside_a_stocked_rack(1);
    game.world.get_mut::<Hopper>(rig).unwrap().standing_tool = None;

    game.tick();
    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert_eq!(shelf_of(&game, rack).len(), 1, "the rack keeps it");
    assert_eq!(
        game.world.get::<crate::components::MachineStatus>(rig),
        Some(&crate::components::MachineStatus::Starved),
        "nothing is feeding it, and hand-loading it once is the fix"
    );
}

/// **The first of the two silent rules.** Freeing a body mid-trip destroys
/// what it is holding, and a carrier is a kill the player cannot get back.
#[test]
fn the_scheduler_never_frees_a_body_carrying_a_program() {
    let (mut game, _rig, _rack, worker) = a_rig_beside_a_stocked_rack(1);
    game.tick();
    assert!(game.world.get::<CarryingProgram>(worker).is_some());

    // Everything that would otherwise take it off shift, at once.
    game.world
        .entity_mut(worker)
        .insert(crate::components::OffShift {
            need: crate::needs::NeedId::from("coherence"),
        });
    let amenities = game.amenities_for_test();
    assert!(
        game.is_on_shift(worker, &amenities),
        "a body holding a carrier is never freed"
    );
}

/// **The second.** Destroying the building must not destroy the kill.
#[test]
fn destroying_the_rig_by_removal_returns_the_carrier() {
    let (mut game, rig, rack, worker) = a_rig_beside_a_stocked_rack(1);
    game.tick();
    assert!(game.world.get::<CarryingProgram>(worker).is_some());

    game.remove_structure(rig).unwrap();
    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert_eq!(
        shelf_of(&game, rack),
        vec![201],
        "it goes back on the rack rather than dropping with the task"
    );
}

#[test]
fn destroying_the_rig_by_damage_returns_the_carrier() {
    let (mut game, rig, rack, worker) = a_rig_beside_a_stocked_rack(1);
    game.tick();
    assert!(game.world.get::<CarryingProgram>(worker).is_some());
    // `spawn_machine_at` writes no `Durability` — `damage_structure` reads
    // one and returns without it, so the fixture would assert nothing.
    game.world
        .entity_mut(rig)
        .insert(crate::components::Durability { hp: 10, max_hp: 10 });

    game.damage_structure(rig, 100_000, "The Teardown Rig");
    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert_eq!(shelf_of(&game, rack), vec![201]);
}

/// With no rack left to take it, the carrier goes to the player's own store
/// rather than being dropped.
#[test]
fn a_returned_carrier_falls_back_to_the_players_store() {
    let (mut game, rig, rack, worker) = a_rig_beside_a_stocked_rack(1);
    game.tick();
    // Despawned rather than removed through the door: `remove_structure`
    // spends a tick, and that tick is one the body would spend delivering
    // the carrier it is holding.
    game.world.despawn(rack);

    game.remove_structure(rig).unwrap();
    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert!(
        pack_of(&game).contains(&201),
        "the kill survives the machine"
    );
}

#[test]
fn an_in_transit_carrier_survives_save_and_load() {
    let (mut game, _rig, _rack, worker) = a_rig_beside_a_stocked_rack(1);
    game.tick();
    assert!(game.world.get::<CarryingProgram>(worker).is_some());

    let path = std::env::temp_dir().join(format!("feral_in_transit_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let held = loaded
        .world
        .iter_entities()
        .find_map(|e| e.get::<CarryingProgram>())
        .expect("the trip survives quitting mid-walk");
    assert_eq!(held.0.level, 201);
}
