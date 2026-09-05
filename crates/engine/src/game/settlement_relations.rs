//! Standing with a town: the one writer and its movers.
//!
//! `settlements::relations` holds the shape — the band ladder, the clamp,
//! the consequence queries — and this holds the verbs. Every mover is a
//! call to `Game::adjust_standing`, never a write beside it, which is what
//! makes the clamp, the log and any future consequence of *changing* band a
//! thing written once. `set_machine_status`' rule is the one being copied:
//! the door speaks only on a transition, so entering a band is news and
//! staying in it is not.

use crate::resources;
use crate::settlements::SettlementKey;
use crate::settlements::relations::{self, Standing};
use crate::tuning::SETTLEMENT_NOTICE_RADIUS;

use crate::Game;
use crate::resources::MessageKind;

impl Game {
    /// What the town `key` names thinks of the party, as a number.
    pub fn standing(&self, key: SettlementKey) -> i32 {
        self.world
            .resource::<resources::Standings>()
            .0
            .get(&key)
            .map(|relation| relation.standing)
            .unwrap_or(0)
    }

    /// The same answer, banded — the form every reader outside this module
    /// actually wants, since the raw number is a budget and the band is the
    /// meaning.
    pub fn standing_band(&self, key: SettlementKey) -> Standing {
        relations::band(self.standing(key))
    }

    /// **The one door a standing is written through.** Clamps to the
    /// feature's bounds and announces a band crossing, in either direction.
    pub(crate) fn adjust_standing(&mut self, key: SettlementKey, delta: i32) {
        if delta == 0 {
            return;
        }
        let before = self.standing(key);
        let after = relations::clamp(before + delta);
        if after == before {
            return;
        }
        self.world
            .resource_mut::<resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .standing = after;
        let (was, now) = (relations::band(before), relations::band(after));
        if was != now {
            let name = self.settlement_name(key);
            self.log_kind(
                MessageKind::Outcome,
                format!("{name} now regards you as {}.", now.label()),
            );
        }
    }

    /// Folds a basket's whole turnover into the town's trade record and
    /// pays out whatever standing it bought — see `Relation::trade_credits`
    /// for why the remainder is kept rather than rounded away.
    pub(crate) fn credit_trade_volume(&mut self, key: SettlementKey, credits: u32) {
        if credits == 0 {
            return;
        }
        let points = self
            .world
            .resource_mut::<resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .credit_trade(credits);
        self.adjust_standing(key, points);
    }

    /// Every town near enough to `tile` to have heard about it, moved by
    /// `delta`.
    ///
    /// Known towns only. A settlement the party has never walked to has no
    /// record of them either, so crediting one would bank goodwill with a
    /// place that has never met them — and the resolved tile a distance is
    /// measured from only exists once the town has been materialized.
    pub(crate) fn credit_nearby_settlements(&mut self, tile: (i32, i32), delta: i32) {
        let nearby: Vec<SettlementKey> = self
            .world
            .resource::<resources::Settlements>()
            .0
            .iter()
            .filter(|(_, known)| {
                (known.tile.0 - tile.0).abs() <= SETTLEMENT_NOTICE_RADIUS
                    && (known.tile.1 - tile.1).abs() <= SETTLEMENT_NOTICE_RADIUS
            })
            .map(|(key, _)| *key)
            .collect();
        for key in nearby {
            self.adjust_standing(key, delta);
        }
    }

    /// The town's own name, for a line about it — falling back to a generic
    /// rather than refusing to speak, since `adjust_standing` is reachable
    /// with a key no town has been materialized for.
    fn settlement_name(&self, key: SettlementKey) -> String {
        self.world
            .resource::<resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.def.name.clone())
            .unwrap_or_else(|| "The settlement".to_string())
    }
}
