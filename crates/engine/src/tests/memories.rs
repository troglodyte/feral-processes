//! Entity memories: the identity a memory is *about*, and what one is worth.
//!
//! `Entity` is written to the save nowhere, because entity ids are not
//! stable across a round trip. `ProgramId` is what a memory names instead,
//! minted at `Game::roster_parts` — the one barrier all four doors into the
//! roster pass through.

use super::support::*;
use crate::components::MachineStatus;
use crate::components::{Memories, Memory, MemorySubject, ProgramId};
use crate::game::base::work_orders::park_tile;
use crate::game::memories::Remembered;
use crate::memories::{MemoryDef, MemoryId, MemorySubjectKind};
use crate::resources::GameClock;
use crate::tuning::{MEMORY_CAP_PER_PROGRAM, MEMORY_POSTING_PERIOD};
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

/// A **save → load → assert**, not a RON round trip: a round trip cannot
/// tell a field that fails to reach the file from one that does, which is
/// exactly what `#[serde(skip)]` looks like from its side.
#[test]
fn a_program_id_survives_a_save_and_load() {
    let dir = scratch_assets_dir("program_id_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = game.adopt_program("scrapper", 4, 4, 1.0).unwrap();
    let id = game.world.get::<ProgramId>(program).unwrap().0;
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(owned_ids(&mut loaded), vec![id], "the id has to travel");
}

/// A save written before this feature carries the sentinel for everyone, so
/// the load path mints. Distinct ids, or two programs share an identity for
/// the rest of the run.
#[test]
fn a_legacy_save_mints_an_id_for_every_owned_program() {
    let dir = scratch_assets_dir("program_id_legacy");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = three_owned_programs(&dir);
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    for c in &mut data.creatures {
        c.program_id = 0;
    }
    data.next_program_id = 0;
    crate::save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let ids = owned_ids(&mut loaded);
    assert_eq!(ids.len(), 3, "every program came back");
    assert!(
        ids.iter().all(|&id| id != 0),
        "none kept the sentinel: {ids:?}"
    );
    let mut distinct = ids.clone();
    distinct.dedup();
    assert_eq!(distinct, ids, "and no two share one: {ids:?}");
}

/// Minting is for the sentinel alone. An id already in the file is that
/// program's name and nothing may reissue it.
#[test]
fn an_id_already_in_the_file_is_never_minted_again() {
    let dir = scratch_assets_dir("program_id_mixed");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = three_owned_programs(&dir);
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    let mut kept = Vec::new();
    for (i, c) in data.creatures.iter_mut().filter(|c| c.tamed).enumerate() {
        if i == 0 {
            kept.push(c.program_id);
        } else {
            c.program_id = 0;
        }
    }
    crate::save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let ids = owned_ids(&mut loaded);
    assert!(
        ids.contains(&kept[0]),
        "the saved id survives untouched: {ids:?} should hold {kept:?}"
    );
    assert_eq!(
        ids.iter().filter(|&&id| id == kept[0]).count(),
        1,
        "and nothing was minted on top of it: {ids:?}"
    );
}

/// The counter is restored past the highest id *in the file*, not from the
/// saved counter alone — a hand-edited or savetool-packed save can carry ids
/// the counter has never seen, and reissuing one names two programs at once.
#[test]
fn the_counter_lands_above_every_id_seen() {
    let dir = scratch_assets_dir("program_id_counter");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = three_owned_programs(&dir);
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    if let Some(c) = data.creatures.iter_mut().find(|c| c.tamed) {
        c.program_id = 500;
    }
    data.next_program_id = 1;
    crate::save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let minted = loaded.adopt_program("scrapper", 7, 7, 1.0).unwrap();
    let id = loaded.world.get::<ProgramId>(minted).unwrap().0;
    assert!(
        id > 500,
        "the next id must clear every id in the file: {id}"
    );
}

/// Three owned programs of shipped species — a fixture species would be
/// dropped on load, since `Game::load` resolves a creature against
/// `SpeciesDb` and skips what it cannot name.
fn three_owned_programs(_dir: &ScratchAssets) -> Game {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for (species, x) in [("scrapper", 4), ("glitch", 5), ("drone", 6)] {
        game.adopt_program(species, x, 4, 1.0).unwrap();
    }
    game
}

/// Every owned program's id, sorted. `Option` rather than a filter, so a
/// program that came back with no component at all reads as the sentinel
/// rather than vanishing from the assertion.
fn owned_ids(game: &mut Game) -> Vec<u32> {
    let mut ids: Vec<u32> = game
        .world
        .query_filtered::<Option<&ProgramId>, With<crate::components::Tamed>>()
        .iter(&game.world)
        .map(|p| p.map_or(0, |p| p.0))
        .collect();
    ids.sort_unstable();
    ids
}

// ---------------------------------------------------------------------------
// Phase 2: the record, and intensity derived from the clock.
//
// Pure arithmetic on hand-built values — no `Game`, no world, no assets. What
// a memory *is worth right now* is a function of the def, the clock and two
// integers, and nothing else may be needed to state it.
// ---------------------------------------------------------------------------

/// A def differing only in the fields a test cares about. Deliberately not a
/// shipped one: a census-approved `.ron` retuned tomorrow must not move an
/// arithmetic test.
fn test_def(valence: f32, half_life: u64, strike_cap: u32) -> MemoryDef {
    MemoryDef {
        id: MemoryId::from("fixture"),
        name: "Fixture".to_string(),
        blurb: "b".to_string(),
        valence,
        half_life,
        subject: MemorySubjectKind::Nothing,
        strike_cap,
    }
}

fn memory_at(reinforced: u64, strikes: u32) -> Memory {
    Memory {
        def: MemoryId::from("fixture"),
        subject: MemorySubject::Nothing,
        subject_name: None,
        reinforced,
        strikes,
    }
}

/// The one test that says the exponent is `(now - reinforced) / half_life`
/// and not something adjacent to it. Halving is what a half-life *means*, so
/// this is a definition rather than a sample.
#[test]
fn intensity_halves_at_one_half_life_and_quarters_at_two() {
    let def = test_def(4.0, 100, 5);
    let m = memory_at(1000, 1);

    let fresh = m.intensity(&def, 1000);
    let one = m.intensity(&def, 1100);
    let two = m.intensity(&def, 1200);

    assert!((fresh - 4.0).abs() < 1e-5, "{fresh}");
    assert!(
        (one - 2.0).abs() < 1e-5,
        "one half-life left {one}, wanted 2.0"
    );
    assert!(
        (two - 1.0).abs() < 1e-5,
        "two half-lives left {two}, wanted 1.0"
    );
}

/// The moment it forms is the zero point of the decay, not tick zero. A
/// memory formed on tick 900 of a long run is worth its full valence, or
/// every memory a veteran roster forms is born already faded.
#[test]
fn intensity_is_undecayed_at_the_moment_it_forms() {
    let def = test_def(4.0, 100, 5);
    let now = 900;
    let m = memory_at(now, 3);

    let at_formation = m.intensity(&def, now);

    assert!(
        (at_formation - 12.0).abs() < 1e-5,
        "a memory formed on tick {now} is worth {at_formation}, wanted 12.0 — \
         the decay's zero point is `reinforced`, not tick 0"
    );
}

/// Reinforcement compounds, and the def's cap is where it stops. The two
/// caps are compared against each other so the test cannot pass by ignoring
/// `strike_cap` altogether.
#[test]
fn strikes_compound_intensity_up_to_the_cap_and_no_further() {
    let now = 500;
    let three = memory_at(now, 3);
    let four = memory_at(now, 4);

    let under = three.intensity(&test_def(4.0, 100, 3), now);
    let clamped = three.intensity(&test_def(4.0, 100, 2), now);
    let past = four.intensity(&test_def(4.0, 100, 3), now);

    assert!(
        (under - 12.0).abs() < 1e-5,
        "three strikes of three: {under}"
    );
    assert!(
        (clamped - 8.0).abs() < 1e-5,
        "a cap of 2 must bind the third strike: {clamped}"
    );
    assert!(
        (past - under).abs() < 1e-5,
        "a fourth strike past a cap of 3 changed the figure: {past} vs {under}"
    );
}

/// Decay is a magnitude scale, never a sign flip. `morale` is a signed sum
/// over exactly this figure, so a grudge that decayed into a fondness would
/// read as the roster cheering up because it was hurt a while ago.
#[test]
fn a_negative_valence_stays_negative_however_it_decays() {
    let def = test_def(-8.0, 100, 4);
    let m = memory_at(200, 2);

    for elapsed in [0, 50, 100, 400, 10_000] {
        let v = m.intensity(&def, 200 + elapsed);
        assert!(v < 0.0, "a grudge read {v} after {elapsed} ticks");
    }
    assert!((m.intensity(&def, 300) + 8.0).abs() < 1e-5);
}

/// The global stickiness dial has to actually be in the denominator, and at
/// its shipped neutral value that is invisible in any single figure — so the
/// test varies the dial and asserts the *ordering*, rather than pinning a
/// number and stopping the dial being a dial.
#[test]
fn the_half_life_multiplier_scales_every_grudge_at_once() {
    let def = test_def(-8.0, 100, 4);
    let m = memory_at(0, 1);
    let dial = crate::tuning::MEMORY_HALF_LIFE_MULTIPLIER;

    let normal = m.intensity_with(&def, 100, dial);
    let stickier = m.intensity_with(&def, 100, dial * 2.0);

    assert!(
        stickier.abs() > normal.abs(),
        "doubling the dial left {stickier} against {normal}; the same elapsed \
         ticks must cost less intensity when memory is stickier"
    );
    assert!(
        (m.intensity(&def, 100) - normal).abs() < 1e-6,
        "`intensity` must be `intensity_with` at the shipped dial, or the dial \
         turns nothing"
    );
}

// ---------------------------------------------------------------------------
// Phase 2: the one door.
//
// `Game::remember` is the only writer, on the model of `Game::apply_damage`.
// Everything below asserts a property of that door: what it forms, what it
// reinforces, what it evicts, and — as often — what it refuses to touch.
// ---------------------------------------------------------------------------

/// What `who` is holding. Panics on a missing component, because every test
/// past the no-op ones is already claiming the store exists.
fn memories_of(game: &Game, who: Entity) -> Vec<Memory> {
    game.world
        .get::<Memories>(who)
        .expect("an owned program holds a store")
        .0
        .clone()
}

/// Sets the clock directly rather than ticking to it. `Game::tick` runs every
/// background system — spawns, raids, entropy — and what these tests are
/// about is arithmetic against the clock, not what else a thousand ticks do.
fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

fn id_of(game: &Game, who: Entity) -> ProgramId {
    *game
        .world
        .get::<ProgramId>(who)
        .expect("an owned program carries an id")
}

/// One `remember` on an empty store is one entry, at full strength, stamped
/// with the tick it landed on. The clock is moved off zero first, or the
/// stamp assertion passes against a hardcoded `0`.
#[test]
fn a_first_remember_forms_one_entry_at_the_current_tick() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    set_tick(&mut game, 4_000);

    let outcome = game.remember(program, "hard_won", MemorySubject::Nothing);

    assert_eq!(outcome, Remembered::Written);
    let held = memories_of(&game, program);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].def, MemoryId::from("hard_won"));
    assert_eq!(held[0].strikes, 1);
    assert_eq!(
        held[0].reinforced, 4_000,
        "the stamp is the clock, not tick zero"
    );
    assert_eq!(held[0].reinforced, game.current_tick());
}

