//! Whether a town has grown, and how well it is doing.
//!
//! **One door for the effective kind.** `SettlementKind` is now two things
//! folded together — what the catalogue authored and what the run has done
//! — and four sites read it (the shelf's rows, its standout share, the map
//! glyph, the town page's label). Each of them asking
//! `known.def.kind` directly is how they drift apart, and the load path is
//! where that drift is invisible: `restore_settlements` would rebuild a
//! grown city's entity from the authored kind and redraw it as a town.
//! `settlement_kind` is the only reader of `Relation::grown` outside the
//! tick that writes it.

use crate::Game;
use crate::settlements::{SettlementKey, SettlementKind, Vitality, growth};

impl Game {
    /// What this settlement *is* right now — the catalogue's answer, or a
    /// `Mainframe` if the run has grown it there.
    ///
    /// `None` for a key with no materialized record, which is the same
    /// condition `town_garrisons` and `raiding_towns` exclude by
    /// construction: a town whose tile has never been resolved has no
    /// entity to draw and no shelf to stock.
    pub fn settlement_kind(&self, key: SettlementKey) -> Option<SettlementKind> {
        let authored = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)?
            .def
            .kind;
        if authored == SettlementKind::Mainframe {
            return Some(SettlementKind::Mainframe);
        }
        let grown = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .is_some_and(|relation| relation.grown);
        Some(if grown {
            SettlementKind::Mainframe
        } else {
            SettlementKind::Server
        })
    }

    /// How well this city is doing, or `None` if it is not a city.
    ///
    /// A Server answers `None` rather than `Steady`: it has no band, and a
    /// band it never falls out of would put a word on its page that never
    /// changes. See `Vitality::rows`' note on why a Server never asks.
    pub(crate) fn settlement_vitality(&self, key: SettlementKey) -> Option<Vitality> {
        if self.settlement_kind(key)? != SettlementKind::Mainframe {
            return None;
        }
        let commerce = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(0, |relation| relation.commerce);
        Some(growth::vitality(commerce))
    }
}
