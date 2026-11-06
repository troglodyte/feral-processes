//! The shell around the Sites stores (`sites/`): every writer goes through
//! `note_site`, so a log line is one call at the door that did the event.

use crate::RouteDestinationId;
use crate::settlements::SettlementKey;
use crate::sites::{SeenShelf, SiteLogEntry, SiteLogKind, TownGoods, seen_offers, time_band};
use crate::tuning::{LIVE_SHELF_RESEARCH, SITE_LOG_CAP};
use crate::*;

impl Game {
    /// Logs `text` on `site`'s ring at the current tick. Draws no RNG.
    pub(crate) fn note_site(&mut self, site: RouteDestinationId, kind: SiteLogKind, text: String) {
        let tick = self.current_tick();
        self.sites
            .log
            .push(site, SiteLogEntry { kind, tick, text }, SITE_LOG_CAP);
    }

    /// Remembers `key`'s shelf as the party sees it now, prices as shown.
    /// A no-op when the party is out of reach or the town has shut its
    /// counter: a closed view shows nothing, and an older sighting is worth
    /// more than an empty one. Draws no RNG: the shelf is its own seeded
    /// derivation.
    pub(crate) fn record_shelf_seen(&mut self, key: SettlementKey) {
        let Some(view) = self.settlement_view(key) else {
            return;
        };
        if view.closed {
            return;
        }
        let shelf = SeenShelf {
            tick: self.current_tick(),
            offers: seen_offers(&view.offers),
        };
        self.sites.seen.record(key, shelf);
    }

    /// Whether the Price Feeds research is known, which swaps every town's
    /// last-seen shelf for the live one.
    pub fn live_shelf_known(&self) -> bool {
        self.is_researched(LIVE_SHELF_RESEARCH)
    }

    /// What `key`'s Goods tab shows. The live shelf is read-only and listed
    /// exactly as the counter lists it (`settlement_shelf_grouped`); buying
    /// still needs the site.
    pub fn town_goods(&mut self, key: SettlementKey) -> TownGoods {
        if self.live_shelf_known() {
            let offers = self.settlement_shelf_grouped(key);
            return TownGoods::Live {
                offers: seen_offers(&offers),
            };
        }
        let now = self.current_tick();
        match self.sites.seen.get(key) {
            Some(shelf) => TownGoods::Seen {
                offers: shelf.offers.clone(),
                age: time_band(now.saturating_sub(shelf.tick)).ago(),
            },
            None => TownGoods::NotSeen,
        }
    }
}