/// Reinforcement is the whole reason a memory is keyed rather than appended:
/// the same thing happening twice is one memory that got worse, not two.
#[test]
fn remembering_the_same_thing_again_reinforces_rather_than_forking() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    set_tick(&mut game, 100);
    game.remember(program, "hard_won", MemorySubject::Nothing);

    set_tick(&mut game, 600);
    game.remember(program, "hard_won", MemorySubject::Nothing);

    let held = memories_of(&game, program);
    assert_eq!(held.len(), 1, "one memory, not two: {held:?}");
    assert_eq!(held[0].strikes, 2);
    assert_eq!(
        held[0].reinforced, 600,
        "reinforcement resets the decay's zero point"
    );
}

/// Identity is the `(def, subject)` pair, not the def. Being bonded to two
/// programs is two memories or the second one silently overwrites the first.
#[test]
fn two_subjects_of_one_def_are_two_memories() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = spawn_tamed(&mut game, 10, 3);
    let one = spawn_tamed(&mut game, 10, 3);
    let two = spawn_tamed(&mut game, 10, 3);
    let (a, b) = (id_of(&game, one), id_of(&game, two));

    game.remember(holder, "bonded_in_battle", MemorySubject::Program(a));
    game.remember(holder, "bonded_in_battle", MemorySubject::Program(b));

    let held = memories_of(&game, holder);
    assert_eq!(held.len(), 2, "one def, two subjects: {held:?}");
    assert!(held.iter().all(|m| m.strikes == 1), "{held:?}");
}

/// The remembered name is refreshed, never compared. In the key it would
/// fork a program's whole history the first time the player renames it.
#[test]
fn a_renamed_subject_reinforces_rather_than_forking() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = spawn_tamed(&mut game, 10, 3);
    let subject = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, subject);
    game.remember(holder, "bonded_in_battle", MemorySubject::Program(id));
    let before = memories_of(&game, holder)[0].subject_name.clone();
    assert!(before.is_some(), "a living program's name is captured");

    game.rename_companion(subject, Some("Kestrel".to_string()))
        .unwrap();
    set_tick(&mut game, 50);
    game.remember(holder, "bonded_in_battle", MemorySubject::Program(id));

    let held = memories_of(&game, holder);
    assert_eq!(held.len(), 1, "a rename must not fork a history: {held:?}");
    assert_eq!(held[0].strikes, 2);
    let after = held[0].subject_name.clone();
    assert!(
        after.as_deref().is_some_and(|n| n.contains("Kestrel")),
        "the name is refreshed at the write, got {after:?}"
    );
    assert_ne!(after, before, "and it actually moved");
}

/// The def's `strike_cap` is where compounding stops, and it binds at the
/// write rather than only at the read — an uncapped counter would keep
/// climbing and make the cap invisible in the store.
#[test]
fn strikes_saturate_at_the_defs_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);

    for _ in 0..5 {
        game.remember(program, "hard_won", MemorySubject::Nothing);
    }

    let held = memories_of(&game, program);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(
        held[0].strikes, 3,
        "`hard_won` caps at three strikes: {held:?}"
    );
}

/// Eviction is lazy and this is where it happens — nothing sweeps. The
/// unrelated memory that triggers it is a *grudge*, so a threshold that
/// compared the signed value rather than the magnitude would throw the fresh
/// one away too and leave the store empty.
#[test]
fn a_faded_entry_is_dropped_at_the_next_formation() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    set_tick(&mut game, 1_000);
    game.remember(
        program,
        "stranded_at",
        MemorySubject::BaseTile { x: 1, y: 1 },
    );
    assert_eq!(memories_of(&game, program).len(), 1);

    // Six half-lives of `stranded_at`, which takes |-6.0| under the
    // forget threshold with room to spare.
    set_tick(&mut game, 21_000);
    game.remember(
        program,
        "mauled_by",
        MemorySubject::Species("scrapper".to_string()),
    );

    let held = memories_of(&game, program);
    assert_eq!(held.len(), 1, "the faded one is gone: {held:?}");
    assert_eq!(
        held[0].def,
        MemoryId::from("mauled_by"),
        "and the fresh grudge is what survived: {held:?}"
    );
}

/// Over the cap the weakest goes. By **magnitude** — under a signed
/// comparison the deepest grudge in the store is the smallest number in it,
/// so the strongest memory a program holds would be the first one dropped.
#[test]
fn over_the_cap_the_weakest_goes_and_the_strongest_survives() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    let strongest = MemorySubject::BaseTile { x: 0, y: 0 };
    for _ in 0..3 {
        game.remember(program, "stranded_at", strongest.clone());
    }

    for tile in 1..=MEMORY_CAP_PER_PROGRAM as i32 {
        game.remember(
            program,
            "stranded_at",
            MemorySubject::BaseTile { x: tile, y: 0 },
        );
    }

    let held = memories_of(&game, program);
    assert_eq!(held.len(), MEMORY_CAP_PER_PROGRAM, "{held:?}");
    let kept = held.iter().find(|m| m.subject == strongest);
    assert!(
        kept.is_some_and(|m| m.strikes == 3),
        "the three-strike memory is the strongest in the store and must be \
         the last thing dropped, not the first: {held:?}"
    );
}

/// A body with no store is a silent no-op, the same asymmetry `spend_power`
/// uses for a missing `PowerReserve` — it is what keeps every call site from
/// needing a branch. All three in one test: the hostile arm alone passes
/// against a fix that only checks `Hostile`.
#[test]
fn remember_is_a_no_op_on_a_hostile_a_structure_and_the_player() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let hostile = spawn_wild_on_player_tile(&mut game);
    let structure = spawn_machine_at(&mut game, "lathe", 6, 6);
    let player = game.player_entity();

    for (what, who) in [
        ("a hostile", hostile),
        ("a structure", structure),
        ("the player", player),
    ] {
        assert_eq!(
            game.remember(who, "hard_won", MemorySubject::Nothing),
            Remembered::NoStore,
            "{what} holds no memories"
        );
        assert!(
            game.world.get::<Memories>(who).is_none(),
            "{what} must not acquire a store by being remembered at"
        );
    }
}

/// The def declares what it is about and the write checks the pairing. A
/// mismatch is a programming error, so it is refused rather than absorbed —
/// and refused *before* anything is written.
#[test]
fn a_subject_of_the_wrong_kind_is_refused() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);

    let outcome = game.remember(
        program,
        "stranded_at",
        MemorySubject::Species("scrapper".to_string()),
    );

    assert_eq!(outcome, Remembered::WrongSubject);
    assert!(memories_of(&game, program).is_empty());
}

/// The deleted-mod-file case, and the empty-database property at the write
/// end: an id no file defines writes nothing at all.
#[test]
fn an_unknown_def_id_is_a_silent_no_op() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);

    let outcome = game.remember(program, "no_such_memory", MemorySubject::Nothing);

    assert_eq!(outcome, Remembered::UnknownDef);
    assert!(memories_of(&game, program).is_empty());
    let log = game.message_log(20);
    assert!(
        !log.iter().any(|l| l.text.contains("no_such_memory")),
        "and it is silent: {:?}",
        log.iter().map(|l| &l.text).collect::<Vec<_>>()
    );
}

/// `remember` draws no RNG at all — not on the written path and not on any
/// of the three refusals. That is what keeps every seeded test and every
/// `dev-arenas/` report where they are. Two games from one seed, identically
/// set up, and the next draw compared: internals are never read.
#[test]
fn remember_draws_no_rng() {
    fn probe(game: &mut Game) -> u64 {
        game.world.resource_mut::<GameRng>().0.random()
    }
    let mut control = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut subject = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let _held = spawn_tamed(&mut control, 10, 3);
    let program = spawn_tamed(&mut subject, 10, 3);

    for _ in 0..4 {
        subject.remember(program, "hard_won", MemorySubject::Nothing);
    }
    subject.remember(
        program,
        "stranded_at",
        MemorySubject::BaseTile { x: 2, y: 2 },
    );
    subject.remember(program, "no_such_memory", MemorySubject::Nothing);
    subject.remember(program, "mauled_by", MemorySubject::Nothing);
    subject.remember(subject.player_entity(), "hard_won", MemorySubject::Nothing);

    assert_eq!(
        probe(&mut subject),
        probe(&mut control),
        "a `remember` moved the shared stream"
    );
}

