//! The Weapons research tree: one node per weapon part and per capacity
//! step, synthesised from the Rust catalogue (ADR 0031), the routine tree's
//! counterpart. A node id is the part's own item id, so "researched" means
//! `Research` contains `weapon_part/<key>`.

use crate::gear_craft::weapon::Category;
use crate::gear_craft::{WeaponPart, capacity_node_id};
use crate::research::{ResearchDef, ResearchTree};
use crate::routine_craft::Version;

/// Which scope rung `part` prices at: a frame is the base, a modifier one
/// step up, a payoff another.
fn scope_rank(part: WeaponPart) -> u8 {
    match part.category() {
        Category::Frame => 0,
        Category::Exploit => 2,
        _ => 1,
    }
}

fn node(
    id: String,
    discoverable: bool,
    name: String,
    description: String,
    cost: u32,
    zone: u32,
    requires: Vec<String>,
) -> ResearchDef {
    ResearchDef {
        id,
        name,
        description,
        cost,
        materials: Vec::new(),
        min_zone: zone,
        requires,
        recommended: false,
        unlocks_structures: Vec::new(),
        unlocks_recipes: Vec::new(),
        unlocks_tools: Vec::new(),
        tree: ResearchTree::Weapons,
        opens_routine_tree: false,
        requires_subject: false,
        discoverable,
        unlocks_fusion: false,
        opens_affix_tree: false,
        decompiler: None,
    }
}

/// What `part`'s node needs first: a frame version needs the one below, and
/// an Exploit needs the Rider that sets up the same status (Stun has no
/// Rider, so its Exploit stands alone).
fn requires(part: WeaponPart) -> Vec<String> {
    match part {
        WeaponPart::Frame(frame, Version::V2) => {
            vec![WeaponPart::Frame(frame, Version::V1).node_id()]
        }
        WeaponPart::Frame(frame, Version::V3) => {
            vec![WeaponPart::Frame(frame, Version::V2).node_id()]
        }
        WeaponPart::Exploit(status) => {
            let rider = WeaponPart::Rider(status);
            if WeaponPart::all().contains(&rider) {
                vec![rider.node_id()]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// One node per part and one per capacity step. Visibility is the zone gate
/// and the discovery gate (v1 frames and capacity steps skip the latter); the cost's zone term shares the zone.
pub fn synthesise_nodes() -> Vec<ResearchDef> {
    let mut nodes: Vec<ResearchDef> = WeaponPart::all()
        .into_iter()
        .map(|part| {
            let version = match part {
                WeaponPart::Frame(_, v) => v.number(),
                _ => 1,
            };
            node(
                part.node_id(),
                // The first frames show when the tree opens; every other
                // part is found by a study, a salvage or a shelf.
                !matches!(part, WeaponPart::Frame(_, Version::V1)),
                part.label(),
                format!(
                    "Learn to craft the {} part for weapons designed at the Workbench.",
                    part.label()
                ),
                crate::tuning::weapon_research_cost(scope_rank(part), version, version),
                version,
                requires(part),
            )
        })
        .collect();
    let start = crate::tuning::GEAR_PART_CAP_START;
    for cap in (start + 1)..=crate::tuning::GEAR_PART_CAP_MAX {
        let step = (cap - start) as u32;
        let zone = crate::tuning::GEAR_CAPACITY_FIRST_ZONE + step - 1;
        nodes.push(node(
            capacity_node_id(cap),
            false,
            format!("Weapon capacity {cap}"),
            format!("A weapon design may hold {cap} modifiers."),
            crate::tuning::weapon_research_cost(2, 1, zone),
            zone,
            if cap > start + 1 {
                vec![capacity_node_id(cap - 1)]
            } else {
                Vec::new()
            },
        ));
    }
    nodes
}
