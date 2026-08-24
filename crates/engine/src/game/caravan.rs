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

/// Salts the visit seed for the shelf, so what a trader carries does not
/// correlate with which direction it walked in from. One scheme, per
/// `FrameSpec::salted`'s doc — not a second seed source.
const SHELF_SALT: u64 = 0x5_4E1F;

impl Game {
    /// What the visiting trader has on it, bought rows included — the
    /// derived shelf `resources::CaravanMemory` indexes into.
    ///
    /// **Derived, never stored**, for `Game::market_offers`' forced reason:
    /// the player is shown a price before they pay it, so the answer has to
    /// survive a save and load, and `GameRng`'s stream position is not
    /// persisted. A `GameRng` draw would also shift every later roll in the
    /// run merely because somebody opened a shop screen.
    ///
    /// `&mut self` rather than `&self` only because the row wording reaches
    /// `Game::copy_name` and `Game::copy_bonus`; the shelf itself is a pure
    /// function of `visit`.
    pub(crate) fn caravan_shelf(&mut self, visit: &CaravanVisit) -> Vec<views::CaravanOffer> {
        let Some(def) = self
            .world
            .resource::<crate::caravans::CaravanDb>()
            .get(&visit.def_id)
            .cloned()
        else {
            return Vec::new();
        };
        let mut rng = StdRng::seed_from_u64(self.visit_seed(visit.visit) ^ SHELF_SALT);
        let gear = self.stock_pool(|d| d.equipment.is_some());
        let routines = self.routine_disk_pool();
        // Every other piece of cargo, craftable or not — a caravan pulls up
        // beside a base and the base wants feedstock, so salvage it would
        // otherwise have to go and mine belongs here. That width is what
        // makes `stock_pool`'s currency exclusion the only thing standing
        // between a shelf and a Portal Fragment.
        let materials =
            self.stock_pool(|d| d.equipment.is_none() && d.id.etched_ability().is_none());
        let programs: Vec<String> = self
            .world
            .resource::<SpeciesDb>()
            .all()
            .filter(|d| !d.is_boss)
            .map(|d| d.id.clone())
            .collect();

        let mut kinds = Vec::new();
        for _ in 0..def.rows {
            // The weights are re-read every row rather than a pool being
            // shuffled once, so a trader with nothing in one category still
            // fills its shelf out of the others instead of coming up short.
            let mut buckets: Vec<(u32, u8)> = Vec::new();
            if !gear.is_empty() {
                buckets.push((def.weights.gear, 0));
            }
            if !routines.is_empty() {
                buckets.push((def.weights.routines, 1));
            }
            if !programs.is_empty() {
                buckets.push((def.weights.programs, 2));
            }
            if !materials.is_empty() {
                buckets.push((def.weights.materials, 3));
            }
            let total: u32 = buckets.iter().map(|(w, _)| w).sum();
            if total == 0 {
                break;
            }
            let mut roll = rng.random_range(0..total);
            let bucket = buckets
                .iter()
                .find(|(w, _)| match roll.checked_sub(*w) {
                    Some(rest) => {
                        roll = rest;
                        false
                    }
                    None => true,
                })
                .map(|(_, b)| *b)
                .unwrap_or(3);
            kinds.push(match bucket {
                0 => {
                    let item = gear[rng.random_range(0..gear.len())].clone();
                    views::CaravanOfferKind::Gear(self.roll_shelf_copy(item, &mut rng))
                }
                1 => views::CaravanOfferKind::Routine(
                    routines[rng.random_range(0..routines.len())].clone(),
                ),
                2 => views::CaravanOfferKind::Program(
                    programs[rng.random_range(0..programs.len())].clone(),
                ),
                _ => views::CaravanOfferKind::Material(
                    materials[rng.random_range(0..materials.len())].clone(),
                ),
            });
        }

        kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let qty = match &kind {
                    views::CaravanOfferKind::Material(_) => {
                        rng.random_range(1..=crate::tuning::CARAVAN_MATERIAL_STACK)
                    }
                    _ => 1,
                };
                self.caravan_row(index, kind, qty)
            })
            .collect()
    }

    /// The items a caravan may stock, id-sorted, filtered by `keep`.
    ///
    /// **Nothing carrying an `EconomyRole` is ever stockable**, and that one
    /// clause is what keeps Portal Fragments off every shelf at every sector
    /// however a def's weights are set. It is the data-driven form of the
    /// rule `assets/structures/black_market.ron` states by hand: progression
    /// is earned by fighting and descending, and a currency sold over a
    /// counter is the back door onto it. A banked item is excluded for
    /// `market_sell_rows`' reason — a bank is not cargo.
    fn stock_pool(&self, keep: impl Fn(&crate::items_db::ItemDef) -> bool) -> Vec<ItemId> {
        self.world
            .resource::<ItemDb>()
            .all()
            .filter(|d| d.role.is_none() && !d.banked)
            .filter(|d| keep(d))
            .map(|d| d.id.clone())
            .collect()
    }

    /// The ability ids a caravan may sell disks of.
    ///
    /// The same two exclusions `Game::market_offers` makes and for the same
    /// reasons: `AbilityDb::wild_pool` is hunt-only and a shop selling one is
    /// the "just target the species" shortcut that boundary exists to break,
    /// and `exclusive_pool` is the one thing that cannot be etched at home —
    /// a caravan is convenience, and convenience must not be the way past
    /// either boundary.
    fn routine_disk_pool(&self) -> Vec<String> {
        let db = self.world.resource::<AbilityDb>();
        let hunt_only: Vec<&str> = db
            .wild_pool()
            .into_iter()
            .map(|(def, _)| def.id.as_str())
            .collect();
        db.all()
            .filter(|def| !hunt_only.contains(&def.id.as_str()) && !def.exclusive)
            .map(|def| def.id.clone())
            .collect()
    }

    /// One shelf copy, rolled off the shelf's own stream rather than
    /// `GameRng`.
    ///
    /// The three axes are the same three `Game::grant_gear_drop` rolls and
    /// they go through the same functions — `spawning::rarity_for_roll`,
    /// `combat_rewards::pick_affix` and `spawning::quality_for_luck` — so a
    /// retune of any of them moves a caravan's stock with the rest of the
    /// game. A copy here is priced off the item, not off what it rolled; see
    /// `caravan_unit_cost`.
    fn roll_shelf_copy(&self, item: ItemId, rng: &mut StdRng) -> GearCopy {
        let rarity = crate::game::spawning::rarity_for_roll(rng.random_range(0.0..1.0));
        let affix = self
            .equipment_of(&item)
            .map(|(slot, _)| {
                let pool: Vec<(AffixId, u32)> = self
                    .world
                    .resource::<AffixDb>()
                    .pool_for(slot)
                    .into_iter()
                    .map(|def| (def.id.clone(), def.weight))
                    .collect();
                crate::game::combat_rewards::pick_affix(&pool, rng)
            })
            .unwrap_or_default();
        let quality = crate::game::spawning::quality_for_luck(
            crate::tuning::QUALITY_DROP_BASE,
            rng.random_range(0..=crate::game::spawning::quality_luck_steps()),
        );
        GearCopy::with_affixes(item, rarity, 0, affix.into_iter().collect(), quality)
    }

    /// One shelf row, worded and priced.
    fn caravan_row(
        &mut self,
        index: usize,
        kind: views::CaravanOfferKind,
        qty: u32,
    ) -> views::CaravanOffer {
        let (name, detail, unit_cost) = match &kind {
            views::CaravanOfferKind::Gear(copy) => (
                self.copy_name(copy),
                // The item's authored line, not a stat block: what a copy is
                // worth is the wearer's question and `[I]` answers it through
                // `Game::gear_detail`, which scales to whoever is holding it.
                // A figure quoted here would be scaled to nobody.
                self.item_description(&copy.item)
                    .unwrap_or_default()
                    .to_string(),
                self.caravan_unit_cost(&copy.item),
            ),
            views::CaravanOfferKind::Routine(ability) => {
                let disk = ItemId::etched(ability);
                (
                    self.item_name(&disk).to_string(),
                    self.world
                        .resource::<AbilityDb>()
                        .get(ability)
                        .map(|def| def.description.clone())
                        .unwrap_or_default(),
                    self.caravan_unit_cost(&disk),
                )
            }
            views::CaravanOfferKind::Program(species) => {
                let def = self.world.resource::<SpeciesDb>().get(species).cloned();
                let mult = self.world.resource::<ZoneLevel>().stat_multiplier() as f32;
                let name = def
                    .as_ref()
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| species.clone());
                let detail = match &def {
                    Some(d) => {
                        let at = |base: i32| (base as f32 * mult).round() as i32;
                        format!(
                            "{} HP, {} ATK, {} DEF — compiled to your control",
                            at(d.base_hp),
                            at(d.base_atk),
                            at(d.base_mitigation)
                        )
                    }
                    None => String::new(),
                };
                (name, detail, self.program_price(species, mult))
            }
            views::CaravanOfferKind::Material(item) => (
                self.item_name(item).to_string(),
                self.item_description(item).unwrap_or_default().to_string(),
                self.caravan_unit_cost(item),
            ),
        };
        views::CaravanOffer {
            index,
            kind,
            name,
            detail,
            unit_cost,
            qty,
        }
    }

    /// What a caravan charges for one of `item`.
    ///
    /// `Game::item_value` at `CARAVAN_MARKUP`, scaled by the sector, and then
    /// floored **strictly above** what the recipe's ingredients are worth.
    ///
    /// The markup is the whole product: everything a caravan sells is
    /// compilable at a bench or findable in the Stack, so a trader that
    /// undercut either would make both pointless. The craft floor is the
    /// second bound and the non-obvious one — a craftable sold for less than
    /// its ingredients is an infinite Credit loop through the nearest
    /// counter, the same fault `every_craftable_is_worth_more_than_its_parts`
    /// holds shut on the item set itself. Read off `ItemDef::craftable`
    /// rather than `Game::craft_recipes`, so `Perk::LeanCompiler` cannot buy
    /// its way under the floor.
    pub(crate) fn caravan_unit_cost(&self, item: &ItemId) -> u32 {
        let zone = self.world.resource::<ZoneLevel>().stat_multiplier().max(1) as u32;
        let marked = (self.item_value(item) as f32 * crate::tuning::CARAVAN_MARKUP).ceil() as u32;
        let parts: u32 = self
            .world
            .resource::<ItemDb>()
            .get(item.as_str())
            .and_then(|def| def.craftable.as_ref())
            .map(|c| {
                c.cost
                    .iter()
                    .map(|(ingredient, qty)| self.item_value(ingredient) * qty)
                    .sum()
            })
            .unwrap_or(0);
        (marked * zone).max(parts * zone + 1).max(1)
    }
}
