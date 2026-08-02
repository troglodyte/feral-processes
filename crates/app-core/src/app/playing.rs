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
            GameKey::Char('W') => {
                self.mode = Mode::WorkStructure;
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
            GameKey::Char('d') => {
                self.mode = Mode::ManifestPick;
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
            GameKey::Char('a') => {
                self.mode = Mode::FieldCast;
                return;
            }
            GameKey::Char('M') => {
                self.mode = Mode::Extract;
                return;
            }
            GameKey::Char('t') => {
                // Opening the trader list from the map is a fresh visit, not
                // the tail of a sale begun in the inventory.
                self.trade_origin = TradeOrigin::Trader;
                self.pending_trade_choice = None;
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
            GameKey::Char('L') => {
                self.mode = Mode::History;
                return;
            }
            GameKey::Char('B') => {
                self.mode = Mode::Structures;
                return;
            }
            GameKey::Char('s') => {
                self.save_game();
                return;
            }
            GameKey::Char('q') => {
                self.mode = Mode::QuitRunConfirm;
                return;
            }
            GameKey::Char('?') => {
                self.mode = Mode::Help;
                return;
            }
            // Whichever map is on screen: underground the zone map is not
            // drawn at all, so resizing its tiles from down there would be
            // a keypress with nothing to show for it.
            GameKey::Char('+') | GameKey::Char('=') => {
                if self.game.as_ref().is_some_and(|g| g.is_underground()) {
                    self.stack_zoom = (self.stack_zoom + 1).min(STACK_MAP_MAX_ZOOM);
                } else {
                    self.zoom = (self.zoom + 1).min(MAX_ZOOM);
                }
                return;
            }
            GameKey::Char('-') | GameKey::Char('_') => {
                if self.game.as_ref().is_some_and(|g| g.is_underground()) {
                    self.stack_zoom = self.stack_zoom.saturating_sub(1).max(STACK_MAP_MIN_ZOOM);
                } else {
                    self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
                }
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
        // Underground the same four keys steer a party that has a facing:
        // forward, back, and turn in place. Deliberately the same keys rather
        // than a separate set — walking is walking, and the view makes which
        // one you're doing obvious.
        if self.game.as_ref().is_some_and(|g| g.is_underground()) {
            self.handle_stack_key(key, is_move_key);
            return;
        }

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
                _ => false,
            }
        };
        self.after_world_action(acted, is_move_key);
    }

    /// Movement for a party that has a facing. Forward and back walk along
    /// it; left and right turn in place, which is what makes the Stack a
    /// first-person space rather than a top-down one seen at an angle.
    ///
    /// Everything else on the map screen is left alone — the mode keys above
    /// already ran, and the ones that need open grid refuse in the engine
    /// (see `Game::require_surface`).
    fn handle_stack_key(&mut self, key: GameKey, is_move_key: bool) {
        // The same `g` that is a no-op on the surface. Checked before the
        // game is borrowed below, and costing no tick: reading your own map
        // is not an action, and the Stack advancing a turn every time you
        // checked where you were would punish mapping.
        if key == GameKey::Char('g') {
            self.mode = Mode::FrameMap;
            return;
        }

        // Set after the `self.game` borrow below releases. A refusal is not
        // an action, so it leaves `acted` false and `after_world_action`
        // returns before it can clear the line that explains why.
        let mut refusal = None;
        let acted = {
            let Some(game) = &mut self.game else { return };
            match key {
                GameKey::Up | GameKey::Char('k') => {
                    game.step_forward();
                    true
                }
                GameKey::Down | GameKey::Char('j') => {
                    game.step_back();
                    true
                }
                GameKey::Left | GameKey::Char('h') => {
                    game.turn_left();
                    true
                }
                GameKey::Right | GameKey::Char('l') => {
                    game.turn_right();
                    true
                }
                GameKey::Char('>') => {
                    game.descend();
                    true
                }
                GameKey::Char('<') => {
                    game.ascend();
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
                // Not 't', which the mode block above already spends on the
                // trader list before this arm is ever reached. 'o' is free
                // and matches the glyph the orphan draws as in both views.
                GameKey::Char('o') => match game.adopt_orphan() {
                    Ok(()) => true,
                    Err(reason) => {
                        refusal = Some(reason);
                        false
                    }
                },
                _ => false,
            }
        };
        if refusal.is_some() {
            self.status_line = refusal;
        }
        self.after_world_action(acted, is_move_key);
    }

    /// The bookkeeping that follows any action that advanced the world,
    /// whichever locale it happened in: clearing the status line, dropping
    /// into `Mode::Battle` if one just started, the movement cue, and the
    /// game-over check.
    ///
    /// Shared by the surface and Stack paths rather than copied into both.
    /// The battle transition especially: Phase 2 puts random encounters
    /// underground, and a second copy of this is exactly the kind of thing
    /// that gets updated on one side only.
    fn after_world_action(&mut self, acted: bool, is_move_key: bool) {
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
