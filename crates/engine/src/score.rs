//! The run score: a pure formula over counts, with no `World`.
//!
//! `Game::score_card` gathers the counts (the saved `RunTally` plus live
//! state) into `ScoreInputs`; every screen reads the card it returns, so no
//! display can compute the total its own way. `creation_bonus` is the only
//! reader of the lifetime score, for the same reason.

use crate::resources::{DifficultyMode, EnemyStrength};
use crate::tuning;

/// Every count the card needs. Plain data so the formula is testable alone.
#[derive(Clone, Debug)]
pub struct ScoreInputs {
    pub foe_levels: u64,
    pub bosses: u64,
    pub deepest_depth: u64,
    pub keys: u64,
    pub escaped: bool,
    pub structures: u64,
    pub compiled: u64,
    pub achievements: u64,
    pub mode: DifficultyMode,
    pub band: EnemyStrength,
}

/// One row of the card. A zero count still gets a line, so the card always
/// has the same shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreLine {
    pub label: &'static str,
    pub count: u64,
    pub points: u64,
}

#[derive(Clone, Debug)]
pub struct ScoreCard {
    pub lines: Vec<ScoreLine>,
    pub multiplier: f32,
    pub total: u64,
}

fn line(label: &'static str, count: u64, per: u64) -> ScoreLine {
    ScoreLine {
        label,
        count,
        points: count * per,
    }
}

/// The difficulty multiplier on its own, for the card's multiplier line.
pub fn multiplier(mode: DifficultyMode, band: EnemyStrength) -> f32 {
    let base = match mode {
        DifficultyMode::Permadeath => tuning::SCORE_PERMADEATH_MULT,
        DifficultyMode::Forgiving => 1.0,
    };
    base + band.ordinal() as f32 * tuning::SCORE_PER_BAND
}

pub fn card(inputs: &ScoreInputs) -> ScoreCard {
    let lines = vec![
        line(
            "Programs defeated",
            inputs.foe_levels,
            tuning::SCORE_PER_FOE_LEVEL,
        ),
        line("Bosses defeated", inputs.bosses, tuning::SCORE_PER_BOSS),
        line(
            "Deepest Stack depth",
            inputs.deepest_depth,
            tuning::SCORE_PER_DEPTH,
        ),
        line("Phase keys held", inputs.keys, tuning::SCORE_PER_KEY),
        line(
            "Escaped the Basin",
            inputs.escaped as u64,
            tuning::SCORE_ESCAPE,
        ),
        line(
            "Structures standing",
            inputs.structures,
            tuning::SCORE_PER_STRUCTURE,
        ),
        line(
            "Programs compiled",
            inputs.compiled,
            tuning::SCORE_PER_PROGRAM,
        ),
        line(
            "Achievements earned this run",
            inputs.achievements,
            tuning::SCORE_PER_ACHIEVEMENT,
        ),
    ];
    let multiplier = multiplier(inputs.mode, inputs.band);
    let sum: u64 = lines.iter().map(|l| l.points).sum();
    ScoreCard {
        lines,
        multiplier,
        total: (sum as f64 * multiplier as f64) as u64,
    }
}

/// What the lifetime score buys at the next character creation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreationBonus {
    pub stat_points: u32,
    pub perk_points: u32,
    pub credits: u32,
}

fn ladder(lifetime: u64, per: u64, cap: u32) -> u32 {
    (lifetime / per).min(cap as u64) as u32
}

