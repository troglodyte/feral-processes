// ─────────────────────────────────────────────────────────────────────────
// Damage types
// ─────────────────────────────────────────────────────────────────────────

/// The most a summed resistance to one damage type may cut, in percent.
/// Gear stacks resistance additively, so the cap is what keeps a full set
/// from making a type harmless.
pub const DAMAGE_RESIST_CAP: i32 = 75;

/// The resistance (percent) at which a type's side effect no longer lands.
pub const SIDE_EFFECT_BLOCK_THRESHOLD: i32 = 50;

// ── Noise: jammed ───────────────────────────────────────────────────────

/// The built-in status a landed Noise hit arms (`damage::statuses`).
pub const NOISE_JAMMED_STATUS: &str = "jammed";

/// Percent a jammed body's accuracy is cut by.
pub const NOISE_JAM_ACCURACY_CUT: i32 = 30;

/// Rounds a Noise hit keeps its target jammed.
pub const NOISE_JAM_ROUNDS: u32 = 2;

// ── Radiation: corrupted ────────────────────────────────────────────────

/// The built-in status a landed Radiation hit arms (`damage::statuses`).
pub const RADIATION_CORRUPTED_STATUS: &str = "corrupted";

/// Integrity a corrupted body loses each round.
pub const RADIATION_CORRUPTION_POWER: i32 = 2;

/// Rounds a Radiation hit keeps its target corrupted.
pub const RADIATION_CORRUPTION_ROUNDS: u32 = 3;

// ── Thermal: heat ───────────────────────────────────────────────────────

/// The built-in status a landed Thermal hit arms (`damage::statuses`).
pub const THERMAL_HEAT_STATUS: &str = "heat";

/// Most stacks of heat a body can carry.
pub const THERMAL_HEAT_MAX_STACKS: u32 = 3;

/// Percent more damage a hot body takes, per stack.
pub const THERMAL_HEAT_DAMAGE_TAKEN_PCT: i32 = 10;

/// Rounds a Thermal hit keeps its target hot.
pub const THERMAL_HEAT_ROUNDS: u32 = 3;
