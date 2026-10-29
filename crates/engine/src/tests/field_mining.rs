//! Mining a blocker on the field: the pure adjacency gate and the strike
//! that turns a blocker into an `Excavated` override. The bump and the crew
//! are wired to these in a later phase.

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::game::field_mining::minable;
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

const WALL: (i32, i32) = (40, 40);

fn blocker(biome: Biome) -> Tile {
    Tile {
        biome,
        walkable: false,
        rock_shade: None,
    }
}

fn wall_at(game: &mut Game, cell: (i32, i32), biome: Biome) {
    game.world
        .resource_mut::<WorldMap>()
        .set_override(cell.0, cell.1, blocker(biome));
}

fn walkable(game: &mut Game, cell: (i32, i32)) -> bool {
    game.world
        .resource_mut::<WorldMap>()
        .tile(cell.0, cell.1)
        .walkable
}

fn fragments(game: &Game) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT))
}

/// Swings until the cell is no longer a blocker, returning the swing count.
fn cut_through(game: &mut Game, cell: (i32, i32)) -> u32 {
    let player = game.player_entity();
    for n in 1..=500 {
        game.strike_field_rock(player, cell.0, cell.1);
        if walkable(game, cell) {
            return n;
        }
    }
    panic!("{cell:?} never gave way");
}

#[test]
fn a_blocker_beside_floor_is_minable() {
    let mut game = game(7);
    wall_at(&mut game, WALL, Biome::DataVoid);
    let mut floor = BaseGrid::default();
    floor.lay_floor(WALL.0 - 1, WALL.1);
    let mut map = game.world.resource_mut::<WorldMap>();
    assert!(minable(WALL, &floor, &mut map));
}

#[test]
fn a_blocker_not_beside_floor_is_not_minable() {
    let mut game = game(7);
    wall_at(&mut game, WALL, Biome::DataVoid);
    let mut floor = BaseGrid::default();
    // Diagonal and two-away floor do not count; only orthogonal neighbours.
    floor.lay_floor(WALL.0 - 1, WALL.1 - 1);
    floor.lay_floor(WALL.0 + 2, WALL.1);
    let mut map = game.world.resource_mut::<WorldMap>();
    assert!(!minable(WALL, &floor, &mut map));
}

#[test]
fn walkable_ground_beside_floor_is_not_minable() {
    let mut game = game(7);
    let open = Tile {
        biome: Biome::OpenGrid,
        walkable: true,
        rock_shade: None,
    };
    game.world
        .resource_mut::<WorldMap>()
        .set_override(WALL.0, WALL.1, open);
    let mut floor = BaseGrid::default();
    floor.lay_floor(WALL.0 - 1, WALL.1);
    let mut map = game.world.resource_mut::<WorldMap>();
    assert!(!minable(WALL, &floor, &mut map));
}

#[test]
fn striking_a_field_blocker_excavates_it_and_the_save_keeps_it() {
    let mut game = game(11);
    wall_at(&mut game, WALL, Biome::BlackIce);
    cut_through(&mut game, WALL);
    let tile = game.world.resource_mut::<WorldMap>().tile(WALL.0, WALL.1);
    assert_eq!(tile.biome, Biome::Excavated);
    assert!(tile.walkable);

    let dir = scratch_assets_dir("field_mining_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let tile = loaded.world.resource_mut::<WorldMap>().tile(WALL.0, WALL.1);
    assert_eq!(tile.biome, Biome::Excavated);
    assert!(tile.walkable);
}

#[test]
fn striking_walkable_ground_changes_nothing() {
    let mut game = game(11);
    let player = game.player_entity();
    let open = Tile {
        biome: Biome::OpenGrid,
        walkable: true,
        rock_shade: None,
    };
    game.world
        .resource_mut::<WorldMap>()
        .set_override(WALL.0, WALL.1, open);
    game.strike_field_rock(player, WALL.0, WALL.1);
    let after = game.world.resource_mut::<WorldMap>().tile(WALL.0, WALL.1);
    assert_eq!(after.biome, Biome::OpenGrid);
}

#[test]
fn harder_biomes_take_longer_to_cut() {
    let swings = |biome| {
        let mut game = game(3);
        wall_at(&mut game, WALL, biome);
        cut_through(&mut game, WALL)
    };
    assert!(swings(Biome::BlackIce) >= swings(Biome::DataVoid));
}

#[test]
fn the_fragment_drop_is_deterministic_for_a_seed() {
    let run = || {
        let mut game = game(21);
        let mut found = Vec::new();
        for i in 0..30 {
            let cell = (WALL.0 + i, WALL.1);
            wall_at(&mut game, cell, Biome::DataVoid);
            let before = fragments(&game);
            cut_through(&mut game, cell);
            found.push(fragments(&game) - before);
        }
        found
    };
    let a = run();
    assert_eq!(a, run());
    assert!(a.contains(&1), "30 cuts at 25% paid nothing");
    assert!(a.contains(&0), "30 cuts at 25% always paid");
}

#[test]
fn field_rock_kinds_follow_the_biome() {
    let game = game(1);
    let db = game.world.resource::<crate::rock::RockDb>();
    assert_eq!(db.kind_at(Biome::DataVoid).id, "compacted");
    assert_eq!(db.kind_at(Biome::BlackIce).id, "fused");
    assert_eq!(db.kind_at(Biome::Entropy).id, "entropy");
}
