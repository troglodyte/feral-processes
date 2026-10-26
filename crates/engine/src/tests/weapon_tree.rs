//! The Weapons research tree (ADR 0031, phase 3): its gate, its nodes, and
//! what a study may discover once it is open.

use super::support::*;
use crate::gear_craft::capacity_node_id;
use crate::gear_craft::weapon::{Frame, WeaponPart};
use crate::research::{ResearchDb, ResearchTree};
use crate::routine_craft::{StatusKind, Version};
use crate::*;

fn listed(game: &Game) -> Vec<String> {
    game.research_nodes(ResearchTree::Weapons)
        .into_iter()
        .map(|n| n.id)
        .collect()
}

fn game_in_base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    game
}

fn edge_v1() -> WeaponPart {
    WeaponPart::Frame(Frame::Edge, Version::V1)
}

#[test]
fn the_tree_is_closed_before_weapon_bench_and_open_after() {
    let mut game = game_in_base(72_001);
    assert!(!game.weapon_tree_open());
    assert!(listed(&game).is_empty(), "a closed tree lists nothing");
    let err = game.select_research(&edge_v1().node_id()).unwrap_err();
    assert_eq!(err, "Unknown research.");

    unlock_research_chain(&mut game, "weapon_bench");
    assert!(game.weapon_tree_open());
    let bleed = WeaponPart::Rider(StatusKind::Bleed).node_id();
    let open_with = listed(&game);
    assert!(
        open_with.contains(&edge_v1().node_id()),
        "an opened tree shows its v1 frames without a study"
    );
    assert!(
        !open_with.contains(&bleed),
        "a modifier waits for a study, a salvage or a shelf"
    );
    game.discover_research(&bleed);
    assert!(listed(&game).contains(&bleed));
}

#[test]
fn every_weapon_part_and_capacity_step_has_exactly_one_node() {
    let game = game_in_base(72_002);
    let db = game.world.resource::<ResearchDb>();
    let nodes: Vec<_> = db
        .all()
        .filter(|d| d.tree == ResearchTree::Weapons)
        .collect();
    let ids: std::collections::BTreeSet<String> = nodes.iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids.len(), nodes.len(), "no id twice");
    let mut expected: std::collections::BTreeSet<String> =
        WeaponPart::all().iter().map(|p| p.node_id()).collect();
    for cap in (crate::tuning::GEAR_PART_CAP_START + 1)..=crate::tuning::GEAR_PART_CAP_MAX {
        expected.insert(capacity_node_id(cap));
    }
    assert_eq!(ids, expected);
    // The first frames and the capacity steps are the tree's front door;
    // every other node is found through study, salvage or shelves.
    let v1_frames: std::collections::BTreeSet<String> = WeaponPart::all()
        .into_iter()
        .filter(|p| matches!(p, WeaponPart::Frame(_, Version::V1)))
        .map(|p| p.node_id())
        .collect();
    for d in &nodes {
        let front_door = d.id.starts_with("weapon_capacity/") || v1_frames.contains(&d.id);
        assert_eq!(d.discoverable, !front_door, "{}", d.id);
    }
}

#[test]
fn frames_chain_by_version_and_an_exploit_needs_its_riders_node() {
    let game = game_in_base(72_003);
    let db = game.world.resource::<ResearchDb>();
    let v2 = db
        .get(&WeaponPart::Frame(Frame::Lance, Version::V2).node_id())
        .unwrap();
    assert_eq!(
        v2.requires,
        vec![WeaponPart::Frame(Frame::Lance, Version::V1).node_id()]
    );
    assert_eq!(v2.min_zone, 2);
    let exploit = db
        .get(&WeaponPart::Exploit(StatusKind::Bleed).node_id())
        .unwrap();
    assert_eq!(
        exploit.requires,
        vec![WeaponPart::Rider(StatusKind::Bleed).node_id()]
    );
    let stun = db
        .get(&WeaponPart::Exploit(StatusKind::Stun).node_id())
        .unwrap();
    assert!(stun.requires.is_empty(), "stun has no rider to follow");
}

#[test]
fn a_study_discovers_weapon_nodes_only_when_the_tree_is_open() {
    let mut game = game_in_base(72_004);
    let weapon_ids = |game: &Game| -> Vec<String> {
        game.eligible_discoveries()
            .into_iter()
            .filter(|id| id.starts_with("weapon_"))
            .collect()
    };
    assert!(weapon_ids(&game).is_empty());
    unlock_research_chain(&mut game, "weapon_bench");
    let pool = weapon_ids(&game);
    assert!(
        pool.contains(&WeaponPart::Rider(StatusKind::Bleed).node_id()),
        "{pool:?}"
    );
    assert!(
        !pool.contains(&edge_v1().node_id()),
        "a v1 frame is visible from the start, so there is nothing to find"
    );
    assert!(
        !pool.contains(&WeaponPart::Frame(Frame::Edge, Version::V2).node_id()),
        "a frame's second version waits on its first"
    );
    assert!(
        !pool.contains(&WeaponPart::Exploit(StatusKind::Bleed).node_id()),
        "an exploit waits on its rider"
    );
}

#[test]
fn capacity_research_raises_the_design_cap_step_by_step() {
    let mut game = game_in_base(72_005);
    unlock_research_chain(&mut game, "weapon_bench");
    discover_all_research(&mut game);
    assert_eq!(game.gear_part_cap(), crate::tuning::GEAR_PART_CAP_START);
    unlock_research_chain(
        &mut game,
        &capacity_node_id(crate::tuning::GEAR_PART_CAP_START + 1),
    );
    assert_eq!(game.gear_part_cap(), crate::tuning::GEAR_PART_CAP_START + 1);
}

#[test]
fn the_opener_is_named_from_its_research_node() {
    let game = game_in_base(72_010);
    let def = game
        .research_defs()
        .into_iter()
        .find(|d| d.id == crate::tuning::WEAPON_TREE_OPENER)
        .expect("the opener ships as a base node");
    assert_eq!(game.weapon_tree_opener_name(), def.name);
}
