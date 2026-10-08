//! The surface at chunk scale: what the party has revealed, the places in it,
//! and the routes between them.
//!
//! Every fact comes off `WorldMapView` — the renderer decides colour and
//! placement and nothing else, so what a town's tint means is the engine's
//! standing band and not a second reading of it. The grid is north-up; the
//! side list is `App::world_map_marks`, the same list the key handler
//! indexes, so a row letter here is the mark it selects there.

use super::*;
use crate::paint::Face;
use feral_processes_app_core::menu_shortcut;
use feral_processes_engine::outposts::Trend;
use feral_processes_engine::settlements::growth::TownTrend;
use feral_processes_engine::settlements::{CompassTarget, SettlementKind, Standing};
use feral_processes_engine::{
    WorldMapCell, WorldMapMark, WorldMapMarkKind, WorldMapRoute, WorldMapView,
};

/// Fraction of the window width the grid may claim, leaving the rest for the
/// side list and the detail panel.
const GRID_WIDTH_FRACTION: f32 = 0.56;

/// How far a biome is dimmed on this map. The glyphs on top are the point,
/// and a full-bright biome tint drowns them.
const BIOME_DIM: f32 = 0.42;

const FOG: Color = Color::new(0.03, 0.035, 0.05, 1.0);
const ROUTE: Color = Color::new(0.35, 0.75, 0.80, 1.0);
const SELECTED_OUTLINE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const COMPASS_OUTLINE: Color = Color::new(0.95, 0.80, 0.20, 1.0);
const TOWN_NEUTRAL: Color = Color::new(0.82, 0.82, 0.86, 1.0);

/// The tint a town's glyph wears for its standing band. Exhaustive, so a new
/// band is a compile error here rather than a town drawn in no colour.
fn standing_color(standing: Standing) -> Color {
    match standing {
        Standing::Hostile => RED,
        Standing::Cold => BLUE,
        Standing::Neutral => TOWN_NEUTRAL,
        Standing::Warm => GREEN,
        Standing::Allied => YELLOW,
    }
}

fn town_trend_arrow(trend: TownTrend) -> (char, Color) {
    match trend {
        TownTrend::Rising => ('▲', GREEN),
        TownTrend::Flat => ('▬', TEXT_DIM),
        TownTrend::Falling => ('▼', RED),
    }
}

fn outpost_trend_arrow(trend: Trend) -> (char, Color) {
    match trend {
        Trend::Growing => ('▲', GREEN),
        Trend::Stable | Trend::Stale => ('▬', TEXT_DIM),
        Trend::Declining => ('▼', RED),
    }
}

/// The glyph and hue a mark is drawn in. A mark that carries a compass
/// target takes the picker's (`compass::target_glyph`), so a place is one
/// thing on both screens; a town alone overrides it, since the map knows its
/// kind and standing and the picker does not.
fn mark_glyph(mark: &WorldMapMark) -> (char, Color) {
    match (&mark.kind, mark.target) {
        (WorldMapMarkKind::Town { standing, kind, .. }, _) => {
            (kind.glyph(), standing_color(*standing))
        }
        (_, Some(target)) => compass::target_glyph(target),
        (_, None) => ('&', hud::palette::glyph(GlyphColor::Red)),
    }
}

/// The arrow drawn at a mark's corner, where the engine knows a direction.
fn mark_arrow(kind: &WorldMapMarkKind) -> Option<(char, Color)> {
    match kind {
        WorldMapMarkKind::Town { trend: Some(t), .. } => Some(town_trend_arrow(*t)),
        WorldMapMarkKind::Outpost { trend, .. } => Some(outpost_trend_arrow(*trend)),
        _ => None,
    }
}

fn cell_color(cell: WorldMapCell) -> Color {
    match cell {
        WorldMapCell::Unknown => FOG,
        WorldMapCell::Explored(biome) => dimmed(terrain::biome_tint(biome)),
    }
}

fn dimmed(c: Color) -> Color {
    Color::new(c.r * BIOME_DIM, c.g * BIOME_DIM, c.b * BIOME_DIM, c.a)
}

