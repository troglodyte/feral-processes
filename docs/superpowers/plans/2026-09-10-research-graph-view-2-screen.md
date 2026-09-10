# Research Graph View, Phase 2: the screen

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw the research tree as a flow chart alongside the existing list —
arrow-key navigated, bought with `Enter`, toggled with `G` — reading Phase 1's
one layout derivation for both the cursor and the boxes.

**Architecture:** A `bool` on `App` chooses which of two draws the
`Mode::Research` arm makes; there is no new `Mode` and no census to update.
app-core converts `menu_selected` to a `ResearchId`, calls
`ResearchGraph::step`, and converts back — it computes no neighbours of its
own. gui's new `render/research_graph.rs` derives its geometry from `tiers`
and `widest` and draws the detail panel out of the *same* row builders the
list uses.

**Tech Stack:** Rust 2024, `bevy_ecs` (engine only), `bevy` + `bevy_egui`
behind `crates/gui/src/paint.rs`.

**Spec:** [`docs/superpowers/specs/2026-09-10-research-graph-view-design.md`](../specs/2026-09-10-research-graph-view-design.md)

**Depends on:** [Phase 1](2026-09-10-research-graph-view-1-layout.md), which
must be green on `cargo test --workspace` before this starts. It ships
`ResearchGraph`, `ResearchCell`, `GraphDir`, `Game::research_graph()` and
`ResearchGraph::step`, all reachable as `feral_processes_engine::*`.

## Global Constraints

- **No new `Mode` variant.** The view is a `bool` on `App`. `ALL_MODES`,
  `needs_status_banner` and every mode census stay as they are — and would
  not have failed to compile if a variant had been added, which is why this
  decision is worth restating here.
- **`App::menu_selected` is the one selection**, an index into
  `Game::research_nodes()`. The graph converts it to an id and back. Two
  cursors that could disagree is exactly what this avoids.
- **The toggle key is uppercase `G`.** `App::selected_index` returns `None`
  for any non-lowercase char (`crates/app-core/src/app/input.rs:108`), so a
  lowercase toggle would both pick a row and flip the view.
- **`render/` names no graphics library.** Everything draws through
  `Painter`; the fifteen operations plus `Color`/`Rect`/`TextDims`/`TextRun`
  are the whole vocabulary.
- **Every figure on the panel is a call into the list's own row builders.**
  A second wording of a node's materials or conversions is the copy that
  drifts.
- Gates: `cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`.

## Two spec figures were wrong; this plan carries the corrected ones

Verified on 2026-09-10 against `assets/research/` and
`crates/engine/src/text.rs`, not remembered:

1. **The longest unbreakable word is `Self-Execution` at 14 characters, not
   `Fabrication` at 11.** `text::wrap` splits on `split_whitespace()` only
   (`crates/engine/src/text.rs:16`), so a hyphen is not a break opportunity
   and `Self-Execution` cannot be split across two lines of a box. The fit
   constraint is 27% wider than the spec assumed.

   **The consequence, and it is load-bearing:** at the body font a box that
   fits 14 characters does not fit six across the graph pane at 1280x720. So
   **a box's label is drawn at `Metrics::small()`** (`font_size - 4`, which
   is 12px at 720p where the body is 16) rather than at `m.font_size`. That
   is what makes the shipped tree fit without eliding a shipped name, and
   `Metrics::small()` already exists for chrome that has to be smaller than
   the body. Task 2's test is the arbiter — if it fails, the knob to turn is
   `PANEL_FRACTION` or `GUTTER_X`, never the assertion.

2. **The field is `App::menu_selected`, not `App::selected`**
   (`crates/app-core/src/lib.rs:2569`). Named correctly throughout below.

## File Structure

| File | Responsibility |
|---|---|
| `crates/app-core/src/lib.rs` (modify) | `App::research_graph_view: bool`, beside `menu_selected` |
| `crates/app-core/src/app/progression.rs` (modify) | `handle_research_key` grows the toggle and the four arrows |
| `crates/app-core/src/tests/research.rs` (modify) | The toggle, the step, and the refusal |
| `crates/gui/src/render/research_graph.rs` (create) | Geometry, then the draw — the whole screen |
| `crates/gui/src/render/mod.rs` (modify) | `mod research_graph;` and the `Mode::Research` guard arm |
| `crates/gui/src/render/progression.rs` (modify) | `material_rows`/`conversion_rows` take a column budget and go `pub(super)` |
| `crates/gui/src/render/popup.rs` (modify) | `description_rows_at`, which `description_rows` becomes a call into |

---

### Task 1: The flag, the toggle and the arrows

**Files:**
- Modify: `crates/app-core/src/lib.rs` — one field on `App`, near
  `menu_selected` (line 2569)
- Modify: `crates/app-core/src/app/progression.rs` —
  `handle_research_key` (lines 53-77)
