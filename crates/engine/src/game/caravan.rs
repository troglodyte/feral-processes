//! Periodic caravan traders: when one is due, who it is, and what it sells.
//!
//! **The schedule and the shelf are derived; the journey is entity state.**
//! Those are two different questions and keeping them apart is what stops
//! them becoming two sources of one truth. When a trader is due, which
//! trader it is and what is on its shelf are all a fold of the base's own
//! seed and the visit index — so they survive a reload with no save field,
//! spend no `GameRng` draw and so shift nobody's stream, cannot be rerolled
//! by save-scumming, and rotate on their own. Where it is standing right now
//! and which of five stages it is in are properties of the moment and are
//! saved, exactly as `DigSite` is.
//!
//! That is `Game::contract_board`'s argument reached from the base's side
//! rather than the sector's, and the seed is the difference: a board is a
//! property of the *sector* and re-derives on a breach, while a caravan's
//! rhythm belongs to the base and travels with it.

use crate::base_grid::BaseGrid;
use crate::game::contracts::fold;
use crate::*;

/// One scheduled visit — everything about it that is derived rather than
/// lived. Cheap to rebuild, and rebuilt rather than stored for that reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaravanVisit {
    /// Which interval since the run began this visit belongs to.
    pub visit: u64,
    /// The `CaravanDef` walking in.
    pub def_id: String,
    /// The tick it appears on the sector surface.
    pub arrival_tick: u64,
    /// The tick it packs up, whether or not it ever reached the Market.
    pub depart_tick: u64,
    /// Which of the eight compass directions it walks in from, as an index
    /// into `BEARINGS`.
    pub bearing: u8,
}

/// The eight directions a caravan may walk in from, in a fixed order — the
/// derived bearing is an index into this, so reordering it moves every
/// caravan in every existing save.
pub(crate) const BEARINGS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

impl Game {
    /// Which visit interval the clock is in now.
    pub(crate) fn visit_index(&self) -> u64 {
        self.current_tick() / crate::tuning::CARAVAN_VISIT_INTERVAL_TICKS
    }

    /// The seed one visit's three independent draws are taken from: the
    /// base's own seed, the caravan salt and the visit index, folded FNV-1a a
    /// byte at a time.
    ///
    /// **`BaseGrid::seed()`, not `WorldMap::seed()`.** The base's seed is
    /// minted once at `Game::new` and travels across a breach; the world's is
    /// re-minted per zone. A rhythm keyed to the world seed would restart
    /// every time the party moved on, which is the opposite of what a
    /// recurring visitor to *your base* means.
    ///
    /// Byte-at-a-time rather than one XOR-and-multiply per word, for
    /// `FrameSpec::salted`'s measured reason: a whole-word XOR leaves the low
    /// output bits a fixed function of the input, and consecutive visit
    /// indices differ in exactly one low bit.
    pub(crate) fn visit_seed(&self, visit: u64) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for word in [
            self.world.resource::<BaseGrid>().seed() as u64,
            crate::tuning::CARAVAN_SALT,
            visit,
        ] {
            h = fold(h, &word.to_le_bytes());
        }
        h
    }

    /// The visit that is *currently open*, or `None`.
    ///
    /// `None` in three cases that mean different things to the code and the
    /// same thing to the player — no trading structure standing, no trader
    /// whose window covers this sector, and a clock outside the visit's own
    /// arrival-to-departure window — because none of them is something the
    /// player can do anything about from here. Whether the caravan has
    /// *arrived* is a separate question and is `components::Caravan`'s.
    ///
    /// Readable from anywhere, underground included: it makes no claim about
    /// where the party is standing, so there is nothing for distance to
    /// invalidate. Same argument as `Game::contract_board`'s.
    pub(crate) fn scheduled_visit(&self) -> Option<CaravanVisit> {
        if !self.has_trading_structure() {
            return None;
        }
        let visit = self.visit_index();
        let candidate = self.visit_at(visit)?;
        let now = self.current_tick();
        (now >= candidate.arrival_tick && now < candidate.depart_tick).then_some(candidate)
    }

    /// The visit belonging to interval `visit`, whether or not the clock is
    /// inside its window. Split out so the schedule is testable across many
    /// intervals without winding the clock, and so `caravan_tick` can ask
    /// about the visit an already-standing caravan belongs to.
    ///
    /// **Each of the three draws re-folds the seed with its own tag** rather
    /// than reading three slices of one number, so adding a fourth draw later
    /// cannot shift the three that exist. Every reduction reads the *high*
    /// bits (`derive::index`), never `%`: a modulo on a fold reads little but
    /// the low bits the final multiply provably never disturbs, which
    /// anti-correlates consecutive visits while looking perfectly reasonable.
    pub(crate) fn visit_at(&self, visit: u64) -> Option<CaravanVisit> {
        let zone = self.world.resource::<ZoneLevel>().0;
        let pool = self
            .world
            .resource::<crate::caravans::CaravanDb>()
            .for_zone(zone);
        if pool.is_empty() {
            return None;
        }
        let seed = self.visit_seed(visit);
        let interval = crate::tuning::CARAVAN_VISIT_INTERVAL_TICKS;
        let jitter = crate::tuning::CARAVAN_ARRIVAL_JITTER_TICKS;
        let offset = crate::derive::index(fold(seed, b"arrival"), jitter.max(1) as usize) as u64;
        let def_id = pool[crate::derive::index(fold(seed, b"trader"), pool.len())]
            .id
            .clone();
        let bearing = crate::derive::index(fold(seed, b"bearing"), BEARINGS.len()) as u8;
        let arrival_tick = visit * interval + offset;
        Some(CaravanVisit {
            visit,
            def_id,
            arrival_tick,
            depart_tick: arrival_tick + crate::tuning::CARAVAN_STAY_TICKS,
            bearing,
        })
    }

    /// Whether the base has a counter for a caravan to stand beside at all.
    ///
    /// Asked of `StructureDef::trade` rather than of a hardcoded `"market"`
    /// id, so a mod's own trading post gates a caravan exactly as the iso
    /// Market does — the standing rule that content is data.
    pub(crate) fn has_trading_structure(&self) -> bool {
        self.trading_structures().next().is_some()
    }

    /// Every deployed structure that trades, **sorted by `(x, y)`**.
    ///
    /// Sorted because a base may hold more than one and bevy's query
    /// iteration order is not stable: a caravan that docked at a different
    /// Market between two loads of one save would be reporting iteration
    /// order rather than reporting the base. Same trap `assembler_system`
    /// sorts machines to avoid.
    pub(crate) fn trading_structures(&self) -> impl Iterator<Item = (Entity, Position)> {
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(Entity, Position)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                db.get(kind)?.trade.as_ref()?;
                Some((e.id(), *e.get::<Position>()?))
            })
            .collect();
        found.sort_by_key(|(e, p)| (p.x, p.y, *e));
        found.into_iter()
    }
}
