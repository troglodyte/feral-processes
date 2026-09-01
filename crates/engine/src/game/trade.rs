//! Buying and selling at a trader structure, including selling programs
//! off the roster.

use crate::components::Downed;
use crate::*;

impl Game {
    /// `entity`'s trading-post terms (see `StructureDef::trade`), if it's a
    /// structure with any — used both by `sell_item`/`buy_item` and by the
    /// renderer to show prices before the player commits.
    pub fn trade_options(&self, entity: Entity) -> Option<TradeDef> {
        let kind = self.world.get::<Structure>(entity)?.kind.clone();
        let mut trade = self
            .world
            .resource::<StructureDb>()
            .get(&kind)?
            .trade
            .clone()?;
        // Sorted on the clone, never in `StructureDb` — the catalogue is
        // loaded once from `.ron` and read by everything, so reordering it
        // in place would be a global edit to satisfy one screen.
        trade
            .buy
            .sort_by_key(|(item, _)| self.category_sort_key(item));
        Some(trade)
    }

    /// What `structure` pays per unit for `item`: the item's own
    /// `ItemDef::value` (see `Game::item_value`) scaled by that trader's
    /// `sell_rate`. `None` for a structure that doesn't trade.
    ///
    /// Public because the trade screen quotes a per-row price before the
    /// player commits, and used to derive it from `sell_rate` alone — right
    /// only while every item was worth the same. A renderer that works a
    /// price out itself is one retune away from quoting a number the sale
    /// doesn't honour.
    pub fn sell_price(&self, structure: Entity, item: &ItemId) -> Option<u32> {
        Some(self.item_value(item) * self.trade_options(structure)?.sell_rate)
    }

    /// Sells `qty` copies of `item` at fusion `tier` to the trading post
    /// `structure`, crediting the trade currency at `sell_price` per unit.
    /// The trade currency itself can't be sold (trading it for more of the
    /// same thing is meaningless, and would be exploitable if a modded
    /// `sell_rate` was ever above 1). Build salvage *is* sellable — that is
    /// the on-ramp that lets a player convert a doomed zone-local stockpile
    /// into Credits before breaching.
    ///
    /// A fused copy fetches the same unit price as an ordinary one, so
    /// `Game::item_value` is untouched and `ItemDef::value`'s second meaning
    /// — the boss-loot bands `surface_boss_loot` derives from it — is not
    /// disturbed. What the tier decides is which store the copy leaves and
    /// what the shelf gets back.
    pub fn sell_item(&mut self, structure: Entity, copy: GearCopy, qty: u32) -> Result<(), String> {
        let item = copy.item.clone();
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        if qty == 0 {
            return Err("Sell at least 1.".into());
        }
        let currency = self.trade_currency();
        if item == currency {
            let money = self.item_name(&currency);
            return Err(format!("{money} aren't worth trading for more {money}."));
        }
        // A bank is not a good. No menu can currently reach this — the sell
        // list is built from `PlayerStatus::inventory`, which omits banked
        // items — but that filter is a *consequence* of this rule, not a
        // substitute for it: `sell_item` is public API, and a caller naming
        // an `ItemId` directly would otherwise turn a Research Node into a
        // slow Credit press.
        if self.is_banked(&item) {
            return Err(format!("{} can't be traded.", self.item_name(&item)));
        }
        let trade = self
            .trade_options(structure)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let player = self.player_entity();
        let have = self.count_copies(&copy);
        if have == 0 {
            return Err(format!("You don't have any {}.", self.item_name(&item)));
        }
        let taken = have.min(qty);
        let payout = self.item_value(&item) * trade.sell_rate * taken;
        let name = self.item_name(&item).to_string();
        let money = self.item_name(&currency).to_string();
        self.take_copies(&copy, taken);
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(currency, payout);
        self.stock_shelf(structure, &copy, taken);
        self.log(format!("You sell {taken} {name} for {payout} {money}."));
        self.tick();
        Ok(())
    }

    /// The `BuybackLedger` key for `entity`, if it's a structure with a
    /// position — its kind and the tile it stands on. Every public buyback
    /// call resolves an `Entity` through here, so no caller learns how the
    /// ledger is keyed.
    fn shelf_key(&self, entity: Entity) -> Option<resources::ShelfKey> {
        let kind = self.world.get::<Structure>(entity)?.kind.clone();
        let pos = self.world.get::<Position>(entity)?;
        Some((kind, (pos.x, pos.y)))
    }

