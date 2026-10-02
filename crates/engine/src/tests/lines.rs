//! Production lines: which machines feed which, and how a connected run
//! groups into one job.

use super::support::*;
use crate::game::base::lines::LineKey;
use crate::systems::{feeds, feeds_fuel, feeds_ingredient};
use crate::*;

fn def(game: &Game, id: &str) -> crate::structures::StructureDef {
    game.structure_defs()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("{id} should be a shipped structure"))
}

fn game() -> Game {
    Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

#[test]
fn a_mining_node_feeds_a_lathe_its_ingredient() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let (mining, lathe) = (def(&g, "mining_node"), def(&g, "lathe"));
    assert!(feeds_ingredient(&mining, &lathe, items));
    assert!(feeds(&mining, &lathe, items));
    assert!(!feeds_ingredient(&lathe, &mining, items));
}

#[test]
fn a_power_conduit_feeds_a_recharger_its_fuel() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let (conduit, recharger) = (def(&g, "power_conduit"), def(&g, "recharger_node"));
    assert!(feeds_fuel(&conduit, &recharger));
    assert!(!feeds_ingredient(&conduit, &recharger, items));
    assert!(feeds(&conduit, &recharger, items));
}

#[test]
fn a_machine_does_not_feed_its_own_kind() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let mining = def(&g, "mining_node");
    assert!(!feeds(&mining, &mining, items));
}

#[test]
fn a_teardown_rig_feeds_nothing() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let rig = def(&g, "teardown_rig");
    for other in g.structure_defs() {
        assert!(
            !feeds(&rig, &other, items),
            "rig must not feed {}",
            other.id
        );
    }
}

#[test]
fn two_touching_mining_nodes_are_two_lines() {
    let mut g = game();
    let a = spawn_machine_at(&mut g, "mining_node", 0, 0);
    let b = spawn_machine_at(&mut g, "mining_node", 1, 0);
    let lines = g.production_lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(g.line_of(a), None);
    assert_eq!(g.line_of(b), None);
}

#[test]
fn a_lathe_beside_a_mining_node_ranks_the_lathe_first() {
    let mut g = game();
    let mining = spawn_machine_at(&mut g, "mining_node", 0, 0);
    let lathe = spawn_machine_at(&mut g, "lathe", 1, 0);
    let lines = g.production_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].members, vec![lathe, mining]);
    assert_eq!(lines[0].rank, vec![0, 1]);
    assert_eq!(lines[0].key, LineKey((0, 0)));
    assert_eq!(g.line_of(lathe), Some(LineKey((0, 0))));
    assert_eq!(g.line_of(mining), Some(LineKey((0, 0))));
}

#[test]
fn a_conduit_and_a_recharger_are_one_line() {
    let mut g = game();
    let conduit = spawn_machine_at(&mut g, "power_conduit", 4, 4);
    let recharger = spawn_machine_at(&mut g, "recharger_node", 4, 5);
    let lines = g.production_lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].members, vec![recharger, conduit]);
    assert_eq!(g.line_of(recharger), Some(LineKey((4, 4))));
}

#[test]
fn a_rig_beside_a_machine_is_a_line_of_one() {
    let mut g = game();
    let rig = spawn_machine_at(&mut g, "teardown_rig", 0, 0);
    spawn_machine_at(&mut g, "lathe", 1, 0);
    assert_eq!(g.line_of(rig), None);
}

#[test]
fn building_between_two_lines_merges_them_and_demolishing_splits_them() {
    let mut g = game();
    let compiler = spawn_machine_at(&mut g, "compiler", 0, 0);
    let lathe = spawn_machine_at(&mut g, "lathe", 2, 0);
    let far = spawn_machine_at(&mut g, "mining_node", 3, 0);
    assert_eq!(g.line_of(compiler), None);
    let key = g.line_of(lathe);
    assert_eq!(key, Some(LineKey((2, 0))));
    assert_eq!(g.line_of(far), key);

    let between = spawn_machine_at(&mut g, "mining_node", 1, 0);
    let merged = g.line_of(compiler);
    assert_eq!(merged, Some(LineKey((0, 0))));
    for e in [lathe, far, between] {
        assert_eq!(g.line_of(e), merged);
    }
    assert_eq!(g.production_lines().len(), 1);

    g.world.despawn(between);
    assert_eq!(g.line_of(compiler), None);
    assert_eq!(g.line_of(lathe), Some(LineKey((2, 0))));
}

