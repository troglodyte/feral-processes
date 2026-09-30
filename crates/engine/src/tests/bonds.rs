//! Bands, avoidance, witnessing and departures: what one program's memories
//! of another do to the base.

use super::support::*;
use crate::bonds::Bond;
use crate::components::{Memories, Memory, MemorySubject, ProgramId};
use crate::memories::MemoryId;
use crate::*;

/// Writes a memory straight into the store, bypassing `remember` — the
/// catalogue and the strike cap are not what these tests are about.
fn implant(game: &mut Game, who: Entity, def: &str, subject: MemorySubject) {
    let now = game.current_tick();
    game.world
        .get_mut::<Memories>(who)
        .expect("an owned program holds a store")
        .0
        .push(Memory {
            def: MemoryId::from(def),
            subject,
            subject_name: None,
            reinforced: now,
            strikes: 1,
        });
}

fn id_of(game: &Game, who: Entity) -> ProgramId {
    *game.world.get::<ProgramId>(who).unwrap()
}

#[test]
fn bond_reads_the_holders_opinion_of_the_subject() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    let about = id_of(&game, other);
    assert_eq!(
        game.bond(holder, about),
        Bond::Neutral,
        "no memories, no bond"
    );

    implant(
        &mut game,
        holder,
        "turned_on_me",
        MemorySubject::Program(about),
    );
    implant(
        &mut game,
        holder,
        "turned_on_me",
        MemorySubject::Program(about),
    );
    let opinion = game.opinion_of(holder, &MemorySubject::Program(about));
    assert_eq!(game.bond(holder, about), crate::bonds::band(opinion));
    assert!(game.bond(holder, about).avoids(), "{opinion}");
}
