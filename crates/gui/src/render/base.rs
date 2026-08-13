//! The map screen: terrain, entities, effects, and the status panel beside them.

use super::bars::*;
use super::field::draw_status_buffs;
use super::stack::draw_stack;
use super::*;

/// Fraction of the window the map pane occupies — the zone map or, while the
/// party is underground, the first-person corridor. Named rather than inline
/// because the corner map inset is placed and sized inside this rect, and
/// `frame_map`'s tests have to be able to build the pane the real one draws
/// into rather than a plausible-looking copy of it.
pub(super) const PANE_W: f32 = 0.7;
pub(super) const PANE_H: f32 = 0.72;

/// How far a bare tile's background may stray from its biome's flat colour,
/// as a fraction either side. Enough to break up a field of identical tiles,
/// not enough to read as two different biomes.
const SHADE_JITTER: f32 = 0.08;
/// How dark the map pane's corners get relative to its centre. Floored well
/// short of illegible: the vignette is depth, and must never be the reason a
/// hostile at the pane's edge goes unnoticed.
const VIGNETTE_MIN: f32 = 0.75;

/// The staffed mark's side, as a fraction of the tile, and how far it is held
/// off the tile's edges. The inset is not cosmetic: `outline_open` drops the
/// edges a chained pair shares, and a mark flush into the corner would read as
/// painting one of those absent lines back in.
const STAFFED_MARK: f32 = 0.28;
const STAFFED_MARK_INSET: f32 = 2.0;

/// A tile's own brightness multiplier, so a field of one biome reads as
/// ground rather than as a flat colour swatch.
///
/// Hashed from the world coordinate, never the screen cell: the camera now
/// slides continuously across tiles, and a shade tied to screen position
/// would crawl over the terrain as it went. The two axes are mixed with
/// different constants because a symmetric hash bands the map along its
/// diagonal, which reads as a pattern instead of as texture.
fn tile_shade(world: (i32, i32)) -> f32 {
    let t = (tile_hash(world) & 0xFFFF) as f32 / 65535.0;
    1.0 - SHADE_JITTER + 2.0 * SHADE_JITTER * t
}

