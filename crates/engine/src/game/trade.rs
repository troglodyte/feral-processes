//! Buying and selling at a trader structure, including selling programs
//! off the roster.

use crate::*;

impl Game {
    /// `entity`'s trading-post terms (see `StructureDef::trade`), if it's a
    /// structure with any — used both by `sell_item`/`buy_item` and by the
    /// renderer to show prices before the player commits.
    pub fn trade_options(&self, entity: Entity) -> Option<TradeDef> {
        let kind = self.world.get::<Structure>(entity)?.kind.clone();
        self.world
            .resource::<StructureDb>()
            .get(&kind)?
            .trade
            .clone()
    }

    /// Sells `qty` of `item` from inventory to the trading post `structure`,
    /// crediting Core Fragments at its flat `sell_rate` per unit. Core
    /// Fragments themselves can't be sold (trading them for more of the
    /// same thing is meaningless, and would be exploitable if a modded
    /// `sell_rate` was ever above 1).
    pub fn sell_item(&mut self, structure: Entity, item: ItemId, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if qty == 0 {
            return Err("Sell at least 1.".into());
        }
        // Same reasoning as `erase_item`: a routine can be one-of-a-kind
        // (nothing else grants decompile), so refusing to sell it for a
        // single Core Fragment costs the player nothing next to what losing
        // it for good would.
        if self.is_routine(&item) {
            return Err(
                "That's a routine, not scrap — install it on a program instead of selling it."
                    .into(),
            );
        }
        let currency = self.currency();
        if item == currency {
            return Err("Core Fragments aren't worth trading for more Core Fragments.".into());
        }
        let trade = self
            .trade_options(structure)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let player = self.player_entity();
        let have = self.world.get::<Inventory>(player).unwrap().count(&item);
        if have == 0 {
            return Err(format!("You don't have any {}.", self.item_name(&item)));
        }
        let taken = have.min(qty);
        let payout = trade.sell_rate * taken;
        // Refuse rather than clamp: the item is already gone once `take`
        // runs, so checking room only after taking would let a refusal
        // destroy the sold item for nothing.
        self.check_room(&currency, payout)?;
        let name = self.item_name(&item).to_string();
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(item, taken);
            inv.add(currency, payout);
        }
        self.log(format!(
            "You sell {taken} {name} for {payout} Core Fragments."
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
    pub(crate) fn program_payout(&self, structure: Entity, creature: Entity) -> Option<u32> {
        let divisor = self.program_sell_divisor(structure)?;
        let power = self.world.get::<Stats>(creature)?.power().max(0) as u32;
        Some((power / divisor).max(1))
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
                    activity: pet.activity,
                    detaches: self.sale_detachments(pet.entity),
                })
            })
            .collect()
    }

    /// What `creature` is doing right now, as a terse status for any dialog
    /// that lists programs: `"in party"`, the bare name of the structure it
    /// works, `"guarding <structure>"`, or `"idle"`.
    ///
    /// A worker reads as the bare structure name and a guard carries the
    /// verb, because that is the only thing distinguishing them. Owned by the
    /// engine rather than assembled per screen: it has to read `TaskKind` and
    /// resolve a structure label, and it was previously duplicated across
    /// three dialogs — each of which called a guard "on a cronjob", since the
    /// field they read was `Task.target` with the kind thrown away.
    pub fn program_activity(&self, creature: Entity) -> String {
        if self.world.resource::<Party>().0.contains(&creature) {
            return "in party".to_string();
        }
        match self.world.get::<Task>(creature) {
            Some(task) => {
                let target = self.entity_label(task.target);
                match task.kind {
                    TaskKind::GatherResource => target,
                    TaskKind::Guard => format!("guarding {target}"),
                }
            }
            None => "idle".to_string(),
        }
    }

    /// What selling `creature` would also cancel, worded for display. Built
    /// here rather than in a renderer because it has to name a structure and
    /// know what a `Task` kind means.
    pub(crate) fn sale_detachments(&self, creature: Entity) -> Vec<String> {
        let mut out = Vec::new();
        if self.world.resource::<Party>().0.contains(&creature) {
            out.push("leaves your battle party".to_string());
        }
        if let Some(task) = self.world.get::<Task>(creature) {
            let target = self.entity_label(task.target);
            out.push(match task.kind {
                TaskKind::GatherResource => format!("stops working {target}"),
                TaskKind::Guard => format!("stops guarding {target}"),
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
        let name = self.creature_label(creature);
        for detached in self.sale_detachments(creature) {
            self.log(format!("{name} {detached}."));
        }
        self.world
            .resource_mut::<Party>()
            .0
            .retain(|&e| e != creature);
        self.world.entity_mut(creature).remove::<Task>();
        self.world.despawn(creature);
        name
    }

    /// Sells `creature` to the trading post `structure` for a share of its
    /// power, despawning it and freeing the roster slot it held — the only
    /// way to shed a tamed program without fusing it into another.
    ///
    /// Whatever the program was doing is cancelled: a party slot, a cronjob,
    /// a guard post. Each is logged, so a structure that stops producing
    /// says so rather than going quiet.
    ///
    /// The payout is checked for room *before* the program is destroyed, for
    /// the reason `sell_item` documents about its own ordering: the currency
    /// can be bank-limited, and discovering that after despawning would eat
    /// the program for nothing.
    pub fn sell_companion(&mut self, structure: Entity, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
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
        let payout = self
            .program_payout(structure, creature)
            .ok_or_else(|| "That program can't be appraised.".to_string())?;
        let currency = self.currency();
        self.check_room(&currency, payout)?;

        let name = self.dissolve_tamed_program(creature);
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(currency, payout);
        self.log(format!("You sell {name} for {payout} Core Fragments."));
        self.tick();
        Ok(())
    }

    /// Buys `qty` of `item` from the trading post `structure`, at its
    /// listed per-unit Core Fragment cost.
    pub fn buy_item(&mut self, structure: Entity, item: ItemId, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
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
        let currency = self.currency();
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&currency)
            < total_cost
        {
            return Err(format!("Not enough Core Fragments (need {total_cost})."));
        }
        self.check_room(&item, qty)?;
        let name = self.item_name(&item).to_string();
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(currency, total_cost);
            inv.add(item, qty);
        }
        self.log(format!(
            "You buy {qty} {name} for {total_cost} Core Fragments."
        ));
        self.tick();
        Ok(())
    }
}
