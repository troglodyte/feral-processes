//! The mouse's one entry into app-core for anything drawn in the map pane.
//!
//! The renderer resolves a pixel to a cell through the layout the draw pass
//! published and hands the cell here with the button; what the cell *means*
//! is decided from `App::mode`, the way `handle_key` decides a key. `crates/
//! gui` has no `match app.mode` of its own, so a gesture can never do on the
//! screen what it would not do here.
//!
//! Every gesture ends in a call the keyboard already makes — a walk is
//! `Game::tactical_step` a cell a beat, a swing is `Game::tactical_attack`,
//! an aim is `commit_tactical_aim` — so there is no second spelling of any
//! of them, and a refusal says what the key's would.

use crate::{App, EditorButton, GameKey, HubPageKey, Mode, PointerButton, TargetMark};

/// Which way a scroll marker or the wheel moves the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// A hub's two halves, as a pointer target the wheel and a click on bare
/// background can resolve to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Rail,
    Page,
}

/// What the pointer is over, resolved by the renderer to an identity and
/// never a pixel. **Every arm ends in a call the keyboard already makes**
/// (`handle_pointer_event`), so a click can do nothing a key cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerHit {
    /// The sprite editor's canvas cell.
    Cell(u8, u8),
    Swatch(u8),
    Button(EditorButton),
    Subject(usize),
    /// A rail entry.
    Page(HubPageKey),
    /// A tab of the held page, by position.
    Tab(usize),
    /// A selectable row of the shown screen, by its ordinal among the
    /// screen's `Row::Item`s — the index its handler selects by.
    Row(usize),
    /// A keycap: the key it names.
    Key(GameKey),
    /// A "↑/↓ n more" marker on the page's list.
    More(Direction),
    MapTile(i32, i32),
    Region(Region),
}

/// One frame of pointer input, resolved against the frame just drawn. The
/// reader in gui builds it; nothing in it is a pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerEvent {
    /// The topmost target under the pointer.
    pub hover: Option<PointerHit>,
    /// A button went down this frame, over `hover`.
    pub press: bool,
    /// A button is held and the pointer moved.
    pub drag: bool,
    /// A button was released this frame.
    pub up: bool,
    /// A press and its release landed on one target without a drag between:
    /// a click on `hover`. **A hub acts on this**, never on the press, so a
    /// drag that starts on a row clicks nothing. There is no double-click
    /// flag: a click on the row the cursor is already on is `Enter`, so two
    /// clicks are select then `Enter` once, however slowly they come.
    pub click: bool,
    pub button: PointerButton,
    /// Whole wheel notches this frame, positive up.
    pub wheel: i32,
}

impl App {
    /// The one entry for the pointer layer: records what is hovered and
    /// turns a press or the wheel into the keys it stands for.
    pub fn handle_pointer_event(&mut self, event: PointerEvent) {
        self.pointer_hover = event.hover;
        let Some(hit) = event.hover else { return };
        if event.wheel != 0 {
            self.hub_pointer_wheel(hit, event.wheel);
        }
        if event.click && event.button == PointerButton::Primary {
            self.pointer_click(hit);
        }
    }

    /// Whether `hit` is what the pointer is over, for a renderer to tint.
    pub fn hovered(&self, hit: &PointerHit) -> bool {
        self.pointer_hover.as_ref() == Some(hit)
    }

    fn pointer_click(&mut self, hit: PointerHit) {
        match hit {
            PointerHit::Key(key) => self.handle_key(key),
            PointerHit::Page(_)
            | PointerHit::Tab(_)
            | PointerHit::Row(_)
            | PointerHit::More(_)
            | PointerHit::Region(_) => self.hub_pointer_click(hit),
            // The sprite editor and the map have their own pointer paths
            // until they move onto this one.
            PointerHit::Cell(..)
            | PointerHit::Swatch(_)
            | PointerHit::Button(_)
            | PointerHit::Subject(_)
            | PointerHit::MapTile(..) => {}
        }
    }
}

impl App {
    /// Records the cell under the pointer this frame, `None` off the pane.
    ///
    /// In `Mode::TacticalAim` the cell cursor follows the pointer — but
    /// only when the pointer **enters a different cell**, never every
    /// frame, or a pointer parked over the board would snap the cursor back
    /// after every arrow key. Off the board the cursor stays where it was:
    /// the pane shows cells past the edge whenever the camera sits near
    /// one, and a cursor dragged to the edge by every pointer twitch out
    /// there would fight the arrows exactly as a parked pointer would.
    pub fn set_map_hover(&mut self, cell: Option<(i32, i32)>) {
        let entered = cell.filter(|&c| Some(c) != self.map_hover);
        self.map_hover = cell;
        if self.mode != Mode::TacticalAim {
            return;
        }
        if let Some(cell) = entered
            && self.on_board(cell)
        {
            self.tactical_cursor = Some(cell);
        }
    }

    /// Whether `cell` is one of the board's own — the pane can name cells
    /// past the edge, and `tile_at_px` does not know where the edge is.
    fn on_board(&mut self, cell: (i32, i32)) -> bool {
        self.game
            .as_mut()
            .and_then(|g| g.tactical_view())
            .is_some_and(|v| {
                (0..v.board.side).contains(&cell.0) && (0..v.board.side).contains(&cell.1)
            })
    }

