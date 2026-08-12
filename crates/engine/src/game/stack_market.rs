//! The stalls a Stack frame stands, and everything one sells.
//!
//! A market is the sixth thing in a frame that can be used up, and it is
//! built the way the other five are (`game/stack_features.rs`): a `CellKind`
//! in the generated frame, and a record in `resources::FrameMemory` saying
//! what has been spent. What differs is the granularity — a cache is
//! emptied whole, where a market is emptied a row at a time, so the record
//! is `FrameMemory::bought` keyed by row rather than a set of cells.
//!
//! **The shelf is derived, never stored.** `market_offers` is a pure
//! function of `FrameSpec`, drawn from a local `StdRng` salted off
//! `FrameSpec::rng_seed` exactly as `Game::orphan_species` is and for the
//! same forced reason: the player is shown a price before they pay it, so
//! the answer has to survive a save and load, and `GameRng`'s stream
//! position is not persisted. Drawing from `GameRng` would also shift every
//! later roll in the run merely because somebody opened a shop screen.
//!
//! **A trader down here remembers nothing.** There is no `BuybackLedger`
//! entry and no shelf key: what the player sells is gone, and what they buy
//! is gone from the frame. Once every row has been bought the cell reads as
//! plain floor in both views, like an emptied cache — which is the whole of
//! what makes these markets ephemeral rather than a shop the party can walk
//! back to.
//!
//! **Everything sold here is a disk, never a slot.** A routine row hands
//! over etched Routine Disks and asks nothing about who they are for; the
//! player answers that later, through `Game::install_disk`, wherever they
//! happen to be. That is what makes a trader one of the two doors an
//! *exclusive* routine can come through — the other being a boss — since an
//! exclusive routine has no etch path and can only ever exist as a disk
//! somebody already wrote.

use super::stack::StackPos;
use crate::abilities::AbilityDb;
use crate::components::Inventory;
use crate::resources::StackMemory;
use crate::stack::CellKind;
use crate::tuning::{
    STACK_MARKET_EXCLUSIVE_CHANCE_BASE, STACK_MARKET_EXCLUSIVE_CHANCE_PER_DEPTH,
    STACK_MARKET_EXCLUSIVE_PRICE, STACK_MARKET_PROGRAM_CHANCE,
    STACK_MARKET_PROGRAM_PRICE_PER_POWER, STACK_MARKET_ROUTINE_OFFERS, STACK_MARKET_SELL_RATE,
};
use crate::views::{MarketOffer, MarketOfferKind, MarketSellRow, RoutineScope, StackMarketView};
use crate::*;
use std::collections::BTreeSet;

/// Salts `FrameSpec::rng_seed` for the shelf, so which routines are on sale
/// does not correlate with the shape of the junction they are sold from.
/// One scheme, per `FrameSpec::salted`'s doc — not a second seed source.
const MARKET_SALT: u64 = 0x5701_5A1E;

