//! Entity memories: the identity a memory is *about*, and what one is worth.
//!
//! `Entity` is written to the save nowhere, because entity ids are not
//! stable across a round trip. `ProgramId` is what a memory names instead,
//! minted at `Game::roster_parts` — the one barrier all four doors into the
//! roster pass through.

use super::support::*;
use crate::components::{Memories, Memory, MemorySubject, ProgramId};
use crate::game::memories::Remembered;
use crate::memories::{MemoryDef, MemoryId, MemorySubjectKind};
use crate::resources::GameClock;
use crate::tuning::MEMORY_CAP_PER_PROGRAM;
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