    /// A click on a cell of whatever the map pane is drawing.
    ///
    /// On the surface map a primary click travels; on a battle map it
    /// walks, marks or swings; with the aim cursor up it commits the aim
    /// (primary) or backs out of it (secondary). Every other screen the
    /// pane draws under — the Stack corridor, the base grid, a popup —
    /// takes nothing from the pointer.
    pub fn map_click(&mut self, cell: (i32, i32), button: PointerButton) {
        match self.mode {
            Mode::Playing if button == PointerButton::Primary => self.travel_to(cell.0, cell.1),
            Mode::TacticalBattle => self.tactical_board_click(cell, button),
            Mode::TacticalAim => match button {
                // Committed at the pointer's cell, which the cursor has
                // already followed to; a cell past the edge is nothing to
                // aim at, so the aim stays open rather than being spent on
                // a refusal Enter could never produce.
                PointerButton::Primary if self.on_board(cell) => {
                    self.tactical_cursor = Some(cell);
                    self.commit_tactical_aim(cell);
                }
                PointerButton::Primary => {}
                PointerButton::Secondary => self.handle_tactical_aim_key(GameKey::Esc),
                PointerButton::Middle => {}
            },
            _ => {}
        }
    }

    /// What a click means on the board on the player's turn: a hostile is
    /// marked, the marked hostile is swung at, and anything else is walked
    /// to — the click that clears a mark still does what it would have
    /// done without one.
    ///
    /// The gates come first and are the keyboard's: a live walk is stopped
    /// and the click swallowed, and on the wild side's turn or under
    /// auto-attack the click does exactly what a key does there, which is
    /// stop the robot and nothing else.
    fn tactical_board_click(&mut self, cell: (i32, i32), button: PointerButton) {
        if self.tactical_auto {
            self.tactical_auto = false;
            self.status_line = Some("Auto-attack off.".to_string());
            self.stop_player_walk();
            return;
        }
        if !self.tactical_player_turn() || self.stop_player_walk() {
            return;
        }
        if button != PointerButton::Primary {
            self.tactical_target = None;
            return;
        }
        match self.hostile_at(cell) {
            Some((hostile, mark)) => self.tactical_pick_or_swing(hostile, mark),
            None => {
                self.tactical_target = None;
                self.tactical_walk_to(cell);
            }
        }
    }

    /// The hostile body standing on `cell`, with the mark a click on it
    /// would leave — the acting body's turn, read off the live view.
    fn hostile_at(&mut self, cell: (i32, i32)) -> Option<(crate::Entity, TargetMark)> {
        let game = self.game.as_mut()?;
        let view = game.tactical_view()?;
        let hostile = game
            .tactical_occupant(cell)
            .filter(|&body| view.bodies.iter().any(|b| b.entity == body && b.is_hostile))?;
        let actor = view.order[view.active?].entity;
        Some((
            hostile,
            TargetMark {
                actor,
                round: view.round,
                target: hostile,
            },
        ))
    }

    /// A primary click on a hostile: the first marks it, the second swings.
    /// A refused swing keeps the mark so the player can walk and click
    /// again; a landed one clears it.
    fn tactical_pick_or_swing(&mut self, hostile: crate::Entity, mark: TargetMark) {
        let marked = self.tactical_target_cell().is_some()
            && self.tactical_target.map(|m| m.target) == Some(hostile);
        if !marked {
            self.tactical_target = Some(mark);
            return;
        }
        let landed = self
            .game
            .as_mut()
            .is_some_and(|game| game.tactical_attack(hostile));
        if landed {
            self.tactical_target = None;
        } else {
            self.refuse("Not from here.");
        }
        self.after_tactical_action();
    }

    /// Where the marked hostile stands, read off the live board — and the
    /// mark dropped when it no longer can be: its body is off the board, or
    /// the turn that marked it has passed. The renderer draws off this and
    /// the click path validates through it, so no turn-change event is
    /// needed to keep a stale mark from reaching `Game::tactical_attack`.
    pub fn tactical_target_cell(&mut self) -> Option<(i32, i32)> {
        let mark = self.tactical_target?;
        let cell = self
            .game
            .as_mut()
            .and_then(|g| g.tactical_view())
            .filter(|v| {
                v.round == mark.round && v.active.map(|i| v.order[i].entity) == Some(mark.actor)
            })
            .and_then(|v| v.bodies.into_iter().find(|b| b.entity == mark.target))
            .map(|b| b.cell);
        if cell.is_none() {
            self.tactical_target = None;
        }
        cell
    }

    /// The walk a primary click on the hovered cell would commit, for the
    /// board to wash before it is committed. Empty whenever the click would
    /// not walk: off the player's turn, mid-walk, off the board, on the
    /// acting body's own cell, or out of reach.
    pub fn tactical_hover_path(&mut self) -> Vec<(i32, i32)> {
        if self.mode != Mode::TacticalBattle || !self.tactical_player_turn() {
            return Vec::new();
        }
        let Some(cell) = self.map_hover else {
            return Vec::new();
        };
        let Some(game) = &mut self.game else {
            return Vec::new();
        };
        if game.tactical_walking() {
            return Vec::new();
        }
        let Some(view) = game.tactical_view() else {
            return Vec::new();
        };
        let own = game
            .tactical_occupant(cell)
            .is_some_and(|body| Some(body) == game.tactical_actor());
        if own || !view.reachable.contains(&cell) {
            return Vec::new();
        }
        game.tactical_path_to(cell)
    }
}
