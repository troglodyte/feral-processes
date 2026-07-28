use crate::tuning::DECOMPILER_SKILL_BONUS;
use crate::tuning::{
    CAPTURE_CHANCE_MAX, CAPTURE_CHANCE_MIN, CAPTURE_DIFFICULTY_PENALTY, CAPTURE_HP_PENALTY,
    CAPTURE_POTENCY_CEILING,
};

/// ICE-breaking odds: weaker (lower `hp_fraction`) and easier-compiled
/// species are more likely to be decompiled, and stronger breakers help. A
/// more practiced player (`decompiler_skill`) *multiplies* whatever those
/// three are worth rather than adding to it, so neither the species' own
/// resistance nor the work of weakening it first can be skilled past — see
/// `DECOMPILER_SKILL_BONUS`. The `0.9` ceiling (rather than a full `1.0`)
/// means even a fully-weakened, zero-difficulty target isn't a sure thing on
/// item potency alone.
pub fn capture_chance(
    hp_fraction: f32,
    item_potency: f32,
    taming_difficulty: f32,
    decompiler_skill: i32,
) -> f32 {
    let base = item_potency
        * (CAPTURE_POTENCY_CEILING - hp_fraction * CAPTURE_HP_PENALTY)
        * (1.0 - taming_difficulty * CAPTURE_DIFFICULTY_PENALTY);
    let skill_multiplier = 1.0 + decompiler_skill as f32 * DECOMPILER_SKILL_BONUS;
    (base * skill_multiplier).clamp(CAPTURE_CHANCE_MIN, CAPTURE_CHANCE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weaker_prey_is_easier_to_tame() {
        let full_hp = capture_chance(1.0, 0.55, 0.2, 0);
        let low_hp = capture_chance(0.1, 0.55, 0.2, 0);
        assert!(low_hp > full_hp);
    }

    #[test]
    fn harder_species_resist_taming() {
        let easy = capture_chance(0.5, 0.55, 0.1, 0);
        let hard = capture_chance(0.5, 0.55, 0.9, 0);
        assert!(hard < easy);
    }

    #[test]
    fn higher_decompiler_skill_improves_odds() {
        let unskilled = capture_chance(0.5, 0.55, 0.5, 0);
        let skilled = capture_chance(0.5, 0.55, 0.5, 10);
        assert!(skilled > unskilled);
    }

    /// The whole point of the multiplicative skill term: a well-practiced
    /// player is better at everything, but a boss-grade species stays
    /// meaningfully harder than trash forever. Under the old additive term
    /// both of these pinned to `CAPTURE_CHANCE_MAX` and the spread vanished.
    #[test]
    fn high_skill_does_not_flatten_the_gap_between_easy_and_boss_species() {
        let drone = capture_chance(0.0, 0.4, 0.15, 40);
        let boss = capture_chance(0.0, 0.4, 0.9, 40);

        assert!(
            drone < CAPTURE_CHANCE_MAX,
            "skill 40 should not pin even an easy species to the clamp: {drone}"
        );
        assert!(
            drone > boss * 1.5,
            "an easy species must stay far ahead of a boss at high skill: \
             {drone} vs {boss}"
        );
    }

    #[test]
    fn chance_is_always_within_bounds() {
        for hp in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for diff in [0.0, 0.5, 1.0] {
                for skill in [0, 5, 50] {
                    let c = capture_chance(hp, 0.55, diff, skill);
                    assert!((0.05..=0.95).contains(&c), "out of bounds: {c}");
                }
            }
        }
    }
}