/// The one hash keyed on a world coordinate, shared by `tile_shade` and by
/// every biome pattern that varies from tile to tile.
///
/// Hashed from the world coordinate, never the screen cell: the camera
/// slides continuously across tiles, and anything tied to screen position
/// would crawl over the terrain as it went. The two axes are mixed with
/// different constants because a symmetric hash bands the map along its
/// diagonal, which reads as a pattern instead of as texture.
fn tile_hash(world: (i32, i32)) -> u32 {
    let mut h =
        (world.0 as u32).wrapping_mul(0x9E37_79B9) ^ (world.1 as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    h
}

/// Radial dimming toward the edge of the map pane, given a tile's offset from
/// the pane centre and the pane's half-extent, both in pixels.
///
/// Anchored to the pane rather than to the grid, so it stays put while the
/// camera slides beneath it. Normalising by the half-extent is what keeps the
/// gradient the same shape at every zoom step and window size, and squaring
/// the radius keeps the centre broadly flat so the falloff reads only near
/// the edges.
fn vignette(dx: f32, dy: f32, half_w_px: f32, half_h_px: f32) -> f32 {
    let r = ((dx / half_w_px).powi(2) + (dy / half_h_px).powi(2))
        .sqrt()
        .min(1.0);
    1.0 - (1.0 - VIGNETTE_MIN) * r * r
}

/// A biome's colour, and with it the map's one colour rule: **hue answers
/// "can I cross this", pattern answers "what is it".** The five walkable
/// biomes stay in a cool family and the two that are holes in the map — see
/// `Biome::walkable` — go hot amber. Which is why this returns a
/// colour rather than drawing one: `every_biomes_tint_says_whether_it_can_be
/// _walked_on` can only assert that rule against a value, and a biome tinted
/// into the wrong family tells the player they may walk into the void.
///
/// Exhaustive on purpose, the same call `render/stack.rs`'s `cell_mark`
/// makes: a new `Biome` must not compile until someone has decided which
/// side of that rule it falls on.
fn biome_tint(biome: Biome) -> Color {
    match biome {
        // Hot: the edge of the world. Nothing is ever placed on these, so
        // they never share a tile with the red durability wash
        // `Fx::structure_condition` paints — but they do sit next to it, so
        // they stay dark ground while that stays a bright wash under a glyph.
        Biome::DataVoid => Color::new(0.95, 0.60, 0.15, 1.0),
        Biome::BlackIce => Color::new(0.95, 0.32, 0.18, 1.0),
        // Cool: ground. The four the world generates are spread across
        // brightness rather than hue, since hue is already spoken for by the
        // rule above.
        //
        // Platform is the exception, held apart by being much the darkest of
        // the five. It is the only biome the player lays, so it covers whole
        // screens wherever a base stands, and the base is the one screen with
        // a dozen glyphs and machine-status outlines to read at once — at the
        // bright cyan this used to be, it drowned them. Dark navy is what that
        // brightness problem actually needed: still unmistakably cool, so the
        // hue rule above is untouched, but dark enough to sit under a full
        // base. Taken down a second time after being seen on screen — the
        // number that reads as "dark navy" in this table is brighter than the
        // one that reads as dark navy behind a base, because everything else
        // on that screen is competing with it.
        Biome::Platform => Color::new(0.06, 0.11, 0.32, 1.0),
        Biome::Mainframe => Color::new(0.25, 0.85, 0.85, 1.0),
        Biome::StaticField => Color::new(0.70, 0.92, 0.95, 1.0),
        Biome::OpenGrid => Color::new(0.35, 0.85, 0.60, 1.0),
        Biome::NullSector => Color::new(0.20, 0.50, 0.52, 1.0),
    }
}

/// Whether the edge these two biomes share is the edge of the walkable
/// world. This is the whole of what makes the map read as terrain rather
/// than as a colour field: the rim is drawn where it returns true and
/// nowhere else, so a shoreline appears around every hole in the map
/// without anything having to know which biomes are holes.
fn rim(a: Biome, b: Biome) -> bool {
    a.walkable() != b.walkable()
}

/// How bright a biome's pattern is against its own ground fill, and how
/// bright the rim along the edge of the walkable world is against both.
/// The ground stays at `GROUND_LEVEL` so terrain never competes with the
/// entity glyphs standing on it — the whole point of keeping glyphs for
/// actors is that they win.
const GROUND_LEVEL: f32 = 0.18;
const PATTERN_LEVEL: f32 = 0.55;
const RIM_LEVEL: f32 = 0.95;
/// The faint lit line between adjacent walkable tiles. Dim enough to read as
/// a substrate the world is printed on rather than as content.
const GRID_LEVEL: f32 = 0.10;

/// What an impassable biome's pattern drops to. Seen on screen, DataVoid and
/// BlackIce at the full `PATTERN_LEVEL` dominated the pane — a wall of amber
/// rings and red shards louder than the ground the player actually walks on,
/// and louder than the entities standing on it. They are terrain the player
/// can never interact with, so they belong in the background: the rim already
/// says "you cannot cross here", and the pattern only has to say which of the
/// two it is.
const VOID_PATTERN_LEVEL: f32 = 0.30;

/// How many tiles beyond the visible pane `draw_surface_map` fetches. One for
/// the camera to slide in from, one so that tile has neighbours to compare
/// biomes against. See the call site for why both are needed.
const RINGS: i32 = 2;

/// `c` scaled toward black by `level`, alpha untouched. Every terrain colour
/// on the map is this function applied to `biome_tint`, which is what keeps
/// ground, pattern and rim reading as three depths of one material instead
/// of three colours that happen to sit together.
fn at_level(c: Color, level: f32) -> Color {
    Color::new(c.r * level, c.g * level, c.b * level, c.a)
}

/// The geometry that says which biome this is, drawn inside the tile at
/// `(px, py)`. Pattern carries identity because hue is already spoken for by
/// passability — see `biome_tint`.
///
/// Exhaustive for the same reason `biome_tint` is: a new `Biome` should stop
/// the build until someone has drawn it, rather than shipping as bare
/// ground the way a `_ => {}` arm would let it. This is the trap
/// `render/stack.rs`'s `cell_mark` was fixed for, and it is the same trap.
fn draw_biome(painter: &Painter, biome: Biome, r: Rect, tint: Color, world: (i32, i32)) {
    let ink = at_level(
        tint,
        if biome.walkable() {
            PATTERN_LEVEL
        } else {
            VOID_PATTERN_LEVEL
        },
    );
    let h = tile_hash(world);
    match biome {
        Biome::Mainframe => draw_traces(painter, r, ink, h),
        Biome::OpenGrid => draw_dot(painter, r, ink),
        Biome::NullSector => draw_broken_grid(painter, r, ink, h),
        Biome::StaticField => draw_speckle(painter, r, ink, h),
        Biome::Platform => draw_slab(painter, r, ink),
        Biome::DataVoid => draw_depth(painter, r, ink),
        Biome::BlackIce => draw_shards(painter, r, ink, h),
    }
}

/// Mainframe: a circuit trace entering the tile and terminating in a pad.
/// Which side it enters from is hashed, so a field of Mainframe reads as
/// routed board rather than as one motif stamped in a grid.
fn draw_traces(painter: &Painter, r: Rect, ink: Color, h: u32) {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let t = (r.w * 0.09).max(1.0);
    let (ex, ey) = match h % 4 {
        0 => (r.x, cy),
        1 => (r.x + r.w, cy),
        2 => (cx, r.y),
        _ => (cx, r.y + r.h),
    };
    painter.line(ex, ey, cx, cy, t, ink);
    let pad = r.w * 0.18;
    painter.rect(cx - pad / 2.0, cy - pad / 2.0, pad, pad, ink);
}

/// OpenGrid: a single centred node. The sparsest pattern in the set, because
/// this is the biome the player crosses most and it should read as open.
fn draw_dot(painter: &Painter, r: Rect, ink: Color) {
    let d = (r.w * 0.14).max(1.0);
    painter.rect(r.x + (r.w - d) / 2.0, r.y + (r.h - d) / 2.0, d, d, ink);
}

/// NullSector: the grid, but with pieces missing. Two dashes out of a
/// possible four, hashed — dead substrate rather than live board.
fn draw_broken_grid(painter: &Painter, r: Rect, ink: Color, h: u32) {
    let t = (r.w * 0.07).max(1.0);
    let len = r.w * 0.3;
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    if h & 1 == 0 {
        painter.line(r.x, cy, r.x + len, cy, t, ink);
    }
    if h & 2 == 0 {
        painter.line(r.x + r.w - len, cy, r.x + r.w, cy, t, ink);
    }
    if h & 4 == 0 {
        painter.line(cx, r.y, cx, r.y + len, t, ink);
    }
}

/// StaticField: three specks at hashed offsets. Noise, so it wants no
/// structure at all — the only pattern in the set with nothing aligned to
/// the tile's centre or edges.
fn draw_speckle(painter: &Painter, r: Rect, ink: Color, h: u32) {
    let d = (r.w * 0.09).max(1.0);
    for i in 0..3 {
        let hx = h.rotate_left(i * 7);
        let fx = (hx & 0xFF) as f32 / 255.0;
        let fy = ((hx >> 8) & 0xFF) as f32 / 255.0;
        let x = r.x + d + fx * (r.w - 2.0 * d);
        let y = r.y + d + fy * (r.h - 2.0 * d);
        painter.rect(x, y, d, d, ink);
    }
}

/// Platform: a solid slab, inset so the base's floor reads as laid over the
/// terrain rather than as more terrain. Deliberately the *darkest* ground on
/// the map: the base is the one place the player has a dozen glyphs and
/// machine-status outlines to read at once, and the floor's whole job there
/// is to stay out from under them. What says "the player made this" is the
/// slab's shape — the only unbroken fill in the set — not its brightness.
fn draw_slab(painter: &Painter, r: Rect, ink: Color) {
    let i = r.w * 0.12;
    painter.rect(r.x + i, r.y + i, r.w - 2.0 * i, r.h - 2.0 * i, ink);
}

/// DataVoid: concentric rings falling away to black, so a hole in the map
/// reads as depth rather than as flat colour. No grid and no ink —
/// everything else on the map is printed on a substrate, and the point of
/// this one is that the substrate has ended.
fn draw_depth(painter: &Painter, r: Rect, ink: Color) {
    // One ring, not a nest of them. Three read as a target stamped on every
    // tile, which made a lake of DataVoid look tiled rather than deep — the
    // opposite of the point. A single inset ring gives the tile an inner
    // shadow and lets the expanse stay an expanse.
    let i = r.w * 0.22;
    painter.rect_lines(r.x + i, r.y + i, r.w - 2.0 * i, r.h - 2.0 * i, 1.0, ink);
}

/// The four edges of one tile: a bright rim wherever the walkable world ends,
/// and a faint grid line wherever two walkable tiles simply meet.
///
/// The rim is drawn by the *walkable* tile only, never by the void beside it.
/// That is what makes it a shoreline rather than an outline — the lit edge
/// belongs to the ground it bounds, so a lake of DataVoid is ringed once, from
/// the outside, instead of twice with the two halves fighting over the same
/// pixels. It is also why this can run per tile with no memory of what it drew
/// before: `rim` is symmetric, and the walkable-side rule is what breaks the tie.
///
/// Neighbours come from the fetched grid rather than the engine, which is the
/// whole reason edge-awareness costs nothing here: `view_tiles` already hands
/// back `RINGS` tiles more than the pane shows in every direction, so every
/// tile that can be drawn has all four of its neighbours in hand. An absent
/// neighbour therefore means the outermost fetched ring, which is off-pane —
/// it draws nothing rather than guessing.
fn draw_tile_edges(
    painter: &Painter,
    tiles: &[Vec<Tile>],
    rx: usize,
    ry: usize,
    cell: Rect,
    tint: Color,
    vig: f32,
) {
    let here = tiles[ry][rx].biome;
    let neighbour = |dx: i32, dy: i32| -> Option<Biome> {
        let nx = usize::try_from(rx as i32 + dx).ok()?;
        let ny = usize::try_from(ry as i32 + dy).ok()?;
        Some(tiles.get(ny)?.get(nx)?.biome)
    };
    // Each edge as (neighbour offset, the two endpoints of the shared side).
    let edges = [
        ((0, -1), (cell.x, cell.y), (cell.x + cell.w, cell.y)),
        (
            (0, 1),
            (cell.x, cell.y + cell.h),
            (cell.x + cell.w, cell.y + cell.h),
        ),
        ((-1, 0), (cell.x, cell.y), (cell.x, cell.y + cell.h)),
        (
            (1, 0),
            (cell.x + cell.w, cell.y),
            (cell.x + cell.w, cell.y + cell.h),
        ),
    ];
    for ((dx, dy), (x1, y1), (x2, y2)) in edges {
        let Some(there) = neighbour(dx, dy) else {
            continue;
        };
        if rim(here, there) {
            if !here.walkable() {
                continue;
            }
            let t = (cell.w * 0.10).max(1.5);
            // The halo goes down first and wider, so the rim sits in its own
            // bloom rather than beside it. This is the map's only glow: the
            // edge of the world is the one thing worth spending it on.
            painter.line(x1, y1, x2, y2, t * 2.5, at_level(tint, 0.20 * vig));
            painter.line(x1, y1, x2, y2, t, at_level(tint, RIM_LEVEL * vig));
        } else if here.walkable() && (dx + dy) > 0 {
            // Right and bottom only — an edge is shared, and both owners
            // drawing it would double the alpha on every interior line while
            // the pane's outer edges stayed single.
            painter.line(x1, y1, x2, y2, 1.0, at_level(tint, GRID_LEVEL * vig));
        }
    }
}

/// BlackIce: an upward shard. The one pattern built from `poly` rather than
/// rects and lines, because a jagged silhouette is the read — this is the
/// biome that kills you, and it should not look machined like the rest.
fn draw_shards(painter: &Painter, r: Rect, ink: Color, h: u32) {
    let lean = ((h & 0xFF) as f32 / 255.0 - 0.5) * r.w * 0.3;
    let base = r.y + r.h * 0.82;
    let apex = r.y + r.h * 0.18;
    let cx = r.x + r.w / 2.0;
    painter.poly(
        &[
            (cx + lean, apex),
            (r.x + r.w * 0.82, base),
            (r.x + r.w * 0.18, base),
        ],
        ink,
    );
}

/// The world grid, status panel, and message feed — the base layer shown
/// under `Mode::Playing` and every menu popup, same as `ui.rs::render_playing`.
pub(super) fn draw_playing_base(app: &mut App, fx: &mut Fx, painter: &Painter, m: &Metrics) {
    let (tile_px, glyph_px) = map_cell(app.zoom);
    let status_line = app.status_line.clone();
    // Read before the `game` borrow, like `status_line` above.
    let stack_zoom = app.stack_zoom;
    // The pane's rows, chosen by app-core (see `pane_rows`), and the header
    // that says which channel they are. Both read before the `game` borrow.
    let log_h = painter.screen_h() - painter.screen_h() * PANE_H;
    let log_capacity = ((log_h - m.line_height) / m.line_height).max(1.0) as usize;
    let log_lines = app.visible_log(log_capacity.saturating_sub(1));
    let log_header = log_pane_header(app.log_filter, app.filtered_out_log_lines());
    let Some(game) = &mut app.game else { return };

    let map_w = painter.screen_w() * PANE_W;
    let map_h = painter.screen_h() * PANE_H;

    let status = game.player_status();
    // `Game::active_buffs` needs `&mut self`; fetched here rather than
    // inside `draw_status_panel`, which only ever needed `&Game` before
    // this and shouldn't have to start borrowing mutably just to draw.
    let buffs = game.active_buffs();
    if let Some(view) = game.stack_view() {
        draw_stack(&view, painter, map_w, map_h, m);
        // Over the corridor, not part of it: the same map the `g` screen
        // draws, small enough to leave the view readable.
        if let Some(map) = game.frame_map() {
            draw_map_inset(&map, stack_zoom, painter, map_w, map_h, m);
        }
    } else {
        draw_surface_map(game, fx, painter, map_w, map_h, tile_px, glyph_px, &status);
    }

    draw_status_panel(
        Rect::new(map_w, 0.0, painter.screen_w() - map_w, map_h),
        &status,
        &buffs,
        game,
        painter,
        m,
    );

    let log_y = map_h;
    painter.rect(0.0, log_y, painter.screen_w(), log_h, PANEL_BG);
    painter.rect_lines(
        0.0,
        log_y,
        painter.screen_w(),
        log_h,
        2.0,
        fx.log_border(BORDER),
    );
    let mut ly = log_y + m.inset + m.font_size as f32 / 2.0;
    if let Some(s) = &status_line {
        painter.ui(s, m.inset, ly, m.font_size, RED);
        ly += m.line_height;
    }
    // Drawn even under `LogFilter::All`, so the key is discoverable from the
    // screen rather than only from the help popup.
    let runs: Vec<TextRun> = log_header
        .iter()
        .map(|p| TextRun {
            text: &p.text,
            bold: p.bold,
            color: p.color,
        })
        .collect();
    painter.ui_runs(&runs, m.inset, ly, m.font_size);
    ly += m.line_height;
    for e in &log_lines {
        if ly > painter.screen_h() - m.gap {
            break;
        }
        draw_message_line(e.kind, &e.text, m.inset, ly, painter, m);
        ly += m.line_height;
    }
}

/// One styled stretch of the log pane's header. Owned rather than
/// `paint::TextRun`, which borrows: the pieces are built from a formatted
/// count that has to outlive the call, and the caller turns them into runs at
/// the point it draws.
struct HeaderPiece {
    text: String,
    bold: bool,
    color: Color,
}

impl HeaderPiece {
    fn dim(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            color: GRAY,
        }
    }
}

