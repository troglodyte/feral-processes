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
    /// bonus plus its affix, put through all three scaling axes in the
    /// canonical order.
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
        // The affix is added to the *base* before any scaling, so it grows
        // with gear level and both tiers exactly as the item's own bonus
        // does. Added after would make an affix worth steadily less as a run
        // goes on, which is the opposite of what a rolled property is for —
        // and would make a scavenged weapon with a good affix worthless
        // after one breach.
        let affixed = match self.affix_of(copy) {
            Some(affix) => items::EquipmentStats {
                atk: base.atk + affix.stats.atk,
                def: base.def + affix.stats.def,
                decompiler: base.decompiler + affix.stats.decompiler,
            },
            None => base,
        };
        Some(
            affixed
                .scaled_for_level(level)
                .fused_for_tier(copy.tier)
                .for_rarity(copy.rarity),
        )
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
                    def: acc.def + mods.def,
                    decompiler: acc.decompiler + mods.decompiler,
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
    /// **The two copies must match on every property, rare tier included**,
    /// and the result keeps it. That falls out of `GearCopy` being the unit
    /// of interchangeability rather than being a rule added on top: two
    /// copies that differ are not two of a thing. The consequence worth
    /// knowing is that an Overclocked copy can only be fused with another
    /// Overclocked copy of the same item — deliberately, since the
    /// alternative launders a rare tier either into or out of the result
    /// depending on which parent won, and there is no midpoint tier for it
    /// to land on (the same argument `fuse_companions` makes for taking
    /// `max`, which it can do because it is combining two creatures rather
    /// than consuming two of one thing).
    pub fn fuse_item(&mut self, copy: &GearCopy) -> Result<String, String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
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
        let fused = GearCopy {
            tier: fused_tier,
            ..copy.clone()
        };

        // Matched on the whole copy: wearing an ordinary one must not pay
        // for a fusion of two T1s, or of two Overclocked ones.
        let worn = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot))
            .filter(|eq| &eq.copy == copy);
        let from_cargo = crate::tuning::ITEM_FUSION_COST - u32::from(worn.is_some());

        let have = self.count_copies(copy);
        if have < from_cargo {
            return Err(format!("Need {from_cargo} {name} to fuse (have {have})."));
        }
        self.take_copies(copy, from_cargo);

        // A fusion yields one stronger copy, so only the *other* one is
        // spent. When a copy is worn it is the survivor and never leaves the
        // slot; otherwise the survivor is promoted into the tier above.
        if worn.is_none() {
            self.add_copies(&fused, 1);
        }

        // Swap the worn copy's equip-time bonus for the new tier's so the
        // boost is felt at once, not only after an unequip/re-equip. Both
        // figures come from `worn_bonus`, so the subtraction matches what
        // the equip added and the addition matches what a re-equip would.
        if let Some(worn) = worn {
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
