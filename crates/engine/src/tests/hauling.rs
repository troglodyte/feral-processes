//! Programs that walk: taking a post, carrying a full buffer to a depot,
//! and coming back.

use super::support::*;
use crate::components::Squeezing;
use crate::components::{Memories, MemorySubject};
use crate::game::base::hauling::{Step, Stride, blocked_tiles, step_to_post, stride};
use crate::memories::MemoryId;
use crate::tuning::STARTING_POCKET_RADIUS;
use crate::*;

/// A Home on the player's own tile — walkable by definition — plus enough
/// Core Fragments to deploy anything these fixtures need. The Home's slab
/// makes the whole build box walkable, so nothing here depends on the seed's
/// terrain.
fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    // Standing in it, not out on the zone surface looking at where it used
    // to be: hauling is entirely a base subject, and deploying, posting and
    // collecting are all `Game::require_base` now.
    stand_in_base(&mut game);
    game
}

/// Deploys `kind` at the party's base cell plus `(dx, dy)` and returns it.
/// `place_structure` reports only success, so the entity is found by the
/// cell it must now be standing on.
fn deploy(game: &mut Game, kind: &str, dx: i32, dy: i32) -> Entity {
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    place_now(game, kind, dx, dy).unwrap();
    let (x, y) = (px + dx, py + dy);
    let mut query = game.world.query::<(Entity, &Position, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, p, _)| p.x == x && p.y == y)
        .map(|(e, ..)| e)
        .expect("the structure was just deployed")
}

/// A worker with enough Integrity to outlast the ambient GC Entropy Sweeps
/// a base takes while these fixtures run.
///
/// Not belt-and-braces: a posted program defends its machine for
/// `RAID_DEFENDER_DAMAGE` a sweep, these tests tick for up to 400, and a
/// 10 HP worker dies to two unlucky rolls — which surfaces as a hauling
/// assertion failing on a `Position` that is suddenly gone, hundreds of
/// lines from the sweep that caused it. The HP is incidental to every test
/// here, so it is set where it cannot be read as part of the fixture's
/// meaning.
fn hauler(game: &mut Game) -> Entity {
    spawn_tamed(game, 500, 3)
}

fn move_to(game: &mut Game, entity: Entity, x: i32, y: i32) {
    let mut pos = game.world.get_mut::<Position>(entity).unwrap();
    pos.x = x;
    pos.y = y;
}

fn fill_output(game: &mut Game, structure: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(structure).unwrap();
    stock.output.insert(ItemId::from(item), qty);
}

fn capacity_of(game: &Game, structure: Entity) -> u32 {
    game.world.get::<Stock>(structure).unwrap().capacity
}

/// Fills `structure`'s output to the brim, so the next completed cycle finds
/// nowhere to put its payout.
fn fill_to_capacity(game: &mut Game, structure: Entity, item: &str) {
    let cap = capacity_of(game, structure);
    fill_output(game, structure, item, cap);
}

/// Ticks until `done`, or `limit` ticks, whichever comes first. Every wait
/// in this module is bounded — a loop that never ends reads as a hang rather
/// than a failure.
fn tick_until(game: &mut Game, limit: u32, done: impl Fn(&Game) -> bool) {
    for _ in 0..limit {
        if done(game) {
            return;
        }
        game.tick();
    }
}

#[test]
fn a_clogged_machine_sends_its_worker_off_with_a_bounded_load() {
    let mut game = base(1);
    let node = deploy(&mut game, "mining_node", 1, 0);
    // A consumer beside it, so this is the *clogged* errand rather than the
    // one a machine with nothing downstream starts every cycle — see
    // `a_machine_with_nothing_downstream_delivers_as_it_produces`. Standing
    // there unstaffed it pulls nothing, so the buffer stays where the
    // fixture put it — and it takes the order below to make it a consumer at
    // all, which is
    // `a_neighbour_nothing_has_been_ordered_from_is_not_an_attached_building`.
    let lathe = spawn_machine_at(&mut game, "lathe", 2, 0);
    // Somewhere to take a load: with no depot there is no errand, which is
    // `with_no_depot_a_clogged_machine_just_stays_clogged` below.
    deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    // The Lathe is backed up too, so `can_progress` refuses it and the
    // scheduler leaves the one body on the node. `assign_cronjob` no longer
    // pins a worker in place — every program the player owns is base staff,
    // so the scheduler owns the posting, and a Lathe it *can* staff is a
    // Lathe that pulls, which would drain the very clog this test is about.
    // Backed up it is still a consumer, because `consumer_beside` asks the
    // recipe rather than whether the neighbour is currently pulling.
    let lathe_cap = capacity_of(&game, lathe);
    fill_output(&mut game, lathe, "blank_substrate", lathe_cap);
    // Ordered past what that fill holds, or the order would be *satisfied*
    // by it — a filled queue drops the Lathe out of `queue_needs`, which
    // makes it a bystander rather than an attached building and turns this
    // into the deliver-as-you-produce errand instead of the clogged one.
    game.queue_work_order(WorkOrder::batch(
        ItemId::from("blank_substrate"),
        lathe_cap + 5,
    ))
    .unwrap();

    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);

    tick_until(&mut game, 40, |g| g.world.get::<Carrying>(worker).is_some());

    let carrying = game
        .world
        .get::<Carrying>(worker)
        .expect("a clogged machine's worker should pick up a load");
    assert_eq!(carrying.qty, tuning::HAUL_CARRY_CAPACITY);
    assert_eq!(carrying.item, ItemId::from(ids::CORE_FRAGMENT));

    let task = game.world.get::<Task>(worker).unwrap();
    assert_eq!(
        task.progress, task.required,
        "progress must stay held at required so the machine pays out the \
         tick the worker is back, not restart the cycle"
    );

    assert_eq!(
        game.world.get::<Stock>(node).unwrap().output_used(),
        cap - tuning::HAUL_CARRY_CAPACITY,
        "the cap is what leaves a buffer for a downstream neighbour to pull from"
    );
}

#[test]
fn a_worker_off_its_tile_produces_nothing_and_says_so() {
    let mut game = base(2);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    // Well outside the four tiles the node can be worked from, and outside
    // any cost field a walk could build, so it never arrives — which is
    // `Stranded` rather than merely `Unstaffed`. `unstaffed_wins_over_running`
    // below is the reachable half of the same gate.
    move_to(&mut game, worker, 400, 400);

    let before = game.world.get::<Task>(worker).unwrap().progress;
    for _ in 0..10 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<Task>(worker).unwrap().progress,
        before,
        "production must not advance while the worker is away from its post"
    );
    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Stranded,
    );
}

#[test]
fn unstaffed_wins_over_running() {
    let mut game = base(3);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    // Off its post but with a clear route to it, so this is a worker that is
    // merely walking. Deliberately not the unreachable tile the test above
    // uses: that one reads `Stranded`, and asserting the precedence from
    // there would pass on the marker's one-tick lag rather than on the rule.
    move_to(&mut game, worker, 4, 0);
    // An empty output buffer would otherwise read as Running.
    game.world.get_mut::<Stock>(node).unwrap().output.clear();

    game.tick();

    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Unstaffed,
        "a machine with nothing wrong but nobody there is not Running"
    );
}

/// Bevy's query iteration order is not stable, so the two depots are
/// deployed in the *opposite* order to their positions. Deployed in position
/// order this would pass on iteration order alone, which is the bug the
/// distance sort and the tie-break exist to prevent.
///
/// End to end rather than against `nearest_depot` alone: the pure function
/// takes a slice a caller already ordered, so testing it in isolation could
/// not catch the system handing it an unordered one.
#[test]
fn a_worker_delivers_to_the_nearer_of_two_depots() {
    let mut game = base(4);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let far = deploy(&mut game, "depot", 4, 1);
    let near = deploy(&mut game, "depot", 2, 1);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 300, |g| {
        node_output(g, near, ids::CORE_FRAGMENT) > 0 || node_output(g, far, ids::CORE_FRAGMENT) > 0
    });

    assert_eq!(
        node_output(&game, near, ids::CORE_FRAGMENT),
        tuning::HAUL_CARRY_CAPACITY,
        "the load belongs in the nearer depot"
    );
    assert_eq!(
        node_output(&game, far, ids::CORE_FRAGMENT),
        0,
        "and nothing should have reached the far one"
    );
}

/// **The corrected B3, made measurable.** Adjacency is a throughput
/// multiplier and not a requirement, so what a hub-fed machine actually
/// costs is the walk — and the record has to carry the distance, or the
/// analysis has a starve fraction with nothing to plot it against.
///
/// The post's tile and not the worker's: by the time an errand acts the two
/// are the same place, and what the analysis groups by is the machine.
#[test]
fn a_delivered_load_records_the_errand_and_how_far_it_went() {
    let mut game = base(6);
    game.world
        .resource_mut::<crate::resources::BattleTelemetry>()
        .on = true;
    let node = deploy(&mut game, "mining_node", 0, 1);
    let depot = deploy(&mut game, "depot", 3, 1);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 300, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    let hauls: Vec<(String, String, u32, u32)> = game
        .world
        .resource::<crate::resources::BattleTelemetry>()
        .records
        .iter()
        .filter_map(|r| match r {
            crate::telemetry::Record::Haul {
                errand,
                kind,
                qty,
                distance,
                ..
            } => Some((errand.clone(), kind.clone(), *qty, *distance)),
            _ => None,
        })
        .collect();

    assert!(
        hauls
            .iter()
            .any(|(errand, kind, qty, distance)| errand == "deposit"
                && kind == "mining_node"
                && *qty > 0
                && *distance == 3),
        "the delivery has to record its errand, its post's kind and the \
         three tiles it walked, got {hauls:?}"
    );
}