/// The store is minted at the roster barrier, so every door hands one out.
/// `fuse_companions` is the one that assembles its own component list, which
/// makes it the door a widened tuple can silently skip — and the symptom is
/// one companion whose screen is always empty, which reads as memories being
/// broken rather than as a door short a component.
#[test]
fn every_door_into_the_roster_hands_out_a_memory_store() {
    let dir = scratch_assets_with_achievement(
        "remembering_program",
        r#"(
            id: "remembering_program",
            name: "Remembering Program",
            description: "d",
            trigger: ZoneReached(2),
            reward: StartingProgram("scrapper"),
        )"#,
    );
    let mut game = Game::new(38, DifficultyMode::Forgiving, &dir).unwrap();
    game.install_profile(super::achievements::profile_of("remembering_program", None));
    game.grant_profile_rewards();

    let granted = *owned_programs(&mut game)
        .first()
        .expect("the profile hands the run a program");
    let adopted = game.adopt_program("scrapper", 4, 4, 1.0).unwrap();
    let one = spawn_tamed(&mut game, 20, 10);
    let two = spawn_tamed(&mut game, 10, 6);
    let before = owned_programs(&mut game);
    game.fuse_companions(one, two, None).unwrap();
    let fused = *owned_programs(&mut game)
        .iter()
        .find(|e| !before.contains(e))
        .expect("fusion leaves a program behind");

    for (door, who) in [
        ("grant_starting_program", granted),
        ("adopt_program", adopted),
        ("fuse_companions", fused),
    ] {
        assert!(
            game.world.get::<Memories>(who).is_some(),
            "{door} handed out a program that can never hold a memory"
        );
    }
}

// ---------------------------------------------------------------------------
// Phase 2: the two readers.
//
// `morale` is a signed sum over every memory's *current* intensity, and
// `opinion_of` is that same sum restricted to one subject. Both are `&self`:
// they derive, and eviction stays `remember`'s alone, or a read-only screen
// would rewrite the roster it is drawing.
// ---------------------------------------------------------------------------

/// Pushes an entry the write door would refuse, so a reader can be asked
/// about a store the door could never have built — an id no file defines is
/// the deleted-mod-file state, and `remember` returns `UnknownDef` rather
/// than writing one.
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

/// The sum is **signed**, so a fondness and a grudge cancel rather than
/// compound. Summing magnitudes would make the most miserable program in the
/// base read exactly like the happiest one.
#[test]
fn morale_sums_every_memorys_current_intensity() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    set_tick(&mut game, 1_000);
    game.remember(program, "hard_won", MemorySubject::Nothing);
    game.remember(
        program,
        "mauled_by",
        MemorySubject::Species("scrapper".to_string()),
    );

    set_tick(&mut game, 4_000);
    let morale = game.morale(program);

    // The two figures written out rather than taken from `intensity`: this
    // test is about the fold, and a sum of whatever the store happens to say
    // asserts nothing about what was summed.
    let good = 5.0 * 2f32.powf(-3_000.0 / 5_000.0);
    let bad = -8.0 * 2f32.powf(-3_000.0 / 6_000.0);
    assert!(
        (morale - (good + bad)).abs() < 1e-4,
        "{morale} against {}",
        good + bad
    );
    assert!(
        morale < 0.0,
        "the deeper grudge outweighs the win: {morale}"
    );
    assert!(
        (morale - (good - bad)).abs() > 1.0,
        "and it is not a sum of magnitudes: {morale}"
    );
}

/// Nothing stores what a memory is worth, so morale is a reading of the
/// clock and not of a field. Two readings of one unchanged store, and the
/// later one is strictly lower.
#[test]
fn morale_falls_as_a_good_memory_fades() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    set_tick(&mut game, 1_000);
    game.remember(program, "hard_won", MemorySubject::Nothing);

    let fresh = game.morale(program);
    set_tick(&mut game, 6_000);
    let later = game.morale(program);

    assert!((fresh - 5.0).abs() < 1e-4, "undecayed: {fresh}");
    assert!(
        later < fresh,
        "the same store read later is worth less: {later} against {fresh}"
    );
    assert!(later > 0.0, "and it is a fade, not a sign flip: {later}");
}

/// Identity is the subject, and an opinion is the total of what one program
/// holds *about that one thing* — not its whole mood, and not one entry.
#[test]
fn opinion_of_counts_only_memories_about_that_subject() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    set_tick(&mut game, 1_000);
    let scrapper = MemorySubject::Species("scrapper".to_string());
    game.remember(program, "mauled_by", scrapper.clone());
    game.remember(
        program,
        "mauled_by",
        MemorySubject::Species("sentinel".to_string()),
    );
    game.remember(program, "hard_won", MemorySubject::Nothing);

    let opinion = game.opinion_of(program, &scrapper);
    let morale = game.morale(program);

    assert!((opinion - -8.0).abs() < 1e-4, "one mauling: {opinion}");
    assert!((morale - -11.0).abs() < 1e-4, "all three: {morale}");
}

/// A subject nothing has ever happened about is a real answer, not a missing
/// one: zero, finite, and no panic on a store that has entries in it.
#[test]
fn opinion_of_an_unremembered_subject_is_zero() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    game.remember(program, "hard_won", MemorySubject::Nothing);

    let opinion = game.opinion_of(program, &MemorySubject::Species("no_such".to_string()));

    assert!(opinion.is_finite(), "not NaN: {opinion}");
    assert_eq!(opinion, 0.0);
}

/// Reading a body that holds nothing and reading a body that *cannot* hold
/// anything are the same answer. Both halves in one test: an empty-store
/// assertion alone passes against a reader that unwraps the component.
#[test]
fn a_program_with_no_memories_has_zero_morale() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    let hostile = spawn_wild_on_player_tile(&mut game);
    let player = game.player_entity();
    assert!(memories_of(&game, program).is_empty());

    for (what, who) in [
        ("an owned program with an empty store", program),
        ("a hostile, which has no store at all", hostile),
        ("the player, likewise", player),
    ] {
        assert_eq!(game.morale(who), 0.0, "{what}");
        assert_eq!(game.opinion_of(who, &MemorySubject::Nothing), 0.0, "{what}");
    }
}

/// A removed mod file leaves entries the catalogue can no longer weigh. They
/// are kept — restoring the file restores them — so every reader has to skip
/// what it cannot resolve rather than unwrap it.
#[test]
fn a_memory_naming_an_unknown_def_contributes_nothing() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    let orphan = MemorySubject::Species("scrapper".to_string());
    game.remember(program, "hard_won", MemorySubject::Nothing);
    implant(&mut game, program, "no_such_memory", orphan.clone());
    assert_eq!(memories_of(&game, program).len(), 2, "the entry is kept");

    assert!(
        (game.morale(program) - 5.0).abs() < 1e-4,
        "only the resolvable one counts: {}",
        game.morale(program)
    );
    assert_eq!(game.opinion_of(program, &orphan), 0.0);
}

/// Deleting `assets/memories/` is a supported way to play, and this is that
/// property at the read end: the store still holds what it held, and every
/// reader answers zero because nothing can weigh it.
#[test]
fn an_empty_database_leaves_every_reader_at_zero() {
    let dir = scratch_assets_dir("memories_absent");
    std::fs::create_dir_all(&*dir).unwrap();
    copy_shipped_assets(&dir, &[]);
    // Asserted rather than assumed: `copy_shipped_assets` walks a hardcoded
    // list of subdirectory names, and adding `"memories"` to it later would
    // invert this test in silence. This is what turns that edit into a
    // failure instead.
    assert!(
        !dir.join("memories").exists(),
        "this test is about an install with no catalogue at all"
    );

    let mut game = Game::new(41, DifficultyMode::Forgiving, &dir).unwrap();
    let program = spawn_tamed(&mut game, 10, 3);
    let subject = MemorySubject::Species("scrapper".to_string());
    implant(&mut game, program, "mauled_by", subject.clone());
    implant(&mut game, program, "hard_won", MemorySubject::Nothing);

    assert_eq!(
        memories_of(&game, program).len(),
        2,
        "the store is not the point"
    );
    assert_eq!(game.morale(program), 0.0);
    assert_eq!(game.opinion_of(program, &subject), 0.0);
}

// ---------------------------------------------------------------------------
// Phase 5: the save round trip.
//
// Every test here goes through `Game::save` and `Game::load` — the real
// doors, and a file on disk. A RON round trip is the weaker instrument and
// deliberately not what these use: `#[serde(skip)]` leaves one green while
// the field never reaches the file at all.
// ---------------------------------------------------------------------------

/// Saves to scratch, lets `edit` stand in for however the file came to be
/// what it is, and loads it back.
///
/// The scratch guard lives for the whole call, so the file outlives the save
/// and dies with the directory — an engine fixture leaking into `/tmp`
/// exhausted the tmpfs inode table here once already.
fn round_trip_with(
    game: &mut Game,
    tag: &str,
    edit: impl FnOnce(&mut crate::save::SaveData),
) -> Game {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    edit(&mut data);
    crate::save::save_to_file(&path, &data).unwrap();
    Game::load(&path, &test_assets_dir()).unwrap()
}

fn round_trip(game: &mut Game, tag: &str) -> Game {
    round_trip_with(game, tag, |_| {})
}

/// The program answering to `id` after a load. Entity ids are not stable
/// across the round trip, which is the whole reason `ProgramId` exists.
fn by_id(game: &mut Game, id: ProgramId) -> Entity {
    game.world
        .query::<(Entity, &ProgramId)>()
        .iter(&game.world)
        .find(|(_, held)| **held == id)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{id:?} came back from the load"))
}

