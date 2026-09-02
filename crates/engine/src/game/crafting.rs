//! Recipes, crafting, and the equipment the results go into: equip,
//! unequip, fuse, and erase.

use crate::tuning::{
    QUALITY_BASE, QUALITY_BENCH_PER_TIER, QUALITY_CAREFUL_BONUS, QUALITY_CAREFUL_COST_PERCENT,
};
use crate::*;

/// Everything about *who is compiling, and where* that decides how good
/// a copy comes out — see `Game::craft_quality_floor`, which is the one
/// expression that turns it into a number.
///
/// **A struct at a single implementor, deliberately.** The direct
/// version — `craft` reading the bench and the toggle inline — would
/// normally win here and the spec says so; it loses only because the
/// second gatherer is already named and requested: a base-roster program
/// compiling at a bench while the player is somewhere else. The named
/// axis of change is the crafter, so the crafter is what the type
/// captures, and the roll never learns there is more than one.
///
/// The four *terms* are emphatically not an axis. They are addends in
/// one legible formula, and a trait with an implementor per term would
/// be the over-engineered reading of this.
/// One ingredient line's price when the player compiles carefully:
/// `QUALITY_CAREFUL_COST_PERCENT` more, rounded **up**, so a line already
/// down at one unit costs two and the toggle is never free.
///
/// A free function beside `CraftOrder` rather than a method, because it is
/// arithmetic on a quantity and knows nothing about a `Game`.
fn careful_price(qty: u32) -> u32 {
    qty + (qty * QUALITY_CAREFUL_COST_PERCENT).div_ceil(100)
}

pub(crate) struct CraftOrder {
    /// The tier of the best bench the recipe's structure is standing at,
    /// or 1 when the recipe names no bench — see
    /// `Game::best_structure_tier` for why a bench that cannot be
    /// upgraded and one that has not been are the same number.
    bench_tier: u32,
    /// How many levels of `Perk::TightenTolerances` whoever is compiling
    /// has bought. A program has none of its own, which is why this is
    /// gathered per crafter rather than read inside the floor.
    perk_level: u32,
    /// Whether the player chose to spend extra materials on this batch.
    careful: bool,
}

impl Game {
    /// The full list of things the player can compile right now: every item
    /// declaring a `craftable` def (see `assets/items/*.ron`) whose bench, if
    /// it names one, is deployed, plus every recipe from an unlocked research
    /// node whose bench (`ResearchRecipe::requires_structure`) is currently
    /// deployed. Recipe data lives in `assets/{items,research}/*.ron` so a mod
    /// can add one without touching Rust.
    ///
    /// **The costs are the discounted ones**, not the authored `.ron`
    /// quantities — `Perk::LeanCompiler` is applied here, at the one point
    /// every reader of a player-facing recipe passes through. It used to be
    /// applied in `craft_cost` alone, which meant the price a screen *quoted*
    /// and the price `craft` *charged* came from two different places, and
    /// the Compile screen quoted the undiscounted one. A machine's recipe is
    /// not affected and must not be: `systems::assembly_recipe` reads
    /// `ItemDb` directly, so a perk of the player's cannot reach into what a
    /// structure consumes.
    pub fn craft_recipes(&self) -> Vec<CraftRecipe> {
        let perks = self.player_perks();
        let charged = |cost: &[(ItemId, u32)]| -> Vec<(ItemId, u32)> {
            cost.iter()
                .map(|(item, qty)| {
                    (
                        item.clone(),
                        crate::perks::discounted_craft_cost(perks, *qty),
                    )
                })
                .collect()
        };
        let mut recipes: Vec<CraftRecipe> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter_map(|def| {
                let c = def.craftable.as_ref()?;
                let bench_ready = c
                    .requires_structure
                    .as_ref()
                    .is_none_or(|s| self.has_structure(s));
                bench_ready.then(|| CraftRecipe {
                    result: def.id.clone(),
                    cost: charged(&c.cost),
                    requires_structure: c.requires_structure.clone(),
                })
            })
            .collect();
        for def in self.world.resource::<ResearchDb>().all() {
            if !self.is_researched(&def.id) {
                continue;
            }
            for recipe in &def.unlocks_recipes {
                let bench_ready = recipe
                    .requires_structure
                    .as_ref()
                    .is_none_or(|s| self.has_structure(s));
                if bench_ready {
                    recipes.push(CraftRecipe {
                        result: recipe.result.clone(),
                        cost: charged(&recipe.cost),
                        requires_structure: recipe.requires_structure.clone(),
                    });
                }
            }
        }
        // After the researched half is pushed, not before: sorted early, every
        // unlocked recipe would trail the list in a block of its own and the
        // Compile screen's category column would contradict itself at the
        // bottom. The ordering is decided here rather than in the renderer
        // because `App::handle_craft_key` dispatches `recipes[idx]` from a
        // different `craft_recipes` call than the one the screen draws.
        recipes.sort_by_key(|r| self.category_sort_key(&r.result));
        recipes
    }

    /// Whether a structure of `kind` exists anywhere right now. Every
    /// structure is player-built, so this doubles as "has the player built
    /// one of these" — backs `ResearchRecipe::requires_structure`, the bench
    /// a researched recipe needs deployed before it shows up (see
    /// `craft_recipes`).
    pub(crate) fn has_structure(&self, kind: &str) -> bool {
        self.world
            .iter_entities()
            .any(|e| e.get::<Structure>().is_some_and(|s| s.kind == kind))
    }