#[test]
fn a_depot_is_not_offered_as_a_cronjob() {
    let mut game = base(5);
    let depot = deploy(&mut game, "depot", 2, 0);
    let worker = hauler(&mut game);

    assert!(
        game.assign_cronjob(worker, depot).is_err(),
        "a depot is delivered to, not worked — accepts_a_program must \
         already refuse it with no new code"
    );
}

#[test]
fn a_carried_load_ends_up_in_the_depot_and_in_your_cargo() {
    let mut game = base(6);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);

    tick_until(&mut game, 200, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) >= tuning::HAUL_CARRY_CAPACITY,
        "the worker should have walked a load to the depot"
    );
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "the load is dropped on arrival, which is what flips the destination back"
    );

    // Consolidation costs no new code: a depot is a `Stock` with an output,
    // which is the only thing a take has ever asked about.
    let depot_pos = *game.world.get::<Position>(depot).unwrap();
    let player = game.player_entity();
    move_to(&mut game, player, depot_pos.x - 1, depot_pos.y);
    let taken = take_everything_adjacent(&mut game);
    assert!(
        taken
            .iter()
            .any(|(id, n)| *id == ItemId::from(ids::CORE_FRAGMENT) && *n > 0),
        "a take must work on a depot unchanged: {taken:?}"
    );
}

#[test]
fn a_posted_program_walks_to_its_machine_before_producing() {
    let mut game = base(7);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let node_pos = *game.world.get::<Position>(node).unwrap();
    let worker = hauler(&mut game);
    // The distance is the *player's*: a posted program sets off from
    // wherever you were standing when you posted it, so posting from the
    // far side of the pocket is what buys the walk.
    stand_in_base_at(
        &mut game,
        node_pos.x + STARTING_POCKET_RADIUS - 1,
        node_pos.y,
    );
    game.assign_cronjob(worker, node).unwrap();

    let start = *game.world.get::<Position>(worker).unwrap();
    game.tick();
    let after = *game.world.get::<Position>(worker).unwrap();
    assert_ne!(
        (start.x, start.y),
        (after.x, after.y),
        "a program takes its post by walking to it"
    );

    tick_until(&mut game, 40, |g| {
        game::base::hauling::at_station(*g.world.get::<Position>(worker).unwrap(), node_pos, 1)
    });
    assert!(
        game::base::hauling::at_station(*game.world.get::<Position>(worker).unwrap(), node_pos, 1),
        "it should arrive"
    );
    game.tick();
    assert!(
        game.world.get::<Task>(worker).unwrap().progress > 0,
        "and start producing once it does"
    );
}

/// A full buffer moves `HAUL_CARRY_CAPACITY` at a time, so it takes several
/// round trips to shift — which is what makes the base's motion continuous
/// rather than one big haul.
#[test]
fn clearing_a_full_buffer_takes_several_trips() {
    let mut game = base(11);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    let cap = capacity_of(&game, node);

    // Topped back up every tick, which drops two couplings this test never
    // meant to have. A worker departs only from a *clogged* machine, so
    // without the refill each trip after the first waits on the node
    // re-filling its own buffer — which puts a hauling test at the mercy of
    // how fast the posted program extracts, and then of whether a GC Entropy
    // Sweep flattens the Depot before the fourth load lands. Both were true
    // here: the run held together on seed luck until the extraction rate
    // moved underneath it. What is under test is that a buffer's worth
    // crosses in `HAUL_CARRY_CAPACITY` loads, not how quickly it refills.
    for _ in 0..600 {
        if node_output(&game, depot, ids::CORE_FRAGMENT) >= cap {
            break;
        }
        fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);
        game.tick();
    }

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) >= cap,
        "a {cap}-unit buffer moves {} units per trip, so it takes {} of them",
        tuning::HAUL_CARRY_CAPACITY,
        cap / tuning::HAUL_CARRY_CAPACITY,
    );
}

/// The invariant that makes the whole feature opt-in: a base with no depot
/// behaves exactly as it did before depots existed.
#[test]
fn with_no_depot_a_clogged_machine_just_stays_clogged() {
    let mut game = base(13);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);
    let post = *game.world.get::<Position>(worker).unwrap();

    for _ in 0..60 {
        game.tick();
    }

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "with nowhere to take a load there is no errand to start"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        cap,
        "the buffer is untouched"
    );
    let now = *game.world.get::<Position>(worker).unwrap();
    assert_eq!((post.x, post.y), (now.x, now.y), "and nobody goes anywhere");
}

/// The depot fills up *while the worker is walking to it*, which is the only
/// way to reach the return path: a depot with no room is not a destination in
/// the first place, so filling it beforehand would just stop the errand
/// starting.
#[test]
fn a_load_with_nowhere_to_land_goes_back_and_re_clogs_the_machine() {
    let mut game = base(8);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    let node_cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, node_cap);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    // Brim-full with something a Mining Node never makes, so the only reason
    // the load cannot land is room.
    fill_to_capacity(&mut game, depot, ids::POWER_CELL);

    tick_until(&mut game, 300, |g| {
        g.world.get::<Carrying>(worker).is_none()
    });

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "the load must go back into the machine rather than ride forever"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        node_cap,
        "the base stalls loudly instead of the goods vanishing"
    );
}

#[test]
fn demolishing_a_machine_takes_its_workers_load_with_it() {
    let mut game = base(9);
    let node = deploy(&mut game, "mining_node", 1, 0);
    deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    game.remove_structure(node).unwrap();

    assert!(game.world.get::<Task>(worker).is_none());
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "a worker whose task is gone must not keep a load with nowhere to put it"
    );
}

#[test]
fn a_sweep_that_destroys_a_machine_takes_its_workers_load_too() {
    let mut game = base(14);
    let node = deploy(&mut game, "mining_node", 1, 0);
    deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    let hp = game.world.get::<Durability>(node).unwrap().hp;
    game.damage_structure(node, hp, "Mining Node", "a GC Entropy Sweep");

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "the raid path clears a load exactly as demolition does — two paths, \
         one obligation"
    );
}

#[test]
fn a_depot_demolished_mid_walk_re_targets_the_next_one() {
    let mut game = base(12);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let near = deploy(&mut game, "depot", 3, 0);
    let far = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    game.remove_structure(near).unwrap();

    tick_until(&mut game, 400, |g| {
        node_output(g, far, ids::CORE_FRAGMENT) > 0
    });
    assert!(
        node_output(&game, far, ids::CORE_FRAGMENT) > 0,
        "a worker whose depot vanished mid-walk delivers to the next nearest"
    );
}

#[test]
fn a_carried_load_survives_a_save_and_load() {
    let mut game = base(10);
    let node = deploy(&mut game, "mining_node", 1, 0);
    deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    let before = game
        .world
        .get::<Carrying>(worker)
        .cloned()
        .expect("precondition");

    let path = std::env::temp_dir().join(format!(
        "feral_processes_hauling_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut q = loaded.world.query::<(&Carrying, &Task)>();
    let (carrying, _) = q
        .iter(&loaded.world)
        .next()
        .expect("the load must come back with the worker");
    assert_eq!(carrying.item, before.item);
    assert_eq!(carrying.qty, before.qty);
}

/// Every tile `worker` stands on across `limit` ticks, including where it
/// starts. Recorded rather than asserted per tick so a failure can name the
/// tile that was walked over.
fn tiles_walked(game: &mut Game, worker: Entity, limit: u32) -> Vec<(i32, i32)> {
    let mut seen = Vec::new();
    for _ in 0..limit {
        let pos = *game.world.get::<Position>(worker).unwrap();
        if seen.last() != Some(&(pos.x, pos.y)) {
            seen.push((pos.x, pos.y));
        }
        game.tick();
    }
    seen
}

fn structure_tiles(game: &mut Game) -> Vec<(i32, i32)> {
    let mut query = game.world.query_filtered::<&Position, With<Structure>>();
    query.iter(&game.world).map(|p| (p.x, p.y)).collect()
}

/// A hauler routes around the base rather than over it.
///
/// A *wall* rather than a single blocker: the step rule picks the cheapest
/// neighbour by `(cost, x, y)`, so one structure on the straight line is
/// dodged by the tie-break alone and the test passes without the fix. Three
/// abreast leaves no equal-cost tile to slip through, and the only route to
/// the depot is around the end of the wall.
#[test]
fn a_hauler_never_walks_over_a_structure() {
    let mut game = base(20);
    let node = deploy(&mut game, "mining_node", 1, 0);
    deploy(&mut game, "depot", 4, 0);
    let blocker = deploy(&mut game, "mining_node", 3, 0);
    deploy(&mut game, "mining_node", 3, -1);
    deploy(&mut game, "mining_node", 3, 1);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    let walked = tiles_walked(&mut game, worker, 60);
    let blocked = structure_tiles(&mut game);

    let trespass: Vec<(i32, i32)> = walked
        .iter()
        .copied()
        .filter(|t| blocked.contains(t))
        .collect();
    assert!(
        trespass.is_empty(),
        "a hauler walked over {trespass:?}; its route was {walked:?}"
    );
    let blocker_pos = *game.world.get::<Position>(blocker).unwrap();
    assert!(
        walked.len() > 1,
        "precondition: the worker has to actually set off, route was {walked:?}"
    );
    assert_eq!(
        (blocker_pos.x, blocker_pos.y),
        (3, 0),
        "precondition: the blocker sits between the two posts"
    );
}

/// A machine the base has been built around has no tile to stand on, and
/// posting to it is refused before anything is spent — the same check
/// `haul_step_system` would fail, asked up front.
#[test]
fn posting_to_a_boxed_in_machine_is_refused() {
    let mut game = base(21);
    let node = deploy(&mut game, "mining_node", 2, 0);
    for (dx, dy) in [(1, 0), (3, 0), (2, 1), (2, -1)] {
        deploy(&mut game, "mining_node", dx, dy);
    }
    let worker = hauler(&mut game);

    let err = game
        .assign_cronjob(worker, node)
        .expect_err("nothing can stand next to a machine walled in on all four sides");

    assert!(err.contains("walled in"), "unexpected refusal: {err}");
    assert!(
        game.world.get::<Task>(worker).is_none(),
        "a refused cronjob must leave no Task behind"
    );
}

/// A route lost *after* the posting. `assign_cronjob` checks the walk to the
/// machine, not the walk to a depot, so a depot the base has closed in is
/// reachable at assignment and unreachable by the time there is a load to
/// carry — which is the case `Stranded` exists to name.
#[test]
fn a_worker_with_nowhere_to_deliver_strands_its_machine() {
    let mut game = base(22);
    lay_long_floor(&mut game);
    let node = deploy(&mut game, "mining_node", 0, 2);
    deploy(&mut game, "depot", 12, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);
    lose_the_route_mid_carry(&mut game, worker, close_the_depot_in);

    tick_until(&mut game, 40, |g| {
        g.world.get::<MachineStatus>(node) == Some(&MachineStatus::Stranded)
    });

    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "precondition: the worker has to be holding a load it cannot deliver"
    );
    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Stranded,
        "a machine whose worker has nowhere to go says so, rather than \
         reading as merely away"
    );
}

