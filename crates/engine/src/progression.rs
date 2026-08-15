use crate::components::{Experience, Stats};
use crate::tuning::{
    ATK_PER_LEVEL, DEF_PER_LEVEL, DIFFICULTY_EASY_MAX, HP_PER_LEVEL, SETBACK_XP_PENALTY_FRACTION,
    XP_CHALLENGE_CEIL, XP_CHALLENGE_FLOOR, XP_PER_LEVEL_STEP,
};

/// One stat's flat per-level growth, scaled by `growth_multiplier` and
/// rounded to the nearest whole point. With `ATK_PER_LEVEL`/`DEF_PER_LEVEL`
/// both at 2, a multiplier has to cross a rounding boundary (roughly
/// +0.25) to actually change those two — `HP_PER_LEVEL` (24) has much finer
/// effective granularity.
fn scaled_growth(per_level: i32, growth_multiplier: f32) -> i32 {
    (per_level as f32 * growth_multiplier).round() as i32
}

/// What a call to `add_xp` granted: how many levels, and the stat growth
/// those levels came with, summed across all of them.
///
/// Returned rather than left for callers to work out by diffing `Stats`
/// themselves, because three separate log sites report it — the player, a
/// party member and a posted worker — and a fourth added later would
/// otherwise silently report nothing. `hp` is deliberately absent: a
/// level-up full-heals, so a delta on it measures the heal, not the growth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelGain {
    pub levels: u32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
}

impl LevelGain {
    /// Folds another `add_xp`'s gain into this one, so a tally can cover a
    /// whole fight rather than a single kill (see `resources::XpTally`).
    ///
    /// Plain summation is exactly right here because `stat_rows` recovers
    /// each row's "before" by subtracting the delta from the stats as they
    /// stand now: three kills' deltas summed, read against the stats after
    /// the third, give the range across all three.
    pub fn absorb(&mut self, other: LevelGain) {
        self.levels += other.levels;
        self.max_hp += other.max_hp;
        self.atk += other.atk;
        self.def += other.def;
    }

    /// The three rows a level-up's stat block always has, measured against
    /// `stats` as they stand *after* the `add_xp` call that produced this —
    /// so each row's "before" is recovered by subtracting the delta rather
    /// than by the caller having snapshotted anything.
    pub fn stat_rows(&self, stats: &Stats) -> [StatRow; 3] {
        [
            StatRow::grown("Max HP", stats.max_hp, self.max_hp),
            StatRow::grown("ATK", stats.atk, self.atk),
            StatRow::grown("DEF", stats.def, self.def),
        ]
    }
}

/// One line of a level-up's stat block: what grew, and from what to what.
///
/// Deliberately not tied to `LevelGain`'s three fields. The player's block
/// also reports the Perk Point and Decompiler skill a level grants, and
/// neither is anything `add_xp` computes — so the block is built from a list
/// of rows that any caller can extend, rather than from a struct that would
/// carry two fields only one of the three callers ever fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatRow {
    pub label: &'static str,
    pub before: i32,
    pub after: i32,
}

impl StatRow {
    pub fn new(label: &'static str, before: i32, after: i32) -> Self {
        Self {
            label,
            before,
            after,
        }
    }

    /// A row phrased as "this ended at `after`, having gained `delta`".
    fn grown(label: &'static str, after: i32, delta: i32) -> Self {
        Self::new(label, after - delta, after)
    }
}

/// The indented lines under a level-up announcement, one per stat that
/// actually moved. A stat that didn't is dropped: `ATK_PER_LEVEL` and
/// `DEF_PER_LEVEL` are both 2, so a low `growth_multiplier` rounds them away
/// on a given level (see `scaled_growth`), and "ATK 14 → 14" is noise.
///
/// Lives here rather than in a frontend because all three sites that
/// announce a level-up push these straight into `MessageLog` — the engine
/// owns the wording of a log line, and a renderer-side version would leave
/// the base log's copy unformatted.
pub fn stat_block(rows: &[StatRow]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.before != r.after)
        .map(|r| format!("  {} {} → {}", r.label, r.before, r.after))
        .collect()
}

/// XP required to advance from `level` to `level + 1`.
pub fn xp_for_level(level: u32) -> u32 {
    level * XP_PER_LEVEL_STEP
}

