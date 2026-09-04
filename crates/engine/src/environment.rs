//! Ambient environment effects: what the ground does to you for standing on
//! it.
//!
//! One variant per condition, and the catalogue beside it — the shape
//! `notifications.rs` took, and for the same reason: a later weather layer's
//! ambush multiplier is a hook into `maybe_ambush`'s roll, not a shape a
//! `.ron` file can express, and the loader's whole cost — an id newtype, an
//! absent-directory rule, a malformed-file rule, three load-time refusals,
//! and a pairing census to catch a def nothing resolves — was being paid to
//! make three strings editable. So this module is the whole feature: the
//! enum is the census, `GroundCondition::def` is the copy, and adding a
//! condition is a variant plus an arm.
//!
//! **`def` is an exhaustive match and must stay so** — `cell_mark`'s rule.
//! Written as a table lookup with a fallback, a new variant would ship
//! blank; written as a match, it fails to compile until somebody writes the
//! words.
//!
//! This used to be `assets/environment/*.ron` behind an `EnvironmentDb`, on
//! the load-and-warn pattern every other asset db follows. It came home
//! because nothing here is authored by a stranger: the ceilings that used to
//! refuse a bad file at load now guard the *fold* instead — a compile-time
//! census in `tests::environment` rather than a startup check, and strictly
//! stronger for it.
//!
//! Data only: nothing here knows about a `Game`. The one reader that
//! resolves a tile to an effect, and the zone-1 gate it holds, live in
//! `game/environment.rs`.

use crate::tuning::{
    DANGLING_READS_ATTRITION, DANGLING_READS_FLOOR, LEAKING_MEMORY_ATTRITION, LEAKING_MEMORY_FLOOR,
    LEAKING_MEMORY_WEIGHT, LOCK_CONTENTION_DRAG_TICKS, MAX_ENVIRONMENT_ATTRITION,
    MAX_ENVIRONMENT_DRAG_TICKS, MAX_ENVIRONMENT_MIN_DAMAGE, MAX_STATIC_AMBUSH_MULT,
    PACKET_FLOOD_AMBUSH_MULT, PACKET_FLOOD_DRAG_TICKS, PACKET_FLOOD_WEIGHT,
    SIGNAL_NOISE_AMBUSH_MULT, SIGNAL_NOISE_WEIGHT, THERMAL_LOAD_ATTRITION, THERMAL_LOAD_FLOOR,
    THREAD_STORM_AMBUSH_MULT, THREAD_STORM_DRAG_TICKS, THREAD_STORM_WEIGHT,
};
use crate::world::Biome;

/// What standing on this ground costs — every term at once, not a one-of.
/// Folding two sources (ground and, later, weather) into one answer is
/// arithmetic: attrition and drag add, the ambush multiplier multiplies, and
/// `bite` prices the summed attrition through a single floor rather than
/// once per source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvironmentEffect {
    /// A bite out of the player's Integrity, as a fraction of their maximum.
    /// A fraction rather than a flat figure: terrain is uncorrelated with
    /// player level, so any constant is lethal at level 1 and free by
    /// mid-run.
    pub attrition_percent: f32,
    /// The floor under `attrition_percent`'s bite, so the effect does not
    /// round away to nothing at low levels.
    pub min_damage: i32,
    /// Extra ticks the step costs, on top of the one every step costs.
    pub extra_ticks: u32,
    /// Multiplies `RANDOM_ENCOUNTER_CHANCE` in `maybe_ambush`. Every ground
    /// condition ships `1.0`; a live `StaticEvent` is what gives it a value
    /// other than that.
    pub ambush_mult: f32,
}

impl EnvironmentEffect {
    /// The identity: no bite, no drag, no bias. What ground with no
    /// condition, or a zone-1 step, resolves to.
    pub const NONE: EnvironmentEffect = EnvironmentEffect {
        attrition_percent: 0.0,
        min_damage: 0,
        extra_ticks: 0,
        ambush_mult: 1.0,
    };

    /// Stacks `self` under `other`: attrition, the floor and drag add; the
    /// ambush multiplier multiplies. The shape a later weather layer folds
    /// into the ground with, so "the ground you know is still doing what it
    /// does, and something is on top of it" is arithmetic rather than a
    /// case-split.
    pub(crate) fn fold(self, other: EnvironmentEffect) -> EnvironmentEffect {
        EnvironmentEffect {
            attrition_percent: self.attrition_percent + other.attrition_percent,
            min_damage: self.min_damage + other.min_damage,
            extra_ticks: self.extra_ticks + other.extra_ticks,
            ambush_mult: self.ambush_mult * other.ambush_mult,
        }
    }

