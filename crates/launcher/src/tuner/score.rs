//! What "better" means, as one number.
//!
//! A roster is scored by how far every authored target sits from what the
//! arena actually measured. Lower is better, zero is exact. This is the one
//! definition of the objective — the search reads it, the report prints it,
//! and a later learned policy would take its reward from it. Two copies of
//! it would be two answers to "is this roster good".

use feral_processes_engine::arena::Summary;

/// One scenario paired with what it should measure.
///
/// `reps` overrides the scenario file's own count: a hand-built scenario is
/// usually `reps: 1` so the builder screen prints a transcript, and a rate
/// cannot be read off one fight.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Target {
    pub scenario: String,
    pub reps: u32,
    pub want_win_rate: f32,
    pub want_hp_left: f32,
    /// Relative importance of this scenario against the others. 1.0 is
    /// ordinary; raise it for a band the roster must get right.
    pub weight: f32,
}

/// How much a miss on surviving HP counts against a miss on winning at all.
///
/// Deliberately below 1.0. Win rate is the difficulty question; HP left is
/// how comfortable the win was. A roster that hits every win rate while
/// leaving the party a little healthier than asked is close to right, and
/// scoring the two equally lets a comfort miss outvote a band that cannot
/// be won.
const HP_ERROR_WEIGHT: f32 = 0.5;

/// Squared error of one measured fight against what it was supposed to be.
///
/// Squared rather than absolute so that one badly missed band outweighs
/// several near misses — a depth nobody can clear is a different kind of
/// problem from five fights being slightly off, and the search should spend
/// its moves on the former.
pub fn target_error(target: &Target, summary: &Summary) -> f32 {
    let win = summary.win_rate - target.want_win_rate;
    let hp = summary.mean_player_hp_fraction - target.want_hp_left;
    target.weight * (win * win + HP_ERROR_WEIGHT * hp * hp)
}

/// The whole objective: every target's error, summed.
///
/// Takes measurements positionally against targets, so a caller that drops
/// a scenario silently would score a roster against the wrong band. The
/// length mismatch is an error rather than a zip that quietly truncates.
pub fn total_error(targets: &[Target], summaries: &[Summary]) -> Result<f32, String> {
    if targets.len() != summaries.len() {
        return Err(format!(
            "scored {} summaries against {} targets",
            summaries.len(),
            targets.len()
        ));
    }
    Ok(targets
        .iter()
        .zip(summaries)
        .map(|(t, s)| target_error(t, s))
        .sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(want_win_rate: f32, want_hp_left: f32, weight: f32) -> Target {
        Target {
            scenario: "dev-arenas/opening-fight.ron".into(),
            reps: 40,
            want_win_rate,
            want_hp_left,
            weight,
        }
    }

    fn summary(win_rate: f32, hp: f32) -> Summary {
        Summary {
            reps: 40,
            wins: (win_rate * 40.0) as u32,
            win_rate,
            mean_rounds: 5.0,
            median_rounds: 5,
            mean_player_hp_fraction: hp,
            mean_companions_downed: 0.0,
            loss_seeds: Vec::new(),
        }
    }

    #[test]
    fn a_roster_that_hits_its_target_exactly_scores_zero() {
        let t = target(0.9, 0.6, 1.0);
        assert_eq!(target_error(&t, &summary(0.9, 0.6)), 0.0);
    }

    #[test]
    fn missing_the_win_rate_costs_more_than_missing_the_hp_band() {
        let t = target(0.9, 0.6, 1.0);
        let win_miss = target_error(&t, &summary(0.7, 0.6));
        let hp_miss = target_error(&t, &summary(0.9, 0.4));
        assert!(
            win_miss > hp_miss,
            "win miss {win_miss} should outweigh equal-sized hp miss {hp_miss}"
        );
    }

    #[test]
    fn one_badly_missed_band_outweighs_several_near_misses() {
        let t = target(0.9, 0.6, 1.0);
        // The signal the search needs: an unwinnable band is a different
        // problem from four fights being slightly off, and squaring is what
        // makes the score say so.
        let unwinnable = target_error(&t, &summary(0.0, 0.6));
        let near: f32 = (0..4).map(|_| target_error(&t, &summary(0.8, 0.6))).sum();
        assert!(
            unwinnable > near,
            "unwinnable {unwinnable} should outweigh four near misses {near}"
        );
    }

    #[test]
    fn weight_scales_a_targets_contribution() {
        let plain = target_error(&target(0.9, 0.6, 1.0), &summary(0.7, 0.6));
        let heavy = target_error(&target(0.9, 0.6, 3.0), &summary(0.7, 0.6));
        assert!((heavy - plain * 3.0).abs() < 1e-6);
    }

    #[test]
    fn the_total_is_every_targets_error() {
        let targets = vec![target(0.9, 0.6, 1.0), target(0.5, 0.3, 2.0)];
        let summaries = vec![summary(0.9, 0.6), summary(0.5, 0.3)];
        assert_eq!(total_error(&targets, &summaries).unwrap(), 0.0);
    }

    #[test]
    fn scoring_fewer_summaries_than_targets_is_an_error_not_a_truncation() {
        let targets = vec![target(0.9, 0.6, 1.0), target(0.5, 0.3, 1.0)];
        assert!(total_error(&targets, &[summary(0.9, 0.6)]).is_err());
    }
}
