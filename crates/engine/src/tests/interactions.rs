//! `Game::note_interactions`: when two idle programs talk, and what a rumour
//! may be about. The pure pieces (`pair_idle`, `pick`) are tested beside
//! their definitions; this file is the pass that reads a real base.
//!
//! No test-only chance override: each test searches, over the same pure
//! `derive::unit` fold the pass uses, for a tick whose roll passes (or
//! fails), sets the clock there and runs the pass.

use super::support::*;
use crate::components::{Memories, Memory, MemorySubject, Position, ProgramId, Task, TaskKind};
use crate::derive::{FNV_BASIS, fold, unit};
use crate::interactions::InteractionDb;
use crate::memories::{MemoryDb, MemoryId};
use crate::resources::GameClock;
use crate::sociability::Sociability;
use crate::tuning::{
    BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES, INTERACTION_CHANCE, INTERACTION_PERIOD,
    INTERACTION_SALT,
};
use crate::*;

fn memory_text(id: &str, valence: f32, spreads_as: Option<&str>) -> String {
    let spreads = spreads_as
        .map(|t| format!(", spreads_as: Some(\"{t}\")"))
        .unwrap_or_default();
    format!(
        "(id: \"{id}\", name: \"n\", blurb: \"b\", valence: {valence:?}, half_life: 100000, \
         subject: Program, strike_cap: 3{spreads})"
    )
}

/// A hand-built catalogue, so these tests keep testing the mechanism when
/// the shipped content is retuned.
fn install_memories(game: &mut Game, tag: &str) {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    for (id, valence, spreads) in [
        ("chatted_with", 1.5, None),
        ("heard_ill_of", -2.0, None),
        ("heard_well_of", 1.5, None),
        ("turned_on_me", -4.0, Some("heard_ill_of")),
        ("idled_with", 1.5, Some("heard_well_of")),
    ] {
        std::fs::write(
            dir.join(format!("{id}.ron")),
            memory_text(id, valence, spreads),
        )
        .unwrap();
    }
    let (db, warnings) = MemoryDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    game.world.insert_resource(db);
}

const TALK: &str = "(id: \"talk\", name: \"n\", listener_memory: \"chatted_with\", \
                    speaker_memory: Some(\"chatted_with\"), weight: 1.0)";
const GOSSIP: &str = "(id: \"gossip\", name: \"n\", listener_memory: \"x\", weight: 1.0, \
                      gossip: true)";

fn install_interactions(game: &mut Game, tag: &str, defs: &[&str]) {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    for (i, def) in defs.iter().enumerate() {
        std::fs::write(dir.join(format!("{i}.ron")), def).unwrap();
    }
    let (db, warnings) = InteractionDb::load_dir(&dir, game.world.resource::<MemoryDb>()).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    game.world.insert_resource(db);
}

/// A base of `staff` programs under `structures` buildings. Staff 0 and 1
/// stand together at the origin; every other body is a world away from
/// everyone, so exactly one pair can form unless a test moves someone.
fn base(
    seed: u32,
    staff: usize,
    structures: usize,
    tag: &str,
    defs: &[&str],
) -> (Game, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    if structures > 0 {
        place_home(&mut game);
    }
    let depot = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get(&crate::structures::StructureId::from("depot"))
        .expect("a Depot ships")
        .clone();
    for i in 1..structures {
        game.spawn_structure(&depot, -20 - i as i32, 20, None);
    }
    let bodies: Vec<Entity> = (0..staff).map(|_| spawn_tamed(&mut game, 10, 3)).collect();
    for (i, &b) in bodies.iter().enumerate() {
        let mut p = game.world.get_mut::<Position>(b).unwrap();
        p.x = if i < 2 { i as i32 } else { 40 * i as i32 };
        p.y = 0;
    }
    install_memories(&mut game, &format!("{tag}_memories"));
    install_interactions(&mut game, tag, defs);
    (game, bodies)
}

fn established(tag: &str, defs: &[&str]) -> (Game, Vec<Entity>) {
    base(
        7,
        BASE_ESTABLISHED_STAFF,
        BASE_ESTABLISHED_STRUCTURES,
        tag,
        defs,
    )
}

fn id_of(game: &Game, e: Entity) -> ProgramId {
    *game.world.get::<ProgramId>(e).unwrap()
}

