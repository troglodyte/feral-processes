//! Weapon crafting's verbs (ADR 0031): the Workbench side of a design.
//! Crafting spends a material for a part; a design is a saved recipe; a
//! build spends the design's parts and the frame's bill for one ordinary
//! copy of its weapon.
//!
//! Each verb is gather, decide, apply. The decisions are `gear_craft`'s pure
//! `check_design` and `plan_build`, so the Forge page previews exactly what
//! these refuse and spend. Every refusal is typed (ADR 0027) and lands
//! before anything is spent.

use crate::affixes::{AffixDb, AffixId};
use crate::base_ledger::LootSource;
use crate::components::{Equipment, FloorItem, GearCopies};
use crate::game::gear_power::ReferenceFoe;
use crate::gear_craft::weapon::Category;
use crate::gear_craft::{
    self, DesignHolder, GearCraftRefusal, GearDesign, GearPart, WeaponPart, capacity_node_id,
};
use crate::items::FloorThing;
use crate::resources::{BuybackLedger, GearDesigns, KnownAffixes, Research};
use crate::tuning::{GEAR_PART_CAP_MAX, GEAR_PART_CAP_START};
use crate::views::{
    DesignAffixOption, DesignBill, DesignPartOption, DesignPreview, DesignRow, DesignSlotRow,
    MaterialLine, SalvageRow,
};
use crate::*;

