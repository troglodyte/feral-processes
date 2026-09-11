//! The research tree drawn as a flow chart — the second view of
//! `Mode::Research`, toggled with `G`.
//!
//! **The tree is larger than the pane and the pane pans.** Boxes are a fixed
//! readable size and the viewport follows the cursor, rather than the tree
//! being divided into the pane and every box shrinking as a mod adds nodes.
//! That trades one correctness property for another: "the whole tree fits at
//! 1280x720" is gone, and "the selected node is always fully in view" is what
//! replaces it. The geometry is still a pure function of the window, the
//! tree's shape and which node is selected, so both halves stay measurable.
//!
//! The viewport is **derived from the selection, never stored** — there is no
//! scroll offset to keep in step with the cursor, and no key of its own. The
//! cost is that the view recentres on every move instead of sitting still
//! until the selection nears an edge; the benefit is that a cursor and a
//! viewport cannot disagree about where the tree is.

use crate::paint::{Color, Painter, Rect};
use crate::text::Metrics;
use feral_processes_engine::{Game, ResearchGraph, ResearchId};

use super::popup::{DESCRIPTION_INDENT, description_rows_at, draw_row};
use super::progression::{conversion_rows, material_rows, row_color};
use super::{BORDER, PANEL_BG, RED, SELECT_BG, TEXT_DIM};

/// Fraction of the window width the detail panel takes.
const PANEL_FRACTION: f32 = 0.30;
/// A box's height in `line_height`s: two label lines and its cost.
const CELL_LINES: f32 = 3.0;
/// A box's width in UI font sizes. 11 puts about twenty monospace cells of
/// `small()` text inside the padding, against the fourteen `Self-Execution`
/// needs — the slack is what a modded name spends before it is elided.
const CELL_FONTS: f32 = 11.0;
/// A name wraps onto at most two lines. A third would not fit `cell_h`, and
/// eliding is what a modded name gets instead.
const LABEL_LINES: usize = 2;

/// One edge's route across a gutter: which lane it takes, and how many
/// lanes that gutter is carrying.
///
/// **Every edge makes its vertical run in the gutter immediately right of
/// its source**, even the one shipped edge that spans two tiers — that one's
/// last horizontal run passes under the intervening tier's boxes, which is
/// what it did when there was one lane and is why boxes are drawn last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EdgeRoute {
    pub from: ResearchId,
    pub to: ResearchId,
    /// The gutter right of tier `gutter`.
    pub gutter: usize,
    pub lane: usize,
}

/// How many edges make their vertical run in the gutter right of each tier.
///
/// This is what sizes a gutter, so the crowded one gets room and the sparse
/// one does not waste it: the shipped tree runs 8, 5, 9, 6 and 3 edges
/// through its five gutters.
fn lane_counts(graph: &ResearchGraph) -> Vec<usize> {
    let mut counts = vec![0usize; graph.tiers.saturating_sub(1)];
    for (from, to) in &graph.edges {
        let (Some(a), Some(b)) = (graph.cell(from), graph.cell(to)) else {
            continue;
        };
        if b.tier > a.tier && a.tier < counts.len() {
            counts[a.tier] += 1;
        }
    }
    counts
}

/// Every edge with the lane it takes, in a deterministic order.
///
/// Sorted by the slots it joins, so a parent's fan stays bundled and two
/// edges never swap lanes between frames. Ids break the tie, because two
/// edges can join the same pair of slots across a tier-skipping gap.
pub(super) fn assign_lanes(graph: &ResearchGraph) -> Vec<EdgeRoute> {
    let mut routed: Vec<(usize, usize, usize, &ResearchId, &ResearchId)> = graph
        .edges
        .iter()
        .filter_map(|(from, to)| {
            let (a, b) = (graph.cell(from)?, graph.cell(to)?);
            (b.tier > a.tier).then_some((a.tier, a.slot, b.slot, from, to))
        })
        .collect();
    routed.sort_by(|l, r| (l.0, l.1, l.2, l.3, l.4).cmp(&(r.0, r.1, r.2, r.3, r.4)));
    let mut lane_in: Vec<usize> = vec![0; graph.tiers];
    routed
        .into_iter()
        .map(|(gutter, _, _, from, to)| {
            let lane = lane_in[gutter];
            lane_in[gutter] += 1;
            EdgeRoute {
                from: from.clone(),
                to: to.clone(),
                gutter,
                lane,
            }
        })
        .collect()
}

