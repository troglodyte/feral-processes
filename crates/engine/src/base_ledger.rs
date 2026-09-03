//! What the base produced and consumed, bucketed over the run.
//!
//! The counter half of base instrumentation: every production seam emits one
//! [`Event`], which folds in here unconditionally and appends to
//! `crate::telemetry` only when a dev log is armed. One emission, two
//! consumers, so the player's screen and the analysis a retune was done from
//! cannot disagree about what happened.
//!
//! Bucketing is on `resources::GameClock::tick`, never on wall time — rest
//! does not advance the clock, so the tick is a clean monotonic counter of
//! actions taken.

use std::collections::{BTreeMap, VecDeque};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::items::ItemId;

/// Ticks in one bucket. A Mining Node cycle is 10 ticks and a Fabricator or
/// Armory cycle 30, so a bucket is ~100 of the former and ~33 of the latter
/// — never mostly-zero, which is what makes a sparkline of them readable.
pub const BUCKET_TICKS: u64 = 1_000;

/// How many buckets are kept. 64 of them is 64,000 ticks of visible history.
///
/// **This is a save-size decision, not a display one.** Halving it later is
/// easy; doubling it later invalidates the window every earlier save
/// recorded.
pub const MAX_BUCKETS: usize = 64;

/// One item's running totals, never rolled off.
///
/// The three production fields are separate rather than one `produced`
/// because the player screen splits them and a combined figure would hide
/// what hand-compiling is actually doing — see the `COMPILED machine hand`
/// columns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTotals {
    /// Landed from an extractor — a structure declaring `work`.
    pub mined: u32,
    /// Landed from an assembler — a structure declaring `assembles`.
    pub compiled: u32,
    /// Landed from the player's own hands.
    pub hand: u32,
    /// Drained as an input, or spent to raise something.
    pub consumed: u32,
    /// Rolled by a machine but with nowhere to land: the clog loss.
    pub lost: u32,
}

impl ItemTotals {
    /// Everything that reached a shelf, however it was made.
    pub fn produced(&self) -> u32 {
        self.mined + self.compiled + self.hand
    }
}

/// One window of the run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bucket {
    /// The first tick this window covers, always a multiple of
    /// [`BUCKET_TICKS`].
    pub start_tick: u64,
    /// The sector the window opened in.
    ///
    /// **Now or never.** Counters roll across a breach — `enter_next_zone`
    /// does not touch the base, which is the whole point of a base that
    /// travels — so without this the history is a blend of sectors and
    /// nothing in it can be attributed.
    pub zone: u32,
    pub produced: BTreeMap<ItemId, u32>,
    pub consumed: BTreeMap<ItemId, u32>,
}

/// What one production seam did, in the cheapest form that still answers the
/// questions. Carries no `String`: building one costs an allocation the
/// ledger fold would pay on every cycle whether or not a dev log is armed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// An extractor finished a cycle. `ok` is false for a fizzle, which is
    /// the only empirical route to `systems::mining_success_chance`.
    Extract {
        item: ItemId,
        rolled: u32,
        landed: u32,
        ok: bool,
    },
    /// An assembler completed one unit, draining its inputs.
    Assemble {
        product: ItemId,
        inputs: Vec<(ItemId, u32)>,
    },
    /// The player finished one unit by hand.
    HandCraft { item: ItemId, qty: u32 },
}

/// The bucketed counters, saved with the run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Resource, Serialize, Deserialize)]
pub struct BaseLedger {
    /// Per item, for the whole run. Never rolls off, so a fresh save shows
    /// something from tick one rather than an empty screen that reads as
    /// broken.
    pub lifetime: BTreeMap<ItemId, ItemTotals>,
    /// Newest at the back. Windows with no events are skipped rather than
    /// stored empty, so a quiet stretch costs nothing.
    pub buckets: VecDeque<Bucket>,
}

impl BaseLedger {
    /// Folds one event in at `tick`, opening a new bucket if the window has
    /// moved on.
    pub fn fold(&mut self, tick: u64, zone: u32, event: &Event) {
        let start_tick = tick - tick % BUCKET_TICKS;
        if self.buckets.back().map(|b| b.start_tick) != Some(start_tick) {
            self.buckets.push_back(Bucket {
                start_tick,
                zone,
                ..Bucket::default()
            });
            while self.buckets.len() > MAX_BUCKETS {
                self.buckets.pop_front();
            }
        }

        match event {
            Event::Extract {
                item,
                rolled,
                landed,
                ok,
            } => {
                if !ok {
                    return;
                }
                self.produce(item, *landed, rolled.saturating_sub(*landed), |t| {
                    &mut t.mined
                });
            }
            Event::Assemble { product, inputs } => {
                self.produce(product, 1, 0, |t| &mut t.compiled);
                for (item, qty) in inputs {
                    self.lifetime.entry(item.clone()).or_default().consumed += qty;
                    if let Some(bucket) = self.buckets.back_mut() {
                        *bucket.consumed.entry(item.clone()).or_default() += qty;
                    }
                }
            }
            Event::HandCraft { item, qty } => {
                self.produce(item, *qty, 0, |t| &mut t.hand);
            }
        }
    }

    /// The one write path for something that landed: the lifetime total, the
    /// clog loss beside it, and the current bucket, so a caller cannot
    /// update one and forget another.
    fn produce(
        &mut self,
        item: &ItemId,
        landed: u32,
        lost: u32,
        field: impl Fn(&mut ItemTotals) -> &mut u32,
    ) {
        let totals = self.lifetime.entry(item.clone()).or_default();
        *field(totals) += landed;
        totals.lost += lost;
        if landed == 0 {
            return;
        }
        if let Some(bucket) = self.buckets.back_mut() {
            *bucket.produced.entry(item.clone()).or_default() += landed;
        }
    }

