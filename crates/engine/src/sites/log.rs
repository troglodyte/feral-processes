//! The per-site event log: a capped ring of engine-worded entries, oldest
//! first inside, read newest first.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::RouteDestinationId;

/// Appended to, never inserted into: the variant name is the save.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteLogKind {
    Visit,
    FirstVisit,
    Trade,
    Standing,
    ContractAccepted,
    ContractDelivered,
    ContractAbandoned,
    Gift,
    Grew,
    RouteDelivered,
    RoutePreyedOn,
    RaidedBase,
    Founded,
    TierReached,
    Trend,
    Raided,
    Dark,
    Repaired,
    CrewPosted,
    CrewRecalled,
    StockTaken,
    RouteHauledHome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteLogEntry {
    pub kind: SiteLogKind,
    pub tick: u64,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct SiteLog {
    rings: BTreeMap<RouteDestinationId, VecDeque<SiteLogEntry>>,
}

/// One site's ring as saved, oldest first. A `Vec` of records rather than a
/// map keyed by an enum, so the RON text of a save stays plain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteLogSave {
    pub site: RouteDestinationId,
    pub entries: Vec<SiteLogEntry>,
}

impl SiteLog {
    /// Adds `entry` to `site`'s ring, dropping the oldest past `cap`.
    pub fn push(&mut self, site: RouteDestinationId, entry: SiteLogEntry, cap: usize) {
        let ring = self.rings.entry(site).or_default();
        ring.push_back(entry);
        while ring.len() > cap {
            ring.pop_front();
        }
    }

    pub fn newest_first(&self, site: RouteDestinationId) -> impl Iterator<Item = &SiteLogEntry> {
        self.rings
            .get(&site)
            .into_iter()
            .flat_map(|ring| ring.iter().rev())
    }

    pub fn to_save(&self) -> Vec<SiteLogSave> {
        self.rings
            .iter()
            .map(|(site, ring)| SiteLogSave {
                site: *site,
                entries: ring.iter().cloned().collect(),
            })
            .collect()
    }

    pub fn from_save(saved: Vec<SiteLogSave>) -> SiteLog {
        SiteLog {
            rings: saved
                .into_iter()
                .map(|s| (s.site, s.entries.into_iter().collect()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlements::SettlementKey;

    fn entry(n: u64) -> SiteLogEntry {
        SiteLogEntry {
            kind: SiteLogKind::Visit,
            tick: n,
            text: format!("entry {n}"),
        }
    }

    fn town(rx: i32) -> RouteDestinationId {
        RouteDestinationId::Settlement(SettlementKey { rx, ry: 0 })
    }

    #[test]
    fn the_ring_drops_the_oldest_past_its_cap() {
        let mut log = SiteLog::default();
        for n in 1..=13 {
            log.push(town(0), entry(n), 12);
        }
        let ticks: Vec<u64> = log.newest_first(town(0)).map(|e| e.tick).collect();
        assert_eq!(ticks.len(), 12);
        assert_eq!(ticks[0], 13);
        assert_eq!(*ticks.last().unwrap(), 2);
    }

    #[test]
    fn sites_keep_separate_rings() {
        let mut log = SiteLog::default();
        log.push(town(0), entry(1), 12);
        log.push(RouteDestinationId::Outpost((3, 4)), entry(2), 12);
        assert_eq!(log.newest_first(town(0)).count(), 1);
        assert_eq!(log.newest_first(town(1)).count(), 0);
        let outpost = RouteDestinationId::Outpost((3, 4));
        assert_eq!(log.newest_first(outpost).next().unwrap().tick, 2);
    }
}
