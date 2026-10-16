//! The Workbench: one hand-compile bench you stand beside, whose tier sets
//! the quality floor, and whose recipes keep the research gate of the bench
//! they came from.

use super::support::*;
use crate::*;

fn game_for(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn listed(game: &Game, item: &str) -> bool {
    game.craft_recipes()
        .iter()
        .any(|r| r.result == ItemId::from(item))
}

#[test]
fn a_bench_recipe_is_offered_only_beside_a_workbench() {
    let mut game = game_for(601);
    unlock_research_chain(&mut game, "armor_bench");
    spawn_structure_at(&mut game, "workbench", 1, 0);

    stand_in_base_at(&mut game, 0, 0);
    assert!(listed(&game, "hardened_shell"), "beside the bench");

    stand_in_base_at(&mut game, -1, 0);
    assert!(!listed(&game, "hardened_shell"), "two tiles away");

    game.world.insert_resource(Locale::Surface);
    assert!(!listed(&game, "hardened_shell"), "in the field");
}

#[test]
fn a_benchless_recipe_compiles_anywhere() {
    let game = game_for(602);
    assert!(listed(&game, "ice_breaker"));
    assert!(listed(&game, "power_cell"));
}

#[test]
fn the_touched_workbench_sets_the_quality_floor() {
    use crate::tuning::{QUALITY_BASE, QUALITY_BENCH_PER_TIER};
    let mut game = game_for(603);
    unlock_research_chain(&mut game, "armor_bench");
    for (x, y, tier) in [(1, 0, 3), (20, 20, 5)] {
        let bench = spawn_structure_at(&mut game, "workbench", x, y);
        game.world.entity_mut(bench).insert(StructureTier(tier));
    }
    stand_in_base_at(&mut game, 0, 0);
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == ItemId::from("hardened_shell"))
        .expect("listed beside the bench");
    let order = game.player_craft_order(&recipe, false);
    assert_eq!(
        game.craft_quality_floor(&order),
        QUALITY_BASE + 2 * QUALITY_BENCH_PER_TIER,
        "the Mk3 bench in reach, not the Mk5 across the base"
    );
}

#[test]
fn touching_picks_the_highest_tier_then_the_lowest_tile() {
    let mut game = game_for(604);
    let a = spawn_structure_at(&mut game, "workbench", 1, 0);
    let b = spawn_structure_at(&mut game, "workbench", -1, 0);
    stand_in_base_at(&mut game, 0, 0);
    assert_eq!(
        game.touched_bench("workbench"),
        Some((b, 1)),
        "tie: lowest tile"
    );
    game.world.entity_mut(a).insert(StructureTier(2));
    assert_eq!(game.touched_bench("workbench"), Some((a, 2)));
    game.world.insert_resource(Locale::Surface);
    assert_eq!(game.touched_bench("workbench"), None);
}

#[test]
fn a_workbench_recipe_keeps_the_gate_of_the_structure_it_came_from() {
    let mut game = game_for(605);
    spawn_structure_at(&mut game, "workbench", 1, 0);
    stand_in_base_at(&mut game, 0, 0);
    assert!(listed(&game, "bytecode_block"));
    assert!(!listed(&game, "blank_substrate"));
    assert!(!listed(&game, "honeypot"));

    unlock_research_chain(&mut game, "routine_fabrication");
    assert!(listed(&game, "blank_substrate"));
    assert!(!listed(&game, "honeypot"));

    unlock_research_chain(&mut game, "deception");
    assert!(listed(&game, "honeypot"));
}

#[test]
fn extraction_needs_automation_and_a_workbench_somewhere() {
    let mut game = game_for(606);
    assert!(!game.can_extract_routines(), "no bench");
    spawn_structure_at(&mut game, "workbench", 30, 30);
    assert!(!game.can_extract_routines(), "automation not researched");
    unlock_research_chain(&mut game, "automation");
    assert!(game.can_extract_routines());
    game.world.insert_resource(Locale::Surface);
    assert!(game.can_extract_routines(), "ownership, not touch");
}

#[test]
fn a_research_node_names_the_machine_recipes_it_gates() {
    let game = game_for(607);
    let def = game
        .research_defs()
        .into_iter()
        .find(|d| d.id == "automation")
        .unwrap();
    let text = game.research_unlocks(&def).unwrap_or_default();
    assert!(text.contains("Printer: ICE Breaker"), "{text}");
}

#[test]
fn a_wide_bench_is_touched_from_any_of_its_cells() {
    let mut game = game_for(606);
    let mut def = game
        .world
        .resource::<StructureDb>()
        .get("workbench")
        .cloned()
        .expect("workbench ships");
    def.id = "wide_bench".to_string();
    def.footprint = 2;
    game.world.resource_mut::<StructureDb>().insert(def);
    let bench = spawn_structure_at(&mut game, "wide_bench", 1, 0);
    // East of the footprint's far column: not orthogonal to the anchor.
    stand_in_base_at(&mut game, 3, 1);
    assert_eq!(game.touched_bench("wide_bench"), Some((bench, 1)));
    stand_in_base_at(&mut game, 5, 1);
    assert_eq!(game.touched_bench("wide_bench"), None);
}