#[test]
fn a_cycle_neither_panics_nor_reorders() {
    let mut g = game();
    let es: Vec<_> = (0..3).map(|_| g.world.spawn_empty().id()).collect();
    let nodes: Vec<_> = es
        .iter()
        .enumerate()
        .map(|(i, e)| (*e, (i as i32, 0)))
        .collect();
    // 0 -> 1 -> 2 -> 0
    let edge = |a: usize, b: usize| (a + 1) % 3 == b;
    let first = crate::game::base::lines::group(&nodes, edge);
    let second = crate::game::base::lines::group(&nodes, edge);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].members.len(), 3);
    assert_eq!(first[0].members, second[0].members);
    assert_eq!(first[0].rank, second[0].rank);
}

// ---------------------------------------------------------------------
// Standing jobs reach the line
// ---------------------------------------------------------------------

#[test]
fn a_standing_job_on_one_member_reaches_the_whole_line() {
    let mut g = game();
    let mining = spawn_machine_at(&mut g, "mining_node", 0, 0);
    let lathe = spawn_machine_at(&mut g, "lathe", 1, 0);
    g.set_standing_job(lathe, true, true).unwrap();
    assert_eq!(g.standing_job(lathe), Some((true, true)));
    assert_eq!(
        g.standing_job(mining),
        Some((true, false)),
        "guard stays per structure"
    );
    g.set_standing_job(mining, false, false).unwrap();
    assert_eq!(g.standing_job(mining), None);
    assert_eq!(g.standing_job(lathe), Some((false, true)));
}

#[test]
fn a_standing_job_on_a_line_of_one_is_as_before() {
    let mut g = game();
    let mining = spawn_machine_at(&mut g, "mining_node", 0, 0);
    let other = spawn_machine_at(&mut g, "mining_node", 1, 0);
    g.set_standing_job(mining, true, false).unwrap();
    assert_eq!(g.standing_job(other), None);
}

#[test]
fn a_merged_line_with_one_flagged_member_wants_every_member() {
    let mut g = game();
    let compiler = spawn_machine_at(&mut g, "compiler", 0, 0);
    let lathe = spawn_machine_at(&mut g, "lathe", 2, 0);
    let far = spawn_machine_at(&mut g, "mining_node", 3, 0);
    g.set_standing_job(lathe, true, false).unwrap();
    assert_eq!(g.standing_job(far), Some((true, false)));
    assert_eq!(g.standing_job(compiler), None);

    let between = spawn_machine_at(&mut g, "mining_node", 1, 0);
    let lines = g.production_lines();
    let mut wanted: Vec<Entity> = g
        .standing_wants(&lines)
        .into_iter()
        .map(|(e, kind)| {
            assert_eq!(kind, TaskKind::GatherResource);
            e
        })
        .collect();
    wanted.sort();
    let mut all = vec![compiler, lathe, far, between];
    all.sort();
    assert_eq!(wanted, all);
}

// ---------------------------------------------------------------------
// Staffing: one worker, pulling from the end
// ---------------------------------------------------------------------

fn base_game(seed: u32) -> Game {
    let mut g = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut g);
    place_home(&mut g);
    g
}

/// Mining (2,0) -> Lathe (3,0) -> Disk Press (4,0).
fn disk_line(g: &mut Game) -> (Entity, Entity, Entity) {
    let mine = spawn_machine_at(g, "mining_node", 2, 0);
    let lathe = spawn_machine_at(g, "lathe", 3, 0);
    let press = spawn_machine_at(g, "disk_press", 4, 0);
    (mine, lathe, press)
}