/// What a defeated program is worth: its whole HP bar, scaled by how hard it
/// was to put down.
///
/// `power_ratio` is `game::inspection::power_ratio` — the victim's
/// `Stats::power` over the player's, and the very number `difficulty_color`
/// buckets into the con-colours already drawn on the map. That sharing is
/// the point rather than a convenience: the glyph's colour is the only
/// advance notice a fight's XP value gets, so the two must be one
/// computation. Full XP lands at `DIFFICULTY_EASY_MAX`, the green/yellow
/// boundary, which makes the whole rule "green pays less, yellow and up pays
/// full or more".
///
/// Takes the ratio rather than the two powers so it stays a pure function of
/// numbers, testable without a `Game` — the same reason `add_xp` takes
/// `xp_boost_pct` instead of reading a buff off the world. `Game::kill_xp`
/// is the one caller that knows where the powers come from.
///
/// See `XP_CHALLENGE_FLOOR`/`XP_CHALLENGE_CEIL` for why both clamps are
/// load-bearing, in opposite directions.
pub fn kill_xp(victim_max_hp: i32, power_ratio: f64) -> u32 {
    let factor = (power_ratio / DIFFICULTY_EASY_MAX).clamp(XP_CHALLENGE_FLOOR, XP_CHALLENGE_CEIL);
    (victim_max_hp.max(0) as f64 * factor).round() as u32
}