pub(super) struct GraphGeometry {
    /// The viewport onto the box field, left of the panel. Screen space.
    pub pane: Rect,
    /// The detail panel down the right. Screen space.
    pub panel: Rect,
    pub cell_w: f32,
    pub cell_h: f32,
    /// Content-space left edge of each tier's column.
    tier_x: Vec<f32>,
    /// Content-space width of the gutter right of each tier.
    gutter_w: Vec<f32>,
    /// How many lanes each gutter carries — what `lane_x` divides by, so a
    /// lane's share of its gutter is the same figure that sized it.
    lanes: Vec<usize>,
    /// Content-space distance between one slot and the next.
    pitch_y: f32,
    content_w: f32,
    content_h: f32,
}

impl GraphGeometry {
    /// The box for a cell at `(tier, slot)`, in content space.
    pub fn content_rect(&self, tier: usize, slot: usize) -> Rect {
        let x = self.tier_x.get(tier).copied().unwrap_or(0.0);
        Rect::new(x, slot as f32 * self.pitch_y, self.cell_w, self.cell_h)
    }

    /// Where the viewport sits while `(tier, slot)` is selected: that cell
    /// centred, then clamped so the view never runs off the content. An
    /// axis whose content already fits reads 0 rather than a negative
    /// offset that would float the tree away from its own corner.
    pub fn offset(&self, tier: usize, slot: usize) -> (f32, f32) {
        let r = self.content_rect(tier, slot);
        let span = |centre: f32, view: f32, content: f32| {
            (centre - view * 0.5).clamp(0.0, (content - view).max(0.0))
        };
        (
            span(r.x + r.w * 0.5, self.pane.w, self.content_w),
            span(r.y + r.h * 0.5, self.pane.h, self.content_h),
        )
    }

    /// The box for a cell at `(tier, slot)`, in screen pixels at `offset`.
    pub fn cell_rect(&self, tier: usize, slot: usize, offset: (f32, f32)) -> Rect {
        let r = self.content_rect(tier, slot);
        Rect::new(
            self.pane.x + r.x - offset.0,
            self.pane.y + r.y - offset.1,
            r.w,
            r.h,
        )
    }

    /// Content-space x of one lane of the gutter right of `tier`. Lanes are
    /// spread across the gutter rather than pinned to its middle, which is
    /// the whole of what stops nine edges reading as one vertical bar.
    pub fn lane_x(&self, tier: usize, lane: usize) -> f32 {
        let left = self.tier_x.get(tier).copied().unwrap_or(0.0) + self.cell_w;
        let gutter = self.gutter_w.get(tier).copied().unwrap_or(0.0);
        let lanes = self.lanes.get(tier).copied().unwrap_or(0);
        // Evenly across the gutter, so neither the first nor the last lane
        // grazes the box it runs beside.
        left + gutter * (lane as f32 + 1.0) / (lanes as f32 + 1.0)
    }

    /// How many characters of a label fit one line of a box at `small`.
    /// Measured through the real font rather than assumed off an advance
    /// ratio — a row width computed instead of measured has bitten this
    /// repo before.
    pub fn label_columns(&self, painter: &Painter, m: &Metrics) -> usize {
        columns_for(painter, self.cell_w - m.pad * 2.0, m.small())
    }

    /// What the viewport is not showing, in whole columns and rows, as
    /// `(left, right, above, below)`.
    pub fn hidden(&self, offset: (f32, f32)) -> (usize, usize, usize, usize) {
        let cols = |x: f32| (x / self.pitch_x_at()).floor().max(0.0) as usize;
        let rows = |y: f32| (y / self.pitch_y).floor().max(0.0) as usize;
        (
            cols(offset.0),
            cols((self.content_w - self.pane.w - offset.0).max(0.0)),
            rows(offset.1),
            rows((self.content_h - self.pane.h - offset.1).max(0.0)),
        )
    }

