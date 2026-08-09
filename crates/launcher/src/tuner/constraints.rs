//! The rules a candidate roster is *rejected* for breaking, rather than
//! scored down for.
//!
//! These aren't preferences the objective can trade against. A roster that
//! empties the opening ring is not a slightly worse roster — it is a game
//! whose first fight cannot be won, and no win-rate target elsewhere should
//! be able to buy that. Rejecting also happens to be the cheap path, since
//! it skips the battles entirely.
//!
//! What is deliberately *not* checked here is boss coverage. The tuner moves
//! numeric stats only and never touches `habitats` or `is_boss`, so
//! `every_biome_a_stack_link_can_open_in_fields_a_boss` cannot be broken by
//! anything a candidate does — a check for it would be a second copy of a
//! census that this tool cannot violate.

use feral_processes_engine::balance_sim::beatable_by_a_fresh_player;
use feral_processes_engine::species::SpeciesDef;

/// Why a candidate roster was thrown out.
#[derive(Clone, Debug, PartialEq)]
pub enum Rejection {
    /// No species is beatable by a fresh player, so the opening ring has
    /// nothing to draw from.
    OpeningRingEmpty,
    /// *Every* species is beatable by a fresh player, so the roster has no
    /// upper half left and the run flattens out.
    NothingBeyondTheOpeningRing,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rejection::OpeningRingEmpty => write!(
                f,
                "no species is beatable by a fresh player: `habitat_pools` falls back to \
                 the biome's unfiltered roster, so the opening ring fills with programs \
                 that cannot be beaten while still looking intact"
            ),
            Rejection::NothingBeyondTheOpeningRing => write!(
                f,
                "every species is beatable by a fresh player: the roster has no upper \
                 half and the run has nothing to grow into"
            ),
        }
    }
}

/// Checks a candidate roster against the rules no score may outvote.
///
/// Calls `balance_sim::beatable_by_a_fresh_player` rather than restating
/// what "beatable" means — the predicate the shipped census
/// (`the_shipped_roster_has_species_on_both_sides_of_the_opening_ring`)
/// uses is the predicate a proposal has to satisfy, and a second copy here
/// is one that would drift.
pub fn check(defs: &[SpeciesDef]) -> Result<(), Rejection> {
    let (easy, hard): (Vec<_>, Vec<_>) = defs.iter().partition(|s| beatable_by_a_fresh_player(s));
    if easy.is_empty() {
        return Err(Rejection::OpeningRingEmpty);
    }
    if hard.is_empty() {
        return Err(Rejection::NothingBeyondTheOpeningRing);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Vec<SpeciesDef> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/species");
        std::fs::read_dir(&dir)
            .expect("shipped species dir")
            .filter_map(|e| {
                let path = e.ok()?.path();
                (path.extension()? == "ron").then_some(path)
            })
            .filter_map(|p| ron::from_str::<SpeciesDef>(&std::fs::read_to_string(p).ok()?).ok())
            .collect()
    }

    /// Scales every stat on a roster, standing in for a candidate the search
    /// pushed too far in one direction.
    fn scaled(mut defs: Vec<SpeciesDef>, factor: f32) -> Vec<SpeciesDef> {
        for def in &mut defs {
            def.base_hp = ((def.base_hp as f32) * factor).round().max(1.0) as i32;
            def.base_atk = ((def.base_atk as f32) * factor).round().max(1.0) as i32;
            def.base_def = ((def.base_def as f32) * factor).round().max(0.0) as i32;
        }
        defs
    }

    #[test]
    fn the_shipped_roster_passes() {
        let defs = shipped();
        assert!(
            defs.len() > 5,
            "expected the real roster, got {}",
            defs.len()
        );
        assert_eq!(check(&defs), Ok(()));
    }

    #[test]
    fn a_roster_raised_out_of_reach_empties_the_opening_ring() {
        // The trap this exists for: `habitat_pools` falls back to the
        // biome's *unfiltered* roster when nothing qualifies, so raising
        // the four easy species empties the ring while leaving it looking
        // intact. The search must not be able to buy a win-rate target
        // with this.
        assert_eq!(
            check(&scaled(shipped(), 40.0)),
            Err(Rejection::OpeningRingEmpty)
        );
    }

    #[test]
    fn a_roster_flattened_to_trivial_has_nothing_past_the_ring() {
        assert_eq!(
            check(&scaled(shipped(), 0.02)),
            Err(Rejection::NothingBeyondTheOpeningRing)
        );
    }
}