/// `place_structure` never checks whether a program is standing on the tile,
/// so a building can go up on top of a hauler. It has to be able to step off
/// its own tile — the walk refuses occupied tiles as *destinations*, not as
/// starting points.
///
/// Built over mid-errand rather than at its post: a worker standing at its
/// own machine is `at_station` and never builds a field at all, so it would
/// carry on working from under the building and prove nothing.
#[test]
fn a_worker_built_over_can_still_step_off_its_own_tile() {
    let mut game = base(23);
    let node = deploy(&mut game, "mining_node", 2, 0);
    let depot = deploy(&mut game, "depot", -2, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 40, |g| g.world.get::<Carrying>(worker).is_some());
    let parked = *game.world.get::<Position>(worker).unwrap();
    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "precondition: the worker must be holding a load and have somewhere to take it"
    );

    // Deployed from beside the worker onto the very tile it is standing on.
    stand_player_at(&mut game, parked.x, parked.y + 1);
    deploy(&mut game, "mining_node", 0, -1);

    tick_until(&mut game, 60, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) > 0,
        "a worker built over must be able to walk out from under it and \
         finish its delivery"
    );
}

/// The nearest tile beside a machine is not always a tile you may stand on.
/// `station_tile` has to skip an occupied neighbour rather than nominate it
/// and leave the worker walking at a building forever.
#[test]
fn a_worker_parks_on_the_free_side_of_its_machine() {
    let mut game = base(24);
    let node = deploy(&mut game, "mining_node", 2, 0);
    // The two sides facing the player, so the nearest neighbour by distance
    // is the occupied one and only the far side is legal.
    deploy(&mut game, "mining_node", 1, 0);
    deploy(&mut game, "mining_node", 2, 1);
    deploy(&mut game, "mining_node", 2, -1);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();

    tick_until(&mut game, 40, |g| {
        g.world.get::<MachineStatus>(node) == Some(&MachineStatus::Running)
    });

    let pos = *game.world.get::<Position>(worker).unwrap();
    assert_eq!(
        (pos.x, pos.y),
        (3, 0),
        "the worker must walk around to the one free side"
    );
}

/// An extractor nobody is posted to reports `Idle`, the same as an
/// assembler nobody is posted to.
///
/// `MachineStatus` defaults to `Running`, and for a long time the only thing
/// that ever said otherwise for an unworked machine was `assembler_system` —
/// which skips anything that does not declare `assembles`. So a freshly
/// deployed Research Node sat green on the map, reading as producing, for as
/// long as it went unstaffed.
#[test]
fn an_extractor_with_no_program_reports_idle() {
    let mut game = base(25);
    let node = deploy(&mut game, "research_node", 1, 0);
    let mine = deploy(&mut game, "mining_node", 0, 2);

    game.tick();

    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Idle,
        "a Research Node with nobody on it is idle, not running"
    );
    assert_eq!(
        *game.world.get::<MachineStatus>(mine).unwrap(),
        MachineStatus::Idle,
        "and so is every other extractor — this was never research-specific"
    );
}

/// The other half: a machine that *is* worked must not be dragged back to
/// `Idle` by the pass that sets it.
#[test]
fn a_worked_extractor_does_not_read_idle() {
    let mut game = base(26);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    tick_until(&mut game, 20, |g| {
        g.world.get::<MachineStatus>(node) == Some(&MachineStatus::Running)
    });

    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Running,
    );
}

/// A machine with nobody downstream is not a feed buffer, so hoarding
/// twenty units in it before the first trip serves nothing — the goods
/// belong where the base can count and collect them.
///
/// Deliberately measured against the *buffer*, not against the clock: the
/// assertion is that a load leaves while there is still room in the output,
/// which is exactly what a clog-only pickup could never do.
#[test]
fn a_machine_with_nothing_downstream_delivers_as_it_produces() {
    let mut game = base(30);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    let cap = capacity_of(&game, node);
    tick_until(&mut game, 120, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) > 0,
        "a lone extractor's payout should reach the depot without the \
         machine having to clog first"
    );
    assert!(
        game.world.get::<Stock>(node).unwrap().output_used() < cap,
        "and it should never have filled up on the way"
    );
}

/// The other half, and the one that keeps a production line a production
/// line: an orthogonal neighbour whose recipe names this machine's product
/// *is* the attached building, so its feed buffer is left alone for
/// `assembler_system` to pull from.
///
/// The order is what makes the Lathe an attached building rather than a
/// bystander — see
/// `a_neighbour_nothing_has_been_ordered_from_is_not_an_attached_building`.
#[test]
fn a_machine_feeding_a_neighbour_keeps_its_buffer() {
    let mut game = base(31);
    let node = deploy(&mut game, "mining_node", 1, 0);
    // A Lathe assembles Blank Substrate out of Core Fragments, so it is a
    // consumer of exactly what the node beside it makes. Spawned rather than
    // deployed because it is gated behind research this fixture has no
    // business unlocking.
    let node_pos = *game.world.get::<Position>(node).unwrap();
    spawn_machine_at(&mut game, "lathe", node_pos.x + 1, node_pos.y);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from("blank_substrate"), 5))
        .unwrap();

    for _ in 0..20 {
        game.tick();
    }

    // Asserted against the *Depot* rather than against `Carrying`: a load
    // taken and delivered inside the twenty ticks leaves empty hands behind
    // it, so a `Carrying` check reads the same either way. What the Depot
    // holds only ever goes up.
    assert_eq!(
        node_output(&game, depot, ids::CORE_FRAGMENT),
        0,
        "a machine with a consumer beside it feeds the line, not the depot"
    );
    let _ = worker;
}

/// The case the two halves above did not separate: a neighbour whose recipe
/// names this machine's product, standing beside it with nothing asking for
/// what it makes.
///
/// An unstaffed assembler pulls nothing (`assembler_system` returns before
/// its pull phase with no program posted), so treating it as an attached
/// building reserved the whole buffer for a machine that would never take
/// it — measured at 400 ticks before a single Core Fragment reached the
/// Depot, against the first cycle for the same node standing alone.
#[test]
fn a_neighbour_nothing_has_been_ordered_from_is_not_an_attached_building() {
    let mut game = base(33);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let node_pos = *game.world.get::<Position>(node).unwrap();
    spawn_machine_at(&mut game, "lathe", node_pos.x + 1, node_pos.y);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, 3);
    // The order is for the *node's own* product, so nothing in the queue's
    // recipe tree names Blank Substrate and the Lathe has no reason to run.
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 60))
        .unwrap();

    tick_until(&mut game, 60, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) > 0,
        "the goods belong where the base can count them, not reserved for a \
         machine nobody has ordered from"
    );
}

/// The reason `queue_needs` is a *closure* over recipes rather than a look at
/// the ordered item alone: the order names Routine Disks, and it is the Disk
/// Press two links down that needs them. A one-hop rule would take the Lathe
/// for a bystander and dismantle the line the order was filed to run.
///
/// Deliberately without the Press standing: what is under test is that the
/// item tree reaches Core Fragments, and a deployed Press would let the
/// weaker rule pass by naming Blank Substrate directly.
#[test]
fn an_order_two_links_downstream_still_keeps_the_feeder_hoarding() {
    let mut game = base(34);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let node_pos = *game.world.get::<Position>(node).unwrap();
    spawn_machine_at(&mut game, "lathe", node_pos.x + 1, node_pos.y);
    spawn_machine_at(&mut game, "disk_press", node_pos.x + 2, node_pos.y);
    // Off to the side rather than further along the row: a fourth tile out
    // is past `MAX_BUILD_DISTANCE_FROM_HOME` once the Press has taken the
    // third.
    let depot = deploy(&mut game, "depot", 0, 3);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 5))
        .unwrap();

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(
        node_output(&game, depot, ids::CORE_FRAGMENT),
        0,
        "Routine Disks are made of Blank Substrate, which is made of these — \
         the Lathe is on the ordered line"
    );
}

/// The other half of "a reason to run", and the one that has nothing to do
/// with the queue: a standing work job is the player saying *keep this
/// running* outside any order, so the machine beside it is feeding a line
/// whether or not anything is queued.
#[test]
fn a_standing_job_on_the_neighbour_is_reason_enough_to_keep_feeding_it() {
    let mut game = base(35);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let node_pos = *game.world.get::<Position>(node).unwrap();
    let lathe = spawn_machine_at(&mut game, "lathe", node_pos.x + 1, node_pos.y);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, 3);
    game.world.entity_mut(lathe).insert(StandingJob {
        work: true,
        guard: false,
    });

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(
        node_output(&game, depot, ids::CORE_FRAGMENT),
        0,
        "an empty queue is no instruction to take a standing line apart"
    );
}