    /// The totals for one sector alone, summed over the buckets stamped with
    /// it.
    pub fn produced_in_zone(&self, zone: u32, item: &ItemId) -> u32 {
        self.buckets
            .iter()
            .filter(|b| b.zone == zone)
            .filter_map(|b| b.produced.get(item))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> ItemId {
        ItemId(id.to_string())
    }

    #[test]
    fn an_extract_lands_in_lifetime_and_in_a_bucket() {
        let mut ledger = BaseLedger::default();
        ledger.fold(
            0,
            1,
            &Event::Extract {
                item: item("core_fragment"),
                rolled: 3,
                landed: 3,
                ok: true,
            },
        );

        assert_eq!(ledger.lifetime[&item("core_fragment")].mined, 3);
        assert_eq!(ledger.buckets.len(), 1);
        assert_eq!(ledger.buckets[0].produced[&item("core_fragment")], 3);
        assert_eq!(ledger.buckets[0].zone, 1);
    }

    #[test]
    fn a_fizzle_produces_nothing_but_is_still_a_cycle() {
        let mut ledger = BaseLedger::default();
        ledger.fold(
            0,
            1,
            &Event::Extract {
                item: item("core_fragment"),
                rolled: 0,
                landed: 0,
                ok: false,
            },
        );

        assert_eq!(ledger.lifetime.get(&item("core_fragment")), None);
    }

    #[test]
    fn what_a_clog_ate_is_counted_apart_from_what_landed() {
        let mut ledger = BaseLedger::default();
        ledger.fold(
            0,
            1,
            &Event::Extract {
                item: item("core_fragment"),
                rolled: 5,
                landed: 2,
                ok: true,
            },
        );

        let totals = ledger.lifetime[&item("core_fragment")];
        assert_eq!(totals.mined, 2);
        assert_eq!(totals.lost, 3);
    }

    #[test]
    fn an_assemble_counts_its_product_and_drains_its_inputs() {
        let mut ledger = BaseLedger::default();
        ledger.fold(
            0,
            1,
            &Event::Assemble {
                product: item("bytecode_block"),
                inputs: vec![(item("raw_trace"), 2)],
            },
        );

        assert_eq!(ledger.lifetime[&item("bytecode_block")].compiled, 1);
        assert_eq!(ledger.lifetime[&item("raw_trace")].consumed, 2);
        assert_eq!(ledger.buckets[0].consumed[&item("raw_trace")], 2);
    }

    #[test]
    fn a_hand_craft_is_counted_apart_from_a_machine_one() {
        let mut ledger = BaseLedger::default();
        ledger.fold(
            0,
            1,
            &Event::HandCraft {
                item: item("hardened_shell"),
                qty: 2,
            },
        );

        let totals = ledger.lifetime[&item("hardened_shell")];
        assert_eq!(totals.hand, 2);
        assert_eq!(totals.compiled, 0);
        assert_eq!(totals.produced(), 2);
    }

    #[test]
    fn a_new_window_opens_a_bucket_and_a_quiet_one_is_skipped() {
        let mut ledger = BaseLedger::default();
        let ev = Event::HandCraft {
            item: item("hardened_shell"),
            qty: 1,
        };

        ledger.fold(0, 1, &ev);
        ledger.fold(BUCKET_TICKS - 1, 1, &ev);
        assert_eq!(ledger.buckets.len(), 1, "one window, one bucket");

        ledger.fold(BUCKET_TICKS, 1, &ev);
        assert_eq!(ledger.buckets.len(), 2);
        assert_eq!(ledger.buckets[1].start_tick, BUCKET_TICKS);

        // Nothing happens for a long stretch: the empty windows are not stored.
        ledger.fold(BUCKET_TICKS * 40, 1, &ev);
        assert_eq!(ledger.buckets.len(), 3);
        assert_eq!(ledger.buckets[2].start_tick, BUCKET_TICKS * 40);
    }

    #[test]
    fn the_window_rolls_off_at_the_cap_but_lifetime_does_not() {
        let mut ledger = BaseLedger::default();
        let ev = Event::HandCraft {
            item: item("hardened_shell"),
            qty: 1,
        };

        for window in 0..(MAX_BUCKETS as u64 + 10) {
            ledger.fold(window * BUCKET_TICKS, 1, &ev);
        }

        assert_eq!(ledger.buckets.len(), MAX_BUCKETS);
        assert_eq!(ledger.buckets[0].start_tick, 10 * BUCKET_TICKS);
        assert_eq!(
            ledger.lifetime[&item("hardened_shell")].hand,
            MAX_BUCKETS as u32 + 10,
            "the lifetime total never rolls off"
        );
    }

    #[test]
    fn a_sector_total_reads_only_the_buckets_stamped_with_it() {
        let mut ledger = BaseLedger::default();
        let ev = Event::HandCraft {
            item: item("hardened_shell"),
            qty: 1,
        };

        ledger.fold(0, 1, &ev);
        ledger.fold(BUCKET_TICKS, 2, &ev);
        ledger.fold(BUCKET_TICKS * 2, 2, &ev);

        assert_eq!(ledger.produced_in_zone(1, &item("hardened_shell")), 1);
        assert_eq!(ledger.produced_in_zone(2, &item("hardened_shell")), 2);
        assert_eq!(ledger.produced_in_zone(3, &item("hardened_shell")), 0);
    }
}
