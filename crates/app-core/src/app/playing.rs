//! The map screen — movement, and the keys that open every other mode.

use crate::DEV_CONSOLE_KEY;
use crate::*;

impl App {
    pub(crate) fn handle_playing_key(&mut self, key: GameKey) {
        match key {
            // The two group menus. Seventeen keys used to sit on this
            // screen doing what these two now reach; see `group_menu.rs`
            // for why none of them survive as an alias.
            // Dev-only, and inert unless FERAL_DEV_CONSOLE opened the gate.
            GameKey::Char(c) if c == DEV_CONSOLE_KEY && self.dev_console_enabled() => {
                self.mode = Mode::DevConsole;
                self.menu_selected = 0;
                return;
            }
            GameKey::Char('b') => {
                self.mode = Mode::BaseMenu;
                return;
            }
            GameKey::Char('p') => {
                self.mode = Mode::PartyMenu;
                return;
            }
            // No menu of its own: the pack is a single screen, so the group
            // key opens it directly.
            GameKey::Char('i') => {
                self.mode = Mode::Inventory;
                return;
            }
            GameKey::Char('u') => {
                self.mode = Mode::Symlink;
                return;
            }
            // Examine, in the roguelike sense — `i` went to the pack, and
            // perks moving into the party menu freed the key a player would
            // guess for it anyway.
            GameKey::Char('x') => {
                self.mode = Mode::InspectDirection;
                return;
            }
            // Flat despite belonging to the party, like `c` and `t` below:
            // these three are pressed every few turns while walking, and a
            // group menu is a keystroke tax on anything that frequent.
            GameKey::Char('a') => {
                self.mode = Mode::FieldCast;
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
            GameKey::Char('L') => {
                self.mode = Mode::History;
                return;
            }
            // Next to `L` because it acts on the same log, and a `return`
            // rather than a fallthrough because changing what you can see
            // must not cost a turn. History keeps the only uppercase letter
            // left on this screen: `l` walks east.
            GameKey::Char('f') => {
                self.log_filter = self.log_filter.next();
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
                // Surface only, and not bound in the Stack block below: a
                // base's buffers are something you walk up to, and the
                // engine refuses it underground anyway.
                GameKey::Char('c') => {
                    game.collect_adjacent();
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
                // Undocumented on purpose — see `crates/engine/EASTER_EGGS.md`.
                // Uppercase, so it can never collide with the row shortcuts,
                // and `Z` specifically because every other capital here is a
                // shift-slip of a key the player presses constantly: `K J H L`
                // are movement, `S` is saving, `Q` is quitting, and a fumbled
                // step that quietly spent a turn and raised Trace would be a
                // bug the player could not diagnose.
                GameKey::Char('Z') => match game.listen() {
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
    pub(crate) fn after_world_action(&mut self, acted: bool, is_move_key: bool) {
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