/// The log pane's one-line header: every filter with the active one picked
/// out, the key that cycles them, and — when a channel is being suppressed —
/// how much of it is going unread. That last part is what stops a raid alert
/// landing unseen while the pane is showing only field news.
///
/// All three are listed rather than only the active one, which is what the
/// header used to do: named alone, "Field" says nothing about what the other
/// settings are or which way the key steps, and a player reading a base line
/// under a header they thought said otherwise has no way to tell whether the
/// filter or the tagging is wrong. The order is `LogFilter::ALL`, which is the
/// order the key walks.
fn log_pane_header(filter: LogFilter, filtered_out: usize) -> Vec<HeaderPiece> {
    let mut pieces = vec![HeaderPiece::dim("LOG  ")];
    for (i, option) in LogFilter::ALL.iter().enumerate() {
        if i > 0 {
            pieces.push(HeaderPiece::dim(" · "));
        }
        pieces.push(if *option == filter {
            HeaderPiece {
                text: option.label().to_string(),
                bold: true,
                color: GREEN,
            }
        } else {
            HeaderPiece::dim(option.label())
        });
    }
    // Lower case because the binding is: `App::handle_playing_key` matches
    // `'f'` and nothing matches `'F'`, so the old wording sent anyone reaching
    // for shift to a key that does nothing.
    pieces.push(HeaderPiece::dim("   f to cycle"));
    if let Some(channel) = filter.hidden_channel()
        && filtered_out > 0
    {
        pieces.push(HeaderPiece::dim(format!(
            "   {filtered_out} {channel} hidden"
        )));
    }
    pieces
}

