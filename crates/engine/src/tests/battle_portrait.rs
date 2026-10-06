//! The battle screen's picture window: who it shows and what it looks like.

use super::support::*;
use crate::*;

fn a_game() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn a_hostile(game: &mut Game, hp: i32) -> Entity {
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    game.world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Glyph {
                ch: species.glyph,
                color: GlyphColor::Red,
            },
            Position { x: 3, y: 3 },
            Stats {
                hp,
                max_hp: hp,
                atk: 20,
                mitigation: 1,
            },
            StatusEffects::default(),
        ))
        .id()
}

#[test]
fn a_hostiles_portrait_carries_its_species_sprite_and_glyph() {
    let mut game = a_game();
    let wild = a_hostile(&mut game, 50);
    let def = game.species_defs().into_iter().next().unwrap();

    let portrait = game.portrait_of(wild).expect("a creature has a portrait");

    assert_eq!(portrait.sprite.as_deref(), Some(def.sprite_name()));
    assert_eq!(portrait.glyph, def.glyph);
    assert_eq!(portrait.color, GlyphColor::Red);
    assert!(!portrait.drawn_icon, "only the player has a drawn icon");
}

#[test]
fn a_player_with_a_drawn_icon_asks_for_it() {
    let mut game = a_game();
    let player = game.player_entity();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = Some(PlayerIcon::default());

    assert!(game.portrait_of(player).unwrap().drawn_icon);
}

#[test]
fn a_player_without_a_drawn_icon_does_not() {
    let mut game = a_game();
    let player = game.player_entity();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = None;

    assert!(!game.portrait_of(player).unwrap().drawn_icon);
}

/// The map draws an emulating player as the form, never as the drawn icon;
/// the portrait must agree or the two would show different bodies.
#[test]
fn an_emulating_player_shows_the_form_not_the_drawn_icon() {
    let mut game = a_game();
    let player = game.player_entity();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = Some(PlayerIcon::default());
    game.world.entity_mut(player).insert(Emulation {
        species: "drone".into(),
        rounds_left: 3,
    });
    let drone = game.form_look(player).expect("drone is a real species");

    let portrait = game.portrait_of(player).unwrap();

    assert!(!portrait.drawn_icon);
    assert_eq!(portrait.sprite, drone.sprite);
    assert_eq!(portrait.glyph, drone.glyph);
}