- Test: `crates/app-core/src/tests/research.rs`

**Interfaces:**
- Consumes: `Game::research_graph() -> ResearchGraph`,
  `ResearchGraph::{cell, step}`, `GraphDir`, `Game::research_nodes()`,
  `Game::unlock_research`, `App::{report, close_screen, menu_selected}`.
- Produces, for gui's Task 3:

```rust
impl App {
    /// Whether the research screen is drawing the graph rather than the
    /// list. A view flag and not a `Mode`, so no mode census moves.
    pub research_graph_view: bool,   // public field on App
}
```

`false` on a fresh `App` and reset to `false` by nothing — it is session
state that survives closing and reopening the screen, which is what a player
who prefers one view expects.

- [ ] **Step 1: Write the failing tests**

Append to `crates/app-core/src/tests/research.rs`:

```rust
/// The toggle is uppercase because `selected_index` treats every lowercase
/// letter as a row label past the digits — a lowercase toggle on a 34-row
/// screen would buy a node *and* flip the view on one keypress.
#[test]
fn g_toggles_the_graph_view_and_back() {
    let mut app = test_app(520);
    open_via_menu(&mut app, 'b', "Research");
    assert!(!app.research_graph_view, "the list is the opening view");
    app.handle_key(GameKey::Char('G'));
    assert!(app.research_graph_view);
    app.handle_key(GameKey::Char('G'));
    assert!(!app.research_graph_view);
    assert_eq!(app.mode, Mode::Research, "the toggle is not a mode change");
}

/// One selection, converted at the boundary. Toggling either way has to land
/// on the node the player was looking at, or the two views are two cursors.
#[test]
fn the_toggle_preserves_the_selected_node_in_both_directions() {
    let mut app = test_app(521);
    open_via_menu(&mut app, 'b', "Research");
    for _ in 0..5 {
        app.handle_key(GameKey::Down);
    }
    let before = selected_research_id(&app);
    app.handle_key(GameKey::Char('G'));
    assert_eq!(
        selected_research_id(&app),
        before,
        "flipping to the graph keeps the node"
    );
    app.handle_key(GameKey::Char('G'));
    assert_eq!(
        selected_research_id(&app),
        before,
        "and flipping back keeps it too"
    );
}

/// The whole reason the layout is derived in the engine: app-core computes
/// no neighbours. What an arrow does on this screen is whatever
/// `ResearchGraph::step` says, converted back through the id.
#[test]
fn an_arrow_in_the_graph_view_lands_where_step_says() {
    let mut app = test_app(522);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    for (key, dir) in [
        (GameKey::Down, GraphDir::Down),
        (GameKey::Right, GraphDir::Right),
        (GameKey::Up, GraphDir::Up),
        (GameKey::Left, GraphDir::Left),
    ] {
        let from = selected_research_id(&app);
        let want = app
            .game
            .as_ref()
            .unwrap()
            .research_graph()
            .step(&from, dir);
        app.handle_key(key);
        assert_eq!(
            selected_research_id(&app),
            want,
            "{key:?} must land where step({from:?}, {dir:?}) says"
        );
    }
}

/// `step` is total, so a held arrow at the edge of the grid is a no-op and
/// never a panic or a wrap. Driven through `handle_key` because that is the
/// path a key repeat actually takes.
#[test]
fn holding_an_arrow_at_the_edge_of_the_grid_does_nothing() {
    let mut app = test_app(523);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    for _ in 0..40 {
        app.handle_key(GameKey::Up);
        app.handle_key(GameKey::Left);
    }
    let corner = selected_research_id(&app);
    for _ in 0..10 {
        app.handle_key(GameKey::Up);
        app.handle_key(GameKey::Left);
    }
    assert_eq!(corner, selected_research_id(&app), "the corner clamps");
    assert_eq!(app.mode, Mode::Research, "and the screen stays open");
}

/// `Enter` is the same door the list's row keys call, so it must refuse the
/// same way and spend nothing on a refusal.
#[test]
fn enter_on_an_unaffordable_node_refuses_without_spending() {
    let mut app = test_app(524);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    // Walk to the deepest tier, where a fresh run can afford nothing.
    for _ in 0..10 {
        app.handle_key(GameKey::Right);
    }
    let target = selected_research_id(&app);
    let held_before = research_data_held(&app);
    app.handle_key(GameKey::Enter);
    assert!(
        app.status_line.is_some(),
        "a refusal has to say why, exactly as the list's does"
    );
    assert_eq!(
        research_data_held(&app),
        held_before,
        "nothing is spent on a refusal"
    );
    assert!(
        !app.game.as_ref().unwrap().is_researched(&target),
        "and nothing is unlocked"
    );
    assert_eq!(app.mode, Mode::Research, "the screen stays open");
}

/// Esc closes the screen from either view — the graph is a view of this
/// screen, not a screen of its own to back out of first.
#[test]
fn esc_closes_the_screen_from_the_graph_view() {
    let mut app = test_app(525);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    assert!(
        app.research_graph_view,
        "the chosen view survives closing the screen"
    );
}
```

