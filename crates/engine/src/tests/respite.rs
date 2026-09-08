//! Where a program in a bad mood takes itself, and what standing there is
//! worth.
//!
//! `OffShift`'s errand on the morale meter. A need has a reserve to refill;
//! morale has none, so what the errand buys is a *memory* — a fondness for
//! the amenity, written on a period the way `note_postings` writes service,
//! and folded back through `Game::morale` by the one door that already
//! exists.

use super::support::*;
use crate::components::{Carrying, Disgruntled, Grievance, Memories, Position, Task};
use crate::tuning::MEMORY_POSTING_PERIOD;
use crate::*;

/// A Home, a Sandbox well off to one side, and `n` staff standing on laid
/// floor beside the Home.
///
/// The amenity is placed at a distance so that "walked toward it" and
/// "arrived" are different assertions — a fixture that stands the body on
/// top of the amenity cannot tell a walk from a no-op.
fn a_base_with_an_amenity(game: &mut Game, n: usize) -> Vec<Entity> {
    stand_in_base(game);
    place_home(game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(game, "defrag_bay", 4, 0).expect("a Defrag Bay is buildable from the start");
    park_staff(game, n)
}

/// The same base with nothing that services any need — a Depot instead.
fn a_base_with_no_amenity(game: &mut Game, n: usize) -> Vec<Entity> {
    stand_in_base(game);
    place_home(game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(game, "depot", 4, 0).expect("a Depot is buildable from the start");
    park_staff(game, n)
}

fn park_staff(game: &mut Game, n: usize) -> Vec<Entity> {
    let mut staff: Vec<Entity> = (0..n).map(|_| spawn_tamed(game, 10, 3)).collect();
    staff.sort();
    for (i, &worker) in staff.iter().enumerate() {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = -2 - i as i32;
        pos.y = 0;
    }
    staff
}

/// Puts `who` genuinely on the mild rung, through the meter rather than by
/// hand.
///
/// **A hand-inserted marker does not survive the beat.** `update_disgruntled`
/// runs at the top of `schedule_base_labour` and removes one from a body
/// whose morale is above `MORALE_RECOVERED_AT` — which a freshly spawned
/// program's is, at zero. So the fixture has to move the sum the gate reads.
///
/// Two strikes of `ran_down` is -12 against a `MORALE_SULKS_AT` of -8 and a
/// `MORALE_DOWNS_TOOLS_AT` of -50: on the ladder, on the mild rung, and
/// close enough to the recovery line at -6 that the errand can actually
/// carry it back over.
fn sulk(game: &mut Game, who: Entity) {
    for _ in 0..2 {
        game.remember(who, "ran_down", crate::components::MemorySubject::Nothing);
    }
    assert!(
        game.morale(who) <= crate::tuning::MORALE_SULKS_AT,
        "the fixture must put the body on the ladder, got {}",
        game.morale(who)
    );
    game.update_disgruntled(&[who]);
    assert_eq!(
        game.world.get::<Disgruntled>(who).map(|d| d.grievance),
        Some(Grievance::Sulking),
        "and on the mild rung, not a deeper one"
    );
}

fn at(game: &Game, who: Entity) -> Position {
    *game.world.get::<Position>(who).expect("a body has a tile")
}

fn fondness(game: &Game, who: Entity) -> u32 {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.def.as_str() == "unwound_at")
                .map(|m| m.strikes)
                .sum()
        })
        .unwrap_or(0)
}

/// The gate: a program in a bad mood, with somewhere to go, stops taking
/// postings. This is the whole of "sulking pulls a body off a post".
#[test]
fn a_disgruntled_program_with_an_amenity_leaves_the_posting_pool() {
    // A base with a **workable** machine, because the assertion is that a
    // body which would otherwise be posted is not: on a base with nothing
    // to do, "holds no `Task`" is true of everybody and the test measures
    // nothing.
    let mut game = base_with_a_built_node(70, 0.5, 0.5);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "defrag_bay", 4, 0).expect("a Defrag Bay is buildable from the start");
    let node = first_structure(&mut game, "mining_node");
    game.set_standing_job(node, true, false)
        .expect("a Mining Node takes a standing work job");
    let worker = game.base_staff()[0];

    game.schedule_base_labour();
    assert!(
        game.world.get::<Task>(worker).is_some(),
        "the control: a contented body at a workable machine is posted"
    );

    sulk(&mut game, worker);
    game.schedule_base_labour();

    assert!(
        game.world.get::<Task>(worker).is_none(),
        "and a body on respite is taken off it"
    );
}

/// The other half of the gate, and what keeps `refuses_post` reachable: a
/// base with nowhere to unwind keeps its sulking programs in the pool, where
/// the grudge-against-a-machine rung still governs where they will work.
#[test]
fn a_disgruntled_program_with_no_amenity_stays_in_the_posting_pool() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_no_amenity(&mut game, 1);
    sulk(&mut game, staff[0]);

    let amenities = game.amenities_for_test();
    assert!(
        !game.on_respite(staff[0], &amenities),
        "nothing services a need here, so there is no errand to take"
    );
}

/// **The two morale exclusions are not one question**, and this is the whole
/// of the ladder keeping two rungs. Written after collapsing them into one
/// predicate silently put a program at -50 back on the line at a base with no
/// amenity — which the disposition suite caught, but only because it happened
/// to use a base with none.
#[test]
fn the_severe_rung_leaves_the_pool_with_or_without_somewhere_to_go() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_no_amenity(&mut game, 2);
    let (sulking, quit) = (staff[0], staff[1]);
    sulk(&mut game, sulking);
    game.world.entity_mut(quit).insert(Disgruntled {
        grievance: Grievance::DownedTools,
        stranded: false,
    });

    let amenities = game.amenities_for_test();

    assert!(
        game.is_on_shift(sulking, &amenities),
        "a sulking body with nowhere to unwind still works — refuses_post is \
         what governs where, and collapsing this leaves that rung unreachable"
    );
    assert!(
        !game.is_on_shift(quit, &amenities),
        "a body at -50 does not work, and whether the base has an amenity has \
         nothing to do with it"
    );
}

