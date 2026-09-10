//! The research tree drawn as a flow chart — the second view of
//! `Mode::Research`, toggled with `G`.
//!
//! The whole tree is visible at once: there is no pan and no scroll, so
//! "fits" is a correctness property of this screen rather than a polish
//! one, and the geometry is a pure function of the window and the tree's
//! shape so it can be held to a measured assertion.

use crate::paint::{Color, Painter, Rect};
use crate::text::Metrics;
use feral_processes_engine::{Game, ResearchGraph};

use super::popup::{DESCRIPTION_INDENT, description_rows_at, draw_row};
use super::progression::{conversion_rows, material_rows, row_color};
use super::{RED, SELECT_BG, TEXT_DIM};

/// Fraction of the window width the detail panel takes.
const PANEL_FRACTION: f32 = 0.30;
/// Horizontal room between two tiers' boxes — where the edges are drawn, so
/// it is an elbow's whole budget and not decoration.
const GUTTER_X: f32 = 16.0;
const GUTTER_Y: f32 = 16.0;
/// A name wraps onto at most two lines. A third would not fit `cell_h` at
/// nine slots, and eliding is what a modded name gets instead.
const LABEL_LINES: usize = 2;

pub(super) struct GraphGeometry {
    /// The box field, left of the panel.
    pub pane: Rect,
    /// The detail panel down the right.
    pub panel: Rect,
    pub cell_w: f32,
    pub cell_h: f32,
    pitch_x: f32,
    pitch_y: f32,
}

impl GraphGeometry {
    /// The box for a cell at `(tier, slot)`, in screen pixels.
    pub fn cell_rect(&self, tier: usize, slot: usize) -> Rect {
        Rect::new(
            self.pane.x + tier as f32 * self.pitch_x,
            self.pane.y + slot as f32 * self.pitch_y,
            self.cell_w,
            self.cell_h,
        )
    }

    /// How many characters of a label fit one line of a box at `small`.
    /// Measured through the real font rather than assumed off an advance
    /// ratio — a row width computed instead of measured has bitten this
    /// repo before.
    pub fn label_columns(&self, painter: &Painter, m: &Metrics) -> usize {
        columns_for(painter, self.cell_w - m.pad * 2.0, m.small())
    }
}

/// How many characters of `size` text fit `width` pixels.
pub(super) fn columns_for(painter: &Painter, width: f32, size: u16) -> usize {
    let advance = painter.measure_ui_advance("M", size);
    if advance <= 0.0 || width <= 0.0 {
        return 0;
    }
    (width / advance).floor() as usize
}

pub(super) fn geometry(
    screen_w: f32,
    screen_h: f32,
    graph: &ResearchGraph,
    m: &Metrics,
) -> GraphGeometry {
    let header = m.line_height + m.pad;
    let footer = m.line_height + m.pad;
    let body_y = header;
    let body_h = (screen_h - header - footer).max(1.0);
    let split = screen_w * (1.0 - PANEL_FRACTION);
    let pane = Rect::new(m.pad, body_y, (split - m.pad * 2.0).max(1.0), body_h);
    let panel = Rect::new(split, body_y, (screen_w - split - m.pad).max(1.0), body_h);
    // An empty `assets/research/` is a supported install, and a NaN rect
    // propagates into egui silently.
    let pitch_x = if graph.tiers == 0 {
        pane.w
    } else {
        pane.w / graph.tiers as f32
    };
    let pitch_y = if graph.widest == 0 {
        pane.h
    } else {
        pane.h / graph.widest as f32
    };
    GraphGeometry {
        pane,
        panel,
        cell_w: (pitch_x - GUTTER_X).max(1.0),
        cell_h: (pitch_y - GUTTER_Y).max(1.0),
        pitch_x,
        pitch_y,
    }
}

/// A box's label: the name wrapped onto at most `LABEL_LINES` lines at
/// `columns`, each line elided with `…` if it still overruns. A word longer
/// than `columns` comes back from `wrap` on a line of its own, over budget,
/// which is the case the elision exists for.
pub(super) fn box_label(name: &str, columns: usize) -> Vec<String> {
    super::popup::wrap_text(name, columns)
        .into_iter()
        .take(LABEL_LINES)
        .map(|line| {
            if line.chars().count() <= columns {
                line
            } else {
                let keep = columns.saturating_sub(1);
                let mut out: String = line.chars().take(keep).collect();
                out.push('…');
                out
            }
        })
        .collect()
}

/// An edge that does not end at the selected node.
const EDGE_DIM: Color = Color::new(0.35, 0.35, 0.42, 1.0);
/// An edge into the selected node — the whole of what makes the one
/// tier-skipping edge readable, since there is no edge router.
const EDGE_LIVE: Color = Color::new(0.25, 0.85, 0.85, 1.0);

