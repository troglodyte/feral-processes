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
        discoverable: false,
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
            let node = part_node(
                part_node_id(&part),
                part.label(),
                crate::routine_craft::part_effect(&part),
                scope,
                version,
                version,
                requires,
            );
            ResearchDef {
                discoverable: !crate::tuning::STARTING_PARTS.contains(&part),
                ..node
            }
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

/// Every part some shipped program can carry, a Core's lower rungs included
/// since `findable_parts` walks down to them: a species' kit and the wild
/// pool. An exclusive routine is not carried: no program ever has one, it
/// only drops from a boss or sells at a trader (`exclusive_only_parts`). A
/// discoverable part outside it has no carrier, so a study of any subject
/// may find it (ADR 0041).
pub fn program_carried_parts(
    abilities: &crate::abilities::AbilityDb,
    species: &crate::species::SpeciesDb,
) -> std::collections::BTreeSet<Subroutine> {
    let mut ids: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    ids.extend(
        abilities
            .wild_pool()
            .into_iter()
            .map(|(d, _)| d.id.as_str()),
    );
    for def in species.all() {
        ids.extend(def.abilities.iter().map(|a| a.id.as_str()));
    }
    parts_with_rungs(abilities, ids)
}

/// The parts only an exclusive routine makes (lower rungs included) that
/// `program_carried_parts` does not contain. They are found from the drop or
/// the trade that pays them, or by a study at a lower weight.
pub fn exclusive_only_parts(
    abilities: &crate::abilities::AbilityDb,
    species: &crate::species::SpeciesDb,
) -> std::collections::BTreeSet<Subroutine> {
    let carried = program_carried_parts(abilities, species);
    let ids = abilities
        .exclusive_pool()
        .into_iter()
        .map(|d| d.id.as_str())
        .collect();
    parts_with_rungs(abilities, ids)
        .into_iter()
        .filter(|p| !carried.contains(p))
        .collect()
}

fn parts_with_rungs(
    abilities: &crate::abilities::AbilityDb,
    ids: std::collections::BTreeSet<&str>,
) -> std::collections::BTreeSet<Subroutine> {
    let carried: Vec<Subroutine> = ids
        .into_iter()
        .filter_map(|id| abilities.get(id).and_then(crate::routine_craft::decompose))
        .flatten()
        .collect();
    findable_parts(&carried, |_| true, |_| true)
        .into_iter()
        .collect()
}

/// The index `roll` (below the weights' sum) lands on when each weight is
/// a slice of the line, in order. A zero weight is never landed on.
pub fn weighted_pick(weights: &[u32], roll: u32) -> usize {
    let mut rest = roll;
    for (i, &w) in weights.iter().enumerate() {
        if rest < w {
            return i;
        }
        rest -= w;
    }
    weights.len().saturating_sub(1)
}

/// Which of a study's `len` pool entries it finds. With no exclusive-only
/// entry this is the one uniform draw it always was, so no seeded study
/// shifts; otherwise one draw over the weights
/// (`STUDY_EXCLUSIVE_PART_PICK_WEIGHT` against `STUDY_PICK_WEIGHT`).
pub fn study_pick(
    len: usize,
    is_exclusive_only: impl Fn(usize) -> bool,
    rng: &mut rand::rngs::StdRng,
) -> usize {
    use rand::RngExt;
    if !(0..len).any(&is_exclusive_only) {
        return rng.random_range(0..len);
    }
    let weights: Vec<u32> = (0..len)
        .map(|i| {
            if is_exclusive_only(i) {
                crate::tuning::STUDY_EXCLUSIVE_PART_PICK_WEIGHT
            } else {
                crate::tuning::STUDY_PICK_WEIGHT
            }
        })
        .collect();
    weighted_pick(&weights, rng.random_range(0..weights.iter().sum()))
}
