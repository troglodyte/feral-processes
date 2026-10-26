//! The Workbench's weapon-crafting pages (ADR 0031): the Forge (designs),
//! New design, Weapon parts and Salvage.
//!
//! The new-design page keeps a draft (`DesignDraft`) on `App`; every figure
//! it shows comes from `Game::design_*` views, and Save calls
//! `Game::create_design`, so the screen cannot promise what the engine would
//! refuse. Every refusal is the engine's typed `GearCraftRefusal`, worded
//! here by an exhaustive match.

use crate::app::list_filter::filter_matches;
use crate::*;
use feral_processes_engine::affixes::AffixId;
use feral_processes_engine::gear_craft::weapon::{self, Category, WeaponPart};
use feral_processes_engine::gear_craft::{
    CompileError, DesignHolder, FitError, GearCraftRefusal, GearPart,
};

/// Longest design name the name field takes.
pub use feral_processes_engine::tuning::GEAR_NAME_MAX as DESIGN_NAME_MAX;

/// The new-design page's rows: the Frame and each modifier category, then
/// the affixes, the name field and Save.
const CATEGORY_ROWS: usize = 1 + Category::MODIFIERS.len();
pub const DESIGN_AFFIX_ROW: usize = CATEGORY_ROWS;
pub const DESIGN_NAME_ROW: usize = CATEGORY_ROWS + 1;
pub const DESIGN_SAVE_ROW: usize = CATEGORY_ROWS + 2;
pub const DESIGN_PAGE_ROWS: usize = CATEGORY_ROWS + 3;

/// The category a new-design row opens a list for, or `None` for the rows
/// that are not categories.
pub fn design_row_category(row: usize) -> Option<Category> {
    match row {
        0 => Some(Category::Frame),
        r if r < CATEGORY_ROWS => Some(Category::MODIFIERS[r - 1]),
        _ => None,
    }
}

/// What `Mode::DesignPick` is listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignPicking {
    Category(Category),
    Affixes,
}

/// The weapon being designed. Session state: closing the Workbench discards
/// it, and nothing is saved until Save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DesignDraft {
    pub parts: Vec<WeaponPart>,
    pub affixes: Vec<AffixId>,
    pub name: String,
    pub picking: Option<DesignPicking>,
    /// The page row to put the highlight back on after a pick or a name.
    pub row: usize,
}

impl DesignDraft {
    /// The draft as the engine's design parts: weapon parts, then affixes.
    pub fn gear_parts(&self) -> Vec<GearPart> {
        self.parts
            .iter()
            .copied()
            .map(GearPart::Weapon)
            .chain(self.affixes.iter().cloned().map(GearPart::Affix))
            .collect()
    }
}

/// How an affix reads in a list.
pub fn affix_label(affix: &AffixId) -> String {
    affix.0.replace('_', " ")
}

/// The labels the open part list shows, in order, before the clear row.
pub fn design_pick_labels(game: &Game, draft: &DesignDraft) -> Vec<String> {
    match draft.picking {
        Some(DesignPicking::Category(category)) => game
            .design_part_options(category, &draft.parts, draft.affixes.len())
            .into_iter()
            .map(|o| o.label)
            .collect(),
        Some(DesignPicking::Affixes) => game
            .design_affix_options(&draft.affixes)
            .iter()
            .map(|o| affix_label(&o.affix))
            .collect(),
        None => Vec::new(),
    }
}

/// Indices into the open list that `filter` leaves.
pub fn design_pick_visible(game: &Game, draft: &DesignDraft, filter: &str) -> Vec<usize> {
    let labels = design_pick_labels(game, draft);
    filter_matches(labels.iter().map(String::as_str), filter)
}

fn fit_line(why: &FitError) -> String {
    match why {
        FitError::NoFrame => "Pick a Frame first.".to_string(),
        FitError::TwoFrames => "A design has one Frame.".to_string(),
        FitError::CategoryTaken(category) => {
            format!("The design already has a {}.", category.label())
        }
        FitError::OverCap { cap } => format!("A design holds at most {cap} modifiers."),
        FitError::RangeAtMax(frame) => {
            format!("A {} already swings at full range.", frame.name())
        }
        FitError::StunRider => "A stun can't be a rider.".to_string(),
    }
}

fn compile_line(why: &CompileError) -> String {
    match why {
        CompileError::Fit(fit) => fit_line(fit),
        CompileError::UnknownAffix(a) => format!("{} isn't a known affix.", affix_label(a)),
        CompileError::AffixDoesNotFit(a) => format!("{} doesn't fit a weapon.", affix_label(a)),
        CompileError::UnsupportedSlot(_) => "Only weapons can be designed so far.".to_string(),
    }
}