fn held(game: &Game, e: Entity) -> Vec<Memory> {
    game.world.get::<Memories>(e).unwrap().0.clone()
}

fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

/// The first on-period tick after `after` whose roll for this pair passes
/// (or fails), from the same fold the pass makes.
fn period_tick(speaker: ProgramId, listener: ProgramId, passes: bool, after: u64) -> u64 {
    let chance = INTERACTION_CHANCE * Sociability::of(speaker).chance_mult();
    (after / INTERACTION_PERIOD + 1..)
        .map(|n| n * INTERACTION_PERIOD)
        .find(|&t| {
            let seed = fold(
                FNV_BASIS,
                &[t, speaker.0 as u64, listener.0 as u64, INTERACTION_SALT],
            );
            (unit(seed) < chance) == passes
        })
        .unwrap()
}

fn total_memories(game: &Game, bodies: &[Entity]) -> usize {
    bodies.iter().map(|&b| held(game, b).len()).sum()
}

#[test]
fn an_idle_pair_on_a_passing_tick_talk_both_ways() {
    let (mut game, b) = established("talk_both", &[TALK]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));

    game.note_interactions();

    let heard = held(&game, b[1]);
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].def, MemoryId::from("chatted_with"));
    assert_eq!(heard[0].subject, MemorySubject::Program(s));
    let said = held(&game, b[0]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].subject, MemorySubject::Program(l));
    assert_eq!(total_memories(&game, &b), 2);
}

#[test]
fn a_failing_roll_writes_nothing() {
    let (mut game, b) = established("roll_fails", &[TALK]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, false, 0));
    game.note_interactions();
    assert_eq!(total_memories(&game, &b), 0);
}

#[test]
fn off_the_period_nothing_is_written() {
    let (mut game, b) = established("off_period", &[TALK]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0) + 1);
    game.note_interactions();
    assert_eq!(total_memories(&game, &b), 0);
}

#[test]
fn before_the_base_is_established_nothing_is_written() {
    let (mut game, b) = base(
        7,
        BASE_ESTABLISHED_STAFF - 1,
        BASE_ESTABLISHED_STRUCTURES,
        "young_staff",
        &[TALK],
    );
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    game.note_interactions();
    assert_eq!(total_memories(&game, &b), 0, "too few staff");

    let (mut game, b) = base(
        7,
        BASE_ESTABLISHED_STAFF,
        BASE_ESTABLISHED_STRUCTURES - 1,
        "young_structures",
        &[TALK],
    );
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    game.note_interactions();
    assert_eq!(total_memories(&game, &b), 0, "too few structures");
}

#[test]
fn a_lone_idle_program_says_nothing() {
    let (mut game, b) = established("lone", &[TALK]);
    game.world.get_mut::<Position>(b[1]).unwrap().x = 500;
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    game.note_interactions();
    assert_eq!(total_memories(&game, &b), 0);
}

#[test]
fn a_program_with_a_task_is_never_paired() {
    let (mut game, b) = established("busy", &[TALK]);
    game.world.entity_mut(b[1]).insert(Task {
        kind: TaskKind::Guard,
        target: b[0],
        progress: 0,
        required: 10,
    });
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    game.note_interactions();
    assert_eq!(total_memories(&game, &b), 0);
}

#[test]
fn a_program_takes_part_in_one_interaction_per_pass() {
    let (mut game, b) = established("one_each", &[TALK]);
    game.world.get_mut::<Position>(b[2]).unwrap().x = 1;
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));

    game.note_interactions();

    assert_eq!(held(&game, b[2]).len(), 0, "the third wheel is left out");
    assert_eq!(total_memories(&game, &b), 2);
}