/// It walks. The destination is derived per beat, exactly as an off-shift
/// body's is, so nothing about the route is stored.
#[test]
fn a_disgruntled_program_walks_toward_the_amenity() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    sulk(&mut game, staff[0]);
    let start = at(&game, staff[0]);
    assert!(
        (start.x - 4).abs() > 1,
        "the fixture must start the body away from the Bay, got {start:?}"
    );

    for _ in 0..30 {
        game.schedule_base_labour();
        // Held on the rung by hand: this test is about the walk, and a body
        // that recovers on the way stops walking. The memory is re-struck
        // rather than the marker re-inserted, so the gate stays honest.
        game.remember(
            staff[0],
            "ran_down",
            crate::components::MemorySubject::Nothing,
        );
    }

    let now = at(&game, staff[0]);
    // **Arrival, not "closer".** The wander alone moves an idle body a tile
    // a beat, so a test that only asks whether the gap shrank passes without
    // the errand existing at all.
    assert!(
        (now.x - 4).abs() <= 1 && now.y.abs() <= 1,
        "it must reach the Bay at (4, 0): started {start:?}, reached {now:?}"
    );
}

/// Standing there is worth something, and it is worth it **on a period** —
/// `note_postings`' rule, so `strikes` measures time spent rather than
/// saturating in three ticks.
#[test]
fn standing_at_the_amenity_writes_the_fondness_on_a_period() {
    let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    sulk(&mut game, staff[0]);

    assert_eq!(fondness(&game, staff[0]), 0, "nothing yet");
    for _ in 0..MEMORY_POSTING_PERIOD * 2 {
        game.tick();
    }

    assert!(
        fondness(&game, staff[0]) > 0,
        "a program that walked to the Bay and stood there must come away fond of it"
    );
}

/// End to end, and the reason the memory is the mechanism: the fondness is
/// what lifts the meter back over `MORALE_RECOVERED_AT`, through
/// `Game::morale`'s existing fold and no second meter.
#[test]
fn the_errand_lifts_morale_back_over_the_recovery_line() {
    let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let worker = staff[0];
    sulk(&mut game, worker);
    let before = game.morale(worker);

    for _ in 0..MEMORY_POSTING_PERIOD * 3 {
        game.tick();
    }

    assert!(
        game.morale(worker) > before,
        "the errand must move the meter: {before} -> {}",
        game.morale(worker)
    );
    assert!(
        game.world.get::<Disgruntled>(worker).is_none(),
        "and far enough to clear the marker"
    );
}

/// A `Carrying` holder stays on shift even in a bad mood — the existing
/// never-free-a-`Carrying`-holder rule, not a second one. Freeing a loaded
/// body destroys the goods.
#[test]
fn a_disgruntled_program_holding_a_load_stays_on_shift() {
    let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    sulk(&mut game, staff[0]);
    game.world.entity_mut(staff[0]).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 1,
    });

    let amenities = game.amenities_for_test();
    assert!(
        game.is_on_shift(staff[0], &amenities),
        "a loaded body delivers before it goes anywhere to feel better"
    );
}

/// An amenity that exists and cannot be walked to puts the body **back** in
/// the pool rather than leaving it out of it forever, and latches so the
/// walk is not re-attempted every beat — `step_off_shift`'s rule.
#[test]
fn an_unreachable_amenity_strands_the_errand_and_returns_the_body() {
    let mut game = Game::new(76, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let worker = staff[0];
    sulk(&mut game, worker);
    // Off the pocket entirely: no route exists from here to anything.
    let mut pos = game.world.get_mut::<Position>(worker).unwrap();
    pos.x = 900;
    pos.y = 900;

    game.schedule_base_labour();

    assert!(
        game.world
            .get::<Disgruntled>(worker)
            .is_some_and(|d| d.stranded),
        "the walk is the one place a route is judged, and it latches"
    );
    let amenities = game.amenities_for_test();
    assert!(
        !game.on_respite(worker, &amenities),
        "a stranded body is back in the pool, where refuses_post governs it"
    );
}

/// **No grudge on the stranded branch.** `fray` writes one when a need goes
/// unanswered; doing it here would deepen the exact hole that sent the body
/// out, which is a loop with no floor.
#[test]
fn a_stranded_respite_writes_no_grudge() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let worker = staff[0];
    sulk(&mut game, worker);
    let before = game.morale(worker);
    let mut pos = game.world.get_mut::<Position>(worker).unwrap();
    pos.x = 900;
    pos.y = 900;

    game.schedule_base_labour();

    assert_eq!(
        game.morale(worker),
        before,
        "a body the base cannot reach must not be made to feel worse for it"
    );
}

/// The latch survives a reload, `OffShift`'s reason: dropped, a stranded
/// body would spend a fresh Dijkstra every beat for the rest of the run.
#[test]
fn the_stranded_latch_survives_a_save_round_trip() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    game.world.entity_mut(staff[0]).insert(Disgruntled {
        grievance: Grievance::Sulking,
        stranded: true,
    });

    let dir = scratch_assets_dir("respite_latch");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.ron");
    game.save(&path).expect("save");
    let reloaded = Game::load(&path, &test_assets_dir()).expect("load");

    let marker = reloaded
        .world
        .iter_entities()
        .filter_map(|e| e.get::<Disgruntled>())
        .next()
        .expect("the marker is saved");
    assert!(marker.stranded, "and so is the latch on it");
}
