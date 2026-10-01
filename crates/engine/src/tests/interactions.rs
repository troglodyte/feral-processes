//! `Game::note_interactions`: when two idle programs talk, and what a rumour
//! may be about. The pure pieces (`pair_idle`, `pick`) are tested beside
//! their definitions; this file is the pass that reads a real base.
//!
//! No test-only chance override: each test searches, over the same pure
//! `derive::unit` fold the pass uses, for a tick whose roll passes (or
//! fails), sets the clock there and runs the pass.

use super::support::*;
use crate::components::{
    Conversations, Memories, Memory, MemorySubject, Position, ProgramId, Task, TaskKind,
};
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

    // Second pass: the listener is now the speaker, to a bystander who is
    // neither the subject nor the original speaker.
    game.world.get_mut::<Position>(b[0]).unwrap().x = 900;
    game.world.get_mut::<Position>(b[3]).unwrap().x = 2;
    let (s2, l2) = (id_of(&game, b[1]), id_of(&game, b[3]));
    let now = game.world.resource::<GameClock>().tick;
    set_tick(&mut game, period_tick(s2, l2, true, now));

    game.note_interactions();

    assert_eq!(held(&game, b[3]).len(), 0, "a rumour does not travel twice");
}

// ---------------------------------------------------------------------------
// Conversations
// ---------------------------------------------------------------------------

const TALK_SAYS: &str = "(id: \"talk\", name: \"n\", listener_memory: \"chatted_with\", \
    weight: 1.0, exchanges: [[(by: Speaker, text: \"Hello {listener}.\"), \
    (by: Listener, text: \"Hello {speaker}.\")], \
    [(by: Speaker, text: \"About {topic}.\"), (by: Listener, text: \"Yes.\")]])";

