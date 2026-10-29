//! Weapon crafting's verbs (ADR 0031): the Workbench side of a design.
//! Crafting spends a material for a part; a design is a saved recipe; a
//! build spends the design's parts and the frame's bill for one ordinary
//! copy of its weapon.
//!
//! Each verb is gather, decide, apply. The decisions are `gear_craft`'s pure
//! `check_design` and `plan_build`, so the Build weapon page previews exactly what
//! these refuse and spend. Every refusal is typed (ADR 0027) and lands
//! before anything is spent.

use crate::affixes::{AffixDb, AffixId};
use crate::base_ledger::LootSource;
use crate::game::gear_power::ReferenceFoe;
use crate::gear_craft::weapon::Category;
use crate::gear_craft::{
    self, GearCraftRefusal, GearDesign, GearPart, WeaponPart, capacity_node_id,
};
use crate::resources::{GearDesigns, KnownAffixes, Research};
use crate::tuning::{GEAR_PART_CAP_MAX, GEAR_PART_CAP_START};
use crate::views::{
    DesignAffixOption, DesignPartOption, DesignPreview, DesignSlotRow, MaterialLine, SalvageRow,
};
use crate::*;

/// Why a copy cannot be fused. Typed per ADR 0027; `fuse_copy` words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FusionRefusal {
    /// Fusion is for drops only: a weapon built at the Workbench never fuses.
    CraftedGear,
}

/// What `Game::decide_compile` settled on.
struct CompileDecision {
    design: GearDesign,
    /// No stored design matches, so a compile stores this one.
    is_new: bool,
    plan: gear_craft::BuildPlan,
}

impl Game {
    /// How many modifiers a design may hold now: the start cap raised by one
    /// for each capacity node researched. Derived from `Research`, so there
    /// is no second copy of it to drift.
    pub fn gear_part_cap(&self) -> usize {
        let research = self.world.resource::<Research>();
        ((GEAR_PART_CAP_START + 1)..=GEAR_PART_CAP_MAX)
            .take_while(|cap| research.0.contains(&capacity_node_id(*cap)))
            .last()
            .unwrap_or(GEAR_PART_CAP_START)
    }

    /// Whether `part`'s research is done, which lets the player craft it.
    pub fn weapon_part_known(&self, part: WeaponPart) -> bool {
        self.world
            .resource::<Research>()
            .0
            .contains(&part.node_id())
    }

    /// How many of `part` the Workbench can draw on.
    pub fn weapon_parts_held(&self, part: WeaponPart) -> u32 {
        self.held_in_reach(&ItemId::weapon_part(part))
    }

    /// Whether a design may carry `affix`: its `affix:<id>` node is
    /// researched, or it was salvaged off a copy that carried it.
    pub fn affix_known(&self, affix: &AffixId) -> bool {
        self.world.resource::<KnownAffixes>().0.contains(affix)
            || self
                .world
                .resource::<Research>()
                .0
                .contains(&crate::affix_tree::node_id(&affix.0))
    }

    /// A part can go into a design once it is researched or one is held.
    fn weapon_part_usable(&self, part: WeaponPart) -> bool {
        self.weapon_part_known(part) || self.weapon_parts_held(part) > 0
    }

    /// The designs the player has made, in the order they were made.
    pub fn gear_designs(&self) -> Vec<GearDesign> {
        self.world.resource::<GearDesigns>().designs.clone()
    }