Add the two helpers at the top of the file, under its module doc:

```rust
/// The id under the highlight, resolved the way the handlers do — through
/// `research_nodes()`, whose order changes as the player buys things.
fn selected_research_id(app: &App) -> String {
    let nodes = app.game.as_ref().expect("a run").research_nodes();
    nodes[app.menu_selected.min(nodes.len() - 1)].id.clone()
}

fn research_data_held(app: &App) -> u32 {
    let game = app.game.as_ref().expect("a run");
    game.banked(&game.research_currency())
}
```

`GraphDir` needs importing — `use feral_processes_engine::GraphDir;` beside
the file's existing `use feral_processes_engine::MessageKind;`.

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p feral-processes-app-core research
```

Expected: FAIL to compile — `App::research_graph_view` does not exist.

- [ ] **Step 3: Add the field**

`crates/app-core/src/lib.rs`, beside `menu_selected` (line 2569) and
defaulted `false` wherever `App` is constructed. Its doc comment says the two
things a reader needs: that it is a view flag rather than a `Mode` and why
(no census moves), and that it is session state that outlives the screen.

- [ ] **Step 4: Grow `handle_research_key`**

`crates/app-core/src/app/progression.rs:55`. The order of its branches is the
whole of the task:

1. `Esc` → `close_screen()`, unchanged and still first.
2. `GameKey::Char('G')` → flip the flag and return. **Before**
   `selected_index`, for the reason `handle_perks_key`'s `X` is checked
   before its own: uppercase is not a row selector but the check has to
   happen anyway, and putting it after would be a live hazard the day
   somebody widens `selected_index`.
3. `if !self.research_graph_view` → the existing body, untouched.
4. Otherwise, the graph arm:
   - The four arrows map to `GraphDir` and are the *only* keys that move the
     cursor here. Resolve `menu_selected` to an id through
     `research_nodes()`, call `graph.step(&id, dir)`, then scan
     `research_nodes()` for the landed id and write its index back to
     `menu_selected`. A scan of 34 entries per keypress; do not build an
     index map, and say so in a comment — the spec's decision, and the
     alternative is a second cursor.
   - `GameKey::Enter` → `Game::unlock_research` on the selected id, through
     `self.report(outcome)`. The same two calls the list's arm makes.
   - Everything else falls through and does nothing. Digits and lowercase
     letters are **not** row selectors in this view: on the graph a row
     number labels nothing the player can see.

Borrow discipline: `self.game` is borrowed immutably to build the graph and
read the ids, and mutably for `unlock_research`. Collect what you need into
owned `String`s and let the immutable borrow end before the mutable one
begins — the existing arm's comment at lines 60-62 records exactly this
constraint for `selected_index`, and it applies unchanged.

- [ ] **Step 5: Run the tests green**

```bash
cargo test -p feral-processes-app-core research
```

Expected: PASS, all six new plus the file's existing cases.

- [ ] **Step 6: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test -p feral-processes-app-core
git add crates/app-core/src/lib.rs crates/app-core/src/app/progression.rs \
        crates/app-core/src/tests/research.rs
git commit -m "feat(research): G toggles the graph view, arrows walk it

A bool on App rather than a Mode, so no mode census moves.
menu_selected stays the one selection: the graph converts it to an id,
calls ResearchGraph::step, and converts back. Enter is the same
unlock_research door the list calls, reported the same way.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: The geometry, and the fit

**Why this is its own task.** The fit is the risky half of the screen and it
is a pure function of the window and the tree's shape, so it can be held to a
measured assertion before a single box is drawn. If six tiers by nine slots
does not fit at 1280x720, that has to surface here and not as an eyeballed
screenshot.

**Files:**
- Create: `crates/gui/src/render/research_graph.rs` — geometry only in this
  task; the draw is Task 3
- Modify: `crates/gui/src/render/mod.rs` — `mod research_graph;` in
  alphabetical position (between `mod progression;` and what follows)

**Interfaces:**
- Consumes: `ResearchGraph::{tiers, widest, cells}`, `paint::{Painter, Rect}`,
  `text::Metrics`, `Metrics::{small, font_size, pad, line_height}`.
- Produces, for Task 3:

```rust
pub(super) struct GraphGeometry {
    /// The box field, left of the panel.
    pub pane: Rect,
    /// The detail panel down the right.
    pub panel: Rect,
    pub cell_w: f32,
    pub cell_h: f32,
}

