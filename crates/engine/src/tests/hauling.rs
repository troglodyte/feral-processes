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

/// Parks `worker` on the tile east of `structure`, which is a post it can
/// work from — otherwise the production gate holds it at `Unstaffed`.
fn park_at_post(game: &mut Game, worker: Entity, structure: Entity) {
    let pos = *game.world.get::<Position>(structure).unwrap();
    move_to(game, worker, pos.x + 1, pos.y);
}

fn fill_output(game: &mut Game, structure: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(structure).unwrap();
    stock.output.insert(ItemId::from(item), qty);
}

fn capacity_of(game: &Game, structure: Entity) -> u32 {
    game.world.get::<Stock>(structure).unwrap().capacity
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
