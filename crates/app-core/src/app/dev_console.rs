//! A dev-only keypad for provoking events the game otherwise rolls for.
//!
//! It exists because a visual feature cannot be verified by a test suite,
//! and provoking the state one draws in can cost an hour of play — a GC
//! Entropy Sweep is 1.2% per tick, and wearing a single structure down to
//! nothing takes hundreds of them. That cost is why things in this repo
//! ship unplayed.
//!
//! Deliberately not a text console: no parser, no history, no text-entry
//! mode in app-core's input state machine. Every trigger is one row and one
//! key, which is all a keypad needs to be.
//!
//! Nothing here decides what an event *is*. Each row calls a `dev_` entry on
//! `Game` that runs the same body the game runs, so what the console puts on
//! screen is evidence about the game rather than about the console.

use crate::{App, GameKey, Mode};

/// Opens the console from the map. Backtick, by console convention, and it
/// is otherwise unbound.
pub const DEV_CONSOLE_KEY: char = '`';

/// Cycles burned by one press of the tick row. Enough that ambient systems
/// — spawns, production, needs decay — visibly move, small enough that a
/// press is not a fast-forward past the thing you were watching for.
pub const DEV_CONSOLE_TICKS: u32 = 25;

/// What a row does when it is fired.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DevAction {
    Sweep,
    DestroyStructure,
    Encounter,
    AdvanceTicks,
}

pub struct DevConsoleRow {
    pub label: &'static str,
    pub action: DevAction,
}

/// The one source of both the labels the renderer draws and the actions the
/// handler fires.
///
/// No row is ever hidden, unlike the group menus: a trigger with nothing to
/// act on is a no-op rather than a mistake, and a dev tool whose buttons
/// come and go is harder to reason about than one whose presses sometimes
/// do nothing.
const DEV_ROWS: &[DevConsoleRow] = &[
    DevConsoleRow {
        label: "Trigger GC Entropy Sweep",
        action: DevAction::Sweep,
    },
    DevConsoleRow {
        label: "Destroy nearest structure",
        action: DevAction::DestroyStructure,
    },
    DevConsoleRow {
        label: "Spawn wild encounter",
        action: DevAction::Encounter,
    },
    DevConsoleRow {
        label: "Advance 25 cycles",
        action: DevAction::AdvanceTicks,
    },
];

/// Whether a dev flag is set: present, non-empty and not `"0"`.
///
/// One answer to "is a dev flag set", shared with `dev_arena_enabled`. Two
/// answers is exactly the drift this repo keeps catching.
pub(crate) fn dev_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0")
}

/// Whether `FERAL_DEV_CONSOLE` was set when this `App` was built.
pub(crate) fn dev_console_enabled() -> bool {
    dev_flag("FERAL_DEV_CONSOLE")
}

impl App {
    pub fn dev_console_rows() -> &'static [DevConsoleRow] {
        DEV_ROWS
    }

    /// Whether the console may be opened at all.
    pub fn dev_console_enabled(&self) -> bool {
        self.dev_console
    }

    /// Opens the gate without touching the environment the parallel test
    /// suite shares.
    #[cfg(test)]
    pub(crate) fn enable_dev_console_for_test(&mut self) {
        self.dev_console = true;
    }

    pub(crate) fn handle_dev_console_key(&mut self, key: GameKey) {
        let rows = Self::dev_console_rows();
        match key {
            GameKey::Esc => self.mode = Mode::Playing,
            GameKey::Up => self.menu_selected = self.menu_selected.saturating_sub(1),
            GameKey::Down => {
                self.menu_selected = (self.menu_selected + 1).min(rows.len().saturating_sub(1))
            }
            GameKey::Enter => {
                if let Some(row) = rows.get(self.menu_selected) {
                    self.fire_dev_action(row.action);
                }
            }
            _ => {}
        }
    }

    /// Runs one trigger against the live game.
    ///
    /// Ends in `after_world_action` rather than its own copy of the tail, so
    /// a forced encounter that opens a fight reaches `Mode::Battle` and a
    /// forced sweep that kills the player reaches the game-over check by
    /// exactly the route a real one does.
    fn fire_dev_action(&mut self, action: DevAction) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        match action {
            DevAction::Sweep => game.dev_force_raid(),
            DevAction::DestroyStructure => game.dev_destroy_structure(),
            DevAction::Encounter => game.dev_force_encounter(),
            DevAction::AdvanceTicks => {
                for _ in 0..DEV_CONSOLE_TICKS {
                    // A tick can now open a fight on its own —
                    // `nest_aggro_tick` calls `start_battle` from inside
                    // `tick_inner` — so this loop carries the same check
                    // `Game::rest` needed for the same reason. Without it a
                    // press would keep ticking the world through a battle
                    // the player is standing in.
                    if game.has_active_battle() {
                        break;
                    }
                    game.wait();
                }
            }
        }
        self.after_world_action(true, false);
    }
}
