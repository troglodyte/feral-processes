//! Routine crafting (ADR 0029): every figure `routine_craft::compile` turns a
//! build into. Each base figure names the shipped v1 file it was lifted from,
//! so retuning an authored routine and retuning its Core are one decision
//! made in two places on purpose — the authored files stay the wild
//! programs' kits, these are the player's.

/// The built-in status a `HealOverTime` arms. Defined in
/// `damage::statuses`, so it exists whatever `assets/statuses/` holds.
pub const REGEN_STATUS: &str = "regen";

/// What one Core is at v1.0, before any modifier. Not every field means
/// something to every Core (a Cleanse has no magnitude); an unused one is 0.
#[derive(Clone, Copy, Debug)]
pub struct CoreFigures {
    /// Rounds before it can be run again, at v1.0 with no Area or Trickle.
    pub cooldown: u32,
    /// Power cost at v1.0 with no modifier.
    pub power_cost: f32,
    /// Flat to-hit bonus, as an authored `accuracy`.
    pub accuracy: i32,
    /// The centre of the magnitude: points of damage, heal, stat or percent.
    pub magnitude: i32,
    /// Half-width of the band around `magnitude`.
    pub spread: i32,
    /// Rounds (turns for a Tamper) the effect lasts; a field Core's duration
    /// is `CRAFT_FIELD_DURATION` instead.
    pub rounds: u32,
    /// Power each version above v1.0 adds.
    pub cost_per_version: f32,
    /// Cooldown each version above v1.0 adds.
    pub cooldown_per_version: u32,
}

const fn figures(
    cooldown: u32,
    power_cost: f32,
    accuracy: i32,
    magnitude: i32,
    spread: i32,
    rounds: u32,
) -> CoreFigures {
    CoreFigures {
        cooldown,
        power_cost,
        accuracy,
        magnitude,
        spread,
        rounds,
        cost_per_version: CRAFT_COST_PER_VERSION,
        cooldown_per_version: CRAFT_COOLDOWN_PER_VERSION,
    }
}

// ---------------------------------------------------------------------------
// Versions
// ---------------------------------------------------------------------------

/// Magnitude of v1.0, v2.0, v3.0. The shipped families run 1 : 1.8 : 2.8
/// (`segfault_v1..v3` 6, 11, 17; `skim_v1..v3` 5, 9, 14) to 1 : 2 : 3.5
/// (`rollback`); the spec takes the first.
pub const CRAFT_VERSION_MAGNITUDE: [f32; 3] = [1.0, 1.8, 2.8];

/// Power a version above v1.0 adds: `rollback_v1..v3` cost 6, 8, 10.
pub const CRAFT_COST_PER_VERSION: f32 = 2.0;

/// Cooldown a version above v1.0 adds: `rollback_v1..v3` wait 2, 3, 4, as
/// every shipped family does.
pub const CRAFT_COOLDOWN_PER_VERSION: u32 = 1;

/// The fraction of damage a Drain returns at v1.0, v2.0, v3.0
/// (`skim_v1..v3`).
pub const CRAFT_DRAIN_HEAL_FRACTION: [f32; 3] = [0.4, 0.45, 0.5];

/// A Fork's `(count, extra, rarity_penalty)` at v1.0, v2.0, v3.0: v1 is
/// `fork_program`, v3 is `fork_cluster`, v2 the step between.
pub const CRAFT_FORK_BODIES: [(u32, u32, u32); 3] = [(1, 0, 0), (1, 1, 0), (2, 1, 1)];

// ---------------------------------------------------------------------------
// Cores at v1.0
// ---------------------------------------------------------------------------

