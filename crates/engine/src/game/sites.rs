//! The shell around the Sites stores (`sites/`): every writer goes through
//! `note_site`, so a log line is one call at the door that did the event.

use std::collections::BTreeSet;

use crate::RouteDestinationId;
use crate::game::settlement_relations::{
    AID_GARRISON, AID_GIFT_READY, THREAT_RAID_REACH, THREAT_RAIDERS,
};
use crate::routes::RouteLeg;
use crate::settlements::{SettlementKey, Standing};
use crate::sites::{
    CaravanSchedule, RouteLine, SeenShelf, SiteLogEntry, SiteLogKind, SiteLogLine, SiteRailEntry,
    SiteReach, SiteSection, TownGoods, TownStatus, seen_offers, time_band,
};
use crate::tuning::{LIVE_SHELF_RESEARCH, OUTPOST_MAX_INTEGRITY, SITE_LOG_CAP};
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

/// The views the Sites hub reads. Each is a call into the door that already
/// answers the question, so the hub and the visit pages cannot disagree.
impl Game {
    /// Where the party stands relative to the sites. `beside` is a visited
    /// settlement within `settlement_reach`, else an outpost within
    /// `standing_at_outpost`; ties go to the lowest key, then the lowest tile.
    pub fn site_reach(&self) -> SiteReach {
        let town = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .iter()
            .find(|(key, known)| known.visited && self.settlement_reach(**key))
            .map(|(key, _)| RouteDestinationId::Settlement(*key));
        let beside = town.or_else(|| {
            self.world
                .resource::<crate::resources::Outposts>()
                .0
                .keys()
                .find(|tile| self.standing_at_outpost(**tile))
                .map(|tile| RouteDestinationId::Outpost(*tile))
        });
        SiteReach {
            on_base: self.on_base_floor(),
            beside,
        }
    }

    /// Every visited settlement by name, then every outpost by name. Ties
    /// fall to the key, so two towns with one name stay in a stable order.
    pub fn site_rail(&mut self) -> Vec<SiteRailEntry> {
        let stalled: BTreeSet<RouteDestinationId> = self
            .route_reports()
            .into_iter()
            .filter(|r| r.stalled)
            .map(|r| r.destination)
            .collect();
        let mut towns: Vec<(String, SettlementKey)> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .iter()
            .filter(|(_, known)| known.visited)
            .map(|(key, known)| (known.def.name.clone(), *key))
            .collect();
        towns.sort();
        let mut rail = Vec::new();
        for (name, key) in towns {
            let site = RouteDestinationId::Settlement(key);
            let attention = self.standing_band(key) == Standing::Hostile
                || self.gift_available_in(key) == Some(0)
                || stalled.contains(&site);
            let glyph = self.settlement_kind(key).map_or('s', |kind| kind.glyph());
            rail.push(SiteRailEntry {
                site,
                name,
                glyph,
                section: SiteSection::Settlements,
                attention,
            });
        }
        let mut outposts: Vec<(String, crate::views::OutpostMark)> = self
            .outpost_marks()
            .into_iter()
            .map(|mark| (self.outpost_destination_name(mark.tile), mark))
            .collect();
        outposts.sort_by(|a, b| (&a.0, a.1.tile).cmp(&(&b.0, b.1.tile)));
        for (name, mark) in outposts {
            let site = RouteDestinationId::Outpost(mark.tile);
            let damaged = self
                .world
                .resource::<crate::resources::Outposts>()
                .0
                .get(&mark.tile)
                .is_some_and(|o| o.integrity < OUTPOST_MAX_INTEGRITY);
            rail.push(SiteRailEntry {
                site,
                name,
                glyph: mark.glyph,
                section: SiteSection::Outposts,
                attention: mark.dark || damaged || stalled.contains(&site),
            });
        }
        rail
    }