fn put_output(g: &mut Game, machine: Entity, item: &str, qty: u32) {
    let mut stock = g.world.get_mut::<Stock>(machine).unwrap();
    *stock.output.entry(ItemId::from(item)).or_default() += qty;
}

fn hire(g: &mut Game, n: usize) -> Vec<Entity> {
    let mut staff: Vec<Entity> = (0..n).map(|_| spawn_tamed(g, 10, 3)).collect();
    staff.sort();
    staff
}

/// Every body holding a `GatherResource` task, with its target.
fn gatherers(g: &mut Game) -> Vec<(Entity, Entity)> {
    let mut query = g.world.query::<(Entity, &Task)>();
    let mut held: Vec<_> = query
        .iter(&g.world)
        .filter(|(_, t)| t.kind == TaskKind::GatherResource)
        .map(|(e, t)| (e, t.target))
        .collect();
    held.sort();
    held
}

fn posted(g: &Game, worker: Entity) -> Option<Entity> {
    g.world.get::<Task>(worker).map(|t| t.target)
}

#[test]
fn a_three_machine_line_under_a_standing_job_has_one_body_and_two_free() {
    let mut g = base_game(61);
    let (mine, lathe, _press) = disk_line(&mut g);
    let staff = hire(&mut g, 3);
    g.set_standing_job(mine, true, false).unwrap();
    // Both downstream machines can progress, so three members want a body.
    put_output(&mut g, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut g, lathe, ids::BLANK_SUBSTRATE, 8);
    g.tick();
    let held = gatherers(&mut g);
    assert_eq!(held.len(), 1, "one body runs the line: {held:?}");
    assert_eq!(
        staff.iter().filter(|&&s| posted(&g, s).is_none()).count(),
        2
    );
}

#[test]
fn the_worker_pulls_from_the_end_of_the_line() {
    let mut g = base_game(62);
    let (mine, lathe, _press) = disk_line(&mut g);
    let staff = hire(&mut g, 1);
    g.set_standing_job(mine, true, false).unwrap();
    put_output(&mut g, mine, ids::CORE_FRAGMENT, 8);
    g.tick();
    assert_eq!(posted(&g, staff[0]), Some(lathe));
}

#[test]
fn an_order_for_a_middle_product_never_posts_the_end_machine() {
    let mut g = base_game(63);
    let (mine, lathe, press) = disk_line(&mut g);
    let staff = hire(&mut g, 3);
    put_output(&mut g, mine, ids::CORE_FRAGMENT, 8);
    // The press could run, which is what makes this a test of the wanted
    // set rather than of what can progress.
    put_output(&mut g, lathe, ids::BLANK_SUBSTRATE, 8);
    g.queue_work_order(WorkOrder::batch(ItemId::from(ids::BLANK_SUBSTRATE), 40))
        .unwrap();
    for _ in 0..3 {
        g.tick();
        let held = gatherers(&mut g);
        assert_eq!(held.len(), 1, "{held:?}");
        assert!(held.iter().all(|&(_, target)| target != press));
    }
    assert!(staff.iter().filter(|&&s| posted(&g, s).is_some()).count() == 1);
}

/// Posts the line's one worker on the Mining Node mid-cycle, with the Lathe
/// not yet workable, and returns `(worker, mine, lathe)`.
fn mid_cycle_on_mining(g: &mut Game, staff: usize) -> (Entity, Entity, Entity) {
    let mine = spawn_machine_at(g, "mining_node", 2, 0);
    let lathe = spawn_machine_at(g, "lathe", 3, 0);
    hire(g, staff);
    g.set_standing_job(mine, true, false).unwrap();
    g.tick();
    let (worker, target) = gatherers(g)[0];
    assert_eq!(target, mine, "precondition: only the mine can progress");
    park_at_post(g, worker, mine);
    for _ in 0..200 {
        let t = g.world.get::<Task>(worker).unwrap();
        if t.progress > 0 {
            break;
        }
        g.tick();
    }
    let t = g.world.get::<Task>(worker).unwrap();
    assert!(
        0 < t.progress && t.progress + 2 < t.required,
        "precondition: mid-cycle with ticks to spare, got {}/{}",
        t.progress,
        t.required
    );
    (worker, mine, lathe)
}

