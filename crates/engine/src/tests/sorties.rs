//! Sorties: the role, the catalogue, the board, dispatch, the trip and the
//! return.

use super::support::{scratch_assets_dir, stand_in_base, test_assets_dir};
use crate::Game;
use crate::game::party::ProgramRole;
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
