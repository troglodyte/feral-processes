use crate::components::{Experience, Stats};

const HP_PER_LEVEL: i32 = 12;
const ATK_PER_LEVEL: i32 = 1;
const DEF_PER_LEVEL: i32 = 1;

/// Growth-rate multiplier for anything with no species-specific rate of
/// its own. The player (who has no species at all) always levels at this
/// rate; it's also `SpeciesDef::growth_multiplier`'s default, so a species
/// file written before that field existed keeps growing exactly as before.
pub const BASELINE_GROWTH_MULTIPLIER: f32 = 1.0;

/// Absolute level ceiling for any live entity (the player or a tamed
/// creature), regardless of XP source. `add_xp` stops leveling — and
/// stops accumulating XP at all — once this is reached, so a maxed-out
/// entity's `Experience::xp` just stalls instead of piling up into a huge,
/// meaningless number. This is a live-gameplay cap only: it deliberately
/// doesn't apply to `crate::balance`'s offline curve-shape projections,
/// which search well past any level actually reachable in play on
/// purpose (see that module's docs).
pub const MAX_LEVEL: u32 = 12;

/// One stat's flat per-level growth, scaled by `growth_multiplier` and
/// rounded to the nearest whole point. With `ATK_PER_LEVEL`/`DEF_PER_LEVEL`
/// both at 1, a multiplier has to cross a rounding boundary (roughly
/// +0.5) to actually change those two — `HP_PER_LEVEL` (12) has much finer
/// effective granularity.
fn scaled_growth(per_level: i32, growth_multiplier: f32) -> i32 {
    (per_level as f32 * growth_multiplier).round() as i32
}

/// Fraction of in-level XP knocked back by a "setback" penalty (a flatline,
/// a Forgiving-mode reboot, or a forced jack-out mid-battle) — see
/// `apply_setback_xp_penalty`. Deliberately mild: it erodes progress toward
/// the next level, never the level or stats themselves.
const SETBACK_XP_PENALTY_FRACTION: f64 = 0.2;

/// XP required to advance from `level` to `level + 1`.
pub fn xp_for_level(level: u32) -> u32 {
    level * 20
}

/// `base` after `levels_gained` level-ups at `growth_multiplier`, fully
/// healed — the same growth `add_xp` applies per level-up, computed
/// directly rather than by spending XP one level at a time. Lets balance
/// projections (see `crate::balance`) reuse the real growth constants
/// instead of re-deriving them.
pub fn stats_after_levels(base: Stats, levels_gained: u32, growth_multiplier: f32) -> Stats {
    let levels_gained = levels_gained as i32;
    let max_hp = base.max_hp + scaled_growth(HP_PER_LEVEL, growth_multiplier) * levels_gained;
    Stats {
        hp: max_hp,
        max_hp,
        atk: base.atk + scaled_growth(ATK_PER_LEVEL, growth_multiplier) * levels_gained,
        def: base.def + scaled_growth(DEF_PER_LEVEL, growth_multiplier) * levels_gained,
    }
}

/// Docks `exp` a mild fraction (`SETBACK_XP_PENALTY_FRACTION`) of its
/// current in-level XP as a death/jack-out penalty, returning how much was
/// lost (0 if there was none to lose). Never drops `xp` below 0 and never
/// touches `level` or `xp_to_next` — nothing drastic, just a setback.
pub fn apply_setback_xp_penalty(exp: &mut Experience) -> u32 {
    let lost = ((exp.xp as f64) * SETBACK_XP_PENALTY_FRACTION).round() as u32;
    exp.xp -= lost;
    lost
}

