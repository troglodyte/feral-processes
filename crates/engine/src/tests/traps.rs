//! Honeypots: the entity, its placement, its tick, and what walking into
//! one does.

use super::support::*;
use crate::components::{TRAP_GLYPH_ARMED, TRAP_GLYPH_SPRUNG, Trap};
use crate::*;

fn honeypot() -> ItemId {
    ItemId::from("honeypot")
}

fn player_tile(game: &Game) -> Position {
    *game.world.get::<Position>(game.player_entity()).unwrap()
}

/// A trap stood on a tile by hand, which is what every test that is not
/// about `place_trap`'s own refusals wants.
fn stand_a_trap(game: &mut Game, x: i32, y: i32, caught: Option<items::DownedProgram>) -> Entity {
    let ch = if caught.is_some() {
        TRAP_GLYPH_SPRUNG
    } else {
        TRAP_GLYPH_ARMED
    };
    game.world
        .spawn((
            Trap {
                item: honeypot(),
                next_roll: crate::tuning::TRAP_PERIOD_TICKS,
                caught,
            },
            Position { x, y },
            Glyph {
                ch,
                color: GlyphColor::Yellow,
            },
        ))
        .id()
}

fn a_caught_program() -> items::DownedProgram {
    items::DownedProgram {
        species: "shellmon".to_string(),
        level: 3,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 60,
        carried: None,
    }
}

