//! Situational thoughts reaching morale: the one derivation, the isolation
//! from opinion, the cap against the ladder, and the empty catalogue.
//! `situations.rs` holds the per-trigger tests.

use super::support::*;
use crate::bonds::Bond;
use crate::components::{
    Disgruntled, Grievance, Memories, Memory, MemorySubject, Position, ProgramId,
};
use crate::disposition::Disposition;
use crate::memories::MemoryId;
use crate::situations::{Situation, ThoughtDb, Trigger};
use crate::tuning::{MORALE_SULKS_AT, SITUATION_MAX_TOTAL};
use crate::*;

/// A temp path unique to the calling test, `alerts.rs`'s reason.
fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_situations_{tag}_{}.bin",
        std::process::id()
    ))
}

fn thought_file(trigger: &str, intensity: f32) -> String {
    format!(
        "(\n    trigger: {trigger},\n    name: \"{trigger}\",\n    blurb: \"b\",\n    \
         intensity: {intensity:?},\n)\n"
    )
}

/// A scratch install whose `assets/thoughts/` holds exactly `files`.
fn assets_with_thoughts(tag: &str, files: &[(&str, String)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    let thoughts = dir.join("thoughts");
    std::fs::create_dir_all(&thoughts).unwrap();
    for (name, body) in files {
        std::fs::write(thoughts.join(name), body).unwrap();
    }
    dir
}

fn implant(game: &mut Game, who: Entity, def: &str, strikes: u32, subject: MemorySubject) {
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
            strikes,
        });
}

fn set_situation(game: &mut Game, who: Entity, thoughts: &[Trigger]) {
    game.world.entity_mut(who).insert(Situation {
        thoughts: thoughts.to_vec(),
    });
}

const EVERY_NEGATIVE: [Trigger; 3] = [Trigger::BesideRival, Trigger::Unpowered, Trigger::NoAmenity];

/// A Home and one idle staff program, on a base with no amenity.
fn a_base_with_one_staff(game: &mut Game) -> Entity {
    stand_in_base(game);
    place_home(game);
    let worker = spawn_tamed(game, 10, 3);
    let mut pos = game.world.get_mut::<Position>(worker).unwrap();
    pos.x = -2;
    pos.y = 0;
    worker
}

#[test]
fn morale_is_the_memory_fold_plus_the_situational_sum() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    implant(
        &mut game,
        worker,
        "saw_turn_on",
        1,
        MemorySubject::Program(ProgramId(999)),
    );
    let remembered = game.morale(worker);
    assert!(remembered < 0.0, "{remembered}");

    set_situation(
        &mut game,
        worker,
        &[Trigger::BesideFriend, Trigger::MachineRunning],
    );
    assert!((game.morale(worker) - (remembered + 3.0)).abs() < 1e-5);
}

#[test]
fn a_program_with_no_situation_folds_as_empty() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).remove::<Situation>();
    assert_eq!(game.morale(worker), 0.0);
}

/// The system's own fold, observed through what morale buys: a paired run of
/// the same seeds with and without a thought that weighs the cap. Same RNG
/// stream in both arms, and a lower success chance can only turn a hit into a
/// miss. Only the aggregate over the seeds is asserted: later state can
/// diverge between the arms, so no single seed is promised to be ordered.
#[test]
fn the_cronjob_folds_the_situation_too() {
    const SEEDS: std::ops::Range<u32> = 1..41;
    const TICKS: u32 = 180;
    let heavy = assets_with_thoughts(
        "situation_cronjob",
        &[(
            "no_amenity.ron",
            thought_file("NoAmenity", -SITUATION_MAX_TOTAL),
        )],
    );
    let mut with = 0;
    let mut without = 0;
    for seed in SEEDS {
        for (hits, silenced) in [(&mut with, false), (&mut without, true)] {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &heavy).unwrap();
            if silenced {
                game.world.insert_resource(ThoughtDb::default());
            }
            stand_in_base(&mut game);
            let node = deploy_upgradeable_node(&mut game);
            let worker = spawn_tamed(&mut game, 40, 4);
            stand_player_at_post(&mut game, node);
            game.assign_cronjob(worker, node)
                .expect("a node takes a posted program");
            park_at_post(&mut game, worker, node);
            for _ in 0..TICKS {
                game.tick();
            }
            *hits += node_output(&game, node, ids::CORE_FRAGMENT);
        }
    }
    assert!(
        with < without,
        "a program at the situational floor mined {with}, one with no thoughts {without}"
    );
}

#[test]
fn an_active_rival_beside_changes_no_opinion_and_no_bond() {
    let mut game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    let about = *game.world.get::<ProgramId>(other).unwrap();
    let subject = MemorySubject::Program(about);
    implant(&mut game, holder, "turned_on_me", 1, subject.clone());
    let opinion = game.opinion_of(holder, &subject);
    let bond = game.bond(holder, about);
    assert_eq!(bond, Bond::Rival);

    set_situation(&mut game, holder, &[Trigger::BesideRival]);
    assert_eq!(game.opinion_of(holder, &subject), opinion);
    assert_eq!(game.bond(holder, about), bond);
}

#[test]
fn no_memories_and_every_negative_thought_never_sulks() {
    let mut game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    // `Abrasive` feels every thought 1.4x heavier: -9.8 raw, so it is the
    // clamp and not the catalogue's modesty that keeps this out of Sulking.
    game.world.entity_mut(worker).insert(Disposition::Abrasive);
    set_situation(&mut game, worker, &EVERY_NEGATIVE);
    assert!(
        game.morale(worker) > MORALE_SULKS_AT,
        "{}",
        game.morale(worker)
    );
    game.update_disgruntled(&[worker]);
    assert!(game.world.get::<Disgruntled>(worker).is_none());
}

