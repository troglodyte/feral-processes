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
use feral_processes_engine::species::{ShapeFault, SpeciesDef, stat_shape_faults};

/// Why a candidate roster was thrown out.
#[derive(Clone, Debug, PartialEq)]
pub enum Rejection {
    /// No species is beatable by a fresh player, so the opening ring has
    /// nothing to draw from.
    OpeningRingEmpty,
    /// *Every* species is beatable by a fresh player, so the roster has no
    /// upper half left and the run flattens out.
    NothingBeyondTheOpeningRing,
    /// An ordinary species' numbers stopped agreeing with the class its
    /// affinities name.
    ///
    /// One variant for all four of the derived-stat rules rather than four,
    /// because they are one rule read at four points and a candidate
    /// usually breaks several at once — moving `growth_multiplier` alone
    /// changes the budget, which changes the total, which changes every
    /// axis target. Reporting the first and stopping is how the last
    /// proposal's two budget violations went unseen behind a speed band.
    OffClassShape {
        species: String,
        faults: Vec<ShapeFault>,
    },
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
            Rejection::OffClassShape { species, faults } => {
                write!(
                    f,
                    "{species}'s stat block is derived from its growth band's budget and \
                     its class's share of it, and no longer is:"
                )?;
                for fault in faults {
                    write!(f, "\n  - {fault}")?;
                }
                Ok(())
            }
        }
    }
}