/// `segfault_v1`.
pub const CRAFT_DAMAGE: CoreFigures = figures(2, 6.0, 6, 6, 2, 0);
/// `rollback_v1`.
pub const CRAFT_HEAL: CoreFigures = figures(2, 6.0, 0, 10, 2, 0);
/// `skim_v1`.
pub const CRAFT_DRAIN: CoreFigures = figures(2, 6.0, 6, 5, 1, 0);
/// `priority_boost`.
pub const CRAFT_BOOST_ATTACK: CoreFigures = figures(1, 5.0, 0, 3, 0, 3);
/// `sandbox`.
pub const CRAFT_BOOST_MITIGATION: CoreFigures = figures(1, 5.0, 0, 9, 0, 3);
/// `clock_gate`, the single-target Atk sap.
pub const CRAFT_SAP_ATTACK: CoreFigures = figures(2, 8.0, 0, 5, 0, 3);
/// `acid_wash`, the single-target Mitigation sap (-15% for 3 rounds). `etch`
/// (-12) and `oxide_strip` (-9) are exactly this at the Group and Everyone
/// magnitude factors.
pub const CRAFT_SAP_MITIGATION: CoreFigures = figures(2, 8.0, 0, 15, 0, 3);
/// The Debuff Core's own price; the status it carries sets its power and
/// rounds (`CRAFT_DEBUFF_STATUS`). `deadlock` and `write_lock`.
pub const CRAFT_DEBUFF: CoreFigures = figures(2, 6.0, 0, 0, 0, 0);
/// `invalidate_line`.
pub const CRAFT_CLEANSE: CoreFigures = figures(2, 4.0, 0, 0, 0, 0);
/// `detach`.
pub const CRAFT_CLOAK: CoreFigures = figures(4, 11.0, 0, 0, 0, 3);
/// `fork_program` at v1.0; v3.0 lands on `fork_cluster`'s 38.
pub const CRAFT_FORK: CoreFigures = CoreFigures {
    cost_per_version: 9.0,
    cooldown_per_version: 0,
    ..figures(4, 20.0, 0, 0, 0, 0)
};
/// `respawn` is the v3.0 (3 bodies, Power 30, wait 5); v1.0 steps down
/// from it by `cost_per_version` and a wait.
pub const CRAFT_RESPAWN: CoreFigures = CoreFigures {
    cost_per_version: 6.0,
    ..figures(3, 18.0, 0, 0, 0, 0)
};
/// `cold_sample`; its temperature is `CRAFT_COLD_TEMPERATURE`.
pub const CRAFT_COLD_SAMPLE: CoreFigures = figures(3, 6.0, 0, 0, 0, 3);
/// `heat_injection` less the group premium `heat_injection`'s 12 includes
/// (`CRAFT_AREA_GROUP_POWER`'s neighbourhood), since this Core aims at one.
pub const CRAFT_HEAT_INJECTION: CoreFigures = figures(4, 8.0, 0, 0, 0, 2);
/// `inference_probe`.
pub const CRAFT_PROFILE: CoreFigures = figures(3, 5.0, 0, 0, 0, 3);
/// `prompt_injection`.
pub const CRAFT_INJECT: CoreFigures = figures(5, 12.0, 0, 0, 0, 1);
/// `hallucination` less its group premium; Area is required to take hold.
pub const CRAFT_HALLUCINATE: CoreFigures = figures(5, 9.0, 0, 0, 0, 3);
/// `teleport`.
pub const CRAFT_TELEPORT: CoreFigures = figures(3, 8.0, 0, 0, 0, 0);
/// `call_reinforcements`.
pub const CRAFT_REINFORCE: CoreFigures = figures(5, 30.0, 0, 0, 0, 0);
/// `emulate`.
pub const CRAFT_EMULATE: CoreFigures = figures(4, 20.0, 0, 0, 0, 10);

/// The temperature of Cold Sample and of Heat Injection (`cold_sample`,
/// `heat_injection`).
pub const CRAFT_COLD_TEMPERATURE: f32 = 0.0;
/// See `CRAFT_COLD_TEMPERATURE`.
pub const CRAFT_HEAT_TEMPERATURE: f32 = 2.0;
/// Decoys a Hallucinate seeds (`hallucination`).
pub const CRAFT_HALLUCINATE_DECOYS: u32 = 3;
/// A Respawn's area and reach, which no modifier sets (`respawn`).
pub const CRAFT_RESPAWN_RADIUS: u32 = 2;
/// See `CRAFT_RESPAWN_RADIUS`.
pub const CRAFT_RESPAWN_REACH: u32 = 5;

