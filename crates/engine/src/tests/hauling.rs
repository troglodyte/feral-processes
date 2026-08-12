//! Programs that walk: taking a post, carrying a full buffer to a depot,
//! and coming back.

use super::support::*;
use crate::*;

/// A Home on the player's own tile — walkable by definition — plus enough
/// Core Fragments to deploy anything these fixtures need. The Home's slab
/// makes the whole build box walkable, so nothing here depends on the seed's
/// terrain.
fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    game
}

/// Deploys `kind` at the player's position plus `(dx, dy)` and returns it.
/// `place_structure` reports only success, so the entity is found by the
/// tile it must now be standing on.
fn deploy(game: &mut Game, kind: &str, dx: i32, dy: i32) -> Entity {
    game.place_structure(kind, dx, dy).unwrap();
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let (x, y) = (ppos.x + dx, ppos.y + dy);
    let mut query = game.world.query::<(Entity, &Position, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, p, _)| p.x == x && p.y == y)
        .map(|(e, ..)| e)
        .expect("the structure was just deployed")
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
    // Somewhere to take a load: with no depot there is no errand, which is
    // `with_no_depot_a_clogged_machine_just_stays_clogged` below.
    deploy(&mut game, "depot", 4, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);

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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    let far = deploy(&mut game, "depot", 5, 1);
    let near = deploy(&mut game, "depot", 2, 1);
    let worker = spawn_tamed(&mut game, 10, 3);
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

#[test]
fn a_depot_is_not_offered_as_a_cronjob() {
    let mut game = base(5);
    let depot = deploy(&mut game, "depot", 2, 0);
    let worker = spawn_tamed(&mut game, 10, 3);

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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    // which is the only thing `collect_adjacent` has ever asked about.
    let depot_pos = *game.world.get::<Position>(depot).unwrap();
    let player = game.player_entity();
    move_to(&mut game, player, depot_pos.x - 1, depot_pos.y);
    let taken = game.collect_adjacent();
    assert!(
        taken
            .iter()
            .any(|(id, n)| *id == ItemId::from(ids::CORE_FRAGMENT) && *n > 0),
        "collect_adjacent must work on a depot unchanged: {taken:?}"
    );
}

#[test]
fn a_posted_program_walks_to_its_machine_before_producing() {
    let mut game = base(7);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let node_pos = *game.world.get::<Position>(node).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    // The distance is the *player's*: a posted program sets off from
    // wherever you were standing when you posted it, so posting from five
    // tiles out is what buys the walk.
    let player = game.player_entity();
    move_to(&mut game, player, node_pos.x + 5, node_pos.y);
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
        game::hauling::at_station(*g.world.get::<Position>(worker).unwrap(), node_pos)
    });
    assert!(
        game::hauling::at_station(*game.world.get::<Position>(worker).unwrap(), node_pos),
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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    deploy(&mut game, "depot", 6, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
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
    deploy(&mut game, "depot", 6, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

    tick_until(&mut game, 200, |g| {
        g.world.get::<Carrying>(worker).is_some()
    });
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    let hp = game.world.get::<Durability>(node).unwrap().hp;
    game.damage_structure(node, hp, "Mining Node");

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
    let far = deploy(&mut game, "depot", 6, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
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
    deploy(&mut game, "depot", 6, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
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
    let node = deploy(&mut game, "mining_node", 2, 0);
    deploy(&mut game, "depot", 6, 0);
    let blocker = deploy(&mut game, "mining_node", 4, 0);
    deploy(&mut game, "mining_node", 4, -1);
    deploy(&mut game, "mining_node", 4, 1);
    let worker = spawn_tamed(&mut game, 10, 3);
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
        (4, 0),
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
    let worker = spawn_tamed(&mut game, 10, 3);

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
    let node = deploy(&mut game, "mining_node", 0, 2);
    deploy(&mut game, "depot", 4, 0);
    for (dx, dy) in [(3, 0), (5, 0), (4, 1), (4, -1)] {
        deploy(&mut game, "mining_node", dx, dy);
    }
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    fill_to_capacity(&mut game, node, ids::CORE_FRAGMENT);

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
    let worker = spawn_tamed(&mut game, 10, 3);
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
    let worker = spawn_tamed(&mut game, 10, 3);
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