/// The message log in full — everything the pane at the bottom of the map
/// has room for a few lines of.
///
/// The footer states the screen's three limits rather than leaving the player
/// to infer them from an absence: the engine keeps `MESSAGE_LOG_CAP` lines,
/// repeats are folded into one row apiece, and
/// `MessageLog::retain_outcomes_since_battle` drops a finished intrusion's
/// blow-by-blow, so an old fight reads as its results.
pub(super) fn draw_history(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let entries = game.message_history(MESSAGE_LOG_CAP);
    let mut rows = history_rows(&entries, selected);
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "The last {MESSAGE_LOG_CAP} lines, repeats folded. A finished intrusion keeps its results, not its blow-by-blow."
    )));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("History", PopupSize::Large, &rows, painter, m);
}

/// The scrollable body of the history screen: one row per folded entry (see
/// `Game::message_history`), oldest first to match the map's pane, each in its
/// `MessageKind` colour through the same `message_color` that pane's
/// `draw_message_line` calls.
///
/// Rows are `Row::Item` because that is what `popup_layout` scrolls; the
/// highlight is the scroll position, not a selection, and
/// `App::handle_history_key` accepts nothing that would pick one. Which makes
/// the row count load-bearing: app-core counts the same folded entries to
/// bound that highlight, so a row here without one there is a highlight
/// pointing at nothing.
fn history_rows(entries: &[LogEntry], selected: usize) -> Vec<Row> {
    if entries.is_empty() {
        return vec![text_row("Nothing has happened yet.")];
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            counted_item_row(
                &entry.text,
                entry.repeats,
                i == selected,
                message_color(entry.kind),
            )
        })
        .collect()
}

