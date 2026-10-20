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
