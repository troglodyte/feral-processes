//! Biome and terrain painting primitives: the colour, pattern and depth a
//! tile is drawn with, independent of what drives the map.

use super::*;
use feral_processes_engine::components::POWER_MAX;

/// How far a bare tile's background may stray from its biome's flat colour,
/// as a fraction either side. Enough to break up a field of identical tiles,
/// not enough to read as two different biomes.
pub(super) const SHADE_JITTER: f32 = 0.08;
/// How dark the map pane's corners get relative to its centre, at a full
/// Power reserve and at an empty one. Floored well short of illegible: the
/// vignette is depth, and must never be the reason a hostile at the pane's
/// edge goes unnoticed.
///
/// **That rule is why the drained floor is a deepening and not a blackout.**
/// The glyph takes the vignette (see its call site below), so every step
/// down here dims a hostile standing at the pane's edge — and an empty
/// reserve is precisely when the player can least afford to miss one. The
/// gap between the two is meant to be felt at a glance and read through
/// regardless. These are the numbers that decide that legibility on the
/// whole map — `CLOUD_DEPTH` does not, since a shadow never touches a
/// glyph — so a request to darken the map is answered here last and by the
/// smallest step that reads.
pub(super) const VIGNETTE_FLOOR_FULL: f32 = 0.68;
pub(super) const VIGNETTE_FLOOR_EMPTY: f32 = 0.52;

/// A tile's own brightness multiplier, so a field of one biome reads as
/// ground rather than as a flat colour swatch.
///
/// Hashed from the world coordinate, never the screen cell: the camera now
/// slides continuously across tiles, and a shade tied to screen position
/// would crawl over the terrain as it went. The two axes are mixed with
/// different constants because a symmetric hash bands the map along its
/// diagonal, which reads as a pattern instead of as texture.
pub(super) fn tile_shade(world: (i32, i32)) -> f32 {
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
pub(super) fn vignette(dx: f32, dy: f32, half_w_px: f32, half_h_px: f32, floor: f32) -> f32 {
    let r = ((dx / half_w_px).powi(2) + (dy / half_h_px).powi(2))
        .sqrt()
        .min(1.0);
    1.0 - (1.0 - floor) * r * r
}

/// How dark the corners are allowed to get, given the player's Power reserve.
///
/// The vignette is **always on** and what Power moves is its depth. One that
/// only appeared below some threshold would be news the first time and
/// unread for the rest of the run; one that is always there, and tightens,
/// is a gauge the player takes in without looking away from the map.
///
/// Deliberately not keyed to `tuning::LOW_POWER_ATTACK_THRESHOLD`'s knee.
/// That constant is where Power starts costing you damage, and a kink here
/// at the same place would claim the darkening and the penalty are one
/// statement — they are not, and the PWR meter is where a threshold reading
/// belongs.
///
/// `render/stack.rs`'s `fog` is this same idea turned down the corridor's own
/// axis, and the two are deliberately *not* a shared function: a floor on a
/// radial falloff and a per-cell fog rate are different quantities that
/// happen to be driven by the same reserve.
pub(super) fn vignette_floor(power: f32) -> f32 {
    let fraction = (power / POWER_MAX).clamp(0.0, 1.0);
    VIGNETTE_FLOOR_EMPTY + (VIGNETTE_FLOOR_FULL - VIGNETTE_FLOOR_EMPTY) * fraction
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
/// This used to take a `(ground, hazard)` hue pair and rotate the table
/// below by however far a zone's sector had moved its band — the world is
/// persistent now, there is one palette for the whole run, and sectors are
/// retired, so there is nothing left to rotate. `biome_tint` stays the named
/// door onto this table rather than folding into `biome_reference_tint`
/// because it is the one CLAUDE.md cites as the map's colour rule, and
/// because the exhaustive match belongs to a function whose whole job is
/// answering "what colour is this biome" — a caller should never have to
/// know the reference table is a second function away.
pub(super) fn biome_tint(biome: Biome) -> Color {
    biome_reference_tint(biome)
}

/// `c` scaled toward white by `factor`, hue untouched.
///
/// Clamped at 1.0 so a channel cannot wrap, and applied to the *biome's own*
/// colour rather than to a fixed value, so a rock face brightens whatever
/// colour the hole around it is drawn in.
pub(super) fn brighten(c: Color, factor: f32) -> Color {
    Color::new(
        (c.r * factor).min(1.0),
        (c.g * factor).min(1.0),
        (c.b * factor).min(1.0),
        c.a,
    )
}

/// The colour each biome has. Was "the colour each biome has in a neutral
/// sector, the table every sector's palette is a rotation of" — sectors are
/// retired and there is one palette for the whole run, so this table is now
/// the whole of `biome_tint` rather than what it rotated.
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
        Biome::Backplane => Color::new(0.25, 0.85, 0.85, 1.0),
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
pub(super) fn rim(a: Biome, b: Biome) -> bool {
    a.walkable() != b.walkable()
}

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

/// `c` scaled toward black by `level`, alpha untouched. Every terrain colour
/// on the map is this function applied to `biome_tint`, which is what keeps
/// ground, pattern and rim reading as three depths of one material instead
/// of three colours that happen to sit together.
pub(super) fn at_level(c: Color, level: f32) -> Color {
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
pub(super) fn draw_biome(painter: &Painter, biome: Biome, r: Rect, tint: Color, world: (i32, i32)) {
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
        Biome::Backplane => draw_traces(painter, r, ink, h),
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

/// Backplane: a circuit trace entering the tile and terminating in a pad.
/// Which side it enters from is hashed, so a field of Backplane reads as
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
/// `cloud` reaches the grid line and deliberately not the rim.
///
/// The rim is the edge of the walkable world and is the map's one glow —
/// see below — and a passing shadow has no business putting the edge of the
/// world out. The grid line is the substrate the ground is printed on, so
/// it goes under a cloud with the ground it belongs to. Left lit, it would
/// *gain* against ground dimmed by `CLOUD_DEPTH` — `GRID_LEVEL` is already
/// only a little under `GROUND_LEVEL` — and a mesh would surface inside
/// every shadow.
///
/// `vig` and `cloud` stay two arguments rather than one product for exactly
/// that reason: the rim takes only the first.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tile_edges(
    painter: &Painter,
    tiles: &[Vec<Tile>],
    rx: usize,
    ry: usize,
    cell: Rect,
    tint: Color,
    vig: f32,
    cloud: f32,
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
            painter.line(
                x1,
                y1,
                x2,
                y2,
                1.0,
                at_level(tint, GRID_LEVEL * vig * cloud),
            );
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