/// The return leg. A worker at a bench that cannot assemble a batch, with
/// the ingredient sitting in a depot, goes and gets it rather than standing
/// there starved.
///
/// The load lands in the machine's `input` — the one place outside
/// `assembler_system` that writes it, and defensible because this is the
/// machine's own posted program loading its hopper rather than a neighbour
/// reaching in.
#[test]
fn a_worker_short_an_ingredient_fetches_it_from_the_depot() {
    let mut game = base(32);
    let press = spawn_machine_at(&mut game, "disk_press", 1, 0);
    let depot = deploy(&mut game, "depot", 3, 0);
    fill_output(&mut game, depot, "blank_substrate", 10);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, press).unwrap();
    park_at_post(&mut game, worker, press);

    tick_until(&mut game, 120, |g| {
        g.world
            .get::<Stock>(press)
            .unwrap()
            .input
            .get(&ItemId::from("blank_substrate"))
            .copied()
            .unwrap_or(0)
            > 0
    });

    assert!(
        game.world
            .get::<Stock>(press)
            .unwrap()
            .input
            .get(&ItemId::from("blank_substrate"))
            .copied()
            .unwrap_or(0)
            > 0,
        "the ingredient should have been carried from the depot into the press"
    );
    assert!(
        node_output(&game, depot, "blank_substrate") < 10,
        "and taken out of the depot on the way"
    );
}

// ---------------------------------------------------------------------------
// Keeping a burning supplier fed. A structure declaring
// `StructureDef::power_upkeep` wants one Power Cell within reach at all times
// and has no program of its own to fetch one, so the scheduler files a want
// for it — above every work order and below only a build request. See
// `Game::fuel_wants`.
// ---------------------------------------------------------------------------

/// A Recharger Node with nothing touching it, and a Depot four tiles off
/// holding `cells` Power Cells. Far enough apart that the node's own
/// four-tile reach cannot see the shelf, so only a program walking closes
/// the gap.
fn recharger_and_a_distant_depot(game: &mut Game, cells: u32) -> (Entity, Entity) {
    let recharger = deploy(game, "recharger_node", 1, 0);
    let depot = deploy(game, "depot", 4, 0);
    if cells > 0 {
        fill_output(game, depot, ids::POWER_CELL, cells);
    }
    (recharger, depot)
}

/// What is in a structure's *input* hopper — where a fetched load lands, as
/// against `node_output`'s shelf.
fn hopper(game: &Game, structure: Entity, item: &str) -> u32 {
    game.world
        .get::<Stock>(structure)
        .and_then(|s| s.input.get(&ItemId::from(item)).copied())
        .unwrap_or(0)
}

/// The structure a program is currently posted to, whatever the kind.
fn posted_to(game: &Game, worker: Entity) -> Option<Entity> {
    game.world.get::<Task>(worker).map(|t| t.target)
}

/// A base that may build a Power Conduit — `power_grid` is its research node.
fn base_with_conduits(seed: u32) -> Game {
    let mut game = base(seed);
    unlock_research_chain(&mut game, "power_grid");
    game
}

/// A standing order big enough that no fixture fill can satisfy it, so the
/// machine below stays a want for the whole of a test.
fn order_core_fragments(game: &mut Game) {
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 500))
        .unwrap();
}

#[test]
fn a_burner_with_no_cell_within_reach_is_fetched_one_off_the_shelf() {
    let mut game = base(9101);
    let (recharger, depot) = recharger_and_a_distant_depot(&mut game, 4);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, recharger);

    tick_until(&mut game, 80, |g| hopper(g, recharger, ids::POWER_CELL) > 0);

    assert!(
        hopper(&game, recharger, ids::POWER_CELL) > 0,
        "a supplier with nothing to burn and cells on a shelf should have \
         had one carried to it"
    );
    assert!(
        node_output(&game, depot, ids::POWER_CELL) < 4,
        "and taken off the shelf on the way"
    );
}

#[test]
fn feeding_a_burner_outranks_a_queued_work_order() {
    let mut game = base(9102);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let (recharger, _) = recharger_and_a_distant_depot(&mut game, 4);
    order_core_fragments(&mut game);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, node);

    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        Some(recharger),
        "with one body and both wants standing, the fuel want is filed \
         above the order"
    );
}

#[test]
fn a_burner_with_a_stocked_buffer_beside_it_is_no_want_at_all() {
    let mut game = base(9103);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let recharger = deploy(&mut game, "recharger_node", 1, 0);
    // Touching the node, which is the hand-stocked arrangement the feature
    // shipped with: the supplier reaches this shelf itself.
    let beside = deploy(&mut game, "depot", 2, 0);
    fill_output(&mut game, beside, ids::POWER_CELL, 4);
    order_core_fragments(&mut game);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, node);

    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        Some(node),
        "a supplier that can already reach a cell wants nobody, so the \
         order keeps the body"
    );
    assert_eq!(
        hopper(&game, recharger, ids::POWER_CELL),
        0,
        "and nothing is carried to it"
    );
}

#[test]
fn a_burner_is_no_want_when_no_shelf_holds_its_fuel() {
    let mut game = base(9104);
    let node = deploy(&mut game, "mining_node", 0, 1);
    // A Depot, but an empty one: the want is gated on the base actually
    // holding the fuel, or a body walks to a shelf that cannot pay it.
    recharger_and_a_distant_depot(&mut game, 0);
    order_core_fragments(&mut game);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, node);

    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        Some(node),
        "nothing in store to fetch is nothing to want a body for"
    );
}

#[test]
fn a_short_burner_with_nothing_in_store_puts_a_body_on_what_makes_its_fuel() {
    // The other half of the stock gate above. Nothing to fetch is not
    // nothing to do: the base can *make* the fuel, and a body left on a
    // Mining Node while every Recharger runs dry is how a run used to end
    // with a working Conduit standing idle beside it.
    let mut game = base_with_conduits(9110);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let conduit = deploy(&mut game, "power_conduit", -2, 0);
    recharger_and_a_distant_depot(&mut game, 0);
    order_core_fragments(&mut game);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, node);

    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        Some(conduit),
        "with no cell anywhere, the fuel want is a want on the machine that \
         makes one, and it still outranks the order"
    );
}

#[test]
fn a_base_with_no_cell_anywhere_relights_its_own_recharger() {
    // End to end, and the whole of what the want above is for: nothing on
    // any shelf, nothing in any hopper, and the base gets a cell into the
    // Recharger by itself. The Conduit's worker fills its buffer and
    // unloads it onto the Depot, and the fetch half of `fuel_wants` takes
    // it from there.
    let mut game = base_with_conduits(9111);
    deploy(&mut game, "power_conduit", -2, 0);
    let (recharger, _) = recharger_and_a_distant_depot(&mut game, 0);
    hauler(&mut game);
    hauler(&mut game);

    tick_until(&mut game, 600, |g| {
        hopper(g, recharger, ids::POWER_CELL) > 0
    });

    assert!(
        hopper(&game, recharger, ids::POWER_CELL) > 0,
        "a base that can make its own fuel must never stay dark for want \
         of a body to make it"
    );
}

/// The deadlock a real save reached: a Power Conduit well out of the
/// Recharger's reach sitting on a full buffer of cells, and the only Depot
/// full of something else. The cells can never reach a shelf, so a fetch
/// that reads shelves alone never fires, and the make half names nothing
/// because a full Conduit cannot progress.
fn cells_stranded_in_a_distant_conduit(game: &mut Game) -> (Entity, Entity) {
    let recharger = deploy(game, "recharger_node", 1, 0);
    let conduit = deploy(game, "power_conduit", -3, 0);
    fill_output(game, conduit, ids::POWER_CELL, 20);
    let depot = deploy(game, "depot", 4, 0);
    fill_output(game, depot, ids::CORE_FRAGMENT, 50);
    game.world
        .get_mut::<components::PowerFuel>(recharger)
        .unwrap()
        .ticks_left = 0;
    (recharger, conduit)
}

#[test]
fn a_burner_fetches_its_fuel_off_a_machine_when_no_shelf_holds_any() {
    let mut game = base_with_conduits(9113);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let (recharger, _) = cells_stranded_in_a_distant_conduit(&mut game);
    order_core_fragments(&mut game);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, node);

    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        Some(recharger),
        "cells in a machine's output are fuel in store, so the burner is a \
         fetch want"
    );
}

#[test]
fn a_full_depot_does_not_strand_a_recharger_beside_a_full_conduit() {
    let mut game = base_with_conduits(9114);
    let (recharger, conduit) = cells_stranded_in_a_distant_conduit(&mut game);
    hauler(&mut game);

    tick_until(&mut game, 200, |g| {
        hopper(g, recharger, ids::POWER_CELL) > 0
    });

    assert!(
        hopper(&game, recharger, ids::POWER_CELL) > 0,
        "a cell sitting in the Conduit's output must be carried to the \
         Recharger even with nowhere to shelve it"
    );
    assert!(
        node_output(&game, conduit, ids::POWER_CELL) < 20,
        "and taken off the Conduit on the way"
    );
}

#[test]
fn a_dark_fuel_maker_is_not_handed_a_body() {
    // A body posted to a Conduit the grid cannot run stands there making
    // nothing for as long as the base is short — which is exactly as long
    // as the want lasts, so on a one-body base it never comes back. Five
    // Conduits against the Home's 4: the fifth in tile order is dark.
    let mut game = base_with_conduits(9112);
    let conduits: Vec<Entity> = (0..5)
        .map(|i| deploy(&mut game, "power_conduit", -2, i - 2))
        .collect();
    let (recharger, _) = recharger_and_a_distant_depot(&mut game, 0);
    // A fresh Recharger comes with a full window of charge, which would put
    // supply at 8 and every Conduit in the light. The blackout is the case.
    game.world
        .get_mut::<components::PowerFuel>(recharger)
        .unwrap()
        .ticks_left = 0;
    let workers: Vec<Entity> = (0..5).map(|_| hauler(&mut game)).collect();

    game.tick();

    let dark = conduits[4];
    assert!(
        game.world
            .resource::<resources::PowerGrid>()
            .dark
            .contains(&dark),
        "fixture: the fifth Conduit is past the Home's supply"
    );
    for w in workers {
        assert_ne!(
            posted_to(&game, w),
            Some(dark),
            "nobody posted to a dark Conduit"
        );
    }
}

