//! A siege: the sector's wild programs walk in through the base's one
//! door, steal what they can carry, wreck what they cannot, and withdraw
//! on a morale break.
//!
//! Separate from the GC Entropy Sweep (`game/base/upkeep.rs`). The two
//! events share nothing but the shape of their clock — `clock.rs` is
//! `resources::RaidPressure`'s pattern applied to its own
//! `resources::SiegePressure` rather than that resource — and the
//! `StructureDef::raid_defense` a turret keeps paying into.
//!
//! `docs/superpowers/specs/2026-09-22-siege-design.md` is the design
//! record; `docs/superpowers/plans/2026-09-22-siege.md` is the task
//! breakdown this module is built against.

pub(crate) mod clock;