/// Field Cores. A field routine has no cooldown (the loader warns on one).
/// `repair_loop`: 2 Integrity every 4th turn.
pub const CRAFT_FIELD_REGEN: CoreFigures = figures(0, 18.0, 0, 2, 0, 0);
/// How often Field Regen ticks, in turns (`repair_loop`).
pub const CRAFT_FIELD_REGEN_INTERVAL: u32 = 4;
/// `trickle_charge`: +1 Power a turn.
pub const CRAFT_RECHARGE: CoreFigures = figures(0, 25.0, 0, 1, 0, 0);
/// `overclock`.
pub const CRAFT_FIELD_ATTACK: CoreFigures = figures(0, 14.0, 0, 4, 0, 0);
/// `hardened_shell`.
pub const CRAFT_FIELD_MITIGATION: CoreFigures = figures(0, 14.0, 0, 12, 0, 0);
/// `deep_scan`.
pub const CRAFT_CAPTURE_BOOST: CoreFigures = figures(0, 18.0, 0, 20, 0, 0);
/// `trace_analysis`.
pub const CRAFT_XP_BOOST: CoreFigures = figures(0, 18.0, 0, 20, 0, 0);
/// `stealth_protocol`.
pub const CRAFT_ENCOUNTER_DAMP: CoreFigures = figures(0, 18.0, 0, 20, 0, 0);
/// `salvage_routine`.
pub const CRAFT_DROP_BOOST: CoreFigures = figures(0, 18.0, 0, 20, 0, 0);
/// `buffer_overrun`.
pub const CRAFT_PHASE: CoreFigures = figures(0, 12.0, 0, 0, 0, 0);
/// `wild_jump`.
pub const CRAFT_JUMP: CoreFigures = figures(0, 20.0, 0, 0, 0, 0);
/// `symlink`.
pub const CRAFT_SYMLINK: CoreFigures = figures(0, 25.0, 0, 0, 0, 0);

/// Ticks a timed field Core (Field Regen, Recharge) runs at no Trickle:
/// `repair_loop` and `trickle_charge` both author 600.
pub const CRAFT_FIELD_DURATION: u32 = 600;

// ---------------------------------------------------------------------------
// Debuff statuses and Damage riders
// ---------------------------------------------------------------------------

/// A Debuff's `(power, rounds)` per status, in `StatusKind` order — bleed,
/// stun, exposed, poison, throttled, locked. `memory_leak`, `deadlock`,
/// `data_poisoning`, `thermal_throttle`, `write_lock`; exposed has no
/// shipped Debuff, so it takes the common three rounds.
pub const CRAFT_DEBUFF_STATUS: [(i32, u32); 6] = [(2, 3), (0, 1), (0, 3), (5, 3), (0, 3), (0, 3)];

/// A Damage rider's `(chance, rounds, power)` per status, in `StatusKind`
/// order. Bleed sits between `fork_bomb` (0.35) and `stack_smash` (0.6);
/// stun at `pipeline_stall`/`branch_hazard`'s 0.3-0.4; poison carries
/// `data_poisoning`'s damage a point lighter, as bleed's rider does against
/// its Debuff; the rest take the common 0.4 for three rounds.
pub const CRAFT_RIDER: [(f32, u32, i32); 6] = [
    (0.5, 3, 3),
    (0.35, 1, 0),
    (0.4, 3, 0),
    (0.4, 3, 4),
    (0.4, 3, 0),
    (0.4, 3, 0),
];

/// Power a Status rider on a Damage Core adds (`stack_smash` 8 and
/// `pipeline_stall` 9 against `segfault_v1`'s 6).
pub const CRAFT_RIDER_POWER: f32 = 2.0;

/// Power a Damage type adds (`packet_shred` 11 against its untyped group
/// sibling).
pub const CRAFT_DAMAGE_TYPE_POWER: f32 = 1.0;

// ---------------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------------

/// Per-target magnitude of a Group and an Everyone area against one target:
/// `skim` 5 / 4 / 3, `row_hammer` 7 / 6 / 5, `rollback` party 10 → 8.
pub const CRAFT_AREA_GROUP_MAGNITUDE: f32 = 0.8;
/// See `CRAFT_AREA_GROUP_MAGNITUDE`; `cycle_harvest` is the harsh end at 0.4.
pub const CRAFT_AREA_EVERYONE_MAGNITUDE: f32 = 0.6;
/// Power a Group area adds: `segfault` 6 → 11, `race_condition` 6 → 13,
/// `skim` 6 → 8, `mirror_restore` 6 → 10.
pub const CRAFT_AREA_GROUP_POWER: f32 = 5.0;
/// Power an Everyone area adds: `segfault` 6 → 18, `skim` 6 → 15,
/// `null_route` 6 → 15.
pub const CRAFT_AREA_EVERYONE_POWER: f32 = 10.0;
/// Accuracy a Group and an Everyone area take off an attack's, as the
/// `skim` family shows (6, 4, 2).
pub const CRAFT_AREA_GROUP_ACCURACY: i32 = -2;
/// See `CRAFT_AREA_GROUP_ACCURACY`.
pub const CRAFT_AREA_EVERYONE_ACCURACY: i32 = -4;
/// Cooldown any Area part adds (spec).
pub const CRAFT_AREA_COOLDOWN: u32 = 1;

