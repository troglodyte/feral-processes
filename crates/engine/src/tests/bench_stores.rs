//! A bench pays out of the base's stores, not only the pack: the Workbench's
//! Subroutines and Build pages, a bench recipe's compile, the Mod Bench,
//! forging a tool and fitting a Teardown Rig all reach a Depot when the
//! party stands in base space, base stock first. A no-bench recipe and
//! anything done outside base space still pay from the pack alone.

use super::support::*;
use crate::components::Inventory;
use crate::routine_craft::{Core, Subroutine, Version};
use crate::routine_tree::part_node_id;
use crate::tools::ToolId;
use crate::*;

/// A Depot at an absolute tile with `output` already in its buffer.
fn depot(game: &mut Game, x: i32, y: i32, output: &[(ItemId, u32)]) -> Entity {
    let mut stock = Stock::new(500);
    for (id, n) in output {
        stock.output.insert(id.clone(), *n);
    }
    game.world
        .spawn((
            Structure {
                kind: "depot".to_string(),
            },
            Position { x, y },
            stock,
        ))
        .id()
}

fn shelf(game: &Game, depot: Entity, item: &ItemId) -> u32 {
    game.world
        .get::<Stock>(depot)
        .unwrap()
        .output
        .get(item)
        .copied()
        .unwrap_or(0)
}

fn pack(game: &Game, item: &ItemId) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(item)
}

fn empty_pack(game: &mut Game) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();
}

fn heal() -> Subroutine {
    Subroutine::Core(Core::Heal, Version::V1)
}

/// In base space beside a Workbench, the routine tree's Heal part known,
/// and nothing in the pack.
fn at_the_workbench(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "automation");
    touch_a_workbench(&mut game);
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .insert(part_node_id(&heal()));
    empty_pack(&mut game);
    game
}

#[test]
fn crafting_a_part_pays_from_a_depot() {
    let mut game = at_the_workbench(7101);
    let disk = ItemId::from(ids::ROUTINE_DISK);
    let (material, count) = crate::routine_craft::material(&heal());
    let store = depot(
        &mut game,
        9,
        9,
        &[(disk.clone(), 2), (material.clone(), count)],
    );

    game.craft_subroutine(heal()).unwrap();

    assert_eq!(shelf(&game, store, &disk), 1);
    assert_eq!(shelf(&game, store, &material), 0);
    assert_eq!(game.parts_held(&heal()), 1);
}

#[test]
fn base_stock_is_spent_before_the_pack() {
    let mut game = at_the_workbench(7102);
    let disk = ItemId::from(ids::ROUTINE_DISK);
    let (material, count) = crate::routine_craft::material(&heal());
    let store = depot(&mut game, 9, 9, &[(disk.clone(), 1)]);
    give(&mut game, &disk, 1);
    give(&mut game, &material, count);

    game.craft_subroutine(heal()).unwrap();

    assert_eq!(shelf(&game, store, &disk), 0, "the Depot's disk goes first");
    assert_eq!(pack(&game, &disk), 1, "the carried disk stays carried");
    assert_eq!(
        pack(&game, &material),
        0,
        "the pack covers what the base lacks"
    );
}

#[test]
fn building_a_routine_takes_parts_off_a_depot() {
    let mut game = at_the_workbench(7103);
    let part = ItemId::part(&heal());
    let store = depot(&mut game, 9, 9, &[(part.clone(), 1)]);
    assert_eq!(
        game.parts_held(&heal()),
        1,
        "the Build page counts the Depot"
    );

    game.build_routine("Mend", &[heal()]).unwrap();

    assert_eq!(shelf(&game, store, &part), 0);
}

#[test]
fn a_depot_is_out_of_reach_outside_base_space() {
    let mut game = at_the_workbench(7104);
    let disk = ItemId::from(ids::ROUTINE_DISK);
    let (material, count) = crate::routine_craft::material(&heal());
    let store = depot(
        &mut game,
        9,
        9,
        &[(disk.clone(), 1), (material.clone(), count)],
    );
    game.world.insert_resource(Locale::Surface);

    assert_eq!(game.blank_disks_held(), 0);
    assert!(game.craft_subroutine(heal()).is_err());
    assert_eq!(shelf(&game, store, &disk), 1, "a refusal spends nothing");
}

#[test]
fn a_bench_recipe_compiles_from_a_depot() {
    let mut game = at_the_workbench(7105);
    unlock_research_chain(&mut game, "weapon_bench");
    let lance = ItemId::from("logic_probe");
    let cost = game.craft_cost(&lance, false);
    assert!(!cost.is_empty(), "logic_probe should be compilable here");
    let bill: Vec<_> = cost.iter().map(|(i, q)| (i.clone(), q * 2)).collect();
    let store = depot(&mut game, 9, 9, &bill);
    assert_eq!(game.max_craftable(&lance, false), 2);

    game.craft(&lance, 1, false).unwrap();

    for (item, qty) in &cost {
        assert_eq!(
            shelf(&game, store, item),
            *qty,
            "one unit's {item:?} left the Depot"
        );
    }
}

#[test]
fn a_benchless_recipe_still_pays_from_the_pack_alone() {
    let mut game = at_the_workbench(7106);
    let cell = ItemId::from("power_cell");
    let cost = game.craft_cost(&cell, false);
    assert!(!cost.is_empty(), "power_cell should be compilable here");
    depot(&mut game, 9, 9, &cost);

    assert_eq!(game.max_craftable(&cell, false), 0);
    assert!(game.craft(&cell, 1, false).is_err());
}

#[test]
fn forging_a_tool_pays_from_a_depot() {
    let mut game = at_the_workbench(7107);
    let stripper = ToolId("component_stripper".to_string());
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(stripper.clone());
    let frag = ItemId::from(ids::CORE_FRAGMENT);
    let store = depot(&mut game, 9, 9, &[(frag.clone(), 5)]);

    game.forge_tool(&stripper).unwrap();

    assert_eq!(shelf(&game, store, &frag), 0);
    assert_eq!(pack(&game, &ItemId::tool(&stripper)), 1);
}

#[test]
fn fitting_a_rig_takes_the_carrier_off_a_depot() {
    let mut game = Game::new(7108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let rig = spawn_machine_at(&mut game, "teardown_rig", 3, 3);
    let stripper = ToolId("component_stripper".to_string());
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(stripper.clone());
    let carrier = ItemId::tool(&stripper);
    let store = depot(&mut game, 9, 9, &[(carrier.clone(), 1)]);
    let row = game.rig_tool_view(rig).expect("a rig beside the party");
    assert!(
        row.candidates
            .iter()
            .any(|c| c.id == stripper && c.carriers_held == 1),
        "the Tool page counts the Depot's carrier"
    );

    game.install_rig_tool(rig, &stripper).unwrap();

    assert_eq!(shelf(&game, store, &carrier), 0);
}
