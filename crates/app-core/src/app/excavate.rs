//! The Excavation plan: `Mode::Excavate`, where a base's next wing is drawn
//! before anything is cut.
//!
//! A **mode, not an action**. Nothing here reaches `Game::tick`, so a player
//! can plan a whole corridor without spending a turn on it — walking a
//! cursor around a big box costs nothing, whatever the plan turns out to
//! mark.

use crate::*;
use feral_processes_engine::crops::CropId;

/// How far from the party the cursor may be walked, in cells.
///
/// A bound rather than the unbounded space `BaseGrid` actually is, for two
/// reasons that happen to agree: the map pane is centred on the party and
/// does not scroll while the mode is open, so a cursor past the pane's edge
/// would be a cursor the player cannot see; and it caps how much rock one
/// keypress can mark, which is what stops a mis-drawn box becoming a plan
/// the player has to erase cell by cell.
const CURSOR_RANGE: i32 = 12;

/// One row of the farm plot's recipe picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CropRow {
    pub id: CropId,
    pub label: String,
    pub description: String,
}

/// One row of the brush picker — what picking it sets, what it is called,
/// and the swatch a finish row draws beside its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrushRow {
    pub brush: Option<FinishOrder>,
    pub label: String,
    pub shade: Option<feral_processes_engine::floors::FloorShade>,
}

/// What a key meant to the box cursor `Excavate` and `Stockpile` share.
pub(crate) enum BoxKey {
    /// The second `space`: the box between `anchor` and `corner` is the
    /// caller's to commit.
    Commit {
        anchor: (i32, i32),
        corner: (i32, i32),
    },
    /// Handled here — a move, an anchor drop, or leaving the mode.
    Done,
    /// Not the cursor's key; the mode's own handler decides.
    Unhandled(GameKey),
}

impl App {
    /// Moves the cursor, drops and lifts the anchor, and leaves the mode —
    /// everything the two box modes do identically.
    ///
    /// `space` is the one verb: the first drops an anchor, the second hands
    /// back the box the cursor has been previewing. Whether that box marks
    /// or clears is the **engine's** decision, from the anchor cell — see
    /// `Game::toggle_mark_box` — so this never has to hold a mode of its
    /// own for erasing.
    ///
    /// `esc` takes the anchor back before it takes the mode, so one press is
    /// never two undos.
    pub(crate) fn box_cursor_key(&mut self, key: GameKey) -> BoxKey {
        if key == GameKey::Esc {
            if self.excavate_anchor.take().is_none() {
                self.excavate_cursor = None;
                self.mode = Mode::Playing;
            }
            return BoxKey::Done;
        }
        let Some(party) = self.game.as_ref().and_then(|g| g.base_pos()) else {
            // The party left base space out from under the mode — nothing
            // in here can happen anywhere else, so it closes rather than
            // drawing over the open grid.
            self.excavate_cursor = None;
            self.excavate_anchor = None;
            self.mode = Mode::Playing;
            return BoxKey::Done;
        };
        let Some((cx, cy)) = self.excavate_cursor else {
            self.mode = Mode::Playing;
            return BoxKey::Done;
        };
        let (dx, dy) = match key {
            GameKey::Up | GameKey::Char('k') => (0, -1),
            GameKey::Down | GameKey::Char('j') => (0, 1),
            GameKey::Left | GameKey::Char('h') => (-1, 0),
            GameKey::Right | GameKey::Char('l') => (1, 0),
            GameKey::Char(' ') | GameKey::Enter => {
                return match self.excavate_anchor.take() {
                    Some(anchor) => BoxKey::Commit {
                        anchor,
                        corner: (cx, cy),
                    },
                    None => {
                        self.excavate_anchor = Some((cx, cy));
                        BoxKey::Done
                    }
                };
            }
            other => return BoxKey::Unhandled(other),
        };
        self.excavate_cursor = Some((
            (cx + dx).clamp(party.0 - CURSOR_RANGE, party.0 + CURSOR_RANGE),
            (cy + dy).clamp(party.1 - CURSOR_RANGE, party.1 + CURSOR_RANGE),
        ));
        BoxKey::Done
    }

    pub(crate) fn handle_excavate_key(&mut self, key: GameKey) {
        match self.box_cursor_key(key) {
            BoxKey::Commit { anchor, corner } => {
                if let Some(game) = &mut self.game {
                    if self.excavate_plot {
                        game.toggle_plot_box(anchor, corner);
                    } else {
                        game.toggle_mark_box(anchor, corner, self.excavate_brush.as_ref());
                    }
                }
            }
            BoxKey::Unhandled(GameKey::Char('P')) => {
                // Inert with no crop loaded, `[F]`'s rule: a plot with
                // nothing to grow is a tool that does nothing.
                if self
                    .game
                    .as_ref()
                    .is_some_and(|g| !g.crop_defs().is_empty())
                {
                    self.excavate_plot = !self.excavate_plot;
                }
            }
            BoxKey::Unhandled(GameKey::Char('F')) => {
                // Inert over an empty `FloorDb`: plain is the only brush
                // there is, and a picker of one row is a screen that does
                // nothing.
                if self.excavate_brush_rows().len() > 1 {
                    self.mode = Mode::ExcavateBrush;
                }
            }
            BoxKey::Done | BoxKey::Unhandled(_) => {}
        }
    }