/// A companion whose species a shipped file defines — `Game::load` resolves
/// every creature against `SpeciesDb` and drops what it cannot name, so a
/// `spawn_tamed` fixture never survives a save.
fn adopt(game: &mut Game, species: &str, x: i32) -> Entity {
    game.adopt_program(species, x, 4, 1.0)
        .expect("a shipped species")
}

/// The def, the subject, the strike count and the tick it last landed on all
/// have to travel, and `morale` has to read the same figure on the other
/// side — a reinforcement count that came back as 1 is a program that
/// quietly forgot half of what happened to it.
#[test]
fn a_programs_memories_survive_a_save_and_load() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = adopt(&mut game, "scrapper", 4);
    let id = id_of(&game, program);
    let mauling = MemorySubject::Species("scrapper".to_string());
    set_tick(&mut game, 3_000);
    game.remember(program, "mauled_by", mauling.clone());
    game.remember(program, "mauled_by", mauling.clone());
    set_tick(&mut game, 4_500);
    game.remember(program, "hard_won", MemorySubject::Nothing);
    let before = game.morale(program);

    let mut loaded = round_trip(&mut game, "memories_save");
    let who = by_id(&mut loaded, id);
    let mut held = memories_of(&loaded, who);
    held.sort_by_key(|m| m.def.clone());

    assert_eq!(held.len(), 2, "both entries travelled: {held:?}");
    assert_eq!(held[0].def, MemoryId::from("hard_won"));
    assert_eq!(held[0].subject, MemorySubject::Nothing);
    assert_eq!(held[0].strikes, 1);
    assert_eq!(held[0].reinforced, 4_500, "the tick it landed on");
    assert_eq!(held[1].def, MemoryId::from("mauled_by"));
    assert_eq!(held[1].subject, mauling, "and what it was about");
    assert_eq!(held[1].strikes, 2, "reinforcement is not re-rolled on load");
    assert_eq!(held[1].reinforced, 3_000);
    let after = loaded.morale(who);
    assert!(
        (after - before).abs() < 1e-4,
        "the same store weighs the same: {after} against {before}"
    );
}

/// Decision 2's whole reason. The name is stamped on the memory at the write
/// rather than resolved at the read, so it has to be in the file — resolved
/// at save time instead, a program destroyed before the next save takes its
/// name with it and the screen has nothing left to draw.
#[test]
fn a_remembered_name_survives_the_program_it_names() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = adopt(&mut game, "scrapper", 4);
    let comrade = adopt(&mut game, "glitch", 5);
    let id = id_of(&game, program);
    let comrade_id = id_of(&game, comrade);
    let name = game.creature_label(comrade);
    set_tick(&mut game, 1_000);
    game.remember(
        program,
        "bonded_in_battle",
        MemorySubject::Program(comrade_id),
    );
    game.dissolve_tamed_program(comrade);

    let mut loaded = round_trip(&mut game, "memories_name_save");
    let who = by_id(&mut loaded, id);
    let held = memories_of(&loaded, who);

    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].subject, MemorySubject::Program(comrade_id));
    assert_eq!(
        held[0].subject_name.as_deref(),
        Some(name.as_str()),
        "the name outlives the program"
    );
    assert!(
        loaded
            .world
            .query::<&ProgramId>()
            .iter(&loaded.world)
            .all(|held| *held != comrade_id),
        "and the program itself really is gone, so nothing re-resolved it"
    );
}

/// A file written before this field existed has no `memories` key at all,
/// which is what `#[serde(default)]` answers. Every owned program still gets
/// a store: a loaded companion that cannot *hold* a memory is a companion
/// whose screen stays empty for the rest of the run.
#[test]
fn a_save_written_before_memories_existed_loads_with_an_empty_store() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for (species, x) in [("scrapper", 4), ("glitch", 5)] {
        let program = adopt(&mut game, species, x);
        game.remember(program, "hard_won", MemorySubject::Nothing);
    }

    let mut loaded = round_trip_with(&mut game, "memories_legacy", |data| {
        for c in &mut data.creatures {
            c.memories.clear();
        }
    });

    let owned = owned_programs(&mut loaded);
    assert_eq!(owned.len(), 2, "both programs came back");
    for who in owned {
        let store = loaded.world.get::<Memories>(who);
        assert!(
            store.is_some(),
            "present and empty, never absent — absence means 'not on the roster'"
        );
        assert!(store.unwrap().0.is_empty());
        assert_eq!(
            loaded.remember(who, "hard_won", MemorySubject::Nothing),
            Remembered::Written,
            "and it can hold one from here on"
        );
    }
}

/// `MemorySubject` derives serde directly rather than being mirrored on the
/// save side, so every variant's encoding is this file's business.
/// `Activity(TaskKind)` is the one most likely to break: `TaskKind` gained
/// its derives for this and nothing else uses them.
///
/// Implanted rather than remembered, because the shipped catalogue declares
/// only four of the six kinds — and what is under test here is the encoding
/// of a subject, not the write door's kind check.
#[test]
fn every_subject_kind_survives_the_round_trip() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = adopt(&mut game, "scrapper", 4);
    let comrade = adopt(&mut game, "glitch", 5);
    let id = id_of(&game, program);
    let subjects = vec![
        MemorySubject::Nothing,
        MemorySubject::Program(id_of(&game, comrade)),
        MemorySubject::Species("scrapper".to_string()),
        MemorySubject::Structure("mining_node".to_string()),
        MemorySubject::BaseTile { x: -3, y: 7 },
        MemorySubject::Activity(crate::components::TaskKind::Excavate),
    ];
    for subject in &subjects {
        implant(&mut game, program, "hard_won", subject.clone());
    }

    let mut loaded = round_trip(&mut game, "memories_subjects");
    let who = by_id(&mut loaded, id);
    let held: Vec<MemorySubject> = memories_of(&loaded, who)
        .into_iter()
        .map(|m| m.subject)
        .collect();

    assert_eq!(held, subjects, "every variant, in the order it was written");
}

// ---------------------------------------------------------------------------
// Phase 3: the triggers.
//
// Four hooks, each at the one door its event already goes through. Everything
// below asserts what play writes into a store — the arithmetic of what a
// memory is *worth* is phase 2's business, above.
// ---------------------------------------------------------------------------

/// A hostile of `species`, standing off the map and hitting for `atk`.
///
/// Hand-built rather than spawned through `spawn_wild_creature`, because what
/// the maul threshold measures is a ratio between two hand-set numbers and a
/// rolled stat block cannot state one.
fn hostile_of(game: &mut Game, species: &str, atk: i32) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Position { x: 40, y: 40 },
            Stats {
                hp: 500,
                max_hp: 500,
                atk,
                mitigation: 0,
            },
            Hostile,
        ))
        .id()
}

/// One swing from `attacker` at `defender`, forced to land, for `power`.
///
/// A zero-spread band, so what lands is `power` cut by mitigation and nothing
/// else — the maul rule is a comparison against a fraction of max HP, and a
/// rolled band would make the case the test is pinning a coin flip.
fn one_landed_swing(game: &mut Game, attacker: Entity, defender: Entity, power: i32) -> i32 {
    force_the_next_attack_to_land(game);
    let outcome = game.resolve_and_apply_attack(
        attacker,
        defender,
        crate::battle::Swing::plain(crate::battle::DamageRange::centred(power, 0)),
    );
    outcome.damage_to_defender()
}

fn subjects_of(game: &Game, who: Entity, def: &str) -> Vec<MemorySubject> {
    memories_of(game, who)
        .into_iter()
        .filter(|m| m.def == MemoryId::from(def))
        .map(|m| m.subject)
        .collect()
}

/// Both halves in one test on purpose: the forming half alone passes against
/// a hook with no threshold in it at all, which is exactly the hook a
/// careless implementation writes.
#[test]
fn a_hit_above_the_maul_fraction_is_remembered_and_one_below_it_is_not() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = adopt(&mut game, "scrapper", 4);
    let attacker = hostile_of(&mut game, "glitch", 0);
    let max_hp = game.world.get::<Stats>(program).unwrap().max_hp;
    let over = (max_hp as f32 * crate::tuning::MEMORY_MAUL_FRACTION).ceil() as i32 + 2;
    let under = (max_hp as f32 * crate::tuning::MEMORY_MAUL_FRACTION).floor() as i32 - 2;
    assert!(under > 0, "the fixture needs room under the fraction");

    let landed = one_landed_swing(&mut game, attacker, program, under);
    assert!(
        landed > 0 && landed < over,
        "the light hit landed: {landed}"
    );
    assert!(
        subjects_of(&game, program, "mauled_by").is_empty(),
        "an ordinary hit is not a mauling"
    );

    game.restore_hp(program, max_hp);
    one_landed_swing(&mut game, attacker, program, over);

    assert_eq!(
        subjects_of(&game, program, "mauled_by"),
        vec![MemorySubject::Species("glitch".to_string())],
        "a hit that nearly ended it is remembered, by what swung it"
    );
}

/// The subject is the *attacker's* species. A fixture where both sides are
/// the same species cannot tell the two readings apart, so these differ.
#[test]
fn a_maul_names_the_attackers_species_and_not_its_own() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = adopt(&mut game, "scrapper", 4);
    let attacker = hostile_of(&mut game, "zero_day", 0);
    let max_hp = game.world.get::<Stats>(program).unwrap().max_hp;

    one_landed_swing(&mut game, attacker, program, max_hp);

    assert_eq!(
        subjects_of(&game, program, "mauled_by"),
        vec![MemorySubject::Species("zero_day".to_string())]
    );
}

