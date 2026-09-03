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

use bevy_ecs::prelude::{ResMut, Resource};
use bevy_ecs::system::SystemParam;
use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::resources::BattleTelemetry;
use crate::telemetry::Record;

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
    /// Units left the run for good: burnt as fuel, built into a structure,
    /// spent on a sortie or destroyed by a breach.
    ///
    /// Its own variant rather than a field on the others because a
    /// consumption has no product — and without it the ledger's `produced`
    /// side is a stream with no sink, which is what makes an assembler's
    /// inputs look like the only thing the base ever spends.
    Consume { item: ItemId, qty: u32 },
}

/// Where something that reached the player's pack came from.
///
/// The whole point of `Record::Acquire`: B5 asks what share of a sector's
/// Core Fragments came from a Mining Node against kills, base rock and
/// caches, and a record with no source answers none of it.
///
/// An enum rather than a `&str` at each of the eighteen call sites, for
/// `MachineStatus::as_str`'s reason: the wire strings are written once, a
/// mistyped tag cannot silently create a nineteenth source, and the match
/// is exhaustive so a new variant has to be given a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootSource {
    /// A fight's spoils: gear, a work resource, a Stack boss's fragments.
    Kill,
    /// Cut out of base space by hand.
    Rock,
    /// A cache cracked open — a nest's or the Stack's.
    Cache,
    /// A contract's reward.
    Contract,
    /// Bought: the caravan, or the Stack's market.
    Trade,
    /// Given back — a demolished structure's materials.
    Refund,
    /// A routine disk the player made, by etching a blank or by extracting
    /// one from a program.
    Etch,
}

impl LootSource {
    pub fn as_str(self) -> &'static str {
        match self {
            LootSource::Kill => "kill",
            LootSource::Rock => "rock",
            LootSource::Cache => "cache",
            LootSource::Contract => "contract",
            LootSource::Trade => "trade",
            LootSource::Refund => "refund",
            LootSource::Etch => "etch",
        }
    }
}

/// Why units left the run.
///
/// `LootSource`'s mirror, and its reason: "the ledger's produced side does
/// not balance" is only actionable if the sinks are told apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumeSource {
    /// Burnt to keep a supplier on the grid — `StructureDef::power_upkeep`.
    Fuel,
    /// Built into a structure, at the tick it was raised. Materials are not
    /// spent until then: they stand on the cell, refundable, until the site
    /// is despawned.
    Build,
    /// Taken off the base's shelves for a job it set itself — the dig crew's
    /// tile, a sortie's outfitting.
    Base,
    /// Spent making something by hand: a hand-compile's ingredients, or
    /// the blank a routine was burnt onto.
    Craft,
    /// A routine disk written into a slot. It buys the routine and refunds
    /// nothing, which is what makes it a sink rather than a move.
    Install,
    /// Destroyed by a breach. Core Fragments and Portal Fragments do not
    /// cross, which is a sink nothing else in the ledger could see.
    Breach,
}

impl ConsumeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ConsumeSource::Fuel => "fuel",
            ConsumeSource::Build => "build",
            ConsumeSource::Base => "base",
            ConsumeSource::Craft => "craft",
            ConsumeSource::Install => "install",
            ConsumeSource::Breach => "breach",
        }
    }
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
            Event::Consume { item, qty } => {
                self.lifetime.entry(item.clone()).or_default().consumed += qty;
                if let Some(bucket) = self.buckets.back_mut() {
                    *bucket.consumed.entry(item.clone()).or_default() += qty;
                }
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

/// The two instrumentation resources, bundled so a seam takes one system
/// parameter rather than two.
///
/// Bevy injects one parameter per resource and `task_progress_system` was
/// already at six, which is what makes this a bundle rather than two more
/// arguments — the same reason `CronjobLookups` beside it is one.
#[derive(SystemParam)]
pub struct Instruments<'w> {
    pub ledger: ResMut<'w, BaseLedger>,
    pub telemetry: ResMut<'w, BattleTelemetry>,
}

impl Instruments<'_> {
    /// [`emit`], with the two resources already in hand.
    pub fn emit(
        &mut self,
        tick: u64,
        zone: u32,
        event: &Event,
        record: impl FnOnce(&Event) -> Record,
    ) {
        emit(
            &mut self.ledger,
            &mut self.telemetry,
            tick,
            zone,
            event,
            record,
        );
    }

    /// [`record_in_system`], likewise.
    pub fn record(&mut self, record: impl FnOnce() -> Record) {
        record_in_system(&mut self.telemetry, record);
    }
}

