//! Caravan routes — a one-off dispatch or a standing arrangement running
//! cargo out to a known settlement and Credits back.
//!
//! `Route` is `WorkOrder`'s shape: the record stores what was asked for,
//! never how it will be done, and a one-off is a standing route that simply
//! does not go again (`standing: false`). `resources::Routes` is the live
//! resource this module's records travel in; `save::RouteSave` is the save
//! form.
//!
//! **Stores the whole resolved destination `SettlementDef`**, `ActiveContract`
//! and `SortieSave`'s reason: a catalogue file edited or a board that rotates
//! while a trip is in flight must not be able to rewrite or strand it.
//! Unlike a sortie's squad, a route's cargo names no entity, so there is no
//! membership scheme to reconcile across a save — `RouteSave` carries the
//! whole record directly.

use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::settlements::{SettlementDef, SettlementKey};

/// Which leg of the round trip a route is currently running.
///
/// Outbound completion sells the cargo at the destination and turns the trip
/// around; inbound completion deposits the proceeds into base stock.
///
/// `Serialize`/`Deserialize` even though `Route` itself is not: this enum
/// holds nothing that fails to round-trip, so `save::RouteSave` reuses it
/// directly rather than carrying a duplicate `RouteLegSave`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RouteLeg {
    Outbound,
    Inbound,
}

/// One caravan trip, dispatched or standing.
///
/// Not `Serialize`: the save form is `save::RouteSave`. `resources::Sorties`
/// is the shape being copied, though a route needs no entity reconciliation
/// on load the way a sortie's membership does.
#[derive(Clone, Debug)]
pub struct Route {
    /// The town this trip runs to, by region — the one name for a
    /// settlement that cannot drift, `SettlementKey`'s own reason.
    pub destination: SettlementKey,
    /// The whole resolved destination, not its id — see the module doc.
    pub destination_def: SettlementDef,
    /// The tile the destination actually stands on, recorded rather than
    /// re-derived — `resources::KnownSettlement::tile`'s reason.
    pub destination_tile: (i32, i32),
    /// What the outbound leg carries, spent from base stock at dispatch.
    pub cargo: Vec<(ItemId, u32)>,
    /// Whether this trip reloads and departs again on its own arrival home,
    /// rather than being a one-off. Severing (`Game::sever_route`) clears
    /// this and nothing else — the trip already in flight still completes
    /// and still pays.
    pub standing: bool,
    /// Set when a standing route's reload finds base stock short. Retried
    /// each tick rather than severed — a stalled work order's rule.
    pub stalled: bool,
    pub leg: RouteLeg,
    pub ticks_total: u64,
    pub ticks_elapsed: u64,
    /// Credits banked from the outbound sale, carried until the inbound leg
    /// deposits them into base stock.
    pub proceeds: u32,
    /// What predation has taken from this trip so far, one line per hit —
    /// the report's own words, not a number a screen has to phrase.
    pub losses: Vec<String>,
}
