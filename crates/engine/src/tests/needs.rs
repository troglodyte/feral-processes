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

// ---------------------------------------------------------------------------
// Leaving a post: the gate, and the hysteresis it exists for.
// ---------------------------------------------------------------------------

use crate::components::OffShift;
use crate::game::base::offshift::Amenities;

/// A Home, a Defrag Bay beside it, and `n` staff standing on laid floor.
fn a_base_with_an_amenity(game: &mut Game, n: usize) -> Vec<Entity> {
    stand_in_base(game);
    place_home(game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(game, "defrag_bay", 2, 0).expect("a Defrag Bay is buildable from the start");
    let mut staff: Vec<Entity> = (0..n).map(|_| spawn_tamed(game, 10, 3)).collect();
    staff.sort();
    for (i, &worker) in staff.iter().enumerate() {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = -2 - i as i32;
        pos.y = 0;
    }
    staff
}

/// Drops `who`'s reserve to `value` without running a thousand ticks.
fn set_reserve(game: &mut Game, who: Entity, need: &NeedId, value: f32) {
    let mut store = game.world.get_mut::<Needs>(who).unwrap();
    store.set(need, value);
}

fn threshold(game: &Game, need: &NeedId) -> (f32, f32) {
    let def = game.world.resource::<NeedDb>().get(need).expect("shipped");
    (def.critical, def.content)
}

fn run_the_gate(game: &mut Game, staff: &[Entity]) {
    let amenities = game.amenities();
    game.update_off_shift(staff, &amenities);
}

fn off_shift(game: &Game, who: Entity) -> Option<NeedId> {
    game.world.get::<OffShift>(who).map(|o| o.need.clone())
}

/// Below `critical` with something in the base that answers it: off shift.
#[test]
fn a_critical_reserve_with_an_amenity_takes_a_program_off_shift() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    run_the_gate(&mut game, &staff);

    assert_eq!(off_shift(&game, staff[0]), Some(coherence()));
}

/// **Nothing services it** — the second clause of the gate. A program with
/// nowhere to go stays on shift and acts out instead.
#[test]
fn a_critical_reserve_with_no_amenity_leaves_the_program_on_shift() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let staff = vec![spawn_tamed(&mut game, 10, 3)];
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    run_the_gate(&mut game, &staff);

    assert_eq!(off_shift(&game, staff[0]), None);
}

/// **The latch** — the third clause. A need that has already reported itself
/// stalled does not keep pulling the body off its post every beat; and the
/// latch clears when the reserve recovers.
#[test]
fn a_latched_need_does_not_pull_a_body_off_until_it_recovers() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let (critical, _) = threshold(&game, &coherence());
    game.world
        .get_mut::<Needs>(staff[0])
        .unwrap()
        .latch(&coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    run_the_gate(&mut game, &staff);
    assert_eq!(
        off_shift(&game, staff[0]),
        None,
        "a latched need is one the base has already been told about"
    );

    // Recovered, then run down again: the latch cleared on the way up.
    set_reserve(&mut game, staff[0], &coherence(), critical + 5.0);
    run_the_gate(&mut game, &staff);
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);

    assert_eq!(off_shift(&game, staff[0]), Some(coherence()));
}

/// **The hysteresis, and the whole reason `OffShift` is stored at all.** A
/// program pulled off at `critical` does not go back the tick it crosses
/// `critical` again — it stays until `content`.
#[test]
fn a_program_stays_off_shift_until_it_is_content_not_merely_above_critical() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let (critical, content) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    assert_eq!(off_shift(&game, staff[0]), Some(coherence()), "off it goes");

    set_reserve(&mut game, staff[0], &coherence(), critical + 1.0);
    run_the_gate(&mut game, &staff);
    assert_eq!(
        off_shift(&game, staff[0]),
        Some(coherence()),
        "one point over the line it left at is not being fixed"
    );

    set_reserve(&mut game, staff[0], &coherence(), content);
    run_the_gate(&mut game, &staff);
    assert_eq!(off_shift(&game, staff[0]), None, "content is what ends it");
}