#[test]
fn a_lines_body_goes_to_its_lit_fuel_maker_not_its_dark_end_machine() {
    // The `chains` deadlock. A Conduit -> Winding Node -> Assembly Bay line
    // on the Home's 4, with a second Conduit off to one side: the two
    // Conduits (rung 0) and the Winding Node take all 4, so the Bay (draw 3)
    // is dark. The Bay still has a batch of coils
    // beside it, so an order for Patch Routines names it — and `collapse`
    // aims the line's one body at the furthest-downstream want, the dark
    // Bay, where it makes nothing while the Conduit that could end the
    // blackout stands idle for the rest of the run.
    let mut game = base_with_conduits(9116);
    let bay = deploy(&mut game, "assembly_bay", -2, -1);
    let winding = deploy(&mut game, "winding_node", -2, 0);
    let conduit = deploy(&mut game, "power_conduit", -2, 1);
    deploy(&mut game, "power_conduit", 2, 2);
    fill_output(&mut game, winding, "charge_coil", 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from("patch_routine"), 500))
        .unwrap();
    let worker = hauler(&mut game);

    game.tick();

    assert!(
        game.world.resource::<resources::PowerGrid>().is_dark(bay),
        "fixture: the Bay is past the Home's supply"
    );
    assert!(
        !game
            .world
            .resource::<resources::PowerGrid>()
            .is_dark(conduit),
        "fixture: the Conduit is lit"
    );
    assert_eq!(
        posted_to(&game, worker),
        Some(conduit),
        "the line's one body works the lit machine that can move, not the dark Bay"
    );
}

#[test]
fn a_fed_burner_gives_the_body_back() {
    let mut game = base(9105);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let (recharger, _) = recharger_and_a_distant_depot(&mut game, 4);
    order_core_fragments(&mut game);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, recharger);

    game.tick();
    assert_eq!(
        posted_to(&game, worker),
        Some(recharger),
        "the fixture has to actually hand the body over, or the release \
         below is asserting nothing"
    );

    tick_until(&mut game, 80, |g| hopper(g, recharger, ids::POWER_CELL) > 0);
    // One more beat for the scheduler to see the satisfied want.
    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        Some(node),
        "a stocked supplier stops wanting a body, and the order takes it \
         back — a fuel want that latched would starve production for the \
         rest of the run"
    );
}

/// The same release with **nothing else on the base to do**, which is the
/// case the empty-queue guard in `schedule_base_labour` gets wrong if it is
/// not told about fuel wants: a satisfied want simply vanishes from the
/// list, and `all(posted.contains)` is vacuously true against an empty one.
/// Left to it, the body stands at a stocked Recharger Node for the rest of
/// the run.
#[test]
fn a_fed_burner_gives_the_body_back_on_a_base_with_no_orders_at_all() {
    let mut game = base(9106);
    let (recharger, _) = recharger_and_a_distant_depot(&mut game, 4);
    let worker = hauler(&mut game);
    park_at_post(&mut game, worker, recharger);

    game.tick();
    assert_eq!(
        posted_to(&game, worker),
        Some(recharger),
        "an empty queue is no reason not to keep the grid lit"
    );

    tick_until(&mut game, 80, |g| hopper(g, recharger, ids::POWER_CELL) > 0);
    game.tick();

    assert_eq!(
        posted_to(&game, worker),
        None,
        "with the hopper stocked and nothing else wanted, the body is free"
    );
}

/// The crew's half of `components::DepotFilter`: a shelf that refuses the
/// load is no more a destination than a full one, and the walk simply
/// carries on to the next.
///
/// The mirror of `a_worker_delivers_to_the_nearer_of_two_depots` with one
/// thing changed, so a filter that was quietly ignored fails here as the
/// load landing where the distance sort alone would have put it.
#[test]
fn a_worker_walks_past_a_depot_that_refuses_its_load() {
    let mut game = base(4);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let far = deploy(&mut game, "depot", 4, 1);
    let near = deploy(&mut game, "depot", 2, 1);
    game.set_depot_filter(near, &[ItemId::from(ids::CORE_FRAGMENT)], false);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 300, |g| {
        node_output(g, near, ids::CORE_FRAGMENT) > 0 || node_output(g, far, ids::CORE_FRAGMENT) > 0
    });

    assert_eq!(
        node_output(&game, far, ids::CORE_FRAGMENT),
        tuning::HAUL_CARRY_CAPACITY,
        "the nearer shelf refuses it, so the load belongs in the far one"
    );
    assert_eq!(
        node_output(&game, near, ids::CORE_FRAGMENT),
        0,
        "and nothing may be forced onto the shelf that said no"
    );
}

/// `with_no_depot_a_clogged_machine_just_stays_clogged`, reached the other
/// way: a Depot that will not take what this machine makes is no reason to
/// set off, so nobody picks up a load they would only have to carry back.
#[test]
fn a_depot_that_refuses_the_load_is_no_reason_to_set_off() {
    let mut game = base(13);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let shelf = deploy(&mut game, "depot", 4, 0);
    game.set_depot_filter(shelf, &[ItemId::from(ids::CORE_FRAGMENT)], false);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);
    let post = *game.world.get::<Position>(worker).unwrap();

    for _ in 0..60 {
        game.tick();
    }

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "with nowhere that will take it there is no errand to start"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        cap,
        "the buffer is untouched"
    );
    let now = *game.world.get::<Position>(worker).unwrap();
    assert_eq!((post.x, post.y), (now.x, now.y), "and nobody goes anywhere");
}

/// A filter closed while the worker is already walking, which is the only
/// way to reach the return path: a shelf that refuses the load is not a
/// destination in the first place, so denying it beforehand would just stop
/// the errand starting.
#[test]
fn a_load_refused_mid_walk_goes_back_and_re_clogs_the_machine() {
    let mut game = base(8);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let shelf = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

    let node_cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, node_cap);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    game.set_depot_filter(shelf, &[ItemId::from(ids::CORE_FRAGMENT)], false);

    tick_until(&mut game, 300, |g| {
        g.world.get::<Carrying>(worker).is_none()
    });

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "the load must go back into the machine rather than ride forever"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        node_cap,
        "the base stalls loudly instead of the goods vanishing"
    );
    assert_eq!(
        node_output(&game, shelf, ids::CORE_FRAGMENT),
        0,
        "and the shelf that said no is still empty"
    );
}

/// A later rung of the storage ladder is a depot in every sense, and nothing
/// on the way there names an id.
///
/// The whole ladder — `depot_mk2` through `depot_mk6` — is six `.ron` files
/// and five research nodes with no Rust behind them, which holds only
/// because every reader in the base pipeline filters on
/// `StructureDef::stores` rather than on `"depot"`. A single hardcoded id
/// anywhere in hauling would leave a researched Mk2 standing there while a
/// clogged machine reports `Clogged` beside it, which reads as the research
/// having done nothing.
///
/// The rung is unlocked by writing `Research` directly: what the research
/// screen charges for `paging` is that screen's subject, and paying it here
/// would pin this test to a Research Data price.
#[test]
fn a_researched_depot_rung_takes_a_haul_like_the_first_one() {
    let mut game = base(7);
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .insert("paging".to_string());
    let node = deploy(&mut game, "mining_node", 1, 0);
    let depot = deploy(&mut game, "depot_mk2", 4, 0);
    assert_eq!(
        game.world.get::<Stock>(depot).unwrap().capacity,
        100,
        "the Mk2 is the rung that holds a hundred"
    );
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);

    tick_until(&mut game, 200, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) >= tuning::HAUL_CARRY_CAPACITY,
        "a Mk2 is somewhere a hauler empties into, exactly as the first Depot is"
    );
}

// --- The reach machinery widens with the footprint ---

/// `station_candidates` offers every walkable, unblocked orthogonal
/// neighbour of every footprint cell — the ring around the whole 2x2 — and
/// never one of the footprint's own four cells, which a worker never posts
/// on top of.
#[test]
fn station_candidates_offers_the_ring_around_the_whole_footprint() {
    let mut game = base_with_footprint_fixture(3010, "station_candidates_fixture");
    place_now(&mut game, "station_candidates_fixture", 2, 0).unwrap();
    let grid = game.world.resource::<crate::base_grid::BaseGrid>();
    let candidates: std::collections::HashSet<(i32, i32)> =
        crate::game::base::hauling::station_candidates(grid, Position { x: 2, y: 0 }, 2, |_| false)
            .into_iter()
            .map(|p| (p.x, p.y))
            .collect();

    let expected: std::collections::HashSet<(i32, i32)> = [
        (1, 0),
        (2, -1),
        (4, 0),
        (3, -1),
        (1, 1),
        (2, 2),
        (4, 1),
        (3, 2),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        candidates, expected,
        "the ring around a 2x2 is exactly these eight cells"
    );
    for cell in [(2, 0), (3, 0), (2, 1), (3, 1)] {
        assert!(
            !candidates.contains(&cell),
            "a footprint cell is never its own station: {cell:?}"
        );
    }
}

/// The invariant the plan states as the thing that stops the spin:
/// `at_station` is true for exactly the positions `station_candidates`
/// returns, and nothing else — checked as an equivalence over a box wide
/// enough to hold the whole ring plus a margin, not as two spot checks.
#[test]
fn at_station_agrees_with_station_candidates_exactly() {
    let mut game = base_with_footprint_fixture(3011, "at_station_equivalence_fixture");
    place_now(&mut game, "at_station_equivalence_fixture", 2, 0).unwrap();
    let structure = Position { x: 2, y: 0 };
    let side = 2;
    let grid = game.world.resource::<crate::base_grid::BaseGrid>();
    let candidates: std::collections::HashSet<(i32, i32)> =
        crate::game::base::hauling::station_candidates(grid, structure, side, |_| false)
            .into_iter()
            .map(|p| (p.x, p.y))
            .collect();

    for x in -2..=6 {
        for y in -3..=4 {
            let worker = Position { x, y };
            assert_eq!(
                crate::game::base::hauling::at_station(worker, structure, side),
                candidates.contains(&(x, y)),
                "at_station and station_candidates disagree at ({x}, {y})"
            );
        }
    }
}

