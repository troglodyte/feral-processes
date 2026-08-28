//! Sorties: the role, the catalogue, the board, dispatch, the trip and the
//! return.

use bevy_ecs::prelude::Entity;

use super::support::{scratch_assets_dir, stand_in_base, test_assets_dir};
use crate::Game;
use crate::components::{Glyph, GlyphColor, Position, Structure};
use crate::game::party::ProgramRole;
use crate::game::sortie::SortieReach;
use crate::resources::DifficultyMode;
use crate::resources::{Sortie, Sorties};
use crate::sorties::SortieDb;

// ------------------------------------------------------------- the role

/// A program named by an in-flight sortie is `Sortie`, not `Staff` — and
/// `Staff` stays the leftover rather than becoming something assigned.
#[test]
fn a_dispatched_program_is_not_staff() {
    let mut game = Game::new(4200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("test roster program");

    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert_eq!(
        game.program_role(program),
        Some(ProgramRole::Sortie),
        "a program named by an in-flight sortie has left the labour pool"
    );
}

/// The map and the examine ray both drop an away program, and neither
/// needed a new rule: `position_is_honest` tests for `Staff` exactly.
#[test]
fn a_dispatched_program_leaves_the_map() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("test roster program");

    assert!(game.position_is_honest(program), "idle staff are drawn");

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert!(
        !game.position_is_honest(program),
        "an away program must not claim a tile it is not standing on"
    );
}

/// The base's labour pool is what the scheduler, the drift pass and the
/// entropy pass all read, and an away program is out of it by omission
/// rather than by a check at each of the three.
#[test]
fn a_dispatched_program_leaves_the_labour_pool() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("test roster program");

    assert!(game.base_staff().contains(&program));

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert!(
        !game.base_staff().contains(&program),
        "an away program is not staff, so nothing in the base can post it"
    );
}

// ------------------------------------------------------ the catalogue

/// An absent directory is a supported install: it loads empty, warns about
/// nothing, and the feature is simply absent. `NeedDb` and `MemoryDb`'s
/// property, and the reason nothing may ever gate on the db being
/// non-empty.
#[test]
fn an_absent_catalogue_loads_empty_and_quiet() {
    let (db, warnings) = SortieDb::load_dir(std::path::Path::new("/nonexistent/sorties"))
        .expect("an absent directory is not an error");
    assert!(db.is_empty());
    assert!(warnings.is_empty(), "an absent directory is not a fault");
}

/// A malformed file is skipped with a warning, never a panic that takes
/// startup down with it.
#[test]
fn a_malformed_site_is_skipped_with_a_warning() {
    let scratch = scratch_assets_dir("sortie_malformed");
    let dir = scratch.join("sorties");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.ron"), "(this is not ron").unwrap();
    std::fs::write(
        dir.join("good.ron"),
        r#"(
    id: "good",
    name: "A Good Site",
    description: "Fine.",
    risk: 0,
    battles_min: 4,
    battles_max: 6,
)"#,
    )
    .unwrap();

    let (db, warnings) = SortieDb::load_dir(&dir).unwrap();
    assert_eq!(db.iter().count(), 1, "the good file still loads");
    assert_eq!(warnings.len(), 1, "the broken one is reported, not fatal");
}

/// Sorted by id, because every caller walks it and an unsorted walk is how
/// a derived board stops being reproducible.
#[test]
fn the_catalogue_iterates_in_id_order() {
    let (db, _) = SortieDb::load_dir(&test_assets_dir().join("sorties")).unwrap();
    let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
}

/// A site whose battle range is inverted or empty is a content fault and is
/// refused at load, the way `field_buff_duration_mismatch` refuses its
/// corners — a `battles_max` below `battles_min` would silently roll an
/// empty range at board time, far from the file that caused it.
#[test]
fn an_inverted_battle_range_is_refused_at_load() {
    let scratch = scratch_assets_dir("sortie_inverted");
    let dir = scratch.join("sorties");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("backwards.ron"),
        r#"(
    id: "backwards",
    name: "Backwards",
    description: "Bad.",
    risk: 1,
    battles_min: 9,
    battles_max: 3,
)"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("empty.ron"),
        r#"(
    id: "empty",
    name: "Empty",
    description: "Also bad.",
    risk: 0,
    battles_min: 0,
    battles_max: 3,
)"#,
    )
    .unwrap();

    let (db, warnings) = SortieDb::load_dir(&dir).unwrap();
    assert!(db.is_empty());
    assert_eq!(warnings.len(), 2);
}

