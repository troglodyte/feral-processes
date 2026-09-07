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
use crate::settlements::{SettlementKey, SettlementKind, Standing, Vitality, growth};

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
    ///
    /// **The floor is applied here and nowhere else**, which is what makes
    /// it a property of the band rather than of the number: `commerce`
    /// keeps drifting under a floored city exactly as it does under any
    /// other, so trading with a town it has been holding up does not first
    /// have to climb back out of a hole the player never dug. See
    /// `growth::vitality_floor` for why an untraded town holds at `Steady`
    /// and why `Hostile` lifts that.
    pub(crate) fn settlement_vitality(&self, key: SettlementKey) -> Option<Vitality> {
        if self.settlement_kind(key)? != SettlementKind::Mainframe {
            return None;
        }
        let relation = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .copied()
            .unwrap_or_default();
        let floor = growth::vitality_floor(relation.traded, self.standing_band(key));
        Some(growth::vitality(relation.commerce).max(floor))
    }

    /// Settles every known town's commerce drift, then latches any Server
    /// past its date.
    ///
    /// **Materialized towns only** — the walk is over `Settlements`, which
    /// is `town_garrisons`' and `raiding_towns`' rule and the same reason: a
    /// town whose tile has never been resolved has no entity to repaint and
    /// no name a log line could use. A region nobody has walked into is
    /// evaluated when they do, silently, by `ensure_local_settlements`.
    pub(crate) fn settlement_growth_tick(&mut self) {
        let keys: Vec<SettlementKey> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .keys()
            .copied()
            .collect();
        for key in keys {
            self.settle_commerce_drift(key);
            self.latch_growth(key);
        }
    }

    /// Folds every drift epoch since the last one into `commerce`.
    ///
    /// Lazy against an epoch rather than applied per tick, `static_epoch`'s
    /// shape: a fast-forward cannot be outrun, and no arithmetic runs over
    /// every town on a tick where nothing has changed. The Hostile
    /// surcharge reads the **current** band, never a history — repairing
    /// standing stops the acceleration the tick it lands.
    ///
    /// **The drift is a delta paid through `adjust_commerce`, not a write.**
    /// It owns `commerce_epoch` — the bookmark saying how far it has
    /// settled — and nothing else, which is what keeps the door below the
    /// only place `commerce` itself is assigned.
    pub(crate) fn settle_commerce_drift(&mut self, key: SettlementKey) {
        let epoch = self.current_tick() / crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS;
        let hostile = self.standing_band(key) == Standing::Hostile;
        let elapsed = {
            let mut standings = self.world.resource_mut::<crate::resources::Standings>();
            let relation = standings.0.entry(key).or_default();
            if epoch <= relation.commerce_epoch {
                return;
            }
            let elapsed = (epoch - relation.commerce_epoch).min(i32::MAX as u64) as i32;
            relation.commerce_epoch = epoch;
            elapsed
        };
        let rate = crate::tuning::SETTLEMENT_COMMERCE_DECAY
            + if hostile {
                crate::tuning::SETTLEMENT_COMMERCE_HOSTILE_DECAY
            } else {
                0
            };
        self.adjust_commerce(key, rate.saturating_mul(elapsed).saturating_neg());
    }

    /// Throws the latch if this Server is past its date. `true` if **this
    /// call** flipped it, which is what lets a caller announce a change
    /// without announcing a discovery.
    ///
    /// The inequality lives here and nowhere else. Every reader asks
    /// `settlement_kind`, which is a `||` over a stored bool — so a
    /// decaying commerce can never un-grow a city, and a later change to
    /// the formula cannot leave two sites disagreeing about what a town is.
    pub(crate) fn latch_growth(&mut self, key: SettlementKey) -> bool {
        if self.settlement_kind(key) != Some(SettlementKind::Server) {
            return false;
        }
        let seed = self.world.resource::<crate::world::WorldMap>().seed();
        let commerce = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(0, |relation| relation.commerce);
        let due = growth::due_tick(seed, key) as i64 - growth::pull_ticks(commerce);
        if (self.current_tick() as i64) < due {
            return false;
        }
        self.world
            .resource_mut::<crate::resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .grown = true;
        true
    }

    /// **The one door `Relation::commerce` is assigned through** —
    /// `adjust_standing`'s shape and its reason: one clamp is enough only
    /// because there is one writer, and `growth::clamp_commerce` says the
    /// same thing from the other end.
    ///
    /// Two movers reach it and both hand it a delta rather than a value:
    /// `settle_commerce_drift`, which pays the decay of every elapsed
    /// epoch, and `credit_trade_volume`, which pays what a basket bought.
    /// Neither touches the field. A third mover that assigned `commerce`
    /// beside this would compile clean and put a city at a row count the
    /// constants never authored — the shape `BoughtStats` has already been
    /// bitten by.
    pub(crate) fn adjust_commerce(&mut self, key: SettlementKey, delta: i32) {
        if delta == 0 {
            return;
        }
        let mut standings = self.world.resource_mut::<crate::resources::Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.commerce = growth::clamp_commerce(relation.commerce.saturating_add(delta));
    }
}
