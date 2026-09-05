//! A settlement's shelf: what it stocks, what it charges, and the basket
//! door that spends it.
//!
//! **The shelf is derived off `(WorldMap::seed(), SettlementKey, epoch)`,
//! never stored** — `Game::settlement_shelf`'s whole argument is
//! `placement.rs`'s own: a settlement is a property of the *world*, so what
//! it stocks has to be too, or two visits to the same town on the same
//! epoch could disagree about what is on the shelf. The three prohibitions
//! that module states carry over unchanged: no `resources::GameRng`, a
//! local `StdRng` only, and **never `%`** — reduce through `derive::index`.
//!
//! **Pricing is `Game::marked_unit_cost`/`Game::caravan_unit_cost`'s own
//! formula, scaled by the settlement's `Temperament`** rather than a second
//! formula beside it — see `marked_unit_cost`'s doc for why the craft floor
//! survives a temperament discount. Row identity (`Drawn`, the round-robin
//! gear draw, the standout-share ordinal pick) is `Game::draw_shelf`,
//! shared verbatim with `Game::caravan_shelf`: two vendors drawing rows
//! from the same four pools by the same weighted-bucket mechanism is not a
//! coincidence worth a second copy.
//!
//! **What this module deliberately does not have, yet**: a shelf-slot
//! "already bought" memory. `Game::caravan_shelf` has `CaravanMemory` and
//! the Stack market has `FrameMemory::bought`; a settlement's shelf has
//! nothing that stops the same rolled row being bought twice inside one
//! `SETTLEMENT_MARKET_ROTATION_TICKS` epoch, so a row can be taken as many
//! times as the purse allows until the shelf rotates. That is a real gap
//! and a known one, **deferred to Phase 4** by decision: the shape it wants
//! is `CaravanMemory`'s — a `(SettlementKey, epoch)` keyed set of spent row
//! indices, written where `apply_buys` delivers and filtered out of
//! `settlement_view`'s offer list, additive behind `#[serde(default)]` and
//! so costing no `SAVE_FORMAT_VERSION` bump. It is not an economy exploit
//! in the meantime — buying at `marked_unit_cost` scaled by `buy_mult`
//! and selling back at `SETTLEMENT_SELL_RATE * sell_mult` loses on every
//! round trip — it is an absence of scarcity, which is a Phase 4 concern
//! anyway, since standing is what is meant to gate a town's better rows.

use crate::game::caravan::Drawn;
use crate::game::commerce;
use crate::game::contracts::fold;
use crate::settlements::{SettlementKey, SettlementKind, Specialty, Temperament};
use crate::*;

impl Game {
    /// Which epoch the settlement market's clock is in right now — one
    /// epoch per `tuning::SETTLEMENT_MARKET_ROTATION_TICKS`, `Game::visit_index`'s
    /// shape for a vendor with no single visit to key off.
    pub(crate) fn settlement_epoch(&self) -> u64 {
        self.current_tick() / crate::tuning::SETTLEMENT_MARKET_ROTATION_TICKS
    }

    /// Whether the party is close enough to `key` to trade with it — the
    /// gate `Game::commit_settlement_basket`, `Game::settlement_view` and
    /// `Game::settlement_buy_back` all share.
    ///
    /// **Chebyshev adjacency, not equality.** Walking onto a settlement's
    /// own tile is refused — `find_settlement_at` queues a visit instead of
    /// admitting the player, the same as a wall — so the player's `Position`
    /// can never equal `KnownSettlement::tile`; it can only ever be one of
    /// its eight neighbours, however they approached.
    ///
    /// **The space guard is the first line, not an omission.** A `Position`
    /// is a *surface* tile only while `Locale::Open`: in base space it is a
    /// base-grid cell and underground it is the pinned entrance tile, and
    /// either can land within a tile of a town's coordinates by coincidence
    /// — `find_target_in_direction`'s own settlement arm gates the same way
    /// two files over. Nothing player-facing reaches this today (the market
    /// is only opened from a surface bump or an `x`), but all four callers
    /// are `pub`.
    pub(crate) fn settlement_reach(&self, key: SettlementKey) -> bool {
        if self.in_base() || self.is_underground() {
            return false;
        }
        let Some(known) = self.world.resource::<resources::Settlements>().0.get(&key) else {
            return false;
        };
        let Some(pos) = self.world.get::<Position>(self.player_entity()) else {
            return false;
        };
        (pos.x - known.tile.0).abs() <= 1 && (pos.y - known.tile.1).abs() <= 1
    }

