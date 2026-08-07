//! Recipes, crafting, and the equipment the results go into: equip,
//! unequip, fuse, and erase.

use crate::tuning::LEAN_COMPILER_DISCOUNT_PER_LEVEL;
use crate::*;

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
        let discount =
            LEAN_COMPILER_DISCOUNT_PER_LEVEL * self.player_perk_level(Perk::LeanCompiler);
        let charged = |cost: &[(ItemId, u32)]| -> Vec<(ItemId, u32)> {
            cost.iter()
                .map(|(item, qty)| (item.clone(), qty.saturating_sub(discount).max(1)))
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
                    });
                }
            }
        }
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
    /// entry, which already carries the `Perk::LeanCompiler` discount. Empty
    /// if `result` has no recipe.
    ///
    /// A lookup rather than a second application of the discount: the two
    /// diverged once already, and a screen quoting one while `craft` charges
    /// the other is invisible until a player is told they need three of
    /// something the game would have taken two of.
    pub fn craft_cost(&self, result: &ItemId) -> Vec<(ItemId, u32)> {
        self.craft_recipes()
            .into_iter()
            .find(|r| &r.result == result)
            .map(|r| r.cost)
            .unwrap_or_default()
    }

    /// The most whole units of `result` the player can afford to compile
    /// right now, given `craft_cost` (already Lean-Compiler-adjusted) and
    /// their current inventory. 0 if `result` has no recipe or they can't
    /// afford even one unit yet.
    pub fn max_craftable(&self, result: &ItemId) -> u32 {
        let cost = self.craft_cost(result);
        if cost.is_empty() {
            return 0;
        }
        let inv = self.world.get::<Inventory>(self.player_entity()).unwrap();
        cost.iter()
            .map(|(item, qty)| inv.count(item) / (*qty).max(1))
            .min()
            .unwrap_or(0)
    }

    /// Compiles `quantity` units of `result` per its `craft_recipes` entry.
    pub fn craft(&mut self, result: &ItemId, quantity: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if quantity == 0 {
            return Err("Compile at least 1.".into());
        }
        if self.craft_recipes().iter().all(|r| &r.result != result) {
            return Err(format!("{} can't be compiled.", self.item_name(result)));
        }
        let player = self.player_entity();
        let cost = self.craft_cost(result);
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
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty * quantity);
            }
            inv.add(result.clone(), quantity);
        }
        self.log_base_kind(
            MessageKind::Loot,
            format!(
                "You compile {} {} from salvaged components.",
                quantity,
                self.item_name(result)
            ),
        );
        self.tick();
        Ok(())
    }

    /// How many copies of `item` at fusion `tier` the player is carrying.
    ///
    /// The one place the split between the two cargo stores is decided:
    /// `Inventory` holds tier 0 and `FusedGear` everything above it (see
    /// that component). `count_copies`/`take_copies`/`add_copies` are the
    /// only three callers that know the rule, so an action naming a tier
    /// never has to pick a store itself — and no new action can pick the
    /// wrong one.
    pub(crate) fn count_copies(&self, item: &ItemId, tier: u32) -> u32 {
        let player = self.player_entity();
        if tier == 0 {
            self.world
                .get::<Inventory>(player)
                .map(|inv| inv.count(item))
                .unwrap_or(0)
        } else {
            self.world
                .get::<FusedGear>(player)
                .map(|f| f.count(item, tier))
                .unwrap_or(0)
        }
    }

    /// Removes up to `qty` copies of `item` at `tier`, returning how many
    /// were taken — see `count_copies`.
    pub(crate) fn take_copies(&mut self, item: &ItemId, tier: u32, qty: u32) -> u32 {
        let player = self.player_entity();
        if tier == 0 {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), qty)
        } else {
            self.world
                .get_mut::<FusedGear>(player)
                .unwrap()
                .take(item, tier, qty)
        }
    }

    /// Puts `qty` copies of `item` at `tier` into cargo — see
    /// `count_copies`.
    pub(crate) fn add_copies(&mut self, item: &ItemId, tier: u32, qty: u32) {
        let player = self.player_entity();
        if tier == 0 {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(item.clone(), qty);
        } else {
            self.world
                .get_mut::<FusedGear>(player)
                .unwrap()
                .add(item.clone(), tier, qty);
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
        if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
            stats.atk += sign * mods.atk;
            stats.def += sign * mods.def;
        }
        if mods.decompiler != 0
            && let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player)
        {
            decompiler.skill += sign * mods.decompiler;
        }
    }

    /// Peeks `slot`'s occupant and its base `EquipmentStats` without
    /// mutating anything, erroring if the occupant's id no longer resolves
    /// in `ItemDb` (a save naming a since-removed mod item). `equip` and
    /// `unequip` both resolve the outgoing item this way *before* touching
    /// the slot, so a refusal can't strand gear outside both `Equipment`
    /// and `Inventory`.
    pub(crate) fn slot_occupant_with_mods(
        &self,
        player: Entity,
        slot: EquipmentSlot,
    ) -> Result<Option<(EquippedItem, EquipmentStats)>, String> {
        let Some(equipped) = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot))
        else {
            return Ok(None);
        };
        let Some((_, base_mods)) = self.equipment_of(&equipped.item) else {
            return Err(format!(
                "Your equipped {} is missing from the item set and can't be moved.",
                self.item_name(&equipped.item)
            ));
        };
        Ok(Some((equipped, base_mods)))
    }

    /// Equips the copy of `item` at fusion `tier` from cargo into its slot,
    /// swapping out (and returning to cargo, at its own tier) whatever was
    /// there before. The bonus applied is scaled for the current
    /// `resources::ZoneLevel` — see `items::EquipmentStats::scaled_for_level`
    /// — so gear equipped after breaching deeper is stronger than the same
    /// item equipped earlier.
    ///
    /// The tier is a parameter rather than looked up, because the player
    /// may hold several copies of one item at different tiers and only they
    /// know which one they meant — see `components::FusedGear`.
    pub fn equip(&mut self, item: &ItemId, tier: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some((slot, base_mods)) = self.equipment_of(item) else {
            return Err(format!("{} can't be equipped.", self.item_name(item)));
        };
        let player = self.player_entity();
        // The outgoing item's bonus must resolve before anything moves: a
        // refusal after the swap would leave it in neither Equipment nor
        // cargo, destroying it.
        let outgoing = self.slot_occupant_with_mods(player, slot)?;
        if self.take_copies(item, tier, 1) == 0 {
            return Err(format!("You don't have a {}.", self.item_name(item)));
        }
        let level = self.world.resource::<ZoneLevel>().0;
        let fusion_tier = tier;

        {
            let mut equipment = self.world.get_mut::<Equipment>(player).unwrap();
            *equipment.slot_mut(slot) = Some(EquippedItem {
                item: item.clone(),
                level,
                fusion_tier,
            });
        }
        if let Some((old, old_base_mods)) = outgoing {
            self.apply_equipment_delta(
                player,
                old_base_mods
                    .scaled_for_level(old.level)
                    .fused_for_tier(old.fusion_tier),
                -1,
            );
            self.add_copies(&old.item, old.fusion_tier, 1);
        }
        self.apply_equipment_delta(
            player,
            base_mods
                .scaled_for_level(level)
                .fused_for_tier(fusion_tier),
            1,
        );
        let mut notes = Vec::new();
        if level > 1 {
            notes.push(format!("level {level}"));
        }
        if fusion_tier > 0 {
            notes.push(format!("fusion tier {fusion_tier}"));
        }
        let note = if notes.is_empty() {
            String::new()
        } else {
            format!(" ({})", notes.join(", "))
        };
        self.log(format!("You equip {}{note}.", self.item_name(item)));
        self.tick();
        Ok(())
    }

    /// Unequips whatever's in `slot`, returning it to cargo — to
    /// `FusedGear` if the worn copy was fused, to `Inventory` if it wasn't.
    /// The copy keeps the tier it went on with, so gear does not launder
    /// its fusion through a slot in either direction.
    pub fn unequip(&mut self, slot: EquipmentSlot) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        // Every refusal must come before the item leaves its Equipment slot:
        // a refusal after removal would leave the gear in neither place,
        // destroying it.
        let Some((equipped, base_mods)) = self.slot_occupant_with_mods(player, slot)? else {
            return Err(format!("Nothing equipped in your {} slot.", slot.label()));
        };
        {
            let mut equipment = self.world.get_mut::<Equipment>(player).unwrap();
            *equipment.slot_mut(slot) = None;
        }
        self.apply_equipment_delta(
            player,
            base_mods
                .scaled_for_level(equipped.level)
                .fused_for_tier(equipped.fusion_tier),
            -1,
        );
        self.add_copies(&equipped.item, equipped.fusion_tier, 1);
        self.log(format!("You unequip {}.", self.item_name(&equipped.item)));
        self.tick();
        Ok(())
    }

    /// Consumes `crate::tuning::ITEM_FUSION_COST` copies of `item` at
    /// fusion `tier` and yields **one physical copy** at `tier + 1`, whose
    /// equipped bonus is another `crate::tuning::ITEM_FUSION_BONUS_PER_TIER`
    /// stronger (see `components::FusedGear`,
    /// `EquipmentStats::fused_for_tier`) — a sink for extra copies of gear.
    /// Only equippable items qualify.
    ///
    /// The ingredients come from whichever store holds that tier
    /// (`count_copies`), so climbing the ladder costs 2 base copies for a
    /// T1, 4 for a T2 and 8 for a T3. Spares are untouched: they stay
    /// ordinary, which is the whole of this feature.
    ///
    /// A copy currently worn in the item's slot *at that same tier* counts
    /// as one of the two, so wearing it plus a single spare is enough; that
    /// worn copy is the survivor, stays equipped, and picks up the new
    /// tier's bonus immediately rather than only on the next re-equip.
    /// Returns the confirmation line on success so the caller can surface
    /// it — unlike equipping, a fusion changes nothing else the player can
    /// see.
    ///
    /// Bounded by `tuning::MAX_FUSIONS`, the same ceiling a program's
    /// lineage has — see `Game::fuse_companions`. Both refusals sit above
    /// the first `take_copies` deliberately: a refused fusion must spend
    /// nothing from either store, the same ordering `install_routine` and
    /// `use_symlink` keep.
    pub fn fuse_item(&mut self, item: &ItemId, tier: u32) -> Result<String, String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some((slot, base_mods)) = self.equipment_of(item) else {
            return Err(format!("{} can't be fused.", self.item_name(item)));
        };
        let name = self.item_name(item).to_string();
        let fused_tier = tier + 1;
        if fused_tier > crate::tuning::MAX_FUSIONS {
            return Err(format!(
                "{name} has already been fused {} times — it can't be fused again.",
                crate::tuning::MAX_FUSIONS
            ));
        }
        let player = self.player_entity();

        // Matched on tier as well as item: wearing an ordinary copy must not
        // pay for a fusion of two T1s, and vice versa.
        let worn = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot))
            .filter(|eq| &eq.item == item && eq.fusion_tier == tier);
        let from_cargo = crate::tuning::ITEM_FUSION_COST - u32::from(worn.is_some());

        let have = self.count_copies(item, tier);
        if have < from_cargo {
            return Err(format!("Need {from_cargo} {name} to fuse (have {have})."));
        }
        self.take_copies(item, tier, from_cargo);

        // A fusion yields one stronger copy, so only the *other* one is
        // spent. When a copy is worn it is the survivor and never leaves the
        // slot; otherwise the survivor is promoted into the tier above.
        if worn.is_none() {
            self.add_copies(item, fused_tier, 1);
        }

        // Swap the worn copy's equip-time bonus for the new tier's so the
        // boost is felt at once, not only after an unequip/re-equip.
        if let Some(worn) = worn {
            self.apply_equipment_delta(
                player,
                base_mods
                    .scaled_for_level(worn.level)
                    .fused_for_tier(worn.fusion_tier),
                -1,
            );
            if let Some(eq) = self
                .world
                .get_mut::<Equipment>(player)
                .unwrap()
                .slot_mut(slot)
                .as_mut()
            {
                eq.fusion_tier = fused_tier;
            }
            self.apply_equipment_delta(
                player,
                base_mods
                    .scaled_for_level(worn.level)
                    .fused_for_tier(fused_tier),
                1,
            );
        }

        let msg = format!(
            "You fuse {} {name} into a tier {fused_tier} bonus ({}% stronger equipped).",
            crate::tuning::ITEM_FUSION_COST,
            (fused_tier as f64 * crate::tuning::ITEM_FUSION_BONUS_PER_TIER * 100.0).round() as i32
        );
        self.log(msg.clone());
        self.tick();
        Ok(msg)
    }

    /// Permanently removes `qty` copies of `item` at fusion `tier` from
    /// cargo. Only ever acts on unequipped stock; an equipped item must be
    /// unequipped first.
    pub fn erase_item(&mut self, item: &ItemId, tier: u32, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let taken = self.take_copies(item, tier, qty);
        if taken == 0 {
            return Err(format!("You don't have any {}.", self.item_name(item)));
        }
        self.log(format!("You erase {taken} {}.", self.item_name(item)));
        self.tick();
        Ok(())
    }
}