/// What the comparison reads is what *landed*, not what was rolled — the
/// figure `apply_damage` returns after mitigation. Armour heavy enough to
/// pull a mauling swing under the fraction leaves nothing remembered.
#[test]
fn mitigation_can_take_a_swing_under_the_maul_fraction() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = adopt(&mut game, "scrapper", 4);
    let attacker = hostile_of(&mut game, "glitch", 0);
    let max_hp = {
        let mut stats = game.world.get_mut::<Stats>(program).unwrap();
        stats.mitigation = crate::tuning::MAX_MITIGATION_PERCENT;
        stats.max_hp
    };
    // Over the fraction before mitigation, under it after: the one band where
    // the two readings disagree.
    let rolled = (max_hp as f32 * crate::tuning::MEMORY_MAUL_FRACTION).ceil() as i32 + 2;

    let landed = one_landed_swing(&mut game, attacker, program, rolled);

    assert!(
        (landed as f32) < max_hp as f32 * crate::tuning::MEMORY_MAUL_FRACTION,
        "the fixture must land under the fraction: {landed} of {max_hp}"
    );
    assert!(
        subjects_of(&game, program, "mauled_by").is_empty(),
        "armour that absorbed the blow leaves no scar"
    );
}

/// The no-op rule, at the trigger rather than at the door: a body with no
/// store is not a body that panics. Two of them — a wild program mauled by
/// another, and the player.
#[test]
fn a_maul_on_a_body_with_no_store_is_a_no_op() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let attacker = hostile_of(&mut game, "glitch", 0);
    let wild = hostile_of(&mut game, "zero_day", 0);
    let player = game.player_entity();

    let wild_hp = game.world.get::<Stats>(wild).unwrap().max_hp;
    one_landed_swing(&mut game, attacker, wild, wild_hp);
    let player_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    one_landed_swing(&mut game, attacker, player, player_hp);

    assert!(game.world.get::<Memories>(wild).is_none());
    assert!(game.world.get::<Memories>(player).is_none());
}

/// Puts `who` in the party. `spawn_tamed` builds a program the player owns;
/// owning one and fighting beside it are different things.
fn join_party(game: &mut Game, who: Entity) {
    game.world.resource_mut::<Party>().0.push(who);
}

fn power_of(game: &Game, who: Entity) -> i32 {
    game.world.get::<Stats>(who).unwrap().power()
}

fn outmatched(game: &Game) -> bool {
    game.world.resource::<BattleState>().outmatched
}

/// The verdict `hard_won` is later read off, taken at the bell because by the
/// time the fight ends the hostiles are dead by definition.
#[test]
fn a_fight_records_whether_the_hostiles_outweighed_the_party() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = power_of(&game, game.player_entity());

    let overwhelming = hostile_of(&mut game, "glitch", player * 4);
    game.start_battle(vec![overwhelming]);
    assert!(outmatched(&game), "a pack four times the party's weight");

    game.world.remove_resource::<BattleState>();
    let scrap = hostile_of(&mut game, "glitch", 0);
    {
        let mut stats = game.world.get_mut::<Stats>(scrap).unwrap();
        stats.hp = 1;
        stats.max_hp = 1;
    }
    game.start_battle(vec![scrap]);
    assert!(!outmatched(&game), "and one that weighs nothing");
}

/// The party side of the comparison is the player *and* the companions.
/// `kill_xp` deliberately reads the player alone so recruiting cannot dock
/// XP; that argument does not transfer to "were we outmatched", and this is
/// the only fixture that can tell the two readings apart — the hostile
/// outweighs the companions on their own and is outweighed once the player
/// is counted.
#[test]
fn the_party_side_of_the_comparison_counts_the_player() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 12, 3);
    join_party(&mut game, companion);
    let player = power_of(&game, game.player_entity());
    let escort = power_of(&game, companion);
    assert!(
        player > 1,
        "the fixture needs the player to weigh something"
    );

    let hostile = hostile_of(&mut game, "glitch", 0);
    {
        let mut stats = game.world.get_mut::<Stats>(hostile).unwrap();
        stats.hp = escort + 1;
        stats.max_hp = escort + 1;
    }
    assert!(
        power_of(&game, hostile) > escort && power_of(&game, hostile) < escort + player,
        "the fixture must sit between the two readings"
    );
    game.start_battle(vec![hostile]);

    assert!(
        !outmatched(&game),
        "a party is not outmatched merely because the player is one body in it"
    );
}

/// A fight the party wins, torn down. The groups are emptied by hand because
/// an emptied roster **is** the engine's definition of a win — the same read
/// `settle_rewards` and the `FightEnd` record make — and hand-emptying it
/// states that in one line where playing the fight out would take a seeded
/// stream and a dozen rounds.
fn win_the_fight(game: &mut Game) {
    game.world.resource_mut::<BattleState>().groups.clear();
    let player = game.player_entity();
    game.end_battle(player, None);
}

/// Two companions, in the party, owned through the roster barrier.
fn two_companions(game: &mut Game) -> (Entity, Entity) {
    let a = adopt(game, "scrapper", 4);
    let b = adopt(game, "glitch", 5);
    join_party(game, a);
    join_party(game, b);
    (a, b)
}

fn bonded_subjects(game: &Game, who: Entity) -> Vec<MemorySubject> {
    subjects_of(game, who, "bonded_in_battle")
}

/// Each survivor remembers the *other*, and neither remembers itself.
#[test]
fn winning_together_bonds_each_survivor_to_the_others() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (a, b) = two_companions(&mut game);
    let (id_a, id_b) = (id_of(&game, a), id_of(&game, b));
    let hostile = hostile_of(&mut game, "glitch", 0);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![hostile]);

    win_the_fight(&mut game);

    assert_eq!(
        bonded_subjects(&game, a),
        vec![MemorySubject::Program(id_b)]
    );
    assert_eq!(
        bonded_subjects(&game, b),
        vec![MemorySubject::Program(id_a)]
    );
}

/// The player is never a subject and never a holder — not by a `Player`
/// check, but because the player carries no `ProgramId` at all.
#[test]
fn the_player_is_neither_bonded_to_nor_bonded_by() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = adopt(&mut game, "scrapper", 4);
    join_party(&mut game, companion);
    let player = game.player_entity();
    let hostile = hostile_of(&mut game, "glitch", 0);
    insert_battle(&mut game, player, vec![hostile]);

    win_the_fight(&mut game);

    assert!(
        bonded_subjects(&game, companion).is_empty(),
        "a lone companion has nobody to have fought beside"
    );
    assert!(game.world.get::<Memories>(player).is_none());
}

/// A companion that died winning is not bonded to: `bonded_in_battle` is
/// what *surviving* together is worth.
///
/// The hook is called directly rather than through `end_battle`, which reaps
/// the dead out of `Party` a few lines above it — through that door the rule
/// holds by another function's ordering and the test cannot tell whether this
/// one states it at all.
#[test]
fn a_companion_that_died_winning_is_not_bonded_to() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (survivor, fallen) = two_companions(&mut game);
    let id_fallen = id_of(&game, fallen);
    game.world.get_mut::<Stats>(fallen).unwrap().hp = 0;
    let hostile = hostile_of(&mut game, "glitch", 0);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![hostile]);
    game.world.resource_mut::<BattleState>().groups.clear();

    game.form_victory_memories();

    assert!(
        !bonded_subjects(&game, survivor).contains(&MemorySubject::Program(id_fallen)),
        "the survivor bonded with a program that did not survive"
    );
    assert!(
        memories_of(&game, fallen).is_empty(),
        "and the fallen came away with nothing"
    );
}

/// Fleeing leaves the groups standing, and that is the whole gate — a hook
/// without it passes every won-fight test above.
#[test]
fn a_fight_the_party_jacks_out_of_forms_nothing() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (a, b) = two_companions(&mut game);
    let hostile = hostile_of(&mut game, "glitch", 0);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![hostile]);
    game.world.resource_mut::<BattleState>().outmatched = true;

    game.end_battle(player, None);

    assert!(
        memories_of(&game, a).is_empty(),
        "{:?}",
        memories_of(&game, a)
    );
    assert!(memories_of(&game, b).is_empty());
}

/// Both halves, one test: a win against the odds is worth remembering and an
/// even one is not. The flag is `begin_battle`'s verdict, set here by hand
/// because what is under test is the hook that reads it.
#[test]
fn a_win_against_the_odds_is_remembered_and_an_even_one_is_not() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = adopt(&mut game, "scrapper", 4);
    join_party(&mut game, companion);
    let player = game.player_entity();
    let hostile = hostile_of(&mut game, "glitch", 0);

    insert_battle(&mut game, player, vec![hostile]);
    win_the_fight(&mut game);
    assert!(
        subjects_of(&game, companion, "hard_won").is_empty(),
        "an even fight is just a fight"
    );

    let hostile = hostile_of(&mut game, "glitch", 0);
    insert_battle(&mut game, player, vec![hostile]);
    game.world.resource_mut::<BattleState>().outmatched = true;
    win_the_fight(&mut game);

    assert_eq!(
        subjects_of(&game, companion, "hard_won"),
        vec![MemorySubject::Nothing]
    );
}

/// A second win beside the same program reinforces the one entry rather than
/// forking it — `remember`'s rule, reached through the trigger.
#[test]
fn winning_twice_together_reinforces_one_bond() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (a, b) = two_companions(&mut game);
    let player = game.player_entity();

    for _ in 0..2 {
        let hostile = hostile_of(&mut game, "glitch", 0);
        insert_battle(&mut game, player, vec![hostile]);
        win_the_fight(&mut game);
    }

    let held = memories_of(&game, a);
    let bonds: Vec<&Memory> = held
        .iter()
        .filter(|m| m.def == MemoryId::from("bonded_in_battle"))
        .collect();
    assert_eq!(bonds.len(), 1, "one entry, not two: {bonds:?}");
    assert_eq!(bonds[0].strikes, 2);
    assert_eq!(bonds[0].subject, MemorySubject::Program(id_of(&game, b)));
}