    /// The per-unit cost to compile `result` right now — its `craft_recipes`
    /// entry, which already carries the `Perk::LeanCompiler` discount, plus
    /// the careful-compile surcharge if `careful`. Empty if `result` has no
    /// recipe.
    ///
    /// A lookup rather than a second application of the discount: the two
    /// diverged once already, and a screen quoting one while `craft` charges
    /// the other is invisible until a player is told they need three of
    /// something the game would have taken two of. The surcharge is added
    /// **here and nowhere else** for exactly that reason, which is also why
    /// `careful` is a parameter of all three price questions rather than a
    /// flag the caller applies afterwards.
    ///
    /// The surcharge is charged on the discounted number, rounded up. The
    /// order is what makes the perk and the toggle compose the way a player
    /// would read them: the perk makes a recipe cheaper, and being careful
    /// costs half again of what they actually pay. Reversed, a fully perked
    /// recipe — every line floored at 1 — would be careful for free.
    pub fn craft_cost(&self, result: &ItemId, careful: bool) -> Vec<(ItemId, u32)> {
        self.craft_recipes()
            .into_iter()
            .find(|r| &r.result == result)
            .map(|r| {
                if !careful {
                    return r.cost;
                }
                r.cost
                    .into_iter()
                    .map(|(item, qty)| (item, careful_price(qty)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The most whole units of `result` the player can afford to compile
    /// right now, given `craft_cost` (already Lean-Compiler-adjusted, and
    /// carrying the careful surcharge when `careful`) and their current
    /// inventory. 0 if `result` has no recipe or they can't afford even one
    /// unit yet.
    pub fn max_craftable(&self, result: &ItemId, careful: bool) -> u32 {
        let cost = self.craft_cost(result, careful);
        if cost.is_empty() {
            return 0;
        }
        let inv = self.world.get::<Inventory>(self.player_entity()).unwrap();
        cost.iter()
            .map(|(item, qty)| inv.count(item) / (*qty).max(1))
            .min()
            .unwrap_or(0)
    }

    /// What the player, standing where they are with the base they have,
    /// compiles `recipe` at.
    ///
    /// One of two gatherers by design (see `CraftOrder`); the other belongs
    /// to a base-roster program and is not built yet.
    pub(crate) fn player_craft_order(&self, recipe: &CraftRecipe, careful: bool) -> CraftOrder {
        CraftOrder {
            bench_tier: recipe
                .requires_structure
                .as_ref()
                .and_then(|kind| self.best_structure_tier(kind))
                .unwrap_or(1),
            perk_level: self.player_perk_level(Perk::TightenTolerances),
            careful,
        }
    }

    /// The floor a copy compiled under this order rolls off:
    /// `QUALITY_BASE` plus a term per input the player built toward.
    ///
    /// **The one expression of the floor**, so a screen that wants to quote
    /// what a bench is worth and the compile that charges for it cannot
    /// disagree. The clamp is not here — `Game::roll_quality` holds the one
    /// clamp for every source of a copy — so this may legitimately return a
    /// number above `QUALITY_MAX`, and a modded bench with an absurd
    /// `max_tier` saturates rather than wrapping.
    pub(crate) fn craft_quality_floor(&self, order: &CraftOrder) -> u8 {
        let bench = order.bench_tier.saturating_sub(1) * QUALITY_BENCH_PER_TIER as u32;
        let perk = crate::perks::quality_floor_bonus(order.perk_level);
        let care = if order.careful {
            QUALITY_CAREFUL_BONUS as u32
        } else {
            0
        };
        (QUALITY_BASE as u32 + bench + perk + care).min(u8::MAX as u32) as u8
    }

    /// How long compiling one unit of `item` by hand takes, in ticks.
    ///
    /// **The one door onto the number**, so the screen that quotes it and
    /// the loop that spends it cannot disagree: `HAND_CRAFT_TICK_MULT`
    /// times the cycle of the machine that exists to do the job — the
    /// `assembles` block naming this item, else the `work` block producing
    /// it, else `HAND_CRAFT_DEFAULT_CYCLE`. A screen recomputing the
    /// product from the constant is the second copy this exists to prevent.
    ///
    /// Both lookups walk `StructureDb::all`, whose order is sorted, so a
    /// mod shipping two machines for one item resolves the same way every
    /// session.
    pub fn hand_craft_ticks(&self, item: &ItemId) -> u32 {
        let db = self.world.resource::<StructureDb>();
        let cycle = db
            .all()
            .find_map(|d| {
                d.assembles
                    .as_ref()
                    .filter(|a| &a.item == item)
                    .map(|a| a.ticks_per_unit)
            })
            .or_else(|| {
                db.all().find_map(|d| {
                    d.work
                        .as_ref()
                        .filter(|w| &w.produces == item)
                        .map(|w| w.ticks_per_unit)
                })
            })
            .unwrap_or(crate::tuning::HAND_CRAFT_DEFAULT_CYCLE);
        crate::tuning::HAND_CRAFT_TICK_MULT * cycle
    }

    /// Whether a hand-compile is in flight — see `resources::HandCraft`.
    pub fn hand_craft_in_progress(&self) -> bool {
        self.world
            .contains_resource::<crate::resources::HandCraft>()
    }

    /// Arms a hand-compile of `quantity` units of `result` and spends
    /// nothing.
    ///
    /// **Every refusal lands here, before a tick or a unit of material is
    /// spent.** `craft` is this call plus the drain, so the headless
    /// compile and the screen refuse identically and in the same order.
    ///
    /// The affordability check is over the whole batch, which is what makes
    /// the quoted refusal name the batch's bill; the *spending* is per unit
    /// (see `advance_hand_craft`), so the two can come apart if something
    /// takes from the pack while the compile runs. That is a real edge —
    /// a build crew reaches into the player's pack from base space — and
    /// the loop ends the batch rather than compiling out of nothing.
    pub fn begin_hand_craft(
        &mut self,
        result: &ItemId,
        quantity: u32,
        careful: bool,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if quantity == 0 {
            return Err("Compile at least 1.".into());
        }
        let Some(recipe) = self
            .craft_recipes()
            .into_iter()
            .find(|r| &r.result == result)
        else {
            return Err(format!("{} can't be compiled.", self.item_name(result)));
        };
        let player = self.player_entity();
        let cost = self.craft_cost(result, careful);
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item) < *qty * quantity {
                    return Err(format!(
                        "Compiling {} {} needs {} {}.",
                        quantity,
                        self.item_name(result),
                        qty * quantity,
                        self.item_name(item)
                    ));
                }
            }
        }
        let quality_floor = self
            .equipment_of(result)
            .is_some()
            .then(|| self.craft_quality_floor(&self.player_craft_order(&recipe, careful)));
        self.world.insert_resource(crate::resources::HandCraft {
            item: result.clone(),
            units: quantity,
            remaining: quantity,
            ticks_done: 0,
            spent: false,
            completed: 0,
            careful,
            quality_floor,
        });
        Ok(())
    }

    /// Spends one tick of the compile in flight and reports where it is,
    /// or `None` when nothing is in flight.
    ///
    /// **The only code that spends a unit's material or grants one.** The
    /// ingredients come out of the pack at the unit's *start* and the copy
    /// is rolled and granted at its end, so an abort keeps every finished
    /// unit, refunds the one in flight, and costs only the time already
    /// spent — the same shape as *materials are not spent until the
    /// structure is raised*, rather than a second rule about part payment.
    ///
    /// A tick can start a fight and a tick can end the run, and either ends
    /// the batch here exactly as it ends a drag step's extra ticks in
    /// `Game::move_player`: the rest must not resolve behind a screen the
    /// player has not seen.
    pub fn advance_hand_craft(&mut self) -> Option<crate::resources::HandCraftProgress> {
        let (item, careful, spent) = {
            let job = self.world.get_resource::<crate::resources::HandCraft>()?;
            (job.item.clone(), job.careful, job.spent)
        };
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Some(self.close_hand_craft());
        }
        let ticks_total = self.hand_craft_ticks(&item);
        if !spent {
            if !self.take_hand_craft_unit(&item, careful) {
                return Some(self.close_hand_craft());
            }
            self.world
                .resource_mut::<crate::resources::HandCraft>()
                .spent = true;
        }
        self.tick();
        let unit_done = {
            let mut job = self.world.resource_mut::<crate::resources::HandCraft>();
            job.ticks_done += 1;
            job.ticks_done >= ticks_total
        };
        if unit_done {
            self.grant_hand_craft_unit(&item);
            let finished = {
                let mut job = self.world.resource_mut::<crate::resources::HandCraft>();
                job.remaining = job.remaining.saturating_sub(1);
                job.completed += 1;
                job.ticks_done = 0;
                job.spent = false;
                job.remaining == 0
            };
            if finished {
                return Some(self.close_hand_craft());
            }
        }
        let job = self.world.resource::<crate::resources::HandCraft>();
        Some(crate::resources::HandCraftProgress {
            item,
            unit: (job.units - job.remaining + 1).min(job.units),
            units: job.units,
            ticks_done: job.ticks_done,
            ticks_total,
            finished: false,
        })
    }

    /// Walks away from the compile in flight, refunding the unit that was
    /// part-way through and keeping every one already finished.
    pub fn abort_hand_craft(&mut self) {
        if self.hand_craft_in_progress() {
            self.close_hand_craft();
        }
    }

    /// Takes one unit's ingredients out of the pack, or reports that the
    /// pack can no longer cover them.
    fn take_hand_craft_unit(&mut self, item: &ItemId, careful: bool) -> bool {
        let player = self.player_entity();
        let cost = self.craft_cost(item, careful);
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            if cost.iter().any(|(id, qty)| inv.count(id) < *qty) {
                return false;
            }
        }
        let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
        for (id, qty) in &cost {
            inv.take(id.clone(), *qty);
        }
        true
    }

