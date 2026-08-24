//! The map screen: terrain, entities, effects, and the status panel beside them.

use super::bars::*;
use super::field::draw_status_buffs;
use super::stack::draw_stack;
use super::*;
use feral_processes_engine::views::drawn_on_surface_map;

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

/// The Excavation plan's three washes, all one hue so a plan reads as one
/// thing. A committed mark's fill is dim enough to walk over without the
/// base becoming unreadable and its edge carries the shape; the box being
/// previewed is brighter than either, because it is the thing about to
/// happen and has to read over a mark it may be drawn across.
const MARK_FILL: Color = Color::new(0.9, 0.8, 0.2, 0.18);
const MARK_EDGE: Color = Color::new(0.9, 0.8, 0.2, 0.45);
const PREVIEW_FILL: Color = Color::new(0.9, 0.8, 0.2, 0.35);

/// The nemesis mark's side, as a fraction of the tile, and how far it sits
/// off the tile's edges. Smaller than `STAFFED_MARK` and placed in the
/// opposite corner (top-right rather than bottom-left), so a marked program
/// standing on a machine-adjacent tile can never collide with either a
/// staffed mark or the outline `outline_open` drops along a chained pair's
/// shared edge.
const NEMESIS_MARK: f32 = 0.22;
const NEMESIS_MARK_INSET: f32 = 2.0;

/// Where the nemesis mark sits on a tile — the top-right corner, dropped
/// below `RARITY_BAR_PX` so it never overlaps the bar running the width of
/// the top edge, and inset from both remaining edges for the same reason
/// `STAFFED_MARK_INSET` is: flush into a corner it would read as painting
/// back in an edge `outline_open` deliberately left off.
///
/// A free function rather than inlined at the call site so the geometry is
/// unit-testable without a `Painter` — see the `nemesis_mark_clears_the_
/// rarity_bar` test.
fn nemesis_mark_rect(px: f32, py: f32, tile_px: f32) -> Rect {
    let size = (tile_px - 1.0) * NEMESIS_MARK;
    Rect::new(
        px + tile_px - 1.0 - NEMESIS_MARK_INSET - size,
        py + RARITY_BAR_PX + NEMESIS_MARK_INSET,
        size,
        size,
    )
}

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
///
/// `hues` is the sector's `(ground, hazard)` pair from `Game::sector_hues`,
/// and it moves the two *bands* rather than recolouring biomes. The match
/// below stays the reference table it always was — the sector rotates each
/// entry's hue by however far its own band's anchor has moved, so every
/// biome keeps its offset within the band along with its saturation and
/// value. That is what leaves the brightness spread — the thing that
/// actually separates the five walkable biomes from each other, and that
/// keeps Platform much the darkest — untouched by a sector.
///
/// **The neutral pair is a zero rotation and is not applied**, so an install
/// with no `assets/sectors/` draws these literals bit for bit.
fn biome_tint(biome: Biome, hues: (f32, f32)) -> Color {
    let base = biome_reference_tint(biome);
    let (anchor, authored) = if biome.walkable() {
        (feral_processes_engine::sectors::NEUTRAL_GROUND_HUE, hues.0)
    } else {
        (feral_processes_engine::sectors::NEUTRAL_HAZARD_HUE, hues.1)
    };
    rotate_hue(base, authored - anchor)
}

/// `c` scaled toward white by `factor`, hue untouched.
///
/// Clamped at 1.0 so a channel cannot wrap, and applied to the *biome's own*
/// colour rather than to a fixed value, so a rock face brightens whatever
/// the sector palette has done to the hole around it.
fn brighten(c: Color, factor: f32) -> Color {
    Color::new(
        (c.r * factor).min(1.0),
        (c.g * factor).min(1.0),
        (c.b * factor).min(1.0),
        c.a,
    )
}

/// `c` with its hue moved `degrees` around the wheel, saturation and value
/// untouched.
///
/// The saturation/value spread is what separates the biomes within a band
/// from each other; hue is what separates walkable from not. Moving only H
/// shifts the band without disturbing either — see `biome_tint`.
///
/// A zero rotation returns `c` itself rather than round-tripping it through
/// HSV and back. Not an optimisation: it is what makes "deleting
/// `assets/sectors/` restores today's game" exactly true rather than true to
/// within a float epsilon.
fn rotate_hue(c: Color, degrees: f32) -> Color {
    if degrees == 0.0 {
        return c;
    }
    let (h, s, v) = rgb_to_hsv(c);
    hsv_to_rgb((h + degrees).rem_euclid(360.0), s, v, c.a)
}