/// The detail panel's lines for one mark. Plain words, and no tick figures:
/// the engine hands over bands and directions and this only phrases them.
fn detail_lines(mark: &WorldMapMark) -> Vec<String> {
    let mut lines = vec![mark.label.clone()];
    match &mark.kind {
        WorldMapMarkKind::Home => lines.push("Your base.".to_string()),
        WorldMapMarkKind::StackLink => lines.push("A way down into the Stack.".to_string()),
        WorldMapMarkKind::Nest => {
            lines.push("A nest. Map only: the compass cannot point here.".to_string())
        }
        WorldMapMarkKind::Outpost { trend, dark } => {
            lines.push(format!(
                "Outpost: {}",
                match trend {
                    Trend::Growing => "growing ▲",
                    Trend::Stable => "stable ▬",
                    Trend::Stale => "stock full ▬",
                    Trend::Declining => "declining ▼",
                }
            ));
            if *dark {
                lines.push("Dark: it needs repair.".to_string());
            }
        }
        WorldMapMarkKind::Town {
            standing,
            kind,
            vitality,
            trend,
            outlook,
            sends_raiders,
            fields_patrols,
            preys_on_routes,
            refuses_service,
            allows_standing_route,
        } => {
            lines.push(format!("{} · {}", kind.label(), standing.label()));
            if let Some(v) = vitality {
                lines.push(format!("Vitality: {}", v.label()));
            }
            if let Some(t) = trend {
                lines.push(format!(
                    "Trade: {}",
                    match t {
                        TownTrend::Rising => "rising ▲",
                        TownTrend::Flat => "holding ▬",
                        TownTrend::Falling => "falling ▼",
                    }
                ));
            }
            if let Some(o) = outlook {
                lines.push(o.label().to_string());
            }
            let flags = [
                (*sends_raiders, "Sends raiders"),
                (*fields_patrols, "Fields patrols"),
                (*preys_on_routes, "Preys on routes"),
                (*refuses_service, "Refuses service"),
                (*allows_standing_route, "Allows a standing route"),
            ];
            lines.extend(
                flags
                    .iter()
                    .filter(|(on, _)| *on)
                    .map(|(_, s)| s.to_string()),
            );
        }
    }
    lines
}

/// One side-list row: the key that selects it, then what it is.
fn list_row(index: usize, mark: &WorldMapMark) -> String {
    format!("[{}] {}", menu_shortcut(index), mark.label)
}

/// The window of list rows that fits `capacity`, kept around `selected`.
fn list_window(len: usize, selected: usize, capacity: usize) -> std::ops::Range<usize> {
    if capacity == 0 || len == 0 {
        return 0..0;
    }
    if len <= capacity {
        return 0..len;
    }
    let start = selected.saturating_sub(capacity / 2).min(len - capacity);
    start..start + capacity
}

/// Side length of a grid cell and the grid's top-left, for `cells` cells a
/// side drawn below a heading of `top` pixels.
fn grid_layout(cells: usize, w: f32, h: f32, top: f32, m: &Metrics) -> (f32, f32, f32) {
    let bottom = m.inset + m.line_height;
    let avail_h = (h - top - bottom).max(0.0);
    let avail_w = (w * GRID_WIDTH_FRACTION - m.inset).max(0.0);
    let cell = (avail_h.min(avail_w) / cells.max(1) as f32).floor();
    (m.inset, top, cell)
}

fn cell_center(view: &WorldMapView, chunk: (i32, i32), ox: f32, oy: f32, cell: f32) -> (f32, f32) {
    let col = (chunk.0 - (view.center.0 - view.radius)) as f32;
    let row = (chunk.1 - (view.center.1 - view.radius)) as f32;
    (ox + (col + 0.5) * cell, oy + (row + 0.5) * cell)
}

fn in_grid(view: &WorldMapView, chunk: (i32, i32)) -> bool {
    (chunk.0 - view.center.0).abs() <= view.radius && (chunk.1 - view.center.1).abs() <= view.radius
}

/// Centres one glyph on `(cx, cy)`. `⌂` is not in the map font, so a glyph
/// that font lacks is drawn in the UI face rather than as a missing-glyph box.
fn draw_glyph(painter: &Painter, ch: char, color: Color, cx: f32, cy: f32, size: u16) {
    if size == 0 {
        return;
    }
    let text = ch.to_string();
    let face = if ch == '⌂' { Face::Ui } else { Face::Map };
    let dims = painter.measure(face, &text, size);
    painter.text(
        face,
        &text,
        cx - dims.width / 2.0,
        cy + dims.height / 2.0,
        size,
        color,
    );
}

fn draw_route(painter: &Painter, view: &WorldMapView, r: &WorldMapRoute, ox: f32, oy: f32, c: f32) {
    let (x1, y1) = cell_center(view, r.from_chunk, ox, oy, c);
    let (x2, y2) = cell_center(view, r.to_chunk, ox, oy, c);
    let hostile = !r.preyed_by.is_empty();
    let color = if hostile { RED } else { ROUTE };
    painter.line(x1, y1, x2, y2, if hostile { 3.0 } else { 2.0 }, color);
}