impl Game {
    /// The shelf of the market the party is standing on, or `None` if there
    /// is no live one under them.
    ///
    /// One call answers both "is there a stall here" and "what is on it",
    /// so no screen has to ask those separately and then disagree about the
    /// answer.
    pub fn stack_market(&mut self) -> Option<StackMarketView> {
        let pos = self.stack_pos()?;
        if self.cell_underfoot() != Some(CellKind::Market) || !self.market_live(pos) {
            return None;
        }
        let currency = self.trade_currency();
        let credits = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&currency))
            .unwrap_or(0);
        let spent = self.market_spent(pos);
        let offers = self
            .market_offers(pos)
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !spent.contains(index))
            .map(|(index, kind)| self.offer_row(index, kind, credits))
            .collect();
        Some(StackMarketView {
            offers,
            sells: self.market_sell_rows(),
            credits,
            currency: self.item_name(&currency).to_string(),
        })
    }

    /// One shelf row, worded. Split out so the two things a row needs from
    /// the rest of the engine — an ability's authored description, a
    /// species' name and what it would spawn at — are resolved in one place
    /// rather than inside the list comprehension that builds the shelf.
    fn offer_row(&mut self, index: usize, kind: MarketOfferKind, credits: u32) -> MarketOffer {
        let (name, detail, price) = match &kind {
            MarketOfferKind::Routine { ability, scope } => (
                format!(
                    "{} ×{} — {}",
                    self.ability_display_name(ability),
                    scope.disks(),
                    scope.label()
                ),
                self.world
                    .resource::<AbilityDb>()
                    .get(ability)
                    .map(|def| def.description.clone())
                    .unwrap_or_default(),
                scope.price(),
            ),
            MarketOfferKind::ExclusiveDisk { ability } => (
                // Named as the disk rather than as the routine, because
                // that is what changes hands and what will be sitting in
                // cargo afterwards. The ordinary rows above can say the
                // routine's name plainly; this one is the only shelf row
                // that is a *thing*.
                self.item_name(&ItemId::etched(ability)).to_string(),
                self.world
                    .resource::<AbilityDb>()
                    .get(ability)
                    .map(|def| format!("{} Nobody can write this one.", def.description))
                    .unwrap_or_default(),
                STACK_MARKET_EXCLUSIVE_PRICE,
            ),
            MarketOfferKind::Program { species } => {
                let def = self.world.resource::<SpeciesDb>().get(species).cloned();
                let name = def
                    .as_ref()
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| species.clone());
                // Its stats as it would spawn here, which is also what it is
                // priced off — a shelf that quoted base stats and delivered
                // depth-scaled ones would be quoting a different program.
                let detail = match &def {
                    Some(def) => {
                        let mult = crate::stack::depth_stat_multiplier(self.stack_depth());
                        let at = |base: i32| (base as f32 * mult).round() as i32;
                        format!(
                            "{} HP, {} ATK, {} DEF — compiled to your control",
                            at(def.base_hp),
                            at(def.base_atk),
                            at(def.base_def)
                        )
                    }
                    None => String::new(),
                };
                (name, detail, self.market_program_price(species))
            }
        };
        MarketOffer {
            index,
            kind,
            name,
            detail,
            price,
            affordable: credits >= price,
        }
    }

    /// What this frame's market has on it, bought rows included — the
    /// derived shelf `FrameMemory::bought` indexes into, and the one
    /// definition of a market's shape. `market_live` counts these rather
    /// than working the length out from the constants, because a second
    /// expression of "how big is a shelf" is a copy that drifts.
    ///
    /// Two routines at all three bundle sizes, then a program on some
    /// markets and not others, then — rarely, and more often the deeper the
    /// frame — one exclusive routine's disk.
    ///
    /// The ordinary routine pool deliberately **excludes everything in
    /// `AbilityDb::wild_pool`**: those are the hunt-only routines, reachable
    /// exactly one way — off a wild carrier — and a shop selling them is
    /// precisely the "just target the species" shortcut that boundary exists
    /// to break. See `no_species_or_research_file_grants_a_wild_only_ability`,
    /// which makes the same demand of species and research files.
    ///
    /// It excludes `AbilityDb::exclusive_pool` for the opposite reason: those
    /// are not cheaper or rarer versions of an ordinary row, they are the one
    /// thing on the shelf that cannot be etched at home, and listing one at
    /// six copies for a party would make it exactly as ordinary as Throttle.
    ///
    /// The program is drawn from the **whole non-boss roster**, not from the
    /// entrance biome's `habitat_pools` the way `orphan_species` is, and the
    /// difference is the point: an orphan is a property of the place — it
    /// was left running *here* — where stock is a property of the trader,
    /// who carried it in from somewhere else. It also keeps this derivation
    /// `&self`, which is what lets both Stack views ask whether a stall is
    /// still worth drawing without a mutable borrow.
    ///
    /// Bosses are excluded for `orphan_species`' reason: a fight the party
    /// did not see coming is refused a boss, and a boss bought off a shelf
    /// is a stronger version of the same objection.
    pub(crate) fn market_offers(&self, pos: StackPos) -> Vec<MarketOfferKind> {
        let spec = self.frame_spec(pos.depth, pos.frames, pos.entrance);
        let mut rng = StdRng::seed_from_u64(spec.rng_seed() ^ MARKET_SALT);

        // `AbilityDb::all` and `SpeciesDb::all` both iterate id-sorted, so a
        // shelf is a function of the seed rather than of however the asset
        // files happened to load — the same guarantee `open_cache`'s drop
        // table makes by sorting its own table.
        let db = self.world.resource::<AbilityDb>();
        let hunt_only: Vec<&str> = db
            .wild_pool()
            .into_iter()
            .map(|(def, _)| def.id.as_str())
            .collect();
        let mut pool: Vec<String> = db
            .all()
            .filter(|def| !hunt_only.contains(&def.id.as_str()) && !def.exclusive)
            .map(|def| def.id.clone())
            .collect();

        let mut offers = Vec::new();
        for _ in 0..STACK_MARKET_ROUTINE_OFFERS.min(pool.len()) {
            let ability = pool.swap_remove(rng.random_range(0..pool.len()));
            for scope in RoutineScope::ALL {
                offers.push(MarketOfferKind::Routine {
                    ability: ability.clone(),
                    scope,
                });
            }
        }
        // Rolled after the routines so the two are independent: a market
        // with no program on it still lists the routines it would have.
        if rng.random_bool(STACK_MARKET_PROGRAM_CHANCE) {
            let roster: Vec<&str> = self
                .world
                .resource::<SpeciesDb>()
                .all()
                .filter(|def| !def.is_boss)
                .map(|def| def.id.as_str())
                .collect();
            if !roster.is_empty() {
                offers.push(MarketOfferKind::Program {
                    species: roster[rng.random_range(0..roster.len())].to_string(),
                });
            }
        }
        // And the rare row, rolled last for the same reason the program row
        // is rolled after the routines: appending a draw at the end shifts
        // no earlier one, so a market that had no exclusive disk before this
        // existed still has the same routines and the same program on it.
        //
        // The chance climbs with depth, which is what stops these being
        // farmed off depth-1 frames a short walk from the breach.
        let exclusive_chance = (STACK_MARKET_EXCLUSIVE_CHANCE_BASE
            + STACK_MARKET_EXCLUSIVE_CHANCE_PER_DEPTH * pos.depth as f64)
            .clamp(0.0, 1.0);
        if rng.random_bool(exclusive_chance) {
            let pool: Vec<String> = self
                .world
                .resource::<AbilityDb>()
                .exclusive_pool()
                .into_iter()
                .map(|def| def.id.clone())
                .collect();
            if !pool.is_empty() {
                offers.push(MarketOfferKind::ExclusiveDisk {
                    ability: pool[rng.random_range(0..pool.len())].clone(),
                });
            }
        }
        offers
    }

    /// What a market charges for `species`: its power as the species would
    /// spawn at this depth, at `STACK_MARKET_PROGRAM_PRICE_PER_POWER` a
    /// point.
    ///
    /// Priced off `stack::depth_stat_multiplier` rather than
    /// `Game::stack_depth_multiplier`, which is the same figure with the
    /// party's Trace folded in. A quote that rose because the party had been
    /// noisy would be a price that changed while the player was looking at
    /// it, for a reason nothing on the screen explains. What the party
    /// actually *gets* is scaled by Trace like every other program spawned
    /// down here — see `buy_market_offer`.
    fn market_program_price(&self, species: &str) -> u32 {
        let power = self
            .world
            .resource::<SpeciesDb>()
            .get(species)
            .map(|def| (def.base_hp + def.base_atk + def.base_def).max(0) as u32)
            .unwrap_or(0);
        let scaled = power as f32 * crate::stack::depth_stat_multiplier(self.stack_depth());
        (scaled.round() as u32 * STACK_MARKET_PROGRAM_PRICE_PER_POWER).max(1)
    }

    /// How deep the party is, or 1 on the surface — the depth a quote is
    /// worked out at. Named rather than inlined twice, because the price
    /// and the stat line under it must be the same program.
    fn stack_depth(&self) -> u32 {
        self.stack_pos().map(|pos| pos.depth).unwrap_or(1)
    }

    /// Every stack of cargo a market will take, priced at
    /// `STACK_MARKET_SELL_RATE` a point of `ItemDef::value`.
    ///
    /// The same two exclusions `Game::sell_item` makes, and for the same
    /// reasons: the trade currency (buying Credits with Credits is
    /// meaningless) and anything banked (a bank is not a good, and a
    /// Research Node would otherwise be a slow Credit press).
    fn market_sell_rows(&mut self) -> Vec<MarketSellRow> {
        let currency = self.trade_currency();
        self.player_status()
            .inventory
            .iter()
            .filter(|row| row.item != currency && !self.is_banked(&row.item))
            .map(|row| MarketSellRow {
                name: self.item_name(&row.item).to_string(),
                item: row.item.clone(),
                tier: row.tier,
                qty: row.qty,
                unit_price: self.market_unit_price(&row.item),
            })
            .collect()
    }

    /// What a market pays for one copy of `item`. Through `Game::item_value`
    /// like every other price in the game, so a retune moves this with it.
    fn market_unit_price(&self, item: &ItemId) -> u32 {
        self.item_value(item) * STACK_MARKET_SELL_RATE
    }

    /// Which rows of this frame's market have already been bought.
    fn market_spent(&self, pos: StackPos) -> BTreeSet<usize> {
        self.world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .map(|m| m.bought.clone())
            .unwrap_or_default()
    }

    /// Whether this frame's market still has anything on it — what both
    /// views use to stop advertising a stall that has been bought out,
    /// exactly as `cache_unopened` does for an emptied cache.
    ///
    /// A bought-out stall stops buying too, which is why this gates
    /// `sell_to_market` as well as the two views. That is a judgement about
    /// what "ephemeral" means rather than a constraint: a trader you can
    /// still sell to forever is a permanent shop in the frame, and the
    /// frame regenerates, so it would be a permanent shop for the run.
    pub(crate) fn market_live(&self, pos: StackPos) -> bool {
        self.market_spent(pos).len() < self.market_offers(pos).len()
    }

    /// Buys row `index` off the market the party is standing on.
    ///
    /// A routine row hands over etched disks and asks nothing about who they
    /// are for — that question is deferred to `Game::install_disk`, whenever
    /// the player gets round to it. Selling disks rather than writing slots
    /// is what deleted the target picker, the "nobody has a free slot"
    /// refusal, and the two helpers that decided who a purchase reached.
    ///
    /// **Every refusal lands before anything is spent**, which is this
    /// function's whole ordering and the reason it is worth reading:
    /// `adopt_orphan` and `etch_disk` make the same promise, and a purchase
    /// that took the Credits and then failed would be the one bug a player
    /// cannot undo — there is no buyback here.
    pub fn buy_market_offer(&mut self, index: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(pos) = self.stack_pos() else {
            return Err("There's nobody selling anything here.".into());
        };
        if self.cell_underfoot() != Some(CellKind::Market) || !self.market_live(pos) {
            return Err("There's nobody selling anything here.".into());
        }
        if self.market_spent(pos).contains(&index) {
            return Err("That's already been sold.".into());
        }
        let kind = self
            .market_offers(pos)
            .into_iter()
            .nth(index)
            .ok_or_else(|| "There's nothing like that on the shelf.".to_string())?;

        let price = match &kind {
            MarketOfferKind::Routine { scope, .. } => scope.price(),
            MarketOfferKind::ExclusiveDisk { .. } => STACK_MARKET_EXCLUSIVE_PRICE,
            MarketOfferKind::Program { species } => self.market_program_price(species),
        };
        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        let player = self.player_entity();
        let held = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&currency))
            .unwrap_or(0);
        if held < price {
            return Err(format!("Not enough {money} (need {price})."));
        }

        // The last two refusals, and then the goods — the Credits are taken
        // *after*, which is `install_routine`'s ordering and for its reason:
        // nothing may be spent on a path that can still fail. Everything
        // below the two `?`s is infallible, so there is no state in which
        // the player has paid and holds nothing, and none in which they hold
        // it and have not paid.
        let delivered = match &kind {
            MarketOfferKind::Routine { ability, scope } => {
                let qty = scope.disks();
                let disk = ItemId::etched(ability);
                let name = self.item_name(&disk).to_string();
                self.grant_loot(disk, qty);
                format!("{qty} × {name} goes in the pack")
            }
            MarketOfferKind::ExclusiveDisk { ability } => {
                let disk = ItemId::etched(ability);
                let name = self.item_name(&disk).to_string();
                self.grant_loot(disk, 1);
                format!("{name} is yours — the only one you'll see on this shelf")
            }
            MarketOfferKind::Program { species } => {
                if self.pet_count() >= self.pet_capacity() {
                    return Err("Your roster is full.".into());
                }
                // The one thing a market's own price does not describe:
                // what the party gets is scaled by Trace as well as depth,
                // like every other program spawned down here. See
                // `market_program_price`.
                let depth_mult = self.stack_depth_multiplier();
                let (ex, ey) = pos.entrance;
                let program = self
                    .adopt_program(species, ex, ey, depth_mult)
                    .ok_or_else(|| "There's nothing left of it to compile.".to_string())?;
                format!("{} is yours", self.creature_label(program))
            }
        };

        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(currency, price);
        self.frame_memory_mut(pos).bought.insert(index);
        self.log_kind(
            MessageKind::Outcome,
            format!("{price} {money}, and {delivered}."),
        );
        self.tick();
        Ok(())
    }

    /// Sells `qty` copies of `item` at fusion `tier` to the market the party
    /// is standing on.
    ///
    /// The Stack half of `Game::sell_item`, and deliberately not a second
    /// caller of it: that one is keyed on a trading-post `Entity`, is gated
    /// by `require_surface`, and stocks a `BuybackLedger` shelf. A trader
    /// down here has none of those — no entity, no surface, and no memory of
    /// the transaction. What the two do share is the one thing that must not
    /// drift, `Game::item_value`, which both reach through.
    pub fn sell_to_market(&mut self, item: ItemId, tier: u32, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(pos) = self.stack_pos() else {
            return Err("There's nobody buying anything here.".into());
        };
        if self.cell_underfoot() != Some(CellKind::Market) || !self.market_live(pos) {
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
        let have = self.count_copies(&item, tier);
        if have == 0 {
            return Err(format!("You don't have any {}.", self.item_name(&item)));
        }
        let taken = have.min(qty);
        let payout = self.market_unit_price(&item) * taken;
        let name = self.item_name(&item).to_string();
        let money = self.item_name(&currency).to_string();
        self.take_copies(&item, tier, taken);
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(currency, payout);
        self.log(format!(
            "You sell {taken} {name} for {payout} {money}. It goes straight into the stack."
        ));
        self.tick();
        Ok(())
    }
}