// ---------------------------------------------------------------------------
// Phase 3: a grudge against one corner of the base.
// ---------------------------------------------------------------------------

/// A base with one machine and one program posted to it — the smallest thing
/// that can strand a body.
///
/// The worker carries far more Integrity than it needs, for the reason
/// `tests::hauling::hauler` does: a posted program takes `RAID_DEFENDER_DAMAGE`
/// off every ambient sweep, and a body that dies mid-fixture reads as the
/// memory never forming rather than as the worker never surviving.
fn a_posted_worker(game: &mut Game) -> (Entity, Entity) {
    place_home(game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(crate::items::ids::CORE_FRAGMENT), 500);
    stand_in_base(game);
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    game.place_structure("mining_node", 1, 0).unwrap();
    let node = {
        let mut query = game.world.query::<(Entity, &Position, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, p, _)| p.x == px + 1 && p.y == py)
            .map(|(e, ..)| e)
            .expect("the node was just deployed")
    };
    let worker = spawn_tamed(game, 500, 3);
    game.assign_cronjob(worker, node).unwrap();
    (worker, node)
}

/// Puts `who` where no route to its post exists, which is what `Stranded`
/// names. Well outside any cost field a walk could build.
fn cut_off(game: &mut Game, who: Entity, x: i32, y: i32) {
    let mut pos = game.world.get_mut::<Position>(who).unwrap();
    pos.x = x;
    pos.y = y;
}

fn stranded_tiles(game: &Game, who: Entity) -> Vec<MemorySubject> {
    subjects_of(game, who, "stranded_at")
}

/// The tile remembered is the worker's **own**, not the machine's. Both
/// halves matter: a hook keyed to the post would form a memory too, and only
/// the coordinates can tell the two apart — and a memory about a tile a
/// `Structure` stands on could never be read by the parking hook it exists
/// for.
#[test]
fn a_stranded_worker_remembers_the_tile_it_is_standing_on() {
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (worker, node) = a_posted_worker(&mut game);
    let post = *game.world.get::<Position>(node).unwrap();
    cut_off(&mut game, worker, 400, 400);

    game.tick();

    assert_eq!(
        stranded_tiles(&game, worker),
        vec![MemorySubject::BaseTile { x: 400, y: 400 }],
        "the corner it was left in, not the post it never reached ({post:?})"
    );
}

/// The edge rule, and the whole reason `Stranded` carries a tick: a body left
/// standing there does not earn a fresh strike every tick it waits.
#[test]
fn staying_stranded_earns_no_second_strike() {
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (worker, _) = a_posted_worker(&mut game);
    cut_off(&mut game, worker, 400, 400);

    for _ in 0..8 {
        game.tick();
        cut_off(&mut game, worker, 400, 400);
    }

    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "one episode, one entry: {held:?}");
    assert_eq!(held[0].strikes, 1, "and one strike: {held:?}");
}

/// The other side of that rule: the edge has not simply frozen after the
/// first episode. A route repaired and broken again is a second stranding and
/// earns a second strike.
#[test]
fn a_second_stranding_earns_a_second_strike() {
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (worker, node) = a_posted_worker(&mut game);
    let post = *game.world.get::<Position>(node).unwrap();
    cut_off(&mut game, worker, 400, 400);
    game.tick();

    // Back on its post: the marker clears, which is what ends the episode.
    cut_off(&mut game, worker, post.x, post.y);
    game.tick();
    assert!(
        game.world.get::<Stranded>(worker).is_none(),
        "the marker cleared"
    );

    cut_off(&mut game, worker, 400, 400);
    game.tick();

    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "still one entry: {held:?}");
    assert_eq!(held[0].strikes, 2, "two episodes, two strikes: {held:?}");
}

/// A worker merely walking to its post has nothing to hold against the
/// ground it is walking over.
#[test]
fn a_worker_with_a_route_remembers_nothing() {
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (worker, node) = a_posted_worker(&mut game);
    let post = *game.world.get::<Position>(node).unwrap();
    cut_off(&mut game, worker, post.x + 2, post.y);

    for _ in 0..5 {
        game.tick();
    }

    assert!(
        stranded_tiles(&game, worker).is_empty(),
        "{:?}",
        memories_of(&game, worker)
    );
}

/// The marker is a cache of the tick's answer and stays unsaved, `since` and
/// all — the walk that produced it runs again on the next tick.
#[test]
fn the_stranding_marker_does_not_travel_through_a_save() {
    let dir = scratch_assets_dir("stranded_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (worker, _) = a_posted_worker(&mut game);
    let id = id_of(&game, worker);
    cut_off(&mut game, worker, 400, 400);
    game.tick();
    assert!(
        game.world.get::<Stranded>(worker).is_some(),
        "the fixture is stranded"
    );
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let who = by_id(&mut loaded, id);
    assert!(loaded.world.get::<Stranded>(who).is_none());
}

/// Deleting `assets/memories/` is a supported way to play, and phase 2
/// asserted that of `Game::remember`. This asserts it of the **triggers**,
/// which is where it can now lapse: a hook that minted a store, logged a
/// line, or wrote an entry before resolving the def would break the property
/// while `remember` itself stayed innocent.
///
/// All three hooks in one test on purpose — the property is about the feature
/// being *additive*, and one hook proving it says nothing about the other two.
#[test]
fn with_no_catalogue_loaded_every_trigger_is_inert() {
    let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (worker, _) = a_posted_worker(&mut game);
    let comrade = adopt(&mut game, "glitch", 5);
    join_party(&mut game, worker);
    join_party(&mut game, comrade);
    game.world
        .insert_resource(crate::memories::MemoryDb::default());

    // A mauling.
    let attacker = hostile_of(&mut game, "zero_day", 0);
    let max_hp = game.world.get::<Stats>(worker).unwrap().max_hp;
    one_landed_swing(&mut game, attacker, worker, max_hp);
    // A stranding.
    cut_off(&mut game, worker, 400, 400);
    game.tick();
    // A win against the odds.
    let player = game.player_entity();
    let hostile = hostile_of(&mut game, "glitch", 0);
    insert_battle(&mut game, player, vec![hostile]);
    game.world.resource_mut::<BattleState>().outmatched = true;
    win_the_fight(&mut game);

    assert!(
        memories_of(&game, worker).is_empty(),
        "{:?}",
        memories_of(&game, worker)
    );
    assert!(memories_of(&game, comrade).is_empty());
    assert_eq!(game.morale(worker), 0.0);
}

// ---------------------------------------------------------------------------
// Phase 4: what the screen draws.
//
// `Game::memory_report` is the one derivation the page is built from, the way
// `Game::gear_detail` is for the gear inspect page. Every test here reads the
// report and nothing reads the store behind it — a renderer that reached past
// the report for one figure is how four screens came to rebuild `copy_bonus`
// by hand.
// ---------------------------------------------------------------------------

/// A program with a store, on a game whose catalogue is the shipped one.
fn a_program_with_memories(game: &mut Game) -> Entity {
    adopt(game, "scrapper", 4)
}

/// Both fields have never had a reader before this screen. A def carrying a
/// name and a blurb that nothing draws is content the census already refuses
/// to let ship empty, held to a page that never says it.
#[test]
fn a_row_carries_the_defs_name_and_blurb() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    implant(&mut game, program, "hard_won", MemorySubject::Nothing);

    let rows = game.memory_report(program);

    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].name, "Won against the odds");
    assert_eq!(rows[0].blurb, "We had no business walking away from that.");
}

/// **Magnitude, never signed value** — `evict`'s rule, mirrored rather than
/// described. A signed sort files every grudge below every fondness, which
/// puts the deepest scar a program carries at the bottom of the page it is
/// most often opened to read.
///
/// The two are built so a signed sort inverts them: the grudge is the larger
/// magnitude and the smaller number.
#[test]
fn rows_are_ordered_by_magnitude_and_not_by_sign() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    // `hard_won` is +5.0, `mauled_by` -8.0, both at one undecayed strike.
    implant(&mut game, program, "hard_won", MemorySubject::Nothing);
    implant(
        &mut game,
        program,
        "mauled_by",
        MemorySubject::Species("glitch".to_string()),
    );

    let rows = game.memory_report(program);

    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(
        rows[0].name, "Mauled by",
        "the strongest thing it holds leads the page, whatever its sign: {rows:?}"
    );
    assert!(rows[0].intensity < 0.0, "{rows:?}");
    assert!(rows[1].intensity > 0.0, "{rows:?}");
}

/// A memory of an event rather than of a thing has nothing to name, and the
/// row says so by carrying no subject at all rather than by carrying an empty
/// string the renderer would have to test for.
#[test]
fn a_memory_about_nothing_renders_no_subject() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    implant(&mut game, program, "hard_won", MemorySubject::Nothing);

    assert_eq!(game.memory_report(program)[0].subject, None);
}

/// A species is named by its display name and never by its id: `zero_day` is
/// what a file is called, not what the player has ever seen the thing called.
#[test]
fn a_species_subject_renders_its_display_name() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    let wanted = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .get("zero_day")
        .expect("a shipped species")
        .name
        .clone();
    implant(
        &mut game,
        program,
        "mauled_by",
        MemorySubject::Species("zero_day".to_string()),
    );

    let subject = game.memory_report(program)[0].subject.clone();

    assert_eq!(subject.as_deref(), Some(wanted.as_str()));
    assert_ne!(
        subject.as_deref(),
        Some("zero_day"),
        "the id is a filename, not a name"
    );
}

