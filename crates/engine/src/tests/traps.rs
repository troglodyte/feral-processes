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
