//! The world map's stored state: surface fog, and its save coverage.

use super::support::*;
use crate::components::Position;
use crate::resources::{DifficultyMode, ExploredChunks};
use crate::tuning::WORLD_MAP_REVEAL_RADIUS_CHUNKS;
use crate::world::CHUNK_SIZE;
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn explored(game: &Game) -> std::collections::BTreeSet<(i32, i32)> {
    game.world.resource::<ExploredChunks>().0.clone()
}

fn move_party_to(game: &mut Game, x: i32, y: i32) {
    let p = game.player_entity();
    *game.world.get_mut::<Position>(p).unwrap() = Position { x, y };
}

#[test]
fn walking_reveals_the_chunks_within_the_radius() {
    let mut game = game();
    move_party_to(&mut game, 10 * CHUNK_SIZE + 3, -4 * CHUNK_SIZE + 5);
    game.idle_tick();
    let r = WORLD_MAP_REVEAL_RADIUS_CHUNKS;
    let set = explored(&game);
    for cy in (-4 - r)..=(-4 + r) {
        for cx in (10 - r)..=(10 + r) {
            assert!(set.contains(&(cx, cy)), "({cx},{cy}) should be revealed");
        }
    }
    assert!(!set.contains(&(10 + r + 1, -4)));
}

#[test]
fn negative_tiles_floor_to_the_chunk_below() {
    let mut game = game();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    move_party_to(&mut game, -1, -1);
    game.idle_tick();
    assert!(explored(&game).contains(&(-1, -1)));
}

#[test]
fn nothing_is_revealed_in_base_space() {
    let mut game = game();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    move_party_to(&mut game, 50 * CHUNK_SIZE, 50 * CHUNK_SIZE);
    stand_in_base(&mut game);
    game.idle_tick();
    assert!(explored(&game).is_empty());
}

#[test]
fn nothing_is_revealed_in_the_stack() {
    let mut game = game();
    game.world.resource_mut::<ExploredChunks>().0.clear();
    move_party_to(&mut game, 50 * CHUNK_SIZE, 50 * CHUNK_SIZE);
    descend(&mut game);
    game.idle_tick();
    assert!(explored(&game).is_empty());
}

#[test]
fn fog_survives_a_breach() {
    let mut game = game();
    game.idle_tick();
    let before = explored(&game);
    assert!(!before.is_empty());
    game.enter_next_zone();
    assert!(before.is_subset(&explored(&game)));
}

#[test]
fn fog_survives_a_real_save_and_load() {
    let mut game = game();
    move_party_to(&mut game, 7 * CHUNK_SIZE, 7 * CHUNK_SIZE);
    game.idle_tick();
    let before = explored(&game);
    assert!(before.contains(&(7, 7)));
    let path = std::env::temp_dir().join(format!("feral_fog_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(explored(&loaded), before);
}