impl GraphGeometry {
    /// The box for a cell at `(tier, slot)`, in screen pixels.
    pub fn cell_rect(&self, tier: usize, slot: usize) -> Rect;
    /// How many characters of a label fit one line of a box at `small`.
    pub fn label_columns(&self, painter: &Painter, m: &Metrics) -> usize;
}

pub(super) fn geometry(
    screen_w: f32,
    screen_h: f32,
    graph: &ResearchGraph,
    m: &Metrics,
) -> GraphGeometry;

/// A box's label: the name wrapped onto at most `LABEL_LINES` lines at
/// `label_columns`, each line elided with `…` if it still overruns.
pub(super) fn box_label(name: &str, columns: usize) -> Vec<String>;
```

Constants, all `const` in this file with a sentence each:

```rust
/// Fraction of the window width the detail panel takes.
const PANEL_FRACTION: f32 = 0.30;
/// Horizontal room between two tiers' boxes — where the edges are drawn,
/// so it is an elbow's whole budget and not decoration.
const GUTTER_X: f32 = 24.0;
const GUTTER_Y: f32 = 16.0;
/// A name wraps onto at most two lines. A third would not fit `cell_h` at
/// nine slots, and eliding is what a modded name gets instead.
const LABEL_LINES: usize = 2;
```

- [ ] **Step 1: Write the failing tests**

Create `crates/gui/src/render/research_graph.rs` with the module doc, the
constants above, and this test module. Nothing else yet.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use super::super::test_support::test_assets_dir;
    use crate::text::ui_metrics;
    use feral_processes_engine::{DifficultyMode, Game, ResearchCell, ResearchGraph};

    fn shipped_graph() -> ResearchGraph {
        Game::new(930, DifficultyMode::Forgiving, &test_assets_dir())
            .expect("the shipped asset tree builds a fresh game")
            .research_graph()
    }

    /// The whole tree is visible at once — there is no pan and no scroll —
    /// so "fits" is a correctness property of this screen and not a polish
    /// one. 1280x720 is the smallest window the rest of the UI is held to.
    #[test]
    fn the_whole_shipped_tree_fits_the_graph_pane_at_1280x720() {
        let g = shipped_graph();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        assert!(geo.cell_w > 0.0 && geo.cell_h > 0.0);
        for cell in &g.cells {
            let r = geo.cell_rect(cell.tier, cell.slot);
            assert!(
                r.x >= geo.pane.x && r.x + r.w <= geo.pane.x + geo.pane.w + 0.01,
                "{} at tier {} runs outside the pane horizontally",
                cell.id,
                cell.tier
            );
            assert!(
                r.y >= geo.pane.y && r.y + r.h <= geo.pane.y + geo.pane.h + 0.01,
                "{} at slot {} runs outside the pane vertically",
                cell.id,
                cell.slot
            );
        }
        assert!(
            geo.pane.x + geo.pane.w <= geo.panel.x + 0.01,
            "the boxes must not run under the detail panel"
        );
        assert!(
            geo.panel.x + geo.panel.w <= 1280.0 + 0.01,
            "and the panel must not run off the window"
        );
    }

    /// A box has to hold the longest *unbreakable* word in the shipped tree
    /// on one line, because `text::wrap` splits on whitespace only — a
    /// hyphen is not a break opportunity, so `Self-Execution` (14) and not
    /// `Fabrication` (11) is the constraint. Measured through
    /// `with_painter`, which lays out the real font.
    #[test]
    fn the_longest_shipped_word_fits_a_box_at_1280x720() {
        let g = shipped_graph();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        with_painter(|p| {
            let columns = geo.label_columns(p, &m);
            assert!(
                columns >= 14,
                "a box holds {columns} characters at m.small(); \
                 `Self-Execution` needs 14. Widen the pane (PANEL_FRACTION) \
                 or narrow the gutter (GUTTER_X) — do not lower this number"
            );
        });
    }

    /// No *shipped* name may be elided. Elision exists for a modded tree,
    /// and a shipped name losing its tail would be a content bug read as a
    /// rendering one.
    #[test]
    fn no_shipped_node_name_is_elided_at_1280x720() {
        let game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let g = game.research_graph();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        let nodes = game.research_nodes();
        with_painter(|p| {
            let columns = geo.label_columns(p, &m);
            for node in &nodes {
                let lines = box_label(&node.name, columns);
                assert!(
                    lines.len() <= LABEL_LINES,
                    "{:?} wraps onto {} lines",
                    node.name,
                    lines.len()
                );
                assert!(
                    !lines.iter().any(|l| l.ends_with('…')),
                    "{:?} was elided into {lines:?} at {columns} columns",
                    node.name
                );
                assert_eq!(
                    lines.join(" "),
                    node.name,
                    "wrapping must preserve the name exactly"
                );
            }
        });
    }

    /// A modded name longer than a box gets a tail it can read as cut,
    /// rather than silently overrunning its neighbour.
    #[test]
    fn a_name_too_long_for_a_box_is_elided_rather_than_overrunning() {
        let lines = box_label("Supercalifragilisticexpialidocious Subsystem", 12);
        assert_eq!(lines.len(), LABEL_LINES);
        assert!(
            lines[0].ends_with('…'),
            "an unbreakable word past the budget is cut: {lines:?}"
        );
        assert!(
            lines.iter().all(|l| l.chars().count() <= 12),
            "no line may run past the budget: {lines:?}"
        );
    }

    /// A modded tree deeper and wider than the shipped one squeezes; it does
    /// not scroll and it does not produce a negative box. This is the
    /// deliberate YAGNI the spec records — panning is the feature to add if
    /// a mod ever needs it.
    #[test]
    fn a_far_larger_tree_squeezes_rather_than_overflowing() {
        let mut g = ResearchGraph::default();
        for tier in 0..20 {
            for slot in 0..30 {
                g.cells.push(ResearchCell {
                    id: format!("n{tier}_{slot}"),
                    tier,
                    slot,
                });
            }
        }
        g.tiers = 20;
        g.widest = 30;
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        assert!(geo.cell_w > 0.0, "a squeezed box is still a box");
        assert!(geo.cell_h > 0.0);
        for cell in &g.cells {
            let r = geo.cell_rect(cell.tier, cell.slot);
            assert!(r.x + r.w <= geo.pane.x + geo.pane.w + 0.01);
            assert!(r.y + r.h <= geo.pane.y + geo.pane.h + 0.01);
        }
    }

    /// An empty tree is a supported install — `assets/research/` deleted is
    /// the pre-research game — so the geometry must not divide by zero.
    #[test]
    fn an_empty_tree_produces_no_boxes_and_no_nan() {
        let g = ResearchGraph::default();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        assert!(geo.pane.w.is_finite() && geo.panel.w.is_finite());
        assert!(geo.cell_w.is_finite() && geo.cell_h.is_finite());
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p feral-processes-gui research_graph
```

