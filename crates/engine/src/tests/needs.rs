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
    let bays = game.repair_bays();
    game.update_off_shift(staff, &amenities);
    game.drift_idle_staff_for_test(staff, &amenities, &bays);
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

// ---------------------------------------------------------------------------
// Acting out: what a program does when the gate refuses it.
// ---------------------------------------------------------------------------

use crate::components::{Memories, MemorySubject};

/// Every line the log holds, **repeats expanded**. `message_history`
/// condenses an unbroken run into one row with a count, so a bare entry count
/// would read a line said five times as a line said once — which is exactly
/// the thing these tests are about.
fn base_lines(game: &Game) -> Vec<String> {
    game.message_history(500)
        .into_iter()
        .flat_map(|row| std::iter::repeat_n(row.text, row.repeats.max(1)))
        .collect()
}

fn frayed_entries(game: &Game, who: Entity) -> Vec<MemorySubject> {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.def.as_str() == "frayed_here")
                .map(|m| m.subject.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Nothing in the base answers the need: said **once**, however many beats
/// run — and **no grudge**, which is the half of this that is a design
/// decision rather than a latch.
///
/// A base with no Defrag Bay standing has no answer to Coherence, and the
/// player may not have researched one, may not have the materials, and has
/// never been told they want one. A program that holds *the base* to account
/// for that is blaming it for a building that was never an option, and the
/// grudge it writes is a real one — enough on its own to drag a whole base
/// toward sulking with nothing the player could have done differently.
///
/// The base earns a grudge when it had an answer and failed to deliver it,
/// which is the `unreachable` branch and is what
/// `an_unreachable_amenity_says_something_different_from_no_amenity` holds.
/// The line is still said either way: the player is still told.
#[test]
fn a_need_nothing_services_is_announced_but_earns_no_grudge() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let staff = vec![spawn_tamed(&mut game, 10, 3)];
    let (critical, _) = threshold(&game, &coherence());

    for _ in 0..5 {
        set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
        run_the_gate(&mut game, &staff);
    }

    let said: Vec<String> = base_lines(&game)
        .into_iter()
        .filter(|line| line.contains("nothing in the base"))
        .collect();
    assert_eq!(said.len(), 1, "once, not once a beat: {said:?}");
    assert!(
        frayed_entries(&game, staff[0]).is_empty(),
        "a base with no amenity at all has done nothing to be held against it"
    );
}

/// The amenity exists but cannot be walked to. **A different sentence**,
/// because it leaves the player a different errand.
#[test]
fn an_unreachable_amenity_says_something_different_from_no_amenity() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    {
        let mut grid = game.world.resource_mut::<BaseGrid>();
        for y in -6..=6 {
            grid.revert(site.x - 2, y);
        }
        grid.lay_floor(site.x - 1, site.y);
    }
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    drift(&mut game, &staff);
    drift(&mut game, &staff);

    let said: Vec<String> = base_lines(&game)
        .into_iter()
        .filter(|line| line.contains("can't find a way"))
        .collect();
    assert_eq!(said.len(), 1, "once: {said:?}");
    assert!(
        !base_lines(&game)
            .iter()
            .any(|l| l.contains("nothing in the base")),
        "an amenity that exists is not an amenity that is missing"
    );
    assert_eq!(frayed_entries(&game, staff[0]).len(), 1);
}

/// The latch clears on the way back up, so a need that runs down a second
/// time complains a second time — `set_machine_status`' rule.
#[test]
fn a_need_that_recovers_and_runs_down_again_is_announced_again() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let staff = vec![spawn_tamed(&mut game, 10, 3)];
    let (critical, _) = threshold(&game, &coherence());

    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    set_reserve(&mut game, staff[0], &coherence(), critical + 5.0);
    run_the_gate(&mut game, &staff);
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);

    let said = base_lines(&game)
        .iter()
        .filter(|line| line.contains("nothing in the base"))
        .count();
    assert_eq!(said, 2, "twice, because it went wrong twice");
}

/// The latch is not in the save: a reload should say it again.
#[test]
fn a_reload_re_announces_a_stalled_need() {
    let dir = scratch_assets_dir("needs_latch_reload");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let staff = vec![spawn_tamed(&mut game, 10, 3)];
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let reloaded = loaded.base_staff();
    let amenities = loaded.amenities();
    loaded.update_off_shift(&reloaded, &amenities);

    assert!(
        base_lines(&loaded)
            .iter()
            .any(|line| line.contains("nothing in the base")),
        "the latch is not saved, so the complaint stands again"
    );
}

// ---------------------------------------------------------------------------
// Where programs notice each other.
// ---------------------------------------------------------------------------

use crate::components::ProgramId;

