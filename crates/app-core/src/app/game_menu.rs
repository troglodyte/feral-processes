//! The game menu: settings, and save and quit, in one hub. Esc on the map
//! opens it, and `[O]` on the main menu does too, without the Save & quit
//! page since there is no run to leave.
//!
//! Every setting lives in `App::profile` and is written to `profile.ron` the
//! moment it changes; the renderer only reads it.

use crate::*;

/// Percentage points one Left/Right press moves the volume.
const VOLUME_STEP: i16 = 10;
/// Where a full volume sits, and what the profile's integer is clamped to.
const VOLUME_MAX: i16 = 100;

const GAMEPLAY: HubPage = HubPage {
    title: "Gameplay",
    screens: &[Mode::GameplayOptions],
};
const AUDIO: HubPage = HubPage {
    title: "Audio",
    screens: &[Mode::AudioOptions],
};
const GRAPHICS: HubPage = HubPage {
    title: "Graphics",
    screens: &[Mode::GraphicsOptions],
};
const SAVE_AND_QUIT: HubPage = HubPage {
    title: "Save & quit",
    screens: &[Mode::QuitRunConfirm],
};

pub(crate) struct GameMenu;

impl HubKind for GameMenu {
    fn title(&self) -> &'static str {
        "GAME MENU"
    }

    /// The Save & quit page exists only while there is a run to save.
    fn pages(&self, app: &App) -> Vec<&'static HubPage> {
        let mut pages = vec![&GAMEPLAY, &AUDIO, &GRAPHICS];
        if app.game.is_some() {
            pages.push(&SAVE_AND_QUIT);
        }
        pages
    }

    /// A remembered Save & quit page would not exist when the menu is next
    /// opened from the main menu.
    fn remembers(&self) -> bool {
        false
    }

    /// Back to wherever the menu was opened from: a live run means the map.
    /// Every route to the main menu drops the run (`leave_run`,
    /// `handle_game_over_key`), so `game` says where the player came from.
    fn leave(&self, app: &mut App) {
        app.mode = if app.game.is_some() {
            Mode::Playing
        } else {
            Mode::MainMenu
        };
    }
}

impl App {
    /// Whether the Save & quit page is asking "quit without saving?".
    pub fn quit_unsaved_pending(&self) -> bool {
        self.quit_unsaved_pending
    }

    /// The rows of the settings page now showing.
    pub fn option_rows(&self) -> Vec<OptionRow> {
        let toggle = |key, label: &str, on| OptionRow {
            key,
            label: label.to_string(),
            value: on_off(on),
            kind: OptionKind::Toggle,
        };
        match self.mode {
            Mode::AudioOptions => vec![OptionRow {
                key: OptionKey::Volume,
                label: "Volume".to_string(),
                value: format!("{}%", self.profile.volume_percent.min(100)),
                kind: OptionKind::Level,
            }],
            Mode::GraphicsOptions => vec![
                toggle(OptionKey::Effects, "Visual effects", self.profile.effects),
                toggle(
                    OptionKey::PerfOverlay,
                    "Frame timings",
                    self.profile.perf_overlay,
                ),
            ],
            _ => vec![toggle(
                OptionKey::TacticalBattles,
                "Tactical surface battles",
                self.profile.tactical_battles,
            )],
        }
    }

    /// Whether the cursor is on a row Left/Right adjust. The hub's nav
    /// leaves those arrows to the screen only then, so Left still reaches
    /// the rail from a toggle.
    pub(crate) fn on_level_row(&self) -> bool {
        self.option_rows()
            .get(self.menu_selected)
            .is_some_and(|r| r.kind == OptionKind::Level)
    }

    pub(crate) fn handle_options_key(&mut self, key: GameKey) {
        let rows = self.option_rows();
        let Some(row) = rows.get(self.menu_selected) else {
            return;
        };
        match (key, row.kind) {
            (GameKey::Enter, OptionKind::Toggle) => self.toggle_option(row.key),
            (GameKey::Left, OptionKind::Level) => self.step_volume(-VOLUME_STEP),
            (GameKey::Right, OptionKind::Level) => self.step_volume(VOLUME_STEP),
            _ => {
                self.scroll(key, rows.len());
            }
        }
    }

    /// Flips one toggle.
    pub fn toggle_option(&mut self, key: OptionKey) {
        match key {
            OptionKey::TacticalBattles => {
                self.profile.tactical_battles = !self.profile.tactical_battles;
            }
            OptionKey::Effects => self.profile.effects = !self.profile.effects,
            OptionKey::PerfOverlay => self.profile.perf_overlay = !self.profile.perf_overlay,
            OptionKey::Volume => return,
        }
        self.commit_settings();
    }

    fn step_volume(&mut self, delta: i16) {
        let now = i16::from(self.profile.volume_percent.min(100));
        self.profile.volume_percent = (now + delta).clamp(0, VOLUME_MAX) as u8;
        self.commit_settings();
    }

    /// The one write path for a setting: the in-memory value is already
    /// changed and stays changed whatever the disk says, and `profile.ron`
    /// is written at once — nothing downstream is going to save it, and a
    /// setting that forgets itself on quit is indistinguishable from one
    /// that does not work. A failed write costs the setting's persistence
    /// and nothing else.
    fn commit_settings(&mut self) {
        if let Some(game) = &mut self.game {
            game.set_tactical_battles(self.profile.tactical_battles);
        }
        if let Err(e) = self.profile.save(&self.profile_path) {
            self.status_line = Some(format!("Could not write profile: {e}"));
        }
    }

    /// The Save & quit page: three rows, or the yes/no that follows the
    /// third.
    pub(crate) fn handle_quit_run_confirm_key(&mut self, key: GameKey) {
        if self.quit_unsaved_pending {
            self.answer_quit_unsaved(key);
            return;
        }
        let Some(picked) = self.selected_index(key, 3) else {
            return;
        };
        match picked {
            0 => {
                self.save_game();
            }
            // A failed save holds the player here with the error still on
            // screen rather than leaving anyway. Quitting after being asked
            // to save first, and dropping the run regardless, is the one
            // outcome this page exists to prevent.
            1 => {
                if self.save_game() {
                    self.leave_run();
                }
            }
            _ => {
                self.quit_unsaved_pending = true;
                // On "No", so a held Enter cannot answer it.
                self.menu_selected = 1;
            }
        }
    }

    fn answer_quit_unsaved(&mut self, key: GameKey) {
        if matches!(key, GameKey::Esc | GameKey::Left) {
            self.quit_unsaved_pending = false;
            return;
        }
        match self.yes_no(key) {
            Some(true) => self.leave_run(),
            Some(false) => self.quit_unsaved_pending = false,
            None => {}
        }
    }
}

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.to_string()
}
