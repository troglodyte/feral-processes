//! Trace: how loud the party has been, and what the stack does about it.
//!
//! One meter, three sources, four bands, three multipliers. This module owns
//! all of it — reading the current value, classifying it, and handing out the
//! per-band scaling that `stack.rs`, `stack_features.rs` and `spawning.rs`
//! apply at their own call sites.
//!
//! The design argument is in
//! `docs/superpowers/specs/2026-07-31-the-stack-design.md`, "Phase 2". Two
//! points from it are load-bearing enough to repeat here, because both look
//! like arbitrary choices from inside the code:
//!
//! - **A kill is worth a fifth of a cache.** Kills outnumber caches by
//!   roughly eight to one over a thorough crawl, so paying them at cache
//!   rates would make this a combat meter rather than a greed meter — and a
//!   self-feeding one, since more Trace draws more encounters.
//! - **Trace lives in a resource, not in `Locale::Stack`.** See
//!   `resources::Trace`; the short version is that descending rebuilds that
//!   variant.

use crate::resources::{Trace, TraceBand};
use crate::tuning::{TRACE_HUNTED, TRACE_NOTICED, TRACE_TRACED};
use crate::*;

impl TraceBand {
    /// Which band a raw Trace reading falls in. Thresholds are half-open, so
    /// a value sitting exactly on one belongs to the band it names.
    ///
    /// On the enum rather than on `Game` because it needs neither a world nor
    /// a party — it is a reading of a number against three constants, and
    /// putting it here lets the thresholds be tested without standing a
    /// `Game` up around them.
    pub fn from_trace(trace: u32) -> Self {
        match trace {
            t if t >= TRACE_HUNTED => TraceBand::Hunted,
            t if t >= TRACE_TRACED => TraceBand::Traced,
            t if t >= TRACE_NOTICED => TraceBand::Noticed,
            _ => TraceBand::Quiet,
        }
    }
}

impl Game {
    pub(crate) fn trace(&self) -> u32 {
        self.world.resource::<Trace>().0
    }
}