    /// Adds `qty` of exactly this copy to `structure`'s shelf, merging into
    /// the existing row if there is one.
    fn stock_shelf(&mut self, structure: Entity, copy: &GearCopy, qty: u32) {
        let Some(key) = self.shelf_key(structure) else {
            return;
        };
        let shelf = self
            .world
            .resource_mut::<BuybackLedger>()
            .into_inner()
            .0
            .entry(key)
            .or_default();
        match shelf.iter_mut().find(|(c, _)| c == copy) {
            Some((_, held)) => *held += qty,
            None => shelf.push((copy.clone(), qty)),
        }
    }

    /// Announces what a trader's shelf still held as it comes down, and that
    /// rebuilding on the same tile gets it back. Silent for a trader with an
    /// empty shelf, or one that never traded at all.
    ///
    /// Called from both paths that destroy a structure — `damage_structure`
    /// and `remove_structure`. Wiring it into only one is the split that
    /// already made selling a program announce its detachments while fusing
    /// the same program away went quiet.
    pub(crate) fn announce_lost_shelf(&mut self, structure: Entity) {
        let shelf = self.buyback_options(structure);
        if shelf.is_empty() {
            return;
        }
        // Both tiers are named because a shelved special copy is worth many
        // base copies of work — losing "2 Ablative Plating" and losing "2
        // Overclocked Ablative Plating T2" are different sentences to the
        // player, and this line is the only warning either gets.
        let manifest = shelf
            .iter()
            .map(|row| match row.copy.tier {
                0 => format!("{} {}", row.qty, row.name),
                tier => format!("{} {} T{tier}", row.qty, row.name),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let label = self.entity_label(structure);
        self.log_kind(
            MessageKind::Raid,
            format!(
                "{manifest} left on {label}'s shelf — rebuild on the same footprint to reclaim it."
            ),
        );
    }

    /// What `structure` charges per unit to sell `item` back: what it paid
    /// for it, marked up by `BUYBACK_PRICE_MULTIPLIER`. Floored at 1 the way
    /// `program_payout` is, so neither a modded `sell_rate: 0` nor a modded
    /// `value: 0` can hand goods out for free.
    fn buyback_unit_cost(&self, structure: Entity, item: &ItemId) -> Option<u32> {
        Some((self.sell_price(structure, item)? * tuning::BUYBACK_PRICE_MULTIPLIER).max(1))
    }

    /// Everything `structure` has bought off the player and will sell back,
    /// already priced. Empty for a structure that doesn't trade or has an
    /// untouched shelf — which is every trader until the player sells to one.
    pub fn buyback_options(&self, structure: Entity) -> Vec<BuybackOption> {
        let Some(key) = self.shelf_key(structure) else {
            return Vec::new();
        };
        self.world
            .resource::<BuybackLedger>()
            .0
            .get(&key)
            .map(|shelf| {
                let mut options: Vec<BuybackOption> = shelf
                    .iter()
                    .filter_map(|(copy, qty)| {
                        Some(BuybackOption {
                            name: self.copy_name(copy),
                            copy: copy.clone(),
                            qty: *qty,
                            unit_cost: self.buyback_unit_cost(structure, &copy.item)?,
                        })
                    })
                    .collect();
                options.sort_by_key(|o| {
                    (
                        self.category_sort_key(&o.copy.item),
                        o.copy.rarity,
                        o.copy.tier,
                    )
                });
                options
            })
            .unwrap_or_default()
    }

    /// Buys back `qty` of `item` that the player previously sold to
    /// `structure`, at `buyback_unit_cost` each.
    ///
    /// Separate from `buy_item` rather than folded into it: that list is an
    /// infinite catalogue priced per item, this shelf is finite and drains as
    /// it's bought.
    pub fn buy_back(&mut self, structure: Entity, copy: GearCopy, qty: u32) -> Result<(), String> {
        let item = copy.item.clone();
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        if qty == 0 {
            return Err("Buy back at least 1.".into());
        }
        let unit_cost = self
            .buyback_unit_cost(structure, &item)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let key = self
            .shelf_key(structure)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let shelved = self
            .world
            .resource::<BuybackLedger>()
            .0
            .get(&key)
            .and_then(|shelf| shelf.iter().find(|(c, _)| *c == copy))
            .map(|(_, held)| *held)
            .unwrap_or(0);
        let name = self.item_name(&item).to_string();
        if shelved < qty {
            return Err(format!("They only have {shelved} {name} to sell back."));
        }
        let total_cost = unit_cost * qty;
        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&currency)
            < total_cost
        {
            return Err(format!("Not enough {money} (need {total_cost})."));
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(currency, total_cost);
        self.add_copies(&copy, qty);
        {
            let mut ledger = self.world.resource_mut::<BuybackLedger>();
            if let Some(shelf) = ledger.0.get_mut(&key) {
                if let Some(row) = shelf.iter_mut().find(|(c, _)| *c == copy) {
                    row.1 -= qty;
                }
                shelf.retain(|(_, held)| *held > 0);
                if shelf.is_empty() {
                    ledger.0.remove(&key);
                }
            }
        }
        self.log(format!(
            "You buy back {qty} {name} for {total_cost} {money}."
        ));
        self.tick();
        Ok(())
    }

    /// The divisor `structure` prices programs by, if it buys them at all.
    /// `Some(0)` collapses to `None` rather than dividing by zero.
    pub(crate) fn program_sell_divisor(&self, structure: Entity) -> Option<u32> {
        self.trade_options(structure)?
            .program_sell_divisor
            .filter(|&d| d > 0)
    }

    /// What `structure` would pay for `creature`: a fraction of its
    /// `Stats::power()`, floored at 1 so a sale can never destroy a program
    /// for nothing.
    ///
    /// Tiers the player *bought* with Recompile Kernels are divided back out
    /// first, so a trader pays for what a program is rather than for what was
    /// spent on it. Without that, twelve printable Core Fragments buy a
    /// permanent rise in the sale price and the round trip prints Credits —
    /// which compounds, since each kernel doubles the stats the next one
    /// doubles again. Earned tiers are untouched: a program tamed three zones
    /// down really is worth four times one tamed at the entrance, because
    /// beating it is what the game charges for that.
    ///
    /// The divisor comes from `ZoneLevel::stat_multiplier` rather than
    /// `ZONE_STAT_GROWTH.pow(..)`, so it stays the exact inverse of what
    /// `refactored` multiplied in even if that curve stops being geometric.
    pub(crate) fn program_payout(&self, structure: Entity, creature: Entity) -> Option<u32> {
        let divisor = self.program_sell_divisor(structure)?;
        let power = self.world.get::<Stats>(creature)?.power().max(0) as u32;
        Some((self.earned_power(creature, power) / divisor).max(1))
    }

    /// `power` with every bought zone tier divided back out. Saturating and
    /// floored at 1: a save whose `PurchasedTiers` outruns its `ZonePortal`
    /// is nonsense rather than a crash, and a program is never worth nothing.
    fn earned_power(&self, creature: Entity, power: u32) -> u32 {
        let bought = self.purchased_tiers(creature);
        if bought == 0 {
            return power;
        }
        let tier = self.zone_tier(creature);
        let now = ZoneLevel(tier).stat_multiplier().max(1) as u32;
        let before = ZoneLevel(tier.saturating_sub(bought).max(1))
            .stat_multiplier()
            .max(1) as u32;
        (power * before / now).max(1)
    }

    /// Every tamed program the player could sell at `structure`, priced.
    /// Empty when `structure` doesn't buy programs — renderers draw these
    /// rows verbatim and never work a price out themselves.
    pub fn program_sale_options(&mut self, structure: Entity) -> Vec<ProgramSaleOption> {
        if self.program_sell_divisor(structure).is_none() {
            return Vec::new();
        }
        self.owned_pets()
            .into_iter()
            .filter_map(|pet| {
                Some(ProgramSaleOption {
                    entity: pet.entity,
                    name: pet.name,
                    level: pet.level,
                    power: pet.power,
                    payout: self.program_payout(structure, pet.entity)?,
                    fusions: pet.fusions,
                    rarity: pet.rarity,
                    activity: pet.activity,
                    detaches: self.sale_detachments(pet.entity),
                })
            })
            .collect()
    }

    /// What `creature` is doing right now, as a terse status for any dialog
    /// that lists programs: `"in party"`, the bare name of the structure it
    /// works, `"guarding <structure>"`, `"recovering"`, or `"idle"`.
    ///
    /// A worker reads as the bare structure name and a guard carries the
    /// verb, because that is the only thing distinguishing them. Owned by the
    /// engine rather than assembled per screen: it has to read `TaskKind` and
    /// resolve a structure label, and it was previously duplicated across
    /// three dialogs — each of which called a guard "on a cronjob", since the
    /// field they read was `Task.target` with the kind thrown away.
    pub fn program_activity(&self, creature: Entity) -> String {
        // Ahead of everything else. `bench_or_dissolve`'s Forgiving arm has
        // already pulled the program out of the party and off its post
        // (`detach_from_play`), so every check below would find nothing and
        // report `"idle"` — indistinguishable from a companion nobody has
        // ever posted anywhere. `"recovering"` matches the word the log
        // uses when a Repair Bay finishes the job (`run_repair_bays`: "is
        // back on its feet").
        if self.world.get::<Downed>(creature).is_some() {
            return "recovering".to_string();
        }
        // Ahead of the party check: the two are mutually exclusive today
        // (`wield_program` stands a member down, `add_companion` disarms),
        // but stating the order keeps them from both being reported if that
        // ever stops holding.
        if self.wielded_program() == Some(creature) {
            return "equipped as weapon".to_string();
        }
        if self.world.resource::<Party>().0.contains(&creature) {
            return "in party".to_string();
        }
        // Above the post, and below the party and the wield: a body off shift
        // has no `Task` at all, so this arm is what stops "idle" being said of
        // a program that is very much doing something. `program_errand_label`
        // is the one derivation of the verb — the def's own `servicing`
        // string, never a phrase built here.
        if let Some(errand) = self.program_errand_label(creature) {
            return errand.to_lowercase();
        }
        match self.program_post(creature) {
            Some((TaskKind::GatherResource, target)) => target,
            Some((TaskKind::Guard, target)) => format!("guarding {target}"),
            Some((TaskKind::Excavate, target)) => format!("cutting {target}"),
            Some((TaskKind::Construct, target)) => format!("building {target}"),
            None => "idle".to_string(),
        }
    }

    /// Where `creature` is posted, as `(what the post is, the structure's
    /// label)`, or `None` for a program with no `Task` at all.
    ///
    /// The one read of a `Task` into something a screen can show, so the
    /// manifest's own row and the terse status `program_activity` builds on
    /// top of it cannot disagree about which structure a program is standing
    /// at. It knows nothing about the party or the wield — `add_companion`
    /// clears the task, so a party member simply has none, and the ordering
    /// of those two checks stays stated in `program_activity` rather than
    /// hidden in here.
    pub fn program_post(&self, creature: Entity) -> Option<(TaskKind, String)> {
        let task = self.world.get::<Task>(creature)?;
        Some((task.kind, self.entity_label(task.target)))
    }

    /// What selling `creature` would also cancel, worded for display. Built
    /// here rather than in a renderer because it has to name a structure and
    /// know what a `Task` kind means.
    pub(crate) fn sale_detachments(&self, creature: Entity) -> Vec<String> {
        let mut out = Vec::new();
        if self.wielded_program() == Some(creature) {
            out.push("stops being your weapon".to_string());
        }
        if self.world.resource::<Party>().0.contains(&creature) {
            out.push("leaves your battle party".to_string());
        }
        if let Some(task) = self.world.get::<Task>(creature) {
            let target = self.entity_label(task.target);
            out.push(match task.kind {
                TaskKind::GatherResource => format!("stops working {target}"),
                TaskKind::Guard => format!("stops guarding {target}"),
                TaskKind::Excavate => format!("stops cutting {target}"),
                TaskKind::Construct => format!("stops building {target}"),
            });
        }
        out
    }

    /// Logs `sale_detachments`, then drops `creature` out of the party and
    /// the world for good. The one way a tamed program permanently leaves
    /// play — `sell_companion` and `routines::extract_routine` both end a
    /// program this way and only differ in what they hand back for it, so
    /// this is the single call that keeps the two sequences from drifting
    /// apart the way a doc comment merely claiming to mirror one another
    /// couldn't. Returns the label logged, since both callers still need it
    /// for their own payout line afterward.
    pub(crate) fn dissolve_tamed_program(&mut self, creature: Entity) -> String {
        let name = self.detach_from_play(creature);
        self.world.despawn(creature);
        name
    }

    /// Everything a program leaving play does *before* the world decides
    /// whether it comes back: gear returned, detachments announced, out of
    /// the party, off its post.
    ///
    /// Extracted rather than copied into `bench_or_dissolve`'s Forgiving arm
    /// for `dissolve_tamed_program`'s own reason one level up — a comment
    /// claiming two sequences mirror each other cannot hold them in step,
    /// and the copy that drifts is the one nobody runs.
    fn detach_from_play(&mut self, creature: Entity) -> String {
        // Gear is the player's property; the program was only wearing it.
        // Here rather than at the callers because this is the one function
        // sale, extraction, battle death and a raid defender's death already
        // agree through — a fifth caller inherits it for free.
        self.strip_gear(creature);
        let name = self.creature_label(creature);
        for detached in self.sale_detachments(creature) {
            self.log(format!("{name} {detached}."));
        }
        self.world
            .resource_mut::<Party>()
            .0
            .retain(|&e| e != creature);
        self.world.entity_mut(creature).remove::<Task>();
        name
    }

    /// What becomes of an owned program that dies: destroyed under
    /// Permadeath, benched under Forgiving.
    ///
    /// **One door, not a branch at each site.** Battle teardown and a raid
    /// defender's death are the two ways an owned program can be killed, and
    /// they agree on what a death *means* through this function rather than
    /// through two copies of a difficulty check. Returns the label for the
    /// same reason `dissolve_tamed_program` does — both callers still write
    /// their own line about it afterwards.
    ///
    /// The benched arm leaves the program at 1 HP carrying
    /// `components::Downed`, still `Tamed` and therefore still staff by
    /// derivation. `Game::run_repair_bays` is what clears it, at full HP.
    ///
    /// **No longer the only writer of that marker.**
    /// `Game::admit_the_badly_hurt` inserts it too, for a staff program that
    /// has fallen below `tuning::BAY_ADMISSION_HP_FRACTION` without dying —
    /// so this remains the one door a *death* goes through, which is what
    /// the difficulty branch here is about, and is not the one door onto
    /// `Downed`.
    pub(crate) fn bench_or_dissolve(&mut self, creature: Entity) -> String {
        if *self.world.resource::<DifficultyMode>() == DifficultyMode::Permadeath {
            return self.dissolve_tamed_program(creature);
        }
        let name = self.detach_from_play(creature);
        if let Some(mut stats) = self.world.get_mut::<Stats>(creature) {
            stats.hp = 1;
        }
        self.world.entity_mut(creature).insert(Downed);
        name
    }

    /// Sells `creature` to the trading post `structure` for a share of its
    /// power, despawning it and freeing the roster slot it held — the only
    /// way to shed a tamed program without fusing it into another.
    ///
    /// Whatever the program was doing is cancelled: a party slot, a cronjob,
    /// a guard post. Each is logged, so a structure that stops producing
    /// says so rather than going quiet.
    pub fn sell_companion(&mut self, structure: Entity, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        if self.program_sell_divisor(structure).is_none() {
            return Err("That trader doesn't deal in programs.".into());
        }
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        // Explicit here even though `dissolve_tamed_program` strips too:
        // `program_payout` reads `Stats::power()` and runs *before* the
        // dissolve, so a trader would otherwise pay for gear the player is
        // about to get back. After every reachable refusal, so a refused sale
        // leaves the loadout alone — the `ok_or_else` below cannot fire from
        // here, since the divisor is checked above and a `Tamed` has `Stats`.
        self.strip_gear(creature);
        let payout = self
            .program_payout(structure, creature)
            .ok_or_else(|| "That program can't be appraised.".to_string())?;
        let currency = self.trade_currency();

        let name = self.dissolve_tamed_program(creature);
        let money = self.item_name(&currency).to_string();
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(currency, payout);
        self.log(format!("You sell {name} for {payout} {money}."));
        self.tick();
        Ok(())
    }

    /// Buys `qty` of `item` from the trading post `structure`, at its
    /// listed per-unit cost in the trade currency.
    pub fn buy_item(&mut self, structure: Entity, item: ItemId, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        if qty == 0 {
            return Err("Buy at least 1.".into());
        }
        let trade = self
            .trade_options(structure)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let (_, unit_cost) = trade
            .buy
            .iter()
            .find(|(i, _)| *i == item)
            .ok_or_else(|| format!("{} isn't for sale here.", self.item_name(&item)))?;
        let total_cost = unit_cost * qty;
        let currency = self.trade_currency();
        let money = self.item_name(&currency).to_string();
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&currency)
            < total_cost
        {
            return Err(format!("Not enough {money} (need {total_cost})."));
        }
        let name = self.item_name(&item).to_string();
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(currency, total_cost);
            inv.add(item, qty);
        }
        self.log(format!("You buy {qty} {name} for {total_cost} {money}."));
        self.tick();
        Ok(())
    }
}
