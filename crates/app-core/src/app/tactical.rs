//! The three screens a tactical fight is fought through.
//!
//! `Mode::TacticalBattle` is the map screen with a different tile source;
//! `Mode::TacticalRoutine` is its routine picker; `Mode::TacticalAim` is the
//! cell cursor both a swing and a shaped routine commit through.
//!
//! **The wild side is paced, not instant.** A fight resolving one body at a
//! time in front of the player is a different concept from the abstract
//! model's round narration, so none of `BattleReveal`'s machinery is reused:
//! there are no table rows to rewind and nothing is held back from the log.
//! What this holds instead is a carry against `dt` — `advance_compile`'s
//! rule, and for its reason — so the pace is the same on any machine.

use crate::{App, GameKey, Mode, TACTICAL_TURNS_PER_SECOND, TacticalIntent};
use feral_processes_engine::battle::SpecialOption;
use feral_processes_engine::tactical::turn::StepOutcome;

impl App {
    /// Arrows and the numpad step the acting body; a lowercase letter picks
    /// an action; `[E]` hands the turn on without spending it.
    ///
    /// **The diagonals are not a new rule, they are the missing keys for a
    /// rule already in force.** `game::pursuit::walk_field` is Chebyshev, so
    /// `reach::movement_field` already offers diagonal cells and the wild
    /// side already walks them; the player was the one body on the board
    /// that could not.
    pub(crate) fn handle_tactical_key(&mut self, key: GameKey) {
        // A wild body is mid-turn. Its turns are the pacing loop's to
        // spend, and a key pressed into one would act for a body that is
        // not the player's — so the whole handler waits.
        if !self.tactical_player_turn() {
            return;
        }
        match key {
            GameKey::Up => self.tactical_step((0, -1)),
            GameKey::Down => self.tactical_step((0, 1)),
            GameKey::Left => self.tactical_step((-1, 0)),
            GameKey::Right => self.tactical_step((1, 0)),
            GameKey::UpLeft => self.tactical_step((-1, -1)),
            GameKey::UpRight => self.tactical_step((1, -1)),
            GameKey::DownLeft => self.tactical_step((-1, 1)),
            GameKey::DownRight => self.tactical_step((1, 1)),
            GameKey::Char('a') => self.open_tactical_aim(TacticalIntent::Swing),
            // `s`, because the abstract fight has called this `[s]pecial`
            // since long before there was a board to fight on — one model
            // teaching a key the other refuses is what the shared letter
            // buys. Free here: this handler is the only reader of a key in
            // `Mode::TacticalBattle`, so nothing fell through to the map's
            // own `s`.
            GameKey::Char('s') => {
                if self.tactical_routine_rows().is_empty() {
                    self.refuse("Nothing to run.");
                    return;
                }
                self.menu_selected = 0;
                self.mode = Mode::TacticalRoutine;
            }
            // Uppercase, because lowercase letters are row selectors.
            GameKey::Char('E') => {
                if let Some(game) = &mut self.game {
                    game.tactical_end_turn();
                }
                self.after_tactical_action();
            }
            _ => {}
        }
    }