/// All three bonus ladders. Creation and its tests call this; nothing
/// recomputes it.
pub fn creation_bonus(lifetime: u64) -> CreationBonus {
    CreationBonus {
        stat_points: ladder(
            lifetime,
            tuning::SCORE_PER_BONUS_STAT_POINT,
            tuning::BONUS_STAT_POINT_CAP,
        ),
        perk_points: ladder(
            lifetime,
            tuning::SCORE_PER_BONUS_PERK_POINT,
            tuning::BONUS_PERK_POINT_CAP,
        ),
        credits: ladder(
            lifetime,
            tuning::SCORE_PER_BONUS_CREDIT,
            tuning::BONUS_CREDIT_CAP,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero() -> ScoreInputs {
        ScoreInputs {
            foe_levels: 0,
            bosses: 0,
            deepest_depth: 0,
            keys: 0,
            escaped: false,
            structures: 0,
            compiled: 0,
            achievements: 0,
            mode: DifficultyMode::Forgiving,
            band: EnemyStrength::Standard,
        }
    }

    fn points(card: &ScoreCard, label: &str) -> u64 {
        card.lines.iter().find(|l| l.label == label).unwrap().points
    }

    #[test]
    fn an_all_zero_card_keeps_every_line_and_totals_zero() {
        let c = card(&zero());
        assert_eq!(c.lines.len(), 8);
        assert!(c.lines.iter().all(|l| l.count == 0 && l.points == 0));
        assert_eq!(c.total, 0);
    }

    #[test]
    fn each_line_multiplies_its_count_by_its_weight() {
        let c = card(&ScoreInputs {
            foe_levels: 3,
            bosses: 2,
            deepest_depth: 4,
            keys: 5,
            escaped: true,
            structures: 6,
            compiled: 7,
            achievements: 8,
            ..zero()
        });
        assert_eq!(
            points(&c, "Programs defeated"),
            3 * tuning::SCORE_PER_FOE_LEVEL
        );
        assert_eq!(points(&c, "Bosses defeated"), 2 * tuning::SCORE_PER_BOSS);
        assert_eq!(
            points(&c, "Deepest Stack depth"),
            4 * tuning::SCORE_PER_DEPTH
        );
        assert_eq!(points(&c, "Phase keys held"), 5 * tuning::SCORE_PER_KEY);
        assert_eq!(points(&c, "Escaped the Basin"), tuning::SCORE_ESCAPE);
        assert_eq!(
            points(&c, "Structures standing"),
            6 * tuning::SCORE_PER_STRUCTURE
        );
        assert_eq!(
            points(&c, "Programs compiled"),
            7 * tuning::SCORE_PER_PROGRAM
        );
        assert_eq!(
            points(&c, "Achievements earned this run"),
            8 * tuning::SCORE_PER_ACHIEVEMENT
        );
        let sum: u64 = c.lines.iter().map(|l| l.points).sum();
        assert_eq!(c.total, sum);
    }

    #[test]
    fn the_multiplier_follows_mode_and_band() {
        assert_eq!(
            multiplier(DifficultyMode::Forgiving, EnemyStrength::Standard),
            1.0
        );
        assert_eq!(
            multiplier(DifficultyMode::Permadeath, EnemyStrength::Standard),
            tuning::SCORE_PERMADEATH_MULT
        );
        assert_eq!(
            multiplier(DifficultyMode::Forgiving, EnemyStrength::Critical),
            1.0 + 4.0 * tuning::SCORE_PER_BAND
        );
        assert!(
            multiplier(DifficultyMode::Permadeath, EnemyStrength::High)
                > multiplier(DifficultyMode::Permadeath, EnemyStrength::Elevated)
        );
    }

    #[test]
    fn the_total_is_the_scaled_sum_rounded_down() {
        let inputs = ScoreInputs {
            foe_levels: 1,
            mode: DifficultyMode::Permadeath,
            ..zero()
        };
        let c = card(&inputs);
        assert_eq!(
            c.total,
            (tuning::SCORE_PER_FOE_LEVEL as f64 * tuning::SCORE_PERMADEATH_MULT as f64) as u64
        );
        assert_eq!(c.multiplier, tuning::SCORE_PERMADEATH_MULT);
    }

    /// Progression is earned by fighting: if a retune lets a big base beat a
    /// deep fighting run, this fails.
    #[test]
    fn a_fighter_outscores_a_turtle() {
        let fighter = card(&ScoreInputs {
            foe_levels: 400,
            bosses: 2,
            deepest_depth: 6,
            structures: 8,
            compiled: 6,
            ..zero()
        });
        let turtle = card(&ScoreInputs {
            foe_levels: 40,
            structures: 60,
            compiled: 5,
            ..zero()
        });
        assert!(fighter.total > turtle.total);
    }

    #[test]
    fn each_bonus_ladder_has_a_threshold_and_a_cap() {
        assert_eq!(creation_bonus(0), CreationBonus::default());

        let s = tuning::SCORE_PER_BONUS_STAT_POINT;
        assert_eq!(creation_bonus(s - 1).stat_points, 0);
        assert_eq!(creation_bonus(s).stat_points, 1);
        assert_eq!(
            creation_bonus(u64::MAX).stat_points,
            tuning::BONUS_STAT_POINT_CAP
        );

        let p = tuning::SCORE_PER_BONUS_PERK_POINT;
        assert_eq!(creation_bonus(p - 1).perk_points, 0);
        assert_eq!(creation_bonus(p).perk_points, 1);
        assert_eq!(
            creation_bonus(u64::MAX).perk_points,
            tuning::BONUS_PERK_POINT_CAP
        );

        let c = tuning::SCORE_PER_BONUS_CREDIT;
        assert_eq!(creation_bonus(c - 1).credits, 0);
        assert_eq!(creation_bonus(c).credits, 1);
        assert_eq!(creation_bonus(u64::MAX).credits, tuning::BONUS_CREDIT_CAP);
    }
}