#[test]
fn the_same_state_writes_the_same_memories() {
    let run = |tag: &str| {
        let (mut game, b) = established(tag, &[TALK]);
        let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
        set_tick(&mut game, period_tick(s, l, true, 0));
        game.note_interactions();
        b.iter()
            .map(|&e| {
                held(&game, e)
                    .into_iter()
                    .map(|m| (m.def, m.subject, m.reinforced, m.strikes))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let (a, b) = (run("det_a"), run("det_b"));
    assert!(a.iter().any(|v| !v.is_empty()), "the run wrote something");
    assert_eq!(a, b);
}

#[test]
fn the_pass_is_wired_into_the_tick() {
    let (mut game, b) = established("wired", &[TALK]);
    // The same tile, so one idle step apiece cannot part them.
    game.world.get_mut::<Position>(b[1]).unwrap().x = 0;
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    game.tick_inner(false);
    assert_eq!(held(&game, b[1]).len(), 1);
}

// ---------------------------------------------------------------------------
// Gossip
// ---------------------------------------------------------------------------

fn gossip_game(tag: &str) -> (Game, Vec<Entity>, ProgramId, ProgramId) {
    let (mut game, b) = established(tag, &[GOSSIP]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    (game, b, s, l)
}

fn told(game: &mut Game, to: Entity, def: &str, about: ProgramId) {
    game.remember(to, def, MemorySubject::Program(about));
}

#[test]
fn a_rumour_is_never_told_to_its_own_subject() {
    let (mut game, b, _, l) = gossip_game("not_to_subject");
    told(&mut game, b[0], "turned_on_me", l);
    game.note_interactions();
    assert_eq!(held(&game, b[1]).len(), 0);
}

#[test]
fn a_rumour_is_never_about_the_speaker() {
    let (mut game, b, s, _) = gossip_game("not_about_speaker");
    game.world
        .get_mut::<Memories>(b[0])
        .unwrap()
        .0
        .push(Memory {
            def: MemoryId::from("turned_on_me"),
            subject: MemorySubject::Program(s),
            subject_name: None,
            reinforced: 0,
            strikes: 1,
        });
    game.note_interactions();
    assert_eq!(held(&game, b[1]).len(), 0);
}

#[test]
fn the_strongest_tellable_memory_is_the_one_told() {
    let (mut game, b, _, _) = gossip_game("strongest");
    let (c, d) = (id_of(&game, b[2]), id_of(&game, b[3]));
    // Inserted weakest first: the winner is not the first one found.
    told(&mut game, b[0], "idled_with", d);
    told(&mut game, b[0], "turned_on_me", c);

    game.note_interactions();

    let heard = held(&game, b[1]);
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].def, MemoryId::from("heard_ill_of"));
    assert_eq!(heard[0].subject, MemorySubject::Program(c));
}

#[test]
fn equally_strong_memories_tie_to_the_lower_subject_id() {
    let (mut game, b, _, _) = gossip_game("tie");
    let (c, d) = (id_of(&game, b[2]), id_of(&game, b[3]));
    told(&mut game, b[0], "idled_with", d);
    told(&mut game, b[0], "idled_with", c);

    game.note_interactions();

    let heard = held(&game, b[1]);
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].subject, MemorySubject::Program(c));
}

#[test]
fn a_departed_subject_can_still_be_gossiped_about_by_its_stamped_name() {
    // One body over the threshold, so the base stays established without it.
    let (mut game, b) = base(
        7,
        BASE_ESTABLISHED_STAFF + 1,
        BASE_ESTABLISHED_STRUCTURES,
        "departed",
        &[GOSSIP],
    );
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, true, 0));
    let c = id_of(&game, b[2]);
    told(&mut game, b[0], "turned_on_me", c);
    let stamped = held(&game, b[0])[0].subject_name.clone();
    assert!(stamped.is_some());
    game.world.despawn(b[2]);

    game.note_interactions();

    let heard = held(&game, b[1]);
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].subject, MemorySubject::Program(c));
    assert_eq!(heard[0].subject_name, stamped);
}

#[test]
fn hearsay_is_not_retold() {
    let (mut game, b, _, _) = gossip_game("one_hop");
    let c = id_of(&game, b[2]);
    told(&mut game, b[0], "turned_on_me", c);
    game.note_interactions();
    assert_eq!(held(&game, b[1]).len(), 1, "the first hop landed");

    // Second pass: the listener is now the speaker, to a fresh listener.
    game.world.get_mut::<Position>(b[0]).unwrap().x = 900;
    game.world.get_mut::<Position>(b[2]).unwrap().x = 1;
    let (s2, l2) = (id_of(&game, b[1]), id_of(&game, b[2]));
    let now = game.world.resource::<GameClock>().tick;
    set_tick(&mut game, period_tick(s2, l2, true, now));

    game.note_interactions();

    assert_eq!(held(&game, b[2]).len(), 0, "a rumour does not travel twice");
}