/// The spec's case: -4 from memories, then every reachable negative thought
/// (-7 raw, clamped to -5) for -9. Neutral -4 with a rival alone is -7 and
/// does not sulk, so the whole set is what tips it.
#[test]
fn the_situation_tips_a_program_already_soured_by_memories() {
    let mut game = Game::new(66, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    implant(&mut game, worker, "jammed_here", 1, MemorySubject::Nothing);
    assert!(
        (game.morale(worker) + 4.0).abs() < 1e-3,
        "{}",
        game.morale(worker)
    );
    game.update_disgruntled(&[worker]);
    assert!(game.world.get::<Disgruntled>(worker).is_none());

    set_situation(&mut game, worker, &[Trigger::BesideRival]);
    game.update_disgruntled(&[worker]);
    assert!(
        game.world.get::<Disgruntled>(worker).is_none(),
        "a rival alone is -7"
    );

    set_situation(&mut game, worker, &EVERY_NEGATIVE);
    assert!((game.morale(worker) + 9.0).abs() < 1e-3);
    game.update_disgruntled(&[worker]);
    assert_eq!(
        game.world.get::<Disgruntled>(worker).map(|d| d.grievance),
        Some(Grievance::Sulking),
        "morale {}",
        game.morale(worker)
    );
}

#[test]
fn an_empty_thoughts_directory_leaves_morale_the_memory_fold() {
    let dir = assets_with_thoughts("situation_empty", &[]);
    // Not copied by `copy_shipped_assets`, and the memory fold needs defs.
    let memories = dir.join("memories");
    std::fs::create_dir_all(&memories).unwrap();
    for entry in std::fs::read_dir(test_assets_dir().join("memories")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), memories.join(entry.file_name())).unwrap();
    }
    let mut game = Game::new(67, DifficultyMode::Forgiving, &dir).unwrap();
    let worker = a_base_with_one_staff(&mut game);
    implant(
        &mut game,
        worker,
        "saw_turn_on",
        1,
        MemorySubject::Program(ProgramId(999)),
    );
    for _ in 0..3 {
        game.tick();
    }
    let remembered = crate::memories::sum_intensity(
        game.world.get::<Memories>(worker).unwrap(),
        game.world.resource::<crate::memories::MemoryDb>(),
        game.current_tick(),
        Default::default(),
        crate::memories::Read::Morale,
        |_| true,
    );
    assert!(remembered < 0.0, "the fixture must remember something");
    assert!(
        game.world
            .get::<Situation>(worker)
            .is_some_and(|s| s.thoughts.contains(&Trigger::NoAmenity)),
        "the assessment still runs; only the catalogue is empty"
    );
    assert_eq!(game.morale(worker), remembered);
}

#[test]
fn a_loaded_save_reports_the_situational_term_before_any_tick() {
    let mut game = Game::new(68, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = a_base_with_one_staff(&mut game);
    game.tick();
    assert!(
        game.world
            .get::<Situation>(worker)
            .unwrap()
            .thoughts
            .contains(&Trigger::NoAmenity),
        "the fixture base has nothing to unwind at"
    );
    let before = game.morale(worker);
    assert!(before < 0.0, "{before}");

    let path = save_path("situation_load");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let staff = loaded.base_staff();
    assert_eq!(staff.len(), 1);
    assert_eq!(loaded.morale(staff[0]), before);
}

/// The memories page is one derivation with morale: its rows sum to the figure
/// it is headed with, through the clamp, and the thought rows read as "now".
#[test]
fn the_memory_rows_sum_to_morale_including_situational_thoughts() {
    let mut game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    implant(
        &mut game,
        worker,
        "saw_turn_on",
        1,
        MemorySubject::Program(ProgramId(999)),
    );
    for thoughts in [
        &[Trigger::BesideFriend, Trigger::MachineRunning][..],
        &EVERY_NEGATIVE[..],
    ] {
        set_situation(&mut game, worker, thoughts);
        let rows = game.memory_report(worker);
        assert!(rows.iter().any(|r| r.age == "now" && r.subject.is_none()));
        let total: f32 = rows.iter().map(|r| r.intensity).sum();
        assert!((total - game.morale(worker)).abs() < 1e-4, "{total}");
    }
}

#[test]
fn thought_rows_show_without_any_memories() {
    let mut game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).remove::<Memories>();
    set_situation(&mut game, worker, &[Trigger::Unpowered]);
    let rows = game.memory_report(worker);
    assert_eq!(rows.len(), 1);
    assert!((rows[0].intensity - game.morale(worker)).abs() < 1e-5);
}

/// A program that leaves the staff takes its thoughts with it: the assessment
/// only writes staff, so it has to clear the rest, or a party member's page
/// and morale would carry a base it is no longer in.
#[test]
fn a_program_that_leaves_the_staff_loses_its_situation() {
    let mut game = Game::new(67, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    implant(&mut game, worker, "jammed_here", 1, MemorySubject::Nothing);
    let remembered = game.morale(worker);
    set_situation(&mut game, worker, &EVERY_NEGATIVE);
    assert!(game.morale(worker) < remembered);
    let without_thoughts = |game: &mut Game| {
        let held = game.world.entity_mut(worker).take::<Situation>();
        let fold = game.morale(worker);
        if let Some(held) = held {
            game.world.entity_mut(worker).insert(held);
        }
        fold
    };

    game.world.resource_mut::<Party>().0.push(worker);
    game.tick();

    assert!(game.world.get::<Situation>(worker).is_none());
    assert_eq!(game.morale(worker), without_thoughts(&mut game));
    assert!(game.memory_report(worker).iter().all(|r| r.age != "now"));
}
