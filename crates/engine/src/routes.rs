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

/// Where a route runs to — the extension point design correction 4 asks
/// for, in place of the spec's rejected `Route::outpost: Option<_>`.
///
/// **Not `Serialize`.** `Route` itself isn't either — the save split is
/// `save::RouteSave` (settlement, untouched) and `save::OutpostRouteSave`
/// (new, additive), never a serialized `RouteEnd` — changing `RouteSave`'s
/// `destination` field to an enum is not an additive save change, which is
/// the whole reason this collapses three in-memory fields rather than
/// widening the save form.
#[derive(Clone, Debug, PartialEq)]
pub enum RouteEnd {
    Settlement {
        /// The town this trip runs to, by region — the one name for a
        /// settlement that cannot drift, `SettlementKey`'s own reason.
        key: SettlementKey,
        /// The whole resolved destination, not its id — see the module doc.
        def: SettlementDef,
        /// The tile the destination actually stands on, recorded rather
        /// than re-derived — `resources::KnownSettlement::tile`'s reason.
        tile: (i32, i32),
    },
    /// An outpost names no entity and no def of its own worth resolving
    /// ahead of time — `crate::outposts::Outpost`'s record already lives at
    /// this tile in `resources::Outposts`, looked up live on every tick.
    Outpost((i32, i32)),
}

impl RouteEnd {
    /// The tile this endpoint stands on, whichever kind it is — the one
    /// comparison `dispatch_route`'s `Duplicate` refusal and
    /// `Game::sever_route` need, so neither has to match on the variant
    /// itself.
    pub fn tile(&self) -> (i32, i32) {
        match self {
            RouteEnd::Settlement { tile, .. } => *tile,
            RouteEnd::Outpost(tile) => *tile,
        }
    }

    /// `Some` only for a settlement endpoint — `Game::sever_route`'s
    /// settlement-side match, since an outpost route has no `SettlementKey`
    /// to compare against and is matched on `tile()` instead.
    pub fn settlement_key(&self) -> Option<SettlementKey> {
        match self {
            RouteEnd::Settlement { key, .. } => Some(*key),
            RouteEnd::Outpost(_) => None,
        }
    }
}

/// One caravan trip, dispatched or standing.
///
/// Not `Serialize`: the save form is `save::RouteSave` /
/// `save::OutpostRouteSave`. `resources::Sorties` is the shape being copied,
/// though a route needs no entity reconciliation on load the way a sortie's
/// membership does.
#[derive(Clone, Debug)]
pub struct Route {
    /// Which kind of endpoint this trip runs to — see `RouteEnd`.
    pub destination: RouteEnd,
    /// What the outbound leg carries, spent from base stock at dispatch.
    /// Always empty at dispatch for an outpost endpoint — see
    /// `Game::run_routes`'s outpost arm.
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
}

/// Which of `candidates` — known settlements paired with their tile — lie
/// within `ROUTE_PREDATION_RADIUS` of the segment from `base` to
/// `destination`.
///
/// Point-to-**segment** distance, not point-to-point or point-to-line: a
/// town standing beside the middle of a long route has to be caught exactly
/// as one sitting near either end, and a town merely *colinear* with the
/// route but well past one of its ends must not be caught at all — the
/// clamp below is what tells those two apart.
///
/// Pure — no `&Game`, no RNG. Whether a town is close enough to try is a
/// fact about the map; the roll for whether a given try lands is a tick
/// concern (`Game::run_routes`, a later task), not a geometry one. The
/// caller is expected to have already filtered `candidates` down to
/// `Standing::preys_on_routes` — this function does not read standing at
/// all.
pub fn settlements_near_route(
    candidates: &[(SettlementKey, (i32, i32))],
    base: (i32, i32),
    destination: (i32, i32),
) -> Vec<SettlementKey> {
    candidates
        .iter()
        .filter(|&&(_, tile)| {
            distance_to_segment(tile, base, destination)
                <= crate::tuning::ROUTE_PREDATION_RADIUS as f64
        })
        .map(|&(key, _)| key)
        .collect()
}

/// Euclidean distance from `point` to the segment `a`-`b`. The projection of
/// `point` onto the line through `a` and `b` is clamped to `0.0..=1.0` of
/// the way along it, which is what makes this a *segment* distance rather
/// than an infinite-line one — a point past either end measures to the
/// nearest endpoint instead of to a projection that has run off the route.
fn distance_to_segment(point: (i32, i32), a: (i32, i32), b: (i32, i32)) -> f64 {
    let (px, py) = (point.0 as f64, point.1 as f64);
    let (ax, ay) = (a.0 as f64, a.1 as f64);
    let (bx, by) = (b.0 as f64, b.1 as f64);
    let (abx, aby) = (bx - ax, by - ay);
    let len_sq = abx * abx + aby * aby;
    let t = if len_sq > 0.0 {
        (((px - ax) * abx + (py - ay) * aby) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (ax + t * abx, ay + t * aby);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::ROUTE_PREDATION_RADIUS;

    fn key(n: i32) -> SettlementKey {
        SettlementKey { rx: n, ry: n }
    }

    /// A town offset perpendicular from the midpoint, just inside the
    /// radius, is caught — the ordinary case the feature exists for.
    #[test]
    fn a_town_beside_the_line_is_caught() {
        let base = (0, 0);
        let destination = (20, 0);
        let candidates = [(key(1), (10, ROUTE_PREDATION_RADIUS - 1))];
        let caught = settlements_near_route(&candidates, base, destination);
        assert_eq!(caught, vec![key(1)]);
    }

    /// A town sitting exactly on the segment's midpoint is caught at zero
    /// distance.
    #[test]
    fn a_town_at_the_midpoint_is_caught() {
        let base = (0, 0);
        let destination = (20, 0);
        let candidates = [(key(2), (10, 0))];
        let caught = settlements_near_route(&candidates, base, destination);
        assert_eq!(caught, vec![key(2)]);
    }

    /// A town colinear with the route but well past either end is not
    /// caught — the case that tells a segment distance apart from an
    /// infinite-line one, since a point-to-line measure would read zero for
    /// both.
    #[test]
    fn a_town_past_either_end_is_not_caught() {
        let base = (0, 0);
        let destination = (20, 0);
        let past_destination = (20 + ROUTE_PREDATION_RADIUS + 5, 0);
        let past_base = (-(ROUTE_PREDATION_RADIUS + 5), 0);
        let candidates = [(key(3), past_destination), (key(4), past_base)];
        let caught = settlements_near_route(&candidates, base, destination);
        assert!(
            caught.is_empty(),
            "a town this far past either end must not be caught: {caught:?}"
        );
    }
}
