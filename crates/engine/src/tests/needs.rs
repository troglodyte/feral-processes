//! What a program needs: the reserves, the drain, and what survives a save.
//!
//! Seeding lives in exactly one place — `needs_drain_system` — so a fresh
//! program, one that predates a new def and a save written before the feature
//! all come up full through the same code path.

use super::support::*;
use crate::components::{Needs, Task, TaskKind};
use crate::needs::{NEED_MAX, NEED_MIN, NeedDb, NeedId};
use crate::*;

fn coherence() -> NeedId {
    NeedId::from("coherence")
}

fn reserve(game: &Game, who: Entity) -> f32 {
    game.world
        .get::<Needs>(who)
        .expect("a roster program carries a store")
        .get(&coherence())
        .expect("seeded on the first drain")
}

fn drain_rate(game: &Game) -> (f32, f32) {
    let db = game.world.resource::<NeedDb>();
    let def = db.get(&coherence()).expect("shipped");
    (def.drain_per_tick, def.working_multiplier)
}

/// A staff program's reserve falls on its own, at the authored rate.
#[test]
fn a_staff_programs_reserve_falls_by_the_authored_rate() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let (rate, _) = drain_rate(&game);

    game.tick();

    assert!(
        (reserve(&game, worker) - (NEED_MAX - rate)).abs() < 1e-4,
        "one tick off a full reserve, got {}",
        reserve(&game, worker)
    );
}

/// Working costs more than idling, and `working_multiplier` is the knob.
#[test]
fn a_working_program_drains_strictly_faster() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let idle = spawn_tamed(&mut game, 10, 3);
    let busy = spawn_tamed(&mut game, 10, 3);
    let (rate, multiplier) = drain_rate(&game);
    game.world.entity_mut(busy).insert(Task {
        target: idle,
        kind: TaskKind::GatherResource,
        progress: 0,
        required: 100,
    });

    game.tick();

    assert!(
        reserve(&game, busy) < reserve(&game, idle),
        "working must cost more: {} vs {}",
        reserve(&game, busy),
        reserve(&game, idle)
    );
    assert!(
        (reserve(&game, busy) - (NEED_MAX - rate * multiplier)).abs() < 1e-4,
        "and by exactly the authored multiplier, got {}",
        reserve(&game, busy)
    );
}

/// A program spawns with an **empty** store, not a full one — seeding is the
/// drain's first act, so a def added between sessions comes up full too.
#[test]
fn a_fresh_store_is_seeded_full_before_the_first_drain_subtracts() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    assert_eq!(
        game.world.get::<Needs>(worker).unwrap().iter().count(),
        0,
        "roster_parts mints it empty; seeding is the drain's job"
    );
    let (rate, _) = drain_rate(&game);

    game.tick();

    assert!(
        reserve(&game, worker) > NEED_MAX - rate * 2.0,
        "seeded full and then drained once, never seeded at zero"
    );
}

/// Deleting `assets/needs/` is a supported way to play: nothing is seeded and
/// nothing drains, without a branch anywhere.
#[test]
fn an_empty_catalogue_seeds_nothing_and_drains_nothing() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(NeedDb::default());
    let worker = spawn_tamed(&mut game, 10, 3);

    game.tick();

    assert_eq!(
        game.world.get::<Needs>(worker).unwrap().iter().count(),
        0,
        "no defs, no reserves"
    );
}

/// The clamp is the type's, exactly as `PowerReserve`'s is — no caller clamps.
#[test]
fn setting_a_reserve_clamps_at_both_ends() {
    let mut needs = Needs::default();
    needs.set(&coherence(), NEED_MAX + 500.0);
    assert_eq!(needs.get(&coherence()), Some(NEED_MAX));
    needs.set(&coherence(), NEED_MIN - 500.0);
    assert_eq!(needs.get(&coherence()), Some(NEED_MIN));
}

/// Needs are a base-labour concept in v1: a program fighting beside you is
/// not on shift and does not drain.
#[test]
fn a_party_member_does_not_drain() {
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let member = spawn_tamed(&mut game, 10, 3);
    game.world.resource_mut::<crate::resources::Party>().0 = vec![member];

    game.tick();

    assert_eq!(
        game.world.get::<Needs>(member).unwrap().iter().count(),
        0,
        "a party member is not staff, so the drain never even seeds it"
    );
}

/// A field-named RON round trip cannot catch a skipped field, so the reserves
/// are asserted through a **real** save and load as well.
#[test]
fn reserves_survive_a_save_and_load() {
    let dir = scratch_assets_dir("needs_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world
        .get_mut::<Needs>(worker)
        .unwrap()
        .set(&coherence(), 37.5);
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored: Vec<f32> = loaded
        .world
        .query::<&Needs>()
        .iter(&loaded.world)
        .filter_map(|n| n.get(&coherence()))
        .filter(|v| (*v - 37.5).abs() < 1e-4)
        .collect();
    assert_eq!(restored.len(), 1, "the reserve survives the round trip");
}

/// A save written before needs existed carries no key at all, and the program
/// in it must come up full rather than empty or at zero.
#[test]
fn a_save_written_before_needs_loads_and_seeds_full() {
    let dir = scratch_assets_dir("needs_legacy_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(47, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world
        .get_mut::<Needs>(worker)
        .unwrap()
        .set(&coherence(), 12.0);
    game.save(&path).unwrap();
    // The pre-needs file: every creature's key stripped, which is what
    // `#[serde(default)]` has to answer for.
    let mut data = crate::save::load_from_file(&path).unwrap();
    for creature in &mut data.creatures {
        creature.needs.clear();
    }
    crate::save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    loaded.tick();
    let (rate, _) = drain_rate(&loaded);
    let seeded: Vec<f32> = loaded
        .world
        .query::<&Needs>()
        .iter(&loaded.world)
        .filter_map(|n| n.get(&coherence()))
        .collect();
    assert!(
        seeded.iter().all(|v| *v > NEED_MAX - rate * 2.0),
        "an absent key seeds full, never empty and never zero: {seeded:?}"
    );
    let _ = worker;
}