fn holder_line(holder: DesignHolder) -> &'static str {
    match holder {
        DesignHolder::Pack => "A weapon built from it is still in your pack.",
        DesignHolder::Worn => "A weapon built from it is still worn.",
        DesignHolder::Companion => "A weapon built from it is still worn by a companion.",
        DesignHolder::Buyback => "A weapon built from it is still on a trader's buyback shelf.",
        DesignHolder::Floor => "A weapon built from it is still lying on the floor.",
        DesignHolder::Stored => "A weapon built from it is still in a Depot or machine buffer.",
        DesignHolder::Carried => "A weapon built from it is still being carried.",
    }
}

/// One sentence for why a Workbench weapon verb was refused. Exhaustive on
/// purpose: a new variant must be worded before this compiles.
pub fn gear_craft_refusal_line(game: &Game, why: &GearCraftRefusal) -> String {
    match why {
        GearCraftRefusal::Busy => "Can't do that right now.".to_string(),
        GearCraftRefusal::NoWorkbench => "You need to be beside a Workbench.".to_string(),
        GearCraftRefusal::PartNotResearched(part) => {
            format!("{} isn't researched yet.", part.label())
        }
        GearCraftRefusal::ShortOf { item, need, held } => {
            format!(
                "Short of {}: need {need}, have {held}.",
                game.item_name(item)
            )
        }
        GearCraftRefusal::MissingParts(list) => {
            let list: Vec<String> = list
                .iter()
                .map(|(part, n)| format!("{} x{n}", part.label()))
                .collect();
            format!("Missing parts: {}.", list.join(", "))
        }
        GearCraftRefusal::NameEmpty => "Give the design a name.".to_string(),
        GearCraftRefusal::NameTooLong { max } => {
            format!("Design names are at most {max} characters.")
        }
        GearCraftRefusal::Fit(fit) => fit_line(fit),
        GearCraftRefusal::Compile(compile) => compile_line(compile),
        GearCraftRefusal::AffixNotKnown(a) => {
            format!("You haven't learned {} yet.", affix_label(a))
        }
        GearCraftRefusal::NoSuchDesign(_) => "That design is gone.".to_string(),
        GearCraftRefusal::InUse(holder) => holder_line(*holder).to_string(),
        GearCraftRefusal::WeaponTreeClosed => {
            "Research Weapon Fabrication to build and salvage weapons.".to_string()
        }
        GearCraftRefusal::NoCopyHeld => "That weapon isn't in your pack any more.".to_string(),
        GearCraftRefusal::NotSalvageable => "That can't be taken apart.".to_string(),
        GearCraftRefusal::SalvageOnly(part) => {
            format!(
                "{} can't be compiled; it only comes from taking a weapon apart.",
                part.label()
            )
        }
    }
}

impl App {
    pub(crate) fn drop_gear_craft_state(&mut self) {
        self.design_draft = DesignDraft::default();
    }

    /// Reports a typed verdict: a refusal worded here, an `Ok` clearing the
    /// line.
    fn report_craft<T>(&mut self, outcome: Result<T, GearCraftRefusal>) {
        match outcome {
            Ok(_) => self.status_line = None,
            Err(why) => self.refuse_craft(&why),
        }
    }

    fn refuse_craft(&mut self, why: &GearCraftRefusal) {
        let Some(game) = &self.game else { return };
        let line = gear_craft_refusal_line(game, why);
        self.refuse(line);
    }

    pub(crate) fn handle_forge_key(&mut self, key: GameKey) {
        let Some(game) = &self.game else { return };
        let rows = game.design_rows();
        if key == GameKey::Char('D') {
            let Some(row) = rows.get(self.menu_selected.min(rows.len().saturating_sub(1))) else {
                return;
            };
            let id = row.id;
            let Some(game) = &mut self.game else { return };
            let outcome = game.delete_design(id);
            self.report_craft(outcome);
            let left = self.game.as_ref().map_or(0, |g| g.design_rows().len());
            self.menu_selected = self.menu_selected.min(left.saturating_sub(1));
            return;
        }
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let id = rows[idx].id;
        let Some(game) = &mut self.game else { return };
        let outcome = game.build_design(id);
        self.report_craft(outcome);
    }

    pub(crate) fn handle_new_design_key(&mut self, key: GameKey) {
        let Some(idx) = self.selected_index(key, DESIGN_PAGE_ROWS) else {
            return;
        };
        self.design_draft.row = idx;
        match idx {
            DESIGN_NAME_ROW => self.mode = Mode::DesignName,
            DESIGN_SAVE_ROW => self.save_design(),
            _ => {
                let has_frame = self
                    .design_draft
                    .parts
                    .iter()
                    .any(|p| p.category() == Category::Frame);
                let picking = match design_row_category(idx) {
                    Some(category) => DesignPicking::Category(category),
                    None => DesignPicking::Affixes,
                };
                if picking != DesignPicking::Category(Category::Frame) && !has_frame {
                    self.refuse_craft(&GearCraftRefusal::Fit(FitError::NoFrame));
                    return;
                }
                self.design_draft.picking = Some(picking);
                self.menu_selected = 0;
                self.mode = Mode::DesignPick;
            }
        }
    }