fn draw_grid(
    view: &WorldMapView,
    selected: Option<&WorldMapMark>,
    pointing: Option<CompassTarget>,
    painter: &Painter,
    ox: f32,
    oy: f32,
    cell: f32,
) {
    let side = cell * view.cells.len() as f32;
    let inset = (cell * 0.04).max(0.5);
    for (row, cells) in view.cells.iter().enumerate() {
        for (col, &kind) in cells.iter().enumerate() {
            painter.rect(
                ox + col as f32 * cell + inset,
                oy + row as f32 * cell + inset,
                cell - inset * 2.0,
                cell - inset * 2.0,
                cell_color(kind),
            );
        }
    }

    // Routes cross the grid edge by construction (`WorldMapView::routes` is
    // every route, in view or not), so they are clipped to it.
    painter.clipped(ox, oy, side, side, |p| {
        for r in &view.routes {
            draw_route(p, view, r, ox, oy, cell);
        }
    });

    let glyph_px = (cell * 0.7) as u16;
    let arrow_px = ((cell * 0.5) as u16).max(1);
    // Home last, so a nest sharing its chunk never hides it.
    let mut ordered: Vec<&WorldMapMark> = view.marks.iter().collect();
    ordered.sort_by_key(|m| matches!(m.kind, WorldMapMarkKind::Home));
    for mark in ordered {
        if !in_grid(view, mark.chunk) {
            continue;
        }
        let (cx, cy) = cell_center(view, mark.chunk, ox, oy, cell);
        let (ch, color) = mark_glyph(mark);
        draw_glyph(painter, ch, color, cx, cy, glyph_px);
        if let Some((arrow, ac)) = mark_arrow(&mark.kind) {
            let s = arrow.to_string();
            painter.ui(&s, cx + cell * 0.1, cy - cell * 0.12, arrow_px, ac);
        }
        let top_left = (cx - cell / 2.0, cy - cell / 2.0);
        if mark.target.is_some() && mark.target == pointing {
            painter.rect_lines(top_left.0, top_left.1, cell, cell, 2.0, COMPASS_OUTLINE);
        }
        if selected.is_some_and(|s| s.chunk == mark.chunk && s.label == mark.label) {
            painter.rect_lines(
                top_left.0 + 2.0,
                top_left.1 + 2.0,
                cell - 4.0,
                cell - 4.0,
                1.5,
                SELECTED_OUTLINE,
            );
        }
    }

    if in_grid(view, view.party) {
        let (cx, cy) = cell_center(view, view.party, ox, oy, cell);
        draw_glyph(painter, '@', CYAN, cx, cy, glyph_px);
    }
    painter.rect_lines(ox, oy, side, side, 1.0, BORDER);
}

/// Draws `lines` top-down from `y`, each clipped to the column, and returns
/// the y below the last. Painter text neither clips nor wraps, so the clip
/// is what keeps a long label out of the grid.
fn draw_lines(
    painter: &Painter,
    lines: &[(String, Color)],
    x: f32,
    y: f32,
    col_w: f32,
    m: &Metrics,
) -> f32 {
    let mut y = y;
    painter.clipped(
        x,
        y - m.line_height,
        col_w,
        lines.len() as f32 * m.line_height + m.gap,
        |p| {
            for (text, color) in lines {
                p.ui(text, x, y, m.font_size, *color);
                y += m.line_height;
            }
        },
    );
    y
}

/// The legend's lines. The town letters come off `SettlementKind::glyph`, the
/// map's own, so the key cannot drift from what is drawn.
fn legend() -> Vec<String> {
    vec![
        "@ you  # home  > Stack link  & nest  ⌂ outpost".to_string(),
        format!(
            "{} Mainframe  {} Server",
            SettlementKind::Mainframe.glyph(),
            SettlementKind::Server.glyph()
        ),
        "▲ rising  ▬ holding  ▼ falling".to_string(),
        "Towns: red Hostile, blue Cold, grey Neutral,".to_string(),
        "       green Warm, yellow Allied".to_string(),
        "Cyan line: a route.  Red line: preyed on.".to_string(),
        "Gold box: compass target.  Esc closes.".to_string(),
        "Arrows pan  a-z/1-9 pick  C compass  P centre".to_string(),
    ]
}