/// A Sandbox and `n` staff standing in reach of it, all run down to critical.
fn a_sandbox_with_company(game: &mut Game, n: usize) -> (Position, Vec<Entity>) {
    stand_in_base(game);
    place_home(game);
    give(game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(game, "sandbox", 2, 0).expect("a Sandbox is buildable from the start");
    let box_entity = find_structure_by_kind(game, "sandbox").expect("just built");
    let site = *game.world.get::<Position>(box_entity).unwrap();
    let mut staff: Vec<Entity> = (0..n).map(|_| spawn_tamed(game, 10, 3)).collect();
    staff.sort();
    // Every body inside the Sandbox's own reach — the point of the fixture is
    // company, and a body one tile too far is a different test.
    let around = [(0, 1), (-1, 0), (0, -1), (1, 1)];
    for (i, &worker) in staff.iter().enumerate() {
        let (dx, dy) = around[i % around.len()];
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = site.x + dx;
        pos.y = site.y + dy;
    }
    (site, staff)
}

fn slack() -> NeedId {
    NeedId::from("slack")
}

fn idled_strikes(game: &Game, who: Entity, about: Entity) -> u32 {
    let id = *game.world.get::<ProgramId>(about).unwrap();
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| {
                    m.def.as_str() == "idled_with" && m.subject == MemorySubject::Program(id)
                })
                .map(|m| m.strikes)
                .sum()
        })
        .unwrap_or(0)
}

/// Two programs finishing at one Sandbox each hold **one** memory of the
/// other. `strikes == 1`, not merely "an entry exists".
#[test]
fn two_programs_servicing_together_each_remember_the_other_once() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, staff) = a_sandbox_with_company(&mut game, 2);
    let (critical, content) = threshold(&game, &slack());
    for &who in &staff {
        set_reserve(&mut game, who, &slack(), critical - 1.0);
    }
    run_the_gate(&mut game, &staff);
    for &who in &staff {
        set_reserve(&mut game, who, &slack(), content);
    }
    run_the_gate(&mut game, &staff);

    assert_eq!(idled_strikes(&game, staff[0], staff[1]), 1);
    assert_eq!(idled_strikes(&game, staff[1], staff[0]), 1);
}

/// A program that idled alone has nobody to remember.
#[test]
fn a_lone_program_finishing_writes_nothing() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, staff) = a_sandbox_with_company(&mut game, 1);
    let (critical, content) = threshold(&game, &slack());
    set_reserve(&mut game, staff[0], &slack(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    set_reserve(&mut game, staff[0], &slack(), content);
    run_the_gate(&mut game, &staff);

    assert!(
        game.world
            .get::<Memories>(staff[0])
            .unwrap()
            .0
            .iter()
            .all(|m| m.def.as_str() != "idled_with")
    );
}

/// **Once per stretch, never per tick.** A long stretch of shared servicing
/// before either finishes is still one strike.
#[test]
fn a_long_stretch_of_shared_servicing_is_still_one_strike() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (_, staff) = a_sandbox_with_company(&mut game, 2);
    let (critical, content) = threshold(&game, &slack());
    for &who in &staff {
        set_reserve(&mut game, who, &slack(), critical - 1.0);
    }
    for _ in 0..100 {
        run_the_gate(&mut game, &staff);
    }
    for &who in &staff {
        set_reserve(&mut game, who, &slack(), content);
    }
    run_the_gate(&mut game, &staff);

    assert_eq!(idled_strikes(&game, staff[0], staff[1]), 1);
}

/// A program standing somewhere else entirely is not company.
#[test]
fn a_program_out_of_reach_is_not_named() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (site, staff) = a_sandbox_with_company(&mut game, 2);
    {
        let mut pos = game.world.get_mut::<Position>(staff[1]).unwrap();
        pos.x = site.x - 4;
        pos.y = site.y;
    }
    let (critical, content) = threshold(&game, &slack());
    set_reserve(&mut game, staff[0], &slack(), critical - 1.0);
    run_the_gate(&mut game, &staff);
    set_reserve(&mut game, staff[0], &slack(), content);
    run_the_gate(&mut game, &staff);

    assert_eq!(idled_strikes(&game, staff[0], staff[1]), 0);
}

// ---------------------------------------------------------------------------
// Teeth: what a run-down program costs the base.
// ---------------------------------------------------------------------------

use crate::needs::strain;
use crate::systems::{mining_success_chance, need_shift};
use crate::tuning::{DEFAULT_BASE_INT, NEED_STRAIN_MAX_SHIFT};