Expected: FAIL to compile — `geometry`, `GraphGeometry` and `box_label` do
not exist. Add `mod research_graph;` to `render/mod.rs` first if the module
is not being compiled at all.

- [ ] **Step 3: Implement the geometry**

Signatures are in the **Interfaces** block; write them verbatim. The three
things that are not obvious from them:

- `pane` and `panel` split the window horizontally at
  `screen_w * (1.0 - PANEL_FRACTION)`, with a header band of one
  `m.line_height` at the top for the title and held Research Data, and a
  footer band of one for the keybind hint. The panel takes the full height
  between them.
- Pitch is `pane.w / tiers` and `pane.h / widest`; `cell_w` is
  `pitch_x - GUTTER_X` and `cell_h` is `pitch_y - GUTTER_Y`, each
  `.max(1.0)` so a squeezed tree yields a thin box rather than a negative
  one. **Guard `tiers == 0` and `widest == 0` before dividing** — an empty
  `assets/research/` is a supported install and a NaN rect propagates into
  egui silently.
- `label_columns` measures one character of the real font
  (`painter.measure_ui_advance("M", m.small())`) and divides the box's inner
  width by it. Measuring rather than assuming an advance ratio is the point;
  this repo has a recorded bug from a row width computed instead of measured.

`box_label` calls `popup::wrap_text` (which is `engine::text::wrap`) — not a
second wrapper — takes the first `LABEL_LINES` lines, and elides any line
still past `columns` to `columns - 1` characters plus `…`. A word longer than
`columns` comes back from `wrap` on a line of its own, over budget, which is
exactly the case the elision exists for.

- [ ] **Step 4: Run the tests green**

```bash
cargo test -p feral-processes-gui research_graph
```

Expected: PASS, all six. If `the_longest_shipped_word_fits_a_box_at_1280x720`
fails, lower `GUTTER_X` or `PANEL_FRACTION` and re-run — the assertion is the
requirement and does not move.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test -p feral-processes-gui
git add crates/gui/src/render/research_graph.rs crates/gui/src/render/mod.rs
git commit -m "feat(research): the graph screen's geometry and its fit

Six tiers by nine slots at 1280x720, with the box label at
Metrics::small() — text::wrap splits on whitespace only, so
Self-Execution (14 chars) and not Fabrication (11) is the constraint,
and it does not fit the body font six across.

The fit is a correctness property here: the whole tree is visible at
once, with no pan and no scroll, so the tests measure real text.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: The draw — boxes, edges, panel, dispatch

**Files:**
- Modify: `crates/gui/src/render/research_graph.rs` — the drawing half
- Modify: `crates/gui/src/render/mod.rs` — the `Mode::Research` guard arm
  (line 1348) and the flag read in `draw_mode_overlay` (near line 832)