    /// What the settlement `key` names has on its shelf this `epoch` —
    /// `Game::caravan_shelf`'s shape, off `Game::draw_shelf`'s shared core,
    /// with three differences: seeded from the world seed rather than the
    /// base's (a town does not travel), bucket weights biased by
    /// `Specialty` (`specialty_weights`), and row count/standout share
    /// scaled by `SettlementKind` (`settlement_rows`/`settlement_bonus_share`)
    /// rather than read off a per-trader `.ron` def — there is no
    /// `SettlementDef::rows` to read, because a specialty and a kind are
    /// the whole of what a settlement authors about its own shelf.
    ///
    /// `Vec::new()` for a key `resources::Settlements` does not know —
    /// `settlement_at` returning `None` for an empty `SettlementDb`'s own
    /// reason, carried one door over: nothing here may panic on a key that
    /// has not materialized.
    pub(crate) fn settlement_shelf(
        &mut self,
        key: SettlementKey,
        epoch: u64,
    ) -> Vec<views::CaravanOffer> {
        let Some(def) = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.def.clone())
        else {
            return Vec::new();
        };
        let world_seed = self.world.resource::<world::WorldMap>().seed();
        let mut rng = StdRng::seed_from_u64(settlement_shelf_seed(world_seed, key, epoch));
        let weights = specialty_weights(def.specialty);
        let rows = settlement_rows(def.kind);
        let bonus_share = settlement_bonus_share(def.kind);
        let drawn = self.draw_shelf(&mut rng, rows, weights, bonus_share);

        let kinds: Vec<views::CaravanOfferKind> = drawn
            .into_iter()
            .map(|(d, bonus)| match d {
                Drawn::Gear(item) => {
                    views::CaravanOfferKind::Gear(self.roll_shelf_copy(item, &mut rng, bonus))
                }
                Drawn::Routine(ability) => views::CaravanOfferKind::Routine(ability),
                Drawn::Program(species) => views::CaravanOfferKind::Program(species),
                Drawn::Material(item) => views::CaravanOfferKind::Material(item),
            })
            .collect();

