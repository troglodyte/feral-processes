//! The last-seen shelf of each town: what its market showed the last time
//! the party was there, prices frozen as shown.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::settlements::SettlementKey;
use crate::views::CaravanOffer;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenOffer {
    pub name: String,
    pub detail: String,
    pub unit_cost: u32,
    pub qty: u32,
}

pub fn seen_offers(offers: &[CaravanOffer]) -> Vec<SeenOffer> {
    offers
        .iter()
        .map(|o| SeenOffer {
            name: o.name.clone(),
            detail: o.detail.clone(),
            unit_cost: o.unit_cost,
            qty: o.qty,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeenShelf {
    pub tick: u64,
    pub offers: Vec<SeenOffer>,
}

#[derive(Clone, Debug, Default)]
pub struct SeenShelves {
    shelves: BTreeMap<SettlementKey, SeenShelf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeenShelfSave {
    pub key: SettlementKey,
    pub tick: u64,
    pub offers: Vec<SeenOffer>,
}

impl SeenShelves {
    pub fn record(&mut self, key: SettlementKey, shelf: SeenShelf) {
        self.shelves.insert(key, shelf);
    }

    pub fn get(&self, key: SettlementKey) -> Option<&SeenShelf> {
        self.shelves.get(&key)
    }

    pub fn to_save(&self) -> Vec<SeenShelfSave> {
        self.shelves
            .iter()
            .map(|(key, s)| SeenShelfSave {
                key: *key,
                tick: s.tick,
                offers: s.offers.clone(),
            })
            .collect()
    }

    pub fn from_save(saved: Vec<SeenShelfSave>) -> SeenShelves {
        SeenShelves {
            shelves: saved
                .into_iter()
                .map(|s| {
                    (
                        s.key,
                        SeenShelf {
                            tick: s.tick,
                            offers: s.offers,
                        },
                    )
                })
                .collect(),
        }
    }
}
