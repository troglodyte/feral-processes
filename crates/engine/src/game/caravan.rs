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

impl Game {
    /// Walks the visiting caravan one step of its journey — spawning it when
    /// its visit opens, moving it in, standing it beside the Market, and
    /// walking it back out when the visit closes.
    ///
    /// Runs on every tick regardless of where the party is. There is
    /// deliberately **no locale special case**: a caravan is a property of
    /// the base and the base keeps running while the party is four frames
    /// down, exactly as the production chains do. That is also why nothing
    /// here reads the player's `Position` — the tile it walks to is the
    /// anchor's, which is a fixture of the sector.
    pub(crate) fn caravan_tick(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let standing: Option<(Entity, Caravan)> = {
            let mut query = self.world.query::<(Entity, &Caravan)>();
            query.iter(&self.world).map(|(e, c)| (e, c.clone())).next()
        };
        let open = self.scheduled_visit();

        let Some((entity, caravan)) = standing else {
            // Nothing standing: a visit that is open and has not been walked
            // yet is one to start. The `visit` check is what stops a trader
            // that gave up, or was seen off by a breach, from reappearing
            // for the rest of its own window.
            if let Some(visit) = open {
                self.spawn_caravan(&visit);
            }
            return;
        };

        // The visit is over, or the counter it came for has come down. Both
        // send it home, and neither is a stall: a caravan that leaves early
        // has done what it could.
        //
        // One condition covers both, and deliberately: `scheduled_visit`
        // already answers `None` with no counter standing, so a second
        // `has_trading_structure()` clause here would be a redundant reading
        // of the same fact — and the redundant half is the one that would
        // rot. It is read as "the *open* visit is no longer mine" rather than
        // off the clock, so a caravan somehow left standing across a visit
        // boundary still packs up.
        let done = open.as_ref().is_none_or(|v| v.visit != caravan.visit);
        if done && !matches!(caravan.stage, CaravanStage::Leaving) {
            self.send_caravan_home(entity);
            return;
        }

        match caravan.stage {
            CaravanStage::Approaching => self.walk_caravan_in(entity, &caravan),
            CaravanStage::Docking => self.phase_caravan_in(entity),
            CaravanStage::Crossing => self.walk_caravan_to_counter(entity, &caravan),
            CaravanStage::Docked => self.age_caravan(entity),
            CaravanStage::Leaving => self.walk_caravan_out(entity, &caravan),
        }
    }

    /// Puts a trader on the sector surface, `CARAVAN_SPAWN_DISTANCE_TILES`
    /// from the anchor on the visit's own bearing.
    ///
    /// A caravan with nowhere to walk to — no anchor in the sector — is not
    /// spawned at all rather than spawned stuck, since the whole journey is
    /// defined against that tile.
    fn spawn_caravan(&mut self, visit: &CaravanVisit) {
        let Some(anchor) = self.anchor_position() else {
            return;
        };
        let Some(def) = self
            .world
            .resource::<crate::caravans::CaravanDb>()
            .get(&visit.def_id)
            .cloned()
        else {
            return;
        };
        let (dx, dy) = BEARINGS[visit.bearing as usize % BEARINGS.len()];
        let reach = crate::tuning::CARAVAN_SPAWN_DISTANCE_TILES;
        let tile = (anchor.0 + dx * reach, anchor.1 + dy * reach);
        self.world.spawn((
            Caravan {
                stage: CaravanStage::Approaching,
                visit: visit.visit,
                arrival_tile: tile,
                stage_ticks: 0,
                announced_stuck: false,
            },
            Position {
                x: tile.0,
                y: tile.1,
            },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
        ));
        self.log(format!(
            "{} rolls in out of the sector, heading for the anchor.",
            def.name
        ));
    }