        let temperament = def.temperament;
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
                self.settlement_row(index, kind, qty, temperament)
            })
            .collect()
    }

    /// One settlement shelf row, worded and priced — `Game::caravan_row`'s
    /// shape, sharing its `offer_text` and pricing every kind through
    /// `settlement_unit_cost`/`program_price` at `temperament` instead of
    /// `caravan_unit_cost`.
    fn settlement_row(
        &mut self,
        index: usize,
        kind: views::CaravanOfferKind,
        qty: u32,
        temperament: Temperament,
    ) -> views::CaravanOffer {
        let (name, detail) = self.offer_text(&kind);
        let unit_cost = match &kind {
            views::CaravanOfferKind::Gear(copy) => {
                self.settlement_unit_cost(&copy.item, temperament)
            }
            views::CaravanOfferKind::Routine(ability) => {
                self.settlement_unit_cost(&ItemId::etched(ability), temperament)
            }
            views::CaravanOfferKind::Program(species) => {
                let mult = self.world.resource::<ZoneLevel>().stat_multiplier() as f32;
                ((self.program_price(species, mult) as f32 * temperament.buy_mult()).ceil() as u32)
                    .max(1)
            }
            views::CaravanOfferKind::Material(item) => self.settlement_unit_cost(item, temperament),
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

    /// What a settlement charges for one of `item` at `temperament` —
    /// `Game::marked_unit_cost` at the temperament's `buy_mult`. The craft
    /// floor survives every temperament because `marked_unit_cost` applies
    /// it *after* `mult`, never before.
    pub(crate) fn settlement_unit_cost(&self, item: &ItemId, temperament: Temperament) -> u32 {
        self.marked_unit_cost(item, temperament.buy_mult())
    }

    /// What a settlement pays you for one of `item` at `temperament`:
    /// `Game::item_value` at `tuning::SETTLEMENT_SELL_RATE`, scaled by
    /// `temperament`'s `sell_mult`.
    ///
    /// No craft floor on this side — the floor exists to stop a craftable
    /// being *bought* under its ingredients' worth, and nothing about what
    /// a settlement pays you can open that loop.
    pub(crate) fn settlement_sell_price(&self, item: &ItemId, temperament: Temperament) -> u32 {
        (self.item_value(item) as f32
            * crate::tuning::SETTLEMENT_SELL_RATE as f32
            * temperament.sell_mult())
        .round() as u32
    }

    /// A settlement's own counter — see `Game::caravan_view`'s contract,
    /// carried over: `None` answers both "is anyone trading here" and
    /// spares every caller a second reach check.
    pub fn settlement_view(&mut self, key: SettlementKey) -> Option<views::SettlementMarketView> {
        if !self.settlement_reach(key) {
            return None;
        }
        let temperament = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&key)?
            .def
            .temperament;
        let epoch = self.settlement_epoch();
        let mut offers = self.settlement_shelf(key, epoch);
        // Grouped for the eye here and never in `settlement_shelf`, the
        // caravan screen's own rule: the shelf's round-robin lead would
        // stop rotating if sorted in place.
        offers.sort_by_key(|offer| (self.caravan_group(&offer.kind).0, offer.index));
        let currency = self.trade_currency();
        let credits = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&currency))
            .unwrap_or(0);
        Some(views::SettlementMarketView {
            offers,
            sells: self.settlement_sell_rows(temperament),
            credits,
            currency: self.item_name(&currency).to_string(),
        })
    }

    /// Every stack of the player's cargo the settlement `temperament`
    /// belongs to will take, priced at `settlement_sell_price` —
    /// `Game::caravan_sell_rows`' shape, one door over.
    fn settlement_sell_rows(&self, temperament: Temperament) -> Vec<views::CaravanSellRow> {
        let currency = self.trade_currency();
        self.player_status()
            .inventory
            .iter()
            .filter(|row| row.copy.item != currency && !self.is_banked(&row.copy.item))
            .map(|row| views::CaravanSellRow {
                name: self.item_name(&row.copy.item).to_string(),
                unit_price: self.settlement_sell_price(&row.copy.item, temperament),
                copy: row.copy.clone(),
                held: row.qty,
            })
            .collect()
    }

    /// Commits a whole basket at the settlement `key` names: `sells` first,
    /// then the shelf rows named by `buys` — `Game::commit_caravan_basket`'s
    /// shape exactly, sharing its validation (`refuse_sale_offer`,
    /// `refuse_offer_delivery`) and its delivery (`deliver_offer`,
    /// `charge_for_offer`), through the same `Game::settle_basket` core.
    ///
    /// **The one thing this function must get right**: `apply_buys`'
    /// closure charges *exactly* `offer.unit_cost * offer.qty`, the same
    /// expression `cost` was accumulated from below. `settle_basket` checks
    /// the sum against the player's funds; it does not — cannot — check
    /// that what gets charged per line is what was quoted. Nothing in the
    /// compiler holds that either, which is why it has its own test.
    pub fn commit_settlement_basket(
        &mut self,
        key: SettlementKey,
        sells: Vec<(GearCopy, u32)>,
        buys: Vec<usize>,
    ) -> Result<String, String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.settlement_reach(key) {
            return Err("There's nobody trading here.".into());
        }
        if sells.is_empty() && buys.is_empty() {
            return Err("Nothing in the basket.".into());
        }
        let temperament = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&key)
            .expect("settlement_reach confirmed a materialized settlement")
            .def
            .temperament;

        // ---- every question, before anything moves ----
        let mut planned_sells = Vec::new();
        let mut proceeds = 0u32;
        for (copy, qty) in sells {
            let taken = self.refuse_sale_offer(&copy, qty)?;
            proceeds += self.settlement_sell_price(&copy.item, temperament) * taken;
            planned_sells.push((copy, taken));
        }

        let offers = self
            .settlement_view(key)
            .ok_or_else(|| "There's nobody selling anything here.".to_string())?
            .offers;
        let mut planned_buys = Vec::new();
        let mut cost = 0u32;
        let mut room = self.roster_room();
        for index in buys {
            let offer = offers
                .iter()
                .find(|o| o.index == index)
                .cloned()
                .ok_or_else(|| "That's not on the shelf.".to_string())?;
            // Counted down across the basket, not re-read per row —
            // `commit_caravan_basket`'s reason, one vendor over: two
            // programs asked one at a time would both pass against a
            // roster with one slot left.
            self.refuse_offer_delivery(&offer, room)?;
            if matches!(offer.kind, views::CaravanOfferKind::Program(_)) {
                room -= 1;
            }
            cost += offer.unit_cost * offer.qty;
            planned_buys.push(offer);
        }

        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        let sold = planned_sells.len();
        let bought = planned_buys.len();

        self.settle_basket(commerce::BasketPlan {
            proceeds,
            cost,
            sold,
            bought,
            apply_sells: move |game: &mut Game| {
                for (copy, taken) in planned_sells {
                    game.apply_settlement_sale(key, &copy, taken, temperament);
                }
            },
            apply_buys: move |game: &mut Game| {
                for offer in planned_buys {
                    let price = offer.unit_cost * offer.qty;
                    let delivered = game.deliver_offer(&offer);
                    game.charge_for_offer(price, &money, delivered);
                }
            },
        })
    }

    /// The sale itself — infallible behind `refuse_sale_offer`, priced
    /// through `settlement_sell_price` and, unlike a caravan's
    /// `apply_caravan_sale`, **stocking the settlement's own buyback
    /// shelf**: the one way selling to a settlement differs from selling to
    /// a caravan, which keeps no shelf at all because it rolls away.
    fn apply_settlement_sale(
        &mut self,
        key: SettlementKey,
        copy: &GearCopy,
        taken: u32,
        temperament: Temperament,
    ) -> u32 {
        let currency = self.trade_currency();
        let payout = self.settlement_sell_price(&copy.item, temperament) * taken;
        let name = self.item_name(&copy.item).to_string();
        let money = self.item_name(&currency).to_string();
        self.take_copies(copy, taken);
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(currency, payout);
        if let Some(shelf_key) = self.settlement_shelf_key(key) {
            self.stock_shelf(shelf_key, copy, taken);
        }
        self.log(format!("You sell {taken} {name} for {payout} {money}."));
        payout
    }
}

