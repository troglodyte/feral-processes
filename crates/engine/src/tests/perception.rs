//! Perception: the derived stat that sets how many tiles of the surface the
//! player sees. Analysis supplies three quarters of a spread's worth and
//! Entropy the rest.

use super::support::*;
use crate::attributes::AttributeId;
use crate::components::{Attributes, ProgramBase};
use crate::tuning::{PERCEPTION_BASE_RADIUS, PERCEPTION_MIN_RADIUS};
use crate::*;

fn game() -> Game {
    Game::new(7101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn set_attribute(game: &mut Game, id: &str, value: i32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Attributes>(player)
        .unwrap()
        .set(&AttributeId::from(id), value);
    game.recompute_derived(player);
}

fn base_of(game: &Game, id: &str) -> i32 {
    game.world
        .resource::<crate::attributes::AttributeDb>()
        .get(&AttributeId::from(id))
        .unwrap()
        .base
}

#[test]
fn baseline_attributes_see_exactly_the_base_radius() {
    assert_eq!(game().perception_radius(), PERCEPTION_BASE_RADIUS);
}

#[test]
fn a_point_of_analysis_adds_half_a_tile() {
    let mut game = game();
    let base = base_of(&game, "analysis");
    set_attribute(&mut game, "analysis", base + 1);
    assert!((game.perception_radius() - (PERCEPTION_BASE_RADIUS + 0.5)).abs() < 1e-4);
    set_attribute(&mut game, "analysis", base + 14);
    assert!((game.perception_radius() - (PERCEPTION_BASE_RADIUS + 7.0)).abs() < 1e-4);
}

#[test]
fn a_point_of_entropy_adds_a_thirtieth_of_a_tile() {
    let mut game = game();
    let base = base_of(&game, "entropy");
    set_attribute(&mut game, "entropy", base + 1);
    assert!((game.perception_radius() - (PERCEPTION_BASE_RADIUS + 0.033)).abs() < 1e-4);
}

#[test]
fn attributes_far_below_base_clamp_at_the_floor() {
    let mut game = game();
    set_attribute(&mut game, "analysis", 0);
    set_attribute(&mut game, "entropy", 0);
    assert_eq!(game.perception_radius(), PERCEPTION_MIN_RADIUS);
}

/// `DerivedBase` is written into every seated program's `CreatureSave`, and
/// the `dev-saves/` templates spell it out, so a record from before the
/// field must read as the baseline rather than as zero.
#[test]
fn a_seated_programs_old_base_loads_with_the_baseline_perception() {
    let mut game = game();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .to_string();
    let program = game
        .spawn_wild_creature_scaled(&species, 60, 60, 1.0, false)
        .unwrap();
    game.world
        .entity_mut(program)
        .remove::<(Hostile, WanderAi)>();
    let parts = game.roster_parts();
    game.world.entity_mut(program).insert(parts);
    game.seat_derived(program);

    let path = std::env::temp_dir().join("feral_perception_old_base.sav");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("perception:"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(
        stripped.len() < text.len(),
        "the save must have carried the field for this test to mean anything"
    );
    std::fs::write(&path, stripped).unwrap();
    let loaded = Game::load(&path, &test_assets_dir());
    let _ = std::fs::remove_file(&path);
    let loaded = loaded.unwrap();

    let seated: Vec<f32> = loaded
        .world
        .iter_entities()
        .filter_map(|e| e.get::<ProgramBase>().map(|b| b.0.perception))
        .collect();
    assert!(!seated.is_empty(), "the program must come back seated");
    assert!(seated.iter().all(|p| *p == PERCEPTION_BASE_RADIUS));
}