    /// One step of a surface leg, downhill along a `walk_field` rooted at
    /// `target`. `Ok(true)` on arrival, `Ok(false)` mid-walk, `Err(())` when
    /// no route exists at all.
    ///
    /// **`walk_field` and not a second walk**: it is the one Dijkstra search
    /// on the surface and takes its step rule as a parameter for exactly this
    /// — a caravan crosses the ground a `Pursuing` guardian is refused, since
    /// it is walking *to* the base rather than hunting somebody on it.
    fn step_caravan(&mut self, entity: Entity, target: (i32, i32)) -> Result<bool, ()> {
        let Some(pos) = self.world.get::<Position>(entity).copied() else {
            return Err(());
        };
        if (pos.x, pos.y) == target {
            return Ok(true);
        }
        let radius = crate::tuning::CARAVAN_SPAWN_DISTANCE_TILES + CARAVAN_PATH_MARGIN;
        let field = {
            let mut map = self.world.resource_mut::<crate::world::WorldMap>();
            crate::game::pursuit::walk_field(target, radius, |(x, y)| map.tile(x, y).walkable)
        };
        let Some(&here) = field.get(&(pos.x, pos.y)) else {
            return Err(());
        };
        let next = crate::world::NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (pos.x + dx, pos.y + dy))
            .filter_map(|n| field.get(&n).map(|&cost| (cost, n.0, n.1)))
            .min()
            .filter(|&(cost, ..)| cost < here);
        match next {
            Some((_, x, y)) => {
                if let Some(mut at) = self.world.get_mut::<Position>(entity) {
                    *at = Position { x, y };
                }
                Ok((x, y) == target)
            }
            None => Err(()),
        }
    }

    fn walk_caravan_in(&mut self, entity: Entity, caravan: &Caravan) {
        let Some(anchor) = self.anchor_position() else {
            self.give_up(entity, caravan);
            return;
        };
        match self.step_caravan(entity, anchor) {
            Ok(true) => self.set_caravan_stage(entity, CaravanStage::Docking),
            Ok(false) => self.age_caravan(entity),
            // No route across the sector at all — walled in, or the anchor
            // is enclosed. It gives up and goes, which is the honest outcome:
            // there is nothing the player can do about a trader that never
            // arrived, so the visit is simply a miss. Said once by
            // construction, since the entity is gone straight after.
            Err(()) => self.give_up(entity, caravan),
        }
    }

    /// The anchor tile to base space's own door cell, in one tick — the same
    /// step the party takes through `Game::enter_base`, and the reason
    /// `Docking` is a stage rather than an instant: the trader is drawn
    /// standing on the anchor for one tick before it goes out of phase.
    fn phase_caravan_in(&mut self, entity: Entity) {
        let (x, y) = crate::game::base_space::BASE_EXIT_CELL;
        if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
            *pos = Position { x, y };
        }
        self.set_caravan_stage(entity, CaravanStage::Crossing);
    }

    fn walk_caravan_to_counter(&mut self, entity: Entity, caravan: &Caravan) {
        let Some((_, counter)) = self.trading_structures().next() else {
            self.send_caravan_home(entity);
            return;
        };
        let Some(from) = self.world.get::<Position>(entity).copied() else {
            return;
        };
        if crate::game::base::hauling::at_station(from, counter) {
            self.set_caravan_stage(entity, CaravanStage::Docked);
            let name = self.caravan_name(caravan);
            self.log(format!("{name} sets out its stock beside the counter."));
            return;
        }
        let blocked = self.structure_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let step = {
            let grid = self.world.resource::<BaseGrid>();
            crate::game::base::hauling::step_to_post(grid, from, counter, &blocked, pocket_radius)
        };
        match step {
            Ok(Some(next)) => {
                if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
                    *pos = next;
                }
                self.age_caravan(entity);
            }
            Ok(None) => self.age_caravan(entity),
            // Boxed in, or no route from the door: it waits where it landed
            // and leaves at the end of the stay rather than giving up, since
            // the player *can* fix this one by clearing a way through — which
            // is why it is worth saying, and why it is said **once**.
            Err(_) => {
                self.age_caravan(entity);
                if !caravan.announced_stuck {
                    if let Some(mut c) = self.world.get_mut::<Caravan>(entity) {
                        c.announced_stuck = true;
                    }
                    let name = self.caravan_name(caravan);
                    self.log(format!(
                        "{name} can't find a way through to the counter, and waits by the anchor."
                    ));
                }
            }
        }
    }

    fn walk_caravan_out(&mut self, entity: Entity, caravan: &Caravan) {
        match self.step_caravan(entity, caravan.arrival_tile) {
            Ok(true) | Err(()) => {
                let name = self.caravan_name(caravan);
                self.world.despawn(entity);
                self.log(format!("{name} packs up and rolls back out of the sector."));
            }
            Ok(false) => self.age_caravan(entity),
        }
    }

    /// Turns a caravan around wherever it is: back onto the anchor tile if it
    /// was out of phase, and onto `Leaving` either way.
    fn send_caravan_home(&mut self, entity: Entity) {
        let Some(caravan) = self.world.get::<Caravan>(entity).cloned() else {
            return;
        };
        if caravan.stage.in_base_space() {
            // Straight back to the anchor's tile on the surface. Phasing out
            // is not a walk in either direction — `enter_base`/`leave_base`
            // are one step for the party too.
            let anchor = self.anchor_position().unwrap_or(caravan.arrival_tile);
            if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
                *pos = Position {
                    x: anchor.0,
                    y: anchor.1,
                };
            }
        }
        self.set_caravan_stage(entity, CaravanStage::Leaving);
    }

    fn give_up(&mut self, entity: Entity, caravan: &Caravan) {
        let name = self.caravan_name(caravan);
        self.world.despawn(entity);
        self.log(format!("{name} can't find a way in, and turns back."));
    }

    fn set_caravan_stage(&mut self, entity: Entity, stage: CaravanStage) {
        if let Some(mut caravan) = self.world.get_mut::<Caravan>(entity) {
            caravan.stage = stage;
            caravan.stage_ticks = 0;
        }
    }

    fn age_caravan(&mut self, entity: Entity) {
        if let Some(mut caravan) = self.world.get_mut::<Caravan>(entity) {
            caravan.stage_ticks = caravan.stage_ticks.saturating_add(1);
        }
    }

    /// What the log and the map call this trader.
    pub(crate) fn caravan_name(&self, caravan: &Caravan) -> String {
        self.visit_at(caravan.visit)
            .and_then(|v| {
                self.world
                    .resource::<crate::caravans::CaravanDb>()
                    .get(&v.def_id)
                    .map(|d| d.name.clone())
            })
            .unwrap_or_else(|| "The caravan".to_string())
    }
}