/// Full reserves are the baseline and contribute **exactly** nothing, so the
/// shipped extraction rates mean what they have always meant.
#[test]
fn a_program_with_everything_it_needs_has_no_strain() {
    let game = Game::new(100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<NeedDb>();
    let mut needs = Needs::default();
    needs.seed_missing(db);

    assert_eq!(strain(&needs, db), 0.0);
}

/// Deleting `assets/needs/` contributes nothing either, and by arithmetic
/// rather than by a branch.
#[test]
fn an_empty_catalogue_has_no_strain() {
    let mut needs = Needs::default();
    needs.set(&coherence(), 0.0);

    assert_eq!(strain(&needs, &NeedDb::default()), 0.0);
}

/// **The cap is asserted on `need_shift` directly.** Read off the finished
/// chance it cannot be told from the outer `clamp(0.0, 1.0)` swallowing the
/// overshoot, which is a different job.
#[test]
fn the_strain_shift_saturates_at_its_own_cap() {
    assert!((need_shift(-10_000.0) + NEED_STRAIN_MAX_SHIFT).abs() < 1e-9);
    assert!((need_shift(10_000.0) - NEED_STRAIN_MAX_SHIFT).abs() < 1e-9);
}

/// An entry naming a def no file defines is skipped, not counted as
/// zero-weighted noise — the same rule every `Memories` reader follows, and
/// the property the whole empty-catalogue guarantee rests on.
#[test]
fn an_unresolvable_need_is_skipped_rather_than_counted() {
    let game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<NeedDb>();
    let mut needs = Needs::default();
    needs.seed_missing(db);
    needs.set(&NeedId::from("a_mod_removed_this"), 0.0);

    assert_eq!(strain(&needs, db), 0.0);
}

/// A run-down program extracts less reliably, and a program with what it
/// needs extracts at exactly today's shipped rate.
#[test]
fn a_drained_program_extracts_less_reliably_and_a_full_one_is_unchanged() {
    let full = mining_success_chance(4, 0, DEFAULT_BASE_INT, 0.0, 0.0);
    let today = mining_success_chance(4, 0, DEFAULT_BASE_INT, 0.0, 0.0);
    let drained = mining_success_chance(4, 0, DEFAULT_BASE_INT, 0.0, -12.0);

    assert_eq!(full, today, "zero strain is the shipped rate, untouched");
    assert!(
        drained < full,
        "a program running on empty fumbles more: {drained} against {full}"
    );
}

/// `Game::need_strain` is a caller of the fold, not a second copy of it.
#[test]
fn the_games_strain_reader_agrees_with_the_fold() {
    let mut game = Game::new(102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    set_reserve(&mut game, worker, &coherence(), 0.0);

    let direct = {
        let needs = game.world.get::<Needs>(worker).unwrap();
        strain(needs, game.world.resource::<NeedDb>())
    };
    assert_eq!(game.need_strain(worker), direct);
    assert!(direct < 0.0, "an empty reserve is a drag, not a bonus");
}

// ---------------------------------------------------------------------------
// The readout.
// ---------------------------------------------------------------------------

use crate::views::need_band;

/// One row per loaded def, **in id order** — never by value, or the labels
/// move under the eye reading them.
#[test]
fn the_manifest_lists_every_need_in_id_order() {
    let mut game = Game::new(110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.tick();

    let rows = game.need_rows(worker);

    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        vec!["Coherence", "Slack"],
        "id order, whatever the values are"
    );
    assert!(rows.iter().all(|r| r.servicing.is_none()));
    assert!(rows.iter().all(|r| r.band == "steady"));
}

/// With `assets/needs/` deleted the section is absent entirely, not present
/// and empty.
#[test]
fn an_empty_catalogue_draws_no_rows_at_all() {
    let mut game = Game::new(111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.insert_resource(NeedDb::default());
    game.tick();

    assert!(game.need_rows(worker).is_empty());
}

/// The `servicing` verb appears only while the program is off shift for that
/// need, and it is what the examine line reads too.
#[test]
fn the_servicing_verb_shows_only_while_off_shift() {
    let mut game = Game::new(112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, 1);
    // One tick so the drain seeds every def — `set_reserve` only writes the
    // one it names, and a def with no entry has no row.
    game.tick();
    assert_eq!(game.program_errand_label(staff[0]), None, "on shift");
    assert_eq!(game.program_activity(staff[0]), "idle");

    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);
    run_the_gate(&mut game, &staff);

    let rows = game.need_rows(staff[0]);
    let coherence_row = rows.iter().find(|r| r.name == "Coherence").unwrap();
    assert_eq!(coherence_row.servicing.as_deref(), Some("Defragmenting"));
    assert_eq!(coherence_row.band, "critical");
    assert!(
        rows.iter()
            .find(|r| r.name == "Slack")
            .unwrap()
            .servicing
            .is_none()
    );
    assert_eq!(
        game.program_errand_label(staff[0]).as_deref(),
        Some("Defragmenting")
    );
    assert_eq!(
        game.program_activity(staff[0]),
        "defragmenting",
        "a body off shift is not idle"
    );
}

/// Words, never a number — there is no player-facing float in this game.
#[test]
fn the_bands_run_from_steady_to_critical() {
    assert_eq!(need_band(1.0), "steady");
    assert_eq!(need_band(0.5), "fraying");
    assert_eq!(need_band(0.3), "strained");
    assert_eq!(need_band(0.0), "critical");
}