    /// The mean column pitch — gutters vary by lane count, so there is no
    /// single one, and this is only ever used to say "about this many
    /// columns are off to the left".
    fn pitch_x_at(&self) -> f32 {
        let tiers = self.tier_x.len().max(1) as f32;
        (self.content_w / tiers).max(1.0)
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

    // A box is sized off the font, not off the pane: a mod that doubles the
    // tree's width scrolls further, and every box stays as legible as the
    // shipped tree's.
    let cell_w = m.font_size as f32 * CELL_FONTS;
    let cell_h = m.line_height * CELL_LINES + m.pad;
    let lane_pitch = m.inset;

    // A gutter is a base plus a lane per edge crossing it.
    let counts = lane_counts(graph);
    let gutter_w: Vec<f32> = counts
        .iter()
        .map(|lanes| m.pad * 2.0 + *lanes as f32 * lane_pitch)
        .collect();
    let mut tier_x = Vec::with_capacity(graph.tiers);
    let mut x = 0.0;
    for tier in 0..graph.tiers {
        tier_x.push(x);
        x += cell_w + gutter_w.get(tier).copied().unwrap_or(0.0);
    }
    // The trailing gutter is not content: the last column ends at its box.
    let content_w = tier_x.last().map(|last| last + cell_w).unwrap_or(0.0);
    let pitch_y = cell_h + lane_pitch * 2.0;
    let content_h = if graph.widest == 0 {
        0.0
    } else {
        (graph.widest - 1) as f32 * pitch_y + cell_h
    };

    GraphGeometry {
        pane,
        panel,
        cell_w,
        cell_h,
        tier_x,
        gutter_w,
        lanes: counts,
        pitch_y,
        content_w,
        content_h,
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

/// What the pane is not showing, and the keys that reach it.
///
/// `battle.rs::scroll_hint`'s shape and its reason: "there is more over
/// there" is only useful next to the way to get to it, and here the way is
/// the same arrow keys that move the cursor — the view has no key of its
/// own because it has no state of its own.
pub(super) fn view_hint(hidden: (usize, usize, usize, usize)) -> Option<String> {
    let (left, right, above, below) = hidden;
    let mut parts = Vec::new();
    if left + right > 0 {
        parts.push(match (left, right) {
            (0, r) => format!("{r} more right"),
            (l, 0) => format!("{l} more left"),
            (l, r) => format!("{l} left, {r} right"),
        });
    }
    if above + below > 0 {
        parts.push(match (above, below) {
            (0, b) => format!("{b} more below"),
            (a, 0) => format!("{a} more above"),
            (a, b) => format!("{a} above, {b} below"),
        });
    }
    (!parts.is_empty()).then(|| format!("Arrows — {}", parts.join(", ")))
}

/// Where each of `count` lines meets a box's edge: spread down it rather
/// than stacked on its middle, so a parent with four children emits four
/// lines from four points.
fn anchor_y(r: &Rect, index: usize, count: usize) -> f32 {
    r.y + r.h * (index as f32 + 1.0) / (count as f32 + 1.0)
}

fn intersects(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
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
    // The graph draws no popup, so it owes its own backdrop — and it has to
    // come first, or the map behind shows through every box and edge and
    // nothing on the tree is discernible. `draw_frame_map_cursor`'s shape:
    // the panel fill across the whole window, then the border.
    painter.rect(0.0, 0.0, screen_w, screen_h, PANEL_BG);
    painter.rect_lines(0.0, 0.0, screen_w, screen_h, 2.0, BORDER);
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
    let offset = graph
        .cell(&selected_id)
        .map(|c| geo.offset(c.tier, c.slot))
        .unwrap_or((0.0, 0.0));

    let routes = assign_lanes(&graph);
    // How many lines each box emits and receives, so an anchor can be
    // spread down its edge instead of pinned to its middle.
    let fan_out = |id: &str| routes.iter().filter(|r| r.from == id).count();
    let fan_in = |id: &str| routes.iter().filter(|r| r.to == id).count();

    painter.clipped(geo.pane.x, geo.pane.y, geo.pane.w, geo.pane.h, |p| {
        // Edges first, so a box paints over a line rather than the other way
        // round.
        for route in &routes {
            let (Some(a), Some(b)) = (graph.cell(&route.from), graph.cell(&route.to)) else {
                continue;
            };
            let ra = geo.cell_rect(a.tier, a.slot, offset);
            let rb = geo.cell_rect(b.tier, b.slot, offset);
            let lane = geo.pane.x + geo.lane_x(route.gutter, route.lane) - offset.0;
            let span = Rect::new(
                ra.x.min(lane).min(rb.x),
                ra.y.min(rb.y),
                (ra.x.max(rb.x + rb.w).max(lane) - ra.x.min(lane).min(rb.x)).max(1.0),
                (ra.y.max(rb.y) - ra.y.min(rb.y) + ra.h).max(1.0),
            );
            if !intersects(&span, &geo.pane) {
                continue;
            }
            let out_of = fan_out(&route.from);
            let into = fan_in(&route.to);
            let out_index = routes
                .iter()
                .filter(|r| r.from == route.from)
                .position(|r| r == route)
                .unwrap_or(0);
            let in_index = routes
                .iter()
                .filter(|r| r.to == route.to)
                .position(|r| r == route)
                .unwrap_or(0);
            let ay = anchor_y(&ra, out_index, out_of);
            let by = anchor_y(&rb, in_index, into);
            let live = route.to == selected_id;
            let color = if live { EDGE_LIVE } else { EDGE_DIM };
            let thickness = if live { 2.0 } else { 1.0 };
            p.line(ra.x + ra.w, ay, lane, ay, thickness, color);
            p.line(lane, ay, lane, by, thickness, color);
            p.line(lane, by, rb.x, by, thickness, color);
        }

        let columns = geo.label_columns(p, m);
        for cell in &graph.cells {
            let Some(node) = nodes.iter().find(|n| n.id == cell.id) else {
                continue;
            };
            let r = geo.cell_rect(cell.tier, cell.slot, offset);
            if !intersects(&r, &geo.pane) {
                continue;
            }
            if node.id == selected_id {
                p.rect(r.x, r.y, r.w, r.h, SELECT_BG);
            } else {
                // A box is opaque, or an edge routed under it shows through
                // and reads as an edge that ends nowhere.
                p.rect(r.x, r.y, r.w, r.h, PANEL_BG);
            }
            p.rect_lines(r.x, r.y, r.w, r.h, 1.0, row_color(node));
            let mut y = r.y + m.line_height;
            for line in box_label(&node.name, columns) {
                p.ui(line, r.x + m.pad, y, m.small(), row_color(node));
                y += m.line_height;
            }
            p.ui(
                format!("{}", node.cost),
                r.x + m.pad,
                (r.y + r.h - m.pad).max(y),
                m.small(),
                TEXT_DIM,
            );
        }
    });

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

    // The footer carries the refusal when there is one and the view hint
    // otherwise: a refusal is transient and the more urgent of the two, and
    // two strings on one row is how a message gets overdrawn.
    if let Some(refusal) = refusal {
        painter.ui(refusal, m.pad, screen_h - m.pad, m.font_size, RED);
    } else if let Some(hint) = view_hint(geo.hidden(offset)) {
        painter.ui(hint, m.pad, screen_h - m.pad, m.small(), TEXT_DIM);
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

    /// The pane pans, so "the whole tree fits" is no longer true and no
    /// longer the property to hold. **This is what replaces it**: whichever
    /// node the cursor is on is fully inside the pane, so no arrow key can
    /// move the selection somewhere the player cannot see it.
    ///
    /// 1280x720 is the smallest window the rest of the UI is held to.
    #[test]
    fn the_selected_node_is_always_fully_in_view_at_1280x720() {
        let g = shipped_graph();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        assert!(geo.cell_w > 0.0 && geo.cell_h > 0.0);
        for cell in &g.cells {
            let offset = geo.offset(cell.tier, cell.slot);
            let r = geo.cell_rect(cell.tier, cell.slot, offset);
            assert!(
                r.x >= geo.pane.x - 0.01 && r.x + r.w <= geo.pane.x + geo.pane.w + 0.01,
                "{} at tier {} is not in view horizontally: {:?} against {:?}",
                cell.id,
                cell.tier,
                (r.x, r.w),
                (geo.pane.x, geo.pane.w)
            );
            assert!(
                r.y >= geo.pane.y - 0.01 && r.y + r.h <= geo.pane.y + geo.pane.h + 0.01,
                "{} at slot {} is not in view vertically",
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

    /// The view clamps at both ends rather than scrolling into blank space:
    /// selecting the first tier puts the tree's own left edge at the pane's,
    /// and selecting the last puts its right edge at the pane's right.
    #[test]
    fn the_view_never_scrolls_past_the_content() {
        let g = shipped_graph();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        for cell in &g.cells {
            let (x, y) = geo.offset(cell.tier, cell.slot);
            assert!(
                x >= 0.0 && y >= 0.0,
                "{} scrolled before the origin",
                cell.id
            );
            assert!(
                x <= (geo.content_w - geo.pane.w).max(0.0) + 0.01,
                "{} scrolled past the right edge",
                cell.id
            );
            assert!(
                y <= (geo.content_h - geo.pane.h).max(0.0) + 0.01,
                "{} scrolled past the bottom edge",
                cell.id
            );
        }
        let first = g.cell("automation").expect("a tier 0 node");
        assert_eq!(geo.offset(first.tier, first.slot).0, 0.0);
        let last = g
            .cells
            .iter()
            .find(|c| c.tier == g.tiers - 1)
            .expect("a node in the last tier");
        assert!(
            (geo.offset(last.tier, last.slot).0 - (geo.content_w - geo.pane.w)).abs() < 0.01,
            "the last tier clamps against the content's right edge"
        );
    }

    /// The bug this view was reported for: every elbow in a gutter shared
    /// one `mid`, so the nine edges into tier 3 drew as a single vertical
    /// bar with stubs. Each edge now gets its own lane.
    #[test]
    fn no_two_edges_in_a_gutter_share_a_lane() {
        let g = shipped_graph();
        let m = ui_metrics(720.0);
        let geo = geometry(1280.0, 720.0, &g, &m);
        let routes = assign_lanes(&g);
        assert_eq!(routes.len(), g.edges.len(), "every edge is routed");
        for gutter in 0..g.tiers.saturating_sub(1) {
            let lanes: Vec<f32> = routes
                .iter()
                .filter(|r| r.gutter == gutter)
                .map(|r| geo.lane_x(r.gutter, r.lane))
                .collect();
            for (i, a) in lanes.iter().enumerate() {
                for b in &lanes[i + 1..] {
                    assert!(
                        (a - b).abs() > 1.0,
                        "gutter {gutter} draws two vertical runs at {a} and {b}"
                    );
                }
            }
            // And they stay inside the gutter they were sized for.
            let left = geo.content_rect(gutter, 0).x + geo.cell_w;
            let right = geo.content_rect(gutter + 1, 0).x;
            for x in &lanes {
                assert!(
                    *x > left && *x < right,
                    "a lane at {x} escaped its gutter ({left}..{right})"
                );
            }
        }
    }

    /// A parent with four children emits four lines from four points on its
    /// edge, not four from its middle — which is the other half of what made
    /// the fan unreadable.
    #[test]
    fn a_parents_lines_leave_from_distinct_points() {
        let r = Rect::new(0.0, 0.0, 100.0, 80.0);
        let ys: Vec<f32> = (0..4).map(|i| anchor_y(&r, i, 4)).collect();
        for pair in ys.windows(2) {
            assert!(pair[1] > pair[0], "anchors must descend the edge: {ys:?}");
        }
        assert!(
            ys.iter().all(|y| *y > r.y && *y < r.y + r.h),
            "every anchor stays on the edge: {ys:?}"
        );
        assert_eq!(
            anchor_y(&r, 0, 1),
            r.y + r.h * 0.5,
            "a lone line is centred"
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

    /// A modded tree deeper and wider than the shipped one **scrolls rather
    /// than squeezing** — this is the whole point of sizing a box off the
    /// font instead of off the pane. A 20x30 tree draws the same box the
    /// shipped 6x9 one does; what grows is the content behind the viewport.
    #[test]
    fn a_far_larger_tree_scrolls_rather_than_squeezing() {
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
        let shipped = geometry(1280.0, 720.0, &shipped_graph(), &m);
        assert_eq!(
            (geo.cell_w, geo.cell_h),
            (shipped.cell_w, shipped.cell_h),
            "a box does not shrink because the tree got bigger"
        );
        assert!(
            geo.content_w > shipped.content_w && geo.content_h > shipped.content_h,
            "the content grows instead"
        );
        // And every cell of it is still reachable: selecting it brings it
        // into view, which is the property that replaced "it all fits".
        for cell in &g.cells {
            let offset = geo.offset(cell.tier, cell.slot);
            let r = geo.cell_rect(cell.tier, cell.slot, offset);
            assert!(r.x >= geo.pane.x - 0.01 && r.x + r.w <= geo.pane.x + geo.pane.w + 0.01);
            assert!(r.y >= geo.pane.y - 0.01 && r.y + r.h <= geo.pane.y + geo.pane.h + 0.01);
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

    use crate::paint::{painted_fills, painted_rect_stroke_count, painted_text};
    use feral_processes_engine::ResearchState;

    /// Every tier is *reachable*, which is what "the shape is visible" means
    /// now that the pane pans: a node off the side of the viewport is culled
    /// rather than drawn, and selecting it brings it in.
    ///
    /// Asserted a tier at a time rather than over all 34 nodes, so a failure
    /// names where it broke — and by *selecting* a node in each tier, since
    /// with the cursor parked at tier 0 the far end of the tree is off-screen
    /// by construction.
    #[test]
    fn every_tier_is_drawn_when_the_cursor_reaches_it() {
        let mut game = Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nodes = game.research_nodes();
        let m = ui_metrics(720.0);
        for name in [
            "Automation",
            "Cache Coherence",
            "Segmentation",
            "Overclock",
            "Cortex",
            "Mesh Plating",
        ] {
            let selected = nodes
                .iter()
                .position(|n| n.name.contains(name))
                .unwrap_or_else(|| panic!("{name} is a shipped node"));
            let (_, shapes) =
                with_painter(|p| draw_research_graph(&mut game, selected, None, p, &m));
            let text = painted_text(&shapes).join("\n");
            assert!(
                text.contains(name),
                "{name} is not on the screen with its own node selected"
            );
        }
    }

    /// Culling is not an optimisation here, it is what keeps a box from
    /// painting over the detail panel: the far end of a panned tree must not
    /// be drawn at all when the cursor is at the near end.
    #[test]
    fn the_far_end_of_the_tree_is_culled_rather_than_drawn() {
        let mut game = Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_research_graph(&mut game, 0, None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(
            !text.contains("Mesh Plating"),
            "the last tier is five columns right of the first and cannot be on screen with it"
        );
    }

    /// The footer says what the viewport is not showing, `battle.rs`'s
    /// `scroll_hint` convention — "there is more over there" is only useful
    /// beside the way to reach it.
    #[test]
    fn the_footer_names_what_is_off_screen() {
        assert_eq!(
            view_hint((0, 0, 0, 0)),
            None,
            "a tree that fits says nothing"
        );
        let hint = view_hint((2, 1, 0, 3)).expect("a panned view raises a hint");
        assert!(hint.contains("Arrows"), "the hint names the key: {hint}");
        assert!(hint.contains("2 left, 1 right"), "{hint}");
        assert!(hint.contains("3 more below"), "{hint}");
    }

    /// The hint and a refusal share the footer row, so only one may be
    /// drawn — and the refusal is the one that wins.
    #[test]
    fn a_refusal_takes_the_footer_from_the_view_hint() {
        let mut game = Game::new(934, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let m = ui_metrics(720.0);
        let (_, shapes) =
            with_painter(|p| draw_research_graph(&mut game, 0, Some("Not enough data."), p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("Not enough data."), "the refusal is drawn");
        assert!(
            !text.contains("Arrows —"),
            "the view hint must not share the row with it"
        );
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

    /// The graph draws no popup, so it owes its own backdrop — and it has to
    /// be the *first* fill on the screen, covering the whole window, or the
    /// map behind shows through and nothing on the tree is discernible. That
    /// is how this shipped: every box, edge and label was drawn straight onto
    /// the world.
    #[test]
    fn the_graph_paints_a_backdrop_over_the_whole_window_first() {
        let mut game = Game::new(936, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_research_graph(&mut game, 0, None, p, &m));
        let fills = painted_fills(&shapes);
        let (index, rect) = *fills.first().expect("the screen paints something");
        assert_eq!(index, 0, "the backdrop must be the very first shape");
        assert!(
            rect.x <= 0.01 && rect.y <= 0.01,
            "the backdrop starts at the window's origin: {rect:?}"
        );
        assert!(
            rect.w >= p_screen_w() - 0.01 && rect.h >= p_screen_h() - 0.01,
            "the backdrop covers the whole window: {rect:?}"
        );
    }

    /// `with_painter`'s window, which the draw reads off the painter rather
    /// than being handed.
    fn p_screen_w() -> f32 {
        1440.0
    }
    fn p_screen_h() -> f32 {
        900.0
    }
}