/// Checks a candidate roster against the rules no score may outvote.
///
/// Calls `balance_sim::beatable_by_a_fresh_player` and
/// `species::stat_shape_faults` rather than restating what "beatable" or
/// "on budget" mean — the predicates the shipped censuses
/// (`the_shipped_roster_has_species_on_both_sides_of_the_opening_ring`,
/// `every_ordinary_species_stat_shape_agrees_with_its_affinity_class`) use
/// are the predicates a proposal has to satisfy, and a second copy here is
/// one that would drift. The copy nobody runs would be this one.
///
/// The ring rules come first because they are the more fundamental
/// complaint: a roster whose opening fight cannot be won is not a roster
/// with a stat-shape problem.
///
/// **Bosses are exempt from the shape rules**, which is not an
/// optimisation — it is what the census does. They carry no affinities, sit
/// outside the class system, and are the only species the tuner has ever
/// proposed a useful move for.
pub fn check(defs: &[SpeciesDef]) -> Result<(), Rejection> {
    let (easy, hard): (Vec<_>, Vec<_>) = defs.iter().partition(|s| beatable_by_a_fresh_player(s));
    if easy.is_empty() {
        return Err(Rejection::OpeningRingEmpty);
    }
    if hard.is_empty() {
        return Err(Rejection::NothingBeyondTheOpeningRing);
    }

    // Reported by id rather than in whatever order the caller collected the
    // files in, so the same candidate always names the same species and two
    // runs of the search can be compared.
    let mut offenders: Vec<(&str, Vec<ShapeFault>)> = defs
        .iter()
        .filter(|s| !s.is_boss)
        .map(|s| (s.id.as_str(), stat_shape_faults(s)))
        .filter(|(_, faults)| !faults.is_empty())
        .collect();
    offenders.sort_by_key(|(id, _)| *id);
    if let Some((id, faults)) = offenders.into_iter().next() {
        return Err(Rejection::OffClassShape {
            species: id.to_string(),
            faults,
        });
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

    /// The shipped roster with one species edited — a candidate the search
    /// could actually produce, as against `scaled`'s whole-roster sweep.
    fn shipped_with(id: &str, edit: impl Fn(&mut SpeciesDef)) -> Vec<SpeciesDef> {
        let mut defs = shipped();
        let def = defs
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap_or_else(|| panic!("{id} is not in the shipped roster"));
        edit(def);
        defs
    }

    /// The faults `check` rejected `id` for, or a panic naming what it did
    /// instead — a rule that fires on the wrong species is a rule that
    /// passes its own test while freezing something else.
    fn faults_for(defs: &[SpeciesDef], id: &str) -> Vec<ShapeFault> {
        match check(defs) {
            Err(Rejection::OffClassShape { species, faults }) if species == id => faults,
            other => panic!("expected {id} to be rejected off-shape, got {other:?}"),
        }
    }

    /// The one move the first real search made that was worth having:
    /// `overseer.base_atk 17 -> 11`, the lair guardian. It was legal
    /// because a boss sits outside the class system entirely, and it beat
    /// the other thirteen field moves put together.
    ///
    /// The regression this exists for is an over-eager shape rule freezing
    /// the only species the tuner has ever found value in.
    #[test]
    fn a_bosss_stats_may_move_freely() {
        let defs = shipped_with("overseer", |d| {
            assert!(d.is_boss, "overseer must be the boss this test is about");
            d.base_atk = 40;
            d.base_hp = 300;
            d.base_speed = 3;
        });
        assert_eq!(check(&defs), Ok(()));
    }

    #[test]
    fn a_stat_block_off_its_budget_is_rejected() {
        // The real proposal: `rootkit.base_hp +19`, `base_def +3`, taking a
        // Leech at growth 1.5 from the 147 points its band and class allow
        // to 169.
        let defs = shipped_with("rootkit", |d| {
            d.base_hp += 19;
            d.base_def += 3;
        });
        assert!(
            faults_for(&defs, "rootkit").contains(&ShapeFault::OffBudget {
                spent: 169,
                budget: 140,
                expected: 147,
            }),
            "the budget rule did not fire"
        );
    }

    #[test]
    fn an_axis_share_out_of_tolerance_is_rejected() {
        // Total held at the budget and moved *between* axes, which the
        // budget rule alone cannot see: a Leech that spends its 147 points
        // like a Striker is a Striker wearing a Leech's affinities.
        let defs = shipped_with("rootkit", |d| {
            d.base_hp -= 20;
            d.base_atk += 20;
        });
        let faults = faults_for(&defs, "rootkit");
        assert!(
            faults
                .iter()
                .any(|f| matches!(f, ShapeFault::AxisOffShare { axis: "HP", .. })),
            "the axis rule did not fire: {faults:?}"
        );
        assert!(
            !faults
                .iter()
                .any(|f| matches!(f, ShapeFault::OffBudget { .. })),
            "the total was held on budget, so only the shares moved: {faults:?}"
        );
    }

    #[test]
    fn a_speed_outside_its_class_band_is_rejected() {
        // `drone.base_speed 8 -> 6` is the move that broke the shape census
        // in the first real search. Speed is not only initiative — it paces
        // the machine a posted program works.
        let defs = shipped_with("drone", |d| d.base_speed = 6);
        assert!(
            faults_for(&defs, "drone").contains(&ShapeFault::SpeedOutsideBand {
                speed: 6,
                slowest: 8,
                fastest: 9,
            }),
            "the speed band rule did not fire"
        );
    }

    #[test]
    fn a_growth_multiplier_between_the_rungs_is_rejected() {
        // `sprite.growth_multiplier -> 1.238` broke two shipped censuses at
        // once. The budget is a step function, so a value between rungs
        // derives a whole stat block from a number nobody chose.
        let defs = shipped_with("sprite", |d| d.growth_multiplier = 1.238);
        let faults = faults_for(&defs, "sprite");
        assert!(
            faults
                .iter()
                .any(|f| matches!(f, ShapeFault::GrowthOffTier { .. })),
            "the growth tier rule did not fire: {faults:?}"
        );
    }

    /// A candidate breaking several rules at once reports all of them.
    /// Moving `growth_multiplier` alone changes the budget, which changes
    /// the total, which changes every axis target — so "one field moved"
    /// and "one thing is wrong" are different claims, and a reviewer needs
    /// the second.
    #[test]
    fn a_rejection_reports_every_fault_rather_than_the_first() {
        let defs = shipped_with("sprite", |d| d.growth_multiplier = 1.238);
        assert!(
            faults_for(&defs, "sprite").len() > 1,
            "a growth move off-tier also puts the block off budget"
        );
    }
}