    /// Cuts a folded effect down to what a single source may authorise on
    /// its own. A fold can exceed either half's own ceiling — that is why
    /// this is a check on the sum, not the load-time refusal it replaces.
    pub(crate) fn clamped(self) -> EnvironmentEffect {
        EnvironmentEffect {
            attrition_percent: self.attrition_percent.min(MAX_ENVIRONMENT_ATTRITION),
            min_damage: self.min_damage.min(MAX_ENVIRONMENT_MIN_DAMAGE),
            extra_ticks: self.extra_ticks.min(MAX_ENVIRONMENT_DRAG_TICKS),
            ambush_mult: self.ambush_mult.min(MAX_STATIC_AMBUSH_MULT),
        }
    }

    /// The Integrity this effect takes on a step: the summed percent and
    /// summed floor go through one `max`, so two attrition sources stacking
    /// are priced against a single floor rather than one each.
    pub fn bite(self, max_hp: i32) -> i32 {
        ((max_hp as f32 * self.attrition_percent).round() as i32).max(self.min_damage)
    }
}

/// One ground condition's authored copy.
#[derive(Clone, Copy, Debug)]
pub struct ConditionDef {
    /// What the player calls this ground's condition, distinct from the
    /// biome's own name.
    pub name: &'static str,
    /// The sentence under that name.
    pub description: &'static str,
    pub effect: EnvironmentEffect,
}

/// A standing condition claiming a biome — the three ambient effects
/// transcribed from the deleted `assets/environment/*.ron` files. Two
/// conditions claiming one biome is unrepresentable rather than a load-time
/// clash: `for_biome` is a match on the biome, so at most one arm can ever
/// answer for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundCondition {
    DanglingReads,
    ThermalLoad,
    LockContention,
}

impl GroundCondition {
    /// Every condition, for the censuses. A walk over this array is what
    /// makes a census non-vacuous — `NotificationKind::all`'s shape: the
    /// array length fails to compile when a variant is added without being
    /// listed.
    pub fn all() -> [GroundCondition; 3] {
        [
            GroundCondition::DanglingReads,
            GroundCondition::ThermalLoad,
            GroundCondition::LockContention,
        ]
    }

    /// The condition claiming `biome`, or `None` for neutral ground.
    /// Unclaimed is the common case — that is what makes most of the map
    /// read as scenery rather than as a tax on walking.
    pub fn for_biome(biome: Biome) -> Option<GroundCondition> {
        match biome {
            Biome::NullSector => Some(GroundCondition::DanglingReads),
            Biome::Backplane => Some(GroundCondition::ThermalLoad),
            Biome::Deadlock => Some(GroundCondition::LockContention),
            _ => None,
        }
    }

    /// What this condition says and costs. **Exhaustive by construction** —
    /// see the module doc.
    pub fn def(self) -> ConditionDef {
        match self {
            GroundCondition::DanglingReads => ConditionDef {
                name: "Dangling Reads",
                description: "Nothing here is addressed. What you touch answers with garbage.",
                effect: EnvironmentEffect {
                    attrition_percent: DANGLING_READS_ATTRITION,
                    min_damage: DANGLING_READS_FLOOR,
                    extra_ticks: 0,
                    ambush_mult: 1.0,
                },
            },
            GroundCondition::ThermalLoad => ConditionDef {
                name: "Thermal Load",
                description: "Waste heat pours off machinery that has not stopped in years.",
                effect: EnvironmentEffect {
                    attrition_percent: THERMAL_LOAD_ATTRITION,
                    min_damage: THERMAL_LOAD_FLOOR,
                    extra_ticks: 0,
                    ambush_mult: 1.0,
                },
            },
            GroundCondition::LockContention => ConditionDef {
                name: "Lock Contention",
                description: "Every step waits its turn behind something that never let go.",
                effect: EnvironmentEffect {
                    attrition_percent: 0.0,
                    min_damage: 0,
                    extra_ticks: LOCK_CONTENTION_DRAG_TICKS,
                    ambush_mult: 1.0,
                },
            },
        }
    }
}

