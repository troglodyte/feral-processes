use crate::tuning::{
    CAPTURE_CHANCE_MAX, CAPTURE_CHANCE_MIN, CAPTURE_DIFFICULTY_PENALTY, CAPTURE_HP_PENALTY,
    CAPTURE_POTENCY_CEILING, DECOMPILE_ATTEMPT_BONUS_CAP, DECOMPILE_ATTEMPT_BONUS_PCT,
    DECOMPILER_SKILL_BONUS,
};

/// Everything the player themself brings to a decompile attempt, as opposed
/// to what the target and the catalyst bring. Bundled rather than passed as
/// two more positional numbers because `capture_chance` already takes three
/// unlabelled floats and a fourth would be a swap waiting to happen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DecompilerBonuses {
    /// The player's effective `Decompiler` stat: levels plus equipment.
    pub skill: i32,
    /// Shaved off `CAPTURE_HP_PENALTY` by `Perk::ExploitFocus`, floored so
    /// the penalty itself never goes negative — a target's remaining HP must
    /// never *help* the attempt.
    pub hp_penalty_reduction: f32,
    /// Running `FieldBuffKind::CaptureBoost` power, in percentage points.
    /// Assembled by `Game::player_decompiler_bonuses` from the player's own
    /// field buff — that kind is `FieldScope::Run`, so it applies here
    /// regardless of which party member is doing the decompiling.
    pub capture_boost_pct: i32,
}

/// Everything the *target* brings to a decompile attempt, as opposed to what
/// the player and the catalyst bring. Bundled for the same reason
/// `DecompilerBonuses` is — `hp_fraction` and `taming_difficulty` are both
/// bare `f32`s in `0.0..=1.0` sitting next to each other, so a swap would
/// compile and quietly invert the whole formula — and because
/// `Game::target_resistance` needs something to return: it is the one place
/// these terms are assembled, so the odds a player is *shown* and the odds
/// they are *rolled against* cannot drift apart.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TargetResistance {
    /// How much Integrity the target has left, `0.0..=1.0`.
    pub hp_fraction: f32,
    /// `SpeciesDef::taming_difficulty` — how well this species resists
    /// being decompiled at all.
    pub taming_difficulty: f32,
    /// How many decompiles have already been attempted against *this*
    /// program in *this* fight. Counted by `Game::attempt_decompile` into
    /// `BattleState::decompile_attempts`, so it dies with the battle.
    pub prior_attempts: u32,
}

