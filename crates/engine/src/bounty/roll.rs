//! The bounty roll: a pure function of a gathered input and an RNG.

use rand::prelude::*;

use super::BountyQuarry;
use super::names::quarry_name;
use crate::components::Rarity;
use crate::contracts::{ContractDef, ContractId, Objective, ROLLED_ID_SEPARATOR, Reward};
use crate::tuning::*;

/// What the sector offers a bounty, gathered by `Game` (sorted by stable id so
/// the draw never depends on query order).
pub struct BountyInput {
    pub zone: u32,
    /// Eligible non-boss species ids, sorted.
    pub habitat: Vec<String>,
    /// Walkable, filtered anchor tiles, sorted.
    pub anchor_candidates: Vec<(i32, i32)>,
}

/// A rolled bounty, ready to sit on the board as any other contract.
pub struct RolledBounty {
    pub def: ContractDef,
}

fn zones_above_min(zone: u32) -> u32 {
    zone.saturating_sub(BOUNTY_MIN_ZONE)
}

fn rarity_for(zone: u32) -> Rarity {
    if zone >= BOUNTY_PLATINUM_ZONE {
        Rarity::Platinum
    } else if zone >= BOUNTY_GOLD_ZONE {
        Rarity::Gold
    } else {
        Rarity::Silver
    }
}

/// Splits `total` escort members over `species` as evenly as possible, the
/// remainder going to the earlier species. Sorted by species.
fn split_escort(mut species: Vec<String>, total: u32) -> Vec<(String, u32)> {
    species.sort();
    let n = species.len() as u32;
    species
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, total / n + u32::from((i as u32) < total % n)))
        .collect()
}

fn describe(q: &BountyQuarry) -> String {
    format!(
        "{}, a boss-grade {}, is running with an escort of {}. Bring it down.",
        q.name,
        q.species.replace('_', " "),
        q.escort_size()
    )
}

/// Rolls one bounty, or `None` when the sector cannot hold one: below
/// `BOUNTY_MIN_ZONE`, no species to field, or nowhere to place the quarry.
pub fn roll(input: &BountyInput, rng: &mut impl Rng) -> Option<RolledBounty> {
    if input.zone < BOUNTY_MIN_ZONE
        || input.habitat.is_empty()
        || input.anchor_candidates.is_empty()
    {
        return None;
    }
    let up = zones_above_min(input.zone);
    let species = input.habitat[rng.random_range(0..input.habitat.len())].clone();
    let anchor = input.anchor_candidates[rng.random_range(0..input.anchor_candidates.len())];
    let name = quarry_name(rng);

    let mut escort_species = Vec::new();
    while escort_species.len() < BOUNTY_ESCORT_SPECIES.min(input.habitat.len()) {
        let pick = input.habitat[rng.random_range(0..input.habitat.len())].clone();
        if !escort_species.contains(&pick) {
            escort_species.push(pick);
        }
    }
    let escort = split_escort(
        escort_species,
        BOUNTY_ESCORT_BASE + BOUNTY_ESCORT_PER_ZONE * up,
    );

    let quarry = BountyQuarry {
        name,
        species,
        grade_pct: BOUNTY_GRADE_PCT,
        rarity: rarity_for(input.zone),
        escort,
        anchor,
        tether: BOUNTY_TETHER,
        zone: input.zone,
    };
    let id = ContractId::from(format!(
        "bounty{ROLLED_ID_SEPARATOR}{}_{:08x}",
        input.zone,
        rng.random::<u32>()
    ));
    let def = ContractDef {
        id,
        name: format!("Bounty: {}", quarry.name),
        description: describe(&quarry),
        objective: Objective::Bounty(quarry),
        reward: vec![
            Reward::Credits(BOUNTY_CREDITS_BASE + BOUNTY_CREDITS_PER_ZONE * up),
            Reward::Xp(BOUNTY_XP_BASE + BOUNTY_XP_PER_ZONE * up),
        ],
        min_zone: BOUNTY_MIN_ZONE,
        repeatable: false,
        starter: false,
        tutorial: None,
    };
    Some(RolledBounty { def })
}