    /// Puts one finished unit into cargo, rolling its quality if the batch
    /// is gear — see `resources::HandCraft::quality_floor`.
    fn grant_hand_craft_unit(&mut self, item: &ItemId) {
        let floor = self
            .world
            .resource::<crate::resources::HandCraft>()
            .quality_floor;
        match floor {
            Some(floor) => {
                let quality = self.roll_quality(floor);
                let copy = GearCopy {
                    quality,
                    ..GearCopy::plain(item.clone())
                };
                self.add_copies(&copy, 1);
            }
            None => {
                let player = self.player_entity();
                self.world
                    .get_mut::<Inventory>(player)
                    .unwrap()
                    .add(item.clone(), 1);
            }
        }
    }

    /// Ends the batch however it ended: refunds the unit in flight,
    /// announces what actually came out of it, and drops the resource.
    ///
    /// The batch is announced **once, on the way out**, with the count that
    /// was really granted — a line per unit would turn a batch of twelve
    /// into twelve rows of log, and a line at the start would promise units
    /// an abort never delivers.
    fn close_hand_craft(&mut self) -> crate::resources::HandCraftProgress {
        let job = self
            .world
            .remove_resource::<crate::resources::HandCraft>()
            .expect("close_hand_craft is only reached with a compile in flight");
        if job.spent {
            let player = self.player_entity();
            let cost = self.craft_cost(&job.item, job.careful);
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (id, qty) in &cost {
                inv.add(id.clone(), *qty);
            }
        }
        if job.completed > 0 {
            self.log_base_kind(
                MessageKind::Loot,
                format!(
                    "You compile {} {} from salvaged components.",
                    job.completed,
                    self.item_name(&job.item)
                ),
            );
        }
        crate::resources::HandCraftProgress {
            item: job.item,
            unit: (job.units - job.remaining + 1).min(job.units),
            units: job.units,
            ticks_done: job.ticks_done,
            ticks_total: 0,
            finished: true,
        }
    }

    /// Compiles `quantity` units of `result` per its `craft_recipes` entry,
    /// start to finish, spending the whole of `hand_craft_ticks` per unit.
    ///
    /// `careful` spends `QUALITY_CAREFUL_COST_PERCENT` more material for a
    /// better floor on every unit in the batch — the toggle is the batch's,
    /// not the unit's.
    ///
    /// **The loop drained to completion**, and deliberately nothing more:
    /// `begin_hand_craft` holds every refusal and `advance_hand_craft` is
    /// the only code that spends or grants a unit, so the headless compile
    /// every test uses and the screen the player watches are two drivers of
    /// one sequence rather than two copies of it.
    ///
    /// **A piece of gear is rolled per unit**, so compiling five is five
    /// copies to compare rather than a stack of five identical ones; that
    /// spread is the whole of what the quality axis is for. Anything else
    /// stacks in `Inventory` exactly as it did and spends **no** `GameRng`
    /// draw, the property `grant_gear_drop` already holds for a material.
    pub fn craft(&mut self, result: &ItemId, quantity: u32, careful: bool) -> Result<(), String> {
        self.begin_hand_craft(result, quantity, careful)?;
        while let Some(progress) = self.advance_hand_craft() {
            if progress.finished {
                break;
            }
        }
        Ok(())
    }