/// The zone map: terrain, entities and effects, drawn top-down into the pane
/// at the origin. The other half of the pane's contents is `draw_stack`,
/// which replaces this entirely while the party is underground.
#[allow(clippy::too_many_arguments)]
fn draw_surface_map(
    game: &mut Game,
    fx: &mut Fx,
    painter: &Painter,
    map_w: f32,
    map_h: f32,
    tile_px: f32,
    glyph_px: u16,
    status: &feral_processes_engine::PlayerStatus,
) {
    // Two rings wider than the pane can show. The first is the tile the
    // camera's sub-tile offset slides in from, without which the trailing
    // edge goes blank; the second exists so *that* tile has neighbours of its
    // own to compare biomes with, since a tile cannot know whether it sits on
    // the edge of the walkable world without seeing what is beside it. Every
    // grid-to-world conversion below goes through `hw`/`hh` for that reason —
    // the rings shift the whole grid by two cells.
    let half_w = ((map_w / tile_px) / 2.0).max(1.0) as i32;
    let half_h = ((map_h / tile_px) / 2.0).max(1.0) as i32;
    let hw = half_w + RINGS;
    let hh = half_h + RINGS;

    let (off_x, off_y) = fx.camera_offset(status.position, painter.delta());
    let tiles = game.view_tiles(hw, hh);
    let entities: Vec<_> = game
        .view_entities(hw, hh)
        .into_iter()
        // A tamed program is drawn only while it is out on an errand. At its
        // post it sits under its machine's own glyph, so a base at rest
        // reads as buildings and motion is the only thing that draws the
        // eye — a worker appearing *is* the news that it has left to
        // deliver. Everything else tamed stays hidden for a harder reason:
        // nothing ever walks a guard, an idle program or a party member, so
        // each keeps whatever tile it was standing on when it took the job
        // and drawing it would claim it is somewhere it isn't.
        .filter(|e| !e.is_tamed || e.worker_away_from_post)
        .collect();
    let spawn_point = game.zone_spawn_point();
    let shield_outline = fx.shield_outline(game.raid_defense_active());

    painter.rect(0.0, 0.0, map_w, map_h, Color::new(0.03, 0.03, 0.05, 1.0));
    for (ry, row) in tiles.iter().enumerate() {
        for (rx, tile) in row.iter().enumerate() {
            let biome_color = biome_tint(tile.biome);
            // Terrain no longer carries a glyph — the biome is drawn as
            // geometry — so this stays `None` unless something is standing
            // here. That is the whole division of labour on this map:
            // terrain is shapes, actors are glyphs.
            let mut ch = None;
            let mut color = biome_color;
            let mut bg_source = biome_color;
            let world = (
                status.position.0 + rx as i32 - hw,
                status.position.1 + ry as i32 - hh,
            );
            let (px, py) = tile_origin_px(
                world,
                status.position,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
            );
            // The fetched rings exist to be *read* — by the camera slide and
            // by `draw_tile_edges` — not to be drawn. Nothing clips this pane,
            // and the log panel below it is drawn at 0.95 alpha, so a row
            // sitting past the bottom edge shows through it as a band of
            // terrain behind the text. Culling here rather than shrinking the
            // grid keeps every visible tile's neighbours in hand.
            if px >= map_w || py >= map_h || px + tile_px <= 0.0 || py + tile_px <= 0.0 {
                continue;
            }
            let mut machine_status = None;
            let mut linked_edges: &[(i32, i32)] = &[];
            let mut shielded = false;
            let mut critical = false;
            // A structure and an actor genuinely share a tile now that
            // posted workers are drawn: `place_structure` never makes its
            // tile unwalkable, and a hauling program's route to a depot
            // crosses the base slab. So the two are gathered separately
            // rather than last-one-wins, which resolved by
            // `view_entities` iteration order and would have flickered a
            // worker in and out of existence as it walked over a machine.
            let mut structure: Option<&EntityView> = None;
            let mut actor: Option<&EntityView> = None;
            // The entity wearing the mark, and whether its work has hit the
            // dead end below.
            let mut mark: Option<(Entity, bool)> = None;
            for ev in &entities {
                let erx = ev.pos.0 - status.position.0 + hw;
                let ery = ev.pos.1 - status.position.1 + hh;
                if erx != rx as i32 || ery != ry as i32 {
                    continue;
                }
                if ev.is_structure {
                    structure = Some(ev);
                } else if !matches!(actor, Some(a) if a.is_player) {
                    actor = Some(ev);
                }
                if if ev.is_structure {
                    ev.structure_attended
                } else {
                    ev.worker_away_from_post
                } {
                    mark = Some((ev.entity, ev.output_stranded));
                }
            }
            let occupied = structure.is_some() || actor.is_some();
            if let Some(ev) = structure {
                machine_status = ev.machine_status;
                linked_edges = &ev.linked_edges;
                shielded = true;
                ch = Some(ev.glyph);
                // Structures wear their raid damage: the glyph dims as
                // durability drops, and a nearly-destroyed one washes
                // its tile red, so the base's condition reads at a
                // glance instead of only from the inspect menu.
                let dimmed;
                (dimmed, critical) = fx.structure_condition(ev.durability, glyph_color(ev.color));
                // Background follows the damage-dimmed *authored* colour,
                // deliberately taken before the status override below.
                // The tile wash already means raid damage — a clogged
                // machine tinting its tile red too would make a
                // half-destroyed one and a full buffer look alike. Taken
                // from the structure even when someone is standing on it:
                // the wash is the building's condition, not the walker's.
                bg_source = dimmed;
                // A machine's glyph is its state: the `$` of a Mining
                // Node reads green running, yellow starved, red clogged,
                // grey idle. Which structure it is stays legible from the
                // glyph itself, so the authored colour is only carrying
                // identity a machine can spare. Anything that runs no job
                // keeps its authored colour.
                //
                // Damage-tinted through the same call, so a battered
                // machine still dims rather than reading box-fresh.
                color = match ev.machine_status {
                    Some(status) => {
                        fx.structure_condition(ev.durability, machine_color(status))
                            .0
                    }
                    None => dimmed,
                };
            }
            // An actor takes the tile's glyph off a structure and never the
            // other way round. The machine keeps every channel it has —
            // status outline, chain links, shield, damage wash — and gives
            // up only the glyph, which is the one thing that cannot show two
            // things at once.
            if let Some(ev) = actor {
                ch = Some(ev.glyph);
                color = glyph_color(ev.color);
            }
            // Bare ground only. Where something is standing, the background
            // carries the damage-dimmed glyph colour, and jittering that
            // would muddy a structure's durability read.
            let shade = if occupied { 1.0 } else { tile_shade(world) };
            let vig = vignette(
                px + tile_px / 2.0 - map_w / 2.0,
                py + tile_px / 2.0 - map_h / 2.0,
                map_w / 2.0,
                map_h / 2.0,
            );
            let dim = shade * vig;
            let mut bg = at_level(bg_source, GROUND_LEVEL * dim);
            if critical {
                bg = Color::new((bg.r + GROUND_LEVEL).min(1.0), bg.g, bg.b, bg.a);
            }
            let cell = Rect::new(px, py, tile_px - 1.0, tile_px - 1.0);
            painter.rect(cell.x, cell.y, cell.w, cell.h, bg);
            // Bare ground only, for the same reason the shade jitter above is:
            // where something is standing, the tile is carrying that thing's
            // damage-dimmed colour, and a biome pattern drawn through it would
            // muddy the durability read.
            if !occupied {
                draw_biome(painter, tile.biome, cell, at_level(biome_color, dim), world);
                draw_tile_edges(painter, &tiles, rx, ry, cell, biome_color, vig);
            }
            // The glyph takes the vignette but not the shade: depth should
            // apply to everything on the map evenly, while per-tile jitter is
            // a property of the ground, not of what stands on it.
            let color = Color::new(color.r * vig, color.g * vig, color.b * vig, color.a);
            if let Some(ch) = ch {
                let glyph = ch.to_string();
                let dims = painter.measure_map(&glyph, glyph_px);
                let tx = px + (tile_px - dims.width) / 2.0;
                let ty = py + (tile_px + dims.height) / 2.0;
                painter.map(&glyph, tx, ty, glyph_px, color);
            }
            // A rare-spawn tier draws as a bar along the top edge rather
            // than by recolouring the glyph, because the glyph's colour is
            // already spoken for: a hostile is tinted by `difficulty_color`
            // (green through red by power ratio against the player), which
            // is the "can I win this fight" read and cannot be given up.
            // Two readings, two channels — the glyph says how dangerous,
            // the bar says how rare.
            //
            // Keyed off `actor` and never `structure`: a structure has no
            // tier, and the actor is what owns this channel. Vignette but
            // not the tile shade, matching the glyph's rule above.
            if let Some(bar) = actor.and_then(|ev| rarity_color(ev.rarity)) {
                painter.rect(
                    px,
                    py,
                    tile_px - 1.0,
                    RARITY_BAR_PX,
                    Color::new(bar.r * vig, bar.g * vig, bar.b * vig, bar.a),
                );
            }
            // Marks where the player materialized on breaching into this
            // zone (see `Game::zone_spawn_point`) — an outline rather than
            // replacing the glyph, so whatever's actually standing there
            // (the player, a creature, a rebuilt structure) still reads
            // clearly on top of it.
            let spawn_rx = spawn_point.0 - status.position.0 + hw;
            let spawn_ry = spawn_point.1 - status.position.1 + hh;
            if rx as i32 == spawn_rx && ry as i32 == spawn_ry {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, MAGENTA);
            }
            // The shield network is base-wide, not per-structure, so every
            // structure carries the same faint pulse while one is standing.
            // Drawn under the flash so a raid still reads on top of it, and
            // through the same open-sided outline so it cannot paint a joined
            // pair's shared wall back in.
            if let Some(pulse) = shield_outline.filter(|_| shielded) {
                outline_open(painter, px, py, tile_px - 1.0, pulse, linked_edges);
            }
            // A machine wears its state as its outline, and drops the walls
            // it shares with a machine it trades with — so a working chain
            // draws as one continuous shape and a machine that should be
            // joined and isn't shows a seam. Nothing is drawn *between*
            // tiles; the join is the absence of a line.
            //
            // Drawn after the shield pulse, which also outlines every
            // structure: an actionable per-machine state has to win over
            // ambient base-wide info, or a starved machine goes unnoticed
            // inside a shielded base. A structure that runs no job has no
            // status and keeps only the pulse.
            if let Some(color) = machine_status.map(machine_color) {
                outline_open(painter, px, py, tile_px - 1.0, color, linked_edges);
            }
            // "Someone is on this job", on a channel of its own rather than
            // sharing the outline with machine state. It was a yellow outline
            // until machines took that channel over, at which point a machine
            // could never show it at all — leaving grey `Idle` as the only
            // trace of an unstaffed one, a colour meaning absence on an axis
            // that also carries three other things. Now the outline says what
            // the machine is doing and the mark says whether anyone is on it.
            //
            // One sentence covers where it goes: **on the program when the
            // program is drawn, and on the structure when it isn't.** So a
            // machine wears it while its worker stands at its post, the
            // worker takes it along the moment it leaves to deliver, and a
            // guard — which is never drawn — leaves it on the structure for
            // good. Exactly one mark per posted program at every instant,
            // which `a_worked_machine_and_its_worker_never_both_wear_the_
            // mark` holds from the engine side.
            //
            // The machine is not left silent while its worker is out: it
            // goes `Unstaffed` on the outline channel, "its program is
            // away." And with no depot built there is no errand at all, so
            // nothing ever leaves and nothing is ever drawn — see
            // `with_no_depot_a_clogged_machine_just_stays_clogged`.
            //
            // A machine that is full with nowhere to send its output is the
            // one case where the mark stops moving: its worker will never
            // leave, so a bob would promise motion that is never coming. It
            // blinks in place instead — see `Fx::stranded_blink`.
            if let Some((marked, stranded)) = mark {
                let size = (tile_px - 1.0) * STAFFED_MARK;
                // Orange as well as still: colour and motion say the same
                // thing at once, so a stranded machine is legible from a
                // paused screenshot and not only from watching it. It is
                // deliberately not the `RED` a clogged outline already
                // wears — being full is the machine's own problem and
                // recoverable by collecting, while this is the base having
                // nowhere left to put anything.
                let (lift, alpha, base) = if stranded {
                    (0.0, fx.stranded_blink(), ORANGE)
                } else {
                    (fx.staffed_bob(marked), 1.0, GREEN)
                };
                painter.rect(
                    px + STAFFED_MARK_INSET,
                    py + tile_px - 1.0 - STAFFED_MARK_INSET - size - lift,
                    size,
                    size,
                    Color { a: alpha, ..base },
                );
            }
            if let Some(flash) = fx.tile_flash(world) {
                painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
            }
        }
    }
    // After every tile so debris lands on top of the base rather than under
    // it, and before the border so a spark from a structure at the pane's
    // edge cannot draw over the frame.
    fx.draw_bursts(painter, tile_px, |world| {
        tile_origin_px(
            world,
            status.position,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
        )
    });
    painter.rect_lines(0.0, 0.0, map_w, map_h, 2.0, BORDER);
}