    /// Picks the routine the acting body will run, then aims it.
    pub(crate) fn handle_tactical_routine_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::TacticalBattle;
            return;
        }
        let rows = self.tactical_routine_rows();
        let Some(row) = self.selected_index(key, rows.len()) else {
            return;
        };
        let Some(option) = rows.get(row) else { return };
        if let Some(why) = option.unavailable.clone() {
            self.refuse(why);
            return;
        }
        // `SpecialOption::index` is a position in `actor_abilities`, which
        // is what `tactical_use_routine` indexes — never the row.
        self.open_tactical_aim(TacticalIntent::Routine(option.index));
    }

    /// Moves the cell cursor and commits what was chosen to it.
    ///
    /// `handle_field_routine_cell_key`'s shape, and clamped the same way:
    /// the cursor is held on the board, so a commit always names a cell the
    /// engine can answer about.
    pub(crate) fn handle_tactical_aim_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_tactical = None;
            self.tactical_cursor = None;
            self.mode = Mode::TacticalBattle;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let Some(view) = game.tactical_view() else {
            self.mode = Mode::Playing;
            return;
        };
        let side = view.board.side;
        let Some((cx, cy)) = self.tactical_cursor else {
            self.mode = Mode::TacticalBattle;
            return;
        };
        let (dx, dy) = match key {
            GameKey::Up => (0, -1),
            GameKey::Down => (0, 1),
            GameKey::Left => (-1, 0),
            GameKey::Right => (1, 0),
            // The same eight the body walks. A numpad that steers a body but
            // not the cursor it aims with reads as one of the two being
            // broken.
            GameKey::UpLeft => (-1, -1),
            GameKey::UpRight => (1, -1),
            GameKey::DownLeft => (-1, 1),
            GameKey::DownRight => (1, 1),
            GameKey::Enter => {
                self.commit_tactical_aim((cx, cy));
                return;
            }
            _ => return,
        };
        self.tactical_cursor = Some(((cx + dx).clamp(0, side - 1), (cy + dy).clamp(0, side - 1)));
    }

    /// Which screen the fight that just opened belongs on.
    ///
    /// **The engine decided the model; this only reads which one it chose.**
    /// `Game::start_battle` is the router — a per-call-site decision here
    /// would have to re-derive its three gates, and the pursuit path that
    /// opens a fight against a guardian and against a town patrol is one
    /// call site for both.
    pub(crate) fn opened_battle_mode(&self) -> Mode {
        match self.game.as_ref().is_some_and(|g| g.in_tactical_battle()) {
            true => Mode::TacticalBattle,
            false => Mode::Battle,
        }
    }

    /// The routines the acting body may run, as picker rows.
    pub fn tactical_routine_rows(&self) -> Vec<SpecialOption> {
        self.game
            .as_ref()
            .map(|g| g.tactical_routine_options())
            .unwrap_or_default()
    }

    /// Whether the fight is waiting on a key rather than on the pacing loop.
    ///
    /// **Every party body is the player's to command**, companions
    /// included, so this is not "is the player acting".
    pub fn tactical_player_turn(&self) -> bool {
        self.game
            .as_ref()
            .is_some_and(|g| g.tactical_awaits_input())
    }

    /// Spends the wild side's turns, one every `TACTICAL_TURNS_PER_SECOND`.
    ///
    /// `advance_reveal`'s sibling and its counterpart in the second combat
    /// model, called every frame from the same place. The carry is what
    /// makes the pace independent of the frame rate, and the loop is a
    /// `while` so a frame long enough to owe two turns spends two rather
    /// than dropping one.
    pub fn advance_tactical(&mut self, dt: f32) {
        if !matches!(
            self.mode,
            Mode::TacticalBattle | Mode::TacticalRoutine | Mode::TacticalAim
        ) {
            self.tactical_carry = 0.0;
            return;
        }
        if self.tactical_player_turn() {
            // Held at zero rather than accumulated, so the first wild body
            // to act after the player's turn waits a full beat and the
            // handover is legible.
            self.tactical_carry = 0.0;
            return;
        }
        self.tactical_carry += dt * TACTICAL_TURNS_PER_SECOND;
        while self.tactical_carry >= 1.0 {
            self.tactical_carry -= 1.0;
            let ran = self
                .game
                .as_mut()
                .map(|g| g.tactical_ai_turn())
                .unwrap_or(false);
            if !ran {
                break;
            }
            if self.settle_tactical_end() {
                return;
            }
        }
    }

    /// One press of a direction.
    fn tactical_step(&mut self, dir: (i32, i32)) {
        let Some(game) = &mut self.game else { return };
        match game.tactical_step(dir) {
            StepOutcome::Moved => {}
            StepOutcome::Departed | StepOutcome::Refused => {}
        }
        self.after_tactical_action();
    }

    /// Opens the cell cursor on the acting body's own cell.
    ///
    /// Its own cell and not the nearest hostile: a `Radius` centred on the
    /// caster is a legal aim, and starting the cursor somewhere the player
    /// did not choose is how a blast lands on the party.
    fn open_tactical_aim(&mut self, intent: TacticalIntent) {
        let Some(game) = &mut self.game else { return };
        let Some(view) = game.tactical_view() else {
            return;
        };
        let Some(active) = view.active else { return };
        let acting = view.order[active].entity;
        let Some(body) = view.bodies.iter().find(|b| b.entity == acting) else {
            return;
        };
        self.tactical_cursor = Some(body.cell);
        self.pending_tactical = Some(intent);
        self.mode = Mode::TacticalAim;
    }

    /// Spends the chosen action on the cell the cursor is over.
    fn commit_tactical_aim(&mut self, aim: (i32, i32)) {
        let Some(intent) = self.pending_tactical.take() else {
            self.mode = Mode::TacticalBattle;
            return;
        };
        self.tactical_cursor = None;
        self.mode = Mode::TacticalBattle;
        let Some(game) = &mut self.game else { return };
        let landed = match intent {
            TacticalIntent::Swing => match game.tactical_occupant(aim) {
                Some(target) => game.tactical_attack(target),
                None => false,
            },
            TacticalIntent::Routine(index) => game.tactical_use_routine(index, aim),
        };
        if !landed {
            self.refuse("Not from here.");
        }
        self.after_tactical_action();
    }

    /// What every tactical action owes: the fight may have ended inside it.
    fn after_tactical_action(&mut self) {
        self.settle_tactical_end();
    }

    /// Leaves for the results page if the fight is over, and reports whether
    /// it did.
    ///
    /// **`has_active_battle` and not the tactical resource alone.** A fight
    /// that ended took `TacticalBattle` with it, and the screen the player
    /// is owed is the same results page every other fight ends on.
    fn settle_tactical_end(&mut self) -> bool {
        let over = self.game.as_ref().is_some_and(|g| !g.has_active_battle());
        if !over {
            return false;
        }
        self.pending_tactical = None;
        self.tactical_cursor = None;
        self.tactical_carry = 0.0;
        self.mode = Mode::BattleResult;
        self.restart_reveal();
        self.check_game_over();
        true
    }
}