    fn require_forge_free(&self) -> Result<(), GearCraftRefusal> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err(GearCraftRefusal::Busy);
        }
        if !self.can_extract_routines() {
            return Err(GearCraftRefusal::NoWorkbench);
        }
        Ok(())
    }

    /// Pays `bill` from reach, refusing the whole thing, with the line that
    /// is short, before a unit moves. Lines naming one item are summed first,
    /// as `pay_from_reach` sums them, so the two cannot disagree on a bill.
    pub(crate) fn pay_gear_bill(&mut self, bill: &[(ItemId, u32)]) -> Result<(), GearCraftRefusal> {
        let mut need: std::collections::BTreeMap<&ItemId, u32> = std::collections::BTreeMap::new();
        for (item, qty) in bill {
            *need.entry(item).or_default() += qty;
        }
        for (item, need) in &need {
            let held = self.held_in_reach(item);
            if held < *need {
                return Err(GearCraftRefusal::ShortOf {
                    item: (*item).clone(),
                    need: *need,
                    held,
                });
            }
        }
        // Checked above, so the payment cannot find a line short.
        self.pay_from_reach(bill)
            .expect("the whole bill was checked against reach");
        Ok(())
    }

    /// Takes an authored weapon copy from the pack apart: it is consumed,
    /// the parts `decompose` maps it to are handed out and their research
    /// nodes discovered, and the affixes it carried become known to the
    /// design picker. The only source of a Proc part.
    ///
    /// Gather, decide, apply: every refusal lands before the copy moves.
    pub fn salvage_weapon(&mut self, copy: &GearCopy) -> Result<Vec<WeaponPart>, GearCraftRefusal> {
        self.require_forge_free()?;
        if !self.weapon_tree_open() {
            return Err(GearCraftRefusal::WeaponTreeClosed);
        }
        if self.count_copies(copy) == 0 {
            return Err(GearCraftRefusal::NoCopyHeld);
        }
        let parts = match copy.item.crafted_gear_of() {
            Some(_) => None,
            None => self
                .world
                .resource::<ItemDb>()
                .get(copy.item.as_str())
                .and_then(gear_craft::weapon::decompose),
        }
        .ok_or(GearCraftRefusal::NotSalvageable)?;
        self.take_copies(copy, 1);
        for part in &parts {
            self.grant_loot(ItemId::weapon_part(*part), 1, LootSource::Salvage);
            self.discover_research(&part.node_id());
        }
        self.world
            .resource_mut::<KnownAffixes>()
            .0
            .extend(copy.affixes.iter().cloned());
        let name = self.item_name(&copy.item).to_string();
        self.log(format!("You take {name} apart for its parts."));
        Ok(parts)
    }

    /// Compiles one ordinary copy of the weapon `parts` make, named `name`
    /// (ADR 0031): spends the parts held, crafts the rest from materials,
    /// pays the frame's bill and mints one tier-0 copy into cargo. All or
    /// nothing. A design with this name and these parts is reused, so two
    /// compiles of one weapon share an item id; otherwise one is stored,
    /// and only after every refusal has been ruled out.
    pub fn compile_weapon(
        &mut self,
        name: &str,
        parts: Vec<GearPart>,
    ) -> Result<GearCopy, GearCraftRefusal> {
        let CompileDecision {
            design,
            is_new,
            plan,
        } = self.decide_compile(name, &parts)?;
        if is_new {
            self.store_design(design.clone());
        }
        self.build_planned(&design, &plan)
    }

    /// Everything `compile_weapon` would refuse, in the order it refuses,
    /// and the plan it would spend when it does not. The preview reads the
    /// same call, so the page cannot promise a compile the engine declines.
    fn decide_compile(
        &self,
        name: &str,
        parts: &[GearPart],
    ) -> Result<CompileDecision, GearCraftRefusal> {
        self.require_forge_free()?;
        let name = self.check_weapon_design(name, parts)?;
        let stored = self
            .world
            .resource::<GearDesigns>()
            .designs
            .iter()
            .find(|d| d.name == name && d.parts == parts)
            .cloned();
        let is_new = stored.is_none();
        let design = stored.unwrap_or_else(|| self.next_design(name, parts.to_vec()));
        let plan = self.plan_design(&design)?;
        Ok(CompileDecision {
            design,
            is_new,
            plan,
        })
    }

    fn check_weapon_design(
        &self,
        name: &str,
        parts: &[GearPart],
    ) -> Result<String, GearCraftRefusal> {
        gear_craft::check_design(
            name,
            parts,
            self.gear_part_cap(),
            |p| self.weapon_part_usable(p),
            |a| self.affix_known(a),
            self.world.resource::<AffixDb>(),
        )
    }

    /// The design `name` and `parts` would become, under the next free id.
    fn next_design(&self, name: String, parts: Vec<GearPart>) -> GearDesign {
        GearDesign {
            id: self.world.resource::<GearDesigns>().next_id + 1,
            name,
            slot: EquipmentSlot::Weapon,
            parts,
        }
    }

    fn store_design(&mut self, design: GearDesign) {
        let mut designs = self.world.resource_mut::<GearDesigns>();
        designs.next_id = design.id;
        designs.designs.push(design);
        self.sync_crafted_gear();
    }

    /// Spends `plan` and mints the copy of `design`.
    fn build_planned(
        &mut self,
        design: &GearDesign,
        plan: &gear_craft::BuildPlan,
    ) -> Result<GearCopy, GearCraftRefusal> {
        self.pay_gear_bill(&plan.bill)?;
        // Crafted parts pass through the pack so each is receipted as a
        // craft, the way crafting one by hand was.
        for (part, count) in &plan.craft {
            self.grant_loot(ItemId::weapon_part(*part), *count, LootSource::Craft);
        }
        let parts: Vec<(ItemId, u32)> = plan
            .spend
            .iter()
            .chain(&plan.craft)
            .map(|(part, count)| (ItemId::weapon_part(*part), *count))
            .collect();
        self.pay_gear_bill(&parts)?;
        let copy = GearCopy::plain(gear_craft::weapon::item_id(design.id));
        self.grant_loot(copy.item.clone(), 1, LootSource::Craft);
        let name = &design.name;
        let crafted: u32 = plan.craft.iter().map(|(_, n)| n).sum();
        self.log(match crafted {
            0 => format!("You build a {name}."),
            1 => format!("You compile 1 missing part and build a {name}."),
            n => format!("You compile {n} missing parts and build a {name}."),
        });
        Ok(copy)
    }

    /// Stores a design without building it, for tests that need one to
    /// exist before its weapon does.
    #[cfg(test)]
    pub(crate) fn create_design(
        &mut self,
        name: &str,
        parts: Vec<GearPart>,
    ) -> Result<u32, GearCraftRefusal> {
        self.require_forge_free()?;
        let name = self.check_weapon_design(name, &parts)?;
        let design = self.next_design(name, parts);
        let id = design.id;
        self.store_design(design);
        Ok(id)
    }

    /// Builds a stored design, for tests that made it with `create_design`.
    #[cfg(test)]
    pub(crate) fn build_design(&mut self, id: u32) -> Result<GearCopy, GearCraftRefusal> {
        self.require_forge_free()?;
        let design = self.design(id).expect("the test made this design");
        let plan = self.plan_design(&design)?;
        self.build_planned(&design, &plan)
    }

    /// `gear_craft::plan_build` over what is in reach now: the one decision
    /// `compile_weapon` acts on and the Build weapon page reports.
    fn plan_design(&self, design: &GearDesign) -> Result<gear_craft::BuildPlan, GearCraftRefusal> {
        gear_craft::plan_build(&gear_craft::BuildInput {
            design,
            held: &|p| self.weapon_parts_held(p),
            known: &|p| self.weapon_part_known(p),
            in_reach: &|i| self.held_in_reach(i),
        })
    }

    fn bill_of(&self, design: &GearDesign) -> (Vec<MaterialLine>, Vec<String>) {
        if design.frame().is_none() {
            return (Vec::new(), Vec::new());
        }
        let (_, craft) = gear_craft::split_held(design, &|p| self.weapon_parts_held(p));
        let materials = gear_craft::material_bill(design, &craft)
            .iter()
            .map(|(item, need)| MaterialLine {
                label: self.item_name(item).to_string(),
                have: self.held_in_reach(item),
                need: *need,
            })
            .collect();
        let uncraftable = gear_craft::uncraftable(&craft, &|p| self.weapon_part_known(p))
            .into_iter()
            .map(|part| match part {
                WeaponPart::Proc(_) => format!("{} — salvage a weapon first", part.label()),
                _ => format!("{} — research first", part.label()),
            })
            .collect();
        (materials, uncraftable)
    }

    #[cfg(test)]
    fn design(&self, id: u32) -> Option<GearDesign> {
        self.world
            .resource::<GearDesigns>()
            .designs
            .iter()
            .find(|d| d.id == id)
            .cloned()
    }

    /// Why `copy` cannot be fused, if it cannot: fusion is for drops only.
    pub fn fusion_refusal(&self, copy: &GearCopy) -> Option<FusionRefusal> {
        copy.item
            .crafted_gear_of()
            .map(|_| FusionRefusal::CraftedGear)
    }

    /// Rebuilds `ItemDb`'s crafted tier from the surviving designs, so no
    /// def outlives its design.
    pub(crate) fn sync_crafted_gear(&mut self) {
        let designs = self.world.resource::<GearDesigns>().designs.clone();
        self.world.resource_scope(|world, mut items: Mut<ItemDb>| {
            items.set_crafted_gear(&designs, world.resource::<AffixDb>());
        });
    }
}