/// Where a world tile's top-left corner falls in the map pane.
///
/// The tile loop walks grid indices and the spark pass walks world tiles,
/// and both have to land on the same pixel. `half` is the pane's half-extent
/// *without* the extra rings — the rings are fetched to be read, not drawn,
/// so they cost a leading offset that keeps the pane framing the same view
/// it did before the camera existed.
fn tile_origin_px(
    world: (i32, i32),
    player: (i32, i32),
    half: (i32, i32),
    off: (f32, f32),
    tile_px: f32,
) -> (f32, f32) {
    (
        ((world.0 - player.0 + half.0) as f32 - off.0) * tile_px,
        ((world.1 - player.1 + half.1) as f32 - off.1) * tile_px,
    )
}

/// A tile outline with the sides in `open` left off — the sides this
/// machine shares with one it is joined to.
///
/// `EntityView::linked_edges` is symmetric for this to work: both halves of
/// a shared wall have to go, and dropping only the consumer's side would
/// leave a single line between the pair that reads as a rendering fault
/// rather than as a join.
fn outline_open(painter: &Painter, px: f32, py: f32, size: f32, color: Color, open: &[(i32, i32)]) {
    let closed = |d: (i32, i32)| !open.contains(&d);
    if closed((0, -1)) {
        painter.line(px, py, px + size, py, 2.0, color);
    }
    if closed((0, 1)) {
        painter.line(px, py + size, px + size, py + size, 2.0, color);
    }
    if closed((-1, 0)) {
        painter.line(px, py, px, py + size, 2.0, color);
    }
    if closed((1, 0)) {
        painter.line(px + size, py, px + size, py + size, 2.0, color);
    }
}

/// A machine's state colour, worn by both its glyph and its outline. The
/// six are ordered by what the player should do about them: green needs
/// nothing, grey needs a program, yellow needs a feeder or is waiting on one
/// to walk back, red needs the player to go and act — a trip home with `c`
/// for a clog, or a path cleared for a program that cannot get there at all.
fn machine_color(status: MachineStatus) -> Color {
    match status {
        MachineStatus::Running => GREEN,
        MachineStatus::Starved | MachineStatus::Unstaffed => YELLOW,
        // Red rather than yellow: unlike `Unstaffed`, waiting does not fix
        // this one, so it belongs with the states that are asking for you.
        MachineStatus::Clogged | MachineStatus::Stranded => RED,
        MachineStatus::Idle => TEXT_DIM,
    }
}

/// One party member's line in the status column, indented under the
/// `Party: n/m` heading it belongs to.
fn party_row(companion: &feral_processes_engine::CompanionInfo) -> String {
    format!(
        "  {} (HP {}/{}, PWR {})",
        companion.name, companion.hp, companion.max_hp, companion.power
    )
}