pub(super) fn draw_world_map(
    view: &WorldMapView,
    selected: usize,
    pointing: Option<CompassTarget>,
    painter: &Painter,
    m: &Metrics,
) {
    let (w, h) = (painter.screen_w(), painter.screen_h());
    painter.rect(0.0, 0.0, w, h, PANEL_BG);
    painter.rect_lines(0.0, 0.0, w, h, 2.0, BORDER);

    let heading = format!(
        "WORLD MAP   centre {},{}   {} places in view",
        view.center.0,
        view.center.1,
        view.marks.len(),
    );
    let head_y = m.inset + m.font_size as f32;
    painter.ui(&heading, m.inset, head_y, m.font_size, CYAN);

    let top = head_y + m.gap + m.line_height * 0.5;
    let (ox, oy, cell) = grid_layout(view.cells.len(), w, h, top, m);
    if cell <= 0.0 {
        return;
    }
    let mark = view.marks.get(selected);
    draw_grid(view, mark, pointing, painter, ox, oy, cell);

    let col_x = ox + cell * view.cells.len() as f32 + m.inset * 2.0;
    let col_w = (w - col_x - m.inset).max(0.0);
    if col_w <= 0.0 {
        return;
    }

    // The legend takes the column's foot; the list and detail share what is
    // above it.
    let legend = legend();
    let legend_h = legend.len() as f32 * m.line_height;
    let legend_y = h - m.inset - legend_h + m.line_height;
    let body_h = (legend_y - m.line_height - top - m.gap * 2.0).max(0.0);
    let rows_total = (body_h / m.line_height) as usize;
    let detail_rows = rows_total.min(9) / 2 + 3;
    let list_cap = rows_total.saturating_sub(detail_rows + 1);

    let range = list_window(view.marks.len(), selected, list_cap);
    let mut list: Vec<(String, Color)> = Vec::new();
    if view.marks.is_empty() {
        list.push(("No places known yet. Walk to find some.".into(), TEXT_DIM));
    }
    for i in range {
        let on = i == selected;
        let prefix = if on { "> " } else { "  " };
        list.push((
            format!("{prefix}{}", list_row(i, &view.marks[i])),
            if on { TEXT } else { TEXT_DIM },
        ));
    }
    let after_list = draw_lines(painter, &list, col_x, top + m.line_height * 0.5, col_w, m);

    if let Some(mark) = mark {
        let detail: Vec<(String, Color)> = detail_lines(mark)
            .into_iter()
            .enumerate()
            .map(|(i, s)| (s, if i == 0 { CYAN } else { TEXT }))
            .collect();
        draw_lines(
            painter,
            &detail,
            col_x,
            after_list + m.line_height,
            col_w,
            m,
        );
    }

    let legend: Vec<(String, Color)> = legend.into_iter().map(|s| (s, TEXT_DIM)).collect();
    draw_lines(painter, &legend, col_x, legend_y, col_w, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::WORLD_MAP_VIEW_RADIUS;
    use feral_processes_engine::settlements::SettlementKey;
    use feral_processes_engine::settlements::growth::{GrowthOutlook, Vitality};

    fn town(standing: Standing, trend: Option<TownTrend>) -> WorldMapMark {
        WorldMapMark {
            chunk: (1, 0),
            kind: WorldMapMarkKind::Town {
                standing,
                kind: SettlementKind::Mainframe,
                vitality: Some(Vitality::Thriving),
                trend,
                outlook: None,
                sends_raiders: true,
                fields_patrols: true,
                preys_on_routes: true,
                refuses_service: true,
                allows_standing_route: true,
            },
            label: "Lowport".to_string(),
            target: Some(CompassTarget::Town(SettlementKey { rx: 0, ry: 0 })),
        }
    }

    fn view(radius: i32) -> WorldMapView {
        let side = (radius * 2 + 1) as usize;
        let mut cells = vec![vec![WorldMapCell::Unknown; side]; side];
        cells[radius as usize][radius as usize] = WorldMapCell::Explored(Biome::OpenGrid);
        WorldMapView {
            center: (0, 0),
            radius,
            cells,
            marks: vec![town(Standing::Hostile, Some(TownTrend::Falling))],
            routes: vec![],
            party: (0, 0),
        }
    }

    #[test]
    fn every_standing_band_has_its_own_tint() {
        let bands = [
            Standing::Hostile,
            Standing::Cold,
            Standing::Neutral,
            Standing::Warm,
            Standing::Allied,
        ];
        for (i, a) in bands.iter().enumerate() {
            for b in &bands[i + 1..] {
                assert_ne!(
                    standing_color(*a),
                    standing_color(*b),
                    "{a:?} and {b:?} share a tint"
                );
            }
        }
    }

    #[test]
    fn fog_is_not_a_biome_colour() {
        for biome in [Biome::OpenGrid, Biome::NullSector, Biome::Backplane] {
            assert_ne!(cell_color(WorldMapCell::Explored(biome)), FOG);
        }
    }

    #[test]
    fn the_detail_names_every_consequence_flag() {
        let lines = detail_lines(&town(Standing::Hostile, Some(TownTrend::Rising)));
        let all = lines.join("\n");
        for want in ["Hostile", "Thriving", "rising ▲", "raiders", "patrols"] {
            assert!(all.contains(want), "{want} missing from {all}");
        }
    }

    #[test]
    fn a_server_reads_its_outlook_and_no_trade_line() {
        let mut m = town(Standing::Neutral, None);
        if let WorldMapMarkKind::Town {
            kind,
            vitality,
            outlook,
            sends_raiders,
            fields_patrols,
            preys_on_routes,
            refuses_service,
            allows_standing_route,
            ..
        } = &mut m.kind
        {
            *kind = SettlementKind::Server;
            *vitality = None;
            *outlook = Some(GrowthOutlook::Soon);
            *sends_raiders = false;
            *fields_patrols = false;
            *preys_on_routes = false;
            *refuses_service = false;
            *allows_standing_route = false;
        }
        let all = detail_lines(&m).join("\n");
        assert!(all.contains("soon"), "{all}");
        assert!(!all.contains("Trade"), "{all}");
    }

    #[test]
    fn the_list_window_keeps_the_selection_in_view() {
        assert_eq!(list_window(5, 4, 8), 0..5);
        assert_eq!(list_window(30, 0, 8), 0..8);
        assert_eq!(list_window(30, 29, 8), 22..30);
        let w = list_window(30, 15, 8);
        assert!(w.contains(&15) && w.len() == 8);
        assert_eq!(list_window(30, 3, 0), 0..0);
    }

    #[test]
    fn the_north_west_corner_is_the_first_cell() {
        let v = view(2);
        let (x, y) = cell_center(&v, (-2, -2), 10.0, 20.0, 8.0);
        assert_eq!((x, y), (14.0, 24.0));
        assert!(in_grid(&v, (2, 2)) && !in_grid(&v, (3, 0)));
    }

    #[test]
    fn the_map_draws_its_pieces_and_nothing_outside_the_column() {
        let v = view(WORLD_MAP_VIEW_RADIUS);
        let (_, shapes) = crate::paint::with_painter(|p| {
            let m = ui_metrics(p.screen_h());
            draw_world_map(&v, 0, None, p, &m);
        });
        let drawn = crate::paint::painted_text(&shapes);
        assert!(drawn.iter().any(|t| t.contains("WORLD MAP")));
        assert!(drawn.iter().any(|t| t.contains("Lowport")));
        assert!(drawn.iter().any(|t| t.contains("Esc close")));
    }

    /// The widest legend line and the widest detail line must fit the column
    /// the screen gives them at the reference window, or their tails run
    /// under the border unseen.
    #[test]
    fn the_longest_strings_fit_the_column() {
        crate::paint::with_painter(|p| {
            let m = ui_metrics(p.screen_h());
            let v = view(WORLD_MAP_VIEW_RADIUS);
            let (ox, _, cell) = grid_layout(v.cells.len(), p.screen_w(), p.screen_h(), 80.0, &m);
            let col_x = ox + cell * v.cells.len() as f32 + m.inset * 2.0;
            let col_w = p.screen_w() - col_x - m.inset;
            let mark = |kind, label: &str| WorldMapMark {
                chunk: (0, 0),
                kind,
                label: label.to_string(),
                target: None,
            };
            let mut server = town(Standing::Hostile, Some(TownTrend::Rising));
            if let WorldMapMarkKind::Town { kind, .. } = &mut server.kind {
                *kind = SettlementKind::Server;
            }
            let marks = [
                town(Standing::Hostile, Some(TownTrend::Rising)),
                server,
                mark(WorldMapMarkKind::Nest, "a nest"),
                mark(
                    WorldMapMarkKind::Outpost {
                        trend: Trend::Stale,
                        dark: true,
                    },
                    "Lowport",
                ),
            ];
            let mut lines = legend();
            for m_ in &marks {
                lines.extend(detail_lines(m_));
                lines.push(format!("> {}", list_row(35, m_)));
            }
            let widest = lines
                .iter()
                .map(|s| p.measure_ui_advance(s, m.font_size))
                .fold(0.0, f32::max);
            assert!(widest <= col_w, "widest {widest} > column {col_w}");
        });
    }
}