/// `base` after `levels_gained` level-ups at `growth_multiplier`, fully
/// healed — the same growth `add_xp` applies per level-up, computed
/// directly rather than by spending XP one level at a time. Lets balance
/// projections (see `crate::balance_sim`) reuse the real growth constants
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
/// `level_cap` — an already-capped entity doesn't even accumulate the XP.
/// `None` means no ceiling at all (the player); creatures pass
/// `Some(CREATURE_MAX_LEVEL)`. Each level-up grows max HP/attack/defense
/// (scaled by `growth_multiplier` — see `SpeciesDef::growth_multiplier`;
/// pass `BASELINE_GROWTH_MULTIPLIER` for the player, who has no species)
/// and fully heals. Returns a `LevelGain` — how many levels, and the growth
/// they came with — so callers can both decide whether to log a "level up"
/// message and say what it gave.
///
/// `xp_boost_pct` scales `gained` itself, before anything else runs, by a
/// running `FieldBuffKind::XpBoost` field buff's power in percentage
/// points — callers with no such buff pass `0`. Kept as a parameter rather
/// than read off the world in here: `add_xp` has to stay a pure function so
/// it's testable without constructing a `Game`.
pub fn add_xp(
    exp: &mut Experience,
    stats: &mut Stats,
    gained: u32,
    growth_multiplier: f32,
    level_cap: Option<u32>,
    xp_boost_pct: i32,
) -> LevelGain {
    let cap = level_cap.unwrap_or(u32::MAX);
    if exp.level >= cap {
        return LevelGain::default();
    }
    let gained = (gained as f32 * (1.0 + xp_boost_pct as f32 / 100.0)).round() as u32;
    exp.xp += gained;
    let mut gain = LevelGain::default();
    while exp.level < cap && exp.xp >= exp.xp_to_next {
        exp.xp -= exp.xp_to_next;
        exp.level += 1;
        exp.xp_to_next = xp_for_level(exp.level);
        let (hp, atk, def) = (
            scaled_growth(HP_PER_LEVEL, growth_multiplier),
            scaled_growth(ATK_PER_LEVEL, growth_multiplier),
            scaled_growth(DEF_PER_LEVEL, growth_multiplier),
        );
        stats.max_hp += hp;
        stats.hp = stats.max_hp;
        stats.atk += atk;
        stats.def += def;
        gain.levels += 1;
        gain.max_hp += hp;
        gain.atk += atk;
        gain.def += def;
    }
    gain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::{
        BASELINE_GROWTH_MULTIPLIER, CREATURE_MAX_LEVEL, DIFFICULTY_EASY_MAX, DIFFICULTY_EVEN_MAX,
        XP_CHALLENGE_CEIL, XP_CHALLENGE_FLOOR,
    };

    /// The rule a player can state from the map's con-colours alone: full XP
    /// arrives exactly where green becomes yellow, so anything reading yellow
    /// or worse pays its whole HP bar and anything green pays less.
    #[test]
    fn a_kill_pays_its_full_hp_bar_at_the_green_yellow_boundary() {
        assert_eq!(kill_xp(200, DIFFICULTY_EASY_MAX), 200);
        assert!(
            kill_xp(200, DIFFICULTY_EASY_MAX - 0.2) < 200,
            "still green: pays less than its bar"
        );
        assert!(
            kill_xp(200, DIFFICULTY_EVEN_MAX) > 200,
            "an even match is past the boundary and pays more"
        );
    }

    /// The floor is what keeps the opening ring from paying literally
    /// nothing once the party outclasses it — farming should be pointless,
    /// not broken.
    #[test]
    fn an_outclassed_victim_pays_the_floor_rather_than_nothing() {
        assert_eq!(kill_xp(200, 0.001), (200.0 * XP_CHALLENGE_FLOOR) as u32);
        assert_eq!(
            kill_xp(200, 0.0),
            kill_xp(200, 0.001),
            "the floor holds all the way down, so a ratio of zero is not a special case"
        );
    }

    /// Without the ceiling a Stack guardian would earn a multiplier on top
    /// of HP that `STACK_DEPTH_STAT_STEP` has already inflated — the double
    /// count that made a handful of deep fights worth several levels.
    #[test]
    fn an_overwhelming_victim_pays_the_ceiling_rather_than_its_whole_ratio() {
        let capped = (200.0 * XP_CHALLENGE_CEIL) as u32;
        assert_eq!(kill_xp(200, 8.0), capped);
        assert_eq!(
            kill_xp(200, 40.0),
            capped,
            "five times deeper out of its depth is still the same cap"
        );
    }

    /// The whole point of the change: the *same* program is worth less to a
    /// stronger party. Asserted as a fall across a rising player power
    /// rather than against fixed numbers, so it fails if the factor is ever
    /// flattened back to a constant.
    #[test]
    fn the_same_victim_pays_less_as_the_party_outgrows_it() {
        let victim_power = 53.0; // a zone-1 drone, 48 + 4 + 1
        let earned: Vec<u32> = [98.0, 200.0, 400.0]
            .iter()
            .map(|player_power| kill_xp(48, victim_power / player_power))
            .collect();
        assert!(
            earned[0] > earned[1] && earned[1] > earned[2],
            "a drone should pay a level-1 party more than a grown one, got {earned:?}"
        );
    }

    #[test]
    fn a_victim_with_no_hp_bar_pays_nothing_at_any_ratio() {
        assert_eq!(kill_xp(0, 4.0), 0);
    }

    fn base_stats() -> Stats {
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 5,
            def: 5,
        }
    }

    #[test]
    fn a_multi_level_jump_reports_the_summed_stat_growth() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // Exactly the two levels `large_xp_gain_can_grant_multiple_levels`
        // buys, so every delta reported has to be twice one level's growth
        // rather than the last level's alone.
        let two_levels = xp_for_level(1) + xp_for_level(2);
        let gain = add_xp(
            &mut exp,
            &mut stats,
            two_levels,
            BASELINE_GROWTH_MULTIPLIER,
            None,
            0,
        );
        assert_eq!(gain.levels, 2);
        assert_eq!(gain.max_hp, 2 * HP_PER_LEVEL);
        assert_eq!(gain.atk, 2 * ATK_PER_LEVEL);
        assert_eq!(gain.def, 2 * DEF_PER_LEVEL);
    }

    #[test]
    fn a_growth_multiplier_that_rounds_a_stat_away_reports_no_gain_for_it() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // 1.1x leaves ATK/DEF_PER_LEVEL (2) rounding back to 2 — the case
        // `scaled_growth`'s doc warns about — while HP_PER_LEVEL (24) moves.
        // A stat that did move must still be reported, so this is not just
        // "everything is zero". The window is narrower than it was at 1 point
        // a level: 1.25x now genuinely moves ATK, which is the granularity
        // `HP_PER_LEVEL`'s `K = 2` was meant to buy back.
        let gain = add_xp(&mut exp, &mut stats, xp_for_level(1), 1.1, None, 0);
        assert_eq!(gain.levels, 1);
        assert_eq!(gain.max_hp, 26);
        assert_eq!(gain.atk, ATK_PER_LEVEL, "1.1 * 2 rounds back to 2");
        assert_eq!(gain.def, DEF_PER_LEVEL);

        // 0.2x rounds ATK/DEF away entirely: 0.4 rounds to 0.
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(&mut exp, &mut stats, xp_for_level(1), 0.2, None, 0);
        assert_eq!(gain.levels, 1);
        assert_eq!(gain.atk, 0, "0.2 * 2 rounds to no attack gain");
        assert_eq!(gain.def, 0);
    }

    #[test]
    fn a_gain_that_levels_nothing_reports_no_growth() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(&mut exp, &mut stats, 5, BASELINE_GROWTH_MULTIPLIER, None, 0);
        assert_eq!(gain.levels, 0);
        assert_eq!(gain.max_hp, 0);
        assert_eq!(gain.atk, 0);
        assert_eq!(gain.def, 0);
    }

    #[test]
    fn a_stat_block_skips_the_stats_that_did_not_move() {
        let rows = [
            StatRow::new("Max HP", 108, 120),
            StatRow::new("ATK", 14, 14),
            StatRow::new("DEF", 11, 12),
        ];
        assert_eq!(
            stat_block(&rows),
            vec![
                "  Max HP 108 → 120".to_string(),
                "  DEF 11 → 12".to_string()
            ],
            "a stat that did not change is noise, not news"
        );
    }

    #[test]
    fn a_stat_block_with_nothing_to_say_is_empty() {
        let rows = [StatRow::new("ATK", 14, 14)];
        assert!(stat_block(&rows).is_empty());
    }

    #[test]
    fn a_level_gain_builds_its_rows_from_the_stats_it_produced() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            BASELINE_GROWTH_MULTIPLIER,
            None,
            0,
        );
        // The rows are derived from the *post-call* stats and the deltas, so
        // they have to name the values the creature actually ended on.
        assert_eq!(
            stat_block(&gain.stat_rows(&stats)),
            vec![
                format!("  Max HP 10 → {}", 10 + HP_PER_LEVEL),
                format!("  ATK 5 → {}", 5 + ATK_PER_LEVEL),
                format!("  DEF 5 → {}", 5 + DEF_PER_LEVEL),
            ]
        );
    }

    #[test]
    fn xp_below_threshold_does_not_level_up() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(&mut exp, &mut stats, 5, BASELINE_GROWTH_MULTIPLIER, None, 0).levels;
        assert_eq!(levels, 0);
        assert_eq!(exp.level, 1);
        assert_eq!(exp.xp, 5);
        assert_eq!(stats.max_hp, 10);
    }

    #[test]
    fn enough_xp_levels_up_and_grows_stats() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            BASELINE_GROWTH_MULTIPLIER,
            None,
            0,
        )
        .levels;
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
        // Both levels' cost plus 5 to spare should clear both and carry the
        // remainder into the third.
        let levels = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1) + xp_for_level(2) + 5,
            BASELINE_GROWTH_MULTIPLIER,
            None,
            0,
        )
        .levels;
        assert_eq!(levels, 2);
        assert_eq!(exp.level, 3);
        assert_eq!(exp.xp, 5);
    }

    #[test]
    fn growth_multiplier_scales_stat_gains_per_level_up() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // 1.5x rounds HP_PER_LEVEL (24) to 36 and ATK/DEF_PER_LEVEL (2) to 3,
        // crossing the rounding boundary scaled_growth's doc comment warns
        // about — a smaller multiplier like 1.1 wouldn't move ATK/DEF at all.
        let levels = add_xp(&mut exp, &mut stats, xp_for_level(1), 1.5, None, 0).levels;
        assert_eq!(levels, 1);
        assert_eq!(
            stats.max_hp,
            10 + 36,
            "1.5x should scale HP growth up from 24 to 36"
        );
        assert_eq!(
            stats.atk,
            5 + 3,
            "1.5x should scale ATK growth up from 2 to 3"
        );
        assert_eq!(
            stats.def,
            5 + 3,
            "1.5x should scale DEF growth up from 2 to 3"
        );
    }

    #[test]
    fn stats_after_levels_matches_add_xp_at_the_same_growth_multiplier() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        for _ in 0..3 {
            let needed = exp.xp_to_next;
            add_xp(&mut exp, &mut stats, needed, 1.5, None, 0);
        }
        let projected = stats_after_levels(base_stats(), 3, 1.5);
        assert_eq!(stats.max_hp, projected.max_hp);
        assert_eq!(stats.atk, projected.atk);
        assert_eq!(stats.def, projected.def);
    }

    #[test]
    fn add_xp_stops_leveling_at_the_creature_cap() {
        let mut exp = Experience {
            level: CREATURE_MAX_LEVEL,
            xp: 0,
            xp_to_next: xp_for_level(CREATURE_MAX_LEVEL),
        };
        let mut stats = base_stats();

        let levels = add_xp(
            &mut exp,
            &mut stats,
            10_000,
            BASELINE_GROWTH_MULTIPLIER,
            Some(CREATURE_MAX_LEVEL),
            0,
        )
        .levels;

        assert_eq!(
            levels, 0,
            "an already-maxed creature shouldn't level up further"
        );
        assert_eq!(exp.level, CREATURE_MAX_LEVEL);
        assert_eq!(
            exp.xp, 0,
            "XP awarded past the cap shouldn't even accumulate"
        );
        assert_eq!(stats.max_hp, 10, "stats shouldn't grow past the cap");
    }

    #[test]
    fn add_xp_caps_a_multi_level_jump_at_the_creature_cap() {
        let mut exp = Experience {
            level: CREATURE_MAX_LEVEL - 1,
            xp: 0,
            xp_to_next: xp_for_level(CREATURE_MAX_LEVEL - 1),
        };
        let mut stats = base_stats();

        // Enough XP to clear several levels if uncapped.
        let levels = add_xp(
            &mut exp,
            &mut stats,
            100_000,
            BASELINE_GROWTH_MULTIPLIER,
            Some(CREATURE_MAX_LEVEL),
            0,
        )
        .levels;

        assert_eq!(
            levels, 1,
            "should only be able to gain the one level up to the cap"
        );
        assert_eq!(exp.level, CREATURE_MAX_LEVEL);
    }

    /// The player passes no cap at all, so they keep leveling well past
    /// the ceiling creatures stop at.
    #[test]
    fn add_xp_without_a_cap_levels_past_the_creature_ceiling() {
        let mut exp = Experience {
            level: CREATURE_MAX_LEVEL,
            xp: 0,
            xp_to_next: xp_for_level(CREATURE_MAX_LEVEL),
        };
        let mut stats = base_stats();

        let levels = add_xp(
            &mut exp,
            &mut stats,
            100_000,
            BASELINE_GROWTH_MULTIPLIER,
            None,
            0,
        )
        .levels;

        assert!(levels > 0, "an uncapped entity should keep leveling");
        assert!(
            exp.level > CREATURE_MAX_LEVEL,
            "uncapped leveling should pass the creature ceiling, got {}",
            exp.level
        );
        assert!(
            stats.max_hp > 10,
            "uncapped level-ups should still grow stats"
        );
    }

    /// A 50% `XpBoost` turns three-quarters of a level's XP into more than
    /// the whole of it, clearing the threshold to level 2 — a case a flat
    /// "differs in the right direction" assertion wouldn't catch, since the
    /// unboosted gain alone wouldn't level up at all.
    #[test]
    fn xp_boost_pct_scales_gained_xp_before_leveling() {
        let three_quarters = xp_for_level(1) * 3 / 4;
        let mut unboosted_exp = Experience::default();
        let mut unboosted_stats = base_stats();
        let levels = add_xp(
            &mut unboosted_exp,
            &mut unboosted_stats,
            three_quarters,
            BASELINE_GROWTH_MULTIPLIER,
            None,
            0,
        )
        .levels;
        assert_eq!(
            levels, 0,
            "three-quarters of a level's XP alone doesn't clear the threshold"
        );
        assert_eq!(unboosted_exp.xp, three_quarters);

        let mut boosted_exp = Experience::default();
        let mut boosted_stats = base_stats();
        let levels = add_xp(
            &mut boosted_exp,
            &mut boosted_stats,
            three_quarters,
            BASELINE_GROWTH_MULTIPLIER,
            None,
            50,
        )
        .levels;
        assert_eq!(
            levels, 1,
            "a 50% XpBoost lifts three-quarters of a level past the whole of it"
        );
        assert_eq!(boosted_exp.level, 2);
        assert_eq!(
            boosted_exp.xp,
            three_quarters * 3 / 2 - xp_for_level(1),
            "the excess carries over into the new level"
        );
    }

    #[test]
    fn setback_penalty_docks_a_mild_fraction_of_in_level_xp() {
        let mut exp = Experience {
            level: 3,
            xp: 10,
            xp_to_next: 40,
        };
        let lost = apply_setback_xp_penalty(&mut exp);
        assert_eq!(lost, 2, "20% of 10 xp");
        assert_eq!(exp.xp, 8);
        assert_eq!(exp.level, 3, "a setback should never touch level");
        assert_eq!(
            exp.xp_to_next, 40,
            "a setback should never touch xp_to_next"
        );
    }

    #[test]
    fn setback_penalty_is_a_no_op_with_zero_xp() {
        let mut exp = Experience::default();
        assert_eq!(apply_setback_xp_penalty(&mut exp), 0);
        assert_eq!(exp.xp, 0);
    }
}
