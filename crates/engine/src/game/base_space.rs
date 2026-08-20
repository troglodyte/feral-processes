//! Walking in and out of base space through the anchor, and walking around
//! once inside.
//!
//! The counterpart to `game/stack.rs` for the other off-surface locale, and
//! it plays the same trick for the same reason: the party's base-space
//! coordinates live on `resources::Locale`, and the player's `Position`
//! component stays pinned to the anchor tile on the zone surface. See
//! `resources::Locale` for why that is the load-bearing decision — nothing
//! on the surface has to know base space exists.
//!
//! What that costs is a guard on anything that reaches the zone map through
//! `Position`, and `enter_base` is itself one of those things: it asks
//! whether the player is standing on the anchor. Underground the pinned
//! `Position` *is* the entrance tile, and a run that dived from its own
//! starting tile dived from the anchor — so that question answers yes four
//! frames down, and `Game::require_surface` has to be asked first.

use crate::base_grid::BaseGrid;
use crate::resources::Locale;
use crate::*;

/// The one cell of base space the anchor's door opens onto, and the only
/// cell you can leave from — base space's own origin, where the Home
/// stands.
///
/// Here rather than in `tuning.rs`: it is not a knob, it is the origin the
/// whole coordinate space is defined against, and a base space whose exit
/// was somewhere else would be a different space rather than a retuned one.
pub(crate) const BASE_EXIT_CELL: (i32, i32) = (0, 0);

impl Game {
    /// Steps through the anchor and out of phase, landing on
    /// `BASE_EXIT_CELL`.
    ///
    /// Refused unless the party is on the zone surface proper, standing on
    /// the anchor tile, with a Home deployed. That last one is not a
    /// formality: a new run has no base at all, because
    /// `Game::place_structure` refuses every structure until a Home exists
    /// and the Home is player-built. Until one is up, base space is solid
    /// everywhere and the anchor opens onto nothing — so it says so, in
    /// wording that shares nothing with the "you are not on the anchor"
    /// refusal, because those are two different things for a player to fix.
    ///
    /// The player's `Position` is not written here, unlike `enter_stack`'s
    /// pin to the entrance tile. It does not need to be: the guard above is
    /// that the player is *already* standing on the anchor, and nothing in
    /// base space moves `Position` — `move_player` dispatches to
    /// `move_in_base`, which only ever rewrites the locale. The pin is a
    /// property of the two of them together rather than an assignment that
    /// would be a no-op the day it was written.
    pub fn enter_base(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Not right now.".into());
        }
        self.require_surface()?;
        let anchor = self
            .anchor_position()
            .ok_or_else(|| "There's no anchor in this sector.".to_string())?;
        let standing = *self
            .world
            .get::<Position>(self.player_entity())
            .ok_or_else(|| "You aren't anywhere you can phase out from.".to_string())?;
        if (standing.x, standing.y) != anchor {
            return Err("No anchor here — you phase out from the anchor tile.".into());
        }
        if !self.has_structure(HOME_STRUCTURE_ID) {
            return Err(
                "The anchor is dark. There's nothing on the other side until you deploy a Home."
                    .into(),
            );
        }

        let (x, y) = BASE_EXIT_CELL;
        self.world.insert_resource(Locale::Base { x, y });
        self.log("You step into the anchor and phase out of the sector.");
        self.tick();
        Ok(())
    }

    /// Steps back out through the anchor onto the tile it stands on.
    ///
    /// Refused anywhere but `BASE_EXIT_CELL` — base space has one door, not
    /// one per wall. Nothing has to be restored on the way out beyond the
    /// locale: unlike `Game::clear_stack`, which also drops the generated
    /// frame and the Trace built up inside it, base space keeps no
    /// per-visit state at all. `BaseGrid` is the player's own dug ground and
    /// outlives every visit, including a breach into the next zone.
    pub fn leave_base(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Not right now.".into());
        }
        self.require_base()?;
        let standing = self
            .base_pos()
            .expect("require_base passed, so the party is in base space");
        if standing != BASE_EXIT_CELL {
            return Err("The way out is back at the Home — walk to it first.".into());
        }

        self.world.insert_resource(Locale::Surface);
        self.log("You phase back in, standing on the anchor.");
        self.tick();
        Ok(())
    }

    /// One step through base space, reached from `Game::move_player` — the
    /// same four keys, dispatched on locale.
    ///
    /// Reads `BaseGrid::walkable` and nothing else. `WorldMap` is a
    /// different coordinate space entirely and has no say here, and solid
    /// rock is simply not walkable — there is no way to end up standing
    /// inside it, so base space needs no analogue of the Stack's
    /// `die_in_the_rock`.
    ///
    /// **A refused step costs no turn**, unlike shoving at a wall on the zone
    /// surface or in the Stack, which both charge one. Base space is walls
    /// until the player cuts them, and charging a turn for every corner of
    /// your own base you brush against would tax walking around indoors.
    ///
    /// **A refused step still breaks off a posted job**, and that one *does*
    /// match the surface: `move_player` drops the job before it looks at
    /// what is in the way, on the grounds that either way you stopped
    /// working to do it, and `Game::work_structure` promises the player as
    /// much when it posts. The two rules point opposite ways on purpose —
    /// the turn is what the world charges for a step, and the job is what
    /// the player's attention was on — so the order below is load-bearing
    /// rather than incidental.
    pub(crate) fn move_in_base(&mut self, dx: i32, dy: i32) {
        let Some((x, y)) = self.base_pos() else {
            return;
        };
        // Before the wall check, exactly as `move_player` does it.
        self.break_off_job();
        let (nx, ny) = (x + dx, y + dy);
        if !self.world.resource::<BaseGrid>().walkable(nx, ny) {
            return;
        }
        self.world.insert_resource(Locale::Base { x: nx, y: ny });
        self.tick();
    }
}