    /// How many of exactly this copy the player is carrying.
    ///
    /// The one place the split between the two cargo stores is decided:
    /// `Inventory` holds plain copies and `GearCopies` every special one
    /// (see those components). `count_copies`/`take_copies`/`add_copies`
    /// are the only three functions that know the rule, and all three ask
    /// `GearCopy::is_plain` rather than spelling it out — so an action
    /// naming a copy never has to pick a store itself, no new action can
    /// pick the wrong one, and a fourth property added to a copy cannot
    /// leave one of the three disagreeing about where it lives.
    pub(crate) fn count_copies(&self, copy: &GearCopy) -> u32 {
        let player = self.player_entity();
        if copy.is_plain() {
            self.world
                .get::<Inventory>(player)
                .map(|inv| inv.count(&copy.item))
                .unwrap_or(0)
        } else {
            self.world
                .get::<GearCopies>(player)
                .map(|f| f.count(copy))
                .unwrap_or(0)
        }
    }

    /// Removes up to `qty` of this copy, returning how many were taken —
    /// see `count_copies`.
    pub(crate) fn take_copies(&mut self, copy: &GearCopy, qty: u32) -> u32 {
        let player = self.player_entity();
        if copy.is_plain() {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(copy.item.clone(), qty)
        } else {
            self.world
                .get_mut::<GearCopies>(player)
                .unwrap()
                .take(copy, qty)
        }
    }

    /// Puts `qty` of this copy into cargo — see `count_copies`.
    pub(crate) fn add_copies(&mut self, copy: &GearCopy, qty: u32) {
        let player = self.player_entity();
        if copy.is_plain() {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(copy.item.clone(), qty);
        } else {
            self.world
                .get_mut::<GearCopies>(player)
                .unwrap()
                .add(copy.clone(), qty);
        }
    }