/// Two amenities exactly equidistant must resolve to the same one whatever
/// order the world hands its structures back in — `min_by_key` takes the
/// first of several equal minima, which is where bevy's iteration order leaks.
#[test]
fn equidistant_amenities_resolve_to_one_tile_whichever_order_they_are_built_in() {
    let db = |sites: Vec<(&str, i32, i32)>| {
        let game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let kinds: Vec<(crate::structures::StructureId, Position)> = sites
            .into_iter()
            .map(|(kind, x, y)| (kind.to_string(), Position { x, y }))
            .collect();
        let amenities = Amenities::build(
            kinds.iter().map(|(k, p)| (k, p)),
            game.world.resource::<StructureDb>(),
        );
        amenities.nearest(&coherence(), Position { x: 0, y: 0 })
    };

    let forwards = db(vec![("defrag_bay", -3, 0), ("defrag_bay", 3, 0)]);
    let backwards = db(vec![("defrag_bay", 3, 0), ("defrag_bay", -3, 0)]);

    assert_eq!(forwards.map(|(p, _, _)| (p.x, p.y)), Some((-3, 0)));
    assert_eq!(forwards.map(|(p, ..)| p.x), backwards.map(|(p, ..)| p.x));
}

/// The amenity demolished out from under an off-shift program ends the
/// errand: there is nowhere to walk to any more.
#[test]
fn losing_the_amenity_drops_off_shift() {
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    assert_eq!(off_shift(&game, staff[0]), Some(coherence()));

    let bay = find_structure_by_kind(&mut game, "defrag_bay").expect("the fixture built one");
    game.world.entity_mut(bay).despawn();
    run_the_gate(&mut game, &staff);

    assert_eq!(off_shift(&game, staff[0]), None);
}

/// The one stored piece of this feature has to survive a reload, or a program
/// mid-errand at `critical + 1` is judged content and sent back to work.
#[test]
fn off_shift_survives_a_save_and_load() {
    let dir = scratch_assets_dir("needs_offshift_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(56, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored = loaded
        .world
        .query::<&OffShift>()
        .iter(&loaded.world)
        .map(|o| o.need.clone())
        .collect::<Vec<_>>();
    assert_eq!(restored, vec![coherence()]);
}

// ---------------------------------------------------------------------------
// The walk, and the servicing at the end of it.
// ---------------------------------------------------------------------------

use crate::base_grid::BaseGrid;
use crate::resources::GameClock;

/// Winds the clock rather than ticking it: a thousand ticks run every
/// background system, and what these tests are about is the drift beat.
fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

/// Runs one beat of the drift with the gate already applied.
fn drift(game: &mut Game, staff: &[Entity]) {
    let amenities = game.amenities();
    game.update_off_shift(staff, &amenities);
    game.drift_idle_staff_for_test(staff, &amenities);
}

/// An off-shift program walks to its amenity rather than wandering.
#[test]
fn an_off_shift_program_steps_toward_its_amenity() {
    let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    let before = *game.world.get::<Position>(staff[0]).unwrap();
    let gap = |p: Position| (p.x - site.x).abs().max((p.y - site.y).abs());

    // Enough beats to cross the pocket; the walk converges and then holds.
    for _ in 0..40 {
        let next = game.current_tick() + crate::tuning::IDLE_STAFF_STEP_TICKS;
        set_tick(&mut game, next);
        drift(&mut game, &staff);
    }

    let after = *game.world.get::<Position>(staff[0]).unwrap();
    assert!(
        gap(after) < gap(before),
        "it closed on the Defrag Bay at {site:?}: {before:?} -> {after:?}"
    );
    assert!(
        crate::game::base::offshift::in_reach(after, site, 0),
        "and it arrived: {after:?} against {site:?}"
    );
}

/// Standing in reach, the reserve **rises** — the drain is not the only thing
/// writing it.
#[test]
fn standing_at_an_amenity_refills_the_reserve() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    {
        let mut pos = game.world.get_mut::<Position>(staff[0]).unwrap();
        pos.x = site.x - 1;
        pos.y = site.y;
    }
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    let before = reserve(&game, staff[0]);

    game.tick();

    assert!(
        reserve(&game, staff[0]) > before,
        "the Defrag Bay's rate beats the drain: {before} -> {}",
        reserve(&game, staff[0])
    );
}

/// A program with no errand wanders exactly as it did before — the off-shift
/// walk is a fall-through, not a replacement.
#[test]
fn a_program_with_no_errand_still_wanders() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let before = *game.world.get::<Position>(staff[0]).unwrap();

    let mut moved = false;
    for _ in 0..20 {
        let next = game.current_tick() + crate::tuning::IDLE_STAFF_STEP_TICKS;
        set_tick(&mut game, next);
        drift(&mut game, &staff);
        let now = *game.world.get::<Position>(staff[0]).unwrap();
        moved |= (now.x, now.y) != (before.x, before.y);
    }
    assert!(moved, "a content program still drifts around the base");
    assert!(
        off_shift(&game, staff[0]).is_none(),
        "and it never took an errand it had no reason for"
    );
}