/// The one door a production seam reports through.
///
/// **The counter is a reader of the event, not a sibling of it.** The
/// tempting shortcut is to increment the ledger at the seam and separately
/// build a record; that is two copies of one rule, and the copy that drifts
/// would be the player's screen quietly disagreeing with the analysis a
/// retune was done from.
///
/// The fold is unconditional and the record is lazy, which is the split the
/// two halves actually need: folding costs a `BTreeMap` increment per
/// production *cycle*, while a record costs several `String` allocations
/// that the disarmed case must not pay. `Game::record`'s closure discipline,
/// reachable from a bevy system — those have no `&Game` to hand it.
pub(crate) fn emit(
    ledger: &mut BaseLedger,
    telemetry: &mut BattleTelemetry,
    tick: u64,
    zone: u32,
    event: &Event,
    record: impl FnOnce(&Event) -> Record,
) {
    ledger.fold(tick, zone, event);
    if !telemetry.on {
        return;
    }
    let record = record(event);
    telemetry.records.push(record);
}

/// The same discipline for an event the ledger has nothing to count: a
/// machine's status transition is news for the log and moves no units.
///
/// Deliberately not folded through [`Event`] with an empty arm — an event
/// variant no consumer reads is ceremony, and the ledger gains a term for
/// stalls only when it gains busy/idle ticks to put it beside.
pub(crate) fn record_in_system(telemetry: &mut BattleTelemetry, record: impl FnOnce() -> Record) {
    if !telemetry.on {
        return;
    }
    let record = record();
    telemetry.records.push(record);
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

    fn extract() -> Event {
        Event::Extract {
            item: item("core_fragment"),
            rolled: 2,
            landed: 2,
            ok: true,
        }
    }

    fn a_record(_: &Event) -> Record {
        Record::Extract {
            tick: 0,
            zone: 1,
            machine: (0, 0),
            kind: "mining_node".to_string(),
            tier: 1,
            worker_species: None,
            item: "core_fragment".to_string(),
            rolled: 2,
            landed: 2,
            ok: true,
        }
    }

    /// The counter half must not be gated on the dev log. A base that only
    /// counted what it produced while `FERAL_DEV_LOG` was set would show the
    /// player an empty screen for every ordinary run.
    #[test]
    fn the_fold_runs_whether_or_not_the_log_is_armed() {
        let mut ledger = BaseLedger::default();
        let mut telemetry = BattleTelemetry::default();

        emit(&mut ledger, &mut telemetry, 0, 1, &extract(), a_record);

        assert_eq!(ledger.lifetime[&item("core_fragment")].mined, 2);
        assert!(telemetry.records.is_empty());
    }

    /// The spec's explicit obligation. Nothing in the compiler keeps a bevy
    /// seam honest about the `on` check the way `Game::record`'s closure
    /// does, so the guarantee needs a test of its own: the closure must not
    /// even run when disarmed.
    #[test]
    fn no_record_is_built_when_the_log_is_disarmed() {
        let mut ledger = BaseLedger::default();
        let mut telemetry = BattleTelemetry::default();

        emit(&mut ledger, &mut telemetry, 0, 1, &extract(), |_| {
            panic!("the record closure ran while telemetry was off")
        });

        record_in_system(&mut telemetry, || {
            panic!("the record closure ran while telemetry was off")
        });
    }

    #[test]
    fn an_armed_log_gets_the_record_and_the_ledger_still_gets_the_fold() {
        let mut ledger = BaseLedger::default();
        let mut telemetry = BattleTelemetry {
            on: true,
            ..BattleTelemetry::default()
        };

        emit(&mut ledger, &mut telemetry, 0, 1, &extract(), a_record);

        assert_eq!(ledger.lifetime[&item("core_fragment")].mined, 2);
        assert_eq!(telemetry.records.len(), 1);
    }

    /// A base record is keyed to a tick and happens while no fight is open,
    /// so `arena`'s re-keying must leave it alone rather than stamping it
    /// with a fight it had nothing to do with.
    #[test]
    fn re_keying_a_fight_leaves_a_base_record_alone() {
        let mut record = a_record(&extract());
        let before = record.clone();
        record.set_fight(99);
        assert_eq!(record, before);
    }
}