    /// Adds (`sign` = 1) or removes (`sign` = -1) an equipped item's stat
    /// bonus from the player's `Stats`/`Decompiler`. Shared by `equip` and
    /// `unequip` so the two stay symmetric.
    pub(crate) fn apply_equipment_delta(
        &mut self,
        player: Entity,
        mods: items::EquipmentStats,
        sign: i32,
    ) {
        // `atk` and `mitigation` only, and the other three axes are an
        // omission rather than an oversight: `damage`, `accuracy` and
        // `evasion` have no `Stats` field to bake into, and inventing one
        // would give them a second home to drift from. They are read live
        // off `Game::gear_bonus` at the two places that need them —
        // `Game::attack_range` and `Game::combatant_profile`.
        if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
            stats.atk += sign * mods.atk;
            stats.mitigation += sign * mods.mitigation;
        }
        if mods.decompiler != 0
            && let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player)
        {
            decompiler.skill += sign * mods.decompiler;
        }
    }

    /// Peeks `slot`'s occupant without mutating anything, erroring if its id
    /// no longer resolves in `ItemDb` (a save naming a since-removed mod
    /// item). `equip` and `unequip` both resolve the outgoing item this way
    /// *before* touching the slot, so a refusal can't strand gear outside
    /// both `Equipment` and cargo.
    ///
    /// It deliberately does not return the bonus. `worn_bonus` is the one
    /// place a worn item's stats are computed, and handing a second caller
    /// the *base* stats is how a call site ends up scaling them itself in an
    /// order of its own choosing.
    pub(crate) fn slot_occupant(
        &self,
        player: Entity,
        slot: EquipmentSlot,
    ) -> Result<Option<EquippedItem>, String> {
        let Some(equipped) = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot))
        else {
            return Ok(None);
        };
        if self.equipment_of(&equipped.copy.item).is_none() {
            return Err(format!(
                "Your equipped {} is missing from the item set and can't be moved.",
                self.item_name(&equipped.copy.item)
            ));
        }
        Ok(Some(equipped))
    }

    /// What one copy of gear is worth at gear level `level`: its authored
    /// bonus plus its affix, put through all four scaling axes in the
    /// canonical order.
    ///
    /// **The order of the four axes is load-bearing.** The affix is folded
    /// into the base, then level, then quality, then the two floored axes.
    /// Quality carries no floor and so cannot go last (see
    /// `EquipmentStats::for_quality`); keeping fusion and rarity after it is
    /// what preserves the honest form of their guarantee — a rare tier's
    /// floor is worth a rung *against a copy of equal quality*.
    ///
    /// **This is the only place that order is written down**, and the order
    /// is load-bearing rather than stylistic — two of the three axes carry a
    /// per-step floor, and a floor does not commute with a multiplier (see
    /// `the_gear_axes_do_not_commute_so_the_order_is_load_bearing`). Every
    /// equip, unequip, preview and strip resolves through here, so an
    /// unequip cannot subtract a differently-ordered — or differently
    /// scaled — figure from the one its equip added and weld the difference
    /// into the wearer's base `Stats`.
    ///
    /// It is `pub` rather than `pub(crate)` because **the screens need it as
    /// much as the operations do**, and could not have it. Four of them —
    /// the inventory tag, the swap picker's two columns and the equipped
    /// panel — each rebuilt this chain by hand out of `equipment_of`, and
    /// every one of them knew about the properties a `GearCopy` carried on
    /// the day it was written. When the affix landed as the fourth, all four
    /// went on pricing gear as though it did not exist: a row that *named*
    /// "Overdriven Kinetic Edge" costed the bare Kinetic Edge, understating
    /// itself by the affix times the zone. Taking the whole copy rather than
    /// its loose properties is the same argument `EquippedItem`'s doc makes:
    /// a fifth property is then not forgettable at a call site.
    ///
    /// `None` when the item has dropped out of `ItemDb`, which a save naming
    /// a since-removed mod item produces.
    pub fn copy_bonus(&self, copy: &GearCopy, level: u32) -> Option<items::EquipmentStats> {
        let (_, base) = self.equipment_of(&copy.item)?;
        // Affixes are added to the *base* before any scaling, so they grow
        // with gear level and both tiers exactly as the item's own bonus
        // does. Added after would make an affix worth steadily less as a run
        // goes on, which is the opposite of what a rolled property is for —
        // and would make a scavenged weapon with a good affix worthless
        // after one breach.
        //
        // Every affix is summed, so a copy fused from two carrying the same
        // one is worth it twice — the duplicate is the feature, not a row
        // to dedupe.
        let affixed =
            self.affixes_of(copy)
                .into_iter()
                .fold(base, |acc, affix| items::EquipmentStats {
                    atk: acc.atk + affix.stats.atk,
                    mitigation: acc.mitigation + affix.stats.mitigation,
                    decompiler: acc.decompiler + affix.stats.decompiler,
                    damage: crate::battle::DamageRange {
                        min: acc.damage.min + affix.stats.damage.min,
                        max: acc.damage.max + affix.stats.damage.max,
                    },
                    accuracy: acc.accuracy + affix.stats.accuracy,
                    evasion: acc.evasion + affix.stats.evasion,
                });
        Some(
            affixed
                .scaled_for_level(level)
                .for_quality(copy.quality)
                .fused_for_tier(copy.tier)
                .for_rarity(copy.rarity),
        )
    }

    /// A damage band as the string every screen prints — `"4–9"`, or `"6"`
    /// for a degenerate one.
    ///
    /// **One function, the way `Game::copy_name` is the one place a copy's
    /// name is built.** A displayed range that disagrees with the damage
    /// actually rolled is the hand-rolled-chain bug in a new place: sharing
    /// the *formatter* was never enough on `copy_bonus`, four screens rebuilt
    /// the scaling chain themselves and all four dropped the affix at once.
    /// The scaling is `copy_bonus`'s and is not repeated here — this takes a
    /// band that has already been through all three axes.
    ///
    /// An en dash rather than a hyphen: the map's status column is measured
    /// in DejaVu Sans Mono where both are one cell, and the en dash reads as
    /// a range rather than as a minus sign in front of `max`.
    pub fn damage_range_label(&self, range: battle::DamageRange) -> String {
        crate::abilities::range_label(range)
    }

    /// What one *worn* item is worth — `copy_bonus` at the level the copy
    /// remembers being equipped at, which is the only thing a worn item adds
    /// to a carried one.
    pub(crate) fn worn_bonus(&self, worn: &EquippedItem) -> Option<items::EquipmentStats> {
        self.copy_bonus(&worn.copy, worn.level)
    }

    /// What `wearer`'s gear is worth right now — every worn slot's bonus,
    /// through `worn_bonus`. The single definition of that sum: nothing else
    /// walks the slots itself, so a fourth slot cannot be half-counted.
    ///
    /// A slot whose item has dropped out of `ItemDb` contributes nothing
    /// rather than erroring. This is a read used *inside* operations that
    /// must not fail halfway — unlike `unequip`, which can refuse the whole
    /// action and does.
    pub(crate) fn gear_bonus(&self, wearer: Entity) -> items::EquipmentStats {
        let Some(equipment) = self.world.get::<Equipment>(wearer) else {
            return items::EquipmentStats::default();
        };
        EquipmentSlot::ALL
            .into_iter()
            .filter_map(|slot| self.worn_bonus(&equipment.get(slot)?))
            .fold(items::EquipmentStats::default(), |acc, mods| {
                items::EquipmentStats {
                    atk: acc.atk + mods.atk,
                    mitigation: acc.mitigation + mods.mitigation,
                    decompiler: acc.decompiler + mods.decompiler,
                    // Summed like everything else, but only one slot ever
                    // carries a band: `every_weapon_authors_a_range_and_
                    // nothing_else_does` holds the Weapon slot to being the
                    // only source, so this is an override in practice
                    // without needing a rule here to make it one.
                    damage: crate::battle::DamageRange {
                        min: acc.damage.min + mods.damage.min,
                        max: acc.damage.max + mods.damage.max,
                    },
                    accuracy: acc.accuracy + mods.accuracy,
                    evasion: acc.evasion + mods.evasion,
                }
            })
    }

    /// Returns everything `wearer` has on to the player's cargo, leaving no
    /// bonus behind in `Stats`. Idempotent on an entity wearing nothing —
    /// including one that has no `Equipment` at all, which is what a program
    /// that has never been geared looks like.
    ///
    /// Deliberately **not** three `unequip` calls: `unequip` refuses during a
    /// battle and calls `tick()`, and a companion dying mid-battle is
    /// precisely when this runs.
    pub(crate) fn strip_gear(&mut self, wearer: Entity) {
        let Some(equipment) = self.world.get::<Equipment>(wearer).cloned() else {
            return;
        };
        self.apply_equipment_delta(wearer, self.gear_bonus(wearer), -1);
        for slot in EquipmentSlot::ALL {
            if let Some(worn) = equipment.get(slot) {
                self.add_copies(&worn.copy, 1);
            }
        }
        *self.world.get_mut::<Equipment>(wearer).unwrap() = Equipment::default();
    }

    /// Refuses a wearer that is neither the player nor a program they own.
    /// Both `equip` and `unequip` ask *before* anything moves — the ordering
    /// `use_symlink` and `install_routine` keep, so a refusal spends nothing.
    ///
    /// The wearer is an `Entity` rather than a `Wearer` enum because that is
    /// already the idiom (`add_companion`, `refactor_companion`,
    /// `wield_program`, `sell_companion`), and because this guard is where
    /// "a program the player owns" is decided once rather than at each caller.
    fn check_wearer(&self, wearer: Entity) -> Result<(), String> {
        if wearer == self.player_entity() {
            return Ok(());
        }
        match self.world.get::<Tamed>(wearer) {
            Some(tamed) if tamed.owner == self.player_entity() => Ok(()),
            _ => Err("Only you and the programs you own can wear gear.".into()),
        }
    }

    /// Equips the copy of `item` at fusion `tier` from cargo into its slot,
    /// swapping out (and returning to cargo, at its own tier) whatever was
    /// there before. The bonus applied is scaled for the current
    /// `resources::ZoneLevel` — see `items::EquipmentStats::scaled_for_level`
    /// — so gear equipped after breaching deeper is stronger than the same
    /// item equipped earlier.
    ///
    /// Which *copy* is a parameter rather than looked up, because the player
    /// may hold several copies of one item at different tiers and rare tiers
    /// and only they know which one they meant — see
    /// `components::GearCopies`.
    ///
    /// `wearer` is the player or any program they own (`check_wearer`).
    /// `count_copies`/`take_copies`/`add_copies` keep resolving the player
    /// themselves whoever wears the result, and that is the feature rather
    /// than an oversight: **gear comes from and returns to the player's
    /// cargo**, which is what makes a copy interchangeable between them.
    pub fn equip(&mut self, wearer: Entity, copy: &GearCopy) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.check_wearer(wearer)?;
        let item = &copy.item;
        let Some((slot, _)) = self.equipment_of(item) else {
            return Err(format!("{} can't be equipped.", self.item_name(item)));
        };
        // The outgoing item must resolve before anything moves: a refusal
        // after the swap would leave it in neither Equipment nor cargo,
        // destroying it.
        let outgoing = self.slot_occupant(wearer, slot)?;
        if self.take_copies(copy, 1) == 0 {
            return Err(format!("You don't have a {}.", self.item_name(item)));
        }
        let level = self.world.resource::<ZoneLevel>().0;
        let incoming = EquippedItem {
            copy: copy.clone(),
            level,
        };

        // Both deltas go through `worn_bonus`, which is what makes the pair
        // symmetric: the figure subtracted for the outgoing copy is computed
        // from that copy's own stored properties by the same function that
        // computed what its equip added.
        if let Some(old) = outgoing {
            if let Some(mods) = self.worn_bonus(&old) {
                self.apply_equipment_delta(wearer, mods, -1);
            }
            self.add_copies(&old.copy, 1);
        }
        if let Some(mods) = self.worn_bonus(&incoming) {
            self.apply_equipment_delta(wearer, mods, 1);
        }
        {
            // Inserted on demand rather than at every spawn site: absence
            // already reads as an empty loadout everywhere, so a program only
            // grows the component the moment it wears something.
            let mut entity = self.world.entity_mut(wearer);
            entity.insert_if_new(Equipment::default());
            let mut equipment = entity.get_mut::<Equipment>().unwrap();
            *equipment.slot_mut(slot) = Some(incoming);
        }

        let mut notes = Vec::new();
        if level > 1 {
            notes.push(format!("level {level}"));
        }
        if copy.tier > 0 {
            notes.push(format!("fusion tier {}", copy.tier));
        }
        if let Some(tier) = copy.rarity.label() {
            notes.push(tier.to_lowercase());
        }
        let note = if notes.is_empty() {
            String::new()
        } else {
            format!(" ({})", notes.join(", "))
        };
        let line = if wearer == self.player_entity() {
            format!("You equip {}{note}.", self.item_name(item))
        } else {
            format!(
                "{} equips {}{note}.",
                self.entity_label(wearer),
                self.item_name(item)
            )
        };
        self.log(line);
        self.tick();
        Ok(())
    }

    /// Unequips whatever's in `slot`, returning it to cargo — to
    /// `GearCopies` if the worn copy was fused, to `Inventory` if it wasn't.
    /// The copy keeps the tier it went on with, so gear does not launder
    /// its fusion through a slot in either direction.
    pub fn unequip(&mut self, wearer: Entity, slot: EquipmentSlot) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.check_wearer(wearer)?;
        let is_player = wearer == self.player_entity();
        // Every refusal must come before the item leaves its Equipment slot:
        // a refusal after removal would leave the gear in neither place,
        // destroying it.
        let Some(equipped) = self.slot_occupant(wearer, slot)? else {
            return Err(if is_player {
                format!("Nothing equipped in your {} slot.", slot.label())
            } else {
                format!(
                    "Nothing equipped in {}'s {} slot.",
                    self.entity_label(wearer),
                    slot.label()
                )
            });
        };
        {
            let mut equipment = self.world.get_mut::<Equipment>(wearer).unwrap();
            *equipment.slot_mut(slot) = None;
        }
        if let Some(mods) = self.worn_bonus(&equipped) {
            self.apply_equipment_delta(wearer, mods, -1);
        }
        self.add_copies(&equipped.copy, 1);
        let line = if is_player {
            format!("You unequip {}.", self.item_name(&equipped.copy.item))
        } else {
            format!(
                "{} gives up {}.",
                self.entity_label(wearer),
                self.item_name(&equipped.copy.item)
            )
        };
        self.log(line);
        self.tick();
        Ok(())
    }

    /// Consumes `crate::tuning::ITEM_FUSION_COST` copies of `item` at
    /// fusion `tier` and yields **one physical copy** at `tier + 1`, whose
    /// equipped bonus is another `crate::tuning::ITEM_FUSION_BONUS_PER_TIER`
    /// stronger (see `components::GearCopies`,
    /// `EquipmentStats::fused_for_tier`) — a sink for extra copies of gear.
    /// Only equippable items qualify.
    ///
    /// The ingredients come from whichever store holds that tier
    /// (`count_copies`), so climbing the ladder costs 2 base copies for a
    /// T1, 4 for a T2 and 8 for a T3. Spares are untouched: they stay
    /// ordinary, which is the whole of this feature.
    ///
    /// A copy currently worn in the item's slot counts as one of the two if
    /// it is eligible, so wearing it plus a single spare is enough. Whenever
    /// either half was worn the result takes the slot and picks up the new
    /// tier's bonus immediately rather than only on the next re-equip, so
    /// the player is never left bare. Returns the confirmation line on
    /// success so the caller can surface it — unlike equipping, a fusion
    /// changes nothing else the player can see.
    ///
    /// Bounded by `tuning::MAX_FUSIONS`, the same ceiling a program's
    /// lineage has — see `Game::fuse_companions`. Both refusals sit above
    /// the first `take_copies` deliberately: a refused fusion must spend
    /// nothing from either store, the same ordering `install_routine` and
    /// `use_symlink` keep.
    /// **What must match is `GearCopy::fusable_with` — item, rare tier and
    /// fusion tier.** Quality and affixes go free and are carried forward:
    /// the two qualities average and the two affix lists union, duplicates
    /// kept. The **survivor** is the copy passed in, which is the one the
    /// player pressed `[U]` on; the **partner** is chosen automatically as
    /// the best eligible spare, and there is no picker.
    ///
    /// Rarity stays matched deliberately: laundering a rare tier into or out
    /// of the result depending on which parent won is the alternative, and
    /// there is no midpoint tier for a Gold-plus-Ordinary fuse to land on
    /// (the same argument `fuse_companions` makes for taking `max`, which it
    /// can do because it is combining two creatures rather than consuming
    /// two of one thing).
    pub fn fuse_item(&mut self, copy: &GearCopy) -> Result<String, String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let msg = self.fuse_copy(copy)?;
        self.log(msg.clone());
        self.tick();
        Ok(msg)
    }

    /// One fusion, spending no turn and logging nothing.
    ///
    /// The split exists for `fuse_all_items`, which performs many fusions
    /// for one keypress and owes the player one turn and one tally rather
    /// than one of each per pair. Every refusal still sits above the first
    /// `take_copies`, so a caller looping on this until it errors spends
    /// nothing on the call that stops it.
    ///
    /// `copy` is the survivor; `best_fusion_partner` picks what it is fused
    /// with. See `fuse_item` for what must match and what is carried
    /// forward.
    fn fuse_copy(&mut self, copy: &GearCopy) -> Result<String, String> {
        let item = &copy.item;
        let Some((slot, _)) = self.equipment_of(item) else {
            return Err(format!("{} can't be fused.", self.item_name(item)));
        };
        let name = self.item_name(item).to_string();
        let fused_tier = copy.tier + 1;
        if fused_tier > crate::tuning::MAX_FUSIONS {
            return Err(format!(
                "{name} has already been fused {} times — it can't be fused again.",
                crate::tuning::MAX_FUSIONS
            ));
        }
        let player = self.player_entity();

        let worn = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot));
        // The survivor comes off the player's back when it is what they are
        // wearing, and out of cargo otherwise. Which it is decides whether
        // the survivor's own row can also supply the partner.
        let survivor_worn = worn.as_ref().is_some_and(|eq| &eq.copy == copy);

        let partner = if survivor_worn || self.count_copies(copy) > 0 {
            self.best_fusion_partner(copy, survivor_worn, worn.as_ref())
        } else {
            None
        };
        let Some((partner, partner_worn)) = partner else {
            // Counted over the whole eligible *group*, not over exact
            // matches: with quality and affixes free, a count of exact
            // matches would say "have 1" while the player is looking at
            // four copies of the thing.
            let have = self.fusable_group_size(copy, worn.as_ref());
            return Err(format!(
                "Need {} {name} to fuse (have {have}).",
                crate::tuning::ITEM_FUSION_COST
            ));
        };

        let mut affixes = copy.affixes.clone();
        affixes.extend(partner.affixes.iter().cloned());
        let fused = GearCopy::with_affixes(
            copy.item.clone(),
            copy.rarity,
            fused_tier,
            affixes,
            average_quality(copy.quality, partner.quality),
        );

        if !survivor_worn {
            self.take_copies(copy, 1);
        }
        if !partner_worn {
            self.take_copies(&partner, 1);
        }

        // Whenever either half came off the player's back the result takes
        // the slot, so a fusion can never leave them bare. Both figures come
        // from `worn_bonus`, so the subtraction matches what the equip added
        // and the addition matches what a re-equip would.
        if let Some(worn) = worn.filter(|_| survivor_worn || partner_worn) {
            let promoted = EquippedItem {
                copy: fused,
                level: worn.level,
            };
            if let Some(mods) = self.worn_bonus(&worn) {
                self.apply_equipment_delta(player, mods, -1);
            }
            if let Some(mods) = self.worn_bonus(&promoted) {
                self.apply_equipment_delta(player, mods, 1);
            }
            *self
                .world
                .get_mut::<Equipment>(player)
                .unwrap()
                .slot_mut(slot) = Some(promoted);
        } else {
            self.add_copies(&fused, 1);
        }

        let msg = format!(
            "You fuse {} {name} into a tier {fused_tier} bonus ({}% stronger equipped).",
            crate::tuning::ITEM_FUSION_COST,
            (fused_tier as f64 * crate::tuning::ITEM_FUSION_BONUS_PER_TIER * 100.0).round() as i32
        );
        Ok(msg)
    }

    /// The copy `survivor` will be fused with, and whether it is the one
    /// being worn.
    ///
    /// **The order is total**, and that is the point rather than tidiness:
    /// `GearCopies::copies` is a `Vec` in insertion order, which is
    /// play-history dependent, so without a full tie-break the same cargo
    /// could fuse differently between two saves of the same shape.
    ///
    /// Highest `quality` first, because that is what a player would pick.
    /// Then cargo before worn, so a fusion disturbs the slot only when it
    /// has to. Then fewest affixes, which leaves the more interesting spare
    /// in cargo for a later fusion. Then the affix ids themselves, which is
    /// what makes it total.
    fn best_fusion_partner(
        &self,
        survivor: &GearCopy,
        survivor_worn: bool,
        worn: Option<&EquippedItem>,
    ) -> Option<(GearCopy, bool)> {
        let player = self.player_entity();
        let mut candidates: Vec<(GearCopy, bool)> = Vec::new();

        let mut consider_cargo = |candidate: GearCopy, held: u32| {
            // The survivor's own row supplies a partner only when it holds
            // two or more: one of them is the survivor.
            //
            // Saturating because a save is an editable file: nothing the
            // game writes leaves a zero-quantity row (`GearCopies::take`
            // removes one), but a hand-edited save can carry one and
            // `GearCopies::add` pushes it through as-is. A `u32` underflow
            // there is a panic on load-and-fuse, not a wrong number.
            let spare = held.saturating_sub(u32::from(&candidate == survivor && !survivor_worn));
            if spare > 0 && candidate.fusable_with(survivor) {
                candidates.push((candidate, false));
            }
        };

        let plain = GearCopy::plain(survivor.item.clone());
        let held = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&plain.item))
            .unwrap_or(0);
        if held > 0 {
            consider_cargo(plain, held);
        }
        if let Some(ledger) = self.world.get::<GearCopies>(player) {
            for (candidate, held) in ledger.copies.clone() {
                consider_cargo(candidate, held);
            }
        }

        if let Some(worn) = worn.filter(|w| !survivor_worn && w.copy.fusable_with(survivor)) {
            candidates.push((worn.copy.clone(), true));
        }

        candidates.into_iter().min_by_key(|(candidate, from_slot)| {
            (
                std::cmp::Reverse(candidate.quality),
                u8::from(*from_slot),
                candidate.affixes.len(),
                candidate.affixes.clone(),
            )
        })
    }

    /// How many copies the player holds that could take part in this
    /// fusion, worn one included — what the refusal counts.
    fn fusable_group_size(&self, survivor: &GearCopy, worn: Option<&EquippedItem>) -> u32 {
        let player = self.player_entity();
        let plain = GearCopy::plain(survivor.item.clone());
        let cargo_plain = if plain.fusable_with(survivor) {
            self.world
                .get::<Inventory>(player)
                .map(|inv| inv.count(&plain.item))
                .unwrap_or(0)
        } else {
            0
        };
        let cargo_special = self
            .world
            .get::<GearCopies>(player)
            .map(|ledger| {
                ledger
                    .copies
                    .iter()
                    .filter(|(candidate, _)| candidate.fusable_with(survivor))
                    .map(|(_, held)| *held)
                    .sum::<u32>()
            })
            .unwrap_or(0);
        let on_back = u32::from(worn.is_some_and(|eq| eq.copy.fusable_with(survivor)));
        cargo_plain + cargo_special + on_back
    }

    /// Fuses every matching pair in cargo for one keypress and one turn.
    ///
    /// **One pass, not a cascade**: four ordinary copies come out as two
    /// T1s rather than one T2. That is not a rule enforced on top of the
    /// loop — it falls out of iterating a snapshot of the inventory taken
    /// before any fusing starts, so the higher-tier rows this creates are
    /// never rows it was handed. Looping `fuse_copy` on one copy therefore
    /// drains that tier in pairs and stops, and it terminates because every
    /// success removes at least one copy from the store it draws from.
    ///
    /// A worn copy still counts as one of its pair, exactly as it does for
    /// a single `fuse_item` — the rule lives in `fuse_copy`, so pressing
    /// this once and pressing `[U]` down the list are the same fusions.
    ///
    /// One turn total is the whole point: charging need decay, sweep
    /// pressure and spawn rolls per pair would make the convenience key
    /// cost more than the work it saves. A refusal spends no turn at all.
    pub fn fuse_all_items(&mut self) -> Result<String, String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let rows: Vec<GearCopy> = self
            .player_status()
            .inventory
            .into_iter()
            .map(|row| row.copy)
            .collect();

        let mut tally: Vec<(GearCopy, u32)> = Vec::new();
        for copy in rows {
            let mut pairs = 0;
            while self.fuse_copy(&copy).is_ok() {
                pairs += 1;
            }
            if pairs > 0 {
                tally.push((copy, pairs));
            }
        }
        if tally.is_empty() {
            return Err("Nothing in cargo has a matching pair to fuse.".into());
        }

        // A header plus one indented row apiece, the shape `announce_drops`
        // uses and for its reason: a `LogLine` is drawn as exactly one row
        // and never wrapped, so a joined line would grow with the haul.
        let total: u32 = tally.iter().map(|(_, pairs)| pairs).sum();
        let header = format!(
            "You fuse {total} {}:",
            if total == 1 { "pair" } else { "pairs" }
        );
        self.log(header.clone());
        for (copy, pairs) in tally {
            let row = format!(
                "  {pairs} {} -> tier {}",
                self.copy_name(&copy),
                copy.tier + 1
            );
            self.log(row);
        }
        self.tick();
        Ok(header)
    }

    /// Permanently removes `qty` of this copy from cargo. Only ever acts on
    /// unequipped stock; an equipped item must be unequipped first.
    pub fn erase_item(&mut self, copy: &GearCopy, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let taken = self.take_copies(copy, qty);
        if taken == 0 {
            return Err(format!(
                "You don't have any {}.",
                self.item_name(&copy.item)
            ));
        }
        self.log(format!("You erase {taken} {}.", self.item_name(&copy.item)));
        self.tick();
        Ok(())
    }
}

/// The quality of a copy fused from two: the average, snapped to a
/// `QUALITY_STEP`, **ties down**.
///
/// Never rounds up, so no fusion buys quality, and it always lands on a
/// `QUALITY_STEP` multiple — which every drop and every craft roll already
/// guarantees, so no screen learns to show a figure no roll could produce.
///
/// Integer arithmetic on purpose: `GearCopy::quality` is a `u8` precisely so
/// the type can take `Eq`, and a float here would put a rounding difference
/// into the key of three `==`-keyed stores.
fn average_quality(a: u8, b: u8) -> u8 {
    let step = crate::tuning::QUALITY_STEP as u32;
    ((a as u32 + b as u32 + step - 1) / (2 * step) * step) as u8
}
