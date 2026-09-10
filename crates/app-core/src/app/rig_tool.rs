//! The Teardown Rig's tool holder: which tool the base's extractor runs on.
//!
//! Opened with `[F]` beside a rig. `Mode::DepotFilter`'s shape rung for
//! rung — a screen that acts on one machine, re-read after every edit and
//! closing itself the moment that machine stops being one the player is
//! standing at.
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
    /// How many rigs are beside the party, so the screen can say whether
    /// `Tab` will do anything.
    pub of: usize,
}

impl App {
    /// Opens the tool holder for the first adjacent rig, or refuses.
    ///
    /// The refusal goes through `App::refuse`, so the sentence lands in the
    /// log the player scrolls back through as well as on the status line —
    /// a key that reads as doing nothing is the failure being avoided.
    ///
    /// **Answers whether anything happened**, and a refusal answers `false`.
    /// `after_world_action` clears `status_line` on a key that acted, so
    /// reporting a refusal as an action wipes the sentence that was just
    /// written — which is the same silence the refusal exists to prevent.
    pub(crate) fn open_rig_tool(&mut self) -> bool {
        let Some(game) = &self.game else {
            return false;
        };
        let rigs = game.adjacent_teardown_rigs();
        let Some(rig) = rigs.first().copied() else {
            self.refuse("There is no rig here to fit.");
            return false;
        };
        if self.show_rig_tool(rig, rigs.len()) {
            self.menu_selected = 0;
            self.mode = Mode::RigTool;
        }
        true
    }

    /// Moves to the next adjacent rig, wrapping. A no-op with one.
    pub(crate) fn cycle_rig_tool(&mut self) {
        let Some((game, current)) = self
            .game
            .as_ref()
            .zip(self.rig_tool.as_ref().map(|s| s.rig))
        else {
            return;
        };
        let rigs = game.adjacent_teardown_rigs();
        let next = rigs
            .iter()
            .position(|e| *e == current)
            .map(|i| (i + 1) % rigs.len().max(1))
            .and_then(|i| rigs.get(i).copied());
        match next {
            Some(rig) => {
                if self.show_rig_tool(rig, rigs.len()) {
                    self.menu_selected = 0;
                }
            }
            None => self.leave_rig_tool(),
        }
    }

    /// Re-reads one rig into the screen, reporting whether it is still
    /// there. `false` closes the screen — the machine has been demolished,
    /// swept, or walked away from while this was open.
    fn show_rig_tool(&mut self, rig: Entity, of: usize) -> bool {
        let Some(view) = self.game.as_ref().and_then(|g| g.rig_tool_view(rig)) else {
            self.leave_rig_tool();
            return false;
        };
        self.rig_tool = Some(RigToolScreen { rig, view, of });
        true
    }

    pub(crate) fn leave_rig_tool(&mut self) {
        self.rig_tool = None;
        self.mode = Mode::Playing;
    }

    fn refresh_rig_tool(&mut self) {
        if let Some(screen) = &self.rig_tool {
            let (rig, of) = (screen.rig, screen.of);
            self.show_rig_tool(rig, of);
        }
    }

    pub(crate) fn handle_rig_tool_key(&mut self, key: GameKey) {
        let Some(screen) = &self.rig_tool else {
            self.mode = Mode::Playing;
            return;
        };
        let (rig, rows) = (screen.rig, screen.view.candidates.len());
        match key {
            GameKey::Esc => self.leave_rig_tool(),
            GameKey::Up | GameKey::Down => self.scroll(key, rows),
            GameKey::Tab => self.cycle_rig_tool(),
            GameKey::Enter => self.fit_selected_rig_tool(rig),
            // Uppercase, because a lowercase letter picks a row everywhere
            // else in the game — and `selected_index` answers `None` for
            // anything that is not lowercase or a digit, so this cannot
            // also pick a row.
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
