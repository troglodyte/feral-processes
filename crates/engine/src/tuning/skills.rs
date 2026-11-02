//! Work-skill pace and worth (ADR 0038). Only numbers; `skills/` owns the
//! mechanics and the base labour code owns what a level is worth.

/// Highest skill level; practice stops accruing here.
pub const SKILL_LEVEL_CAP: u32 = 20;

/// Practice to leave `level` is `SKILL_COST_BASE + SKILL_COST_QUAD * level^2`.
///
/// **Pace intent.** Assumed: a worker is kept on one job and does progressing
/// work on about 60% of ticks, a zone is about 2,500 ticks and a full game
/// about 25,000. `docs/measurements/2026-09-06-run-length.md` gives only a
/// floor of 5,300-6,900 ticks to a mature state and no zone length, so these
/// are assumptions to retune. That gives roughly 2,200 practice to reach level
/// 10 ("competent") after about two zones, and roughly 15,800 to reach the
/// cap near the end of a full run.
pub const SKILL_COST_BASE: u32 = 50;
pub const SKILL_COST_QUAD: u32 = 6;

/// The level at which a skill's scale is exactly 1.0: today's behaviour.
pub const SKILL_NEUTRAL_LEVEL: u32 = 10;

/// Work scale is `skills::scale(level, WORK_SKILL_SPREAD)`: half speed at
/// level 0, today's speed at 10, 1.5x at the cap.
pub const WORK_SKILL_SPREAD: f32 = 0.5;

/// Combat scale is `skills::scale(level, COMBAT_SKILL_SPREAD)`: 10% worse at
/// level 0, today's figure at 10, 10% better at the cap (ADR 0040).
pub const COMBAT_SKILL_SPREAD: f32 = 0.1;

// Combat practice per event. **Pace intent:** a body that fights most
// encounters reaches level 10 after about two zones and the cap near the end
// of a full run. `SKILL_COST_*` makes level 10 cost 2,210 practice and the
// cap 15,800. `docs/measurements/` records no fights-per-zone, so these rest
// on an assumption to retune: per zone a fighting body makes about 100 attack
// rolls, is rolled at about 100 times, lands about 60 of its own rolls and
// pays for about 35 routines, and a full run is about 12 zones.

/// 100 rolls a zone x 11 = 1,100 a zone: level 10 in two zones.
pub const AIM_PRACTICE_PER_ROLL: u32 = 11;
/// 60 landed a zone x 18 = 1,080 a zone.
pub const FORCE_PRACTICE_PER_HIT: u32 = 18;
/// 100 rolls at the body a zone x 11 = 1,100 a zone.
pub const DEFENCE_PRACTICE_PER_ROLL: u32 = 11;
/// 35 charges a zone x 32 = 1,120 a zone.
pub const ROUTINES_PRACTICE_PER_CHARGE: u32 = 32;