- Modify: `crates/gui/src/render/progression.rs` — `material_rows` and
  `conversion_rows` gain a `columns: usize` parameter and go `pub(super)`;
  `row_color` and `LOCKED_BY_PREREQ` go `pub(super)`
- Modify: `crates/gui/src/render/popup.rs` — add `description_rows_at`

**Interfaces:**
- Consumes: everything from Tasks 1 and 2, plus
  `Game::{research_graph, research_nodes, research_currency, banked}`,
  `popup::{draw_row, Row, wrap_text}`, `Painter::{rect, rect_lines, line, ui}`.
- Produces:

```rust
pub(super) fn draw_research_graph(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
);
```

`draw_research_menu`'s signature exactly, so the `Mode::Research` arm is one
guard and two calls that differ in nothing else.

- [ ] **Step 1: Widen the three row builders**

Do this first and on its own — it is a mechanical change with a compiler
gate, and mixing it into the drawing makes the drawing's diff unreadable.

- `popup.rs`: add
  `pub(super) fn description_rows_at(description: &str, columns: usize) -> impl Iterator<Item = Row> + '_`,
  holding the current body, and make `description_rows` a **call** into it
  with `DESCRIBE_WRAP_COLUMNS - DESCRIPTION_INDENT.chars().count()`. Six
  files call `description_rows`; none of them changes.
- `progression.rs`: `material_rows` and `conversion_rows` each take
  `columns: usize` and use it where they currently compute that same
  expression inline. Both go `pub(super)`. `research_menu_rows` — their only
  existing caller — passes the expression it used to compute.
- `progression.rs`: `row_color` goes `pub(super)`. The graph's box colour is
  a call to it, so cyan, green, amber, blue and dim mean on the boxes exactly
  what they mean on the rows. `LOCKED_BY_PREREQ` (line 119) goes `pub(super)`
  too, so Task 3's outline test can name the colour it is asserting on.

```bash
cargo test -p feral-processes-gui
```

Expected: PASS with nothing changed in behaviour. Commit this step alone:

```bash
git add crates/gui/src/render/popup.rs crates/gui/src/render/progression.rs
git commit -m "refactor(render): let a research row builder take its column budget

The graph view's detail panel is narrower than a Large popup body, and
it has to describe a node with the same functions the list does — a
second wording is the copy that drifts.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 2: Write the failing draw tests**

Append to `research_graph.rs`'s `mod tests`:

```rust
    use crate::paint::{painted_rect_stroke_count, painted_text};
    use feral_processes_engine::ResearchState;

    fn drawn(selected: usize) -> Vec<String> {
        let mut game =
            Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_research_graph(&mut game, selected, None, p, &m));
        painted_text(&shapes)
    }

    /// Every node in the tree is drawn, because the whole point of this view
    /// is that the shape is visible at once. Asserted on a name from each of
    /// the six tiers rather than all 34, so a failure names where it broke.
    #[test]
    fn every_tier_is_drawn() {
        let text = drawn(0).join("\n");
        for name in [
            "Automation",
            "Cache Coherence",
            "Segmentation",
            "Overclock",
            "Cortex",
            "Mesh Plating",
        ] {
            assert!(text.contains(name), "{name} is not on the screen");
        }
    }

    /// The panel describes the node under the highlight with the *same*
    /// derivations the list uses, so the two views cannot word a node
    /// differently. Driven off `research_nodes()` rather than a hardcoded
    /// node, because that vec re-sorts by state.
    #[test]
    fn the_panel_draws_the_selected_nodes_materials_and_conversions() {
        let mut game =
            Game::new(933, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nodes = game.research_nodes();
        let picked = nodes
            .iter()
            .position(|n| !n.materials.is_empty() && !n.conversions.is_empty())
            .expect("some shipped node has both a bill and a conversion");
        let node = &nodes[picked];
        let want_material = node.materials[0].name.clone();
        let want_conversion = node.conversions[0].clone();
        let want_name = node.name.clone();
        let m = ui_metrics(720.0);
        let (_, shapes) =
            with_painter(|p| draw_research_graph(&mut game, picked, None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains(&want_name), "the panel names the node");
        assert!(
            text.contains(&want_material),
            "the panel draws the bill: {want_material:?}"
        );
        assert!(
            text.contains(want_conversion.split(" into ").next().unwrap()),
            "the panel draws the conversion: {want_conversion:?}"
        );
    }

    /// A box's colour is `progression::row_color`, so the five states read on
    /// the graph exactly as they read in the list. Asserted through the
    /// outline count: the shipped tree at a fresh run has locked nodes, so
    /// the amber outline has to appear.
    #[test]
    fn a_boxs_outline_takes_the_lists_row_colour() {
        let mut game =
            Game::new(934, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nodes = game.research_nodes();
        let locked = nodes
            .iter()
            .filter(|n| matches!(n.state, ResearchState::Locked { min_zone: None, .. }))
            .count();
        assert!(locked > 0, "a fresh run has prereq-locked nodes to draw");
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_research_graph(&mut game, 0, None, p, &m));
        assert_eq!(
            painted_rect_stroke_count(&shapes, super::super::progression::LOCKED_BY_PREREQ),
            locked,
            "one amber outline per prereq-locked node"
        );
    }

    /// A refusal raised on this screen has to be drawn on it. The graph
    /// draws no popup, so it carries the line itself.
    #[test]
    fn a_refusal_is_drawn_on_the_graph() {
        let mut game =
            Game::new(935, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| {
            draw_research_graph(&mut game, 0, Some("Requires Zone 3 first."), p, &m)
        });
        assert!(
            painted_text(&shapes)
                .join("\n")
                .contains("Requires Zone 3 first."),
            "a refusal must reach the screen the player typed into"
        );
    }
```

`a_boxs_outline_takes_the_lists_row_colour` names
`progression::LOCKED_BY_PREREQ`, which is a bare `const` private to that file
today (`progression.rs:119`). Make it `pub(super) const` in Step 1's
mechanical pass rather than adding an accessor — the other colour constants
in `render/mod.rs` are shared exactly that way.

- [ ] **Step 3: Run them and watch them fail**

```bash
cargo test -p feral-processes-gui research_graph
```

Expected: FAIL to compile — `draw_research_graph` does not exist.

- [ ] **Step 4: Draw it**

`draw_research_graph` in five short helpers, in this order. Take the
geometry once and pass it down; nothing recomputes it.

1. **Header** — the screen title, `Research Data: {held}` (off
   `game.banked(&game.research_currency())`, the same figure
   `draw_research_menu` puts on its header), and the keybind hint naming
   `G`, the arrows, `Enter` and `Esc`. `research_graph_view`'s existence is
   only discoverable from the list's own hint, so **add `G` to
   `research_menu_rows`' instruction line in the same change** or the
   feature ships unreachable.
2. **Edges, before the boxes**, so a box paints over a line rather than the
   other way round. For each `(from, to)` in `graph.edges`, an orthogonal
   elbow through the gutter: out of `from`'s right edge to the middle of the
   gutter, vertically to `to`'s centre line, then into `to`'s left edge —
   three `Painter::line` calls. Dim by default; **bright for an edge whose
   `to` is the selected node**, which is the whole of what makes the one
   tier-skipping edge readable. There is no edge router; the spec's decision,
   and it is a lot of code for one shipped edge.
3. **Boxes** — `Painter::rect` for the fill, `rect_lines` for the outline in
   `progression::row_color(node)`, and the label lines from `box_label`
   drawn at `m.small()`. The cost goes on the box's last line. The selected
   box takes `SELECT_BG` behind it, the same fill the list's selected row
   takes.
4. **Panel** — the selected node's name, then `material_rows`,
   `description_rows_at` and `conversion_rows` at the panel's own column
   budget (measured the way `label_columns` measures, at `m.font_size`),
   each drawn with `popup::draw_row(row, geo.panel.x, geo.panel.w, cy,
   max_y, painter, m)`. `draw_row` takes an explicit x/w/cy, so the panel
   needs no popup around it.
5. **Refusal** — the footer band, in `RED`, when `refusal` is `Some`.

Resolve the selected node once, at the top:
`let nodes = game.research_nodes(); let node = nodes.get(selected.min(...))`.
An empty tree draws the header and nothing else — no panic, no divide.

- [ ] **Step 5: Dispatch it**

`render/mod.rs`: read the flag alongside `let selected = app.menu_selected;`
in `draw_mode_overlay` (line 832) — before `app.game` is borrowed, which is
what every other value there is doing and why. Then the arm at line 1348
becomes two:

```rust
        Mode::Research if graph_view => {
            research_graph::draw_research_graph(game, selected, refusal, painter, m)
        }
        Mode::Research => draw_research_menu(game, selected, refusal, painter, m),
```

A guard on the arm rather than a branch ahead of the match: it keeps the
research screen's whole dispatch in one place, and a guard arm cannot be
reordered into unreachability the way a pre-match `if` can be.

- [ ] **Step 6: Run the tests green**

```bash
cargo test -p feral-processes-gui research_graph
cargo test -p feral-processes-gui
```

Expected: PASS. The second run matters — `render/mod.rs`'s own census tests
walk every `Mode`, and this task changes what `Mode::Research` draws.

- [ ] **Step 7: Full gate and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/gui/src/render/research_graph.rs crates/gui/src/render/mod.rs \
        crates/gui/src/render/progression.rs
git commit -m "feat(research): draw the tree as a flow chart

Tiers as columns left to right, orthogonal elbows in the gutters,
bright for the selected node's own prerequisites. The detail panel is
built from the list's own material, description and conversion row
builders, and a box takes the list's row_color, so the two views
cannot describe or colour a node differently.

Drawn instead of the research popup from a bool on App — no new Mode
and no census moves.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Documentation and the release

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` (workspace version) — **at the merge, not on the
  branch**
- Modify: `CLAUDE.md` — one sentence in "Load-bearing seams"
- Modify: `.claude/skills/seams/` — the trap behind that sentence
- Memory graph — the argument behind the seam

**Interfaces:** none; this task ships no code.

- [ ] **Step 1: Write the seam's three parts**

A new seam is three writes, and the `seams` skill documents the order.
Invoke it (`Skill: seams`) rather than working from this list.

- **The argument** → the memory graph as `seam:research-graph-layout`: that
  the layout is derived in the engine because two rules for "what is next to
  this node" — app-core's cursor and gui's boxes — would drift, which is
  `balance_sim.rs`'s recurring failure in a new place; and that it is keyed
  by `ResearchId` because `research_nodes()` re-sorts by `ResearchState`.
- **The trap** → the `seams` skill's reference file: that an index-parallel
  structure would be correct on the day it shipped and wrong the first time
  anyone filtered the list, and that `step` returning `Option` would put a
  branch in app-core that is the first half of a second cursor.
- **The rule** → `CLAUDE.md`, one sentence under a heading of its own. One
  sentence is a budget, not a style:

  > - **The research tree's flow-chart layout is derived once, in
  >   `Game::research_graph`, and keyed by `ResearchId`** — `views::
  >   ResearchGraph::step` is the one rule for what an arrow key does, and
  >   app-core computes no neighbours.

- [ ] **Step 2: Changelog**

A `## X.Y.Z` section. Which digit moves is `CHANGELOG.md`'s preamble's call;
this ships no save-format change (`SAVE_FORMAT_VERSION` does not move), so it
is not breaking.

- [ ] **Step 3: Grep for claims this falsifies**

```bash
rg -n "research" docs/ README.md CHANGELOG.md assets/research/README.md | rg -i "list|menu|pick a row"
```

Anything describing the research screen as a list only is now half-true. Fix
what this change falsified; `docs/manual.md` and the root `README.md` are
carved out of that obligation and stay stale.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md CLAUDE.md .claude/skills/seams/
git commit -m "docs: record the research graph's layout seam

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 5: Hand the play question over**

Nothing in this plan has been seen on a screen — agents here have no display
and `cargo run` refuses. A green suite is not evidence of play. Say so once,
plainly, and name the three things only a person at the keyboard can answer:
whether the elbows are readable where they cross, whether `m.small()` is
actually legible at 720p, and whether the arrow-key walk feels like moving
around a tree rather than around a grid.

The merge, the version bump and the tag are the `deploy` skill's, and it
needs an explicit ask.

---

## Self-review against the spec

| Spec section | Covered by |
|---|---|
| Both views, one key toggles | Phase 2 Task 1 (`G`), Task 3 Step 5 (dispatch) |
| Whole tree visible, no panning | Phase 2 Task 2 (`a_far_larger_tree_squeezes_rather_than_overflowing`) |
| Engine derives the layout | Phase 1 Tasks 2-3 |
| Tiers are columns, left to right | Phase 2 Task 2 (`geometry`), Task 3 Step 4 |
| No new `Mode` | Phase 2 Task 1 (the `bool`), Global Constraints |
| The tier census | Phase 1 Task 2 (`the_shipped_tree_has_the_shape_the_screen_is_sized_for`) |
| A box fits the longest word | Phase 2 Task 2 — **corrected to 14 characters** |
| `ResearchGraph` fields | Phase 1 Task 2 Interfaces |
| Keyed by `ResearchId` | Phase 1 Global Constraints; Phase 2 Task 1 |
| Tier is the longest path | Phase 1 Task 2 (`every_edge_points_strictly_rightward`) |
| Slot by first parent then id | Phase 1 Task 2 Step 4 |
| A cycle is dropped, not a stack overflow | Phase 1 Task 1 |
| `step` is the one arrow rule, total | Phase 1 Task 3; Phase 2 Task 1 |
| Panel from the list's own builders | Phase 2 Task 3 Steps 1-2 |
| Box colour is `row_color` | Phase 2 Task 3 Step 1 and its test |
| Orthogonal elbows, no router | Phase 2 Task 3 Step 4 |
| Fit-to-window pitch, no scroll state | Phase 2 Task 2 |
| Uppercase toggle | Phase 2 Task 1 (`g_toggles_the_graph_view_and_back`) |
| One selection, converted at the boundary | Phase 2 Task 1 |
| `Enter` is the list's door; `Esc` closes | Phase 2 Task 1 |

## Out of scope

- Panning, zooming and edge routing.
- Any change to what a node costs, unlocks or requires.
- The perk and talent screens, which are ladders rather than graphs.