    /// A town's Status tab. Built from the band's own named queries and the
    /// shared growth reading, never from `settlement_aid_lines`, which is
    /// reach-gated: the page reads the same from the base and from beside the
    /// town.
    pub fn town_status(&mut self, key: SettlementKey) -> Option<TownStatus> {
        let known = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)?
            .clone();
        let band = self.standing_band(key);
        let kind = self.settlement_kind(key)?;
        let reading = self.growth_reading(key);
        let condition = match (reading.vitality, reading.outlook) {
            (Some(vitality), _) => Some(format!("Vitality: {}", vitality.label())),
            (None, Some(outlook)) => Some(outlook.label().to_string()),
            (None, None) => None,
        };
        Some(TownStatus {
            name: known.def.name,
            kind: kind.label(),
            specialty: known.def.specialty.label(),
            temperament: known.def.temperament.label(),
            standing: band.label(),
            hostile: band == Standing::Hostile,
            meaning: self.standing_meaning(key, band),
            condition,
            gift: self.gift_line(key),
            travel: self.travel_line(band),
        })
    }

    /// What a town's band means for the party, one sentence each.
    fn standing_meaning(&self, key: SettlementKey, band: Standing) -> Vec<String> {
        let mut meaning = Vec::new();
        meaning.push(if band.refuses_service() {
            "Their counter is shut to you.".to_string()
        } else if band.allows_standing_route() {
            "They trade with you and take standing routes.".to_string()
        } else {
            "They trade with you, but take only one-off routes.".to_string()
        });
        if self.town_garrisons(key) > 0 {
            meaning.push(AID_GARRISON.to_string());
        }
        if band.sends_raiders() {
            meaning.push(if self.raiding_towns().contains(&key) {
                THREAT_RAIDERS.to_string()
            } else {
                THREAT_RAID_REACH.to_string()
            });
        }
        if band.fields_patrols() {
            meaning.push("Their patrols go after you.".to_string());
        }
        if band.preys_on_routes() {
            meaning.push("They prey on caravans that pass near them.".to_string());
        }
        meaning
    }

    fn gift_line(&self, key: SettlementKey) -> String {
        match self.gift_available_in(key) {
            None => "They give no gifts at this standing.".to_string(),
            Some(0) => AID_GIFT_READY.to_string(),
            Some(remaining) => format!(
                "Nobody spare for you yet; one frees up {}.",
                time_band(remaining).until()
            ),
        }
    }

    fn travel_line(&mut self, band: Standing) -> String {
        if !band.hosts_a_relay() {
            "They will not hold a relay link open for you.".to_string()
        } else if self.has_relay() {
            "Your Relay can carry you here.".to_string()
        } else {
            "They would hold a relay link open once you build a Relay.".to_string()
        }
    }

    /// The town's own active jobs, in the order they were taken.
    pub fn town_contracts(&self, key: SettlementKey) -> Vec<crate::views::ContractRow> {
        self.active_contracts()
            .into_iter()
            .filter(|row| row.issuer == Some(key))
            .collect()
    }

    /// Routes in flight to or from `site`.
    pub fn site_routes(&self, site: RouteDestinationId) -> Vec<RouteLine> {
        self.route_lines()
            .into_iter()
            .filter(|line| line.site == site)
            .collect()
    }

    /// Every route in flight: settlements first, then outposts, each by name.
    pub fn routes_in_flight(&self) -> Vec<RouteLine> {
        let mut lines = self.route_lines();
        lines.sort_by_key(|line| {
            (
                matches!(line.site, RouteDestinationId::Outpost(_)),
                line.site_name.clone(),
            )
        });
        lines
    }

    fn route_lines(&self) -> Vec<RouteLine> {
        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        self.route_reports()
            .into_iter()
            .map(|r| {
                let cargo = if r.leg == RouteLeg::Inbound && r.proceeds > 0 {
                    format!("{} {money}", r.proceeds)
                } else if r.cargo.is_empty() {
                    "Nothing yet".to_string()
                } else {
                    r.cargo
                        .iter()
                        .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                RouteLine {
                    site: r.destination,
                    site_name: r.destination_name,
                    leg: match r.leg {
                        RouteLeg::Outbound => "Outbound",
                        RouteLeg::Inbound => "Inbound",
                    },
                    cargo,
                    due: if r.stalled {
                        "held up for stock"
                    } else {
                        time_band(r.ticks_left).until()
                    },
                    standing: r.standing,
                    stalled: r.stalled,
                }
            })
            .collect()
    }

    /// The visiting trader: docked now (read off the open visit window, so it
    /// does not depend on where the party stands) and the next to arrive.
    pub fn caravan_schedule(&self) -> CaravanSchedule {
        let now = self.current_tick();
        let name_of = |def_id: &str| -> String {
            self.world
                .resource::<crate::caravans::CaravanDb>()
                .get(def_id)
                .map_or_else(|| "A trader".to_string(), |def| def.name.clone())
        };
        let docked = self.scheduled_visit().map(|visit| {
            (
                name_of(&visit.def_id),
                time_band(visit.depart_tick.saturating_sub(now)).until(),
            )
        });
        let next = if self.has_trading_structure() {
            let index = self.visit_index();
            [index, index + 1]
                .into_iter()
                .filter_map(|i| self.visit_at(i))
                .find(|visit| visit.arrival_tick > now)
                .map(|visit| {
                    (
                        name_of(&visit.def_id),
                        time_band(visit.arrival_tick - now).until(),
                    )
                })
        } else {
            None
        };
        CaravanSchedule { docked, next }
    }

    /// The site's log, newest first, each line aged in words.
    pub fn site_log(&self, site: RouteDestinationId) -> Vec<SiteLogLine> {
        let now = self.current_tick();
        self.sites
            .log
            .newest_first(site)
            .map(|entry| SiteLogLine {
                age: time_band(now.saturating_sub(entry.tick)).ago(),
                text: entry.text.clone(),
            })
            .collect()
    }
}
