use crate::components::{Experience, Stats};

const HP_PER_LEVEL: i32 = 4;
const ATK_PER_LEVEL: i32 = 1;
const DEF_PER_LEVEL: i32 = 1;

/// Fraction of in-level XP knocked back by a "setback" penalty (a flatline,
/// a Forgiving-mode reboot, or a forced jack-out mid-battle) — see
/// `apply_setback_xp_penalty`. Deliberately mild: it erodes progress toward
/// the next level, never the level or stats themselves.
const SETBACK_XP_PENALTY_FRACTION: f64 = 0.2;

/// XP required to advance from `level` to `level + 1`.
pub fn xp_for_level(level: u32) -> u32 {
    level * 20
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
/// enough gain can jump more than one level at once). Each level-up grows
/// max HP/attack/defense and fully heals. Returns how many levels were
/// gained, so callers can decide whether to log a "level up" message.
pub fn add_xp(exp: &mut Experience, stats: &mut Stats, gained: u32) -> u32 {
    exp.xp += gained;
    let mut levels_gained = 0;
    while exp.xp >= exp.xp_to_next {
        exp.xp -= exp.xp_to_next;
        exp.level += 1;
        exp.xp_to_next = xp_for_level(exp.level);
        stats.max_hp += HP_PER_LEVEL;
        stats.hp = stats.max_hp;
        stats.atk += ATK_PER_LEVEL;
        stats.def += DEF_PER_LEVEL;
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
        let levels = add_xp(&mut exp, &mut stats, 5);
        assert_eq!(levels, 0);
        assert_eq!(exp.level, 1);
        assert_eq!(exp.xp, 5);
        assert_eq!(stats.max_hp, 10);
    }

    #[test]
    fn enough_xp_levels_up_and_grows_stats() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(&mut exp, &mut stats, 20);
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
        let levels = add_xp(&mut exp, &mut stats, 65);
        assert_eq!(levels, 2);
        assert_eq!(exp.level, 3);
        assert_eq!(exp.xp, 5);
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