/// `Game::blocked_tiles` takes only the anchor of a footprint — a body may
/// cross the other three cells exactly as it may cross any other laid
/// floor — while `Game::structure_tiles` takes the whole thing. The two
/// sets are documented as not interchangeable for this exact reason.
#[test]
fn blocked_tiles_takes_the_anchor_and_structure_tiles_takes_the_whole_footprint() {
    let mut game = base_with_footprint_fixture(3012, "blocked_tiles_fixture");
    place_now(&mut game, "blocked_tiles_fixture", 2, 0).unwrap();

    let blocked = game.blocked_tiles();
    assert!(blocked.taken((2, 0)), "the anchor blocks a walk");
    for floor_cell in [(3, 0), (2, 1), (3, 1)] {
        assert!(
            !blocked.taken(floor_cell),
            "a floor cell must stay walkable: {floor_cell:?}"
        );
    }

    let tiles = game.structure_tiles();
    for cell in [(2, 0), (3, 0), (2, 1), (3, 1)] {
        assert!(
            tiles.contains(&cell),
            "structure_tiles must cover every footprint cell: {cell:?}"
        );
    }
}

/// A hauler's own walk field is built from `Game::blocked_tiles`, so this is
/// the same claim in the shape the walk actually consults: the anchor never
/// appears as a reachable cell, and a floor cell does.
#[test]
fn a_haulers_walk_field_crosses_footprint_floor_but_never_the_anchor() {
    let mut game = base_with_footprint_fixture(3013, "hauler_walk_fixture");
    place_now(&mut game, "hauler_walk_fixture", 2, 0).unwrap();
    let blocked = game.blocked_tiles();
    let pocket_radius = game.world.resource::<crate::base_grid::BaseGrid>().radius();
    let from = Position { x: 0, y: 3 };
    let grid = game.world.resource::<crate::base_grid::BaseGrid>();
    let field = crate::game::base::hauling::crew_reach(grid, from, &blocked, pocket_radius);

    assert!(
        field.contains_key(&(3, 0)),
        "the walk must be able to step onto the structure's own floor cell"
    );
    assert!(
        !field.contains_key(&(2, 0)),
        "the walk must never step onto the anchor"
    );
}

/// **The nearest Depot is not a destination if nothing can stand beside
/// it.** Ranking stays Chebyshev, but a worker skips a Depot it has no walk
/// to and delivers to the next one. The save that found this had its only
/// free face held by an idle body that could not move either — two Mining
/// Nodes read "cut off" for five hundred ticks beside a second Depot with
/// three open sides. Boxed in by Walls here so the blockage cannot wander
/// off mid-test.
#[test]
fn a_worker_skips_a_nearer_depot_it_cannot_reach() {
    let mut game = base(4);
    let node = deploy(&mut game, "mining_node", 0, 1);
    let boxed = deploy(&mut game, "depot", 2, -1);
    for (dx, dy) in [(1, -1), (3, -1), (2, -2), (2, 0)] {
        deploy(&mut game, "wall", dx, dy);
    }
    let open = deploy(&mut game, "depot", -2, 1);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 300, |g| {
        node_output(g, open, ids::CORE_FRAGMENT) > 0
    });

    assert_eq!(
        node_output(&game, open, ids::CORE_FRAGMENT),
        tuning::HAUL_CARRY_CAPACITY,
        "the load belongs in the depot the worker can reach"
    );
    assert_eq!(node_output(&game, boxed, ids::CORE_FRAGMENT), 0);
}

/// A Mining Node walled in on three sides, its worker standing on the fourth,
/// and the one way out a single-cell corridor — with a Depot past the far
/// end. `plug` lays a downed program in the corridor cell: with no Repair Bay
/// it lies where it fell, so it is a body that does not move off on its own.
///
/// ```text
///   W W W      y = -4
///   W N W      y = -3
///   W F W      y = -2   F: the worker's post
///   W C W      y = -1   C: the corridor cell
///   . . .      y =  0
///   . . .      y =  1
///   . D .      y =  2
/// ```
fn plugged_corridor(seed: u32, plug: bool) -> (Game, Entity, Entity) {
    let mut game = base(seed);
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        for x in 1..=3 {
            for y in -4..=3 {
                grid.lay_floor(x, y);
            }
        }
    }
    let node = deploy(&mut game, "mining_node", 2, -3);
    for (dx, dy) in [
        (1, -4),
        (2, -4),
        (3, -4),
        (1, -3),
        (3, -3),
        (1, -2),
        (3, -2),
        (1, -1),
        (3, -1),
    ] {
        deploy(&mut game, "wall", dx, dy);
    }
    let depot = deploy(&mut game, "depot", 2, 2);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    move_to(&mut game, worker, 2, -2);
    if plug {
        let body = hauler(&mut game);
        game.world
            .entity_mut(body)
            .insert(crate::components::Downed);
        move_to(&mut game, body, 2, -1);
    }
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);
    (game, worker, depot)
}

/// Ticks until `depot` holds anything, and how many that took.
fn ticks_to_delivery(game: &mut Game, depot: Entity, limit: u32) -> u32 {
    for n in 0..limit {
        if node_output(game, depot, ids::CORE_FRAGMENT) > 0 {
            return n;
        }
        game.tick();
    }
    panic!("nothing reached the depot in {limit} ticks");
}

/// **A body in a corridor is squeezed past, not walked around.** The found
/// case: `chains` builds a ring of machines with a one-cell corridor round
/// it, standing programs closed it, and a carrier with no route stood holding
/// its load for thousands of ticks.
#[test]
fn a_carrier_squeezes_past_a_program_standing_in_its_corridor() {
    let (mut game, worker, depot) = plugged_corridor(31, true);

    ticks_to_delivery(&mut game, depot, 60);

    assert_eq!(
        node_output(&game, depot, ids::CORE_FRAGMENT),
        tuning::HAUL_CARRY_CAPACITY,
        "the load went through the plugged corridor"
    );
    assert!(game.world.get::<Stranded>(worker).is_none());
}

/// Squeezing past costs time, and exactly `SQUEEZE_EXTRA_TICKS` of it per
/// occupied cell — the same walk with the corridor clear is the baseline.
#[test]
fn squeezing_past_a_body_costs_the_extra_tick() {
    let (mut clear, _, clear_depot) = plugged_corridor(32, false);
    let (mut plugged, _, plugged_depot) = plugged_corridor(32, true);

    let baseline = ticks_to_delivery(&mut clear, clear_depot, 60);
    let squeezed = ticks_to_delivery(&mut plugged, plugged_depot, 60);

    assert_eq!(
        squeezed,
        baseline + tuning::SQUEEZE_EXTRA_TICKS as u32,
        "one occupied cell on the route costs exactly the squeeze"
    );
}

/// Deploys a Depot at `(x, 0)` with a Wall on each of its four faces —
/// nothing can stand beside it, so a load headed there is `Stranded` for
/// good.
fn walled_depot(game: &mut Game, x: i32) -> Entity {
    let depot = deploy(game, "depot", x, 0);
    for (dx, dy) in [(-1, 0), (1, 0), (0, 1), (0, -1)] {
        deploy(game, "wall", x + dx, dy);
    }
    depot
}

/// Floor out to the far Depots the stranding tests need, past the Home's slab.
fn lay_long_floor(game: &mut Game) {
    let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
    for x in -16..=16 {
        for y in -2..=3 {
            grid.lay_floor(x, y);
        }
    }
}

/// Boxes the Depot at `(12, 0)` in with four more machines. Far enough off
/// that the carrier is still walking when the last one goes up.
fn close_the_depot_in(game: &mut Game) {
    for (dx, dy) in [(11, 0), (13, 0), (12, 1), (12, -1)] {
        deploy(game, "mining_node", dx, dy);
    }
}

/// Picks the worker up holding a load, then closes the route: the structures
/// go up *after* the pickup, which is the only way a carrier is stranded now
/// that `Errand::Tend` lifts nothing it cannot deliver.
fn lose_the_route_mid_carry(game: &mut Game, worker: Entity, close: impl FnOnce(&mut Game)) {
    tick_until(game, 40, |g| g.world.get::<Carrying>(worker).is_some());
    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "precondition: the worker lifted a load"
    );
    close(game);
    tick_until(game, 40, |g| g.world.get::<Stranded>(worker).is_some());
}

/// **Nothing is lifted that cannot be delivered.** The only Depot is walled
/// in from the start, so `Errand::Tend` picks nothing up: no strand, no
/// set-down conveyor, and the machine fills and reads `Stranded` — steadily,
/// rather than flipping as a load is lifted and set down every
/// `STRANDED_SET_DOWN_TICKS`.
#[test]
fn a_walled_in_depot_is_never_lifted_toward() {
    let mut game = base(43);
    let node = deploy(&mut game, "mining_node", 0, 2);
    let depot = walled_depot(&mut game, 3);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    for _ in 0..3 * tuning::STRANDED_SET_DOWN_TICKS {
        game.tick();
        assert!(
            game.world.get::<Carrying>(worker).is_none(),
            "lifted a load toward a Depot nothing can reach, tick {}",
            game.current_tick()
        );
    }

    assert_eq!(node_output(&game, depot, ids::CORE_FRAGMENT), 0);
    assert_eq!(
        game.world.get::<MachineStatus>(node),
        Some(&MachineStatus::Stranded)
    );
}

