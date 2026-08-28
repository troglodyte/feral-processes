//! A program's hidden temperament: the two axes it moves, and what survives
//! a save.
//!
//! The unit tests for the table itself live beside it in
//! `crate::disposition`. What is asserted here is that the table actually
//! *reaches* the two seams — a multiplier nothing multiplies by is the
//! failure mode this whole file exists for, and it is invisible to a test
//! that only reads `Disposition::need_drain()`.

use super::support::*;
use crate::components::{Memories, MemorySubject, Needs, ProgramId};
use crate::disposition::Disposition;
use crate::needs::{NEED_MAX, NeedId};
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

fn set(game: &mut Game, who: Entity, d: Disposition) {
    game.world.entity_mut(who).insert(d);
}

/// The drain axis, through the system rather than through the table. Remove
/// the `* temperament` in `needs_drain_system` and this is what goes red.
#[test]
fn a_languid_program_runs_down_faster_than_a_dogged_one() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let languid = spawn_tamed(&mut game, 10, 3);
    let dogged = spawn_tamed(&mut game, 10, 3);
    set(&mut game, languid, Disposition::Languid);
    set(&mut game, dogged, Disposition::Dogged);

    game.tick();

    let (fast, slow) = (reserve(&game, languid), reserve(&game, dogged));
    assert!(
        fast < slow,
        "Languid must spend more of its reserve than Dogged in the same \
         tick, got {fast} against {slow}"
    );
    assert!(fast < NEED_MAX && slow < NEED_MAX, "both still drain");
}

/// A program with no disposition at all — every fixture written before this
/// feature — must drain at exactly the authored rate. This is the property
/// that keeps `Disposition` from being a difficulty change nobody asked for.
#[test]
fn a_program_with_no_disposition_drains_at_the_authored_rate() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(worker)
        .remove::<Disposition>()
        .insert(Disposition::Steady);
    let neutral = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(neutral).remove::<Disposition>();

    game.tick();

    assert!(
        (reserve(&game, worker) - reserve(&game, neutral)).abs() < 1e-6,
        "an absent disposition must read exactly as Steady"
    );
}

/// The memory axis, through `Game::morale` rather than through the table.
/// Both programs are handed the *same* memory; only how hard it lands
/// differs.
#[test]
fn an_abrasive_program_feels_a_grudge_harder_than_an_amiable_one() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abrasive = spawn_tamed(&mut game, 10, 3);
    let amiable = spawn_tamed(&mut game, 10, 3);
    set(&mut game, abrasive, Disposition::Abrasive);
    set(&mut game, amiable, Disposition::Amiable);

    let where_ = MemorySubject::BaseTile { x: 2, y: 2 };
    game.remember(abrasive, "frayed_here", where_.clone());
    game.remember(amiable, "frayed_here", where_);

    let (sour, sunny) = (game.morale(abrasive), game.morale(amiable));
    assert!(
        sour < 0.0 && sunny < 0.0,
        "a grudge stays a grudge for both"
    );
    assert!(
        sour < sunny,
        "the same grudge must weigh more on Abrasive than on Amiable, got \
         {sour} against {sunny}"
    );
}

/// The other pole of the same axis, so a one-sided implementation that only
/// amplifies cannot pass. A fondness must land harder on `Amiable`.
#[test]
fn an_amiable_program_feels_a_fondness_harder_than_an_abrasive_one() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let amiable = spawn_tamed(&mut game, 10, 3);
    let abrasive = spawn_tamed(&mut game, 10, 3);
    set(&mut game, amiable, Disposition::Amiable);
    set(&mut game, abrasive, Disposition::Abrasive);

    let kind = MemorySubject::Structure("mining_node".into());
    game.remember(amiable, "settled_in", kind.clone());
    game.remember(abrasive, "settled_in", kind);

    let (sunny, sour) = (game.morale(amiable), game.morale(abrasive));
    assert!(sunny > 0.0 && sour > 0.0, "a fondness stays a fondness");
    assert!(
        sunny > sour,
        "the same fondness must weigh more on Amiable, got {sunny} against {sour}"
    );
}