/// Reach of a Near and a Far Range, in cells (spec).
pub const CRAFT_RANGE_NEAR: u32 = 3;
/// See `CRAFT_RANGE_NEAR`.
pub const CRAFT_RANGE_FAR: u32 = 6;
/// Power a Near and a Far Range add. No shipped pair differs by reach alone
/// (`gradient_descent` against `segfault_v1` is the nearest, and a point),
/// so reach is priced small.
pub const CRAFT_RANGE_NEAR_POWER: f32 = 1.0;
/// See `CRAFT_RANGE_NEAR_POWER`.
pub const CRAFT_RANGE_FAR_POWER: f32 = 2.0;

/// Rounds one Trickle adds to a Buff, Debuff, Cloak or Tamper. One round,
/// since `priority_boost` to `bastion_shield_v3` gain one over two versions.
pub const CRAFT_TRICKLE_ROUNDS: u32 = 1;
/// Duration one Trickle adds to a timed field Core, in the units
/// `FieldBuff::duration` carries: half of `trickle_charge`'s 600.
pub const CRAFT_TRICKLE_FIELD_DURATION: u32 = 300;
/// Power each Trickle adds; the same step as a version.
pub const CRAFT_TRICKLE_POWER: f32 = 2.0;
/// Cooldown each Trickle adds (spec).
pub const CRAFT_TRICKLE_COOLDOWN: u32 = 1;
/// Rounds a Trickle on a Heal restores over (user decision 1).
pub const CRAFT_HOT_ROUNDS_PER_TRICKLE: u32 = 2;
/// The share of the Heal Core's band each of those rounds restores. Above
/// half so one Trickle returns 1.2 times the instant heal: slower, and paid
/// for in Power and a longer wait.
pub const CRAFT_HOT_SHARE_PER_ROUND: f32 = 0.6;

// ---------------------------------------------------------------------------
// The part cap and the part items
// ---------------------------------------------------------------------------

/// Parts one routine may hold before any capacity research (spec).
pub const CRAFT_PART_CAP_START: usize = 2;
/// The most parts a routine may ever hold (spec).
pub const CRAFT_PART_CAP_MAX: usize = 6;

/// The three materials a part is crafted from, beside one Routine Disk
/// (spec "Where Subroutines come from").
pub const CRAFT_MATERIAL_BYTECODE: &str = "bytecode_block";
/// See `CRAFT_MATERIAL_BYTECODE`.
pub const CRAFT_MATERIAL_FRAGMENT: &str = "core_fragment";
/// See `CRAFT_MATERIAL_BYTECODE`.
pub const CRAFT_MATERIAL_CELL: &str = "power_cell";

/// What a Subroutine is worth in trade currency: an etched disk's
/// `ETCHED_DISK_VALUE`, which parts replace in the economy. Above
/// `CONTRACT_MAX_DELIVER_VALUE`, so no Requisition asks for twenty of them.
pub const CRAFT_PART_VALUE: u32 = 20;

/// The shortest and longest a built routine's name may be, in characters.
pub const CRAFT_NAME_MIN: usize = 1;
/// See `CRAFT_NAME_MIN`.
pub const CRAFT_NAME_MAX: usize = 24;

/// What a built routine's id starts with; the rest is the counter.
pub const CRAFT_ID_PREFIX: &str = "crafted/";

/// The zone the first capacity node (2 to 3 parts) opens in; each later one
/// opens a zone after (spec "Research").
pub const CRAFT_CAPACITY_FIRST_ZONE: u32 = 1;

/// The name of the routine every new run starts with beside Decompile.
pub const CRAFT_STARTER_NAME: &str = "Patch";
