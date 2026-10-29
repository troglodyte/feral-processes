//! Floor is refused on the field's own fixtures — a Stack link, a
//! settlement's footprint, an outpost — so the base grows around them, never
//! over them (ADR 0032, placement conflicts).

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::*;

/// A cell just past the fixture pocket's east edge, open ground.
const OUTSIDE: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 1, 0);

fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world.resource_mut::<WorldMap>().set_override(
        OUTSIDE.0,
        OUTSIDE.1,
        Tile {
            biome: Biome::OpenGrid,
            walkable: true,
            rock_shade: None,
        },
    );
    game
}

/// A dig mark is a floor request, so a box over `OUTSIDE` files nothing,
/// and the cell stays unfloored.
fn assert_floor_refused(game: &mut Game) {
    assert!(game.floor_conflict(OUTSIDE.0, OUTSIDE.1).is_some());
    game.toggle_mark_box(OUTSIDE, OUTSIDE, None);
    assert!(
        game.dig_site_at(OUTSIDE.0, OUTSIDE.1).is_none(),
        "a floor request was filed on the fixture's cell"
    );
    assert!(!game.world.resource::<BaseGrid>().is_floor(OUTSIDE.0, OUTSIDE.1));
}

#[test]
fn floor_is_refused_on_a_stack_link() {
    let mut game = base(9101);
    game.spawn_entrance_at(OUTSIDE.0, OUTSIDE.1);
    assert_floor_refused(&mut game);
}

#[test]
fn floor_is_refused_on_a_settlement() {
    let mut game = base(9102);
    place_settlement(
        &mut game,
        crate::settlements::SettlementKey { rx: 91, ry: 2 },
        OUTSIDE.0,
        OUTSIDE.1,
    );
    assert_floor_refused(&mut game);
}

#[test]
fn floor_is_refused_on_an_outpost() {
    let mut game = base(9103);
    game.world
        .resource_mut::<crate::resources::Outposts>()
        .0
        .insert(OUTSIDE, super::outposts::a_founded_outpost());
    assert_floor_refused(&mut game);
}

/// The control: the same cell with nothing on it takes the mark, so the
/// three refusals above are about the fixture and not the cell.
#[test]
fn the_same_open_cell_takes_a_floor_request() {
    let mut game = base(9104);
    assert!(game.floor_conflict(OUTSIDE.0, OUTSIDE.1).is_none());
    game.toggle_mark_box(OUTSIDE, OUTSIDE, None);
    assert!(game.dig_site_at(OUTSIDE.0, OUTSIDE.1).is_some());
}

/// Laying a tile by hand meets the same refusal: a player standing in a
/// settlement's footprint cannot floor it.
#[test]
fn laying_a_tile_by_hand_is_refused_on_a_settlement() {
    let mut game = base(9105);
    place_settlement(
        &mut game,
        crate::settlements::SettlementKey { rx: 91, ry: 3 },
        OUTSIDE.0,
        OUTSIDE.1,
    );
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 10);
    stand_in_base_at(&mut game, OUTSIDE.0, OUTSIDE.1);
    assert_eq!(
        game.lay_tile(),
        Err("That ground belongs to a settlement.".to_string())
    );
}
