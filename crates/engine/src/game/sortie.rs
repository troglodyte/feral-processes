//! Sending a squad of base staff away from the base to fight for a while.
//!
//! The whole feature's behaviour: reach, the board, dispatch, the trip and
//! the return. `crate::sorties` is the catalogue and holds no game logic;
//! `resources::Sortie` is the in-flight record.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::Structure;

/// Whether the player can read the board, and whether they can sign for a
/// squad.
///
/// Three states rather than two booleans, for `NoPost::BoxedIn`'s reason:
/// "no Relay built" and "not standing in the base" leave the player
/// different errands, and a screen that cannot tell them apart says the
/// wrong sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortieReach {
    NoRelay,
    OffBase,
    AtRelay,
}

impl Game {
    /// Where the player is standing, as far as sorties are concerned.
    ///
    /// **It measures the base, never the distance to the Relay** — this is
    /// `Game::broker_reach` one verb along, and for its argument:
    /// `place_structure` refuses everything but a Home until a Home is
    /// standing and every structure has to stand on laid floor, so a Relay
    /// is in the base by construction. "Is the player in the base" is
    /// therefore the whole question, which since the base moved out of
    /// phase reads as: the party is in base space, standing on
    /// `BaseCell::Floor`.
    ///
    /// Floor and not merely `walkable`, `broker_reach`'s rule: the mast is
    /// reachable from the base's laid ground, not from a corridor mined out
    /// past its edge.
    pub fn sortie_reach(&mut self) -> SortieReach {
        if !self.has_relay() {
            return SortieReach::NoRelay;
        }
        let Some((x, y)) = self.base_pos() else {
            return SortieReach::OffBase;
        };
        if self
            .world
            .resource::<crate::base_grid::BaseGrid>()
            .is_floor(x, y)
        {
            SortieReach::AtRelay
        } else {
            SortieReach::OffBase
        }
    }

    /// Whether the run has a Relay standing at all, wherever it is.
    fn has_relay(&mut self) -> bool {
        let mut query = self.world.query_filtered::<Entity, With<Structure>>();
        let standing: Vec<Entity> = query.iter(&self.world).collect();
        standing
            .into_iter()
            .any(|entity| self.dispatches_sorties(entity))
    }

    /// Whether `entity` is a structure a squad can be dispatched from —
    /// read off the def's flag and never off the shipped id, so a mod's
    /// second dispatch structure works without an engine change.
    fn dispatches_sorties(&self, entity: Entity) -> bool {
        let Some(kind) = self.world.get::<Structure>(entity).map(|s| &s.kind) else {
            return false;
        };
        self.world
            .resource::<crate::structures::StructureDb>()
            .get(kind)
            .is_some_and(|def| def.dispatches_sorties)
    }

    /// How long a trip to a site of this risk offset, running this many
    /// battles, takes.
    ///
    /// **The one place the figure is computed.** The board quotes it and the
    /// countdown runs it, `views::BuildOrderRow`'s rule that every figure on
    /// a screen is a call rather than a copy — a screen quoting one number
    /// while the countdown runs another is precisely the failure that rule
    /// exists for.
    ///
    /// It reads the site's **risk offset** and never the absolute danger
    /// band, or every trip late in a run would take enormously longer for no
    /// reason the player could name. And there is no term for squad size,
    /// level or power: a stronger squad shows up as better outcomes and
    /// never as a faster cycle.
    pub fn sortie_duration(risk: u32, battles: u32) -> u64 {
        crate::tuning::SORTIE_TRAVEL_BASE_TICKS
            + crate::tuning::SORTIE_TRAVEL_PER_RISK_TICKS * risk as u64
            + crate::tuning::SORTIE_TICKS_PER_BATTLE * battles as u64
    }
}