/// ICE-breaking odds: weaker (lower `hp_fraction`) and easier-compiled
/// species are more likely to be decompiled, and stronger breakers help. A
/// more practiced player (`player.skill`) *multiplies* whatever those three
/// are worth rather than adding to it, so neither the species' own
/// resistance nor the work of weakening it first can be skilled past — see
/// `DECOMPILER_SKILL_BONUS`. The `0.9` ceiling (rather than a full `1.0`)
/// means even a fully-weakened, zero-difficulty target isn't a sure thing on
/// item potency alone.
///
/// `player.hp_penalty_reduction` is the one term that does *not* scale the
/// whole attempt: it only softens how much the target's remaining HP counts
/// against you, so it is worth most at full HP and exactly nothing at zero.
///
/// `player.capture_boost_pct` scales the whole attempt again, on top of
/// `skill_multiplier`, before the final clamp — a running `CaptureBoost`
/// field buff raises the odds by that many percentage points regardless of
/// species or catalyst.
///
/// `target.prior_attempts` scales it once more, for the same reason those
/// two do rather than adding to the base: persistence must not be a way to
/// out-scale a species' own resistance. Every attempt already made on this
/// program leaves the next one better off, up to
/// `DECOMPILE_ATTEMPT_BONUS_CAP` attempts' worth — after which a stubborn
/// target simply stays stubborn, however many catalysts get fed to it.
pub fn capture_chance(
    item_potency: f32,
    target: TargetResistance,
    player: DecompilerBonuses,
) -> f32 {
    let hp_penalty = (CAPTURE_HP_PENALTY - player.hp_penalty_reduction).max(0.0);
    let base = item_potency
        * (CAPTURE_POTENCY_CEILING - target.hp_fraction * hp_penalty)
        * (1.0 - target.taming_difficulty * CAPTURE_DIFFICULTY_PENALTY);
    let skill_multiplier = 1.0 + player.skill as f32 * DECOMPILER_SKILL_BONUS;
    let boost_multiplier = 1.0 + player.capture_boost_pct as f32 / 100.0;
    let attempt_multiplier = 1.0
        + (target.prior_attempts.min(DECOMPILE_ATTEMPT_BONUS_CAP) * DECOMPILE_ATTEMPT_BONUS_PCT)
            as f32
            / 100.0;
    (base * skill_multiplier * boost_multiplier * attempt_multiplier)
        .clamp(CAPTURE_CHANCE_MIN, CAPTURE_CHANCE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The player contributing nothing of their own — the baseline every
    /// test below varies one term away from.
    const RAW: DecompilerBonuses = DecompilerBonuses {
        skill: 0,
        hp_penalty_reduction: 0.0,
        capture_boost_pct: 0,
    };

    fn with_skill(skill: i32) -> DecompilerBonuses {
        DecompilerBonuses {
            skill,
            ..Default::default()
        }
    }

    fn exploit_focus(reduction: f32) -> DecompilerBonuses {
        DecompilerBonuses {
            hp_penalty_reduction: reduction,
            ..Default::default()
        }
    }

    /// A target nobody has tried to decompile yet — the baseline every test
    /// below varies one term away from.
    fn fresh(hp_fraction: f32, taming_difficulty: f32) -> TargetResistance {
        TargetResistance {
            hp_fraction,
            taming_difficulty,
            prior_attempts: 0,
        }
    }

    #[test]
    fn weaker_prey_is_easier_to_tame() {
        let full_hp = capture_chance(0.55, fresh(1.0, 0.2), RAW);
        let low_hp = capture_chance(0.55, fresh(0.1, 0.2), RAW);
        assert!(low_hp > full_hp);
    }

    #[test]
    fn harder_species_resist_taming() {
        let easy = capture_chance(0.55, fresh(0.5, 0.1), RAW);
        let hard = capture_chance(0.55, fresh(0.5, 0.9), RAW);
        assert!(hard < easy);
    }

    #[test]
    fn higher_decompiler_skill_improves_odds() {
        let unskilled = capture_chance(0.55, fresh(0.5, 0.5), RAW);
        let skilled = capture_chance(0.55, fresh(0.5, 0.5), with_skill(10));
        assert!(skilled > unskilled);
    }

    #[test]
    fn hp_penalty_reduction_helps_a_target_that_has_not_been_worn_down() {
        let plain = capture_chance(0.4, fresh(1.0, 0.4), RAW);
        let focused = capture_chance(0.4, fresh(1.0, 0.4), exploit_focus(0.15));
        assert!(
            focused > plain,
            "Exploit Focus should make a full-HP target more decompilable: \
             {focused} vs {plain}"
        );
    }

    /// The reason this perk is worth having alongside `skill` rather than
    /// duplicating it: it buys attempts on *healthier* programs, so it is
    /// worth nothing once the target is already drained. A change that made
    /// it a flat bonus would break this and re-introduce the overlap with
    /// `DECOMPILER_SKILL_PER_LEVEL` that it exists to avoid.
    #[test]
    fn hp_penalty_reduction_is_inert_against_a_fully_drained_target() {
        let plain = capture_chance(0.4, fresh(0.0, 0.4), RAW);
        let focused = capture_chance(0.4, fresh(0.0, 0.4), exploit_focus(0.15));
        assert_eq!(
            plain, focused,
            "at zero HP there is no HP penalty left to reduce"
        );
    }

    /// An absurd stack of `Perk::ExploitFocus` may drive the penalty to zero
    /// but must never invert it — a target at full Integrity can never be
    /// easier to decompile than the same target at death's door.
    #[test]
    fn hp_penalty_never_inverts_however_much_it_is_reduced() {
        let full_hp = capture_chance(0.4, fresh(1.0, 0.4), exploit_focus(5.0));
        let drained = capture_chance(0.4, fresh(0.0, 0.4), exploit_focus(5.0));
        assert!(
            full_hp <= drained,
            "remaining HP must never help: {full_hp} vs {drained}"
        );
    }

    /// The whole point of the multiplicative skill term: a well-practiced
    /// player is better at everything, but a boss-grade species stays
    /// meaningfully harder than trash forever. Under the old additive term
    /// both of these pinned to `CAPTURE_CHANCE_MAX` and the spread vanished.
    #[test]
    fn high_skill_does_not_flatten_the_gap_between_easy_and_boss_species() {
        let drone = capture_chance(0.4, fresh(0.0, 0.15), with_skill(40));
        let boss = capture_chance(0.4, fresh(0.0, 0.9), with_skill(40));

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

    /// The whole point of `capture_boost_pct` existing separately from
    /// `skill`: it's a temporary field-routine effect, not a permanent
    /// stat, but it has to move the same roll the same way.
    #[test]
    fn capture_boost_pct_raises_chance_multiplicatively() {
        let base = capture_chance(0.4, fresh(0.5, 0.4), RAW);
        let boosted = capture_chance(
            0.4,
            fresh(0.5, 0.4),
            DecompilerBonuses {
                capture_boost_pct: 20,
                ..RAW
            },
        );
        assert!(
            boosted > base,
            "a running CaptureBoost should raise the odds: {boosted} vs {base}"
        );
        assert!(
            (boosted - base * 1.2).abs() < 1e-6,
            "20 percentage points should scale the chance by 1.2x ahead of the \
             final clamp: {boosted} vs {}",
            base * 1.2
        );
    }

    fn after_attempts(prior_attempts: u32) -> TargetResistance {
        TargetResistance {
            prior_attempts,
            ..fresh(0.5, 0.4)
        }
    }

    #[test]
    fn each_attempt_raises_the_odds_of_the_next_one() {
        let first = capture_chance(0.4, after_attempts(0), RAW);
        let second = capture_chance(0.4, after_attempts(1), RAW);
        let third = capture_chance(0.4, after_attempts(2), RAW);
        assert!(
            second > first && third > second,
            "each attempt should leave the next better off: \
             {first} then {second} then {third}"
        );
        assert!(
            (second - first * 1.1).abs() < 1e-6,
            "one prior attempt should scale the chance by 1.1x: {second} vs {}",
            first * 1.1
        );
    }

    /// The cap is what keeps this a "you are wearing it down" mechanic
    /// rather than a pity meter that eventually guarantees any capture.
    #[test]
    fn attempts_past_the_cap_buy_nothing_further() {
        let capped = capture_chance(0.4, after_attempts(DECOMPILE_ATTEMPT_BONUS_CAP), RAW);
        let way_past = capture_chance(0.4, after_attempts(DECOMPILE_ATTEMPT_BONUS_CAP + 50), RAW);
        assert_eq!(
            capped, way_past,
            "persistence past the cap must buy nothing: {capped} vs {way_past}"
        );

        let fresh_target = capture_chance(0.4, after_attempts(0), RAW);
        assert!(
            (capped - fresh_target * 1.5).abs() < 1e-6,
            "the cap should be worth exactly 1.5x: {capped} vs {}",
            fresh_target * 1.5
        );
    }

    /// A hard species stays hard. The bonus multiplies like `skill` does
    /// rather than adding to the base, so it can't be used to skip past
    /// `taming_difficulty` — the same property `high_skill_does_not_flatten_
    /// the_gap_between_easy_and_boss_species` pins for skill.
    #[test]
    fn a_maxed_out_attempt_streak_does_not_close_the_gap_between_species() {
        let easy = capture_chance(
            0.4,
            TargetResistance {
                prior_attempts: DECOMPILE_ATTEMPT_BONUS_CAP,
                ..fresh(0.0, 0.15)
            },
            RAW,
        );
        let hard = capture_chance(
            0.4,
            TargetResistance {
                prior_attempts: DECOMPILE_ATTEMPT_BONUS_CAP,
                ..fresh(0.0, 0.9)
            },
            RAW,
        );
        assert!(
            easy > hard * 1.5,
            "an easy species must stay far ahead of a resistant one however \
             many attempts have been spent: {easy} vs {hard}"
        );
    }

    #[test]
    fn chance_is_always_within_bounds() {
        for hp in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for diff in [0.0, 0.5, 1.0] {
                for skill in [0, 5, 50] {
                    for reduction in [0.0, 0.15, 5.0] {
                        let c = capture_chance(
                            0.55,
                            fresh(hp, diff),
                            DecompilerBonuses {
                                skill,
                                hp_penalty_reduction: reduction,
                                capture_boost_pct: 0,
                            },
                        );
                        assert!((0.05..=0.95).contains(&c), "out of bounds: {c}");
                    }
                }
            }
        }
    }
}
