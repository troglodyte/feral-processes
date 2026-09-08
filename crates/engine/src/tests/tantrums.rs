//! What a base does to the programs working it, and what one of them
//! eventually does back.
//!
//! Three features share this file because they are one chain: a need nothing
//! answers now moves morale, a young base is exempt from all of it, and a
//! program far enough into the hole rounds on a colleague.

use super::support::*;
use crate::components::{Memories, MemorySubject, Position, Structure};
use crate::needs::NeedId;
use crate::structures::{StructureDb, StructureId};
use crate::tuning::{BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES};
use crate::*;

fn coherence() -> NeedId {
    NeedId::from("coherence")
}

/// Every memory `who` holds under `def`, by subject.
fn entries(game: &Game, who: Entity, def: &str) -> Vec<MemorySubject> {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.def.as_str() == def)
                .map(|m| m.subject.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Stands the party in a base with `staff` programs and `structures`
/// buildings in it, so the grace gate can be driven from either side.
///
/// The filler is a Depot, which services no need — a base with an amenity in
/// it is a different fixture, and `fray`'s quiet branch is about a base that
/// has none.
fn a_base_of(game: &mut Game, staff: usize, structures: usize) -> Vec<Entity> {
    stand_in_base(game);
    if structures > 0 {
        place_home(game);
    }
    let def = game
        .world
        .resource::<StructureDb>()
        .get(&StructureId::from("depot"))
        .expect("a Depot ships")
        .clone();
    for i in 1..structures {
        game.spawn_structure(&def, -20 - i as i32, 20, None);
    }
    let mut bodies: Vec<Entity> = (0..staff).map(|_| spawn_tamed(game, 10, 3)).collect();
    bodies.sort();
    for (i, &worker) in bodies.iter().enumerate() {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = -2 - i as i32;
        pos.y = 0;
    }
    bodies
}

/// The counts the fixture is claiming, asserted rather than assumed — a
/// `place_home` that stops standing a structure would otherwise turn every
/// gate test below into a vacuous one.
fn structure_count(game: &Game) -> usize {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<Structure>())
        .count()
}

fn an_established_base(game: &mut Game, staff: usize) -> Vec<Entity> {
    let bodies = a_base_of(game, staff, BASE_ESTABLISHED_STRUCTURES);
    assert_eq!(structure_count(game), BASE_ESTABLISHED_STRUCTURES);
    bodies
}

// ---------------------------------------------------------------------------
// A need nothing answers is remembered
// ---------------------------------------------------------------------------

/// The quiet branch: the base has nothing that services this need at all.
/// The blame is withheld — `Nothing` as a subject holds no tile, machine or
/// colleague responsible — but the meter moves, which is what makes needs
/// reach the morale ladder at all.
#[test]
fn fray_with_no_amenity_writes_ran_down() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = an_established_base(&mut game, BASE_ESTABLISHED_STAFF);

    game.fray(staff[0], &coherence(), false);

    assert_eq!(
        entries(&game, staff[0], "ran_down"),
        vec![MemorySubject::Nothing],
        "a need the base never answered is still felt"
    );
    assert!(
        entries(&game, staff[0], "frayed_here").is_empty(),
        "and it is not the unreachable branch's grudge"
    );
}

/// The other branch is unchanged, and this test is why both are written: one
/// test over either passes against a function that writes the same memory
/// both times.
#[test]
fn fray_with_an_unreachable_amenity_still_writes_frayed_here() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = an_established_base(&mut game, BASE_ESTABLISHED_STAFF);

    game.fray(staff[0], &coherence(), true);

    assert_eq!(
        entries(&game, staff[0], "frayed_here").len(),
        1,
        "the amenity existed and could not be reached: the base earns it"
    );
    assert!(
        entries(&game, staff[0], "ran_down").is_empty(),
        "and not the blame-free one"
    );
}

// ---------------------------------------------------------------------------
// The grace period
// ---------------------------------------------------------------------------

/// Below both thresholds nothing counts against the base, on either branch.
#[test]
fn a_young_base_earns_no_need_grudges() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(&mut game, 2, 2);

    game.fray(staff[0], &coherence(), false);
    game.fray(staff[1], &coherence(), true);

    assert!(entries(&game, staff[0], "ran_down").is_empty());
    assert!(entries(&game, staff[1], "frayed_here").is_empty());
}

/// Plenty of buildings, not enough bodies. Wired `||` this passes and the
/// next one fails, which is the pair's whole point.
#[test]
fn a_base_short_of_staff_earns_no_need_grudges() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(
        &mut game,
        BASE_ESTABLISHED_STAFF - 1,
        BASE_ESTABLISHED_STRUCTURES + 2,
    );

    game.fray(staff[0], &coherence(), false);

    assert!(
        entries(&game, staff[0], "ran_down").is_empty(),
        "a sprawling base with nobody in it is not a pressure cooker"
    );
}

/// And the mirror.
#[test]
fn a_base_short_of_structures_earns_no_need_grudges() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(
        &mut game,
        BASE_ESTABLISHED_STAFF + 2,
        BASE_ESTABLISHED_STRUCTURES - 1,
    );

    game.fray(staff[0], &coherence(), false);

    assert!(
        entries(&game, staff[0], "ran_down").is_empty(),
        "twelve programs in a bare base have not had the chance yet"
    );
}

/// The control: a gate nothing can pass is a deleted feature.
#[test]
fn an_established_base_earns_need_grudges() {
    let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = an_established_base(&mut game, BASE_ESTABLISHED_STAFF);

    game.fray(staff[0], &coherence(), false);

    assert_eq!(entries(&game, staff[0], "ran_down").len(), 1);
}

/// The line is said either way. Only the blame is gated — the player must
/// still be told what their programs are short of, because that line is the
/// errand.
#[test]
fn a_young_base_still_says_the_line() {
    let mut game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(&mut game, 2, 2);

    game.fray(staff[0], &coherence(), false);

    assert!(
        game.message_history(50)
            .iter()
            .any(|row| row.text.contains("nothing in the base")),
        "the errand is still handed to the player"
    );
}
