//! The siege clock: accrual, its three holds, and (Task 2) the approach
//! warning and its wall-clock floor.
//!
//! Built to `Game::raid_check`'s pattern (`game/base/upkeep.rs`) and
//! departs from it in exactly the places noted below — see
//! `resources::SiegePressure` for why it is a second meter rather than a
//! second use of `resources::RaidPressure`.

use crate::tuning::{
    SIEGE_MIN_ZONE, SIEGE_PRESSURE_JITTER_PERCENT, SIEGE_PRESSURE_PER_ZONE,
    SIEGE_PRESSURE_THRESHOLD, SIEGE_PRESSURE_WARN_PERCENT,
};
use crate::*;

impl Game {
    /// How close the base is to its next siege, in
    /// `resources::SiegePressure`'s own units — `raid_pressure`'s
    /// counterpart.
    pub fn siege_pressure(&self) -> u32 {
        self.world
            .resource::<crate::resources::SiegePressure>()
            .level
    }

    pub(crate) fn siege_check(&mut self) {
        let zone = self.world.resource::<ZoneLevel>().0;
        // **The sector gate is on accrual and not on firing** —
        // `raid_check`'s reason, applied here: pressure the opening sector
        // built would be pressure it could never spend, so a player
        // crossing into a sector that can fire would be sieged within a
        // tick or two of arriving.
        if zone < SIEGE_MIN_ZONE {
            return;
        }

        let target = match self
            .world
            .resource::<crate::resources::SiegePressure>()
            .next_at
        {
            Some(target) => target,
            None => {
                let target = self.draw_siege_interval();
                self.world
                    .resource_mut::<crate::resources::SiegePressure>()
                    .next_at = Some(target);
                target
            }
        };

        let level = {
            let mut pressure = self.world.resource_mut::<crate::resources::SiegePressure>();
            pressure.level += SIEGE_PRESSURE_PER_ZONE * zone;
            pressure.level
        };

        // Task 2.
        self.warn_of_approaching_siege(zone, target, level);

        if level < target {
            return;
        }

        // **The departure from `raid_check`: three holds, not one.** Each is
        // a `return` before the reset, so the pressure a held tick built is
        // still owed and the siege waits rather than being forgiven.
        //
        // No base — an opening run has not earned the event yet.
        if !self.base_is_established() {
            return;
        }
        // Nothing standing to besiege — `run_raid`'s own emptiness check,
        // asked up front rather than inside the fire, because Task 1's fire
        // is a stub that cannot yet answer it itself.
        if self.nothing_to_besiege() {
            return;
        }
        // Another fight already running — a siege must not open, or resolve,
        // on top of one. `Game::has_active_battle` covers both combat
        // models in one call.
        if self.has_active_battle() {
            return;
        }

        // **Fire, then reset — and only if the fire reported a siege
        // actually happened.** `raid_check`'s `if !self.run_raid() { return; }`
        // pattern: `stage_siege` (Task 4's `resolve_siege_offscreen`, Task
        // 8's home/away branch) answers `bool` for the same reason
        // `run_raid` does, and resetting on `false` would rewind the clock
        // on a no-op.
        if !self.stage_siege() {
            return;
        }
        let mut pressure = self.world.resource_mut::<crate::resources::SiegePressure>();
        pressure.level = 0;
        pressure.warned = false;
        pressure.next_at = None;
    }

    /// Everything a siege *is*, once the clock has decided one happens.
    ///
    /// A stub for exactly two tasks: Task 4 replaces this body with the
    /// off-screen resolution (`Game::resolve_siege_offscreen`) and Task 8
    /// puts the home/away branch in its place. It is never left as a stub
    /// past that.
    fn stage_siege(&mut self) -> bool {
        self.log_base_kind(MessageKind::Raid, "A siege is forming.".to_string());
        true
    }

    /// Whether there is nothing standing for a siege to take or break —
    /// `run_raid`'s own target pool (`With<Durability>, With<Structure>`),
    /// asked as a hold rather than discovered inside the fire.
    fn nothing_to_besiege(&mut self) -> bool {
        let mut query = self
            .world
            .query_filtered::<Entity, (With<Durability>, With<Structure>)>();
        query.iter(&self.world).next().is_none()
    }

    /// One interval, jittered, in `resources::SiegePressure`'s units —
    /// `draw_raid_interval`'s shape, and the run's only `GameRng` draw on
    /// this meter: one per *siege*, never one per tick.
    fn draw_siege_interval(&mut self) -> u32 {
        let low = SIEGE_PRESSURE_THRESHOLD * (100 - SIEGE_PRESSURE_JITTER_PERCENT) / 100;
        let high = SIEGE_PRESSURE_THRESHOLD * (100 + SIEGE_PRESSURE_JITTER_PERCENT) / 100;
        let mut rng = self.world.resource_mut::<GameRng>();
        rng.0.random_range(low..=high)
    }

    /// Fires a siege now, without waiting on the clock — the dev console's
    /// trigger. `dev_force_raid`'s shape exactly: calls the fire
    /// (`stage_siege`) rather than a copy of it and leaves the clock alone,
    /// so what the console puts on screen is evidence about the siege a
    /// player actually meets.
    #[doc(hidden)]
    pub fn dev_force_siege(&mut self) {
        self.stage_siege();
    }

    /// Winds the clock to its own approach warning without firing a siege —
    /// `dev_wind_raid_clock`'s shape exactly, including winding to the
    /// drawn interval's own warn point rather than to a fixed number, so one
    /// press works whatever the jitter rolled.
    #[doc(hidden)]
    pub fn dev_wind_siege_clock(&mut self) {
        let target = match self
            .world
            .resource::<crate::resources::SiegePressure>()
            .next_at
        {
            Some(target) => target,
            None => {
                let target = self.draw_siege_interval();
                self.world
                    .resource_mut::<crate::resources::SiegePressure>()
                    .next_at = Some(target);
                target
            }
        };
        let mut pressure = self.world.resource_mut::<crate::resources::SiegePressure>();
        pressure.level = target * SIEGE_PRESSURE_WARN_PERCENT / 100;
        pressure.warned = false;
    }

    /// Task 2 fills this in: the approach warning and its wall-clock floor.
    /// A no-op today so `siege_check`'s call site does not move.
    fn warn_of_approaching_siege(&mut self, _zone: u32, _target: u32, _level: u32) {}
}
