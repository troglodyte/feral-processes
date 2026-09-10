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

    game.world.get_mut::<Racked>(rack).unwrap().0.push(program(4));
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
        loaded.world.iter_entities().all(|e| e.get::<Racked>().is_none()),
        "a stored shelf on a structure whose def racks nothing is dropped"
    );
}
