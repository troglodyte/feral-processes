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

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::components::{Glyph, GlyphColor, Position, Settlement, SettlementCentre};
use crate::settlements::{SettlementKey, SettlementKind, Standing, Vitality, growth};
use crate::tuning::{
    SETTLEMENT_RADIUS_SERVER, SETTLEMENT_RADIUS_STARVED, SETTLEMENT_RADIUS_STEADY,
    SETTLEMENT_RADIUS_THRIVING,
};

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
            if self.latch_growth(key) {
                self.announce_growth(key);
            }
        }
    }

    /// Repaints the map and tells the player, exactly once.
    ///
    /// Called only on `latch_growth`'s `true` — the flip, not the state.
    /// `settlement_growth_tick` runs every tick and the latch is already set
    /// on the second one, so announcing on the state would write this line
    /// for the rest of the run.
    ///
    /// **The repaint is not optional.** The glyph is baked into every
    /// footprint cell at materialization, so without this the map keeps
    /// drawing the town the city used to be until the next load rebuilds it
    /// — and `restore_settlements` asking `settlement_kind` is exactly what
    /// makes that divergence survive to the next session rather than
    /// announce itself.
    ///
    /// Growing also grows the square: a fresh Mainframe reads Steady
    /// (`growth::vitality_floor`), radius 2 against a Server's 1, so
    /// `sync_settlement_footprint` is doing two jobs at once here — the
    /// repaint this comment used to do by hand, and the size change that
    /// comes with it.
    fn announce_growth(&mut self, key: SettlementKey) {
        self.sync_settlement_footprint(key);
        let name = self.settlement_name(key);
        self.log_kind(
            crate::resources::MessageKind::Info,
            format!("{name} has grown. It is a Mainframe now."),
        );
        // Only for a town the party has actually stood in. A notification
        // takes the screen, and a place they have never reached has not
        // earned that — `KnownSettlement::visited` is the same flag the
        // compass uses to decide whether a town has a name worth showing.
        // The line above is written either way: the log is a record, and a
        // record of a place they have heard of is not an interruption.
        let visited = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .is_some_and(|known| known.visited);
        if visited {
            self.notify(crate::notifications::NotificationKind::SettlementGrown);
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

    /// How big `key`'s footprint reads *right now* — the half-width of the
    /// square `footprint` draws.
    ///
    /// Exhaustive over `settlement_kind` crossed with `settlement_vitality`,
    /// `cell_mark`'s rule: a fifth kind or a fourth band fails to compile
    /// rather than silently drawing whatever radius the match happened to
    /// fall through to.
    ///
    /// `None` for an unmaterialized key, `settlement_kind`'s own reason: a
    /// town whose tile has never been resolved has no square to draw either.
    pub(crate) fn settlement_radius(&self, key: SettlementKey) -> Option<i32> {
        Some(match self.settlement_kind(key)? {
            SettlementKind::Server => SETTLEMENT_RADIUS_SERVER,
            SettlementKind::Mainframe => match self
                .settlement_vitality(key)
                .expect("a materialized Mainframe always answers a vitality band")
            {
                Vitality::Starved => SETTLEMENT_RADIUS_STARVED,
                Vitality::Steady => SETTLEMENT_RADIUS_STEADY,
                Vitality::Thriving => SETTLEMENT_RADIUS_THRIVING,
            },
        })
    }

    /// Every cell `key`'s footprint covers right now, centred on its
    /// resolved tile — the one derivation every reader below calls rather
    /// than each re-deriving the square from a radius of its own.
    ///
    /// `Vec::new()` for an unmaterialized key, `settlement_shelf`'s reason:
    /// nothing here may panic on a key `resources::Settlements` does not
    /// know.
    pub(crate) fn footprint(&self, key: SettlementKey) -> Vec<(i32, i32)> {
        let Some(tile) = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.tile)
        else {
            return Vec::new();
        };
        let Some(radius) = self.settlement_radius(key) else {
            return Vec::new();
        };
        let side = (2 * radius + 1) as usize;
        let mut cells = Vec::with_capacity(side * side);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                cells.push((tile.0 + dx, tile.1 + dy));
            }
        }
        cells
    }

    /// **The one writer of a settlement's map entities.** Diffs the cells
    /// `footprint` says should exist against the ones that do, despawns and
    /// spawns to match, and repaints every surviving cell's glyph to what
    /// `settlement_kind` says the town is now.
    ///
    /// **Early return when the cell count and the centre's glyph already
    /// match** — this runs once a tick for every known town (beside
    /// `latch_growth`, which only fires on a flip) whether or not anything
    /// moved, and re-spawning cells that already exist would repaint the map
    /// every tick for no reason.
    ///
    /// The centre is the cell at the settlement's own tile and only that one
    /// keeps `SettlementCentre` — the radius is never below 1, so the centre
    /// is never among the cells a shrink despawns, which is what lets a
    /// tether stay pointed at one entity through a resize.
    pub(crate) fn sync_settlement_footprint(&mut self, key: SettlementKey) {
        let Some(tile) = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.tile)
        else {
            return;
        };
        let Some(glyph) = self.settlement_kind(key).map(SettlementKind::glyph) else {
            return;
        };
        let wanted = self.footprint(key);

        let existing: Vec<(Entity, (i32, i32), bool)> = {
            let mut query = self
                .world
                .query::<(Entity, &Settlement, &Position, Option<&SettlementCentre>)>();
            query
                .iter(&self.world)
                .filter(|(_, settlement, ..)| settlement.key == key)
                .map(|(entity, _, pos, centre)| (entity, (pos.x, pos.y), centre.is_some()))
                .collect()
        };
        let centre_glyph = existing
            .iter()
            .find(|(_, _, is_centre)| *is_centre)
            .and_then(|(entity, ..)| self.world.get::<Glyph>(*entity))
            .map(|drawn| drawn.ch);
        if existing.len() == wanted.len() && centre_glyph == Some(glyph) {
            return;
        }

        let have: std::collections::HashSet<(i32, i32)> =
            existing.iter().map(|(_, pos, _)| *pos).collect();
        let want: std::collections::HashSet<(i32, i32)> = wanted.iter().copied().collect();

        for &(entity, pos, _) in &existing {
            if !want.contains(&pos) {
                self.world.despawn(entity);
            }
        }

        let newly_covered: Vec<(i32, i32)> = wanted
            .into_iter()
            .filter(|cell| !have.contains(cell))
            .collect();
        self.displace(&newly_covered);
        for cell in newly_covered {
            let mut spawned = self.world.spawn((
                Settlement { key },
                Position {
                    x: cell.0,
                    y: cell.1,
                },
                Glyph {
                    ch: glyph,
                    color: GlyphColor::Orange,
                },
            ));
            if cell == tile {
                spawned.insert(SettlementCentre);
            }
        }

        let mut repaint = self.world.query::<(&Settlement, &mut Glyph)>();
        for (settlement, mut drawn) in repaint.iter_mut(&mut self.world) {
            if settlement.key == key {
                drawn.ch = glyph;
            }
        }
    }

    /// What happens to whatever already stood on a cell a footprint just
    /// grew over.
    ///
    /// **Empty for now** — Phase 2 fills in the design's per-occupant table.
    /// Called with the cells `sync_settlement_footprint` is about to spawn
    /// onto, which is exactly the cells not already this settlement's own
    /// ground: a settled footprint's unchanged cells are never re-displaced
    /// on a tick where nothing moved.
    #[allow(unused_variables)]
    fn displace(&mut self, cells: &[(i32, i32)]) {}
}