/// **The one place a route is judged.** An amenity walled off from the body
/// that needs it costs the errand once and latches the need, and the gate does
/// not hand it straight back on the next beat.
#[test]
fn an_unreachable_amenity_gives_the_errand_up_once_and_stays_given_up() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    // **Two islands.** One cell with no standing room and two cells with
    // standing room and no route are different faults, and a fix for one is
    // not a fix for the other: the Bay keeps its own floor to be stood beside
    // and the rock between it and the body is what refuses the walk.
    {
        let mut grid = game.world.resource_mut::<BaseGrid>();
        for y in -6..=6 {
            for x in -6..=6 {
                if x >= site.x - 1 {
                    continue;
                }
                if x == site.x - 2 {
                    grid.revert(x, y);
                }
            }
        }
        grid.lay_floor(site.x - 1, site.y);
    }
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    drift(&mut game, &staff);
    assert_eq!(
        off_shift(&game, staff[0]),
        None,
        "the walk is what discovers there is no route, and it gives the post up"
    );
    assert!(
        game.world
            .get::<Needs>(staff[0])
            .unwrap()
            .is_latched(&coherence()),
        "and latches, so it is not re-offered every beat"
    );

    drift(&mut game, &staff);
    assert_eq!(
        off_shift(&game, staff[0]),
        None,
        "the next beat must not hand it back — that flicker is what the latch is for"
    );
}

// ---------------------------------------------------------------------------
// Standing down: who the scheduler will still give a job to.
// ---------------------------------------------------------------------------

use crate::components::Carrying;
use crate::game::base::work_orders::WorkOrder;

/// A Home, a mining node, a Defrag Bay and one hired body, with an order
/// standing that wants that body on the node.
fn a_base_with_an_order_and_an_amenity(game: &mut Game) -> (Entity, Entity) {
    stand_in_base(game);
    place_home(game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    let node = spawn_machine_at(game, "mining_node", 2, 0);
    place_now(game, "defrag_bay", 0, 2).expect("a Defrag Bay is buildable from the start");
    let worker = spawn_tamed(game, 10, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50))
        .unwrap();
    (node, worker)
}

/// The point of the whole feature: a program with an errand of its own is not
/// handed a job.
#[test]
fn an_off_shift_program_is_not_posted() {
    let mut game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (node, worker) = a_base_with_an_order_and_an_amenity(&mut game);
    game.tick();
    assert_eq!(
        game.world.get::<Task>(worker).map(|t| t.target),
        Some(node),
        "on shift it takes the post"
    );

    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, worker, &coherence(), critical - 1.0);
    game.tick();

    assert_eq!(off_shift(&game, worker), Some(coherence()));
    assert!(
        game.world.get::<Task>(worker).is_none(),
        "and it is off the node while it sees to itself"
    );
}

/// **The one exception**, and it is the existing never-free-a-`Carrying`-
/// holder rule rather than a second one: freeing a loaded body destroys the
/// goods.
#[test]
fn an_off_shift_program_holding_a_load_stays_posted_until_it_delivers() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, worker) = a_base_with_an_order_and_an_amenity(&mut game);
    game.tick();
    game.world.entity_mut(worker).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 3,
    });

    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, worker, &coherence(), critical - 1.0);
    game.tick();

    assert!(
        game.world.get::<Task>(worker).is_some(),
        "a loaded body keeps its post"
    );
    assert_eq!(
        game.world.get::<Carrying>(worker).map(|c| c.qty),
        Some(3),
        "and its load is not destroyed"
    );
}

/// The work-order header's shortfall *grows* while bodies are off shift.
/// That is the intended readout: the base is short of hands, and why is on
/// the manifest.
#[test]
fn labour_demand_counts_only_the_bodies_still_on_shift() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, worker) = a_base_with_an_order_and_an_amenity(&mut game);
    game.tick();
    let before = game.labour_demand().staff;

    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, worker, &coherence(), critical - 1.0);
    game.tick();

    assert_eq!(
        game.labour_demand().staff,
        before - 1,
        "the one body off shift is one body the base does not have"
    );
}

/// A base whose whole crew has walked off posts nobody, and does not panic
/// doing it.
#[test]
fn a_base_whose_whole_crew_is_off_shift_posts_nobody() {
    let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, worker) = a_base_with_an_order_and_an_amenity(&mut game);
    let second = spawn_tamed(&mut game, 10, 3);
    game.tick();

    let (critical, _) = threshold(&game, &coherence());
    for who in [worker, second] {
        set_reserve(&mut game, who, &coherence(), critical - 1.0);
    }
    game.tick();

    assert_eq!(game.labour_demand().staff, 0);
    for who in [worker, second] {
        assert!(game.world.get::<Task>(who).is_none());
    }
}
