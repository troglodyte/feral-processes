//! The engine half of battle telemetry: a gate, a buffer, and a drain.
//!
//! `crate::telemetry` holds the record shapes; the five combat seams build
//! them through `Game::record`; app-core drains with
//! `Game::take_battle_telemetry` and is the only crate that turns one into
//! text. Nothing here touches the disk.

use crate::resources::BattleTelemetry;
use crate::telemetry::Record;
use crate::*;

impl Game {
    /// Starts collecting. Called once at startup by app-core when
    /// `FERAL_DEV_LOG` is set, never from inside the sim.
    pub fn enable_battle_telemetry(&mut self) {
        self.world.resource_mut::<BattleTelemetry>().on = true;
    }

    /// Hands over everything recorded since the last call, emptying the
    /// buffer. A drain rather than a read, matching
    /// `take_pending_profile_writes`: app-core appends what it is given, so
    /// a second read must not append the same records twice.
    pub fn take_battle_telemetry(&mut self) -> Vec<Record> {
        std::mem::take(&mut self.world.resource_mut::<BattleTelemetry>().records)
    }

    /// Records one event, building it only if telemetry is on.
    ///
    /// **The closure is the point.** An eager `record(Record::EnemyChoice
    /// { .. })` would build the struct — three `String` allocations — on
    /// every swing of every fight even when disabled, and `train` pays that
    /// 1.9M times a session. There is deliberately no eager variant, so a
    /// caller cannot get this wrong.
    ///
    /// **The `&Game` parameter is what makes the lazy form compile.** A
    /// `FnOnce() -> Record` would have to capture `&self` to read a target's
    /// `Stats`, while this holds `&mut self` — that does not borrow check.
    /// Reading `on` before taking the mutable borrow, and passing `self` in,
    /// is the shape that does.
    pub(crate) fn record(&mut self, f: impl FnOnce(&Game) -> Record) {
        if !self.world.resource::<BattleTelemetry>().on {
            return;
        }
        let record = f(self);
        self.world
            .resource_mut::<BattleTelemetry>()
            .records
            .push(record);
    }

    /// Mints the next fight id and makes it the one later records carry.
    /// Called by `begin_battle`; every record in the fight reads it back
    /// through `fight_id`.
    pub(crate) fn next_fight_id(&mut self) -> u64 {
        let mut telemetry = self.world.resource_mut::<BattleTelemetry>();
        telemetry.next_fight += 1;
        telemetry.fight = telemetry.next_fight;
        telemetry.fight
    }
}
