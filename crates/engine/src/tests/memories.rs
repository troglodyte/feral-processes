//! Entity memories — Phase 1: the identity a memory is *about*.
//!
//! `Entity` is written to the save nowhere, because entity ids are not
//! stable across a round trip. `ProgramId` is what a later phase's memory
//! names instead, minted at `Game::roster_parts` — the one barrier all four
//! doors into the roster pass through.

use super::support::*;
use crate::components::ProgramId;
use crate::*;

/// Minting is per-call, not a constant. Two doors, two programs, two ids —
/// and neither may be the unassigned sentinel.
#[test]
fn two_programs_through_different_doors_take_different_ids() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let adopted = game.adopt_program("scrapper", 4, 4, 1.0).unwrap();
    let fixture = spawn_tamed(&mut game, 10, 3);

    let a = game.world.get::<ProgramId>(adopted).map(|p| p.0);
    let b = game.world.get::<ProgramId>(fixture).map(|p| p.0);

    assert!(matches!(a, Some(n) if n != 0), "an adopted program: {a:?}");
    assert!(matches!(b, Some(n) if n != 0), "a captured program: {b:?}");
    assert_ne!(a, b, "two programs may never share an identity");
}

/// `fuse_companions` is the door that hand-writes its own component list, so
/// it is the one that can silently skip a widened tuple — and the symptom
/// reads as fusion producing a bad program rather than as a missing mint.
#[test]
fn a_fused_program_takes_a_fresh_id() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 20, 10);
    let b = spawn_tamed(&mut game, 10, 6);
    let parents = [
        game.world.get::<ProgramId>(a).unwrap().0,
        game.world.get::<ProgramId>(b).unwrap().0,
    ];
    let before: Vec<Entity> = owned_programs(&mut game);

    game.fuse_companions(a, b, None).unwrap();

    let fused = *owned_programs(&mut game)
        .iter()
        .find(|e| !before.contains(e))
        .expect("fusion leaves a program behind");
    let id = game.world.get::<ProgramId>(fused).map(|p| p.0);
    assert!(matches!(id, Some(n) if n != 0), "the child: {id:?}");
    assert!(
        !parents.contains(&id.unwrap()),
        "the child inherits neither parent's identity"
    );
}

/// Minting is pinned to the roster barrier, not to creature spawning
/// generally: nothing wild or hostile passes through `roster_parts`.
#[test]
fn a_wild_program_carries_no_id() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 6, 6).unwrap();

    assert!(game.world.get::<ProgramId>(wild).is_none());
}

/// Every live program the player owns. Fusion despawns both parents and
/// spawns the child, so "which one is new" is a set difference.
fn owned_programs(game: &mut Game) -> Vec<Entity> {
    game.world
        .query_filtered::<Entity, With<crate::components::Tamed>>()
        .iter(&game.world)
        .collect()
}