/// The views the Build weapon page draws.
impl Game {
    /// `design_power` of a def, priced against the reference foe.
    fn design_rating(&self, def: &crate::items_db::ItemDef) -> Option<i32> {
        let foe = ReferenceFoe::median(self.world.resource::<SpeciesDb>());
        gear_craft::design_power(def, &foe)
    }

    /// The Build weapon page's rows: the frame and each modifier category,
    /// with what the design in progress puts there.
    pub fn design_slot_rows(&self, parts: &[WeaponPart]) -> Vec<DesignSlotRow> {
        std::iter::once(Category::Frame)
            .chain(Category::MODIFIERS)
            .map(|category| DesignSlotRow {
                category,
                chosen: parts.iter().copied().find(|p| p.category() == category),
            })
            .collect()
    }

    /// The list a category's row opens: every usable part of it (researched
    /// or held), greyed with the reason when it does not fit the design as
    /// it stands (choosing it replaces what already fills the category).
    pub fn design_part_options(
        &self,
        category: Category,
        parts: &[WeaponPart],
        affixes: usize,
    ) -> Vec<DesignPartOption> {
        let rest: Vec<WeaponPart> = parts
            .iter()
            .copied()
            .filter(|p| p.category() != category)
            .collect();
        WeaponPart::options(category)
            .into_iter()
            .filter(|part| self.weapon_part_usable(*part))
            .map(|part| {
                let mut with = rest.clone();
                with.push(part);
                let unavailable = if category != Category::Frame
                    && gear_craft::weapon::frame_of(&with).is_err()
                {
                    // No frame yet: the modifier's own fit is judged once
                    // a frame is picked.
                    None
                } else {
                    gear_craft::weapon::check(&with, affixes, self.gear_part_cap())
                        .err()
                        .map(GearCraftRefusal::Fit)
                };
                let craft_cost = (self.weapon_part_known(part)
                    && !matches!(part, WeaponPart::Proc(_)))
                .then(|| {
                    let (item, count) = gear_craft::weapon::part_bill(part);
                    (self.item_name(&item).to_string(), count)
                });
                DesignPartOption {
                    part,
                    label: part.label(),
                    held: self.weapon_parts_held(part),
                    craft_cost,
                    unavailable,
                }
            })
            .collect()
    }

