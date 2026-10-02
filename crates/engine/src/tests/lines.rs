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