/// **A machine cut off from every store that would take its output says so,
/// once, and recovers when a route reopens.** `Clogged` would send the
/// player to collect by hand for a cause that is a wall. One alert over the
/// whole episode (`set_machine_status` speaks on transition only), and
/// opening the wall lifts the status and delivers.
#[test]
fn a_machine_cut_off_from_its_depot_reads_stranded_once_and_recovers() {
    use crate::alerts::AlertKind;
    let mut game = base(43);
    let node = deploy(&mut game, "mining_node", 0, 2);
    let depot = walled_depot(&mut game, 3);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    for _ in 0..30 {
        game.tick();
    }
    assert_eq!(
        game.world.get::<MachineStatus>(node),
        Some(&MachineStatus::Stranded)
    );
    let stranded_alerts: u32 = game
        .alerts()
        .iter()
        .filter(|a| a.kind == AlertKind::MachineStalled(MachineStatus::Stranded))
        .map(|a| a.count)
        .sum();
    assert_eq!(stranded_alerts, 1, "one alert for the whole episode");

    let walls: Vec<Entity> = game
        .world
        .query::<(Entity, &Structure)>()
        .iter(&game.world)
        .filter(|(_, s)| s.kind == "wall")
        .map(|(e, _)| e)
        .collect();
    for wall in walls {
        game.world.despawn(wall);
    }
    tick_until(&mut game, 200, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });
    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) > 0,
        "delivered"
    );
    assert_ne!(
        game.world.get::<MachineStatus>(node),
        Some(&MachineStatus::Stranded)
    );
}

/// The same cut-off reading for a machine with an attached consumer: the
/// `Errand::Tend` gate lets only a clogged one through, and `Stranded` has to
/// count as clogged there or the status flaps back to `Clogged` and the
/// marker is cleared every other tick.
#[test]
fn a_cut_off_machine_with_a_consumer_beside_it_stays_stranded() {
    use crate::alerts::AlertKind;
    let mut game = base(44);
    lay_long_floor(&mut game);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let lathe = spawn_machine_at(&mut game, "lathe", 2, 0);
    walled_depot(&mut game, 4);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    let lathe_cap = capacity_of(&game, lathe);
    fill_output(&mut game, lathe, "blank_substrate", lathe_cap);
    game.queue_work_order(WorkOrder::batch(
        ItemId::from("blank_substrate"),
        lathe_cap + 5,
    ))
    .unwrap();
    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);

    for _ in 0..30 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<MachineStatus>(node),
        Some(&MachineStatus::Stranded)
    );
    let flips: u32 = game
        .alerts()
        .iter()
        .filter(|a| a.kind == AlertKind::MachineStalled(MachineStatus::Stranded))
        .map(|a| a.count)
        .sum();
    assert_eq!(flips, 1, "entered once, never flapped back out");
}

/// **A carrier stranded by structures drops its load on the floor after
/// `STRANDED_SET_DOWN_TICKS` and not before.** Both Depots are walled in, so
/// no walk reaches either; the load becomes a pile on the tile the carrier
/// stands on, nothing is deposited anywhere, and the carrier's hands are
/// empty — which is what lets the scheduler free it the way it frees anyone.
#[test]
fn a_carrier_stranded_by_structures_drops_its_load_on_the_floor() {
    let mut game = base(41);
    game.world
        .resource_mut::<crate::resources::BattleTelemetry>()
        .on = true;
    lay_long_floor(&mut game);
    let node = deploy(&mut game, "mining_node", 0, 2);
    let near = deploy(&mut game, "depot", 12, 0);
    let far = deploy(&mut game, "depot", -14, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);
    lose_the_route_mid_carry(&mut game, worker, |g| {
        for x in [12, -14] {
            for (dx, dy) in [(-1, 0), (1, 0), (0, 1), (0, -1)] {
                deploy(g, "wall", x + dx, dy);
            }
        }
    });
    let since = game
        .world
        .get::<Stranded>(worker)
        .expect("precondition: stranded holding a load")
        .since;

    while game.current_tick() < since + tuning::STRANDED_SET_DOWN_TICKS {
        assert!(
            game.world.get::<Carrying>(worker).is_some(),
            "dropped early, at tick {} of an episode from {since}",
            game.current_tick()
        );
        game.tick();
    }
    let at = *game.world.get::<Position>(worker).unwrap();
    tick_until(&mut game, 3, |g| g.world.get::<Carrying>(worker).is_none());

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "still holding the load past the timeout"
    );
    let pile = crate::game::base::floor::floor_pile_at(&mut game.world, at)
        .expect("a pile where the carrier stood");
    assert_eq!(
        game.world
            .get::<crate::components::FloorPile>(pile)
            .unwrap()
            .items
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied(),
        Some(tuning::HAUL_CARRY_CAPACITY)
    );
    assert_eq!(node_output(&game, near, ids::CORE_FRAGMENT), 0);
    assert_eq!(node_output(&game, far, ids::CORE_FRAGMENT), 0);
    assert!(
        !game
            .world
            .resource::<crate::resources::BattleTelemetry>()
            .records
            .iter()
            .any(|r| matches!(r, crate::telemetry::Record::Haul { errand, .. } if errand == "set_down")),
        "a drop is not a delivery and writes no haul record"
    );
}

/// **Nothing is destroyed to free a carrier, and nothing is deposited by
/// it.** Sealed in by Walls away from its machine, a stranded carrier drops
/// its load where it stands even with the one Depot full — the Depot is not
/// touched, the machine's buffer is not refilled, and the carrier is freed.
#[test]
fn a_stranded_carrier_drops_its_load_whatever_the_depots_hold() {
    let mut game = base(42);
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        for x in -4..=-2 {
            for y in -4..=-2 {
                grid.lay_floor(x, y);
            }
        }
    }
    let node = deploy(&mut game, "mining_node", 0, 2);
    let depot = deploy(&mut game, "depot", 3, 0);
    for x in -4..=-2 {
        for y in -4..=-2 {
            if (x, y) != (-3, -3) {
                deploy(&mut game, "wall", x, y);
            }
        }
    }
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    move_to(&mut game, worker, -3, -3);
    fill_to_capacity(&mut game, depot, ids::CORE_FRAGMENT);
    let load = Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: tuning::HAUL_CARRY_CAPACITY,
    };
    game.world.entity_mut(worker).insert(load.clone());
    let node_before = node_output(&game, node, ids::CORE_FRAGMENT);
    tick_until(&mut game, 10, |g| g.world.get::<Stranded>(worker).is_some());
    assert!(
        game.world.get::<Stranded>(worker).is_some(),
        "precondition: stranded"
    );

    for _ in 0..tuning::STRANDED_SET_DOWN_TICKS + 20 {
        game.tick();
    }

    assert!(game.world.get::<Carrying>(worker).is_none());
    let pile = crate::game::base::floor::floor_pile_at(&mut game.world, Position { x: -3, y: -3 })
        .expect("a pile where the carrier stood");
    assert_eq!(
        game.world
            .get::<crate::components::FloorPile>(pile)
            .unwrap()
            .items
            .get(&load.item)
            .copied(),
        Some(load.qty),
        "the load is on the floor whole"
    );
    assert_eq!(
        node_output(&game, depot, ids::CORE_FRAGMENT),
        capacity_of(&game, depot)
    );
    assert_eq!(node_output(&game, node, ids::CORE_FRAGMENT), node_before);
}

/// A hauler stranded with a load it cannot deliver is held on shift by
/// `Carrying` (freeing it would destroy the goods), and `Stranded` is never
/// cleared by anything but a route reopening — so once it also downed tools
/// it carried, and counted as on shift, for as long as the walls stood. Found
/// on the `chains` bench: seeds 2 and 5 read `on_shift_share` 1.0 beside a
/// 3.6%/5.0% `downed_tools` rung share. The drop is what clears it: the
/// load goes on the floor after `STRANDED_SET_DOWN_TICKS`, the hands
/// are empty, and the downed-tools rule takes it off shift.
#[test]
fn a_downed_tools_hauler_stranded_with_a_load_does_not_stay_on_shift() {
    let mut game = base(22);
    lay_long_floor(&mut game);
    let node = deploy(&mut game, "mining_node", 0, 2);
    deploy(&mut game, "depot", 12, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);
    lose_the_route_mid_carry(&mut game, worker, close_the_depot_in);
    tick_until(&mut game, 40, |g| {
        g.world.get::<Stranded>(worker).is_some() && g.world.get::<Carrying>(worker).is_some()
    });
    assert!(
        game.world.get::<Stranded>(worker).is_some()
            && game.world.get::<Carrying>(worker).is_some(),
        "precondition: stranded holding a load"
    );

    let now = game.current_tick();
    let mut n = 0;
    while game.morale(worker) > crate::tuning::MORALE_DOWNS_TOOLS_AT {
        game.world
            .get_mut::<Memories>(worker)
            .unwrap()
            .0
            .push(Memory {
                def: MemoryId::from("frayed_here"),
                subject: MemorySubject::BaseTile { x: n, y: 900 },
                subject_name: None,
                reinforced: now,
                strikes: 1,
            });
        n += 1;
        assert!(n < 400, "morale never reached the rung");
    }
    game.update_disgruntled(&[worker]);
    assert!(game.has_downed_tools(worker), "precondition: tools downed");

    for _ in 0..200 {
        game.tick();
    }

    assert!(game.has_downed_tools(worker), "still downed");
    assert!(
        !game.on_shift(worker),
        "a downed-tools program is still on shift, carrying {:?}, after 200 ticks",
        game.world.get::<Carrying>(worker)
    );
}

fn squeezing(into: (i32, i32), since: u64) -> Squeezing {
    Squeezing {
        into: Position {
            x: into.0,
            y: into.1,
        },
        since,
    }
}

fn squeeze_step(to: (i32, i32)) -> Step {
    Step {
        to: Position { x: to.0, y: to.1 },
        squeeze: true,
    }
}

