//! Read-only lookups against the loaded asset databases — item, structure,
//! and species metadata, plus the capacity checks that gate them.

use crate::*;

impl Game {
    pub fn structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .cloned()
            .collect()
    }

    /// Every loaded item definition, id-sorted (see `ItemDb::all`). Reached
    /// only by engine tests today; `cfg(test)` rather than widening `Game`'s
    /// renderer-facing surface for a need nothing outside the crate has yet.
    #[cfg(test)]
    pub(crate) fn item_defs(&self) -> Vec<ItemDef> {
        self.world.resource::<ItemDb>().all().cloned().collect()
    }

    /// One item definition by id, or `None` if nothing declares it. Same
    /// test-only rationale as `item_defs`.
    #[cfg(test)]
    pub(crate) fn item_def(&self, item: &ItemId) -> Option<ItemDef> {
        self.world.resource::<ItemDb>().get(item.as_str()).cloned()
    }

    /// The display name for `id`, falling back to the raw id if the item set
    /// doesn't define it (a save referencing a since-removed mod item). The
    /// fallback borrows `id`, so the returned reference is bound to the
    /// shorter of `self` and `id`.
    pub fn item_name<'a>(&'a self, id: &'a ItemId) -> &'a str {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or_else(|| id.as_str())
    }

    /// Every conversion a structure runs, each expanded back to the raw
    /// inputs it bottoms out in.
    ///
    /// The roots are `systems::produced_item`'s answer, which is already the
    /// whole of "could this structure put something in an output buffer" —
    /// so this cannot list a machine the base does not actually run, or miss
    /// one it does. Each root's own step comes from `systems::assembly_recipe`
    /// for the same reason: a machine's recipe *is* the assembled item's
    /// recipe, so the chain on screen and the batch the machine stages are
    /// one lookup rather than two that could drift.
    ///
    /// An item can legitimately appear twice with different makers — Power
    /// Cell is a Power Conduit's whole output and is also bench-craftable
    /// from fragments, and ICE Breaker the same with the Compiler. A root
    /// step reports the structure; an expanded dependency reports the item's
    /// own `requires_structure`. Both are true, and the screen shows the one
    /// that matters where it stands.
    ///
    /// Shallowest chain first, then by name, so the screen opens on the taps
    /// and reads down into what needs a base.
    pub fn recipe_chains(&self) -> Vec<RecipeChain> {
        let structures = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        let mut chains: Vec<RecipeChain> = structures
            .all()
            .filter_map(|def| {
                let output = crate::systems::produced_item(def)?;
                let inputs = crate::systems::assembly_recipe(def, items).unwrap_or(&[]);
                let mut steps = Vec::new();
                // Seeded with the root, whose own step is appended below: a
                // recipe that reaches back to what it makes would otherwise
                // emit that step twice.
                let mut seen = vec![output.clone()];
                for (id, _) in inputs {
                    self.expand_recipe(id, &mut seen, &mut steps);
                }
                steps.push(RecipeStep {
                    inputs: self.named_costs(inputs),
                    maker: Some(def.name.clone()),
                    output: self.item_name(output).to_string(),
                });
                Some(RecipeChain {
                    product: self.item_name(output).to_string(),
                    steps,
                })
            })
            .collect();
        chains.sort_by(|a, b| (a.steps.len(), &a.product).cmp(&(b.steps.len(), &b.product)));
        chains
    }

    /// Appends the steps that produce `item`, dependencies first, or nothing
    /// at all if it is a drop rather than something made.
    ///
    /// `seen` is marked *before* recursing, which is what makes a mod recipe
    /// naming itself terminate instead of taking the process down — the same
    /// contract as a malformed `.ron` being skipped rather than panicking.
    /// It doubles as the de-duplicator: two branches needing the same
    /// intermediate list it once, under the first branch that reaches it.
    fn expand_recipe(&self, item: &ItemId, seen: &mut Vec<ItemId>, out: &mut Vec<RecipeStep>) {
        if seen.contains(item) {
            return;
        }
        seen.push(item.clone());
        let items = self.world.resource::<ItemDb>();
        let Some(recipe) = items.get(item.as_str()).and_then(|d| d.craftable.as_ref()) else {
            return;
        };
        let cost = recipe.cost.clone();
        let maker = recipe.requires_structure.clone();
        for (id, _) in &cost {
            self.expand_recipe(id, seen, out);
        }
        out.push(RecipeStep {
            inputs: self.named_costs(&cost),
            maker: maker.and_then(|s| {
                self.world
                    .resource::<StructureDb>()
                    .get(s.as_str())
                    .map(|d| d.name.clone())
            }),
            output: self.item_name(item).to_string(),
        });
    }

    fn named_costs(&self, cost: &[(ItemId, u32)]) -> Vec<(String, u32)> {
        cost.iter()
            .map(|(id, q)| (self.item_name(id).to_string(), *q))
            .collect()
    }

    /// Which group this item lists under. An id with no definition behind it
    /// sorts as salvage rather than panicking, matching `item_name`'s habit
    /// of falling back to the raw id: a list is not the place to discover a
    /// broken mod.
    pub fn item_category(&self, id: &ItemId) -> ItemCategory {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .map(|d| d.category())
            .unwrap_or(ItemCategory::Material)
    }

    /// Sort key putting a list in category order, then alphabetical inside a
    /// category. The one place that ordering is decided, so the inventory
    /// screen and a trader's shelf cannot disagree about it.
    pub(crate) fn category_sort_key(&self, id: &ItemId) -> (ItemCategory, String) {
        (self.item_category(id), self.item_name(id).to_string())
    }

    /// What one unit of `id` is worth in trade currency, before a trader's
    /// own `sell_rate` markup. Falls back to `tuning::DEFAULT_ITEM_VALUE`
    /// for an item priced by no file — a mod written before the field
    /// existed, or an id the current item set doesn't define at all.
    ///
    /// The one place a price is decided, for the reason `category_sort_key`
    /// is: `sell_item`, `buyback_unit_cost` and the trade screen all read
    /// it, and a screen that quoted a price the sale then didn't honour is
    /// worse than either number being wrong on its own.
    pub fn item_value(&self, id: &ItemId) -> u32 {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .and_then(|def| def.value)
            .unwrap_or(tuning::DEFAULT_ITEM_VALUE)
    }

    /// The item's authored description, straight out of its `.ron` file.
    ///
    /// Deliberately *not* derived the way `item_blurb` is: this is prose a
    /// modder writes and can edit without touching Rust, which is the whole
    /// point of it living in the asset. `None` for an item the current item
    /// set doesn't define, or one whose file leaves the field blank.
    pub fn item_description(&self, id: &ItemId) -> Option<&str> {
        let def = self.world.resource::<ItemDb>().get(id.as_str())?;
        (!def.description.is_empty()).then_some(def.description.as_str())
    }

    /// A two-or-three word gloss of what an item *does*, for menus that list
    /// items by name and cost without saying why you'd want one.
    ///
    /// Derived from the item's own definition rather than authored per item,
    /// so a modded item gets one for free and no blurb can drift out of step
    /// with the mechanics it describes. `None` for an item whose definition
    /// says nothing worth glossing — a plain currency reads fine as itself.
    pub fn item_blurb(&self, id: &ItemId) -> Option<String> {
        let def = self.world.resource::<ItemDb>().get(id.as_str())?;
        if let Some((slot, stats)) = &def.equipment {
            let mut parts = Vec::new();
            if stats.atk != 0 {
                parts.push(format!("+{} atk", stats.atk));
            }
            if stats.def != 0 {
                parts.push(format!("+{} def", stats.def));
            }
            if stats.decompiler != 0 {
                parts.push(format!("+{} decomp", stats.decompiler));
            }
            return Some(if parts.is_empty() {
                slot.label().to_string()
            } else {
                parts.join(" ")
            });
        }
        if let Some(c) = &def.consume {
            let mut parts = Vec::new();
            if c.power != 0.0 {
                parts.push(format!("+{:.0} power", c.power));
            }
            if c.fatigue != 0.0 {
                parts.push(format!("+{:.0} rest", c.fatigue));
            }
            if c.heal != 0 {
                parts.push(format!("+{} HP", c.heal));
            }
            if c.prebattle_buff.is_some() {
                parts.push("pre-battle buff".to_string());
            }
            if !parts.is_empty() {
                return Some(parts.join(" "));
            }
        }
        if def.taming_potency.is_some() {
            return Some("taming catalyst".to_string());
        }
        None
    }

    pub fn is_equippable(&self, id: &ItemId) -> bool {
        self.equipment_of(id).is_some()
    }

    pub fn equipment_of(&self, id: &ItemId) -> Option<(EquipmentSlot, EquipmentStats)> {
        self.world.resource::<ItemDb>().get(id.as_str())?.equipment
    }

    pub fn is_consumable(&self, id: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .is_some_and(|d| d.consume.is_some())
    }

    pub fn bank_limit_of(&self, id: &ItemId) -> Option<u32> {
        self.world.resource::<ItemDb>().get(id.as_str())?.bank_limit
    }

    pub fn currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .currency()
            .expect("validated at startup")
            .clone()
    }

    pub fn research_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .research_currency()
            .expect("validated at startup")
            .clone()
    }

    pub fn craft_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .craft_currency()
            .expect("validated at startup")
            .clone()
    }

    /// What every trader pays and charges — see `EconomyRole::TradeCurrency`.
    /// Distinct from `currency`, which is the salvage the build economy runs
    /// on and which no trader deals in.
    pub fn trade_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .trade_currency()
            .expect("validated at startup")
            .clone()
    }

    /// Whether `structure_id` may be built right now. A structure named by
    /// no research file is unlocked by default — that's what keeps Home, the
    /// Mining Node, the Research Node, the Recharger Node and the Zone
    /// Portal available from turn one without a hardcoded whitelist, and
    /// what keeps a structure mod that ships no research file working
    /// unchanged.
    pub(crate) fn structure_unlocked(&self, structure_id: &str) -> bool {
        let db = self.world.resource::<ResearchDb>();
        let mut gates = db
            .all()
            .filter(|def| def.unlocks_structures.iter().any(|s| s == structure_id))
            .peekable();
        if gates.peek().is_none() {
            return true;
        }
        gates.any(|def| self.is_researched(&def.id))
    }

    /// The structures the build menu offers: `structure_defs` minus anything
    /// still behind unfinished research. `structure_defs` itself stays
    /// unfiltered — it's the general lookup, not the menu.
    pub fn buildable_structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| self.structure_unlocked(&def.id))
            .cloned()
            .collect()
    }

    /// How many units of cargo the player can carry right now: the base
    /// How many tamed programs the active battle party can hold right now:
    /// How many tamed programs the player may own in total right now:
    /// `BASE_PET_CAPACITY` plus every deployed structure's `pet_slot_bonus`
    /// (a Data Cache adds two). Derived on each call rather than cached, so a
    /// cache lost to a raid shrinks the limit with no invalidation step and
    /// the save format stays unchanged.
    pub fn pet_capacity(&self) -> usize {
        let kinds: Vec<StructureId> = self
            .world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>().map(|s| s.kind.clone()))
            .collect();
        let db = self.world.resource::<StructureDb>();
        let bonus: u32 = kinds
            .iter()
            .filter_map(|k| db.get(k.as_str()))
            .map(|def| def.pet_slot_bonus)
            .sum();
        BASE_PET_CAPACITY + bonus as usize
    }

    /// How many tamed programs the player currently owns, wherever they are —
    /// active party, cronjob workers, and idle pets all count against
    /// `pet_capacity`.
    pub fn pet_count(&self) -> usize {
        let player = self.player_entity();
        self.world
            .iter_entities()
            .filter(|e| e.get::<Tamed>().is_some_and(|t| t.owner == player))
            .count()
    }

    /// Units of cargo currently carried, excluding banked currency.
    pub fn inventory_used(&self) -> u32 {
        let db = self.world.resource::<ItemDb>();
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.cargo_used(db))
            .unwrap_or(0)
    }

    /// `Ok(())` if `qty` more of `item` would fit. Ordinary cargo (the
    /// Buffer) is unbounded, so this only ever refuses a banked currency (an
    /// item with `ItemDef::bank_limit`, e.g. Research Data) that would exceed
    /// its own separate cap. Used by the paths where the player pays an input
    /// cost — compiling, buying, unequipping — since letting a bank overflow
    /// would destroy value the player already spent.
    pub(crate) fn check_room(&self, item: &ItemId, qty: u32) -> Result<(), String> {
        let db = self.world.resource::<ItemDb>();
        let Some(limit) = db.get(item.as_str()).and_then(|d| d.bank_limit) else {
            return Ok(());
        };
        let used = self
            .world
            .get::<Inventory>(self.player_entity())
            .unwrap()
            .count(item);
        if used.saturating_add(qty) > limit {
            return Err(format!("Research bank full ({used}/{limit})."));
        }
        Ok(())
    }

    /// The actual item cost to deploy `def` right now: `def.build_cost`
    /// unchanged for a normal structure, or each amount grown by
    /// `ZONE_PORTAL_COST_GROWTH_PERCENT` of its base rate per zone level for
    /// a zone-portal structure (see `StructureDef::zone_portal`) — breaching
    /// deeper costs more raw material each time.
    pub fn structure_build_cost(&self, def: &StructureDef) -> Vec<(ItemId, u32)> {
        if !def.zone_portal {
            return def.build_cost.clone();
        }
        let zone = self.world.resource::<ZoneLevel>().0;
        def.build_cost
            .iter()
            .map(|(item, qty)| (item.clone(), zone_portal_cost(*qty, zone)))
            .collect()
    }

    pub fn species_defs(&self) -> Vec<SpeciesDef> {
        self.world.resource::<SpeciesDb>().all().cloned().collect()
    }

    /// Every perk currently on offer, in picker order. The renderer's only
    /// route to a perk's name, description and price — those are authored in
    /// `assets/perks/*.ron`, not derivable from the `Perk` variant, and the
    /// index into this list is what `unlock_perk` expects back.
    pub fn perk_defs(&self) -> Vec<PerkDef> {
        self.world
            .resource::<PerkDb>()
            .catalogue()
            .cloned()
            .collect()
    }
}