    /// The picker's rows: plain, every loaded finish in id order, strip.
    /// The one list both the keys and the popup read, so a row the player
    /// sees is the row a key picks. Empty with no game, and plain alone
    /// with an empty `FloorDb` — strip with no finish to lift is not a
    /// brush anyone asked for.
    pub fn excavate_brush_rows(&self) -> Vec<BrushRow> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let defs = game.floor_defs();
        let mut rows = vec![BrushRow {
            brush: None,
            label: "plain — cut rock, lay tile".to_string(),
            shade: None,
        }];
        if defs.is_empty() {
            return rows;
        }
        rows.extend(defs.into_iter().map(|d| BrushRow {
            brush: Some(FinishOrder::Apply(d.id)),
            label: d.name,
            shade: Some(d.shade),
        }));
        rows.push(BrushRow {
            brush: Some(FinishOrder::Strip),
            label: "strip — lift a finish".to_string(),
            shade: None,
        });
        rows
    }

    /// The row the brush in hand sits on, so the picker opens there and
    /// Enter alone keeps it. A brush naming a finish no longer loaded falls
    /// back to plain's row.
    pub(crate) fn excavate_brush_row(&self) -> usize {
        self.excavate_brush_rows()
            .iter()
            .position(|r| r.brush == self.excavate_brush)
            .unwrap_or(0)
    }

    pub(crate) fn handle_excavate_brush_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Excavate;
            return;
        }
        let mut rows = self.excavate_brush_rows();
        if let Some(idx) = self.selected_index(key, rows.len()) {
            self.excavate_brush = rows.swap_remove(idx).brush;
            self.excavate_plot = false;
            self.mode = Mode::Excavate;
        }
    }

    /// The key bar while a box cursor is on the map, in place of the
    /// playing map's own — the tactical aim cursor's convention, so a mode
    /// that takes the arrows away says what they do now. `None` outside the
    /// two box modes and the brush picker over them.
    ///
    /// The tool in hand rides on its own key's label rather than a header
    /// of its own. `[F]` and `[P]` appear only where they work, the keys'
    /// own "inert with nothing loaded" rule.
    pub fn plan_keybar(&self) -> Option<Vec<(String, String)>> {
        let excavating = match self.mode {
            Mode::Excavate | Mode::ExcavateBrush => true,
            Mode::Stockpile => false,
            _ => return None,
        };
        let row = |k: &str, l: String| (k.to_string(), l);
        let (space, esc) = match self.excavate_anchor {
            Some(_) => ("finish box", "drop corner"),
            None => ("box", "done"),
        };
        let mut rows = vec![row("↑↓←→", "move".into()), row("Space", space.into())];
        if excavating && let Some(game) = &self.game {
            let defs = game.floor_defs();
            if !defs.is_empty() {
                let brush = match &self.excavate_brush {
                    None => "plain",
                    Some(FinishOrder::Apply(id)) => defs
                        .iter()
                        .find(|d| &d.id == id)
                        .map_or(id.as_str(), |d| d.name.as_str()),
                    Some(FinishOrder::Strip) => "strip",
                };
                rows.push(row("F", format!("brush: {brush}")));
            }
            if !game.crop_defs().is_empty() {
                let plot = if self.excavate_plot {
                    "plot (on)"
                } else {
                    "plot"
                };
                rows.push(row("P", plot.into()));
            }
        }
        rows.push(row("Esc", esc.into()));
        Some(rows)
    }

    /// The recipe picker's rows, every loaded crop in id order — the one
    /// list the keys and the popup both read.
    pub fn farm_recipe_rows(&self) -> Vec<CropRow> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        game.crop_defs()
            .into_iter()
            .map(|d| CropRow {
                id: d.id,
                label: d.name,
                description: d.description,
            })
            .collect()
    }

    /// The row the plot's current recipe sits on, so the picker opens
    /// there; row 0 for an unsown plot.
    pub(crate) fn farm_recipe_row(&self) -> usize {
        let current = self
            .game
            .as_ref()
            .zip(self.pending_plot)
            .and_then(|(g, plot)| g.plot_recipe(plot));
        self.farm_recipe_rows()
            .iter()
            .position(|r| Some(&r.id) == current.as_ref())
            .unwrap_or(0)
    }

    /// A plan, like marking: no tick, whatever the row.
    pub(crate) fn handle_farm_recipe_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_plot = None;
            self.mode = Mode::Playing;
            return;
        }
        let mut rows = self.farm_recipe_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let id = rows.swap_remove(idx).id;
        if let (Some(plot), Some(game)) = (self.pending_plot.take(), self.game.as_mut())
            && let Err(reason) = game.set_plot_recipe(plot, &id)
        {
            self.refuse(reason);
        }
        self.mode = Mode::Playing;
    }
}