#[test]
fn a_worker_mid_cycle_is_not_moved_and_moves_when_the_cycle_completes() {
    let mut g = base_game(64);
    let (worker, mine, lathe) = mid_cycle_on_mining(&mut g, 2);
    // The lathe becomes workable while the mine is mid-cycle.
    put_output(&mut g, mine, ids::CORE_FRAGMENT, 8);
    while g.world.get::<Task>(worker).is_some_and(|t| t.progress != 0) {
        let t = g.world.get::<Task>(worker).unwrap();
        assert_eq!(t.target, mine, "moved mid-cycle at {}", t.progress);
        g.tick();
    }
    g.tick();
    assert_eq!(posted(&g, worker), Some(lathe), "moves once the cycle ends");
}

#[test]
fn the_same_body_is_re_posted_when_the_active_machine_moves() {
    let mut g = base_game(65);
    let (worker, mine, lathe) = mid_cycle_on_mining(&mut g, 3);
    put_output(&mut g, mine, ids::CORE_FRAGMENT, 8);
    for _ in 0..200 {
        g.tick();
        if posted(&g, worker) == Some(lathe) {
            break;
        }
    }
    assert_eq!(
        gatherers(&mut g),
        vec![(worker, lathe)],
        "no swap, no second body"
    );
}

#[test]
fn a_carrying_line_worker_keeps_the_lines_want() {
    let mut g = base_game(66);
    let mine = spawn_machine_at(&mut g, "mining_node", 2, 0);
    spawn_machine_at(&mut g, "lathe", 3, 0);
    let staff = hire(&mut g, 2);
    g.set_standing_job(mine, true, false).unwrap();
    g.tick();
    let (worker, _) = gatherers(&mut g)[0];
    let other = *staff.iter().find(|&&s| s != worker).unwrap();
    g.world.entity_mut(worker).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 1,
    });
    // The line's want now names the lathe, not the mine the body stands on.
    put_output(&mut g, mine, ids::CORE_FRAGMENT, 8);
    g.tick();
    assert_eq!(posted(&g, worker), Some(mine), "a carrier is never freed");
    assert!(g.world.get::<Carrying>(worker).is_some());
    assert_eq!(posted(&g, other), None, "and the line is not staffed twice");
}

#[test]
fn a_line_whose_end_machine_lacks_an_ingredient_is_fetched_by_its_one_worker() {
    let mut g = base_game(67);
    // Lathe -> Press: nothing in the line makes the lathe's core fragment.
    let lathe = spawn_machine_at(&mut g, "lathe", 1, 0);
    let press = spawn_machine_at(&mut g, "disk_press", 2, 0);
    let depot = spawn_machine_at(&mut g, "depot", 4, 0);
    put_output(&mut g, depot, ids::CORE_FRAGMENT, 10);
    let staff = hire(&mut g, 2);
    g.set_standing_job(press, true, false).unwrap();
    g.tick();
    let (worker, target) = gatherers(&mut g)[0];
    assert_eq!(target, lathe, "the one machine that can progress");
    park_at_post(&mut g, worker, lathe);
    for _ in 0..300 {
        g.tick();
        let held = g
            .world
            .get::<Stock>(lathe)
            .unwrap()
            .input
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0);
        if held > 0 {
            break;
        }
    }
    let held = g.world.get::<Stock>(lathe).unwrap();
    assert!(
        held.input.contains_key(&ItemId::from(ids::CORE_FRAGMENT)) || !held.output.is_empty(),
        "the lathe was loaded from the depot"
    );
    assert_eq!(gatherers(&mut g).len(), 1);
    assert_eq!(
        staff.iter().filter(|&&s| posted(&g, s).is_some()).count(),
        1
    );
}
