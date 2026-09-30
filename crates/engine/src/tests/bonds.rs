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

// ---------------------------------------------------------------------
// Avoidance
// ---------------------------------------------------------------------

/// A worker and one neighbour, the worker on a beat that offers it a real
/// step. The neighbour stands on the far side of that offered tile, so the
/// tile is 8-adjacent to it and the worker is not (yet).
///
/// Standing well inside the starting pocket, `memories.rs`'s
/// `a_base_with_idle_staff` reason: the bond must be the only thing that can
/// refuse the candidate.
fn a_worker_beside_a_neighbour(game: &mut Game) -> (Vec<Entity>, Position) {
    place_home(game);
    let worker = spawn_tamed(game, 10, 3);
    let other = spawn_tamed(game, 10, 3);
    let mut staff = vec![worker, other];
    staff.sort();
    let (worker, other) = (staff[0], staff[1]);
    {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = 3;
        pos.y = 0;
    }
    let here = *game.world.get::<Position>(worker).unwrap();
    let mut tick = game.current_tick();
    let candidate = loop {
        let step = tick / crate::tuning::IDLE_STAFF_STEP_TICKS;
        if let Some(t) = crate::game::base::work_orders::wander_step(here, 0, step) {
            game.world
                .resource_mut::<crate::resources::GameClock>()
                .tick = tick;
            break t;
        }
        tick += 1;
    };
    let mut pos = game.world.get_mut::<Position>(other).unwrap();
    pos.x = candidate.x * 2 - here.x;
    pos.y = candidate.y * 2 - here.y;
    (staff, candidate)
}

fn drift(game: &mut Game, staff: &[Entity]) {
    let amenities = game.amenities();
    let bays = game.repair_bays();
    game.drift_idle_staff_for_test(staff, &amenities, &bays);
}

fn position(game: &Game, who: Entity) -> (i32, i32) {
    let p = game.world.get::<Position>(who).unwrap();
    (p.x, p.y)
}

#[test]
fn a_worker_will_not_step_beside_a_rival() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, candidate) = a_worker_beside_a_neighbour(&mut game);
    let rival = id_of(&game, staff[1]);
    implant(
        &mut game,
        staff[0],
        "turned_on_me",
        MemorySubject::Program(rival),
    );

    let before = position(&game, staff[0]);
    drift(&mut game, &staff);

    assert_eq!(
        position(&game, staff[0]),
        before,
        "declined, so it stays put"
    );
    assert_ne!(position(&game, staff[0]), (candidate.x, candidate.y));
}

#[test]
fn a_worker_steps_beside_a_friend() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, candidate) = a_worker_beside_a_neighbour(&mut game);
    let friend = id_of(&game, staff[1]);
    implant(
        &mut game,
        staff[0],
        "bonded_in_battle",
        MemorySubject::Program(friend),
    );
    implant(
        &mut game,
        staff[0],
        "bonded_in_battle",
        MemorySubject::Program(friend),
    );
    assert!(!game.bond(staff[0], friend).avoids());

    drift(&mut game, &staff);

    assert_eq!(position(&game, staff[0]), (candidate.x, candidate.y));
}

#[test]
fn a_worker_steps_beside_a_stranger() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, candidate) = a_worker_beside_a_neighbour(&mut game);

    drift(&mut game, &staff);

    assert_eq!(
        position(&game, staff[0]),
        (candidate.x, candidate.y),
        "control: with no bond the same fixture takes the same tile"
    );
}