fn records(game: &Game, e: Entity) -> Vec<crate::interactions::ConversationRecord> {
    game.world
        .get::<Conversations>(e)
        .map(|c| c.0.iter().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn a_fired_interaction_leaves_a_mirrored_record_on_each_side() {
    use crate::interactions::Role;
    let (mut game, b) = established("rec_pair", &[TALK_SAYS]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    let tick = period_tick(s, l, true, 0);
    set_tick(&mut game, tick);

    game.note_interactions();

    let (said, heard) = (records(&game, b[0]), records(&game, b[1]));
    assert_eq!((said.len(), heard.len()), (1, 1), "{said:?} {heard:?}");
    assert_eq!((said[0].role, said[0].other), (Role::Speaker, l));
    assert_eq!((heard[0].role, heard[0].other), (Role::Listener, s));
    assert_eq!(said[0].tick, tick);
    assert_eq!(said[0].interaction, "talk");
    assert_eq!(said[0].exchange, heard[0].exchange);
    assert!(said[0].exchange.is_some());
    assert_eq!(said[0].topic, heard[0].topic);
    assert_eq!(said[0].other_name, game.creature_short_label(b[1]));
    assert_eq!(heard[0].other_name, game.creature_short_label(b[0]));
    assert_eq!(records(&game, b[2]).len(), 0);
}

#[test]
fn nothing_fires_so_no_record_is_written() {
    let (mut game, b) = established("rec_none", &[TALK_SAYS]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, false, 0));
    game.note_interactions();
    assert!(
        b.iter()
            .all(|&e| game.world.get::<Conversations>(e).is_none())
    );
}

/// Runs the pass on `n` successive ticks whose roll passes for this pair, so
/// a test of what a seed decides sees several seeds rather than one lucky one.
fn pass_over_ticks(game: &mut Game, s: ProgramId, l: ProgramId, n: usize) {
    let mut t = 0;
    for _ in 0..n {
        t = period_tick(s, l, true, t);
        set_tick(game, t);
        game.note_interactions();
    }
}

#[test]
fn a_gossip_conversation_is_about_the_subject_it_told() {
    let (mut game, b, s, l) = gossip_game("rec_gossip");
    let (c, d) = (id_of(&game, b[2]), id_of(&game, b[3]));
    told(&mut game, b[0], "idled_with", d);
    told(&mut game, b[0], "turned_on_me", c);
    let label = game.creature_short_label(b[2]);

    pass_over_ticks(&mut game, s, l, 12);

    for e in [b[0], b[1]] {
        let r = records(&game, e);
        assert_eq!(r.len(), 12);
        for rec in r {
            assert_eq!(rec.topic, Some((MemorySubject::Program(c), label.clone())));
        }
    }
}

#[test]
fn a_plain_conversation_is_about_something_the_speaker_holds() {
    let (mut game, b) = established("rec_topic", &[TALK_SAYS]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    let c = id_of(&game, b[2]);
    told(&mut game, b[0], "heard_ill_of", c);
    set_tick(&mut game, period_tick(s, l, true, 0));

    game.note_interactions();

    let r = records(&game, b[0]);
    assert_eq!(
        r[0].topic.as_ref().map(|t| &t.0),
        Some(&MemorySubject::Program(c))
    );
}

#[test]
fn with_nothing_on_its_mind_only_a_topic_free_exchange_is_said() {
    let (mut game, b) = established("rec_no_topic", &[TALK_SAYS]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    pass_over_ticks(&mut game, s, l, 12);
    let r = records(&game, b[0]);
    assert_eq!(r.len(), 12);
    for rec in r {
        assert_eq!((rec.topic, rec.exchange), (None, Some(0)));
    }
}

#[test]
fn the_same_state_writes_the_same_records() {
    let run = |tag: &str| {
        let (mut game, b) = established(tag, &[TALK_SAYS]);
        let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
        let c = id_of(&game, b[2]);
        told(&mut game, b[0], "heard_ill_of", c);
        pass_over_ticks(&mut game, s, l, 12);
        b.iter().map(|&e| records(&game, e)).collect::<Vec<_>>()
    };
    let first = run("rec_det_0");
    assert!(first.iter().any(|v| !v.is_empty()));
    for i in 1..4 {
        assert_eq!(first, run(&format!("rec_det_{i}")));
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn record(
    interaction: &str,
    exchange: Option<u8>,
    role: crate::interactions::Role,
    other: ProgramId,
    topic: Option<(MemorySubject, String)>,
) -> crate::interactions::ConversationRecord {
    crate::interactions::ConversationRecord {
        tick: 1,
        interaction: interaction.into(),
        exchange,
        role,
        other,
        other_name: "Stamped".into(),
        topic,
    }
}

fn give(game: &mut Game, e: Entity, r: crate::interactions::ConversationRecord) {
    if game.world.get::<Conversations>(e).is_none() {
        game.world.entity_mut(e).insert(Conversations::default());
    }
    game.world.get_mut::<Conversations>(e).unwrap().push(r);
}

#[test]
fn slots_fill_and_roles_name_the_right_side() {
    use crate::interactions::Role;
    let (mut game, b) = established("view_fill", &[TALK_SAYS]);
    let (me, other) = (
        game.creature_short_label(b[0]),
        game.creature_short_label(b[1]),
    );
    let (o, c) = (id_of(&game, b[1]), id_of(&game, b[2]));
    let topic = Some((MemorySubject::Program(c), "Old".to_string()));
    let c_name = game.creature_short_label(b[2]);
    give(
        &mut game,
        b[0],
        record("talk", Some(0), Role::Speaker, o, None),
    );
    give(
        &mut game,
        b[0],
        record("talk", Some(1), Role::Listener, o, topic),
    );

    let views = game.conversations(b[0]).unwrap();

    assert_eq!(views.len(), 2);
    // Newest first: the listener-side record with a topic.
    assert_eq!(views[0].lines[0].who, other);
    assert_eq!(views[0].lines[0].text, format!("About {c_name}."));
    assert_eq!(views[0].lines[1].who, me);
    assert_eq!(views[1].lines[0].who, me);
    assert_eq!(views[1].lines[0].text, format!("Hello {other}."));
    assert_eq!(views[1].lines[1].text, format!("Hello {me}."));
}

#[test]
fn a_non_program_topic_is_named_from_the_catalogue_even_when_stamped_empty() {
    use crate::interactions::Role;
    let (mut game, b) = established("view_topic_kinds", &[TALK_SAYS]);
    let o = id_of(&game, b[1]);
    for (subject, want) in [
        (
            MemorySubject::Structure("annealing_node".into()),
            "About Annealing Node.",
        ),
        (MemorySubject::Species("cipher".into()), "About Cipher."),
        (
            MemorySubject::BaseTile { x: 3, y: 4 },
            "About the base at (3, 4).",
        ),
    ] {
        give(
            &mut game,
            b[0],
            record(
                "talk",
                Some(1),
                Role::Listener,
                o,
                Some((subject, String::new())),
            ),
        );
        let views = game.conversations(b[0]).unwrap();
        assert_eq!(views[0].lines[0].text, want);
    }
}

#[test]
fn a_rename_reads_through_and_a_departed_program_uses_its_stamp() {
    use crate::interactions::Role;
    let (mut game, b) = base(
        7,
        BASE_ESTABLISHED_STAFF + 1,
        BASE_ESTABLISHED_STRUCTURES,
        "view_names",
        &[TALK_SAYS],
    );
    let o = id_of(&game, b[1]);
    give(
        &mut game,
        b[0],
        record("talk", Some(0), Role::Speaker, o, None),
    );
    game.rename_companion(b[1], Some("Zed".into())).unwrap();
    let live = game.conversations(b[0]).unwrap();
    assert_eq!(live[0].lines[0].text, "Hello Zed.");

    game.world.despawn(b[1]);
    let gone = game.conversations(b[0]).unwrap();
    assert_eq!(gone[0].lines[0].text, "Hello Stamped.");
}

#[test]
fn a_missing_def_or_bad_index_falls_back_without_panicking() {
    use crate::interactions::Role;
    let (mut game, b) = established("view_fallback", &[TALK_SAYS]);
    let o = id_of(&game, b[1]);
    let (me, other) = (
        game.creature_short_label(b[0]),
        game.creature_short_label(b[1]),
    );
    give(
        &mut game,
        b[0],
        record("gone", Some(0), Role::Speaker, o, None),
    );
    give(
        &mut game,
        b[0],
        record("talk", Some(9), Role::Speaker, o, None),
    );
    give(
        &mut game,
        b[0],
        record("talk", None, Role::Listener, o, None),
    );

    let v = game.conversations(b[0]).unwrap();

    assert_eq!(v[0].lines.len(), 1);
    assert_eq!(v[0].lines[0].text, format!("{other} and {me}: n"));
    assert_eq!(v[1].lines[0].text, format!("{me} and {other}: n"));
    assert_eq!(v[2].lines[0].text, format!("{me} and {other}: gone"));
}

#[test]
fn an_unowned_program_has_no_conversations_and_a_quiet_one_has_none_listed() {
    let (game, b) = established("view_owned", &[TALK_SAYS]);
    assert_eq!(game.conversations(b[0]), Some(Vec::new()));
    assert_eq!(game.conversations(game.player_entity()), None);
}

#[test]
fn one_cue_per_fired_interaction_at_the_speakers_cell_then_drained() {
    let (mut game, b) = established("speech_cue", &[TALK_SAYS]);
    let (s, l) = (id_of(&game, b[0]), id_of(&game, b[1]));
    set_tick(&mut game, period_tick(s, l, false, 0));
    game.note_interactions();
    assert!(game.take_speech().is_empty(), "a failed roll says nothing");

    set_tick(&mut game, period_tick(s, l, true, 0));
    game.note_interactions();

    let speaker = *game.world.get::<Position>(b[0]).unwrap();
    assert_ne!(speaker, *game.world.get::<Position>(b[1]).unwrap());
    assert_eq!(
        game.take_speech(),
        vec![crate::resources::SpeechCue {
            cell: (speaker.x, speaker.y)
        }]
    );
    assert!(game.take_speech().is_empty(), "taking drains");
}