/// How far past `CARAVAN_SPAWN_DISTANCE_TILES` a caravan's surface search
/// reaches, so a route that has to go round something still exists.
///
/// Here rather than in `tuning.rs` for `BASE_EXIT_CELL`'s reason: it is not a
/// knob, it is the slack a straight-line spawn distance needs to be walkable
/// at all. `NEST_PATH_SEARCH_MARGIN` is the same idea on the other search.
const CARAVAN_PATH_MARGIN: i32 = 12;

impl Game {
    /// What the examine ray reads out for a caravan: the trader's name and
    /// its authored line.
    ///
    /// `None` for anything that is not one, so the caller does not have to
    /// ask twice — `Game::describe_base_rock`'s shape, and for its reason.
    pub fn caravan_blurb(&self, entity: Entity) -> Option<String> {
        let caravan = self.world.get::<Caravan>(entity)?;
        let visit = self.visit_at(caravan.visit)?;
        let def = self
            .world
            .resource::<crate::caravans::CaravanDb>()
            .get(&visit.def_id)?;
        Some(format!("{}. {}", def.name, def.description))
    }
}

/// Where the player stands in relation to the visiting trader.
///
/// Three states out of one call rather than two booleans, for
/// `BrokerReach`'s reason and `NoPost::BoxedIn`'s before it: the three leave
/// the player different errands — wait for one, walk home, or trade — and two
/// independent predicates would let the base menu's row and the screen's own
/// header disagree about which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaravanReach {
    /// Nothing is visiting, so there is nobody to be away from.
    NoCaravan,
    /// A caravan is visiting but its shelf is not open to the player: it is
    /// still walking in, waiting for a way through to the counter, or set out
    /// and the party is not in the base.
    ///
    /// One state rather than two, unlike `BrokerReach`'s `OffBase`, because
    /// the map already says which: a board is invisible from outside the
    /// base, so its refusal has to carry the errand, while a caravan is drawn
    /// walking in and drawn standing at the counter. The refusal only has to
    /// say "not yet".
    NotDocked,
    /// Docked beside the counter, with the party in the base. Everything is
    /// available.
    AtCaravan,
}

impl Game {
    /// Where the player stands in relation to the visiting trader.
    ///
    /// `AtCaravan` measures **base space**, exactly as `broker_reach` does,
    /// and deliberately not the distance to the caravan's own tile: a docked
    /// caravan is standing on the base's laid floor by construction, so its
    /// tile says nothing the base does not. The walk to the counter is
    /// visibility and flavour, not a gate — a player who watched a trader
    /// pull up should not then have to work out which cell to stand on.
    pub fn caravan_reach(&mut self) -> CaravanReach {
        let docked = {
            let mut query = self.world.query::<&Caravan>();
            match query.iter(&self.world).next() {
                None => return CaravanReach::NoCaravan,
                Some(caravan) => matches!(caravan.stage, CaravanStage::Docked),
            }
        };
        if !docked {
            return CaravanReach::NotDocked;
        }
        match self.base_pos() {
            Some((x, y))
                if self
                    .world
                    .resource::<crate::base_grid::BaseGrid>()
                    .is_floor(x, y) =>
            {
                CaravanReach::AtCaravan
            }
            _ => CaravanReach::NotDocked,
        }
    }
}

