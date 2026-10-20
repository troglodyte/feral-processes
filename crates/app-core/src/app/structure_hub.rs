//! The structure hub: `[c]` beside a structure opens one frame for it, a rail
//! of that structure's pages and the page on the right.
//!
//! `App::structure_hub` holds the cycle the engine reported
//! (`Game::adjacent_structures`) and which structure is showing; each page is
//! one existing screen, opened for that structure by its `_for` opener. An
//! opener that finds its structure gone closes the hub rather than leaving the
//! previous page up.
//! Opening, cycling and turning pages spend no turn.

use crate::*;

impl App {
    /// `[c]`: the hub for the structures beside the party, on the first
    /// clockwise from north and its first page, cursor on the rail —
    /// `open_base_report`'s rule. With no
    /// structure beside, the pack's downed programs alone open the transfer
    /// screen; the caller has already refused when there is nothing at all.
    pub(crate) fn open_structure_hub(&mut self) {
        let Some(game) = &self.game else { return };
        let cycle = game.adjacent_structures();
        if cycle.is_empty() {
            self.open_transfer_for(TransferAt::PackOnly);
            return;
        }
        self.structure_hub = Some(StructureHub { cycle, at: 0 });
        self.enter_structure_page(0);
        self.hub_on_rail = true;
    }

    /// Opens the current structure's `page`th page: the screen's own opener,
    /// leaving the cursor on whichever half it was. Whatever the previous
    /// page held is dropped first, so a basket left by a page turn or `Tab`
    /// is discarded.
    pub(crate) fn enter_structure_page(&mut self, page: usize) {
        self.drop_page_state();
        let Some(hub) = &self.structure_hub else {
            return;
        };
        let current = hub.current();
        let entity = current.entity;
        let Some(&page) = current.pages.get(page) else {
            return;
        };
        match page {
            StructurePage::Inventory => self.open_transfer_for(TransferAt::Structure(entity)),
            StructurePage::Filter => {
                if !self.open_depot_filter_for(entity) {
                    self.close_structure_hub();
                    return;
                }
            }
            StructurePage::Recipe => {
                if !self.open_machine_recipe_for(entity) {
                    self.close_structure_hub();
                    return;
                }
            }
            StructurePage::Tool => {
                if !self.open_rig_tool_for(entity) {
                    self.close_structure_hub();
                    return;
                }
            }
            StructurePage::Siphon => self.open_siphon_for(entity),
            StructurePage::Splice => self.open_splice_rig(),
            StructurePage::Grow => {
                self.pending_plot = Some(entity);
                self.mode = Mode::FarmRecipe;
            }
            StructurePage::Rebuild => {
                self.pending_build = Some(PendingBuild::Rebuild { site: entity });
                self.mode = Mode::BuildProgram;
            }
            StructurePage::Info => {
                self.pending_structure_manifest = Some(entity);
                self.mode = Mode::StructureManifest;
            }
        }
        self.menu_selected = if self.mode == Mode::FarmRecipe {
            self.farm_recipe_row()
        } else {
            0
        };
    }

    /// `Tab`: the next structure clockwise, wrapping, on its first page, the
    /// cursor staying on whichever half it was.
    pub(crate) fn step_structure(&mut self, by: isize) {
        let Some(hub) = &mut self.structure_hub else {
            return;
        };
        let count = hub.cycle.len() as isize;
        hub.at = (hub.at as isize + by).rem_euclid(count) as usize;
        self.enter_structure_page(0);
    }

    /// Leaves the hub for the map.
    pub(crate) fn close_structure_hub(&mut self) {
        self.drop_page_state();
        self.structure_hub = None;
        self.mode = Mode::Playing;
    }

    /// Re-reads the cycle from the engine, run after every key and tick. A
    /// structure that is gone or no longer beside the party hands the hub to
    /// the next one (the hub closes with none); a page the structure no
    /// longer offers falls back to its first.
    pub(crate) fn refresh_structure_hub(&mut self) {
        let Some(hub) = &self.structure_hub else {
            return;
        };
        let (was, old_at) = (hub.current().entity, hub.at);
        let page = self.hub_at().map(|(_, p, _)| p);
        let cycle = self
            .game
            .as_ref()
            .map(|g| g.adjacent_structures())
            .unwrap_or_default();
        if cycle.is_empty() {
            self.close_structure_hub();
            return;
        }
        let kept = cycle.iter().position(|s| s.entity == was);
        // The vanished structure's successor slides into its slot; past the
        // end the cycle wraps to its first.
        let at = kept.unwrap_or(if old_at < cycle.len() { old_at } else { 0 });
        let page_kept =
            |hub: &StructureHub, p: usize| hub.current().pages.get(p) == cycle[at].pages.get(p);
        let keep = kept.is_some()
            && page.is_none_or(|p| self.structure_hub.as_ref().is_some_and(|h| page_kept(h, p)));
        self.structure_hub = Some(StructureHub { cycle, at });
        if !keep {
            self.enter_structure_page(0);
        }
    }

    /// Clears what the open page keeps on `App`, so a reopened page never
    /// shows the last structure's rows.
    fn drop_page_state(&mut self) {
        self.clear_basket();
        self.depot_filter = None;
        self.machine_recipe = None;
        self.rig_tool = None;
        self.siphon = None;
        self.splice_rig = None;
        self.pending_plot = None;
        self.pending_structure_manifest = None;
        if matches!(self.pending_build, Some(PendingBuild::Rebuild { .. })) {
            self.pending_build = None;
        }
    }
}