/// One weather event's authored copy — `StaticEvent`'s mirror of
/// `ConditionDef`. `biomes` is a slice rather than a single `Biome` because
/// `SignalNoise` claims two; every other event claims one.
#[derive(Clone, Copy, Debug)]
pub struct StaticDef {
    /// What the player calls this event, distinct from both the biome's
    /// name and any standing `GroundCondition` claiming the same ground.
    pub name: &'static str,
    /// The sentence under that name.
    pub description: &'static str,
    /// The biomes this event's pool is drawn into. Non-empty for every
    /// shipped event — an event nothing can claim would never be seen.
    pub biomes: &'static [Biome],
    /// This event's weight in its biome's pool, against
    /// `crate::tuning::STATIC_CLEAR_WEIGHT` and any other event sharing the
    /// pool. Per-event rather than a single shared constant, so one event
    /// can be made rarer without touching the others.
    pub weight: u32,
    /// What standing under this weather does, on top of whatever the ground
    /// itself already does — folded, never substituted, by
    /// `Game::terrain_at`.
    pub effect: EnvironmentEffect,
}

/// A weather event live somewhere on the map right now — "Static" is the
/// player's word for the whole layer. Which event, if any, is live in a
/// biome is derived from the clock in `game/environment.rs`; nothing here
/// is stored, and there is no save field to migrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticEvent {
    LeakingMemory,
    ThreadStorm,
    PacketFlood,
    SignalNoise,
}

impl StaticEvent {
    /// Every event, for the censuses — `GroundCondition::all`'s shape: the
    /// array length fails to compile when a fifth event ships without being
    /// listed.
    pub fn all() -> [StaticEvent; 4] {
        [
            StaticEvent::LeakingMemory,
            StaticEvent::ThreadStorm,
            StaticEvent::PacketFlood,
            StaticEvent::SignalNoise,
        ]
    }

    /// What this event says, costs and claims. **Exhaustive by
    /// construction** — `GroundCondition::def`'s rule: a table lookup with a
    /// fallback would ship a new variant blank, so this stays a match until
    /// somebody writes the words.
    pub fn def(self) -> StaticDef {
        match self {
            StaticEvent::LeakingMemory => StaticDef {
                name: "Leaking Memory",
                description: "Something here allocated and never freed. The leak is still running.",
                biomes: &[Biome::NullSector],
                weight: LEAKING_MEMORY_WEIGHT,
                effect: EnvironmentEffect {
                    attrition_percent: LEAKING_MEMORY_ATTRITION,
                    min_damage: LEAKING_MEMORY_FLOOR,
                    extra_ticks: 0,
                    ambush_mult: 1.0,
                },
            },
            StaticEvent::ThreadStorm => StaticDef {
                name: "Thread Storm",
                description: "Threads spawn faster than anything here can schedule them.",
                biomes: &[Biome::Backplane],
                weight: THREAD_STORM_WEIGHT,
                effect: EnvironmentEffect {
                    attrition_percent: 0.0,
                    min_damage: 0,
                    extra_ticks: THREAD_STORM_DRAG_TICKS,
                    ambush_mult: THREAD_STORM_AMBUSH_MULT,
                },
            },
            StaticEvent::PacketFlood => StaticDef {
                name: "Packet Flood",
                description: "Traffic saturates every link at once, and whatever else is out here rides it in with you.",
                biomes: &[Biome::OpenGrid],
                weight: PACKET_FLOOD_WEIGHT,
                effect: EnvironmentEffect {
                    attrition_percent: 0.0,
                    min_damage: 0,
                    extra_ticks: PACKET_FLOOD_DRAG_TICKS,
                    ambush_mult: PACKET_FLOOD_AMBUSH_MULT,
                },
            },
            StaticEvent::SignalNoise => StaticDef {
                name: "Signal Noise",
                description: "The air is thick with garbled signal, and picking a target out of it works both ways.",
                biomes: &[Biome::Deadlock, Biome::NullSector],
                weight: SIGNAL_NOISE_WEIGHT,
                effect: EnvironmentEffect {
                    attrition_percent: 0.0,
                    min_damage: 0,
                    extra_ticks: 0,
                    ambush_mult: SIGNAL_NOISE_AMBUSH_MULT,
                },
            },
        }
    }

    /// Whether this event's pool is drawn into `biome`.
    pub fn claims(self, biome: Biome) -> bool {
        self.def().biomes.contains(&biome)
    }
}