fn draw_status_panel(
    rect: Rect,
    status: &feral_processes_engine::PlayerStatus,
    buffs: &[feral_processes_engine::ActiveBuffView],
    game: &Game,
    painter: &Painter,
    m: &Metrics,
) {
    let Rect { x, y, w, h } = rect;
    painter.rect(x, y, w, h, PANEL_BG);
    painter.rect_lines(x, y, w, h, 2.0, BORDER);

    // Clears the panel border by one inset, then drops to the first
    // baseline; both terms grow with the font the rows are drawn in.
    let mut cy = y + m.inset + m.font_size as f32 / 2.0;
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        &format!("Integrity {}/{}", status.hp, status.max_hp.max(1)),
        status.hp as f32,
        status.max_hp.max(1) as f32,
        BarStyle::plain(RED),
        painter,
        m,
    );
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        &format!("Power {:.0}/100", status.hunger),
        status.hunger,
        100.0,
        BarStyle::plain(YELLOW),
        painter,
        m,
    );
    cy = draw_bar(
        BarGeometry {
            x: x + m.inset,
            y: cy,
            w: w - m.inset * 2.0,
        },
        &format!("Fatigue {:.0}/100", status.fatigue),
        status.fatigue,
        100.0,
        BarStyle::plain(BLUE),
        painter,
        m,
    );
    cy += m.gap;

    let lines = [
        format!(
            "Level {}  (XP {}/{})  Perk Pts {}",
            status.level, status.xp, status.xp_to_next, status.perk_points
        ),
        format!("Zone {}", status.zone),
        format!("Position: ({}, {})", status.position.0, status.position.1),
        format!(
            "Attack {}  Defense {}  Power {}",
            status.atk, status.def, status.power
        ),
        format!("Decompiler {}", status.decompiler),
    ];
    for line in &lines {
        painter.ui(line, x + m.inset, cy, m.font_size, TEXT);
        cy += m.line_height;
    }
    painter.ui(
        format!(
            "Party: {}/{}",
            status.companions.len(),
            feral_processes_engine::tuning::MAX_PARTY_SIZE
        ),
        x + m.inset,
        cy,
        m.font_size,
        GREEN,
    );
    cy += m.line_height;
    // Drawn between the two counts rather than after both: the rows carry no
    // label of their own, so the only thing saying they are party members and
    // not pets is which heading they follow.
    for companion in &status.companions {
        painter.ui(party_row(companion), x + m.inset, cy, m.font_size, GREEN);
        cy += m.line_height;
    }
    painter.ui(
        format!("Pets: {}/{}", status.pet_count, status.pet_capacity),
        x + m.inset,
        cy,
        m.font_size,
        GREEN,
    );
    cy += m.line_height;
    cy += m.gap;

    // Computed ahead of the routines section (rather than just above the
    // inventory loop, which is the only place that used to need it) so
    // `draw_status_buffs` can clip its own rows against the same footer
    // the inventory list already does — a party running routines on every
    // slot of every holder can outgrow the column exactly the way a full
    // inventory can.
    let keys = [
        "hjkl/arrows move  . wait  e drain  r recharge",
        "b base menu   p party menu   i pack",
        "c collect  t trade  a routine  u symlink  x examine",
        "L history  f filter  s save  q main menu  ? help  +/- zoom",
    ];
    let keys_line_height = m.line_height - m.gap;
    let keys_block_h = keys.len() as f32 * keys_line_height + m.inset;
    let keys_y = y + h - keys_block_h;

    cy = draw_status_buffs(buffs, x + m.inset, cy, keys_y, painter, m);
    painter.ui("Inventory:", x + m.inset, cy, m.font_size, TEXT);
    cy += m.line_height;
    if status.inventory.is_empty() {
        painter.ui("(empty)", x + m.inset, cy, m.font_size, TEXT_DIM);
        cy += m.line_height;
    }

    for row in &status.inventory {
        if cy > keys_y - m.line_height {
            break;
        }
        // Not a menu row, so no `fusion_row` here — the pane's own dim is
        // what the fusion colour replaces. The tier is spelled out beside
        // it because this pane has no room for the equip tag the inventory
        // screen carries, and colour alone doesn't say how deep.
        let tier = match row.copy.tier {
            0 => String::new(),
            tier => format!(" {}", item_fusion_note(tier)),
        };
        painter.ui(
            format!("{}{tier} x{}", game.item_name(&row.copy.item), row.qty),
            x + m.inset,
            cy,
            m.font_size,
            fusion_color(row.copy.tier).unwrap_or(TEXT_DIM),
        );
        cy += m.line_height;
    }

    let mut ky = keys_y;
    for k in keys {
        painter.ui(k, x + m.inset, ky, m.small(), TEXT_DIM);
        ky += keys_line_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::MessageSource;

    /// Every `Biome` variant, listed the way `species.rs`'s own biome census
    /// lists them. A new variant missing from here makes the tint census
    /// below pass vacuously, which is the failure mode that census exists to
    /// prevent — but `biome_tint`'s match is exhaustive, so a new biome
    /// cannot reach a test run without someone having already been sent to
    /// this file by the compiler.
    const ALL_BIOMES: [Biome; 7] = [
        Biome::DataVoid,
        Biome::StaticField,
        Biome::NullSector,
        Biome::Mainframe,
        Biome::OpenGrid,
        Biome::BlackIce,
        Biome::Platform,
    ];

    /// Whether a tint reads as hostile: red dominant over both other
    /// channels. The map's whole colour rule is that hue answers "can I
    /// cross this", so this is the question the census below asks.
    fn reads_as_hostile(c: Color) -> bool {
        c.r > c.g && c.r > c.b
    }

    /// The palette's one load-bearing promise: hue tells the player whether
    /// terrain can be walked on, and pattern tells them which biome it is.
    /// A biome tinted into the wrong family is worse than a drawing bug —
    /// it tells the player they can walk into the void.
    #[test]
    fn every_biomes_tint_says_whether_it_can_be_walked_on() {
        for biome in ALL_BIOMES {
            assert_eq!(
                reads_as_hostile(biome_tint(biome)),
                !biome.walkable(),
                "{biome:?} is walkable={} but its tint {:?} reads the other way — \
                 hue is the map's only signal for passability",
                biome.walkable(),
                biome_tint(biome),
            );
        }
    }

    /// The rim is the edge of the world: it is drawn exactly where
    /// passability changes, so it cannot appear inside walkable ground or
    /// inside the void. Symmetric because an edge is shared by two tiles and
    /// must not depend on which one is asking.
    #[test]
    fn a_rim_marks_only_the_boundary_between_walkable_and_not() {
        for a in ALL_BIOMES {
            for b in ALL_BIOMES {
                assert_eq!(
                    rim(a, b),
                    a.walkable() != b.walkable(),
                    "rim({a:?}, {b:?}) disagrees with their walkability"
                );
                assert_eq!(rim(a, b), rim(b, a), "rim is not symmetric for {a:?}/{b:?}");
            }
        }
    }

    fn header_text(filter: LogFilter, filtered_out: usize) -> String {
        log_pane_header(filter, filtered_out)
            .iter()
            .map(|p| p.text.as_str())
            .collect()
    }

    /// The header is the only place the filter key is advertised, so it draws
    /// under `All` too — a filter you can only discover from the help popup is
    /// one nobody turns on. Lower case `f`, because that is the key that is
    /// bound; `F` reaches nothing.
    #[test]
    fn the_unfiltered_header_still_names_the_key_and_counts_nothing() {
        let header = header_text(LogFilter::All, 0);
        assert!(header.contains("All"), "{header}");
        assert!(header.contains("f to cycle"), "{header}");
        assert!(!header.contains("hidden"), "nothing is hidden: {header}");
    }

    /// The whole set is listed whichever one is active, which is the point of
    /// the row: "Field" alone says nothing about what else there is.
    #[test]
    fn the_header_lists_every_filter_in_cycle_order() {
        for filter in LogFilter::ALL {
            let header = header_text(filter, 0);
            let labels: Vec<&str> = LogFilter::ALL.iter().map(|f| f.label()).collect();
            let mut cursor = 0;
            for label in &labels {
                let at = header[cursor..]
                    .find(label)
                    .unwrap_or_else(|| panic!("{label} missing from {header:?} under {filter:?}"));
                cursor += at + label.len();
            }
        }
    }

    /// Bold green is the only thing distinguishing the active filter from the
    /// two it sits between, so it has to land on exactly one piece.
    #[test]
    fn only_the_active_filter_is_picked_out() {
        for filter in LogFilter::ALL {
            let pieces = log_pane_header(filter, 0);
            let picked: Vec<&str> = pieces
                .iter()
                .filter(|p| p.bold && p.color == GREEN)
                .map(|p| p.text.as_str())
                .collect();
            assert_eq!(picked, [filter.label()], "under {filter:?}");
        }
    }

    /// The count is what stops a raid landing unseen while the pane is showing
    /// only field news.
    #[test]
    fn a_filtered_header_counts_the_channel_it_is_hiding() {
        let header = header_text(LogFilter::Field, 3);
        assert!(header.contains("Field"), "{header}");
        assert!(header.contains("3 base hidden"), "{header}");
    }

    /// A channel with no traffic in it has nothing to report, so the header
    /// stays quiet rather than saying "0 base".
    #[test]
    fn a_filtered_header_with_an_empty_channel_says_nothing() {
        let header = header_text(LogFilter::Base, 0);
        assert!(!header.contains("hidden"), "{header}");
    }

    fn entry(text: &str, repeats: usize) -> LogEntry {
        LogEntry {
            kind: MessageKind::Info,
            source: MessageSource::Field,
            text: text.to_string(),
            repeats,
        }
    }

    fn companion(name: &str) -> feral_processes_engine::CompanionInfo {
        feral_processes_engine::CompanionInfo {
            entity: Entity::PLACEHOLDER,
            name: name.to_string(),
            hp: 22,
            max_hp: 30,
            atk: 8,
            def: 5,
            power: 41,
            status: None,
            ability: "Rally".to_string(),
        }
    }

    /// The row's own word for what it is was redundant against the
    /// `Party: n/m` heading it now sits under, and cost the width the stats
    /// need in a 30%-wide column.
    #[test]
    fn a_party_row_names_the_program_without_labelling_it() {
        let row = party_row(&companion("Sparkgrub"));
        assert!(!row.contains("Companion"), "{row}");
        assert!(row.contains("Sparkgrub"), "{row}");
        assert!(row.contains("HP 22/30"), "{row}");
        assert!(row.contains("PWR 41"), "{row}");
    }

    /// Without the prefix the rows are bare names, so the indent is what
    /// keeps them reading as the heading's contents rather than as further
    /// headings.
    #[test]
    fn a_party_row_is_indented_under_its_heading() {
        let row = party_row(&companion("Hexweave"));
        assert!(row.starts_with("  "), "{row:?}");
        assert!(!row.trim_start().starts_with(' '));
    }

    fn suffix_of(row: &Row) -> Option<&str> {
        match row {
            Row::Item { suffix, .. } => suffix.as_deref(),
            _ => None,
        }
    }

    /// The count is an annotation on the row, not part of the sentence — so it
    /// only appears where there is something to count.
    #[test]
    fn a_folded_row_carries_its_count_and_a_lone_row_carries_none() {
        let entries = vec![entry("extracted 2 Data Shard.", 14), entry("a raid", 1)];
        let rows = history_rows(&entries, 0);
        assert_eq!(rows.len(), 2, "one row per entry, folded or not");
        assert_eq!(suffix_of(&rows[0]), Some("×14"));
        assert_eq!(suffix_of(&rows[1]), None);
    }

    #[test]
    fn an_empty_history_says_so_instead_of_listing_nothing() {
        let rows = history_rows(&[], 0);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0], Row::Text(_)), "nothing to scroll to");
    }

    #[test]
    fn tile_shade_is_stable_for_a_given_world_coordinate() {
        // The camera slides continuously over a tile now. A shade derived
        // from anything but the world coordinate would shimmer as it went.
        assert_eq!(tile_shade((12, -7)), tile_shade((12, -7)));
    }

    #[test]
    fn tile_shade_stays_within_the_jitter_band() {
        for x in -60..60 {
            for y in -60..60 {
                let s = tile_shade((x, y));
                assert!(
                    (1.0 - SHADE_JITTER..=1.0 + SHADE_JITTER).contains(&s),
                    "shade {s} at ({x}, {y}) escaped the band"
                );
            }
        }
    }

    #[test]
    fn tile_shade_actually_varies_between_neighbours() {
        let row: Vec<f32> = (0..40).map(|x| tile_shade((x, 5))).collect();
        let first = row[0];
        assert!(
            row.iter().any(|s| (s - first).abs() > SHADE_JITTER / 4.0),
            "a whole row came out flat — the hash isn't spreading"
        );
    }

    /// A hash that treats the axes alike bands the map along the diagonal,
    /// which reads as a pattern rather than as texture.
    #[test]
    fn tile_shade_distinguishes_the_two_axes() {
        assert_ne!(tile_shade((3, 9)), tile_shade((9, 3)));
    }

    #[test]
    fn the_vignette_leaves_the_centre_of_the_pane_untouched() {
        assert_eq!(vignette(0.0, 0.0, 400.0, 300.0), 1.0);
    }

    #[test]
    fn the_vignette_bottoms_out_at_its_floor_and_never_below() {
        assert!((vignette(400.0, 0.0, 400.0, 300.0) - VIGNETTE_MIN).abs() < 1e-6);
        // The corners sit past the unit radius and must clamp rather than
        // keep darkening.
        assert!(vignette(400.0, 300.0, 400.0, 300.0) >= VIGNETTE_MIN);
        assert!(vignette(9999.0, 9999.0, 400.0, 300.0) >= VIGNETTE_MIN);
    }

    #[test]
    fn the_vignette_darkens_monotonically_outward() {
        let mut previous = f32::MAX;
        for i in 0..=20 {
            let v = vignette(i as f32 * 20.0, 0.0, 400.0, 300.0);
            assert!(
                v <= previous,
                "brightened at step {i}: {v} after {previous}"
            );
            previous = v;
        }
    }

    /// Normalising by the pane's half-extent is what keeps the gradient the
    /// same shape at every zoom step and window size.
    #[test]
    fn the_vignette_depends_on_position_within_the_pane_not_on_its_size() {
        let small = vignette(100.0, 75.0, 400.0, 300.0);
        let large = vignette(200.0, 150.0, 800.0, 600.0);
        assert!((small - large).abs() < 1e-6, "{small} vs {large}");
    }
}
