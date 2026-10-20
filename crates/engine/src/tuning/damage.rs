// ─────────────────────────────────────────────────────────────────────────
// Damage types
// ─────────────────────────────────────────────────────────────────────────

/// The most a summed resistance to one damage type may cut, in percent.
/// Gear stacks resistance additively, so the cap is what keeps a full set
/// from making a type harmless.
pub const DAMAGE_RESIST_CAP: i32 = 75;

/// The resistance (percent) at which a type's side effect no longer lands.
pub const SIDE_EFFECT_BLOCK_THRESHOLD: i32 = 50;
