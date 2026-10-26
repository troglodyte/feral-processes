//! What the Workbench's weapon-crafting pages draw (ADR 0031): the designs
//! list, the new-design rows and part picker, the live preview, and the
//! weapon parts list. Every figure is read off the engine, so a page can
//! never promise a part the build would refuse.

use crate::affixes::AffixId;
use crate::gear_craft::weapon::Category;
use crate::gear_craft::{GearCraftRefusal, WeaponPart};
use crate::items::{EquipmentStats, GearCopy};

/// One row of the Forge's designs list.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignRow {
    pub id: u32,
    pub name: String,
    /// The parts, labelled, frame first, then any affixes.
    pub parts: Vec<String>,
    pub description: String,
    /// `design_power`, the rating the ceiling is held to.
    pub power: Option<i32>,
    /// Why Build would be refused right now, or `None`.
    pub blocked: Option<GearCraftRefusal>,
}

/// One row of the new-design page: a category and what fills it.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignSlotRow {
    pub category: Category,
    pub chosen: Option<WeaponPart>,
}

/// One entry of a category's highlight-and-Enter list.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignPartOption {
    pub part: WeaponPart,
    pub label: String,
    pub held: u32,
    /// Why it is greyed: it does not fit the design as it stands, or the
    /// player has neither the research nor a copy.
    pub unavailable: Option<GearCraftRefusal>,
}

/// One affix the player may put in a design.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignAffixOption {
    pub affix: AffixId,
    /// Whether the design already carries it.
    pub chosen: bool,
}

/// The live preview under the new-design rows.
#[derive(Clone, Debug)]
pub struct DesignPreview {
    /// The generated plain-language description; empty without a legal fit.
    pub description: String,
    /// What a fresh copy is worth at the reference zone — the stats the
    /// power rating is read from, so the page and the rating agree.
    pub stats: Option<EquipmentStats>,
    /// Swing range in cells.
    pub range: u32,
    /// `design_power`, `None` for a design that does not compile.
    pub power: Option<i32>,
    /// Parts held and how many each, labelled.
    pub parts_used: Vec<(String, u32)>,
    /// Parts needed and not held, with how many short.
    pub parts_missing: Vec<(String, u32)>,
    /// Modifiers in the design (affixes included) and the most it may hold.
    pub part_count: usize,
    pub part_cap: usize,
    /// Why saving or building would be refused, or `None`.
    pub blocked: Option<GearCraftRefusal>,
}

/// One row of the Weapon parts list.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponPartRow {
    pub part: WeaponPart,
    pub label: String,
    pub category: Category,
    pub held: u32,
    /// The material a craft spends and how many.
    pub material: String,
    pub material_count: u32,
    /// Whether the player has the material in reach.
    pub affordable: bool,
    /// Whether the part's research is done; a locked part is listed, greyed.
    pub known: bool,
}

/// One row of the Salvage page: a pack copy that can be taken apart.
#[derive(Clone, Debug, PartialEq)]
pub struct SalvageRow {
    pub copy: GearCopy,
    pub name: String,
    pub qty: u32,
    /// What taking one apart hands out, labelled.
    pub parts: Vec<String>,
}