impl Game {
    /// The visiting trader's counter, or `None` when there is nothing to
    /// show.
    ///
    /// **One call answers both "is there a trader" and "what is on the
    /// shelf"**, `Game::stack_market`'s contract, so no screen asks those
    /// separately and then disagrees. Whether the player may *take* any of it
    /// is the third question and `caravan_reach`'s.
    pub fn caravan_view(&mut self) -> Option<views::CaravanView> {
        if self.caravan_reach() != CaravanReach::AtCaravan {
            return None;
        }
        let caravan = {
            let mut query = self.world.query::<&Caravan>();
            query.iter(&self.world).next().cloned()?
        };
        let visit = self.visit_at(caravan.visit)?;
        let def = self
            .world
            .resource::<crate::caravans::CaravanDb>()
            .get(&visit.def_id)
            .cloned()?;
        let spent = self.caravan_spent(caravan.visit);
        let currency = self.trade_currency();
        let credits = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&currency))
            .unwrap_or(0);
        let offers = self
            .caravan_shelf(&visit)
            .into_iter()
            .filter(|offer| !spent.contains(&offer.index))
            .collect();
        Some(views::CaravanView {
            trader: def.name,
            description: def.description,
            offers,
            sells: self.caravan_sell_rows(),
            credits,
            currency: self.item_name(&currency).to_string(),
            ticks_left: visit
                .depart_tick
                .saturating_sub(self.current_tick())
                .min(u32::MAX as u64) as u32,
        })
    }

    /// Which of this visit's rows have already been sold.
    ///
    /// Keyed on the visit index rather than reset anywhere: a memory left
    /// over from a previous visit simply stops matching, so next month's
    /// trader can never arrive already sold out.
    pub(crate) fn caravan_spent(&self, visit: u64) -> std::collections::BTreeSet<usize> {
        let memory = self.world.resource::<crate::resources::CaravanMemory>();
        if memory.visit == visit {
            memory.bought.clone()
        } else {
            Default::default()
        }
    }

    /// Every stack of cargo a caravan will take, at the iso Market's own
    /// rate.
    ///
    /// The same two exclusions `Game::sell_item` and `market_sell_rows` make,
    /// and for the same reasons: the trade currency (buying Credits with
    /// Credits is meaningless) and anything banked (a bank is not a good).
    fn caravan_sell_rows(&mut self) -> Vec<views::CaravanSellRow> {
        let currency = self.trade_currency();
        // The rate is fetched once, above the walk: `caravan_sell_price`
        // wants `&mut self` (it resolves the counter's `TradeDef`), which a
        // closure over `self.player_status()` cannot also hold.
        let rate = self.market_sell_rate();
        let rows: Vec<(GearCopy, u32)> = self
            .player_status()
            .inventory
            .iter()
            .map(|row| (row.copy.clone(), row.qty))
            .collect();
        rows.into_iter()
            .filter(|(copy, _)| copy.item != currency && !self.is_banked(&copy.item))
            .map(|(copy, held)| views::CaravanSellRow {
                name: self.item_name(&copy.item).to_string(),
                unit_price: self.item_value(&copy.item) * rate,
                copy,
                held,
            })
            .collect()
    }

    /// The iso Market's own `sell_rate`, or 1 with no counter standing.
    fn market_sell_rate(&mut self) -> u32 {
        self.trading_structures()
            .next()
            .and_then(|(entity, _)| self.trade_options(entity))
            .map(|trade| trade.sell_rate)
            .unwrap_or(1)
    }

    /// What a caravan pays for one copy of `item`.
    ///
    /// The **iso Market's** rate, read off the counter's own `TradeDef`
    /// rather than restated, so retuning the Market moves this with it. A
    /// caravan that paid better than the standing counter would make the
    /// counter pointless; one that paid worse would make a visit a reason to
    /// stay away.
    fn caravan_sell_price(&mut self, item: &ItemId) -> u32 {
        self.item_value(item) * self.market_sell_rate()
    }

    /// Buys row `index` off the visiting caravan's shelf.
    ///
    /// **Every refusal lands before anything is spent**, which is this
    /// function's whole ordering and `buy_market_offer`'s: a purchase that
    /// took the Credits and then failed is the one bug the player cannot
    /// undo, and a caravan has no buyback to put it right with.
    pub fn buy_caravan_offer(&mut self, index: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let view = self
            .caravan_view()
            .ok_or_else(|| "There's nobody selling anything here.".to_string())?;
        let offer = view
            .offers
            .into_iter()
            .find(|o| o.index == index)
            .ok_or_else(|| "That's not on the wagon.".to_string())?;
        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        let player = self.player_entity();
        let held = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&currency))
            .unwrap_or(0);
        let price = offer.unit_cost * offer.qty;
        if held < price {
            return Err(format!("Not enough {money} (need {price})."));
        }

        // Everything below is infallible bar the roster check, which is the
        // one refusal that cannot be asked earlier without duplicating the
        // match — so it sits at the very top of its own arm, still above the
        // charge at the bottom.
        let delivered = match &offer.kind {
            views::CaravanOfferKind::Gear(copy) => {
                self.add_copies(copy, offer.qty);
                format!("{} goes in the pack", self.copy_name(copy))
            }
            views::CaravanOfferKind::Routine(ability) => {
                let disk = ItemId::etched(ability);
                let name = self.item_name(&disk).to_string();
                self.grant_loot(disk, offer.qty);
                format!("{name} goes in the pack")
            }
            views::CaravanOfferKind::Material(item) => {
                let name = self.item_name(item).to_string();
                self.grant_loot(item.clone(), offer.qty);
                format!("{} × {name} goes in the pack", offer.qty)
            }
            views::CaravanOfferKind::Program(species) => {
                if self.pet_count() >= self.pet_capacity() {
                    return Err("Your roster is full.".into());
                }
                let mult = self.world.resource::<ZoneLevel>().stat_multiplier() as f32;
                let anchor = self.anchor_position().unwrap_or((0, 0));
                let program = self
                    .adopt_program(species, anchor.0, anchor.1, mult)
                    .ok_or_else(|| "There's nothing left of it to compile.".to_string())?;
                format!("{} is yours", self.creature_label(program))
            }
        };

        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(currency, price);
        self.spend_caravan_row(index);
        self.log_kind(
            MessageKind::Outcome,
            format!("{price} {money}, and {delivered}."),
        );
        self.tick();
        Ok(())
    }

    /// Records that this visit's row `index` is gone, under the visit the
    /// caravan standing there belongs to.
    fn spend_caravan_row(&mut self, index: usize) {
        let Some(visit) = ({
            let mut query = self.world.query::<&Caravan>();
            query.iter(&self.world).next().map(|c| c.visit)
        }) else {
            return;
        };
        let mut memory = self.world.resource_mut::<crate::resources::CaravanMemory>();
        if memory.visit != visit {
            memory.visit = visit;
            memory.bought.clear();
        }
        memory.bought.insert(index);
    }

    /// Sells `qty` copies of `copy` to the visiting caravan.
    ///
    /// **It stocks no shelf.** What the player sells here is gone, exactly as
    /// at a Stack market and deliberately unlike `Game::sell_item`: a buyback
    /// needs a counter you can walk back to, and a caravan's whole shape is
    /// that it rolls away. `BuybackLedger` is untouched, which is what a test
    /// asserts on rather than on a screen.
    pub fn sell_to_caravan(&mut self, copy: GearCopy, qty: u32) -> Result<(), String> {
        let item = copy.item.clone();
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.caravan_reach() != CaravanReach::AtCaravan {
            return Err("There's nobody buying anything here.".into());
        }
        if qty == 0 {
            return Err("Sell at least 1.".into());
        }
        let currency = self.trade_currency();
        if item == currency {
            let money = self.item_name(&currency);
            return Err(format!("{money} aren't worth trading for more {money}."));
        }
        if self.is_banked(&item) {
            return Err(format!("{} can't be traded.", self.item_name(&item)));
        }
        let have = self.count_copies(&copy);
        if have == 0 {
            return Err(format!("You don't have any {}.", self.item_name(&item)));
        }
        let taken = have.min(qty);
        let payout = self.caravan_sell_price(&item) * taken;
        let name = self.item_name(&item).to_string();
        let money = self.item_name(&currency).to_string();
        self.take_copies(&copy, taken);
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(currency, payout);
        self.log(format!(
            "You sell {taken} {name} for {payout} {money}. It goes onto the wagon and out of the sector."
        ));
        self.tick();
        Ok(())
    }
}
