//! The attribute feature's engine tests: the four mint doors, the save
//! round trip, and `Game::dossier_report`.
//!
//! `attributes.rs`'s own `mod tests` holds the catalogue, the mint's
//! arithmetic and the derived header, which need no `Game`. What is here
//! needs one.

use super::support::*;
use crate::*;

/// **The test that protects every seeded baseline in the repo.** If the
/// mint ever draws from `resources::GameRng`, the next roll in the run
/// moves — and with it `balance_sim`'s curves, the arena's reports and
/// every test that spawns against a fixed seed.
///
/// Measured on the *second* creature's `Potential`, because that is the
/// first value a draw inside the first spawn would displace. An empty
/// catalogue stands in for "the mint does no work": if the two agree, the
/// mint's work is off the stream.
#[test]
fn minting_attributes_spends_no_rng_draw() {
    let rolls = |empty: bool| {
        let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        if empty {
            game.world
                .insert_resource(crate::attributes::AttributeDb::default());
        }
        let species = game.species_defs()[0].id.to_string();
        let a = game
            .spawn_wild_creature_scaled(&species, 40, 40, 1.0, false)
            .unwrap();
        let b = game
            .spawn_wild_creature_scaled(&species, 41, 40, 1.0, false)
            .unwrap();
        [a, b].map(|e| {
            let p = game.world.get::<crate::components::Potential>(e).unwrap();
            (p.hp_roll, p.atk_roll, p.def_roll)
        })
    };
    assert_eq!(
        rolls(false),
        rolls(true),
        "minting an attribute moved the RNG stream"
    );
}

/// Every wild body has attributes, and two of one species on two tiles
/// differ — which is the whole observable point of the mint.
#[test]
fn two_bodies_of_one_species_on_two_tiles_differ() {
    let mut game = Game::new(4243, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let a = game
        .spawn_wild_creature_scaled(&species, 60, 60, 1.0, false)
        .unwrap();
    let b = game
        .spawn_wild_creature_scaled(&species, 61, 77, 1.0, false)
        .unwrap();
    let of = |e| {
        game.world
            .get::<crate::components::Attributes>(e)
            .cloned()
            .unwrap()
    };
    assert!(!of(a).is_empty(), "a wild body minted no attributes");
    assert_ne!(of(a), of(b), "two bodies on two tiles minted identically");
}

/// The player's own, off the class it chose. A run with no class is
/// supported — `CharacterChoice::default()` has none — and mints the
/// catalogue's bases.
#[test]
fn the_player_has_attributes_from_its_class() {
    let game = Game::new(4244, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let attrs = game
        .world
        .get::<crate::components::Attributes>(player)
        .expect("the player mints attributes at creation");
    assert_eq!(attrs.iter().count(), 5);
}

/// Taming does not rewrite who a program is. A captured body keeps the
/// attributes it had in the wild, which is why the mint is not in
/// `Game::roster_parts`.
#[test]
fn adopting_a_program_keeps_the_attributes_it_had_wild() {
    let mut game = Game::new(4245, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let wild = game
        .spawn_wild_creature_scaled(&species, 70, 70, 1.0, false)
        .unwrap();
    let before = game
        .world
        .get::<crate::components::Attributes>(wild)
        .cloned()
        .unwrap();
    let parts = game.roster_parts();
    game.world.entity_mut(wild).insert(parts);
    let after = game
        .world
        .get::<crate::components::Attributes>(wild)
        .cloned()
        .unwrap();
    assert_eq!(
        before, after,
        "joining the roster rewrote the program's attributes"
    );
}