fn rgb_to_hsv(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == c.r {
        60.0 * (((c.g - c.b) / d).rem_euclid(6.0))
    } else if max == c.g {
        60.0 * ((c.b - c.r) / d + 2.0)
    } else {
        60.0 * ((c.r - c.g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32, a: f32) -> Color {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::new(r + m, g + m, b + m, a)
}

/// The colour each biome has in a neutral sector: the table every sector's
/// palette is a rotation of.
fn biome_reference_tint(biome: Biome) -> Color {
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
        Biome::Deadlock => Color::new(0.70, 0.92, 0.95, 1.0),
        Biome::OpenGrid => Color::new(0.35, 0.85, 0.60, 1.0),
        Biome::NullSector => Color::new(0.20, 0.50, 0.52, 1.0),
        // Excavated: carved but not floored — brighter than Platform (the
        // laid, finished ground) so a mined-but-unbuilt patch of the base
        // still reads as less "done" than a tile with a machine on it,
        // without leaving the cool family passability depends on.
        Biome::Excavated => Color::new(0.15, 0.22, 0.42, 1.0),
        // Entropy: solid, unmined base space — a hole in the map exactly
        // like DataVoid and BlackIce, so it takes the same hot family. Kept
        // close to black rather than their amber/red brightness: this is
        // what "you have not dug here yet" looks like, not a hazard.
        Biome::Entropy => Color::new(0.10, 0.04, 0.04, 1.0),
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
        Biome::Deadlock => draw_speckle(painter, r, ink, h),
        Biome::Platform => draw_slab(painter, r, ink),
        Biome::DataVoid => draw_depth(painter, r, ink),
        Biome::BlackIce => draw_shards(painter, r, ink, h),
        // Rough, unfinished ground — the speckle Deadlock also wears, not
        // the clean laid lines of `draw_slab`: this tile is specifically
        // *not* floored yet.
        Biome::Excavated => draw_speckle(painter, r, ink, h),
        // A hole in the map, drawn the same way the surface's other two
        // holes are.
        Biome::Entropy => draw_depth(painter, r, ink),
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

/// Deadlock: three specks at hashed offsets. Noise, so it wants no
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
/// The map screen and everything ambient around it. `status` is the
/// player's last refusal, and it is `Some` only when this *is* the screen —
/// every mode that draws a popup over the map shows the refusal inside that
/// popup instead, and drawing it here as well would put the same sentence
/// on screen twice.
pub(super) fn draw_playing_base(
    app: &mut App,
    fx: &mut Fx,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (tile_px, glyph_px) = map_cell(app.zoom);
    let status_line = refusal.map(str::to_string);
    // Read before the `game` borrow, like `status_line` above.
    let stack_zoom = app.stack_zoom;
    // The pane's rows, chosen by app-core (see `pane_rows`), and the header
    // that says which channel they are. Both read before the `game` borrow.
    let log_h = painter.screen_h() - painter.screen_h() * PANE_H;
    let log_capacity = ((log_h - m.line_height) / m.line_height).max(1.0) as usize;
    let log_lines = app.visible_log(log_capacity.saturating_sub(1));
    let log_header = log_pane_header(app.log_filter, app.filtered_out_log_lines());
    // Before the `game` borrow, like `status_line` above. `None` outside
    // `Mode::Excavate`, which is what keeps the cursor off the map the rest
    // of the time without the renderer having to know the mode's rules.
    let plan = app
        .excavate_cursor
        .filter(|_| app.mode == Mode::Excavate)
        .map(|cursor| PlanCursor {
            cursor,
            anchor: app.excavate_anchor,
        });
    let Some(game) = &mut app.game else { return };

    // The stock strip claims a row off the top of the window and every pane
    // below it starts clear of it. Taken out of the map's height rather than
    // added to the window's, so the log pane below keeps the position it has
    // always had.
    let strip_h = stock::strip_height(m);
    let map_w = painter.screen_w() * PANE_W;
    let map_h = painter.screen_h() * PANE_H - strip_h;
    let stock_rows = game.base_stock();

    let status = game.player_status();
    // `Game::active_buffs` needs `&mut self`; fetched here rather than
    // inside `draw_status_panel`, which only ever needed `&Game` before
    // this and shouldn't have to start borrowing mutably just to draw.
    let buffs = game.active_buffs();
    let map_pane = Rect::new(0.0, strip_h, map_w, map_h);
    if let Some(view) = game.stack_view() {
        draw_stack(&view, painter, map_pane, m);
        // Over the corridor, not part of it: the same map the `g` screen
        // draws, small enough to leave the view readable.
        if let Some(map) = game.frame_map() {
            draw_map_inset(&map, stack_zoom, painter, map_pane, m);
        }
    } else {
        draw_surface_map(
            game, fx, painter, map_pane, tile_px, glyph_px, &status, plan,
        );
    }

    draw_status_panel(
        Rect::new(map_w, strip_h, painter.screen_w() - map_w, map_h),
        &status,
        &buffs,
        game,
        painter,
        m,
    );
    stock::draw_stock_strip(&stock_rows, painter, m);

    let log_y = strip_h + map_h;
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
        draw_message_line(e, m.inset, ly, painter, m);
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
pub(super) fn draw_history(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let entries = game.message_history(MESSAGE_LOG_CAP);
    let mut rows = history_rows(&entries, selected);
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "The last {MESSAGE_LOG_CAP} lines, repeats folded. A finished intrusion keeps its results, not its blow-by-blow."
    )));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("History", PopupSize::Large, &rows, refusal, painter, m);
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
    pane: Rect,
    tile_px: f32,
    glyph_px: u16,
    status: &feral_processes_engine::PlayerStatus,
    plan: Option<PlanCursor>,
) {
    // Two rings wider than the pane can show. The first is the tile the
    // camera's sub-tile offset slides in from, without which the trailing
    // edge goes blank; the second exists so *that* tile has neighbours of its
    // own to compare biomes with, since a tile cannot know whether it sits on
    // the edge of the walkable world without seeing what is beside it. Every
    // grid-to-world conversion below goes through `hw`/`hh` for that reason —
    // the rings shift the whole grid by two cells.
    let half_w = ((pane.w / tile_px) / 2.0).max(1.0) as i32;
    let half_h = ((pane.h / tile_px) / 2.0).max(1.0) as i32;
    let hw = half_w + RINGS;
    let hh = half_h + RINGS;

    // `status.position` is the player's `Position` component, which stays
    // pinned to the anchor tile on the zone surface while the party is out
    // of phase — see `resources::Locale`'s own doc for why. `view_tiles`
    // and `view_entities` both centre on `Game::base_pos` in that locale
    // instead, so this pane has to agree with them or a base's structures
    // draw offset from its own floor by however far base space has been
    // walked from its origin. `base_pos` is `None` everywhere but base
    // space, which is exactly when the pinned tile is already the right
    // answer.
    let base_pos = game.base_pos();
    let center = base_pos.unwrap_or(status.position);
    // Every `VisualEffect` the engine queues names a *structure's* tile —
    // all three `Game::push_effect` callers are structure damage — so the
    // whole queue is base-space by construction. This pane draws one space
    // at a time, so on the surface those coordinates would land on
    // unrelated open ground: the same cross-space aliasing `view_entities`
    // and the spawn-point outline below both refuse, and the structure
    // being flashed is not even drawn there to explain it.
    //
    // Suppressed rather than moved to the anchor: a raid already reaches a
    // player who is out of the base through the log pane's own flash and a
    // `MessageKind::Raid` line, and neither of those claims a tile.
    let show_effects = base_pos.is_some();
    let (off_x, off_y) = fx.camera_offset(center, painter.delta());
    let tiles = game.view_tiles(hw, hh);
    let entities: Vec<_> = game
        .view_entities(hw, hh)
        .into_iter()
        // A tamed program is drawn while it is out on an errand and while it
        // is loitering with no job at all. At its post it sits under its
        // machine's own glyph, so a base at rest reads as buildings and
        // motion is the only thing that draws the eye — a worker appearing
        // *is* the news that it has left to deliver. A guard and a party
        // member stay hidden for a harder reason: neither is ever walked,
        // so each keeps whatever tile it was standing on when it took the
        // job — out on the surface, or four frames down — and drawing it
        // would claim it is somewhere it isn't.
        //
        // Which *space* a program stands in is a separate question and is
        // already settled inside `view_entities`
        // (`Game::stands_in_base_space`); this one is only ever asked about
        // programs the party is in the same space as.
        //
        // Through the engine's predicate rather than spelled out here,
        // because `Game::find_target_in_direction` filters its ray with the
        // same rule so that `x` can only name what this draws.
        .filter(|e| drawn_on_surface_map(e.is_tamed, e.position_is_honest))
        .collect();
    // Base space only: `marked_cells` answers in base-space coordinates, and
    // asking it on the surface would draw a plan over the zone map at
    // coordinates that mean something else entirely — the cross-space read
    // 0.13.0 shipped two fixes for.
    let marked = if base_pos.is_some() {
        game.marked_cells()
    } else {
        Vec::new()
    };
    let spawn_point = game.zone_spawn_point();
    let shield_outline = fx.shield_outline(game.raid_defense_active());
    // Read once for the whole map: the sector is a property of the zone, so
    // asking per tile would be the same answer several thousand times.
    let hues = game.sector_hues();

    painter.rect(
        pane.x,
        pane.y,
        pane.w,
        pane.h,
        Color::new(0.03, 0.03, 0.05, 1.0),
    );
    for (ry, row) in tiles.iter().enumerate() {
        for (rx, tile) in row.iter().enumerate() {
            // An exposed rock face is brighter than the hole it is part of,
            // and *only* brighter: scaled before `biome_tint`'s hue
            // rotation, so a seam stays inside the impassable band under
            // every sector palette. Hue is already spoken for by
            // passability, which is why a kind cannot author one.
            let biome_color = match tile.rock_shade {
                Some(shade) => brighten(biome_tint(tile.biome, hues), shade),
                None => biome_tint(tile.biome, hues),
            };
            // Terrain no longer carries a glyph — the biome is drawn as
            // geometry — so this stays `None` unless something is standing
            // here. That is the whole division of labour on this map:
            // terrain is shapes, actors are glyphs.
            let mut ch = None;
            let mut color = biome_color;
            let mut bg_source = biome_color;
            let world = (center.0 + rx as i32 - hw, center.1 + ry as i32 - hh);
            let (px, py) = tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            );
            // The fetched rings exist to be *read* — by the camera slide and
            // by `draw_tile_edges` — not to be drawn. Nothing clips this pane,
            // and the log panel below it is drawn at 0.95 alpha, so a row
            // sitting past the bottom edge shows through it as a band of
            // terrain behind the text. Culling here rather than shrinking the
            // grid keeps every visible tile's neighbours in hand.
            if px >= pane.x + pane.w
                || py >= pane.y + pane.h
                || px + tile_px <= pane.x
                || py + tile_px <= pane.y
            {
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
                let erx = ev.pos.0 - center.0 + hw;
                let ery = ev.pos.1 - center.1 + hh;
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
                px + tile_px / 2.0 - (pane.x + pane.w / 2.0),
                py + tile_px / 2.0 - (pane.y + pane.h / 2.0),
                pane.w / 2.0,
                pane.h / 2.0,
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
            // usually spoken for: a hostile is tinted by `difficulty_color`
            // (green through red by power ratio against the player), which
            // is the "can I win this fight" read. That read is not sacred,
            // though — a nemesis spends it on purpose (see the mark just
            // below and `EntityView::rarity`'s doc), so "cannot be given
            // up" is no longer true of it. It just doesn't happen to be
            // rarity that spends it. Two readings, two channels — the glyph
            // says how dangerous (or, for a nemesis, that you already know),
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
            // A nemesis draws a second mark on top of its reserved glyph
            // colour — belt and braces, since a nemesis is worth noticing
            // even at a glance that only catches shape and not hue. Its own
            // corner rather than sharing the bar's, so a nemesis that is
            // also rare (the two are independent) shows both without either
            // being spent to make room.
            if actor.is_some_and(|ev| ev.nemesis) {
                let mark = nemesis_mark_rect(px, py, tile_px);
                painter.rect(
                    mark.x,
                    mark.y,
                    mark.w,
                    mark.h,
                    Color::new(CYAN.r * vig, CYAN.g * vig, CYAN.b * vig, CYAN.a),
                );
            }
            // Marks where the player materialized on breaching into this
            // zone (see `Game::zone_spawn_point`) — an outline rather than
            // replacing the glyph, so whatever's actually standing there
            // (the player, a creature, a rebuilt structure) still reads
            // clearly on top of it. `spawn_point` is a surface coordinate,
            // so this only means anything while the pane is drawing the
            // surface — comparing it against a base-space `center` would be
            // the same cross-space aliasing `view_entities` refuses now.
            if base_pos.is_none() {
                let spawn_rx = spawn_point.0 - center.0 + hw;
                let spawn_ry = spawn_point.1 - center.1 + hh;
                if rx as i32 == spawn_rx && ry as i32 == spawn_ry {
                    painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, MAGENTA);
                }
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
            if show_effects && let Some(flash) = fx.tile_flash(world) {
                painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
            }
        }
    }
    // Over the tiles and under the sparks: a plan is a thing drawn on the
    // ground, and a burst is a thing happening above it.
    if base_pos.is_some() {
        draw_excavation_plan(
            painter,
            &marked,
            plan,
            |world| {
                tile_origin_px(
                    world,
                    center,
                    (half_w, half_h),
                    (off_x, off_y),
                    tile_px,
                    pane,
                )
            },
            tile_px,
            pane,
        );
    }
    // After every tile so debris lands on top of the base rather than under
    // it, and before the border so a spark from a structure at the pane's
    // edge cannot draw over the frame.
    if show_effects {
        fx.draw_bursts(painter, tile_px, |world| {
            tile_origin_px(
                world,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        });
    }
    painter.rect_lines(pane.x, pane.y, pane.w, pane.h, 2.0, BORDER);
}

/// What the Excavation plan is drawing over the base map: where the cursor
/// is, and the anchor it has dropped, both in base-space coordinates.
///
/// Read off `App` before the `game` borrow, the same way `status_line` and
/// `stack_zoom` are, and `None` outside `Mode::Excavate`. The *marks* are not
/// in here: a plan the player drew has to stay visible while they walk it, so
/// those are drawn whenever the pane is showing base space.
#[derive(Clone, Copy)]
pub(super) struct PlanCursor {
    pub cursor: (i32, i32),
    pub anchor: Option<(i32, i32)>,
}

/// The marks, the box being previewed, and the cursor — one pass over world
/// coordinates after the tile loop, the same shape the spark pass takes.
///
/// A pass of its own rather than three more branches inside the tile loop
/// because none of it is a property of a *tile*: a mark is an entity in base
/// space, and the box is a rectangle that happens to cross tiles. Culling is
/// the pane's, so a box drawn past the edge is simply not painted.
#[allow(clippy::too_many_arguments)]
fn draw_excavation_plan(
    painter: &Painter,
    marked: &[(i32, i32)],
    plan: Option<PlanCursor>,
    at: impl Fn((i32, i32)) -> (f32, f32),
    tile_px: f32,
    pane: Rect,
) {
    let size = tile_px - 1.0;
    let tile = |world: (i32, i32), fill: Option<Color>, outline: Option<(f32, Color)>| {
        let (px, py) = at(world);
        if px >= pane.x + pane.w
            || py >= pane.y + pane.h
            || px + tile_px <= pane.x
            || py + tile_px <= pane.y
        {
            return;
        }
        if let Some(fill) = fill {
            painter.rect(px, py, size, size, fill);
        }
        if let Some((thickness, color)) = outline {
            painter.rect_lines(px, py, size, size, thickness, color);
        }
    };
    // The plan itself, drawn under the cursor: a wash rather than a glyph,
    // because the cell underneath is already saying whether it is rock, cut
    // or floor and the mark is a second reading on top of that one.
    for &cell in marked {
        tile(cell, Some(MARK_FILL), Some((1.0, MARK_EDGE)));
    }
    let Some(plan) = plan else { return };
    // The box the anchor is spanning, brighter than a committed mark: this
    // is the thing about to happen, and it has to read over the marks it may
    // be drawn across.
    if let Some(anchor) = plan.anchor {
        let (x0, x1) = (anchor.0.min(plan.cursor.0), anchor.0.max(plan.cursor.0));
        let (y0, y1) = (anchor.1.min(plan.cursor.1), anchor.1.max(plan.cursor.1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                tile((x, y), Some(PREVIEW_FILL), None);
            }
        }
    }
    tile(plan.cursor, None, Some((2.0, WHITE)));
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
    pane: Rect,
) -> (f32, f32) {
    (
        pane.x + ((world.0 - player.0 + half.0) as f32 - off.0) * tile_px,
        pane.y + ((world.1 - player.1 + half.1) as f32 - off.1) * tile_px,
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
        // `Unpowered` joins them for the same reason — a dark machine never
        // resolves itself either, only a Recharger Node fixes it.
        MachineStatus::Clogged | MachineStatus::Stranded | MachineStatus::Unpowered => RED,
        MachineStatus::Idle => TEXT_DIM,
    }
}

/// The player's own figures in the status column: what they hit for, and
/// what they take.
///
/// Built here rather than inline so their width is measurable without a
/// window — see `the_stat_lines_fit_the_status_column`. The column cannot
/// grow horizontally and `Painter::ui` clips nothing, so a line too wide is
/// drawn off the panel in silence.
///
/// `Mitigation`, not `Defense`, and with the percent sign: it is percentage
/// points (`components::Stats::mitigation`), the manifest sheet has always
/// called it that, and a bare `Defense 12` beside a `Mitigation 12%` on the
/// next screen is two words for one number.
///
/// **The regrouping is what pays for the longer word.** `Attack 1234
/// Mitigation 75%  Strength 1234` runs 38px past the column at its widest,
/// so the three figures no longer share a line and `Decompiler` — which had
/// a line to itself — takes the second one in. A fifth line was the other
/// way out and is worse: the column clips vertically against the keybind
/// footer, so a row added here is a row taken off the buff and inventory
/// lists below it.
fn stat_lines(atk: i32, mitigation: i32, strength: i32, decompiler: i32) -> [String; 2] {
    [
        format!("Attack {atk}  Strength {strength}"),
        format!("Mitigation {mitigation}%  Decompiler {decompiler}"),
    ]
}

/// One party member's line in the status column, indented under the
/// `Party: n/m` heading it belongs to.
///
/// The `w|a|m` loadout cell trails the stats rather than leading them: the
/// panel's job while you are walking is the numbers, and the cell is here so
/// an unequipped member is noticeable without opening the roster — see
/// `Game::gear_tag`, which is where both screens get it from.
fn party_row(companion: &feral_processes_engine::CompanionInfo) -> String {
    format!(
        "  {} (HP {}/{}, PWR {}) {}",
        companion.name, companion.hp, companion.max_hp, companion.power, companion.gear
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
        &format!("Power {:.0}/100", status.power),
        status.power,
        100.0,
        BarStyle::plain(YELLOW),
        painter,
        m,
    );
    cy += m.gap;

    let mut lines = vec![
        format!(
            "Level {}  (XP {}/{})  Perk Pts {}",
            status.level, status.xp, status.xp_to_next, status.perk_points
        ),
        format!("Zone {}", status.zone),
        {
            // The pinned surface tile, same as `status.position` always was,
            // except in base space — where the number that has any meaning
            // to a player walking around inside it is `Game::base_pos`, not
            // the anchor tile they stepped through to get there.
            let (x, y) = game.base_pos().unwrap_or(status.position);
            format!("Position: ({x}, {y})")
        },
    ];
    lines.extend(stat_lines(
        status.atk,
        status.mitigation,
        status.strength,
        status.decompiler,
    ));
    for line in &lines {
        painter.ui(line, x + m.inset, cy, m.font_size, TEXT);
        cy += m.line_height;
    }
    // Base space only: out on the surface there is no rock to cut, so the
    // row would be a mode readout for a mode that cannot fire. Eleven cells
    // at its widest, well inside the column's ceiling — the status column
    // cannot grow horizontally and an over-wide row is drawn off the panel
    // in silence.
    if game.in_base() {
        let armed = game.mining();
        painter.ui(
            format!("Mining: {}", if armed { "on" } else { "off" }),
            x + m.inset,
            cy,
            m.font_size,
            if armed { GREEN } else { TEXT_DIM },
        );
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
        "b base menu   p party menu   i pack   n mine",
        "c collect  t trade  a routine  u symlink  x examine  v tile",
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
            format!(
                "{} {}{tier}",
                qty_column(row.qty),
                game.copy_name(&row.copy)
            ),
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
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::MessageSource;

    /// Every `Biome` variant, listed the way `species.rs`'s own biome census
    /// lists them. A new variant missing from here makes the tint census
    /// below pass vacuously, which is the failure mode that census exists to
    /// prevent — but `biome_tint`'s match is exhaustive, so a new biome
    /// cannot reach a test run without someone having already been sent to
    /// this file by the compiler.
    const ALL_BIOMES: [Biome; 9] = [
        Biome::DataVoid,
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Mainframe,
        Biome::OpenGrid,
        Biome::BlackIce,
        Biome::Platform,
        Biome::Excavated,
        Biome::Entropy,
    ];

    /// Whether a tint reads as hostile: red dominant over both other
    /// channels. The map's whole colour rule is that hue answers "can I
    /// cross this", so this is the question the census below asks.
    fn reads_as_hostile(c: Color) -> bool {
        c.r > c.g && c.r > c.b
    }

    /// The neutral pair, which every non-sector screen and every zone 1 draws
    /// with.
    const NEUTRAL: (f32, f32) = (
        feral_processes_engine::sectors::NEUTRAL_GROUND_HUE,
        feral_processes_engine::sectors::NEUTRAL_HAZARD_HUE,
    );

    /// Every hue pair a sector is allowed to author, at one-degree steps.
    ///
    /// The whole band rather than only the hue pairs `assets/sectors/`
    /// happens to ship, because the bounds in `sectors::GROUND_HUE_BAND`
    /// claim a sweep like this exists — and because the failure being
    /// guarded against is a *mod* authoring a legal-but-unreadable palette,
    /// which a census over the shipped three could never see.
    fn every_legal_hue_pair() -> impl Iterator<Item = (f32, f32)> {
        let (glo, ghi) = feral_processes_engine::sectors::GROUND_HUE_BAND;
        let (hlo, hhi) = feral_processes_engine::sectors::HAZARD_HUE_BAND;
        (glo as i32..=ghi as i32)
            .flat_map(move |g| (hlo as i32..=hhi as i32).map(move |h| (g as f32, h as f32)))
    }

    /// The palette's one load-bearing promise: hue tells the player whether
    /// terrain can be walked on, and pattern tells them which biome it is.
    /// A biome tinted into the wrong family is worse than a drawing bug —
    /// it tells the player they can walk into the void.
    ///
    /// This is the gate the whole band-swap design exists to satisfy. A free
    /// per-biome palette could not have one, which is why a sector authors
    /// two numbers inside stated bounds rather than seven colours.
    #[test]
    fn every_biomes_tint_says_whether_it_can_be_walked_on_in_every_legal_sector() {
        for hues in every_legal_hue_pair() {
            for biome in ALL_BIOMES {
                assert_eq!(
                    reads_as_hostile(biome_tint(biome, hues)),
                    !biome.walkable(),
                    "at hues {hues:?}, {biome:?} is walkable={} but its tint {:?} reads \
                     the other way — hue is the map's only signal for passability",
                    biome.walkable(),
                    biome_tint(biome, hues),
                );
            }
        }
    }

    /// What separates the five walkable biomes from each other is brightness,
    /// not hue — hue is spoken for by the rule above. So a sector may move
    /// the band but must not disturb the order inside it, and Platform in
    /// particular has to stay much the darkest: it is the only biome the
    /// player lays, it covers whole screens wherever a base stands, and that
    /// number was taken down twice after being seen on screen behind a full
    /// base.
    #[test]
    fn a_sector_never_reorders_the_walkable_biomes_by_brightness() {
        let value = |c: Color| c.r.max(c.g).max(c.b);
        let walkable: Vec<Biome> = ALL_BIOMES.into_iter().filter(|b| b.walkable()).collect();
        let mut expected: Vec<Biome> = walkable.clone();
        expected.sort_by(|a, b| {
            value(biome_tint(*a, NEUTRAL))
                .partial_cmp(&value(biome_tint(*b, NEUTRAL)))
                .unwrap()
        });
        assert_eq!(
            expected.first(),
            Some(&Biome::Platform),
            "Platform must be the darkest walkable biome"
        );

        for hues in every_legal_hue_pair() {
            let mut got = walkable.clone();
            got.sort_by(|a, b| {
                value(biome_tint(*a, hues))
                    .partial_cmp(&value(biome_tint(*b, hues)))
                    .unwrap()
            });
            assert_eq!(got, expected, "hues {hues:?} reordered the walkable biomes");
        }
    }

    /// Deleting `assets/sectors/` restores today's game **exactly**, not
    /// approximately: the neutral pair is a zero rotation, and a zero
    /// rotation is not applied at all. Asserted against the literals rather
    /// than against a recomputation, so a drift in either direction fails.
    #[test]
    fn the_neutral_hues_reproduce_the_shipped_table_exactly() {
        let table = [
            (Biome::DataVoid, Color::new(0.95, 0.60, 0.15, 1.0)),
            (Biome::BlackIce, Color::new(0.95, 0.32, 0.18, 1.0)),
            (Biome::Platform, Color::new(0.06, 0.11, 0.32, 1.0)),
            (Biome::Mainframe, Color::new(0.25, 0.85, 0.85, 1.0)),
            (Biome::Deadlock, Color::new(0.70, 0.92, 0.95, 1.0)),
            (Biome::OpenGrid, Color::new(0.35, 0.85, 0.60, 1.0)),
            (Biome::NullSector, Color::new(0.20, 0.50, 0.52, 1.0)),
            (Biome::Excavated, Color::new(0.15, 0.22, 0.42, 1.0)),
            (Biome::Entropy, Color::new(0.10, 0.04, 0.04, 1.0)),
        ];
        for (biome, expected) in table {
            let got = biome_tint(biome, NEUTRAL);
            assert_eq!(
                (got.r, got.g, got.b, got.a),
                (expected.r, expected.g, expected.b, expected.a),
                "{biome:?} is not what it was before sectors existed"
            );
        }
    }

    /// The five walkable biomes must stay telling-apart-able in every
    /// sector, and brightness alone does not do it: Mainframe and OpenGrid
    /// have *identical* value (0.85), so hue is the only thing separating
    /// them. A palette that set every walkable biome to one hue — which is
    /// what replacing H rather than rotating it does — would leave those two
    /// as two near-identical cyans differing only in saturation.
    ///
    /// Distinct hues rather than a distance threshold because that is the
    /// mechanism: a rotation moves all five by the same amount, so it
    /// preserves distinctness exactly, and nothing else does.
    #[test]
    fn no_sector_collapses_two_walkable_biomes_onto_one_hue() {
        let walkable: Vec<Biome> = ALL_BIOMES.into_iter().filter(|b| b.walkable()).collect();
        for hues in every_legal_hue_pair() {
            for (i, a) in walkable.iter().enumerate() {
                for b in &walkable[i + 1..] {
                    let (ha, _, _) = rgb_to_hsv(biome_tint(*a, hues));
                    let (hb, _, _) = rgb_to_hsv(biome_tint(*b, hues));
                    assert!(
                        (ha - hb).abs() > 1.0,
                        "at hues {hues:?}, {a:?} and {b:?} are both at hue {ha} — \
                         a sector may move the band, not flatten it"
                    );
                }
            }
        }
    }

    /// The transform's actual job, tested where the zero-rotation
    /// short-circuit above cannot reach: a rotation moves hue and leaves
    /// saturation and value alone. A round trip that clamped or lost
    /// precision would show up here as a drifting S or V, which is exactly
    /// what would flatten the brightness spread the test above depends on.
    #[test]
    fn a_rotation_moves_only_hue() {
        for hues in every_legal_hue_pair() {
            for biome in ALL_BIOMES {
                let (_, s0, v0) = rgb_to_hsv(biome_tint(biome, NEUTRAL));
                let (_, s1, v1) = rgb_to_hsv(biome_tint(biome, hues));
                assert!(
                    (s0 - s1).abs() < 1e-4 && (v0 - v1).abs() < 1e-4,
                    "at hues {hues:?}, {biome:?} moved from S={s0} V={v0} to S={s1} V={v1}"
                );
            }
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
            mitigation: 5,
            power: 41,
            status: None,
            ability: "Rally".to_string(),
            gear: "w|.|.".to_string(),
        }
    }

    /// One word and one unit for the stat, across every screen that names
    /// it: the manifest sheet and the two pickers say `Mitigation` /
    /// `MIT %`, and this column said `Defense 12` for the same number.
    #[test]
    fn the_stat_lines_name_mitigation_in_the_unit_it_is_measured_in() {
        let joined = stat_lines(9, 12, 44, 3).join("\n");
        assert!(joined.contains("Mitigation 12%"), "{joined}");
        assert!(
            !joined.contains("Defense"),
            "one word for one number: {joined}"
        );
    }

    /// The regrouping may not lose a figure: `Decompiler` had a line of its
    /// own before it took the second half of the mitigation line.
    #[test]
    fn the_stat_lines_still_carry_all_four_figures() {
        let joined = stat_lines(9, 12, 44, 3).join("\n");
        for figure in ["Attack 9", "Mitigation 12%", "Strength 44", "Decompiler 3"] {
            assert!(joined.contains(figure), "lost {figure}:\n{joined}");
        }
    }

    /// The status column cannot grow horizontally and `Painter::ui` clips
    /// nothing, so an over-wide line is drawn off the panel in silence —
    /// which is exactly what a rename that lengthens a label risks. The
    /// three-figure line this replaced overflowed by 38px at these values.
    ///
    /// Nothing caps attack, the strength scalar or the decompiler, so all
    /// three take four digits here; mitigation is capped by
    /// `Game::effective_mitigation` at `MAX_MITIGATION_PERCENT`, so that
    /// constant is its widest reading and not a guess.
    #[test]
    fn the_stat_lines_fit_the_status_column() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // The status panel is the window's width less the map pane's
            // `PANE_W`, drawn one inset in, against the 1440x900 geometry
            // `ui_metrics` is calibrated for.
            let room = 1440.0 * (1.0 - PANE_W) - m.inset * 2.0;
            for line in stat_lines(
                1234,
                feral_processes_engine::tuning::MAX_MITIGATION_PERCENT,
                1234,
                1234,
            ) {
                let drawn = p.measure_ui_advance(line.clone(), m.font_size);
                assert!(
                    drawn <= room,
                    "a stat line overflows the status column by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The panel is the only companion list on screen while you are walking
    /// around, so it is where a player notices a party member is still
    /// wearing nothing without stopping to open the roster.
    #[test]
    fn a_party_row_shows_which_gear_slots_are_filled() {
        let row = party_row(&companion("Sparkgrub"));
        assert!(row.contains("w|.|."), "{row}");
        assert!(
            row.find("PWR").unwrap() < row.find("w|.|.").unwrap(),
            "the cell trails the stats it annotates: {row}"
        );
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

    /// The tile mark's whole job is not fighting the rarity bar for the
    /// same pixels — see the block comment above `nemesis_mark_rect`'s call
    /// site. `RARITY_BAR_PX` is the bar's full-width strip along the top;
    /// the mark must start at or below it.
    #[test]
    fn the_nemesis_mark_never_overlaps_the_rarity_bar() {
        for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
            let mark = nemesis_mark_rect(100.0, 200.0, tile_px);
            assert!(
                mark.y >= 200.0 + RARITY_BAR_PX,
                "at tile_px={tile_px}, mark.y={} starts above the rarity bar's \
                 {} px strip",
                mark.y,
                RARITY_BAR_PX
            );
        }
    }

    /// Inset from every edge the way `STAFFED_MARK_INSET` is, and for the
    /// same reason: flush against an edge, it would read as painting back
    /// in a wall `outline_open` deliberately left off a chained structure.
    /// Creatures are never chained, but the mark's geometry shouldn't rely
    /// on that to stay clear.
    #[test]
    fn the_nemesis_mark_stays_inside_the_tile_and_off_every_edge() {
        let (px, py, tile_px) = (50.0_f32, 60.0_f32, 40.0_f32);
        let mark = nemesis_mark_rect(px, py, tile_px);

        assert!(mark.x > px, "mark's left edge touches the tile's left edge");
        assert!(
            mark.x + mark.w < px + tile_px - 1.0,
            "mark's right edge touches or crosses the tile's right edge"
        );
        assert!(
            mark.y + mark.h < py + tile_px - 1.0,
            "mark's bottom edge touches or crosses the tile's bottom edge"
        );
        assert!(mark.w > 0.0 && mark.h > 0.0, "the mark must have real size");
    }

    /// Opposite corners: `STAFFED_MARK` sits bottom-left, this sits
    /// top-right (and below the bar). A nemesis that is also a staffed
    /// program's target — unreachable in play, since a nemesis is wild and
    /// a staffed mark is tamed, but the geometry itself should not depend
    /// on that being true — still can't have the two marks land on the
    /// same pixels.
    #[test]
    fn the_nemesis_mark_sits_in_the_opposite_corner_from_the_staffed_mark() {
        let tile_px = 40.0_f32;
        let (px, py) = (0.0, 0.0);
        let nemesis = nemesis_mark_rect(px, py, tile_px);
        let staffed_size = (tile_px - 1.0) * STAFFED_MARK;
        let staffed_y = py + tile_px - 1.0 - STAFFED_MARK_INSET - staffed_size;

        assert!(
            nemesis.y + nemesis.h < staffed_y,
            "the nemesis mark's bottom ({}) reaches into the staffed mark's \
             row (starts at {staffed_y})",
            nemesis.y + nemesis.h
        );
    }
}