/// The remembered name, resolved at the write, is what the row draws — and
/// the case it exists for is a subject that is **gone**. A live lookup
/// answers nothing here and the row would name an id or a blank.
#[test]
fn a_destroyed_programs_name_still_reaches_the_row() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = adopt(&mut game, "scrapper", 4);
    let subject = adopt(&mut game, "glitch", 5);
    let name = game.creature_label(subject);
    let id = id_of(&game, subject);

    assert_eq!(
        game.remember(holder, "bonded_in_battle", MemorySubject::Program(id)),
        Remembered::Written
    );
    game.dissolve_tamed_program(subject);

    let rows = game.memory_report(holder);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        rows[0].subject.as_deref(),
        Some(name.as_str()),
        "the screen still has to say who it was"
    );
}

/// Age reaches the row **in words, banded against the def's own half-life**
/// — the game has never shown the player a tick, and a raw count here would
/// be the first. Reinforcement is what makes a memory new again, so the
/// phrase and the intensity move together off the one field.
///
/// The bands are walked rather than sampled at one point: a test that only
/// checked "just now" would pass against a function that never says anything
/// else.
#[test]
fn age_is_banded_against_the_defs_own_half_life() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    set_tick(&mut game, 1_000);
    game.remember(program, "hard_won", MemorySubject::Nothing);
    // `hard_won` ships a 5,000-tick half-life.
    let half_life = 5_000;

    let phrase_at = |game: &mut Game, elapsed: u64| {
        set_tick(game, 1_000 + elapsed);
        game.memory_report(program)[0].age.clone()
    };

    assert_eq!(phrase_at(&mut game, 0), "just now");
    assert_eq!(phrase_at(&mut game, half_life / 4), "recently");
    assert_eq!(phrase_at(&mut game, half_life), "a while ago");
    assert_eq!(phrase_at(&mut game, half_life * 3), "long ago");

    // Reinforcing on the far side of that makes it new again.
    game.remember(program, "hard_won", MemorySubject::Nothing);
    assert_eq!(game.memory_report(program)[0].age, "just now");
}

/// The band is a *ratio*, so two defs of different half-lives at the same
/// elapsed ticks say different things. An absolute threshold would call a
/// scar and a bad shift equally old at the same moment, which is what makes
/// the yardstick the def's own.
#[test]
fn two_defs_of_different_half_lives_age_differently_at_one_moment() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    set_tick(&mut game, 1_000);
    // `stranded_at` halves in 3,000 ticks, `mauled_by` in 6,000.
    game.remember(
        program,
        "stranded_at",
        MemorySubject::BaseTile { x: 1, y: 1 },
    );
    game.remember(
        program,
        "mauled_by",
        MemorySubject::Species("glitch".to_string()),
    );

    set_tick(&mut game, 1_000 + 2_000);
    let rows = game.memory_report(program);
    let phrase = |name: &str| {
        rows.iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} is on the page: {rows:?}"))
            .age
            .clone()
    };

    assert_eq!(phrase("Left stranded here"), "a while ago");
    assert_eq!(
        phrase("Mauled by"),
        "recently",
        "the same 2,000 ticks against a longer half-life is not as old"
    );
}

/// The row is a projection of `Memory::intensity` and not a second copy of
/// the formula — the doc-comment-claiming-to-mirror trap this repo has been
/// bitten by four times. Decayed, so a test that ignored the clock would have
/// to quote the undecayed valence instead.
#[test]
fn a_rows_intensity_is_the_one_formula_decayed() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    set_tick(&mut game, 1_000);
    game.remember(program, "hard_won", MemorySubject::Nothing);
    // One shipped half-life for `hard_won`, so the row is worth exactly half.
    set_tick(&mut game, 6_000);

    let held = memories_of(&game, program);
    let def = game
        .world
        .resource::<crate::memories::MemoryDb>()
        .get(&MemoryId::from("hard_won"))
        .expect("a shipped def")
        .clone();
    let wanted = held[0].intensity(&def, 6_000);

    let rows = game.memory_report(program);
    assert!(
        (rows[0].intensity - wanted).abs() < 1e-5,
        "row {} against the formula's {wanted}",
        rows[0].intensity
    );
    assert!(
        (rows[0].intensity - def.valence / 2.0).abs() < 1e-5,
        "and one half-life in, that is half the valence: {}",
        rows[0].intensity
    );
}

/// `morale`'s asymmetry at the report end: a hostile, a structure or the
/// player has no store, and asking is a real answer rather than a panic.
#[test]
fn a_body_with_no_store_reports_no_rows() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 6, 6).unwrap();

    assert!(game.memory_report(wild).is_empty());
    assert!(game.memory_report(game.player_entity()).is_empty());
}

/// The deleting-`assets/memories/` property at the screen: the store holds
/// what it held, and the page draws nothing because nothing can be weighed.
#[test]
fn an_empty_database_reports_no_rows() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    implant(&mut game, program, "hard_won", MemorySubject::Nothing);
    implant(
        &mut game,
        program,
        "mauled_by",
        MemorySubject::Species("glitch".to_string()),
    );
    game.world
        .insert_resource(crate::memories::MemoryDb::default());

    assert_eq!(memories_of(&game, program).len(), 2, "the store is intact");
    assert!(game.memory_report(program).is_empty());
}

/// **A read-only screen may not rewrite the roster it is drawing.** The
/// faded entry below is one `evict` would drop at the next formation, and
/// what makes this test more than a tautology is that the report has to
/// *skip* it without removing it — otherwise what a program remembers would
/// depend on whether anybody looked.
#[test]
fn reading_the_report_evicts_nothing() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = a_program_with_memories(&mut game);
    game.remember(program, "hard_won", MemorySubject::Nothing);
    // Far enough past `hard_won`'s half-life that it is under
    // `MEMORY_FORGET_THRESHOLD` and would not survive another formation.
    set_tick(&mut game, 100_000);

    let before = memories_of(&game, program).len();
    let rows = game.memory_report(program);
    let after = memories_of(&game, program).len();

    assert_eq!(before, 1);
    assert_eq!(after, 1, "the page is a reader, not a sweep");
    assert_eq!(rows.len(), 1, "a faded memory is still one it holds");
}

// ---------------------------------------------------------------------
// Phase 5: the one hook
// ---------------------------------------------------------------------

/// A base with `n` idle programs and a Home to lay the parking ring around,
/// returned in the order `park_idle_staff` will index them.
///
/// Nothing is queued and nothing is deployed but the Home, so every body
/// here stays idle for as long as the test runs — which is the only state
/// the parking rejection is ever read in.
fn a_base_with_idle_staff(game: &mut Game, n: usize) -> (Position, Vec<Entity>) {
    place_home(game);
    let mut staff: Vec<Entity> = (0..n).map(|_| spawn_tamed(game, 10, 3)).collect();
    staff.sort();
    let home_entity = find_home(game).expect("the fixture just placed one");
    let home = *game.world.get::<Position>(home_entity).unwrap();
    (home, staff)
}

/// Where the body at `index` would be parked on the tick that is about to
/// run — the clock has not moved yet, and `park_idle_staff` reads it as it
/// stands.
fn next_park_tile(game: &Game, home: Position, index: usize) -> Position {
    park_tile(home, index, game.current_tick())
}

/// **The hook.** A program that was left stranded on a tile does not get
/// parked back onto it.
///
/// The grudge is implanted at full strength against the very tile the ring
/// is about to offer, so the only thing standing between the body and that
/// tile is the rejection.
#[test]
fn a_program_is_not_parked_on_a_tile_it_holds_a_grudge_against() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, staff) = a_base_with_idle_staff(&mut game, 1);
    let shunned = next_park_tile(&game, home, 0);
    implant(
        &mut game,
        staff[0],
        "stranded_at",
        MemorySubject::BaseTile {
            x: shunned.x,
            y: shunned.y,
        },
    );

    game.tick();

    let pos = *game.world.get::<Position>(staff[0]).unwrap();
    assert_ne!(
        (pos.x, pos.y),
        (shunned.x, shunned.y),
        "the corner it was left standing in is the one corner it will not take"
    );
}

/// The control the test above is worthless without: with no grudge the same
/// fixture parks the same body on the same tile. A rejection that fired on
/// every candidate — or a `park_idle_staff` broken outright — passes the
/// first test and fails this one.
#[test]
fn a_program_with_no_grudge_is_parked_on_that_same_tile() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, staff) = a_base_with_idle_staff(&mut game, 1);
    let candidate = next_park_tile(&game, home, 0);

    game.tick();

    let pos = *game.world.get::<Position>(staff[0]).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (candidate.x, candidate.y),
        "precondition for the sibling test: this is the tile the ring offers"
    );
}

/// A grudge against *somewhere else* is not a grudge against here. The
/// subject is compared, not merely counted — a hook that read `morale`
/// instead of `opinion_of` would keep the body off every tile in the base
/// the moment anything bad ever happened to it.
#[test]
fn a_grudge_against_another_tile_does_not_move_a_program() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, staff) = a_base_with_idle_staff(&mut game, 1);
    let candidate = next_park_tile(&game, home, 0);
    implant(
        &mut game,
        staff[0],
        "stranded_at",
        MemorySubject::BaseTile {
            x: candidate.x + 40,
            y: candidate.y + 40,
        },
    );

    game.tick();

    let pos = *game.world.get::<Position>(staff[0]).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (candidate.x, candidate.y),
        "an avoided tile is one tile, not the whole base"
    );
}

