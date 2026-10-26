//! Weapon crafting (ADR 0031): every figure `gear_craft::weapon::compile`
//! turns a design into. Frame bases are lifted from the authored weapon
//! each frame stands for; the rest are costs that make every modifier a
//! trade on the weapon's own axes (band, accuracy, recharge) rather than a
//! free upgrade.
//!
//! Tables indexed by status are in `routine_craft::StatusKind` order —
//! bleed, stun, exposed, poison, throttled, locked — so a weapon and a
//! routine naming the same status read the same row number.

/// One Frame's v1 figures, before versions and modifiers.
#[derive(Clone, Copy, Debug)]
pub struct GearFrameFigures {
    /// Cells the weapon swings from, before an Extend part.
    pub range: u32,
    pub atk: i32,
    /// The damage band's two ends.
    pub band: (i32, i32),
    /// An authored `accuracy`, before `ItemDef::worn_stats` adds the class
    /// bonus for melee or ranged.
    pub accuracy: i32,
}

/// Edge, Lance, Whip, Launcher (`weapon::Frame` order). Lifted from
/// `kinetic_edge`/`shiv_routine`, `arc_lance`, `monofilament_whip` and
/// `plasma_router`/`siege_compiler`.
pub const GEAR_FRAME: [GearFrameFigures; 4] = [
    GearFrameFigures {
        range: 1,
        atk: 1,
        band: (3, 5),
        accuracy: 3,
    },
    GearFrameFigures {
        range: 2,
        atk: 2,
        band: (3, 6),
        accuracy: 2,
    },
    GearFrameFigures {
        range: 2,
        atk: 2,
        band: (3, 8),
        accuracy: 0,
    },
    GearFrameFigures {
        range: 3,
        atk: 2,
        band: (4, 7),
        accuracy: 0,
    },
];

/// What v1, v2 and v3 multiply a Frame's attack and band by. Gentler than
/// `CRAFT_VERSION_MAGNITUDE`: a routine's version is a new tier of effect,
/// a weapon's is a better blade, and the shipped weapons span 1 : 1.9 from
/// `shiv_routine` to `overclock_core`.
pub const GEAR_VERSION_MAGNITUDE: [f32; 3] = [1.0, 1.35, 1.75];

/// Modifiers a design holds before any capacity research. Mirrors
/// `CRAFT_PART_CAP_START` (spec).
pub const GEAR_PART_CAP_START: usize = 2;
/// The most modifiers a design may ever hold.
pub const GEAR_PART_CAP_MAX: usize = 6;

/// The accuracy an Extend part costs; the weapon also loses the melee class
/// bonus when it stops being melee.
pub const GEAR_EXTEND_ACCURACY: i32 = -1;

/// Band multiplier of a Group and an Everyone reach. `scatter_lance` is a
/// group sweep at 3-6 where its single-target sibling swings 5-10;
/// `broadcast_storm` hits everyone at 4-8.
pub const GEAR_REACH_BAND: [f32; 2] = [0.7, 0.55];
/// Rounds between wide swings, Group then Everyone
/// (`scatter_lance` 2, `broadcast_storm` 5).
pub const GEAR_REACH_RECHARGE: [u32; 2] = [2, 5];

/// Band multiplier of any damage type: the type's own side effect is its
/// benefit, and every type pays the same so none is a cheaper pick.
pub const GEAR_TYPE_BAND: f32 = 0.9;

/// A rider's `(chance, rounds, power)` per status. Every row is below
/// `CRAFT_RIDER`'s row for the same status on chance, and no row exceeds it
/// on rounds or power — `a_weapon_rider_is_weaker_than_a_routines`. Stun's
/// row is never read: a stun on a battle map costs no turn
/// (`stun-costs-no-turn-on-a-battle-map`), so a rider cannot offer it.
pub const GEAR_RIDER: [(f32, u32, i32); 6] = [
    (0.3, 2, 2),
    (0.0, 0, 0),
    (0.25, 2, 0),
    (0.25, 2, 3),
    (0.25, 2, 0),
    (0.25, 2, 0),
];

/// Accuracy a rider costs, per status, beside its band: a whole-point cost
/// that survives rounding at a small band, so a stronger rider is never a
/// free pick over a weaker one.
pub const GEAR_RIDER_ACCURACY: [i32; 6] = [-1, 0, 0, -1, 0, 0];

/// Band multiplier a rider costs, per status; a stronger rider costs more.
pub const GEAR_RIDER_BAND: [f32; 6] = [0.85, 1.0, 0.92, 0.85, 0.92, 0.92];

/// Percent more damage an Exploit does to a target carrying its status.
/// Statuses that are harder to land, or that cost the target its turn, pay
/// more.
pub const GEAR_EXPLOIT_PERCENT: [u32; 6] = [20, 40, 25, 25, 25, 25];

/// The accuracy an Exploit costs, per status: a bigger payoff costs more,
/// so no status is a free pick over another.
pub const GEAR_EXPLOIT_ACCURACY: [i32; 6] = [-1, -4, -2, -2, -2, -2];

/// A Tilt trades one axis for the other: Precise is accuracy for band,
/// Heavy is band for accuracy.
pub const GEAR_TILT_PRECISE_ACCURACY: i32 = 2;
pub const GEAR_TILT_PRECISE_BAND: f32 = 0.85;
pub const GEAR_TILT_HEAVY_ACCURACY: i32 = -2;
pub const GEAR_TILT_HEAVY_BAND: f32 = 1.15;

/// Decompiler a Capture part adds (`shim_blade` 1, `black_ice_pick` 2) and
/// the band it costs.
pub const GEAR_CAPTURE_DECOMPILER: i32 = 1;
pub const GEAR_CAPTURE_BAND: f32 = 0.9;

/// What a granted routine costs the weapon, per Proc (spec: band and
/// accuracy).
pub const GEAR_PROC_BAND: f32 = 0.85;
pub const GEAR_PROC_ACCURACY: i32 = -1;

/// Flat value a rider, an exploit and a proc add to `design_power`, which
/// `copy_power` cannot see because those fields are runtime-only. Without
/// them the riskiest parts would rate *weaker* than a plain weapon.
pub const GEAR_RIDER_POWER: [i32; 6] = [3, 0, 2, 3, 2, 2];
/// See `GEAR_RIDER_POWER`.
pub const GEAR_EXPLOIT_POWER: [i32; 6] = [2, 4, 3, 3, 3, 3];
/// See `GEAR_RIDER_POWER`: `clock_skew`, `core_dump`, `interrupt_request`.
pub const GEAR_PROC_POWER: [i32; 3] = [3, 3, 3];

/// The most `design_power` any compiled design may have, per Frame (Edge,
/// Lance, Whip, Launcher) and version. Tuned just above the best design of
/// the frame, and held below the best authored weapon of the same range by
/// `a_ceiling_never_passes_the_authored_weapon_of_its_range`, so a crafted
/// weapon never makes the drops pointless.
pub const GEAR_FRAME_CEILING: [[i32; 3]; 4] =
    [[35, 42, 54], [37, 47, 62], [37, 49, 64], [35, 50, 64]];
