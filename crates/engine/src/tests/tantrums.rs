//! What a base does to the programs working it, and what one of them
//! eventually does back.
//!
//! Three features share this file because they are one chain: a need nothing
//! answers now moves morale, a young base is exempt from all of it, and a
//! program far enough into the hole rounds on a colleague.

use super::support::*;
use crate::components::{Memories, MemorySubject, Position, Structure};
use crate::needs::NeedId;
use crate::structures::{StructureDb, StructureId};
use crate::tuning::{BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES};
use crate::*;

fn coherence() -> NeedId {
    NeedId::from("coherence")
}

/// Every memory `who` holds under `def`, by subject.
fn entries(game: &Game, who: Entity, def: &str) -> Vec<MemorySubject> {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.def.as_str() == def)
                .map(|m| m.subject.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Stands the party in a base with `staff` programs and `structures`
/// buildings in it, so the grace gate can be driven from either side.
///
/// The filler is a Depot, which services no need — a base with an amenity in
/// it is a different fixture, and `fray`'s quiet branch is about a base that
/// has none.
fn a_base_of(game: &mut Game, staff: usize, structures: usize) -> Vec<Entity> {
    stand_in_base(game);
    if structures > 0 {
        place_home(game);
    }
    let def = game
        .world
        .resource::<StructureDb>()
        .get(&StructureId::from("depot"))
        .expect("a Depot ships")
        .clone();
    for i in 1..structures {
        game.spawn_structure(&def, -20 - i as i32, 20, None);
    }
    let mut bodies: Vec<Entity> = (0..staff).map(|_| spawn_tamed(game, 10, 3)).collect();
    bodies.sort();
    for (i, &worker) in bodies.iter().enumerate() {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = -2 - i as i32;
        pos.y = 0;
    }
    bodies
}

/// The counts the fixture is claiming, asserted rather than assumed — a
/// `place_home` that stops standing a structure would otherwise turn every
/// gate test below into a vacuous one.
fn structure_count(game: &Game) -> usize {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<Structure>())
        .count()
}

fn an_established_base(game: &mut Game, staff: usize) -> Vec<Entity> {
    let bodies = a_base_of(game, staff, BASE_ESTABLISHED_STRUCTURES);
    assert_eq!(structure_count(game), BASE_ESTABLISHED_STRUCTURES);
    bodies
}

// ---------------------------------------------------------------------------
// A need nothing answers is remembered
// ---------------------------------------------------------------------------

/// The quiet branch: the base has nothing that services this need at all.
/// The blame is withheld — `Nothing` as a subject holds no tile, machine or
/// colleague responsible — but the meter moves, which is what makes needs
/// reach the morale ladder at all.
#[test]
fn fray_with_no_amenity_writes_ran_down() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = an_established_base(&mut game, BASE_ESTABLISHED_STAFF);

    game.fray(staff[0], &coherence(), false);

    assert_eq!(
        entries(&game, staff[0], "ran_down"),
        vec![MemorySubject::Nothing],
        "a need the base never answered is still felt"
    );
    assert!(
        entries(&game, staff[0], "frayed_here").is_empty(),
        "and it is not the unreachable branch's grudge"
    );
}

/// The other branch is unchanged, and this test is why both are written: one
/// test over either passes against a function that writes the same memory
/// both times.
#[test]
fn fray_with_an_unreachable_amenity_still_writes_frayed_here() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = an_established_base(&mut game, BASE_ESTABLISHED_STAFF);

    game.fray(staff[0], &coherence(), true);

    assert_eq!(
        entries(&game, staff[0], "frayed_here").len(),
        1,
        "the amenity existed and could not be reached: the base earns it"
    );
    assert!(
        entries(&game, staff[0], "ran_down").is_empty(),
        "and not the blame-free one"
    );
}

// ---------------------------------------------------------------------------
// The grace period
// ---------------------------------------------------------------------------

/// Below both thresholds nothing counts against the base, on either branch.
#[test]
fn a_young_base_earns_no_need_grudges() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(&mut game, 2, 2);

    game.fray(staff[0], &coherence(), false);
    game.fray(staff[1], &coherence(), true);

    assert!(entries(&game, staff[0], "ran_down").is_empty());
    assert!(entries(&game, staff[1], "frayed_here").is_empty());
}

/// Plenty of buildings, not enough bodies. Wired `||` this passes and the
/// next one fails, which is the pair's whole point.
#[test]
fn a_base_short_of_staff_earns_no_need_grudges() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(
        &mut game,
        BASE_ESTABLISHED_STAFF - 1,
        BASE_ESTABLISHED_STRUCTURES + 2,
    );

    game.fray(staff[0], &coherence(), false);

    assert!(
        entries(&game, staff[0], "ran_down").is_empty(),
        "a sprawling base with nobody in it is not a pressure cooker"
    );
}