/// Why a copy cannot be fused. Typed per ADR 0027; `fuse_copy` words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FusionRefusal {
    /// Fusion is for drops only: a weapon built at the Workbench never fuses.
    CraftedGear,
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

    /// Saves a weapon design named `name` from `parts`, returning its id.
    /// The parts are not spent: a design is a recipe, a build spends.
    pub fn create_design(
        &mut self,
        name: &str,
        parts: Vec<GearPart>,
    ) -> Result<u32, GearCraftRefusal> {
        self.require_forge_free()?;
        let name = gear_craft::check_design(
            name,
            &parts,
            self.gear_part_cap(),
            |p| self.weapon_part_usable(p),
            |a| self.affix_known(a),
            self.world.resource::<AffixDb>(),
        )?;
        let mut designs = self.world.resource_mut::<GearDesigns>();
        designs.next_id += 1;
        let id = designs.next_id;
        designs.designs.push(GearDesign {
            id,
            name: name.clone(),
            slot: EquipmentSlot::Weapon,
            parts,
        });
        self.sync_crafted_gear();
        self.log(format!("You save the design {name}."));
        Ok(id)
    }

    /// Builds one copy of design `id`: spends the parts held, crafts the
    /// rest from materials, pays the frame's bill, and mints one Ordinary,
    /// tier-0 copy into cargo. All or nothing.
    pub fn build_design(&mut self, id: u32) -> Result<GearCopy, GearCraftRefusal> {
        self.require_forge_free()?;
        let design = self.design(id).ok_or(GearCraftRefusal::NoSuchDesign(id))?;
        let plan = self.plan_design(&design)?;
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
        let copy = GearCopy::plain(gear_craft::weapon::item_id(id));
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

    /// `gear_craft::plan_build` over what is in reach now: the one decision
    /// `build_design` acts on and the Forge pages report.
    fn plan_design(&self, design: &GearDesign) -> Result<gear_craft::BuildPlan, GearCraftRefusal> {
        gear_craft::plan_build(&gear_craft::BuildInput {
            design,
            held: &|p| self.weapon_parts_held(p),
            known: &|p| self.weapon_part_known(p),
            in_reach: &|i| self.held_in_reach(i),
        })
    }

    /// What Forge would spend on design `id` from materials alone, and what
    /// it can never craft. Shown even when the build would be refused, so
    /// every shortfall is on the page at once.
    pub fn design_bill(&self, id: u32) -> Option<DesignBill> {
        let design = self.design(id)?;
        Some(self.bill_of(&design))
    }

    fn bill_of(&self, design: &GearDesign) -> DesignBill {
        if design.frame().is_none() {
            return DesignBill {
                materials: Vec::new(),
                uncraftable: Vec::new(),
            };
        }
        let (_, craft) = gear_craft::split_held(design, &|p| self.weapon_parts_held(p));
        DesignBill {
            materials: gear_craft::material_bill(design, &craft)
                .iter()
                .map(|(item, need)| MaterialLine {
                    label: self.item_name(item).to_string(),
                    have: self.held_in_reach(item),
                    need: *need,
                })
                .collect(),
            uncraftable: gear_craft::uncraftable(&craft, &|p| self.weapon_part_known(p))
                .into_iter()
                .map(|part| match part {
                    WeaponPart::Proc(_) => format!("{} — salvage a weapon first", part.label()),
                    _ => format!("{} — research first", part.label()),
                })
                .collect(),
        }
    }

    /// Deletes design `id`, which is refused while a copy of its weapon
    /// exists anywhere: a copy whose item has no def would be stranded.
    pub fn delete_design(&mut self, id: u32) -> Result<(), GearCraftRefusal> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err(GearCraftRefusal::Busy);
        }
        let design = self.design(id).ok_or(GearCraftRefusal::NoSuchDesign(id))?;
        if let Some(holder) = self.design_holder(id) {
            return Err(GearCraftRefusal::InUse(holder));
        }
        self.world
            .resource_mut::<GearDesigns>()
            .designs
            .retain(|d| d.id != id);
        self.sync_crafted_gear();
        self.log(format!("You delete the design {}.", design.name));
        Ok(())
    }

    fn design(&self, id: u32) -> Option<GearDesign> {
        self.world
            .resource::<GearDesigns>()
            .designs
            .iter()
            .find(|d| d.id == id)
            .cloned()
    }

    /// Where a copy of design `id`'s weapon is, if one is anywhere: the
    /// pack or gear ledger, worn by the player, worn by a companion, a
    /// trader's buyback shelf, set down on a floor tile, in a Depot or
    /// machine buffer, or in a body's hands.
    pub fn design_holder(&self, id: u32) -> Option<DesignHolder> {
        let item = gear_craft::weapon::item_id(id);
        let player = self.player_entity();
        let in_pack = self
            .world
            .get::<Inventory>(player)
            .is_some_and(|inv| inv.count(&item) > 0)
            || self
                .world
                .get::<GearCopies>(player)
                .is_some_and(|c| c.copies.iter().any(|(copy, _)| copy.item == item));
        if in_pack {
            return Some(DesignHolder::Pack);
        }
        let wears = |equipment: &Equipment| {
            EquipmentSlot::ALL
                .into_iter()
                .filter_map(|slot| equipment.get(slot))
                .any(|worn| worn.copy.item == item)
        };
        if self.world.get::<Equipment>(player).is_some_and(wears) {
            return Some(DesignHolder::Worn);
        }
        let companion_wears = self
            .world
            .iter_entities()
            .filter(|e| e.id() != player)
            .any(|e| e.get::<Equipment>().is_some_and(wears));
        if companion_wears {
            return Some(DesignHolder::Companion);
        }
        let shelved = self
            .world
            .resource::<BuybackLedger>()
            .0
            .values()
            .flatten()
            .any(|(copy, _)| copy.item == item);
        if shelved {
            return Some(DesignHolder::Buyback);
        }
        let on_floor = self
            .world
            .iter_entities()
            .filter_map(|e| e.get::<FloorItem>())
            .any(|floor| match &floor.0 {
                FloorThing::Item(i) => *i == item,
                FloorThing::Gear(copy) => copy.item == item,
                FloorThing::Program(_) => false,
            });
        if on_floor {
            return Some(DesignHolder::Floor);
        }
        // `hauling::pickup_errand` moves a floor copy into any unfiltered
        // Depot, and a raider or hauler holds one in `Carrying` mid-trip.
        let stored = self.world.iter_entities().any(|e| {
            e.get::<Stock>()
                .is_some_and(|s| s.output.contains_key(&item) || s.input.contains_key(&item))
        });
        if stored {
            return Some(DesignHolder::Stored);
        }
        self.world
            .iter_entities()
            .any(|e| {
                e.get::<Carrying>()
                    .is_some_and(|c| c.item == item && c.qty > 0)
            })
            .then_some(DesignHolder::Carried)
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

/// The views the Forge pages draw.
impl Game {
    /// `design_power` of a def, priced against the reference foe.
    fn design_rating(&self, def: &crate::items_db::ItemDef) -> Option<i32> {
        let foe = ReferenceFoe::median(self.world.resource::<SpeciesDb>());
        gear_craft::design_power(def, &foe)
    }

    /// The Forge's designs list: each design with its parts, what it reads
    /// as, how strong it is and whether a build would be refused now.
    pub fn design_rows(&self) -> Vec<DesignRow> {
        let items = self.world.resource::<ItemDb>();
        self.gear_designs()
            .into_iter()
            .map(|design| {
                let def = items.get(gear_craft::weapon::item_id(design.id).as_str());
                let mut parts: Vec<String> =
                    design.weapon_parts().iter().map(|p| p.label()).collect();
                parts.extend(design.affixes().iter().map(|a| a.0.clone()));
                DesignRow {
                    id: design.id,
                    name: design.name.clone(),
                    parts,
                    description: def.map(|d| d.description.clone()).unwrap_or_default(),
                    power: def.and_then(|d| self.design_rating(d)),
                    blocked: self.build_block(&design),
                }
            })
            .collect()
    }

    /// Why building `design` would be refused now, or `None`.
    fn build_block(&self, design: &GearDesign) -> Option<GearCraftRefusal> {
        self.require_forge_free()
            .err()
            .or_else(|| self.plan_design(design).err())
    }

    /// The new-design page's rows: the frame and each modifier category,
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

    /// The list a category's row opens: every part of it, greyed with the
    /// reason when it does not fit the design as it stands (choosing it
    /// replaces what already fills the category) or is not yet usable.
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
            .map(|part| {
                let mut with = rest.clone();
                with.push(part);
                let unavailable = if !self.weapon_part_usable(part) {
                    Some(GearCraftRefusal::PartNotResearched(part))
                } else if category != Category::Frame
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

    /// The new-design page's preview: exactly what `create_design` and
    /// `build_design` would refuse and spend for this name and these parts.
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
                (have < *need).then_some((*p, need - have))
            })
            .collect();
        let bill = self.bill_of(&design);
        let blocked = gear_craft::check_design(
            name,
            parts,
            cap,
            |p| self.weapon_part_usable(p),
            |a| self.affix_known(a),
            self.world.resource::<AffixDb>(),
        )
        .err();
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
            materials: bill.materials,
            uncraftable: bill.uncraftable,
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
