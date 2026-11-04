//! The base AI storyline's figures (ADR 0042). Placeholders until the
//! balance table is measured and approved; only the *shape* of each formula
//! lives in `storyline/`.

/// World ticks the claimed program spends in its fugue at the station, about
/// a minute of game time.
pub const BASE_AI_FUGUE_TICKS: u32 = 120;

/// The fraction of its old research pace the AI starts with.
pub const BASE_AI_START_SPEED: f64 = 0.25;

/// Practice to earn the next Analysis point is
/// `BASE_AI_PRACTICE_BASE + BASE_AI_PRACTICE_QUAD * gained^2`, where `gained`
/// is the points earned since the fusion: the skills shape (ADR 0038).
pub const BASE_AI_PRACTICE_BASE: u32 = 50;
pub const BASE_AI_PRACTICE_QUAD: u32 = 2;

/// Analysis points above the fusion figure at which the AI stalls, indexed by
/// its level. Past the table it never stalls.
pub const BASE_AI_LEVEL_OFFSETS: [i32; 5] = [10, 25, 45, 70, 100];

/// Research points "Train your AI" costs to leave `level`.
pub const BASE_AI_TRAINING_COST_BASE: u32 = 20;
pub const BASE_AI_TRAINING_COST_PER_LEVEL: u32 = 20;
