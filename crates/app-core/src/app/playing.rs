//! The map screen — movement, and the keys that open every other mode.

use crate::*;

impl App {
    pub(crate) fn handle_playing_key(&mut self, key: GameKey) {
        match key {
            GameKey::Char('b') => {
                self.mode = Mode::Build;
                return;
            }
            GameKey::Char('c') => {
                self.mode = Mode::Craft;
                return;
            }
            GameKey::Char('w') => {
                self.mode = Mode::Cronjob;
                return;
            }
            GameKey::Char('G') => {
                self.mode = Mode::Guard;
                return;
            }
            GameKey::Char('R') => {
                self.mode = Mode::Remove;
                return;
            }
            GameKey::Char('U') => {
                self.mode = Mode::Upgrade;
                return;
            }
            GameKey::Char('u') => {
                self.mode = Mode::Symlink;
                return;
            }
            GameKey::Char('i') => {
                self.mode = Mode::InspectDirection;
                return;
            }
            GameKey::Char('v') => {
                self.mode = Mode::Inventory;
                return;
            }
            GameKey::Char('p') => {
                self.mode = Mode::Companion;
                return;
            }
            GameKey::Char('f') => {
                self.mode = Mode::Fuse;
                return;
            }
            GameKey::Char('m') => {
                self.mode = Mode::RoutineTarget;
                return;
            }
            GameKey::Char('M') => {
                self.mode = Mode::Extract;
                return;
            }
            GameKey::Char('t') => {
                self.mode = Mode::Trade;
                return;
            }
            GameKey::Char('x') => {
                self.mode = Mode::Perks;
                return;
            }
            GameKey::Char('T') => {
                self.mode = Mode::Research;
                return;
            }
            GameKey::Char('s') => {
                self.save_game();
                return;
            }
            GameKey::Char('q') => {
                self.game = None;
                self.status_line = None;
                self.mode = Mode::MainMenu;
                return;
            }
            GameKey::Char('?') => {
                self.mode = Mode::Help;
                return;
            }
            GameKey::Char('+') | GameKey::Char('=') => {
                self.zoom = (self.zoom + 1).min(MAX_ZOOM);
                return;
            }
            GameKey::Char('-') | GameKey::Char('_') => {
                self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
                return;
            }
            _ => {}
        }

        let is_move_key = matches!(
            key,
            GameKey::Up
                | GameKey::Down
                | GameKey::Left
                | GameKey::Right
                | GameKey::Char('k')
                | GameKey::Char('j')
                | GameKey::Char('h')
                | GameKey::Char('l')
        );
        let acted = {
            let Some(game) = &mut self.game else { return };
            match key {
                GameKey::Up | GameKey::Char('k') => {
                    game.move_player(0, -1);
                    true
                }
                GameKey::Down | GameKey::Char('j') => {
                    game.move_player(0, 1);
                    true
                }
                GameKey::Left | GameKey::Char('h') => {
                    game.move_player(-1, 0);
                    true
                }
                GameKey::Right | GameKey::Char('l') => {
                    game.move_player(1, 0);
                    true
                }
                GameKey::Char('.') => {
                    game.wait();
                    true
                }
                GameKey::Char('e') => {
                    game.use_power_source();
                    true
                }
                GameKey::Char('r') => {
                    game.rest();
                    true
                }
                GameKey::Char('g') => {
                    game.forage();
                    true
                }
                _ => false,
            }
        };
        if !acted {
            return;
        }
        self.status_line = None;
        let entered_battle = self
            .game
            .as_ref()
            .map(|g| g.has_active_battle())
            .unwrap_or(false);
        if entered_battle {
            self.mode = Mode::Battle;
        }
        if is_move_key {
            self.pending_sounds.push(if entered_battle {
                SoundEvent::BattleStart
            } else {
                SoundEvent::Step
            });
        }
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_sounds.push(SoundEvent::Defeat);
        }
    }
}