/// **It is a threshold and not a flag.** The same memory, faded past
/// `MEMORY_AVOIDANCE_THRESHOLD`, no longer keeps the body away — which is
/// the whole of what makes this a grudge that heals rather than a tile
/// blacklisted for the run.
///
/// **The candidate must not move out from under the grudge while it
/// fades**, or this is a second copy of the wrong-tile test above. The ring
/// returns to the same tile every `IDLE_STAFF_STEP_TICKS * 8 *
/// IDLE_STAFF_RING_TILES` ticks, so the clock is advanced by a whole number
/// of those periods, and the precondition below says so out loud — a
/// retuned ring or step must fail this test rather than quietly hollow it
/// out. That is not hypothetical: it *was* hollow, and a mutation swapping
/// the threshold for a bare `< 0.0` caught it passing.
#[test]
fn a_faded_grudge_stops_keeping_a_program_away() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, staff) = a_base_with_idle_staff(&mut game, 1);
    let candidate = next_park_tile(&game, home, 0);
    let subject = MemorySubject::BaseTile {
        x: candidate.x,
        y: candidate.y,
    };
    implant(&mut game, staff[0], "stranded_at", subject.clone());

    // Two half-lives, rounded up to a whole ring period. Read off the def
    // rather than hardcoded: the number that matters is how far the grudge
    // has decayed, and that is authored in the `.ron`.
    let half_life = game
        .world
        .resource::<crate::memories::MemoryDb>()
        .get(&MemoryId::from("stranded_at"))
        .expect("the fixture assets ship it")
        .half_life;
    let period =
        crate::tuning::IDLE_STAFF_STEP_TICKS * 8 * crate::tuning::IDLE_STAFF_RING_TILES as u64;
    let elapsed = period * (2 * half_life / period + 1);
    let faded = game.current_tick() + elapsed;
    set_tick(&mut game, faded);

    assert_eq!(
        next_park_tile(&game, home, 0),
        candidate,
        "precondition: the ring is offering the tile the grudge is about"
    );
    let opinion = game.opinion_of(staff[0], &subject);
    assert!(
        opinion > crate::tuning::MEMORY_AVOIDANCE_THRESHOLD && opinion < 0.0,
        "precondition: faded under the threshold but still a live grudge, got {opinion}"
    );

    game.tick();

    let pos = *game.world.get::<Position>(staff[0]).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (candidate.x, candidate.y),
        "a memory this light is not worth walking around"
    );
}

/// The deleting-`assets/memories/` property at the hook: the store holds
/// what it held, nothing can be weighed, and parking behaves exactly as it
/// did before the feature shipped.
#[test]
fn an_empty_database_leaves_the_parking_hook_inert() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (home, staff) = a_base_with_idle_staff(&mut game, 1);
    let candidate = next_park_tile(&game, home, 0);
    implant(
        &mut game,
        staff[0],
        "stranded_at",
        MemorySubject::BaseTile {
            x: candidate.x,
            y: candidate.y,
        },
    );
    game.world
        .insert_resource(crate::memories::MemoryDb::default());

    game.tick();

    assert_eq!(
        memories_of(&game, staff[0]).len(),
        1,
        "the store is intact — nothing purges it"
    );
    let pos = *game.world.get::<Position>(staff[0]).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (candidate.x, candidate.y),
        "with no catalogue there is no grudge to read"
    );
}

// ---------------------------------------------------------------------------
// Morale at work: what a program remembers about its job.
//
// `MemorySubject::Structure` and `MemorySubject::Activity` shipped with the
// substrate and with nothing writing either. These are their writers.
// ---------------------------------------------------------------------------

/// A sweep that the machine *survives* is remembered by whoever was posted at
/// it.
///
/// Damage-not-destroy on purpose: `damage_structure` already looked at the
/// workers on the destroyed branch, to clear their cronjobs, so a test on that
/// branch alone would pass against a trigger written in only half the places
/// it belongs. Being caught at a machine that survived is the same memory to
/// the body standing there.
#[test]
fn a_sweep_that_spares_the_machine_is_still_remembered_by_the_worker_on_it() {
    let mut game = Game::new(220, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 20, max_hp: 30 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    game.damage_structure(structure, 10, "Mining Node");

    assert!(
        game.world.get::<Structure>(structure).is_some(),
        "the fixture is about the branch where the machine lives"
    );
    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].def, MemoryId::from("swept_here"));
    assert_eq!(
        held[0].subject,
        MemorySubject::Structure("mining_node".to_string()),
        "the subject is the machine's kind, so the memory outlives the machine"
    );
}

/// The same on the branch that despawns the structure — and the point is the
/// ordering, not the coverage.
///
/// The subject is read off `Structure::kind`, which the destroyed branch is
/// about to take with it. Formed after the despawn there is no kind to name;
/// formed before, the memory outlives the thing it is about, which is the
/// whole reason the variant carries a `StructureId` rather than an `Entity`.
#[test]
fn a_sweep_that_destroys_the_machine_is_remembered_after_it_is_gone() {
    let mut game = Game::new(221, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 10, max_hp: 30 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    game.damage_structure(structure, 10, "Mining Node");

    assert!(
        game.world.get::<Structure>(structure).is_none(),
        "the fixture is about the branch where the machine dies"
    );
    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(
        held[0].subject,
        MemorySubject::Structure("mining_node".to_string())
    );
}

/// A sweep on an empty machine forms nothing, which is what says the trigger
/// is about the body and not about the building.
#[test]
fn a_sweep_on_an_unstaffed_machine_is_remembered_by_nobody() {
    let mut game = Game::new(222, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 20, max_hp: 30 },
        ))
        .id();
    let bystander = spawn_tamed(&mut game, 10, 3);

    game.damage_structure(structure, 10, "Mining Node");

    assert!(
        memories_of(&game, bystander).is_empty(),
        "a program with no posting at the machine has nothing to remember"
    );
}

/// A body posted at a machine remembers the machine — but only when the
/// period comes round.
///
/// Both halves in one test on purpose. The formation half alone passes
/// against a pass with no period at all, which is precisely the shape
/// `note_strandings` rules out: a per-tick write saturates `strike_cap` in
/// three ticks and makes `strikes` mean nothing.
#[test]
fn a_posted_program_settles_in_on_the_period_and_not_before() {
    let mut game = Game::new(230, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    set_tick(&mut game, MEMORY_POSTING_PERIOD + 1);
    game.note_postings();
    assert!(
        memories_of(&game, worker).is_empty(),
        "a tick that is not the period must write nothing, or strikes count ticks"
    );

    set_tick(&mut game, MEMORY_POSTING_PERIOD * 2);
    game.note_postings();

    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].def, MemoryId::from("settled_in"));
    assert_eq!(
        held[0].subject,
        MemorySubject::Structure("mining_node".to_string())
    );
}

/// The same posting at a machine that is backed up forms the opposite memory,
/// on the same subject.
///
/// Asserted as `jammed_here` **and not** `settled_in`, since a pass that
/// wrote both would satisfy a test that only looked for the one it expected —
/// and the two are meant to net against each other over a run, not accumulate
/// together in a single pass.
#[test]
fn a_posting_at_a_clogged_machine_is_remembered_against_it() {
    let mut game = Game::new(231, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            MachineStatus::Clogged,
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    set_tick(&mut game, MEMORY_POSTING_PERIOD);
    game.note_postings();

    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].def, MemoryId::from("jammed_here"));
}

/// A digger remembers the *work* and has no machine to remember instead.
///
/// The absence is the point, and it is structural rather than a check: a
/// `DigSite` is the one `Task` target that is not a `Structure`, so the
/// machine arm has nothing to read. A fixture whose dig target happened to
/// carry a `Structure` would pass the first assertion and hide that.
#[test]
fn a_digger_remembers_the_work_and_has_no_machine_to_remember() {
    let mut game = Game::new(232, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let site = game.world.spawn(Position { x: 2, y: 2 }).id();
    let digger = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(digger).insert(Task {
        kind: TaskKind::Excavate,
        target: site,
        progress: 0,
        required: 4,
    });

    set_tick(&mut game, MEMORY_POSTING_PERIOD);
    game.note_postings();

    let held = memories_of(&game, digger);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].def, MemoryId::from("cutting_rock"));
    assert_eq!(
        held[0].subject,
        MemorySubject::Activity(TaskKind::Excavate),
        "the memory follows the program, not the hole"
    );
}

/// An unposted program remembers nothing from this pass at all.
#[test]
fn an_idle_program_takes_nothing_from_the_posting_pass() {
    let mut game = Game::new(233, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let idle = spawn_tamed(&mut game, 10, 3);

    set_tick(&mut game, MEMORY_POSTING_PERIOD);
    game.note_postings();

    assert!(memories_of(&game, idle).is_empty());
}

/// Reinforcement is what a stretch of service *is*, and it stops at the def's
/// `strike_cap` rather than running away.
///
/// This is the property the period exists to make meaningful: without it the
/// same number of strikes would be bought by a few ticks of standing still.
#[test]
fn service_compounds_to_the_cap_and_no_further() {
    let mut game = Game::new(234, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    for period in 1..=12 {
        set_tick(&mut game, MEMORY_POSTING_PERIOD * period);
        game.note_postings();
    }

    let held = memories_of(&game, worker);
    assert_eq!(held.len(), 1, "reinforcement must not add a second entry");
    let cap = game
        .world
        .resource::<crate::memories::MemoryDb>()
        .get(&MemoryId::from("settled_in"))
        .expect("the def ships")
        .strike_cap;
    assert_eq!(held[0].strikes, cap, "twelve stretches, capped at {cap}");
}

/// Morale is what the whole pass is for, and this is the end-to-end claim:
/// service at a machine leaves the body measurably better disposed than one
/// that has never worked.
#[test]
fn service_moves_morale_off_zero() {
    let mut game = Game::new(235, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    let bystander = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    set_tick(&mut game, MEMORY_POSTING_PERIOD);
    game.note_postings();

    assert!(
        game.morale(worker) > 0.0,
        "a settled worker should read better than neutral"
    );
    assert_eq!(
        game.morale(bystander),
        0.0,
        "and a program that has done nothing is still exactly at the baseline"
    );
}