/// And the mirror.
#[test]
fn a_base_short_of_structures_earns_no_need_grudges() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(
        &mut game,
        BASE_ESTABLISHED_STAFF + 2,
        BASE_ESTABLISHED_STRUCTURES - 1,
    );

    game.fray(staff[0], &coherence(), false);

    assert!(
        entries(&game, staff[0], "ran_down").is_empty(),
        "twelve programs in a bare base have not had the chance yet"
    );
}

/// The control: a gate nothing can pass is a deleted feature.
#[test]
fn an_established_base_earns_need_grudges() {
    let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = an_established_base(&mut game, BASE_ESTABLISHED_STAFF);

    game.fray(staff[0], &coherence(), false);

    assert_eq!(entries(&game, staff[0], "ran_down").len(), 1);
}

/// The line is said either way. Only the blame is gated — the player must
/// still be told what their programs are short of, because that line is the
/// errand.
#[test]
fn a_young_base_still_says_the_line() {
    let mut game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(&mut game, 2, 2);

    game.fray(staff[0], &coherence(), false);

    assert!(
        game.message_history(50)
            .iter()
            .any(|row| row.text.contains("nothing in the base")),
        "the errand is still handed to the player"
    );
}

// ---------------------------------------------------------------------------
// The brawl
// ---------------------------------------------------------------------------

use crate::components::{Disgruntled, Downed, Grievance, ProgramId, Stats};
use crate::resources::{Brawl, Brawls, GameClock, MessageKind};
use crate::tuning::{
    BAY_ADMISSION_HP_FRACTION, TANTRUM_COOLDOWN_TICKS, TANTRUM_TICKS_MAX, TANTRUM_TICKS_MIN,
};

/// An established base whose staff are parked far apart, so a test says which
/// two are in reach of each other rather than inheriting it from the fixture.
fn a_scattered_base(game: &mut Game) -> Vec<Entity> {
    let staff = an_established_base(game, BASE_ESTABLISHED_STAFF);
    for (i, &who) in staff.iter().enumerate() {
        place_at(game, who, 40 * i as i32, 40);
    }
    staff
}

fn place_at(game: &mut Game, who: Entity, x: i32, y: i32) {
    let mut pos = game.world.get_mut::<Position>(who).unwrap();
    pos.x = x;
    pos.y = y;
}

fn set_hp(game: &mut Game, who: Entity, hp: i32, max_hp: i32) {
    let mut stats = game.world.get_mut::<Stats>(who).unwrap();
    stats.hp = hp;
    stats.max_hp = max_hp;
}

fn hp(game: &Game, who: Entity) -> i32 {
    game.world.get::<Stats>(who).unwrap().hp
}

/// Puts `who` on the fourth rung directly. The ladder that gets it there is
/// `disposition.rs`'s business; what this file tests is what being there
/// does.
fn lash_out(game: &mut Game, who: Entity) {
    game.world.entity_mut(who).insert(Disgruntled {
        grievance: Grievance::LashingOut,
        stranded: false,
    });
}

/// Stands a Repair Bay somewhere in the base, which is all
/// `admit_the_badly_hurt` asks of one.
fn stand_a_bay(game: &mut Game) {
    let def = game
        .world
        .resource::<StructureDb>()
        .get(&StructureId::from("repair_bay"))
        .expect("a Repair Bay ships")
        .clone();
    game.spawn_structure(&def, -30, -30, None);
}

/// One base beat's worth of the two steps this feature lives between.
fn beat(game: &mut Game, staff: &[Entity]) {
    let bays = game.repair_bays();
    game.run_tantrums(staff);
    game.admit_the_badly_hurt(staff, &bays);
}

fn open_brawls(game: &Game) -> usize {
    game.world.resource::<Brawls>().open.len()
}

fn lines(game: &Game) -> Vec<String> {
    game.message_history(500)
        .into_iter()
        .flat_map(|row| std::iter::repeat_n(row.text, row.repeats.max(1)))
        .collect()
}

/// Opens a fight by hand, which is what a test that is about the *exchange*
/// wants — the roll that would otherwise gate it is `open_brawls`' business
/// and is tested separately.
fn stage_a_brawl(game: &mut Game, aggressor: Entity, victim: Entity, ticks: u32) {
    game.world.resource_mut::<Brawls>().open.push(Brawl {
        aggressor,
        victim,
        ticks_left: ticks,
        dealt: 0,
        taken: 0,
    });
}