/// Every door into the roster passes through `roster_parts`, so a program
/// cannot arrive without one, and the one it arrives with is the one its id
/// derives.
///
/// Asserted against `adopt_program` and against the barrier itself rather
/// than against `spawn_tamed`, which pins `Steady` deliberately — see the
/// comment there. A fixture that pinned nothing would make every other test
/// in the suite seed-dependent.
#[test]
fn every_program_on_the_roster_is_born_with_a_disposition() {
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let adopted = game.adopt_program("scrapper", 4, 4, 1.0).unwrap();
    let id = game.world.get::<ProgramId>(adopted).expect("an id").0;
    assert_eq!(
        game.world.get::<Disposition>(adopted).copied(),
        Some(Disposition::seed(id)),
        "an adopted program's disposition is the one its id derives"
    );

    // The barrier itself, so a door added later that assembles its own
    // component list still cannot ship a program without one.
    let (.., minted) = game.roster_parts();
    assert!(
        Disposition::ALL.contains(&minted),
        "roster_parts mints a real disposition, got {minted:?}"
    );
}

/// A field-named RON round trip cannot catch a skipped field, so this goes
/// through a **real** save and load.
#[test]
fn a_disposition_survives_a_save_and_load() {
    let dir = scratch_assets_dir("disposition_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    // Written by hand to something the id would not have derived, so a load
    // path that re-seeds instead of reading the file fails here.
    let planted = Disposition::ALL
        .into_iter()
        .find(|d| *d != Disposition::seed(game.world.get::<ProgramId>(worker).unwrap().0))
        .expect("five variants, so one differs");
    set(&mut game, worker, planted);
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored: Vec<Disposition> = loaded
        .world
        .query::<&Disposition>()
        .iter(&loaded.world)
        .copied()
        .filter(|d| *d == planted)
        .collect();
    assert!(
        !restored.is_empty(),
        "the stored disposition survives the round trip rather than being re-seeded"
    );
}

/// A save written before dispositions existed carries no key. Every program
/// in it must come up with the disposition its `ProgramId` derives — not
/// `Steady`, which would leave an existing base full of neutral programs and
/// read as the feature not working.
#[test]
fn a_save_written_before_dispositions_seeds_from_the_program_id() {
    let dir = scratch_assets_dir("disposition_legacy_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(47, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_tamed(&mut game, 10, 3);
    spawn_tamed(&mut game, 10, 3);
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    for creature in &mut data.creatures {
        creature.disposition = None;
    }
    crate::save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let pairs: Vec<(u32, Disposition)> = loaded
        .world
        .query::<(&ProgramId, &Disposition)>()
        .iter(&loaded.world)
        .map(|(id, d)| (id.0, *d))
        .collect();
    assert!(!pairs.is_empty(), "the roster survived the load");
    for (id, d) in pairs {
        assert_eq!(d, Disposition::seed(id), "program {id} seeded off its id");
    }
}

/// The documented exception: a disposition scales what a program *feels*,
/// never which memories it *keeps*. Scaling the eviction weight too would
/// make an Abrasive program drop its fondnesses first and fill its store
/// with grudges — a compounding loop on an axis that already compounds
/// through morale.
#[test]
fn eviction_does_not_read_a_disposition() {
    let cap = crate::tuning::MEMORY_CAP_PER_PROGRAM;
    let mut game = Game::new(48, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abrasive = spawn_tamed(&mut game, 10, 3);
    let steady = spawn_tamed(&mut game, 10, 3);
    set(&mut game, abrasive, Disposition::Abrasive);
    set(&mut game, steady, Disposition::Steady);

    // Alternating poles, past the cap, so eviction has to choose between a
    // fondness and a grudge on every write after the first `cap`.
    for i in 0..(cap as i32 + 4) {
        let (def, subject) = if i % 2 == 0 {
            ("frayed_here", MemorySubject::BaseTile { x: i, y: 0 })
        } else {
            (
                "settled_in",
                MemorySubject::Structure(format!("m{i}").into()),
            )
        };
        game.remember(abrasive, def, subject.clone());
        game.remember(steady, def, subject);
    }

    let kept = |g: &Game, who: Entity| -> Vec<String> {
        g.world
            .get::<Memories>(who)
            .expect("a store")
            .0
            .iter()
            .map(|m| format!("{}:{:?}", m.def.as_str(), m.subject))
            .collect()
    };
    assert_eq!(
        kept(&game, abrasive),
        kept(&game, steady),
        "the store's contents must not depend on the disposition reading them"
    );
}