/// The research tree as a flow chart. `draw_research_menu`'s signature
/// exactly, so the `Mode::Research` arm is one guard and two calls that
/// differ in nothing else.
pub(super) fn draw_research_graph(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (screen_w, screen_h) = (painter.screen_w(), painter.screen_h());
    let research_currency = game.research_currency();
    let held = game.banked(&research_currency);
    let graph = game.research_graph();
    let nodes = game.research_nodes();
    let geo = geometry(screen_w, screen_h, &graph, m);

    painter.ui(
        format!("Research Data: {held}"),
        m.pad,
        m.line_height,
        m.font_size,
        Color::new(0.25, 0.85, 0.85, 1.0),
    );
    painter.ui(
        "G list  arrows move  Enter research  Esc close",
        geo.panel.x,
        m.line_height,
        m.small(),
        TEXT_DIM,
    );

    // An empty `assets/research/` is a supported install: the header is
    // drawn and nothing else.
    let Some(node) = nodes.get(selected.min(nodes.len().saturating_sub(1))) else {
        return;
    };
    let selected_id = node.id.clone();

    // Edges first, so a box paints over a line rather than the other way
    // round.
    for (from, to) in &graph.edges {
        let (Some(a), Some(b)) = (graph.cell(from), graph.cell(to)) else {
            continue;
        };
        let ra = geo.cell_rect(a.tier, a.slot);
        let rb = geo.cell_rect(b.tier, b.slot);
        let live = *to == selected_id;
        let color = if live { EDGE_LIVE } else { EDGE_DIM };
        let thickness = if live { 2.0 } else { 1.0 };
        let ay = ra.y + ra.h * 0.5;
        let by = rb.y + rb.h * 0.5;
        let mid = ra.x + ra.w + (rb.x - (ra.x + ra.w)) * 0.5;
        painter.line(ra.x + ra.w, ay, mid, ay, thickness, color);
        painter.line(mid, ay, mid, by, thickness, color);
        painter.line(mid, by, rb.x, by, thickness, color);
    }

    let columns = geo.label_columns(painter, m);
    for cell in &graph.cells {
        let Some(node) = nodes.iter().find(|n| n.id == cell.id) else {
            continue;
        };
        let r = geo.cell_rect(cell.tier, cell.slot);
        if node.id == selected_id {
            painter.rect(r.x, r.y, r.w, r.h, SELECT_BG);
        }
        painter.rect_lines(r.x, r.y, r.w, r.h, 1.0, row_color(node));
        let mut y = r.y + m.line_height;
        for line in box_label(&node.name, columns) {
            painter.ui(line, r.x + m.pad, y, m.small(), row_color(node));
            y += m.line_height;
        }
        painter.ui(
            format!("{}", node.cost),
            r.x + m.pad,
            (r.y + r.h - m.pad).max(y),
            m.small(),
            TEXT_DIM,
        );
    }

    // The panel, out of the list's own row builders — a second wording of a
    // node's bill or conversions is the copy that drifts.
    let panel_columns = columns_for(painter, geo.panel.w - m.pad * 2.0, m.font_size)
        .saturating_sub(DESCRIPTION_INDENT.chars().count());
    let max_y = geo.panel.y + geo.panel.h;
    let mut cy = geo.panel.y + m.line_height;
    cy = draw_row(
        &super::popup::colored_item_row(node.name.clone(), false, row_color(node)),
        geo.panel.x,
        geo.panel.w,
        cy,
        max_y,
        painter,
        m,
    );
    cy = draw_row(
        &super::popup::text_row(format!("{} Research Data", node.cost)),
        geo.panel.x,
        geo.panel.w,
        cy,
        max_y,
        painter,
        m,
    );
    for row in material_rows(&node.materials, panel_columns)
        .iter()
        .chain(
            description_rows_at(&node.description, panel_columns)
                .collect::<Vec<_>>()
                .iter(),
        )
        .chain(conversion_rows(&node.conversions, panel_columns).iter())
    {
        cy = draw_row(row, geo.panel.x, geo.panel.w, cy, max_y, painter, m);
    }

    if let Some(refusal) = refusal {
        painter.ui(refusal, m.pad, screen_h - m.pad, m.font_size, RED);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::test_assets_dir;
    use super::*;
    use crate::paint::with_painter;
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
    /// `Fabrication` (11) is the constraint.
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
    /// not scroll and it does not produce a negative box. Panning is the
    /// feature to add if a mod ever needs it.
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

    use crate::paint::{painted_rect_stroke_count, painted_text};
    use feral_processes_engine::ResearchState;

    /// Every node in the tree is drawn, because the whole point of this view
    /// is that the shape is visible at once. Asserted on a name from each of
    /// the six tiers rather than all 34, so a failure names where it broke.
    #[test]
    fn every_tier_is_drawn() {
        let mut game = Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_research_graph(&mut game, 0, None, p, &m));
        let text = painted_text(&shapes).join("\n");
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
        let mut game = Game::new(933, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
        let (_, shapes) = with_painter(|p| draw_research_graph(&mut game, picked, None, p, &m));
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
    /// the graph exactly as they read in the list.
    #[test]
    fn a_boxs_outline_takes_the_lists_row_colour() {
        let mut game = Game::new(934, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
        let mut game = Game::new(935, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
}
