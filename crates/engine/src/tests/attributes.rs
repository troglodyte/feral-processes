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

/// The creature standing on `tile`, read straight out of the world — a
/// local helper rather than a `Game` method, because nothing outside a test
/// wants to find a body by its coordinates.
fn creature_attributes_at(game: &Game, tile: (i32, i32)) -> Option<crate::components::Attributes> {
    let mut query = game
        .world
        .try_query::<(
            &crate::components::Position,
            &crate::components::Attributes,
            &crate::components::Creature,
        )>()
        .unwrap();
    query
        .iter(&game.world)
        .find(|(p, _, _)| (p.x, p.y) == tile)
        .map(|(_, a, _)| a.clone())
}

/// A real file round trip, not a RON round trip: a field left out of
/// `creature_save_for`, or marked `#[serde(skip)]`, leaves a
/// serialize-then-deserialize test perfectly green.
#[test]
fn attributes_survive_a_save_and_a_load() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_attributes_roundtrip_{}.bin",
        std::process::id()
    ));
    let mut game = Game::new(4246, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let wild = game
        .spawn_wild_creature_scaled(&species, 80, 80, 1.0, false)
        .unwrap();
    let before = game
        .world
        .get::<crate::components::Attributes>(wild)
        .cloned()
        .unwrap();
    let player_before = game
        .world
        .get::<crate::components::Attributes>(game.player_entity())
        .cloned()
        .unwrap();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let found = creature_attributes_at(&loaded, (80, 80)).expect("the creature came back");
    assert_eq!(
        found, before,
        "a creature's attributes changed across a load"
    );
    assert_eq!(
        loaded
            .world
            .get::<crate::components::Attributes>(loaded.player_entity())
            .cloned()
            .unwrap(),
        player_before,
        "the player's attributes changed across a load"
    );
}

/// A save written before this feature carries no key, so the load mints —
/// which is what gets a run in progress its attributes with no migration
/// and no version bump. Modelled by clearing the field on the way out,
/// which is exactly what an older file's absent key deserialises to.
#[test]
fn an_old_save_mints_on_load_rather_than_staying_blank() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_attributes_old_save_{}.bin",
        std::process::id()
    ));
    let mut game = Game::new(4247, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    game.spawn_wild_creature_scaled(&species, 90, 90, 1.0, false)
        .unwrap();
    game.save(&path).unwrap();

    let mut data = crate::save::load_from_file(&path).unwrap();
    for c in &mut data.creatures {
        c.attributes.clear();
    }
    data.player.attributes.clear();
    crate::save::save_to_file(&path, &data).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        creature_attributes_at(&loaded, (90, 90)).map(|a| a.iter().count()),
        Some(5),
        "an old save's creature stayed blank instead of minting"
    );
    assert_eq!(
        loaded
            .world
            .get::<crate::components::Attributes>(loaded.player_entity())
            .map(|a| a.iter().count()),
        Some(5),
        "an old save's player stayed blank instead of minting"
    );
}

/// And what it mints is what a fresh spawn on that tile would have minted,
/// which is what makes the mint a property of the place rather than of when
/// you happened to load.
#[test]
fn a_minted_old_save_agrees_with_a_fresh_spawn() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_attributes_agree_{}.bin",
        std::process::id()
    ));
    // Spawned fresh, and never saved: this is the answer the place gives.
    let mut fresh = Game::new(4251, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = fresh.species_defs()[0].id.to_string();
    let body = fresh
        .spawn_wild_creature_scaled(&species, 95, 95, 1.0, false)
        .unwrap();
    let expected = fresh
        .world
        .get::<crate::components::Attributes>(body)
        .cloned()
        .unwrap();
    // The same world, saved with the key stripped, so the load has to mint.
    fresh.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    for c in &mut data.creatures {
        c.attributes.clear();
    }
    crate::save::save_to_file(&path, &data).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        creature_attributes_at(&loaded, (95, 95)),
        Some(expected),
        "the load minted a different body than the place would have"
    );
}
