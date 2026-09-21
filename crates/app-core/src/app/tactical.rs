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
//!
//! **A body's walk is paced too, and faster than its turn.** The engine
//! spends a hostile's turn one `AiBeat` at a time; this decides how long
//! each one is on screen, and a cell of an approach is not worth as long as
//! the blow at the end of it.
//!
//! **And the hand-over is paced separately again**, because what has to fit
//! inside it is not a reading time but the camera: the blow held on screen
//! long enough to read, then the pan to whoever is next. See
//! `TACTICAL_HANDOVER_SECONDS`.

use crate::{
    App, GameKey, Mode, TACTICAL_HANDOVER_SECONDS, TACTICAL_STEPS_PER_SECOND,
    TACTICAL_TURNS_PER_SECOND, TacticalIntent,
};
use feral_processes_engine::battle::{SpecialOption, SpecialTargeting};
use feral_processes_engine::tactical::ai::AiBeat;
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
        // **Above the wait below, and above every action.** Auto-attack is a
        // mode rather than an action, so arming and stopping it are the two
        // things this screen answers whoever holds the turn — a stop that only
        // landed on the player's own turn would have the player hunting for a
        // window they cannot see the edges of, and an `[A]` that did the same
        // would read as the key being broken.
        //
        // The stopping key is **swallowed**: it is the key that stops a robot
        // mid-swing, and spending it as a step would walk the body the player
        // was reaching in to save.
        if self.tactical_auto {
            self.tactical_auto = false;
            self.status_line = Some("Auto-attack off.".to_string());
            return;
        }
        if key == GameKey::Char('A') {
            self.tactical_auto = true;
            self.status_line = Some("Auto-attack on. Any key stops it.".to_string());
            return;
        }
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
            // `d`, the letter `Game::battle_action_options` has bound to
            // Defend since long before there was a board — `s`'s argument
            // again, and lowercase for the same reason `a` and `s` are:
            // this screen is the map with a different tile source, not a
            // list of rows.
            GameKey::Char('d') => {
                if let Some(game) = &mut self.game {
                    game.tactical_defend();
                }
                self.after_tactical_action();
            }
            // Uppercase, because lowercase letters are row selectors.
            GameKey::Char('E') => {
                if let Some(game) = &mut self.game {
                    game.tactical_end_turn();
                }
                self.after_tactical_action();
            }
            // Uppercase, `E`'s reason — and free here: `r` already backs out
            // of nothing on this screen (`r_is_not_a_second_way_into_the_
            // picker`), so the shift-slip costs nothing either.
            GameKey::Char('R') => self.auto_resolve(),
            // Uppercase, `E`'s reason again — `Game::tactical_revert`'s own
            // refusal while nothing is emulating is silent here exactly as
            // `d`'s is: `d` above never checks its own bool either, and a
            // key with no submenu to open follows that shape rather than
            // `s`'s, which refuses aloud only because it is deciding whether
            // to open one. `V` and not the group model's lowercase `r`: that
            // letter already selects a row on the routine list one screen
            // over, and `battle_action_options` itself made Revert's key
            // lowercase only because it is a row in an action *list* — this
            // screen has no such list, `d`'s and `s`'s own reason for being
            // lowercase.
            GameKey::Char('V') => {
                if let Some(game) = &mut self.game {
                    game.tactical_revert();
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
        // Emulate has no cell to aim — `Game::tactical_emulate` is its own
        // door — so it leaves here for `Mode::TacticalEmulate` instead of
        // `Mode::TacticalAim`, todo #100 Task 6.
        if option.targeting == SpecialTargeting::Image {
            self.pending_tactical_emulate = Some(option.index);
            self.menu_selected = 0;
            self.mode = Mode::TacticalEmulate;
            return;
        }
        // `SpecialOption::index` is a position in `actor_abilities`, which
        // is what `tactical_use_routine` indexes — never the row.
        // A relocation aims twice — a body at arm's length, then a cell to
        // send it to — so it opens the cursor on the first of the two and
        // `commit_tactical_aim` re-opens it on the second. Routed off the
        // engine's own `targeting`, `SpecialTargeting::Image` one line up.
        if option.targeting == SpecialTargeting::Relocate {
            self.open_tactical_aim(TacticalIntent::TeleportSubject(option.index));
            return;
        }
        self.open_tactical_aim(TacticalIntent::Routine(option.index));
    }

    /// Picks which learned image the routine chosen in
    /// `Mode::TacticalRoutine` invokes, then commits straight through
    /// `Game::tactical_emulate` — todo #100 Task 6. Esc returns to the
    /// routine list, spending nothing, the same shape a cancelled aim has.
    pub(crate) fn handle_tactical_emulate_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_tactical_emulate = None;
            self.mode = Mode::TacticalRoutine;
            return;
        }
        let Some(game) = &self.game else { return };
        let options = game.emulation_options();
        let Some(idx) = self.selected_index(key, options.len()) else {
            return;
        };
        let Some(index) = self.pending_tactical_emulate.take() else {
            return;
        };
        let species = options[idx].species.clone();
        self.mode = Mode::TacticalBattle;
        let Some(game) = &mut self.game else { return };
        if !game.tactical_emulate(index, &species) {
            self.refuse("Couldn't emulate that.");
        }
        self.after_tactical_action();
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

    /// Spends the wild side's turn a beat at a time: a cell of a walk every
    /// `TACTICAL_STEPS_PER_SECOND`, an action every
    /// `TACTICAL_TURNS_PER_SECOND`.
    ///
    /// `advance_reveal`'s sibling and its counterpart in the second combat
    /// model, called every frame from the same place. The carry is what
    /// makes the pace independent of the frame rate, and the loop spends
    /// every beat a long frame owes rather than dropping the surplus.
    ///
    /// **The wait is derived from the fight rather than remembered**, so
    /// there is no second piece of pacing state to hold in step with the
    /// first: `Game::tactical_walking` is true exactly between a body's
    /// first step and its last, which is the same span that owes the faster
    /// rate.
    pub fn advance_tactical(&mut self, dt: f32) {
        if !matches!(
            self.mode,
            Mode::TacticalBattle | Mode::TacticalRoutine | Mode::TacticalAim
        ) {
            self.tactical_carry = 0.0;
            return;
        }
        if self.tactical_player_turn() && !self.tactical_auto {
            // Held at zero rather than accumulated, so the first wild body
            // to act after the player's turn waits a full beat and the
            // handover is legible.
            self.tactical_carry = 0.0;
            return;
        }
        self.tactical_carry += dt;
        // Read before the loop's `&mut self.game` borrow.
        let auto = self.tactical_auto;
        loop {
            let beat = self.tactical_beat();
            if self.tactical_carry < beat {
                return;
            }
            self.tactical_carry -= beat;
            // **The flag picks the door, not whose turn it is.** Auto-attack
            // means one door for the whole board, so the party's turns are
            // paced by the three waits the wild side's already are and a round
            // reads at one speed. The narrower door stays for the fight the
            // player is fighting, where a party body's turn is theirs and this
            // loop must not touch it.
            let spent = self
                .game
                .as_mut()
                .map(|g| match auto {
                    true => g.tactical_auto_beat(),
                    false => g.tactical_ai_beat(),
                })
                .unwrap_or(AiBeat::Idle);
            if spent == AiBeat::Idle {
                return;
            }
            if self.settle_tactical_end() {
                return;
            }
        }
    }

    /// How long the wild side waits before its next beat.
    ///
    /// Three waits, each derived from the fight rather than remembered — a
    /// cell of a walk, the hand-over before a body has spent anything, and
    /// the pause between arriving and striking. The middle one is the one
    /// the camera lives in: `Game::tactical_turn_opening` is true exactly
    /// between one body's action and the next body's first beat, which is
    /// the span `Fx::battle_center` holds the blow on screen and pans out of.
    fn tactical_beat(&self) -> f32 {
        let Some(game) = self.game.as_ref() else {
            return 1.0 / TACTICAL_TURNS_PER_SECOND;
        };
        if game.tactical_walking() {
            return 1.0 / TACTICAL_STEPS_PER_SECOND;
        }
        if game.tactical_turn_opening() {
            return TACTICAL_HANDOVER_SECONDS;
        }
        1.0 / TACTICAL_TURNS_PER_SECOND
    }

    /// One press of a direction.
    ///
    /// Four outcomes and nothing to choose between them: a step, a departure
    /// that closed the fight, a swing at whatever stood in the way, and a
    /// refusal all owe the same settle — which is what
    /// `after_tactical_action` is. The match is kept rather than dropped
    /// because `StepOutcome` is exhaustive here, so a fifth answer has to be
    /// read by somebody before it compiles.
    fn tactical_step(&mut self, dir: (i32, i32)) {
        let Some(game) = &mut self.game else { return };
        match game.tactical_step(dir) {
            StepOutcome::Moved => {}
            StepOutcome::Departed | StepOutcome::Struck | StepOutcome::Refused => {}
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
            // The first of a relocation's two cells spends nothing and
            // commits nothing: it re-opens the cursor on the body it just
            // named. Refused here rather than at the engine's door so the
            // player gets the message while the cursor is still up —
            // `Game::tactical_teleport` would refuse the same cell, but a
            // turn spent walking back to the routine list to find out is
            // not what the other intents do.
            TacticalIntent::TeleportSubject(index) => {
                if game.teleport_subjects().contains(&aim) {
                    self.open_tactical_aim(TacticalIntent::TeleportTo {
                        index,
                        subject: aim,
                    });
                    self.tactical_cursor = Some(aim);
                    return;
                }
                false
            }
            TacticalIntent::TeleportTo { index, subject } => {
                game.tactical_teleport(index, subject, aim)
            }
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

    /// Leaves for the results popup if the fight is over, and reports
    /// whether it did.
    ///
    /// **`has_active_battle` and not the tactical resource alone.** A fight
    /// that ended took `TacticalBattle` with it, and the board the popup is
    /// drawn over is the copy `Game::tactical_result_view` kept.
    pub(crate) fn settle_tactical_end(&mut self) -> bool {
        let over = self.game.as_ref().is_some_and(|g| !g.has_active_battle());
        if !over {
            return false;
        }
        self.pending_tactical = None;
        self.pending_tactical_emulate = None;
        self.tactical_cursor = None;
        self.tactical_carry = 0.0;
        // Per fight: the next one opens hands-on however this one ended.
        self.tactical_auto = false;
        self.mode = Mode::TacticalResult;
        self.check_game_over();
        true
    }

    /// Any key leaves the results popup for the map. No arrow carve-out, as
    /// `handle_battle_result_key` has: that screen scrolls a log pane, and
    /// this one lists results alone.
    pub(crate) fn handle_tactical_result_key(&mut self) {
        self.leave_battle_result();
        self.mode = Mode::Playing;
    }
}