#[test]
fn a_trap_is_named_by_its_item_and_says_whether_it_is_sprung() {
    let mut game = Game::new(7001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = player_tile(&game);

    let armed = stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    let sprung = stand_a_trap(&mut game, pos.x + 2, pos.y, Some(a_caught_program()));

    let armed_label = game.entity_label(armed);
    let sprung_label = game.entity_label(sprung);
    // Asked of the db, never hand-typed, so a renamed item moves the test
    // with the asset.
    let name = game.item_name(&honeypot()).to_string();
    assert!(
        armed_label.contains(&name),
        "an armed trap should name its item, got {armed_label:?}"
    );
    assert!(
        sprung_label.contains(&name),
        "a sprung trap should name its item too, got {sprung_label:?}"
    );
    // The wording is copy; the distinction is the contract, and a line
    // reading the same for both states is the one thing `x` exists to
    // answer.
    assert_ne!(
        armed_label, sprung_label,
        "a sprung trap must not read as an armed one"
    );
}

#[test]
fn find_trap_at_answers_the_tile_it_stands_on_and_nothing_beside_it() {
    let mut game = Game::new(7002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = player_tile(&game);
    let trap = stand_a_trap(&mut game, pos.x + 1, pos.y, None);

    assert_eq!(game.find_trap_at(pos.x + 1, pos.y), Some(trap));
    assert_eq!(game.find_trap_at(pos.x + 2, pos.y), None);
}

#[test]
fn trap_count_is_zero_on_a_fresh_game_and_counts_what_stands() {
    let mut game = Game::new(7003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.trap_count(), 0, "nothing places one at Game::new");
    let pos = player_tile(&game);
    stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    stand_a_trap(&mut game, pos.x + 2, pos.y, Some(a_caught_program()));
    assert_eq!(game.trap_count(), 2);
}

/// Clears the tile `(dx, dy)` from the party of anything the shipped world
/// might have put there and makes it walkable, so a placement test is about
/// the refusal it names rather than about where the seed landed.
fn clear_target(game: &mut Game, dx: i32, dy: i32) -> (i32, i32) {
    let pos = player_tile(game);
    let (nx, ny) = (pos.x + dx, pos.y + dy);
    let squatters: Vec<Entity> = {
        let mut q = game.world.query::<(Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(e, p)| *e != game.player_entity() && p.x == nx && p.y == ny)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    game.world.resource_mut::<WorldMap>().set_override(
        nx,
        ny,
        Tile {
            biome: Biome::Platform,
            walkable: true,
            rock_shade: None,
        },
    );
    (nx, ny)
}

/// A game holding `qty` honeypots with a clear tile to the east.
fn ready_to_place(seed: u32, qty: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(&mut game, &honeypot(), qty);
    clear_target(&mut game, 1, 0);
    game
}

/// Every refusal asserts the `Err` *and* that nothing moved. A refusal test
/// that only checks the `Err` is the vacuous half — five of these six paths
/// never spend anything anyway.
fn assert_refused(game: &mut Game, result: Result<(), String>, held_before: u32) {
    assert!(result.is_err(), "expected a refusal, got {result:?}");
    assert_eq!(
        held(game, &honeypot()),
        held_before,
        "a refusal must not spend the item"
    );
    assert_eq!(game.trap_count(), 0, "a refusal must not stand anything up");
}

#[test]
fn placing_a_honeypot_spends_one_and_stands_it_on_the_tile() {
    let mut game = ready_to_place(7010, 2);
    let (nx, ny) = {
        let p = player_tile(&game);
        (p.x + 1, p.y)
    };

    game.place_trap(&honeypot(), 1, 0).expect("a clear tile");

    assert_eq!(held(&game, &honeypot()), 1, "one unit is spent");
    assert_eq!(game.trap_count(), 1);
    let trap = game
        .find_trap_at(nx, ny)
        .expect("it stands where it was put");
    let placed = game.world.get::<Trap>(trap).unwrap();
    assert_eq!(placed.item, honeypot());
    assert!(placed.caught.is_none(), "a fresh trap has caught nothing");
    assert_eq!(
        placed.next_roll,
        crate::tuning::TRAP_PERIOD_TICKS,
        "the countdown starts full, so nothing is caught on the tick it was set"
    );
    assert_eq!(
        game.world.get::<Glyph>(trap).map(|g| g.ch),
        Some(TRAP_GLYPH_ARMED)
    );
    assert_eq!(
        player_tile(&game).x,
        nx - 1,
        "placing is not a step; the player stays put"
    );
}

#[test]
fn placing_underground_is_refused() {
    let mut game = ready_to_place(7011, 1);
    descend(&mut game);
    let result = game.place_trap(&honeypot(), 1, 0);
    assert_refused(&mut game, result, 1);
}

#[test]
fn placing_an_item_you_do_not_hold_is_refused() {
    let mut game = ready_to_place(7012, 0);
    let result = game.place_trap(&honeypot(), 1, 0);
    assert_refused(&mut game, result, 0);
}

#[test]
fn placing_an_item_with_no_trap_def_is_refused() {
    let mut game = ready_to_place(7013, 1);
    let plain = ItemId::from("ice_breaker");
    give(&mut game, &plain, 1);
    let before = held(&game, &plain);
    let result = game.place_trap(&plain, 1, 0);
    assert!(result.is_err(), "ordinary cargo is not placeable");
    assert_eq!(held(&game, &plain), before, "and nothing is spent");
    assert_eq!(game.trap_count(), 0);
}

#[test]
fn placing_at_the_cap_is_refused() {
    let mut game = ready_to_place(7014, 1);
    let pos = player_tile(&game);
    // Stood up well away from the party, so the cap is the only thing the
    // call can be refusing.
    for i in 0..crate::tuning::TRAP_PLACEMENT_CAP as i32 {
        stand_a_trap(&mut game, pos.x + 50 + i, pos.y + 50, None);
    }
    let result = game.place_trap(&honeypot(), 1, 0);
    assert!(result.is_err(), "the cap refuses");
    assert_eq!(held(&game, &honeypot()), 1, "and spends nothing");
    assert_eq!(game.trap_count(), crate::tuning::TRAP_PLACEMENT_CAP);
}

#[test]
fn placing_on_unwalkable_ground_is_refused() {
    let mut game = ready_to_place(7015, 1);
    let pos = player_tile(&game);
    game.world.resource_mut::<WorldMap>().set_override(
        pos.x + 1,
        pos.y,
        Tile {
            biome: Biome::DataVoid,
            walkable: false,
            rock_shade: None,
        },
    );
    let result = game.place_trap(&honeypot(), 1, 0);
    assert_refused(&mut game, result, 1);
}

#[test]
fn placing_onto_an_occupied_tile_is_refused() {
    let mut game = ready_to_place(7016, 1);
    let pos = player_tile(&game);
    stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    let result = game.place_trap(&honeypot(), 1, 0);
    assert!(result.is_err(), "one trap to a tile");
    assert_eq!(held(&game, &honeypot()), 1);
    assert_eq!(game.trap_count(), 1, "the standing one is untouched");
}
