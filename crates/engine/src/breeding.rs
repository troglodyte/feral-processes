//! The rules of breeding, as pure functions of their inputs: which species a
//! child is, and what it inherits.
//!
//! Nothing here reads a `Game` or a clock; the one source of chance is the
//! `rng` a caller passes in. `Game::breed` (`game/breeding.rs`) is the only
//! caller that rolls, which is what keeps a reload from re-rolling a child.

use crate::attributes::{AttributeDb, AttributeDef, AttributeId};
use crate::components::Potential;
use crate::species::{SpeciesDb, SpeciesDef};
use crate::tuning::{
    BREEDING_ATTRIBUTE_HARD_CAP, BREEDING_GEN_STEP, BREEDING_MUTATION, BREEDING_ROLL_HARD_CAP,
    MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL,
};
use rand::{Rng, RngExt};
use std::collections::BTreeMap;

/// What a parent contributes to `inherit`. A program with no `Generation`
/// component reads as generation 0.
#[derive(Clone, Debug)]
pub struct ParentRolls {
    pub potential: Potential,
    pub attributes: BTreeMap<AttributeId, i32>,
    pub generation: u32,
}

/// What `inherit` hands the child. The same shape as a parent's, so a child
/// is a parent the moment it hatches.
pub type ChildRolls = ParentRolls;

/// A child waiting in a bay: every roll fixed at the start of the breeding,
/// so reloading cannot re-roll it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Incubation {
    pub species: String,
    pub potential: Potential,
    pub attributes: BTreeMap<AttributeId, i32>,
    pub generation: u32,
    pub due: u64,
}

/// Why a breeding is refused. Every variant is checked before anything is
/// spent, so a refused breeding leaves the game as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreedRefusal {
    Busy,
    SameProgram,
    NotYours,
    OnCooldown,
    NoFreeSlot,
    NoSeed,
    UnknownKind,
}

impl BreedRefusal {
    /// A row fragment short enough for a one-line menu entry.
    pub fn reason(self) -> &'static str {
        match self {
            BreedRefusal::Busy => "not now",
            BreedRefusal::SameProgram => "pick two programs",
            BreedRefusal::NotYours => "not yours",
            BreedRefusal::OnCooldown => "ready later",
            BreedRefusal::NoFreeSlot => "no free bay",
            BreedRefusal::NoSeed => "no breeding seed",
            BreedRefusal::UnknownKind => "unknown kind",
        }
    }
}

impl std::fmt::Display for BreedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for BreedRefusal {}

/// The species a child of `a` and `b` is: that species when they match, the
/// authored hybrid for the pair when there is one, else one parent's species
/// at random.
pub fn child_species(a: &str, b: &str, db: &SpeciesDb, rng: &mut impl Rng) -> String {
    if a == b {
        return a.to_string();
    }
    if let Some(hybrid) = db.hybrid_of(a, b) {
        return hybrid.id.to_string();
    }
    if rng.random_bool(0.5) { a } else { b }.to_string()
}

/// The ceiling a roll may reach in a child of `generation`.
fn roll_ceiling(generation: u32) -> f32 {
    (MAX_INDIVIDUAL_ROLL + generation as f32 * BREEDING_GEN_STEP).min(BREEDING_ROLL_HARD_CAP)
}

/// The one clamp every bred roll passes through, `inherit`'s draw and the
/// preview's range alike.
fn clamp_roll(roll: f32, generation: u32) -> f32 {
    roll.clamp(MIN_INDIVIDUAL_ROLL, roll_ceiling(generation))
}

/// The lowest and highest a child's roll can land when its better parent
/// rolled `best`. `inherit` and the preview both read this.
pub fn roll_span(best: f32, generation: u32) -> (f32, f32) {
    (
        clamp_roll(best - BREEDING_MUTATION, generation),
        clamp_roll(best + BREEDING_MUTATION, generation),
    )
}

/// The range an attribute may take in a child of `generation`: the wild range
/// of the child species' authored base, widened at the top by the lineage.
pub fn attribute_span(def: &AttributeDef, species: &SpeciesDef, generation: u32) -> (i32, i32) {
    let base = species
        .attributes
        .get(def.id.as_str())
        .copied()
        .unwrap_or(def.base);
    let lift = generation.min(BREEDING_ATTRIBUTE_HARD_CAP) as i32;
    (base - def.spread, base + def.spread + lift)
}

/// The child's rolls: the better of each parent's, a small mutation, and a
/// ceiling that climbs a little with each generation.
///
/// A parent's attributes include stat points spent at level-up, so the span
/// clamp is the whole of what stops trained points passing down wholesale.
pub fn inherit(
    a: &ParentRolls,
    b: &ParentRolls,
    species: &SpeciesDef,
    attributes: &AttributeDb,
    rng: &mut impl Rng,
) -> ChildRolls {
    let generation = a.generation.max(b.generation) + 1;
    let mut roll = |a: f32, b: f32| {
        let mutation = rng.random_range(-BREEDING_MUTATION..=BREEDING_MUTATION);
        clamp_roll(a.max(b) + mutation, generation)
    };
    let (pa, pb) = (&a.potential, &b.potential);
    let potential = Potential {
        hp_roll: roll(pa.hp_roll, pb.hp_roll),
        atk_roll: roll(pa.atk_roll, pb.atk_roll),
        def_roll: roll(pa.def_roll, pb.def_roll),
        growth_roll: roll(pa.growth_roll, pb.growth_roll),
        assembly_roll: roll(pa.assembly_roll, pb.assembly_roll),
        extraction_roll: roll(pa.extraction_roll, pb.extraction_roll),
    };
    let mut child = BTreeMap::new();
    for def in attributes.iter() {
        let (low, high) = attribute_span(def, species, generation);
        let own = |p: &ParentRolls| p.attributes.get(&def.id).copied().unwrap_or(low);
        let mutation = rng.random_range(-1..=1);
        child.insert(
            def.id.clone(),
            (own(a).max(own(b)) + mutation).clamp(low, high),
        );
    }
    ChildRolls {
        potential,
        attributes: child,
        generation,
    }
}
