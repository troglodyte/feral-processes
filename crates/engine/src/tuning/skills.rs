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

/// Work scale is `SKILL_SCALE_FLOOR + SKILL_SCALE_PER_LEVEL * level`: half
/// speed at level 0, today's speed at 10, 1.5x at the cap.
pub const SKILL_SCALE_FLOOR: f32 = 0.5;
pub const SKILL_SCALE_PER_LEVEL: f32 = 0.05;
