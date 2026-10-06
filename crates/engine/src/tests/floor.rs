//! Floor piles: where an interrupted carrier's load goes.

use super::support::*;
use crate::components::{Carrying, FloorPile};
use crate::game::base::floor::{drop_load, floor_pile_at, take_from_pile};
use crate::*;

fn game() -> Game {
    Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn carrier(game: &mut Game, x: i32, y: i32, item: &str, qty: u32) -> Entity {
    game.world
        .spawn((
            Position { x, y },
            Carrying {
                item: ItemId::from(item),
                qty,
            },
        ))
        .id()
}

fn pile_items(game: &mut Game, x: i32, y: i32) -> Vec<(ItemId, u32)> {
    let e = floor_pile_at(&mut game.world, Position { x, y }).expect("a pile");
    game.world
        .get::<FloorPile>(e)
        .unwrap()
        .items
        .iter()
        .map(|(i, n)| (i.clone(), *n))
        .collect()
}

fn pile_count(game: &mut Game) -> usize {
    game.world.query::<&FloorPile>().iter(&game.world).count()
}

#[test]
fn drop_makes_a_pile_on_the_carriers_tile_and_ends_the_hold() {
    let mut g = game();
    let who = carrier(&mut g, 3, 4, ids::CORE_FRAGMENT, 5);
    drop_load(&mut g.world, who);
    assert!(g.world.get::<Carrying>(who).is_none());
    assert_eq!(
        pile_items(&mut g, 3, 4),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]
    );
}

#[test]
fn two_drops_on_one_tile_merge_into_one_pile() {
    let mut g = game();
    let a = carrier(&mut g, 1, 1, ids::CORE_FRAGMENT, 2);
    let b = carrier(&mut g, 1, 1, ids::CORE_FRAGMENT, 3);
    drop_load(&mut g.world, a);
    drop_load(&mut g.world, b);
    assert_eq!(pile_count(&mut g), 1);
    assert_eq!(
        pile_items(&mut g, 1, 1),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]
    );
}

#[test]
fn drop_with_no_load_is_a_no_op() {
    let mut g = game();
    let who = g.world.spawn(Position { x: 0, y: 0 }).id();
    drop_load(&mut g.world, who);
    assert_eq!(pile_count(&mut g), 0);
}

#[test]
fn take_from_pile_takes_partial_then_whole_and_despawns() {
    let mut g = game();
    let who = carrier(&mut g, 2, 2, ids::CORE_FRAGMENT, 5);
    drop_load(&mut g.world, who);
    let pile = floor_pile_at(&mut g.world, Position { x: 2, y: 2 }).unwrap();
    let item = ItemId::from(ids::CORE_FRAGMENT);
    assert_eq!(take_from_pile(&mut g.world, pile, &item, 2), 2);
    assert_eq!(pile_items(&mut g, 2, 2), vec![(item.clone(), 3)]);
    assert_eq!(take_from_pile(&mut g.world, pile, &item, 99), 3);
    assert!(g.world.get_entity(pile).is_err(), "emptied pile despawns");
}

#[test]
fn take_from_a_missing_pile_returns_zero() {
    let mut g = game();
    let who = carrier(&mut g, 2, 2, ids::CORE_FRAGMENT, 1);
    drop_load(&mut g.world, who);
    let pile = floor_pile_at(&mut g.world, Position { x: 2, y: 2 }).unwrap();
    let item = ItemId::from(ids::CORE_FRAGMENT);
    take_from_pile(&mut g.world, pile, &item, 1);
    assert_eq!(take_from_pile(&mut g.world, pile, &item, 1), 0);
}

#[test]
fn a_pile_survives_save_and_load() {
    let mut g = game();
    let who = carrier(&mut g, -3, 7, ids::CORE_FRAGMENT, 4);
    drop_load(&mut g.world, who);
    let dir = std::env::temp_dir().join("feral-floor-pile-save");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("floor.bin");
    g.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(pile_count(&mut loaded), 1);
    assert_eq!(
        pile_items(&mut loaded, -3, 7),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 4)]
    );
}

#[test]
fn a_save_without_the_field_loads_with_no_piles() {
    let data: crate::save::SaveData = {
        let mut g = game();
        let dir = std::env::temp_dir().join("feral-floor-pile-old");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.bin");
        g.save(&path).unwrap();
        crate::save::load_from_file(&path).unwrap()
    };
    let text = ron::to_string(&data).unwrap();
    let stripped = text.replacen("floor_piles:[],", "", 1);
    assert_ne!(text, stripped, "the field is serialised by name");
    let parsed: crate::save::SaveData = ron::from_str(&stripped).unwrap();
    assert!(parsed.floor_piles.is_empty());
}
