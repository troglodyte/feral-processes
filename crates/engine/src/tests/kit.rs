//! `Game::kit_of`: the one answer to where a body's kit comes from.
//!
//! The player-side tests pin what the four readers answered for the player
//! before they became matches on `Kit`, so converting them has a witness.

use super::support::*;
use super::tactical::body;
use crate::game::kit::Kit;
use crate::tuning::{PLAYER_UNARMED_DAMAGE, TACTICAL_MELEE_RANGE};
use crate::{DifficultyMode, Game};

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn innate_species(game: &Game, entity: bevy_ecs::entity::Entity) -> Option<String> {
    match game.kit_of(entity) {
        Kit::Innate(def) => Some(def.id.clone()),
        Kit::Unarmed => None,
    }
}

#[test]
fn the_player_is_unarmed() {
    let game = game();
    assert!(matches!(game.kit_of(game.player_entity()), Kit::Unarmed));
}

#[test]
fn a_wild_program_carries_its_species_kit() {
    let mut game = game();
    let drone = body(&mut game, "drone");
    assert_eq!(innate_species(&game, drone).as_deref(), Some("drone"));
}

#[test]
fn a_companion_carries_its_species_kit() {
    let mut game = game();
    let pet = spawn_tamed(&mut game, 30, 6);
    let species = game
        .world
        .get::<crate::Creature>(pet)
        .unwrap()
        .species
        .clone();
    assert_eq!(innate_species(&game, pet), Some(species));
}

/// A body whose species no longer resolves — a mod pulled out from under a
/// save — has no kit to borrow, and reads as unarmed rather than panicking.
#[test]
fn a_body_of_an_unknown_species_is_unarmed() {
    let mut game = game();
    let stray = body(&mut game, "no_such_species");
    assert!(matches!(game.kit_of(stray), Kit::Unarmed));
}

#[test]
fn the_unarmed_player_swings_a_data_strike() {
    let mut game = game();
    let player = game.player_entity();
    let (name, band) = game.swing_move_at(player, None);
    assert_eq!(name, "data strike");
    assert_eq!(band, PLAYER_UNARMED_DAMAGE);
}

#[test]
fn the_unarmed_players_natural_band_is_the_unarmed_band() {
    let game = game();
    let player = game.player_entity();
    assert_eq!(
        game.natural_range_of(player),
        game.attack_range(player, PLAYER_UNARMED_DAMAGE)
    );
}

#[test]
fn the_unarmed_player_swings_at_arms_length() {
    let game = game();
    assert_eq!(game.swing_range(game.player_entity()), TACTICAL_MELEE_RANGE);
}