/// What a settlement's own `Specialty` leans its shelf toward —
/// `caravans::CaravanWeights` reused rather than a parallel type, since
/// `Game::draw_shelf` already takes one and the shape is exactly what a
/// specialty needs to express: a bias on one of the same four buckets.
///
/// Exhaustive on `Specialty`, `Specialty`'s own rule: a fifth specialty
/// with no bucket to lean on is one that reads as broken rather than as
/// neutral.
fn specialty_weights(specialty: Specialty) -> crate::caravans::CaravanWeights {
    let mut weights = crate::caravans::CaravanWeights {
        gear: crate::tuning::SETTLEMENT_BASE_WEIGHT,
        routines: crate::tuning::SETTLEMENT_BASE_WEIGHT,
        programs: crate::tuning::SETTLEMENT_BASE_WEIGHT,
        materials: crate::tuning::SETTLEMENT_BASE_WEIGHT,
    };
    let bonus = crate::tuning::SETTLEMENT_SPECIALTY_WEIGHT_BONUS;
    match specialty {
        Specialty::Gear => weights.gear += bonus,
        Specialty::Materials => weights.materials += bonus,
        Specialty::Routines => weights.routines += bonus,
        Specialty::Programs => weights.programs += bonus,
    }
    weights
}

/// How many shelf rows `kind` draws — `tuning::SETTLEMENT_SERVER_ROWS` /
/// `SETTLEMENT_MAINFRAME_ROWS`, `SettlementKind`'s own "a Server is a stop,
/// a Mainframe is a destination" read onto the shelf.
fn settlement_rows(kind: SettlementKind) -> u32 {
    match kind {
        SettlementKind::Server => crate::tuning::SETTLEMENT_SERVER_ROWS,
        SettlementKind::Mainframe => crate::tuning::SETTLEMENT_MAINFRAME_ROWS,
    }
}

/// What share of `kind`'s gear rows are standout stock —
/// `tuning::SETTLEMENT_SERVER_BONUS_SHARE` / `SETTLEMENT_MAINFRAME_BONUS_SHARE`,
/// `bonus_row_count`'s `share` argument.
fn settlement_bonus_share(kind: SettlementKind) -> u32 {
    match kind {
        SettlementKind::Server => crate::tuning::SETTLEMENT_SERVER_BONUS_SHARE,
        SettlementKind::Mainframe => crate::tuning::SETTLEMENT_MAINFRAME_BONUS_SHARE,
    }
}

/// The seed `Game::settlement_shelf` draws from: the market's own salt, the
/// world seed, the region key and the epoch, folded FNV-1a a byte at a
/// time — `Game::visit_seed`'s shape, one vendor over.
///
/// Byte-at-a-time for `region_seed`'s measured reason (`placement.rs`): a
/// whole-word XOR leaves the low output bits a fixed function of the input,
/// and consecutive epochs (like consecutive regions) differ in exactly one
/// low bit.
fn settlement_shelf_seed(world_seed: u32, key: SettlementKey, epoch: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for word in [
        crate::tuning::SETTLEMENT_MARKET_SALT,
        world_seed as u64,
        key.rx as i64 as u64,
        key.ry as i64 as u64,
        epoch,
    ] {
        h = fold(h, &word.to_le_bytes());
    }
    h
}