/// Adds `gained` XP, applying as many level-ups as the total allows (a big
/// enough gain can jump more than one level at once), stopping dead at
/// `MAX_LEVEL` — already-capped entities don't even accumulate the XP.
/// Each level-up grows max HP/attack/defense (scaled by
/// `growth_multiplier` — see `SpeciesDef::growth_multiplier`; pass
/// `BASELINE_GROWTH_MULTIPLIER` for the player, who has no species) and
/// fully heals. Returns how many levels were gained, so callers can decide
/// whether to log a "level up" message.
pub fn add_xp(exp: &mut Experience, stats: &mut Stats, gained: u32, growth_multiplier: f32) -> u32 {
    if exp.level >= MAX_LEVEL {
        return 0;
    }
    exp.xp += gained;
    let mut levels_gained = 0;
    while exp.level < MAX_LEVEL && exp.xp >= exp.xp_to_next {
        exp.xp -= exp.xp_to_next;
        exp.level += 1;
        exp.xp_to_next = xp_for_level(exp.level);
        stats.max_hp += scaled_growth(HP_PER_LEVEL, growth_multiplier);
        stats.hp = stats.max_hp;
        stats.atk += scaled_growth(ATK_PER_LEVEL, growth_multiplier);
        stats.def += scaled_growth(DEF_PER_LEVEL, growth_multiplier);
        levels_gained += 1;
    }
    levels_gained
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_stats() -> Stats {
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 5,
            def: 5,
        }
    }

    #[test]
    fn xp_below_threshold_does_not_level_up() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(&mut exp, &mut stats, 5, BASELINE_GROWTH_MULTIPLIER);
        assert_eq!(levels, 0);
        assert_eq!(exp.level, 1);
        assert_eq!(exp.xp, 5);
        assert_eq!(stats.max_hp, 10);
    }

    #[test]
    fn enough_xp_levels_up_and_grows_stats() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(&mut exp, &mut stats, 20, BASELINE_GROWTH_MULTIPLIER);
        assert_eq!(levels, 1);
        assert_eq!(exp.level, 2);
        assert_eq!(stats.max_hp, 10 + HP_PER_LEVEL);
        assert_eq!(stats.hp, stats.max_hp, "level up should fully heal");
        assert_eq!(stats.atk, 5 + ATK_PER_LEVEL);
        assert_eq!(stats.def, 5 + DEF_PER_LEVEL);
    }

    #[test]
    fn large_xp_gain_can_grant_multiple_levels() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // level 1->2 costs 20, 2->3 costs 40: 65 xp should clear both.
        let levels = add_xp(&mut exp, &mut stats, 65, BASELINE_GROWTH_MULTIPLIER);
        assert_eq!(levels, 2);
        assert_eq!(exp.level, 3);
        assert_eq!(exp.xp, 5);
    }

    #[test]
    fn growth_multiplier_scales_stat_gains_per_level_up() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // 1.5x rounds HP_PER_LEVEL (12) to 18 and ATK/DEF_PER_LEVEL (1) to 2,
        // crossing the rounding boundary scaled_growth's doc comment warns
        // about — a smaller multiplier like 1.25 wouldn't move ATK/DEF at all.
        let levels = add_xp(&mut exp, &mut stats, 20, 1.5);
        assert_eq!(levels, 1);
        assert_eq!(stats.max_hp, 10 + 18, "1.5x should scale HP growth up from 12 to 18");
        assert_eq!(stats.atk, 5 + 2, "1.5x should scale ATK growth up from 1 to 2");
        assert_eq!(stats.def, 5 + 2, "1.5x should scale DEF growth up from 1 to 2");
    }

    #[test]
    fn stats_after_levels_matches_add_xp_at_the_same_growth_multiplier() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        for _ in 0..3 {
            let needed = exp.xp_to_next;
            add_xp(&mut exp, &mut stats, needed, 1.5);
        }
        let projected = stats_after_levels(base_stats(), 3, 1.5);
        assert_eq!(stats.max_hp, projected.max_hp);
        assert_eq!(stats.atk, projected.atk);
        assert_eq!(stats.def, projected.def);
    }

    #[test]
    fn add_xp_stops_leveling_at_max_level() {
        let mut exp = Experience {
            level: MAX_LEVEL,
            xp: 0,
            xp_to_next: xp_for_level(MAX_LEVEL),
        };
        let mut stats = base_stats();

        let levels = add_xp(&mut exp, &mut stats, 10_000, BASELINE_GROWTH_MULTIPLIER);

        assert_eq!(levels, 0, "an already-maxed entity shouldn't level up further");
        assert_eq!(exp.level, MAX_LEVEL);
        assert_eq!(exp.xp, 0, "XP awarded past the cap shouldn't even accumulate");
        assert_eq!(stats.max_hp, 10, "stats shouldn't grow past the cap");
    }

    #[test]
    fn add_xp_caps_a_multi_level_jump_at_max_level() {
        let mut exp = Experience {
            level: MAX_LEVEL - 1,
            xp: 0,
            xp_to_next: xp_for_level(MAX_LEVEL - 1),
        };
        let mut stats = base_stats();

        // Enough XP to clear several levels if uncapped.
        let levels = add_xp(&mut exp, &mut stats, 100_000, BASELINE_GROWTH_MULTIPLIER);

        assert_eq!(levels, 1, "should only be able to gain the one level up to the cap");
        assert_eq!(exp.level, MAX_LEVEL);
    }

    #[test]
    fn setback_penalty_docks_a_mild_fraction_of_in_level_xp() {
        let mut exp = Experience { level: 3, xp: 10, xp_to_next: 40 };
        let lost = apply_setback_xp_penalty(&mut exp);
        assert_eq!(lost, 2, "20% of 10 xp");
        assert_eq!(exp.xp, 8);
        assert_eq!(exp.level, 3, "a setback should never touch level");
        assert_eq!(exp.xp_to_next, 40, "a setback should never touch xp_to_next");
    }

    #[test]
    fn setback_penalty_is_a_no_op_with_zero_xp() {
        let mut exp = Experience::default();
        assert_eq!(apply_setback_xp_penalty(&mut exp), 0);
        assert_eq!(exp.xp, 0);
    }
}