/// **The one this feature is sold on.** `Game::apply_damage` floors HP at
/// zero and reaching zero *is* a kill; nothing inside it refuses a lethal
/// blow, so the clamp before it is the whole guarantee.
#[test]
fn a_tantrum_never_kills() {
    let mut game = Game::new(100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    let (aggressor, victim) = (staff[0], staff[1]);
    place_at(&mut game, victim, 1, 40);
    set_hp(&mut game, aggressor, 100, 100);
    set_hp(&mut game, victim, 1, 100);

    stage_a_brawl(&mut game, aggressor, victim, TANTRUM_TICKS_MAX);
    for _ in 0..TANTRUM_TICKS_MAX + 2 {
        game.run_tantrums(&staff);
    }

    assert_eq!(
        hp(&game, victim),
        1,
        "a body on its last point of Integrity comes out of a full brawl \
         still standing"
    );
    assert!(
        hp(&game, aggressor) >= 1,
        "and so does the one that started it"
    );
}

/// **And the other half: it does put somebody in the bay**, at the *short*
/// end of the tick range. The long case passes for free, and a test written
/// against it would go green with a damage constant far too low to hold the
/// guarantee.
#[test]
fn a_short_brawl_still_fills_the_bay() {
    let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    stand_a_bay(&mut game);
    let (aggressor, victim) = (staff[0], staff[1]);
    place_at(&mut game, victim, 1, 40);
    for who in [aggressor, victim] {
        set_hp(&mut game, who, 100, 100);
    }

    stage_a_brawl(&mut game, aggressor, victim, TANTRUM_TICKS_MIN);
    for _ in 0..TANTRUM_TICKS_MIN {
        beat(&mut game, &staff);
    }

    assert!(
        [aggressor, victim]
            .iter()
            .any(|&who| game.world.get::<Downed>(who).is_some()),
        "four blows from full health must carry somebody under \
         {BAY_ADMISSION_HP_FRACTION}: {} and {} of 100",
        hp(&game, aggressor),
        hp(&game, victim)
    );
}

/// Both sides come away with something, and the grudge names who did it.
#[test]
fn a_brawl_writes_both_memories() {
    let mut game = Game::new(102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    let (aggressor, victim) = (staff[0], staff[1]);
    let id = *game.world.get::<ProgramId>(aggressor).unwrap();

    game.close_brawl(&Brawl {
        aggressor,
        victim,
        ticks_left: 0,
        dealt: 30,
        taken: 12,
    });

    assert_eq!(
        entries(&game, aggressor, "vented"),
        vec![MemorySubject::Nothing],
        "the relief is what keeps a ratcheted rung from meaning `fights forever`"
    );
    assert_eq!(
        entries(&game, victim, "turned_on_me"),
        vec![MemorySubject::Program(id)],
        "and the grudge names the program, not the entity"
    );
}

/// Every line the fight produced, and all of them base news.
#[test]
fn a_brawl_reports_what_each_side_did() {
    let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    let (aggressor, victim) = (staff[0], staff[1]);

    game.close_brawl(&Brawl {
        aggressor,
        victim,
        ticks_left: 0,
        dealt: 34,
        taken: 21,
    });

    let said = lines(&game);
    assert!(
        said.iter().any(|l| l.contains("34 damage")),
        "what the aggressor did: {said:?}"
    );
    assert!(
        said.iter().any(|l| l.contains("21 damage")),
        "and what came back: {said:?}"
    );
    assert!(
        said.iter().any(|l| l.contains("came out of it calmer")),
        "and how it left the two of them: {said:?}"
    );
    assert!(
        game.message_history(500)
            .iter()
            .filter(|row| row.text.contains("damage") || row.text.contains("calmer"))
            .all(|row| row.kind == MessageKind::Tantrum),
        "a scuffle between two staff is not a GC Entropy Sweep"
    );
}

/// A side that landed nothing gets no line, rather than one reading
/// `0 damage`. Reachable exactly where the non-lethal clamp bites: a body on
/// its last point of Integrity is a body every blow aimed at it is clamped
/// to nothing.
#[test]
fn a_one_sided_brawl_omits_the_reply_line() {
    let mut game = Game::new(104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    let (aggressor, victim) = (staff[0], staff[1]);
    place_at(&mut game, victim, 1, 40);
    set_hp(&mut game, aggressor, 1, 100);
    set_hp(&mut game, victim, 100, 100);

    stage_a_brawl(&mut game, aggressor, victim, TANTRUM_TICKS_MIN);
    for _ in 0..TANTRUM_TICKS_MIN + 1 {
        game.run_tantrums(&staff);
    }

    let said = lines(&game);
    assert!(
        said.iter().any(|l| l.contains("in a tantrum")),
        "the fight still happened: {said:?}"
    );
    assert!(
        !said.iter().any(|l| l.contains("fought back")),
        "nothing came back, so nothing is reported as coming back: {said:?}"
    );
}

/// No target in reach means no tantrum, and that is what keeps this feature
/// free of pathing.
#[test]
fn a_program_with_nobody_in_reach_starts_no_brawl() {
    let mut game = Game::new(105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    lash_out(&mut game, staff[0]);

    for _ in 0..300 {
        game.run_tantrums(&staff);
    }

    assert_eq!(open_brawls(&game), 0);
    assert!(!lines(&game).iter().any(|l| l.contains("rounds on")));
}

/// The control, so the three negatives above are not all passing against a
/// feature that never fires: with somebody in reach, one does open.
#[test]
fn a_program_deep_in_the_hole_rounds_on_somebody() {
    let mut game = Game::new(106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    place_at(&mut game, staff[1], 1, 40);
    lash_out(&mut game, staff[0]);
    reseed_rng(&mut game, 7);

    let opened = (0..500).any(|_| {
        game.run_tantrums(&staff);
        open_brawls(&game) > 0
    });

    assert!(opened, "a rung nothing can reach is a deleted feature");
    let said = lines(&game);
    assert!(
        said.iter().any(|l| l.contains("rounds on")),
        "and the alert precedes the damage: {said:?}"
    );
}

/// A program in the bay neither starts a fight nor is picked for one.
#[test]
fn a_downed_program_is_neither_aggressor_nor_victim() {
    let mut game = Game::new(107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    place_at(&mut game, staff[1], 1, 40);
    place_at(&mut game, staff[3], 41, 40);
    // A body already in the bay, standing next to a healthy colleague.
    lash_out(&mut game, staff[0]);
    game.world.entity_mut(staff[0]).insert(Downed);
    // And a healthy body on the rung whose only neighbour is in the bay.
    lash_out(&mut game, staff[2]);
    game.world.entity_mut(staff[3]).insert(Downed);
    reseed_rng(&mut game, 7);

    for _ in 0..500 {
        game.run_tantrums(&staff);
    }

    assert_eq!(
        open_brawls(&game),
        0,
        "neither end of a fight may be downed"
    );
}

/// Coming out of one buys quiet, and the quiet ends.
#[test]
fn the_cooldown_holds() {
    let mut game = Game::new(108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    place_at(&mut game, staff[1], 1, 40);
    for who in [staff[0], staff[1]] {
        set_hp(&mut game, who, 100, 100);
    }
    lash_out(&mut game, staff[0]);

    stage_a_brawl(&mut game, staff[0], staff[1], 1);
    game.run_tantrums(&staff);
    let closed_at = game.current_tick();
    assert_eq!(open_brawls(&game), 0, "the staged fight is over");

    reseed_rng(&mut game, 7);
    game.world.resource_mut::<GameClock>().tick = closed_at + TANTRUM_COOLDOWN_TICKS - 1;
    for _ in 0..500 {
        game.run_tantrums(&staff);
    }
    assert_eq!(
        open_brawls(&game),
        0,
        "one beat short of the cooldown, nothing starts"
    );

    reseed_rng(&mut game, 7);
    game.world.resource_mut::<GameClock>().tick = closed_at + TANTRUM_COOLDOWN_TICKS;
    let opened = (0..500).any(|_| {
        game.run_tantrums(&staff);
        open_brawls(&game) > 0
    });
    assert!(opened, "and once it is up, the rung is live again");
}

/// The grace gate covers the fight as well as the memories: a young base is
/// out of this feature entirely, however deep one of its programs has gone.
#[test]
fn no_tantrum_opens_below_the_grace_thresholds() {
    let mut game = Game::new(109, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_of(&mut game, 2, 2);
    place_at(&mut game, staff[1], 1, 0);
    lash_out(&mut game, staff[0]);
    reseed_rng(&mut game, 7);

    for _ in 0..500 {
        game.run_tantrums(&staff);
    }

    assert_eq!(open_brawls(&game), 0);
    assert!(!lines(&game).iter().any(|l| l.contains("rounds on")));
}

/// A beat with nobody on the rung must not touch `GameRng` at all, or this
/// feature silently shifts the seeded stream and reads later as unrelated
/// tests flaking. `Game::run_routes`' predation test is the pattern.
#[test]
fn a_tantrum_draws_no_rng_when_nobody_is_lashing_out() {
    let mut game = Game::new(110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_scattered_base(&mut game);
    place_at(&mut game, staff[1], 1, 40);

    fn peek(g: &mut Game) -> u64 {
        use rand::RngExt;
        g.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    }

    reseed_rng(&mut game, 55);
    let without = peek(&mut game);

    reseed_rng(&mut game, 55);
    game.run_tantrums(&staff);
    let with = peek(&mut game);

    assert_eq!(
        without, with,
        "nobody is on the rung, so the beat must not draw"
    );
}
