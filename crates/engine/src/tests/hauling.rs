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
    // any cost field a walk could build, so it never arrives.
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
        MachineStatus::Unstaffed,
    );
}

#[test]
fn unstaffed_wins_over_running() {
    let mut game = base(3);
    let node = deploy(&mut game, "mining_node", 1, 0);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    move_to(&mut game, worker, 400, 400);
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
#[test]
fn a_worker_delivers_to_the_nearer_of_two_depots() {
    let mut game = base(4);
    let far = deploy(&mut game, "depot", 6, 0);
    let near = deploy(&mut game, "depot", 2, 0);

    let depots = vec![
        (far, *game.world.get::<Position>(far).unwrap()),
        (near, *game.world.get::<Position>(near).unwrap()),
    ];
    let from = *game.world.get::<Position>(near).unwrap();

    let (chosen, _) = game::hauling::nearest_depot(&depots, from).unwrap();
    assert_eq!(chosen, near);
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
    move_to(&mut game, worker, node_pos.x + 5, node_pos.y);
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
    fill_output(&mut game, node, ids::CORE_FRAGMENT, cap);

    tick_until(&mut game, 600, |g| {
        node_output(g, depot, ids::CORE_FRAGMENT) >= cap
    });

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
