//! The siege clock: accrual, its three holds, the approach warning and its
//! wall-clock floor.
//!
//! Built to `Game::raid_check`'s pattern (`game/base/upkeep.rs`) and
//! departs from it in exactly the places noted below — see
//! `resources::SiegePressure` for why it is a second meter rather than a
//! second use of `resources::RaidPressure`.

use crate::tuning::{
    SIEGE_MIN_ZONE, SIEGE_PRESSURE_JITTER_PERCENT, SIEGE_PRESSURE_PER_ZONE,
    SIEGE_PRESSURE_THRESHOLD, SIEGE_PRESSURE_WARN_PERCENT, SIEGE_WARN_FLOOR_TICKS,
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
        // pattern. Home or away: `Game::open_siege` when the player is
        // standing in base space, `Game::resolve_siege_offscreen`
        // everywhere else — the one departure from `raid_check`'s single
        // fire, since a sweep has no on-screen half.
        // **`open_siege` failing at home falls back to an off-screen
        // resolution, rather than holding the pressure again.** The two
        // refusals `open_siege` can give beyond this point — the door
        // itself unwalkable, or a pack that rolled but seated nobody — are
        // both persistent conditions of the base and the sector, not a
        // one-tick fluke, so a bare hold would retry (and redraw
        // `spawn_siege_pack`'s `GameRng`) every tick from here on with no
        // way to ever spend the pressure it built. The abstract resolution
        // reads no location at all, so it is a legitimate answer to a
        // siege that could not be staged, not merely a consolation prize.
        let fired = match self.base_pos() {
            Some(_) => self.open_siege() || self.resolve_siege_offscreen(),
            None => self.resolve_siege_offscreen(),
        };
        if !fired {
            return;
        }
        let mut pressure = self.world.resource_mut::<crate::resources::SiegePressure>();
        pressure.level = 0;
        pressure.warned = false;
        pressure.next_at = None;
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
    /// trigger. `dev_force_raid`'s shape exactly: calls `siege_check`'s own
    /// home/away branch rather than a copy of it and leaves the clock
    /// alone, so what the console puts on screen — a fought siege at home,
    /// an off-screen one away — is evidence about the siege a player
    /// actually meets.
    #[doc(hidden)]
    pub fn dev_force_siege(&mut self) {
        match self.base_pos() {
            Some(_) => {
                self.open_siege();
            }
            None => {
                self.resolve_siege_offscreen();
            }
        }
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

    /// The approach warning and its wall-clock floor.
    ///
    /// `raid_check`'s latch (`upkeep.rs:534-550`) exactly, with one
    /// difference in *when* it fires: the sweep warns at a share of its
    /// drawn interval, the siege warns at **whichever is earlier**, that
    /// same share or a fixed number of ticks out. `by_floor` is stated in
    /// *pressure*, not ticks — the floor is a wall-clock promise and the
    /// meter is not a clock, so it converts through the sector's own
    /// accrual rate before it can be compared against `by_share`.
    fn warn_of_approaching_siege(&mut self, zone: u32, target: u32, level: u32) {
        let accrual = SIEGE_PRESSURE_PER_ZONE * zone; // never zero: the gate above
        let by_share = target * SIEGE_PRESSURE_WARN_PERCENT / 100;
        let by_floor = target.saturating_sub(SIEGE_WARN_FLOOR_TICKS * accrual);
        let warn_at = by_share.min(by_floor);

        // Latched on the resource rather than re-read, `raid_check`'s
        // reason: as a bare inequality this line would be said twice a
        // second for the whole warning window.
        if level >= warn_at
            && !self
                .world
                .resource::<crate::resources::SiegePressure>()
                .warned
        {
            self.world
                .resource_mut::<crate::resources::SiegePressure>()
                .warned = true;
            self.log_base_kind(
                MessageKind::Raid,
                "Movement gathers at the perimeter. A siege is forming.".to_string(),
            );
        }
    }

    /// Whether the siege clock has warned for the interval it is in — the
    /// attention row's own read, and every test's.
    pub fn siege_warned(&self) -> bool {
        self.world
            .resource::<crate::resources::SiegePressure>()
            .warned
    }
}
