//! The `Game`-facing half of the tactical battle model.

use crate::Game;
use crate::components::{Creature, Position, Stats};
use crate::resources::DifficultyMode;
use crate::species::SpeciesDb;
use crate::tactical::reach::allowance;
use crate::tests::support::{generic_species, test_assets_dir};
use crate::tuning::{DEFAULT_BASE_SPEED, PLAYER_BASE_SPEED, TACTICAL_MOVE_MAX};
use bevy_ecs::prelude::Entity;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A body of `species`, standing nowhere in particular. Only its species
/// and its speed matter here.
fn body(game: &mut Game, species: &str) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Position { x: 0, y: 0 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 3,
                mitigation: 0,
            },
        ))
        .id()
}

/// The player has no `Creature` and so no species to author anything.
///
/// This pins the shape and not the door: `PLAYER_BASE_SPEED` sits one
/// point above `DEFAULT_BASE_SPEED` and the two land in the same band, so
/// no assertion here could tell `combat_speed` from `species_base_speed`.
/// What it does hold is that the player has an allowance at all and that
/// it comes out of the one derivation everybody else's does.
#[test]
fn the_player_moves_at_their_own_baseline() {
    let game = game();
    let player = game.player_entity();
    assert_eq!(
        game.movement_allowance(player),
        allowance(PLAYER_BASE_SPEED, None)
    );
}

#[test]
fn a_body_with_no_authored_figure_derives_one_from_its_speed() {
    let mut game = game();
    let mut species = generic_species();
    species.base_speed = DEFAULT_BASE_SPEED + 4;
    species.movement = None;
    let id = species.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(species);

    let entity = body(&mut game, &id);
    assert_eq!(
        game.movement_allowance(entity),
        allowance(DEFAULT_BASE_SPEED + 4, None)
    );
}

/// The whole of the escape hatch: a species that authors a figure moves at
/// it, whatever its initiative says.
#[test]
fn an_authored_movement_is_what_that_species_moves() {
    let mut game = game();
    let authored = TACTICAL_MOVE_MAX - 1;
    let mut species = generic_species();
    species.base_speed = DEFAULT_BASE_SPEED;
    species.movement = Some(authored);
    let id = species.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(species);

    let entity = body(&mut game, &id);
    assert_ne!(
        allowance(DEFAULT_BASE_SPEED, None),
        authored,
        "the fixture must be able to tell the authored figure from the derived one"
    );
    assert_eq!(game.movement_allowance(entity), authored);
}

/// The field has to survive the round trip a mod actually takes: a `.ron`
/// file through serde. Nothing shipped authors one, so without this the
/// only proof it works at all goes through `SpeciesDb::insert`, which never
/// parses anything.
#[test]
fn a_species_file_can_author_a_movement_figure() {
    const BARE: &str = r#"(
        id: "test_mover",
        name: "Test Mover",
        glyph: 'm',
        color: White,
        base_hp: 10,
        base_atk: 2,
        base_mitigation: 0,
        taming_difficulty: 0.5,
        habitats: [],
        moves: [],
        work_resource: None,
    )"#;

    let plain: crate::species::SpeciesDef =
        ron::from_str(BARE).expect("the fixture must parse without the field");
    assert_eq!(plain.movement, None, "absent must mean derived");

    let authored: crate::species::SpeciesDef = ron::from_str(&BARE.replace(
        "work_resource: None,",
        "work_resource: None, movement: Some(5),",
    ))
    .expect("the fixture must parse with the field");
    assert_eq!(authored.movement, Some(5));
}
