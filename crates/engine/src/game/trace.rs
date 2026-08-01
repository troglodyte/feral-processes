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

    /// The one way Trace goes up, and the only place that knows a band was
    /// crossed.
    ///
    /// Two things live here rather than at the three call sites. The
    /// **underground guard**, because `award_loot` fires for every kill in
    /// the game and the overwhelming majority of those are on the surface —
    /// one check beats three, and a fourth source added later inherits it.
    /// And the **crossing announcement**, logged as `MessageKind::Outcome`
    /// so it survives `MessageLog::retain_outcomes_since_battle`: a
    /// kill-driven crossing is logged during a battle teardown, where a
    /// plain `Info` line would be pruned before the player ever saw it.
    ///
    /// Crossings are monotonic — nothing lowers Trace, and it resets only on
    /// leaving the Stack — so only a rise is ever announced.
    pub(crate) fn raise_trace(&mut self, amount: u32) {
        if !self.is_underground() {
            return;
        }
        let before = TraceBand::from_trace(self.trace());
        let raised = self.trace().saturating_add(amount);
        self.world.insert_resource(Trace(raised));

        let after = TraceBand::from_trace(raised);
        if after == before {
            return;
        }
        let line = match after {
            // Unreachable while Trace only ever rises, and stated as a
            // no-op rather than a panic so that stays a design property
            // rather than a crash if it ever changes.
            TraceBand::Quiet => return,
            TraceBand::Noticed => "Something in the substrate turns to look at you.",
            TraceBand::Traced => "You are being traced. The dark is routing around you.",
            TraceBand::Hunted => "Hunted. Whatever is down here has your address.",
        };
        self.log_kind(MessageKind::Outcome, line);
    }
}
