//! The Teardown Rig's tool holder: which tool the base's extractor runs on.
//!
//! A page of the structure hub (`app/structure_hub.rs`) on a rig.
//! `Mode::DepotFilter`'s shape rung for rung — a screen that acts on one
//! machine, re-read after every edit.
//!
//! It is a separate screen from `Mode::Tools` on purpose: that one is the
//! player's own slots, and one screen meaning two things depending on where
//! the player is standing is the disease that kept this off `c`.

use crate::*;

/// Which rig the screen is fitting and what it last read of it.
///
/// Snapshotted rather than re-derived per frame, `DepotFilterScreen`'s
/// reason: the rows are what the pack is carrying and cannot change under
/// the cursor without a keypress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigToolScreen {
    pub rig: Entity,
    pub view: RigToolView,
}

impl App {
    /// Opens the tool holder on one rig, the hub's Tool page.
    pub(crate) fn open_rig_tool_for(&mut self, rig: Entity) -> bool {
        let shown = self.show_rig_tool(rig);
        if shown {
            self.menu_selected = 0;
            self.mode = Mode::RigTool;
        }
        shown
    }

    /// Re-reads one rig into the screen, reporting whether it is still
    /// there. `false` leaves the screen empty; the hub's refresh closes it.
    fn show_rig_tool(&mut self, rig: Entity) -> bool {
        let Some(view) = self.game.as_ref().and_then(|g| g.rig_tool_view(rig)) else {
            self.rig_tool = None;
            return false;
        };
        self.rig_tool = Some(RigToolScreen { rig, view });
        true
    }

    fn refresh_rig_tool(&mut self) {
        if let Some(rig) = self.rig_tool.as_ref().map(|s| s.rig) {
            self.show_rig_tool(rig);
        }
    }

    pub(crate) fn handle_rig_tool_key(&mut self, key: GameKey) {
        let Some(screen) = &self.rig_tool else {
            return;
        };
        let (rig, rows) = (screen.rig, screen.view.candidates.len());
        match key {
            GameKey::Up | GameKey::Down | GameKey::PageUp | GameKey::PageDown => {
                self.scroll(key, rows)
            }
            GameKey::Enter => self.fit_selected_rig_tool(rig),
            GameKey::Char('R') => {
                let outcome = self.game.as_mut().map(|g| g.remove_rig_tool(rig));
                if let Some(outcome) = outcome {
                    self.report(outcome);
                }
                self.refresh_rig_tool();
            }
            key => {
                if let Some(idx) = self.selected_index(key, rows) {
                    self.menu_selected = idx;
                    self.fit_selected_rig_tool(rig);
                }
            }
        }
    }

    fn fit_selected_rig_tool(&mut self, rig: Entity) {
        let tool = self
            .rig_tool
            .as_ref()
            .and_then(|s| s.view.candidates.get(self.menu_selected))
            .map(|row| row.id.clone());
        let (Some(tool), Some(game)) = (tool, &mut self.game) else {
            return;
        };
        let outcome = game.install_rig_tool(rig, &tool);
        self.report(outcome);
        // The row that was fitted leaves `candidates` — it is the installed
        // tool now — so the cursor has to come back inside the list rather
        // than sit past its end.
        self.menu_selected = 0;
        self.refresh_rig_tool();
        self.after_tick();
    }
}