    /// The affixes the player may add to a design, known ones that fit a
    /// weapon, in id order.
    pub fn design_affix_options(&self, chosen: &[AffixId]) -> Vec<DesignAffixOption> {
        self.world
            .resource::<AffixDb>()
            .all()
            .filter(|a| a.fits(EquipmentSlot::Weapon) && self.affix_known(&a.id))
            .map(|a| DesignAffixOption {
                affix: a.id.clone(),
                chosen: chosen.contains(&a.id),
            })
            .collect()
    }

    /// The Build weapon page's preview: exactly what `compile_weapon`
    /// would refuse (`decide_compile`) and spend for this name and these parts.
    pub fn design_preview(&self, name: &str, parts: &[GearPart]) -> DesignPreview {
        let cap = self.gear_part_cap();
        let design = GearDesign {
            id: 0,
            name: name.trim().to_string(),
            slot: EquipmentSlot::Weapon,
            parts: parts.to_vec(),
        };
        let def = gear_craft::compile(&design, self.world.resource::<AffixDb>()).ok();
        let stats = def.as_ref().and_then(|d| {
            let (_, stats) = d.worn_stats()?;
            let copy = GearCopy::plain(d.id.clone());
            Some(stats.for_copy(crate::tuning::POWER_REFERENCE_ZONE, &copy))
        });
        let label = |list: Vec<(WeaponPart, u32)>| -> Vec<(String, u32)> {
            list.into_iter().map(|(p, n)| (p.label(), n)).collect()
        };
        let used = gear_craft::parts_used(&design);
        let missing = used
            .iter()
            .filter_map(|(p, need)| {
                let have = self.weapon_parts_held(*p);
                (have < *need).then(|| (*p, need - have))
            })
            .collect();
        let (materials, uncraftable) = self.bill_of(&design);
        let blocked = self.decide_compile(name, parts).err();
        DesignPreview {
            description: def
                .as_ref()
                .map(|d| d.description.clone())
                .unwrap_or_default(),
            stats,
            range: def.as_ref().and_then(|d| d.range).unwrap_or(1),
            power: def.as_ref().and_then(|d| self.design_rating(d)),
            parts_used: label(used),
            parts_missing: label(missing),
            materials,
            uncraftable,
            part_count: design
                .weapon_parts()
                .iter()
                .filter(|p| p.category() != Category::Frame)
                .count()
                + design.affixes().len(),
            part_cap: cap,
            blocked,
        }
    }

    /// The Salvage page: the authored weapons in the pack, each with what
    /// taking it apart hands out. A built weapon is not listed.
    pub fn salvage_rows(&self) -> Vec<SalvageRow> {
        let items = self.world.resource::<ItemDb>();
        self.player_status()
            .inventory
            .into_iter()
            .filter(|row| row.copy.item.crafted_gear_of().is_none())
            .filter_map(|row| {
                let parts = items
                    .get(row.copy.item.as_str())
                    .and_then(gear_craft::weapon::decompose)?;
                Some(SalvageRow {
                    name: self.copy_name(&row.copy),
                    qty: row.qty,
                    parts: parts.iter().map(|p| p.label()).collect(),
                    copy: row.copy,
                })
            })
            .collect()
    }
}