// ------------------------------------------------- the risk offset

/// A risk offset reaches the same window `depth` does, so a sortie can ask
/// for tougher opposition without the caller re-deriving the biome rules.
#[test]
fn a_risk_offset_raises_the_habitat_window() {
    let mut game = Game::new(4300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::resources::ZoneLevel(1));
    // Well clear of the opening ring, which filters the pool down to what a
    // fresh player can beat and would mask the window moving at all.
    let (x, y) = (400, 400);

    let Some((base, _)) = game.habitat_pools(x, y, None, 0) else {
        panic!("this tile should be habitable");
    };
    let Some((raised, _)) = game.habitat_pools(x, y, None, 4) else {
        panic!("a raised window should still resolve");
    };

    assert_ne!(
        base, raised,
        "a four-step offset must move the window, or the parameter is inert"
    );
}

/// Zero is exactly today's behaviour, which is what lets every existing
/// caller pass it and nothing move. Asserted against a hand-rolled window at
/// the same step rather than against itself — comparing one call to another
/// passes against an offset that does nothing at all.
#[test]
fn a_zero_offset_is_todays_window() {
    let mut game = Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::resources::ZoneLevel(3));
    let (x, y) = (400, 400);

    let step = game.danger_steps(None);
    let biome = game
        .world
        .resource_mut::<crate::world::WorldMap>()
        .tile(x, y)
        .biome;
    let expected: Vec<String> = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .windowed_matches(biome, step)
        .into_iter()
        .map(|s| s.id.clone())
        .collect();

    let (ordinary, _) = game
        .habitat_pools(x, y, None, 0)
        .expect("this tile should be habitable");
    assert_eq!(ordinary, expected);
}

// ------------------------------------------------------------ the Relay

/// Stands a Home and a Relay up in base space and puts the party on the
/// laid floor beside them, `deploy_broker`'s shape. A Relay without a Home
/// does not survive a save the test later loads.
fn deploy_relay(game: &mut Game) {
    game.lay_starting_pocket();
    deploy(game, "home", 0, 0);
    deploy(game, "relay", 1, 0);
    super::support::stand_in_base_at(game, 1, 1);
}

/// A structure of `kind` standing at `(x, y)` in base space.
fn deploy(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            Glyph {
                ch: 'K',
                color: GlyphColor::Magenta,
            },
        ))
        .id()
}

/// Three states rather than two booleans, `NoPost::BoxedIn`'s rule: "no
/// Relay built" and "not standing in base" leave the player different
/// errands, and a screen that cannot tell them apart says the wrong
/// sentence.
#[test]
fn sortie_reach_reports_the_three_states() {
    let mut game = Game::new(4400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.sortie_reach(), SortieReach::NoRelay);

    deploy_relay(&mut game);
    assert_eq!(game.sortie_reach(), SortieReach::AtRelay);

    // Out of base space entirely — on the open grid, where no floor can
    // answer for the party.
    game.world
        .insert_resource(crate::resources::Locale::Surface);
    assert_eq!(game.sortie_reach(), SortieReach::OffBase);
}

/// It measures the base and not the distance to the mast: a Relay stands on
/// laid floor by construction, so its own tile says nothing the base does
/// not. The far edge of the pocket is the case the rule exists for.
#[test]
fn the_far_side_of_the_base_still_reaches_the_relay() {
    let mut game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let edge = crate::tuning::STARTING_POCKET_RADIUS;
    assert!(edge - 1 > 2, "the far edge must be beyond arm's length");
    super::support::stand_in_base_at(&mut game, edge, 0);
    assert_eq!(game.sortie_reach(), SortieReach::AtRelay);
}