    fn save_design(&mut self) {
        let draft = std::mem::take(&mut self.design_draft);
        let Some(game) = &mut self.game else { return };
        match game.create_design(&draft.name, draft.gear_parts()) {
            Ok(_) => {
                self.design_draft = DesignDraft {
                    row: DESIGN_SAVE_ROW,
                    ..DesignDraft::default()
                };
                self.status_line = None;
            }
            Err(why) => {
                self.design_draft = draft;
                self.refuse_craft(&why);
            }
        }
    }

    pub(crate) fn handle_design_pick_key(&mut self, key: GameKey) {
        if self.take_filter_key(key) {
            return;
        }
        if key == GameKey::Esc {
            self.leave_design_pick();
            return;
        }
        let Some(picking) = self.design_draft.picking else {
            self.leave_design_pick();
            return;
        };
        let Some(game) = &self.game else { return };
        let visible = design_pick_visible(game, &self.design_draft, self.filter_text());
        let Some(idx) = self.selected_index(key, visible.len() + 1) else {
            return;
        };
        if idx == visible.len() {
            match picking {
                DesignPicking::Category(c) => self.design_draft.parts.retain(|p| p.category() != c),
                DesignPicking::Affixes => self.design_draft.affixes.clear(),
            }
            self.prune_design();
            self.leave_design_pick();
            return;
        }
        let src = visible[idx];
        match picking {
            DesignPicking::Category(category) => self.pick_part(category, src),
            DesignPicking::Affixes => self.toggle_affix(src),
        }
    }

    fn pick_part(&mut self, category: Category, src: usize) {
        let Some(game) = &self.game else { return };
        let options = game.design_part_options(
            category,
            &self.design_draft.parts,
            self.design_draft.affixes.len(),
        );
        let option = &options[src];
        if let Some(why) = &option.unavailable {
            let why = why.clone();
            self.refuse_craft(&why);
            return;
        }
        let part = option.part;
        let parts = &mut self.design_draft.parts;
        parts.retain(|p| p.category() != category);
        if category == Category::Frame {
            parts.insert(0, part);
        } else {
            parts.push(part);
        }
        self.prune_design();
        self.status_line = None;
        self.leave_design_pick();
    }

    /// Toggles one affix; the list stays open so several can be taken.
    fn toggle_affix(&mut self, src: usize) {
        let Some(game) = &self.game else { return };
        let options = game.design_affix_options(&self.design_draft.affixes);
        let affix = options[src].affix.clone();
        if options[src].chosen {
            self.design_draft.affixes.retain(|a| *a != affix);
            return;
        }
        let count = self.design_draft.affixes.len() + 1;
        let cap = game.gear_part_cap();
        if let Err(fit) = weapon::check(&self.design_draft.parts, count, cap) {
            self.refuse_craft(&GearCraftRefusal::Fit(fit));
            return;
        }
        self.design_draft.affixes.push(affix);
        self.status_line = None;
    }

    /// Drops every modifier the Frame (or the rest) no longer takes, so
    /// changing the Frame never leaves a design the engine would refuse.
    fn prune_design(&mut self) {
        let Some(game) = &self.game else { return };
        let draft = &self.design_draft;
        let kept: Vec<WeaponPart> = draft
            .parts
            .iter()
            .copied()
            .filter(|part| {
                let category = part.category();
                if category == Category::Frame {
                    return true;
                }
                let rest: Vec<WeaponPart> =
                    draft.parts.iter().copied().filter(|p| p != part).collect();
                game.design_part_options(category, &rest, draft.affixes.len())
                    .iter()
                    .any(|o| o.part == *part && o.unavailable.is_none())
            })
            .collect();
        self.design_draft.parts = kept;
    }

    fn leave_design_pick(&mut self) {
        self.design_draft.picking = None;
        self.menu_selected = self.design_draft.row;
        self.mode = Mode::NewDesign;
    }

    pub(crate) fn handle_design_name_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc | GameKey::Enter => {
                self.menu_selected = self.design_draft.row;
                self.mode = Mode::NewDesign;
            }
            GameKey::Backspace => {
                self.design_draft.name.pop();
            }
            GameKey::Char(c)
                if !c.is_control() && self.design_draft.name.chars().count() < DESIGN_NAME_MAX =>
            {
                self.design_draft.name.push(c);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_weapon_parts_key(&mut self, key: GameKey) {
        let Some(game) = &self.game else { return };
        let rows = game.weapon_part_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let part = rows[idx].part;
        let Some(game) = &mut self.game else { return };
        let outcome = game.craft_weapon_part(part);
        self.report_craft(outcome);
    }

    pub(crate) fn handle_weapon_salvage_key(&mut self, key: GameKey) {
        let Some(game) = &self.game else { return };
        let rows = game.salvage_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let copy = rows[idx].copy.clone();
        let Some(game) = &mut self.game else { return };
        let outcome = game.salvage_weapon(&copy);
        self.report_craft(outcome);
        let left = self.game.as_ref().map_or(0, |g| g.salvage_rows().len());
        self.menu_selected = self.menu_selected.min(left.saturating_sub(1));
    }
}