/// A step into a free cell is taken at once, whatever marker is held.
#[test]
fn a_step_into_a_free_cell_is_taken_at_once() {
    let free = Step {
        to: Position { x: 1, y: 1 },
        squeeze: false,
    };
    assert_eq!(stride(None, free, 10), Stride::Go);
    assert_eq!(stride(Some(squeezing((1, 1), 9)), free, 10), Stride::Go);
}

/// A squeeze waits exactly `SQUEEZE_EXTRA_TICKS` on the same cell: the first
/// tick opens the wait, the one `EXTRA` later takes the step.
#[test]
fn a_squeeze_waits_the_extra_ticks_on_the_same_cell() {
    let step = squeeze_step((2, 0));
    let opened = match stride(None, step, 100) {
        Stride::Wait(m) => m,
        other => panic!("a squeeze opened with {other:?}"),
    };
    assert_eq!(opened, squeezing((2, 0), 100));
    assert_eq!(
        stride(Some(opened), step, 100),
        Stride::Wait(opened),
        "still paying for it on the tick it opened"
    );
    assert_eq!(
        stride(Some(opened), step, 100 + tuning::SQUEEZE_EXTRA_TICKS),
        Stride::Go
    );
}

/// A marker naming a different cell is a route that changed, so the wait
/// starts over rather than handing out a free pass.
#[test]
fn a_changed_route_restarts_the_squeeze_wait() {
    let held = squeezing((1, 0), 100);
    assert_eq!(
        stride(
            Some(held),
            squeeze_step((2, 0)),
            100 + tuning::SQUEEZE_EXTRA_TICKS
        ),
        Stride::Wait(squeezing((2, 0), 100 + tuning::SQUEEZE_EXTRA_TICKS))
    );
}

/// A marker older than twice the wait is left over from an interrupted walk:
/// it starts over too, where one just inside the window takes the step.
#[test]
fn a_stale_squeeze_marker_restarts_the_wait() {
    let step = squeeze_step((2, 0));
    let held = squeezing((2, 0), 100);
    let edge = 100 + 2 * tuning::SQUEEZE_EXTRA_TICKS;
    assert_eq!(stride(Some(held), step, edge), Stride::Go);
    assert_eq!(
        stride(Some(held), step, edge + 1),
        Stride::Wait(squeezing((2, 0), edge + 1))
    );
}

/// `Game::take_base_step`, the `Game`-side caller of `stride`: the dig crew
/// and the builder pay the squeeze through it. First call holds in place and
/// leaves the marker; a call `SQUEEZE_EXTRA_TICKS` later moves and clears it.
#[test]
fn a_game_side_walker_pays_the_squeeze_before_it_moves() {
    let mut game = base(51);
    let worker = hauler(&mut game);
    move_to(&mut game, worker, 0, 0);
    let start = *game.world.get::<Position>(worker).unwrap();
    let step = Step {
        to: Position {
            x: start.x + 1,
            y: start.y,
        },
        squeeze: true,
    };

    game.take_base_step(worker, step);
    assert_eq!(*game.world.get::<Position>(worker).unwrap(), start);
    assert!(game.world.get::<Squeezing>(worker).is_some());

    game.world.resource_mut::<GameClock>().tick += tuning::SQUEEZE_EXTRA_TICKS;
    game.take_base_step(worker, step);
    assert_eq!(*game.world.get::<Position>(worker).unwrap(), step.to);
    assert!(game.world.get::<Squeezing>(worker).is_none());
}

/// **An equal-length detour beats a squeeze.** Two ways round a two-wide
/// corridor, the straight one holding a body: the step taken is the free
/// diagonal. Fails if a body's cell cost nothing extra.
#[test]
fn the_walk_prefers_an_equal_length_detour_to_a_squeeze() {
    let mut grid = crate::base_grid::BaseGrid::default();
    for x in -3..=0 {
        grid.open(x, -1, 0);
        grid.open(x, -2, 0);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: -3, y: -2 };
    let blocked = blocked_tiles(std::iter::empty(), [Position { x: -2, y: -2 }].into_iter());

    assert_eq!(
        step_to_post(&grid, from, target, 1, &blocked, grid.radius()),
        Ok(Some(Step {
            to: Position { x: -2, y: -1 },
            squeeze: false,
        })),
        "the free way round costs the same and pays nothing"
    );
}

fn pile_on(game: &mut Game, x: i32, y: i32, item: &str, qty: u32) {
    crate::game::base::floor::spawn_floor_pile(
        &mut game.world,
        Position { x, y },
        ItemId::from(item),
        qty,
    );
}

fn pile_total(game: &mut Game) -> u32 {
    game.world
        .query::<&crate::components::FloorPile>()
        .iter(&game.world)
        .flat_map(|p| p.items.values().copied())
        .sum()
}

/// A node, a depot and a posted hauler on a clear tile apart from the pile.
/// The node's own mining is slow enough (10 ticks a unit) that the pile is
/// fetched while its output is still empty.
fn pickup_fixture(seed: u32) -> (Game, Entity, Entity, Entity) {
    let mut game = base(seed);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let depot = deploy(&mut game, "depot", 4, 0);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    (game, node, depot, worker)
}

#[test]
fn a_posted_hauler_brings_a_floor_pile_home_to_the_depot() {
    let (mut game, _, depot, _) = pickup_fixture(31);
    let (px, py) = game.base_pos().unwrap();
    pile_on(&mut game, px + 2, py + 2, "cache_grain", 3);

    tick_until(&mut game, 300, |g| node_output(g, depot, "cache_grain") > 0);

    assert_eq!(node_output(&game, depot, "cache_grain"), 3);
    assert_eq!(pile_total(&mut game), 0, "an emptied pile despawns");
}

#[test]
fn an_unreachable_pile_is_left_alone() {
    let (mut game, node, depot, _) = pickup_fixture(32);
    let (px, py) = game.base_pos().unwrap();
    // Far off the laid floor: no route reaches it.
    pile_on(&mut game, px + 90, py + 90, "cache_grain", 3);

    for _ in 0..120 {
        game.tick();
    }

    assert_eq!(node_output(&game, depot, "cache_grain"), 0);
    assert_eq!(pile_total(&mut game), 3);
    assert_ne!(
        game.world.get::<MachineStatus>(node),
        Some(&MachineStatus::Stranded),
        "a pile nobody can reach is not a reason to walk"
    );
}

#[test]
fn a_hauler_with_a_machine_to_clear_does_not_divert_to_a_pile() {
    let (mut game, node, depot, _) = pickup_fixture(33);
    let (px, py) = game.base_pos().unwrap();
    pile_on(&mut game, px + 2, py + 2, "cache_grain", 3);
    let cap = capacity_of(&game, node);
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);

    tick_until(&mut game, 300, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) > 0
    });

    assert!(node_output(&game, depot, ids::CORE_FRAGMENT) > 0);
    assert_eq!(
        pile_total(&mut game),
        3,
        "clearing the machine comes before the pile"
    );
}

#[test]
fn a_mixed_pile_is_lifted_lowest_item_first() {
    let (mut game, _, _, worker) = pickup_fixture(34);
    let (px, py) = game.base_pos().unwrap();
    pile_on(&mut game, px + 2, py + 2, ids::CORE_FRAGMENT, 2);
    pile_on(&mut game, px + 2, py + 2, "cache_grain", 3);

    tick_until(&mut game, 300, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });

    let load = game.world.get::<Carrying>(worker).expect("lifted a load");
    assert_eq!(load.item, ItemId::from("cache_grain"));
    assert_eq!(load.qty, 3);
}

#[test]
fn two_haulers_sent_to_one_pile_conserve_the_total() {
    let (mut game, node, depot, _) = pickup_fixture(35);
    let second = hauler(&mut game);
    game.assign_cronjob(second, node).unwrap();
    park_at_post(&mut game, second, node);
    let (px, py) = game.base_pos().unwrap();
    pile_on(&mut game, px + 2, py + 2, "cache_grain", 3);

    tick_until(&mut game, 400, |g| {
        node_output(g, depot, "cache_grain") >= 3
    });

    assert_eq!(node_output(&game, depot, "cache_grain"), 3);
    assert_eq!(pile_total(&mut game), 0);
}

/// A node, a hauler posted to it and only a walled-in Depot: the pile is
/// within reach but nothing it could be taken to is.
#[test]
fn a_pile_is_left_alone_when_the_only_accepting_depot_is_walled_in() {
    let mut game = base(36);
    let node = deploy(&mut game, "mining_node", 0, 2);
    walled_depot(&mut game, 3);
    let worker = hauler(&mut game);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    let (px, py) = game.base_pos().unwrap();
    pile_on(&mut game, px + 1, py + 3, "cache_grain", 3);

    for _ in 0..3 * tuning::STRANDED_SET_DOWN_TICKS {
        game.tick();
        assert!(
            game.world.get::<Carrying>(worker).is_none(),
            "lifted a pile no Depot can be reached for, tick {}",
            game.current_tick()
        );
    }
    assert_eq!(pile_total(&mut game), 3);
}

/// One pile the shelf refuses must not hide a farther one it takes.
#[test]
fn a_farther_pile_is_fetched_when_the_nearest_is_refused() {
    let (mut game, _, depot, _) = pickup_fixture(37);
    game.set_depot_filter(depot, &[ItemId::from(ids::CORE_FRAGMENT)], false);
    let (px, py) = game.base_pos().unwrap();
    pile_on(&mut game, px + 1, py + 1, ids::CORE_FRAGMENT, 2);
    pile_on(&mut game, px + 2, py + 2, "cache_grain", 4);

    tick_until(&mut game, 400, |g| {
        node_output(g, depot, "cache_grain") >= 4
    });

    assert_eq!(node_output(&game, depot, "cache_grain"), 4);
    assert_eq!(node_output(&game, depot, ids::CORE_FRAGMENT), 0);
}
