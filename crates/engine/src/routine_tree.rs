//! The routine research tree: one node per Subroutine part and per capacity
//! step, synthesised from the Rust catalogue (ADR 0029), plus the one
//! routine permanence rule.

use crate::research::{ResearchDef, ResearchTree};
use crate::routine_craft::{Subroutine, Version};

/// Whether `id` is welded permanently into its slot — `decompile` alone.
/// Keyed to `abilities::DECOMPILE_ABILITY_ID` rather than an
/// asset-authored flag, for `Game::routine_is_permanent`'s own reason:
/// permanence is not a property a file may claim. `Game::routine_is_permanent`
/// calls this rather than restating it, so there is one definition.
pub(crate) fn is_permanent(id: &str) -> bool {
    id == crate::abilities::DECOMPILE_ABILITY_ID
}

/// The research node id of `part`: researched, the player may craft it.
pub fn part_node_id(part: &Subroutine) -> String {
    format!("part/{}", part.key())
}

/// The research node id of the capacity node that raises the part cap to
/// `cap`.
pub fn capacity_node_id(cap: usize) -> String {
    format!("capacity/{cap}")
}

fn part_node(
    id: String,
    name: String,
    description: String,
    scope: u8,
    version: u32,
    zone: u32,
    requires: Vec<String>,
    discoverable: bool,
) -> ResearchDef {
    ResearchDef {
        id,
        name,
        description,
        cost: crate::tuning::routine_research_cost(scope, (version, 0), zone),
        materials: Vec::new(),
        min_zone: zone,
        requires,
        recommended: false,
        unlocks_structures: Vec::new(),
        unlocks_recipes: Vec::new(),
        unlocks_tools: Vec::new(),
        tree: ResearchTree::Routines,
        opens_routine_tree: false,
        requires_subject: false,
        discoverable,
        unlocks_fusion: false,
        opens_affix_tree: false,
        decompiler: None,
    }
}

/// One node per part and one per capacity step, synthesised from the Rust
/// catalogue (ADR 0029): a Core version needs the version below it, a
/// modifier stands alone, and each capacity node needs the one before.
/// A part node is hidden until found (ADR 0041) except the starting parts;
/// a capacity node belongs to no program, so it is listed by zone alone.
pub fn synthesise_part_nodes() -> Vec<ResearchDef> {
    let mut nodes: Vec<ResearchDef> = Subroutine::all()
        .into_iter()
        .map(|part| {
            let (scope, version, requires) = match part {
                Subroutine::Core(core, v) => (
                    0,
                    v.number(),
                    match v {
                        Version::V1 => Vec::new(),
                        Version::V2 => vec![part_node_id(&Subroutine::Core(core, Version::V1))],
                        Version::V3 => vec![part_node_id(&Subroutine::Core(core, Version::V2))],
                    },
                ),
                _ => (1, 1, Vec::new()),
            };
            part_node(
                part_node_id(&part),
                part.label(),
                format!(
                    "Learn to craft the {} part for routines built at the Workbench.",
                    part.label()
                ),
                scope,
                version,
                version,
                requires,
                !crate::tuning::STARTING_PARTS.contains(&part),
            )
        })
        .collect();
    let start = crate::tuning::CRAFT_PART_CAP_START;
    for cap in (start + 1)..=crate::tuning::CRAFT_PART_CAP_MAX {
        let step = (cap - start) as u32;
        let requires = if cap > start + 1 {
            vec![capacity_node_id(cap - 1)]
        } else {
            Vec::new()
        };
        nodes.push(part_node(
            capacity_node_id(cap),
            format!("Routine capacity {cap}"),
            format!("A built routine may hold {cap} parts."),
            2,
            1,
            crate::tuning::CRAFT_CAPACITY_FIRST_ZONE + step - 1,
            requires,
            false,
        ));
    }
    nodes
}

/// The parts of `parts` a program carrying them could let you find now
/// (ADR 0041), in `Subroutine` order with duplicates collapsed. A carried
/// Core offers its lowest unknown version whose rung below is `known`, so a
/// find never skips a rung; a modifier has no rung. `eligible` answers the
/// node's own terms (hidden, unfound, unresearched, inside the zone gate).
/// Read once, so nothing found from this call makes another findable.
pub fn findable_parts(
    parts: &[Subroutine],
    known: impl Fn(&str) -> bool,
    eligible: impl Fn(&str) -> bool,
) -> Vec<Subroutine> {
    let mut found = std::collections::BTreeSet::new();
    for part in parts {
        let rungs: Vec<Subroutine> = match *part {
            Subroutine::Core(core, top) => Version::ALL[..=top.index()]
                .iter()
                .map(|&v| Subroutine::Core(core, v))
                .collect(),
            other => vec![other],
        };
        let mut below_known = true;
        for rung in rungs {
            let id = part_node_id(&rung);
            if below_known && eligible(&id) {
                found.insert(rung);
            }
            below_known = known(&id);
        }
    }
    found.into_iter().collect()
}

/// Which of `findable` an extraction finds: one draw at `chance` per part,
/// in order, and no draw at all for an empty list.
pub fn roll_part_finds(
    findable: &[Subroutine],
    chance: f64,
    rng: &mut rand::rngs::StdRng,
) -> Vec<Subroutine> {
    use rand::RngExt;
    findable
        .iter()
        .copied()
        .filter(|_| rng.random_bool(chance))
        .collect()
}
