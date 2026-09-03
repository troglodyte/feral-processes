//! The map screen: terrain, entities, effects, and the status panel beside them.

use super::stack::draw_stack;
use super::*;
use feral_processes_engine::components::POWER_MAX;
use feral_processes_engine::views::drawn_on_surface_map;

/// How far a bare tile's background may stray from its biome's flat colour,
/// as a fraction either side. Enough to break up a field of identical tiles,
/// not enough to read as two different biomes.
const SHADE_JITTER: f32 = 0.08;
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
const VIGNETTE_FLOOR_FULL: f32 = 0.68;
const VIGNETTE_FLOOR_EMPTY: f32 = 0.52;

/// The staffed mark's side, as a fraction of the tile, and how far it is held
/// off the tile's edges. The inset is not cosmetic: `outline_open` drops the
/// edges a chained pair shares, and a mark flush into the corner would read as
/// painting one of those absent lines back in.
const STAFFED_MARK: f32 = 0.28;
const STAFFED_MARK_INSET: f32 = 2.0;

/// What a Repair Bay wears while a program is recovering in it.
///
/// **A glyph and not a rect**, unlike the two marks a machine wears: those
/// say something about a *job* and share the corners with each other, while
/// this says a body is lying in this building. A cross is the one shape a
/// player already reads as that without being told, and it needs the middle
/// of the tile to be one.
///
/// It draws **over** the Bay's own `r` rather than beside it. The shipped Bay
/// authors `radius: 0`, which `offshift::in_reach` reads as *standing beside
/// it* — a structure's own tile is blocked, so no program ever stands on one
/// — and that is what leaves the middle of the Bay's tile free to be written
/// on. A Bay whose def widened its reach would still be drawn on: the mark
/// names the Bay, not the body.
const RECOVERY_MARK: char = '+';

/// A pending build site's slab and its edge.
///
/// **Grey, and deliberately colourless.** Every other channel on this map is
/// spoken for by a hue that means something — a biome's passability, a
/// machine's status outline, the plan's amber, the mark's green — and a site
/// is the one cell whose whole message is *nothing is here yet*. It reads as
/// an absence because it is one. The orange caret bouncing on top is what
/// carries "and somebody is working on it", so the slab does not have to.
///
/// The edge is darker than the fill rather than lighter, which is `rock`'s
/// `SHADE_BAND` rule in another shape: a bright rim would read as a
/// finished, lit structure. It is opaque so the ground beneath does not show
/// through and make the cell look half-drawn.
///
/// **The two greys stay outside `hud::palette`**, alone among the map's
/// overlays. The palette is addressed by role and has no word for
/// "unfinished construction"; the nearest entries are a pane border and a
/// bar trough, and reaching for one of those would be addressing a chrome
/// grey by its value, which is the one thing that file forbids.
const BUILD_SITE_FILL: Color = Color::new(0.30, 0.30, 0.32, 1.0);
const BUILD_SITE_EDGE: Color = Color::new(0.16, 0.16, 0.18, 1.0);
const BUILD_SITE_EDGE_PX: f32 = 2.0;

/// The Excavation plan's three washes, all one hue so a plan reads as one
/// thing. A committed mark's fill is dim enough to walk over without the
/// base becoming unreadable and its edge carries the shape; the box being
/// previewed is brighter than either, because it is the thing about to
/// happen and has to read over a mark it may be drawn across.
///
/// The hue is `palette::PLAN`, and the three are built off it rather than
/// written out, so the alpha is the only thing that varies — three literals
/// is three chances for a plan to stop being one colour.
const fn wash(alpha: f32) -> Color {
    Color::new(
        hud::palette::PLAN.r,
        hud::palette::PLAN.g,
        hud::palette::PLAN.b,
        alpha,
    )
}
const MARK_FILL: Color = wash(0.18);
const MARK_EDGE: Color = wash(0.45);
const PREVIEW_FILL: Color = wash(0.35);

/// The ring the party's own tile wears while cutting tools are armed.
///
/// The plan's hue at full alpha, because a mark and a swing are the same
/// job: what the tools are out *for* is the rock a plan is drawn on. Full
/// alpha rather than the washes above since this is a mode and not a thing
/// drawn on the ground — it has to read over whatever the party is standing
/// on, a marked cell included.
pub(crate) const CUTTING_OUTLINE: Color = hud::palette::PLAN;

/// An identity mark's side, as a fraction of the tile — the shape a
/// nemesis and a boss both wear, in opposite corners, and how far it sits
/// off the tile's edges. Smaller than `STAFFED_MARK` and placed in the
/// opposite corner (top-right rather than bottom-left), so a marked program
/// standing on a machine-adjacent tile can never collide with either a
/// staffed mark or the outline `outline_open` drops along a chained pair's
/// shared edge.
const IDENTITY_MARK: f32 = 0.22;
const IDENTITY_MARK_INSET: f32 = 2.0;

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
    let size = (tile_px - 1.0) * IDENTITY_MARK;
    Rect::new(
        px + tile_px - 1.0 - IDENTITY_MARK_INSET - size,
        py + RARITY_BAR_PX + IDENTITY_MARK_INSET,
        size,
        size,
    )
}

/// The con read's strip along the **bottom** edge — the rarity bar's mirror,
/// same thickness and same full width, so the tile is framed by its two
/// derived readings and the glyph between them is free to carry identity.
///
/// A free function for `nemesis_mark_rect`'s reason: the geometry is
/// unit-testable without a `Painter`, and `staffed_mark_rect` below has to
/// clear it without keeping a second copy of where it sits.
fn difficulty_bar_rect(px: f32, py: f32, tile_px: f32) -> Rect {
    Rect::new(
        px,
        py + tile_px - 1.0 - RARITY_BAR_PX,
        tile_px - 1.0,
        RARITY_BAR_PX,
    )
}

/// Where the boss mark sits — the bottom-right corner, raised clear of
/// `difficulty_bar_rect` the way `nemesis_mark_rect` drops below the rarity
/// bar, and inset from both remaining edges for that mark's reason.
///
/// **The far corner from the nemesis mark deliberately.** A creature can be
/// both now — `difficulty_color` used to answer with one reserved hue and
/// the nemesis won it, so being a boss went undrawn — and two facts that
/// can hold at once must never fight for pixels. The bottom edge is also
/// the friendlier neighbour by colour: the con rungs under it are all warm,
/// where the rarity bar's Prismatic sits much nearer this mark's magenta.
fn boss_mark_rect(px: f32, py: f32, tile_px: f32) -> Rect {
    let size = (tile_px - 1.0) * IDENTITY_MARK;
    Rect::new(
        px + tile_px - 1.0 - IDENTITY_MARK_INSET - size,
        py + tile_px - 1.0 - RARITY_BAR_PX - IDENTITY_MARK_INSET - size,
        size,
        size,
    )
}

/// The boss mark's colour — the magenta `difficulty_color` used to paint a
/// boss's whole glyph, kept so the fact reads the same after it moved off
/// the glyph and into a corner. A named function rather than a literal at
/// the draw site so the census that holds it apart from every other mark on
/// the tile has something to name.
fn boss_mark_color() -> Color {
    hud::palette::glyph(GlyphColor::Magenta)
}

/// Paints the con read along the bottom edge, or nothing at all when there
/// is no reading to paint.
///
/// Extracted the way `draw_recovery_mark` is: the map's tile loop is far too
/// big to reach with a test, and what has to be pinned here is that `None`
/// paints *nothing* — a bar under a companion would say the player can beat
/// their own program.
///
/// `vig` multiplies the way the rarity bar's does, so a tile darkened at the
/// edge of the light doesn't leave its two bars burning at full brightness.
fn draw_difficulty_bar(
    painter: &Painter,
    difficulty: Option<GlyphColor>,
    px: f32,
    py: f32,
    tile_px: f32,
    vig: f32,
) {
    let Some(rung) = difficulty else {
        return;
    };
    let c = glyph_color(rung);
    let bar = difficulty_bar_rect(px, py, tile_px);
    painter.rect(
        bar.x,
        bar.y,
        bar.w,
        bar.h,
        Color::new(c.r * vig, c.g * vig, c.b * vig, c.a),
    );
}

/// Where the "someone is on this job" mark sits, `lift` px up from its
/// resting place — `Fx::staffed_bob` while a machine is worked, zero at rest
/// and for a stranded mark, which blinks in place instead.
///
/// **Raised clear of `difficulty_bar_rect`**, the mirror of `nemesis_mark_
/// rect` dropping below `RARITY_BAR_PX`. Extracted rather than left inline
/// so the clearance is one expression: the test that pins it used to
/// hand-copy this arithmetic, which is exactly the copy that drifts when the
/// bottom edge gains a bar.
fn staffed_mark_rect(px: f32, py: f32, tile_px: f32, lift: f32) -> Rect {
    let size = (tile_px - 1.0) * STAFFED_MARK;
    Rect::new(
        px + STAFFED_MARK_INSET,
        py + tile_px - 1.0 - RARITY_BAR_PX - STAFFED_MARK_INSET - size - lift,
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
fn vignette(dx: f32, dy: f32, half_w_px: f32, half_h_px: f32, floor: f32) -> f32 {
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
fn vignette_floor(power: f32) -> f32 {
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
fn draw_tile_edges(
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
    // The one derivation of the five regions this screen draws into. Reads
    // no `Game`, so it is computed before the borrow below like every other
    // value gathered here.
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let regions = hud::layout::regions(
        painter.screen_w(),
        painter.screen_h(),
        char_w,
        m,
        app.log_expanded,
    );
    // The pane's rows, chosen by app-core (see `pane_rows`), and the header
    // that says which channel they are. Both read before the `game` borrow.
    //
    // Counted in *entries* against a pane measured in rows, so the two
    // diverge the moment one entry wraps — `draw_log_pane` cuts from the
    // oldest end, so over-asking costs nothing and under-asking loses the
    // newest news. Two rows come off the top: the keybar's, and the filter
    // header's, which is the pane's first body row (`LOG_FILTER_ROWS`).
    let log_capacity = ((regions.log_pane.h - m.line_height * (1.0 + hud::layout::LOG_FILTER_ROWS))
        / m.line_height)
        .max(1.0) as usize;
    let log_lines = app.visible_log(log_capacity);
    let log_filter = app.log_filter;
    // Before the `game` borrow, like `log_filter` above.
    let info_tab = app.info_tab;
    let filtered_out = app.filtered_out_log_lines();
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
    // Before the `game` borrow, like `plan` and `log_filter` above — and
    // this one is a *read that releases*: `App::watch_center` drops
    // `App::watching` the moment the engine stops answering, which is the
    // single rule behind "the program was dissolved, dispatched, taken into
    // the party, or the party left base space".
    let watch = app.watch_center();
    let watching = app.watching;
    let Some(game) = &mut app.game else { return };

    let stock_rows = game.base_stock();
    // One call, shared by every surface that reads it — the status bar's
    // badge here, and the info column's tab markers and collapsed bars once
    // phase 4's column lands. A second derivation is what would make "a
    // closed pane cannot hide an actionable state" a coincidence.
    let attention = game.attention();
    let status = game.player_status();
    // `Game::active_buffs` needs `&mut self`; fetched here rather than
    // inside `draw_status_panel`, which only ever needed `&Game` before
    // this and shouldn't have to start borrowing mutably just to draw.
    let buffs = game.active_buffs();
    // Read here, like `stock_rows`/`attention`/`status`/`buffs` above,
    // rather than inline at the surface branch's `draw_map_frame` call —
    // `Game::terrain_row` takes `&mut self` and this is where every other
    // such call already lands before the borrows below.
    let terrain = game.terrain_row();
    // Read here for `terrain`'s reason. `None` unless the camera is actually
    // on something, so the frame's ground readout keeps its mount for all
    // the time nobody is watching anything.
    let watch_label = watching.map(|e| game.creature_label(e));
    // Mounted on the log pane's top border by `draw_log_pane` at the end of
    // this function, so it is built here — before the `game` borrow it needs
    // — and held until then.
    let vitals = hud::log_frame::Vitals {
        status: &status,
        mining: game.mining(),
    };
    if let Some(view) = game.stack_view() {
        draw_stack(&view, painter, regions.map_pane, m, status.power);
        // Over the corridor, not part of it: the same map the `g` screen
        // draws, small enough to leave the view readable.
        if let Some(map) = game.frame_map() {
            draw_map_inset(&map, stack_zoom, painter, regions.map_pane, m);
        }
        // No surface entities are fetched down here, so there is nothing to
        // count hostiles among — the threat readout names what the surface
        // map shows, not what a stack frame's own view holds. `None` for the
        // same reason: a Stack frame has no biome for the ground readout to
        // name.
        hud::map_frame::draw_map_frame(
            regions.map_pane,
            None,
            hud::map_frame::Threat {
                hostiles: 0,
                shielded: game.raid_defense_active(),
            },
            // Never underground: watching is base space's, and the Stack
            // view is a corridor projection with no camera to move.
            None,
            painter,
            m,
        );
    } else {
        let entities = draw_surface_map(
            game,
            fx,
            painter,
            regions.map_pane,
            tile_px,
            glyph_px,
            &status,
            plan,
            // The party's own cell unless the camera has been sent
            // somewhere: `base_pos` is `Some` only in base space, which is
            // exactly where the pinned `Position` is the wrong answer.
            watch.unwrap_or_else(|| game.base_pos().unwrap_or(status.position)),
        );
        let hostiles = entities.iter().filter(|e| e.is_hostile).count();
        hud::map_frame::draw_map_frame(
            regions.map_pane,
            terrain,
            hud::map_frame::Threat {
                hostiles,
                shielded: game.raid_defense_active(),
            },
            watch_label.as_deref(),
            painter,
            m,
        );
    }

    // The BASE blocks are base-space only, and `structure_report` walks
    // every structure — gathered behind the same test the pane draws them
    // behind rather than every frame the party spends on the surface.
    let in_base = game.in_base();
    let (structures, builds, labour) = if in_base {
        (
            game.structure_report(),
            game.build_order_report(),
            game.labour_demand(),
        )
    } else {
        (Vec::new(), Vec::new(), Default::default())
    };
    let pets = game.owned_pets();
    // Unlike the BASE blocks this is not gated on `in_base`: a contract reads
    // from anywhere, and it is `&self` over at most `MAX_ACTIVE_CONTRACTS`
    // rows. `Game::contract_board` is the expensive one and is not called.
    let contracts = game.active_contracts();
    // `Game::copy_name` is the one place a copy's name is built, and it needs
    // the borrow this data outlives — so the names are resolved here rather
    // than the pane being handed a `Game`.
    let pack: Vec<hud::panes::PackRow> = status
        .inventory
        .iter()
        .map(|row| hud::panes::PackRow {
            qty: row.qty,
            name: game.copy_name(&row.copy),
            tier: row.copy.tier,
        })
        .collect();
    let pane = hud::panes::PaneData {
        roster: (status.pet_count, status.pet_capacity),
        carrying: status.inventory_used,
        buffs: &buffs,
        pack: &pack,
        pets: &pets,
        structures: &structures,
        stock: &stock_rows,
        builds: &builds,
        labour,
        contracts: &contracts,
        shielded: game.raid_defense_active(),
        in_base,
    };

    // The column draws its own fill and frame and hands back the body rect;
    // the open pane's rows are then fitted to it and drawn. The column does
    // not scroll, so `fitting_rows` counts what did not fit rather than
    // letting it fall off the bottom edge in silence.
    let body = hud::column::draw_info_column(
        regions.info_column,
        &hud::column::ColumnState {
            tab: info_tab,
            attention: &attention,
            pane: &pane,
        },
        painter,
        m,
    );
    let pane_rows = hud::panes::rows(info_tab, &pane);
    let (shown, cut) = hud::panes::fitting_rows(&pane_rows, body.h, m);
    hud::panes::draw_rows(body, &shown, cut, painter, m);

    hud::status_bar::draw_status_bar(
        regions.status_bar,
        &hud::status_bar::StatusBarState {
            zone: status.zone,
            position: game.base_pos().unwrap_or(status.position),
            tick: game.current_tick(),
            stock: &stock_rows,
            attention: &attention,
        },
        painter,
        m,
    );

    hud::log_frame::draw_log_pane(
        regions.log_pane,
        &hud::log_frame::LogPane {
            entries: &log_lines,
            filter: log_filter,
            filtered_out,
            vitals: &vitals,
            refusal: status_line.as_deref(),
            border: fx.log_border(hud::palette::PANE_BORDER),
        },
        painter,
        m,
    );
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
///
/// Returns the entities it drew — `draw_map_frame`'s threat readout counts
/// hostiles among them, and hoisting them out here is what keeps that a
/// second look at a Vec already built rather than a second call to
/// `Game::view_entities`.
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
    center: (i32, i32),
) -> Vec<EntityView> {
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
    // Every `VisualEffect` the engine queues names a *structure's* tile —
    // all three `Game::push_effect` callers are structure damage — so the
    // whole queue is base-space by construction. This pane draws one space
    // at a time, so on the surface those coordinates would land on
    // unrelated open ground: the same cross-space aliasing `view_entities`
    // refuses now, and the structure being flashed is not even drawn there
    // to explain it.
    //
    // Suppressed rather than moved to the anchor: a raid already reaches a
    // player who is out of the base through the log pane's own flash and a
    // `MessageKind::Raid` line, and neither of those claims a tile.
    let show_effects = base_pos.is_some();
    // Hoisted out of the tile loop: it is one reading of one reserve, and the
    // whole grid is drawn against the same one.
    let floor = vignette_floor(status.power);
    let (off_x, off_y) = fx.camera_offset(center, painter.delta());
    let tiles = game.view_tiles_at(center, hw, hh);
    let entities: Vec<_> = game
        .view_entities_at(center, hw, hh)
        .into_iter()
        // A tamed program is drawn while it is out on an errand, while it is
        // on a job with no glyph at the far end — a builder walking its
        // materials over and raising them, a digger cutting a wall — and
        // while it is loitering with no job at all. At a *machine's* post it
        // sits under that machine's own glyph, so a base at rest reads as
        // buildings and motion is the only thing that draws the eye. A guard
        // and a party member stay hidden for a harder reason: neither is
        // ever walked, so each keeps whatever tile it was standing on when
        // it took the job — out on the surface, or four frames down — and
        // drawing it would claim it is somewhere it isn't.
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
    // Read once for the whole map rather than per tile, and gated on the
    // space rather than on the flag alone: the tools arm the player's own
    // bump into rock, base space is the only place there is rock to bump
    // into, and nothing disarms them on the way back out through the
    // anchor. Ungated, the ring follows the party onto the zone map and
    // claims something the ground there cannot answer.
    let cutting = base_pos.is_some() && game.mining();
    // Cloud shadows are the zone map's alone. Base space is a pocket cut
    // out of rock with no sky over it, and the Stack draws through
    // `render/stack.rs` and never reaches here at all — so this one flag is
    // the whole of the gate. Read once for the same reason `hues` is: it is
    // a property of the locale, not of a tile.
    let outdoors = base_pos.is_none();
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
            // A third category beside those two, and it has to be: a build
            // site is not a `Structure`, so the `is_structure` test above
            // would drop it into `actor` — where it would either be hidden
            // by the builder standing on it or hide the builder itself. It
            // is ground being worked, which is what the frame below draws it
            // as.
            let mut building: Option<&EntityView> = None;
            // The entity wearing the mark, and whether its work has hit the
            // dead end below.
            let mut mark: Option<(Entity, bool)> = None;
            for ev in &entities {
                let erx = ev.pos.0 - center.0 + hw;
                let ery = ev.pos.1 - center.1 + hh;
                if erx != rx as i32 || ery != ry as i32 {
                    continue;
                }
                // A machine being upgraded carries its own pending row, so
                // `build` alone no longer means "nothing is standing here
                // yet": tested first, a working machine would draw as a bare
                // build slab for as long as its upgrade was on order.
                if ev.build.is_some() && !ev.is_structure {
                    building = Some(ev);
                } else if ev.is_structure {
                    structure = Some(ev);
                } else if !matches!(actor, Some(a) if a.is_player) {
                    actor = Some(ev);
                }
                if if ev.is_structure {
                    ev.structure_attended
                } else {
                    ev.wears_job_mark
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
                // Node reads green running, dim yellow starved, the
                // attention yellow when it is asking for you, grey idle. Which structure it is stays legible from the
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
                // The `@` is the one glyph whose colour is a role. Every
                // other entity wears the hue its file authored; the player
                // wears a colour off the character-creation wizard —
                // `PlayerLook::colour` is a **0-based** index into
                // `hud::palette::PLAYER_CHOICES`, and `None` is *no answer*
                // rather than a bad one: the wizard was never opened, or the
                // save predates it, so the glyph keeps the `PLAYER` role
                // colour. Read off `is_player` rather than off the authored
                // `GlyphColor::Cyan`, which a structure is free to author
                // too.
                color = if ev.is_player {
                    player_look_color(ev.look.as_ref().and_then(|look| look.colour))
                } else {
                    glyph_color(ev.color)
                };
            }
            // Bare ground only. Where something is standing, the background
            // carries the damage-dimmed glyph colour, and jittering that
            // would muddy a structure's durability read.
            // The cloud rides `shade` and not `vig`, which buys three
            // things. `occupied` already excludes it, so a shadow falls on
            // bare ground only — the rule the shade jitter and the biome
            // pattern already follow, and for their reason: a moving dim
            // over a structure would muddy the durability wash. The glyph
            // path below takes `vig` alone, so what stands on the ground
            // stays lit under a cloud. And `vignette` goes on meaning the
            // Power reserve and nothing else.
            let cloud = if outdoors { fx.cloud_shade(world) } else { 1.0 };
            let shade = if occupied {
                1.0
            } else {
                tile_shade(world) * cloud
            };
            let vig = vignette(
                px + tile_px / 2.0 - (pane.x + pane.w / 2.0),
                py + tile_px / 2.0 - (pane.y + pane.h / 2.0),
                pane.w / 2.0,
                pane.h / 2.0,
                floor,
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
                draw_tile_edges(painter, &tiles, rx, ry, cell, biome_color, vig, cloud);
            }
            // A structure the crew has not raised yet: a flat dark slab with
            // a darker edge, drawn over the ground and under everything that
            // stands on it.
            //
            // **A block rather than the structure's own glyph in grey.**
            // What is going up here is not the question a player glances at
            // this cell to answer — the caret says "work is happening", the
            // examine page says what and how far along, and a dim `M` would
            // read as a Mining Node that had gone dark. It also keeps the
            // cell legible as *not yet a thing*: nothing else on this map is
            // a filled block, so a site is never mistaken for a machine.
            //
            // Both take the vignette and not the tile shade, matching the
            // glyph rule below: this is something standing on the ground,
            // not the ground.
            if building.is_some() {
                painter.rect(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    at_level(BUILD_SITE_FILL, vig),
                );
                painter.rect_lines(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    BUILD_SITE_EDGE_PX,
                    at_level(BUILD_SITE_EDGE, vig),
                );
            }
            // The glyph takes the vignette but not the shade: depth should
            // apply to everything on the map evenly, while per-tile jitter is
            // a property of the ground, not of what stands on it.
            let color = Color::new(color.r * vig, color.g * vig, color.b * vig, color.a);
            if let Some(ch) = ch {
                // A sprite *substitutes* for the glyph rather than drawing
                // beside it, so it can only appear on a tile that would have
                // had one — and a miss falls back to that glyph, which is
                // what keeps `assets/sprites/` optional.
                //
                // The anchor is the one fixture left named here. It is not
                // content — a permanent feature of every zone, with no
                // `.ron` file to carry a `sprite:` field — so its name stays
                // written in this match rather than read off the view. The
                // player's name comes off `PlayerLook` instead: the
                // character-creation wizard's own choice, in place of the
                // literal `"player"` this used to read regardless of who was
                // playing.
                //
                // The anchor is the case the sprite seam was worth building
                // for. Its `#` was chosen by elimination rather than by
                // meaning — `>` and `<` were claimed by Stack links and
                // Blue and Cyan were reserved, see `Game::new`'s anchor
                // spawn — so it is the one glyph on this map that says
                // nothing at all about what stands there.
                // Whether the player has a drawing at all is read off
                // their own look and not off the table, so this says which
                // rung the tile is on rather than which keys happen to be
                // loaded. A drawing the table has nothing under — one that
                // is still uploading, or a blank canvas `sync_drawn_icon`
                // declined — misses and falls to the rung below, like any
                // other sprite.
                let drawn_icon = actor.is_some_and(|ev| {
                    ev.is_player && ev.look.as_ref().is_some_and(|look| look.icon.is_some())
                });
                let sprite = actor.and_then(|ev| {
                    if ev.is_player {
                        ev.look
                            .as_ref()
                            .and_then(|look| player_sprite_name(&look.sprite))
                    } else if ev.is_anchor {
                        Some("anchor")
                    } else {
                        None
                    }
                });
                // Centred on the cell, not on measured ink: a square sprite
                // has neither side bearing nor descender. `glyph_px` and not
                // `tile_px`, so the sprite keeps the glyph's margin inside
                // its tile and stays on the integer ladder — 16, 32, 48, 64.
                let inset = (tile_px - glyph_px as f32) / 2.0;
                // **The player's own drawing is the top rung, and it is the
                // one sprite in the game drawn untinted.** Every other
                // sprite is authored near-white and inherits its tile's
                // colour through egui's multiplying tint — that is what
                // `assets/sprites/README.md` asks art for. A drawing is the
                // exception on purpose: the player picked its fifteen
                // colours themselves, and multiplying those by an indigo or
                // rose swatch turns most of them black.
                //
                // So the tint here is neutral **at the vignette's value** —
                // the hue is dropped, the depth shading is kept, exactly as
                // for every other tile. What that costs is the Colour
                // step's swatch on this one tile and nothing else: the
                // `is_player` arm above sets `color` to
                // `player_look_color(...)` and nothing but `vig` touches it
                // after. `App::creation_colour_note` tells the player so on
                // the step where they choose it.
                //
                // **Putting the hue back reads as a bug fix and is not
                // one.** If this ever looks wrong, the answer is art
                // authored near-white — which a hand-drawn icon is not.
                let neutral = Color::new(vig, vig, vig, color.a);
                let drew = (drawn_icon
                    && painter.sprite(
                        crate::sprites::DRAWN_ICON_KEY,
                        px + inset,
                        py + inset,
                        glyph_px as f32,
                        neutral,
                    ))
                    || sprite.is_some_and(|name| {
                        painter.sprite(name, px + inset, py + inset, glyph_px as f32, color)
                    });
                if !drew {
                    let glyph = ch.to_string();
                    let dims = painter.measure_map(&glyph, glyph_px);
                    let tx = px + (tile_px - dims.width) / 2.0;
                    let ty = py + (tile_px + dims.height) / 2.0;
                    painter.map(&glyph, tx, ty, glyph_px, color);
                }
            }
            // The caret, bouncing in the middle of the slab.
            //
            // Drawn here rather than through the `ch` path above because
            // that path has no vertical offset to give it — and the bounce
            // is the whole point: a build site is the one cell on the map
            // where *nothing is happening yet* is the wrong reading. It
            // reuses the raised cosine the "someone is on this job" mark
            // rides, so the two motions on this map agree with each other
            // rather than being two independently-invented curves.
            // Phase-keyed by entity for that helper's own reason: two sites
            // side by side bounce out of step and read as two jobs rather
            // than one animation.
            //
            // **`centred_bob` and not `staffed_bob`**: this caret's rest
            // position is the centre of its own slab, not an inset off the
            // tile's bottom edge, so it has room on both sides. Riding the
            // upward-only form it spent its whole cycle at or above centre
            // and read as sitting high in the tile.
            //
            // Over the glyph layer, so a builder standing on the cell is
            // drawn under its own work. Under the marks and outlines below,
            // which are all about state rather than about the tile.
            if let Some(ev) = building {
                let glyph = ev.glyph.to_string();
                let dims = painter.measure_map(&glyph, glyph_px);
                let lift = fx.centred_bob(ev.entity);
                painter.map(
                    &glyph,
                    px + (tile_px - dims.width) / 2.0,
                    py + (tile_px + dims.height) / 2.0 - lift,
                    glyph_px,
                    Color::new(ORANGE.r * vig, ORANGE.g * vig, ORANGE.b * vig, ORANGE.a),
                );
            }
            // A body a Bay is mending, drawn on the same layer as the build
            // caret and for the same reason: it is a mark that needs the
            // middle of the tile and a vertical offset, and the glyph path
            // above has neither to give it.
            //
            // **`actor` and not `structure`** — the mark says *this program
            // is being repaired*, so it rides the patient. Which is also the
            // glyph the tile is drawing: an actor takes the tile's glyph off
            // a structure, so on a Bay with somebody standing in it the `+`
            // and the body it belongs to are the same cell's ink. Passed the
            // structure instead, a Bay whose reach is more than its own cell
            // would mark itself while the program it is healing stood
            // unmarked a tile away.
            //
            // **Base space, gated the way the raid flash is.** Both ends of
            // this stand in base space and `EntityView::recovering` is
            // derived from base-space `Position`s, so on the zone map this
            // cell's coordinates mean something else entirely — the
            // cross-space aliasing `view_entities` and the spawn-point
            // outline both refuse. Neither would be in `entities` out there
            // anyway; the gate is what keeps that an assertion rather than a
            // coincidence.
            if base_pos.is_some() {
                draw_recovery_mark(
                    painter,
                    actor,
                    fx,
                    Rect::new(px, py, tile_px, tile_px),
                    glyph_px,
                    vig,
                );
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
            // The same reading turned upside down. `difficulty_color` used
            // to *replace* the glyph's hue, so a tile said either what a
            // program is or how dangerous it is; the bottom edge is what
            // buys back the glyph. Keyed off `actor` for the rarity bar's
            // reason — a structure has no con read, and the engine hands
            // `None` for everything that is not hostile.
            draw_difficulty_bar(
                painter,
                actor.and_then(|ev| ev.difficulty),
                px,
                py,
                tile_px,
                vig,
            );
            // A nemesis draws a second mark on top of its reserved glyph
            // colour — belt and braces, since a nemesis is worth noticing
            // even at a glance that only catches shape and not hue. Its own
            // corner rather than sharing the bar's, so a nemesis that is
            // also rare (the two are independent) shows both without either
            // being spent to make room.
            // Its own corner and its own fact. A boss used to be drawn by
            // `difficulty_color` returning magenta for its whole glyph,
            // which spent the con read — and lost outright to the nemesis
            // override when a creature was both. Two marks, two corners,
            // and the con bar answers the fight question for both.
            if actor.is_some_and(|ev| ev.is_boss) {
                let mark = boss_mark_rect(px, py, tile_px);
                painter.rect(
                    mark.x,
                    mark.y,
                    mark.w,
                    mark.h,
                    at_level(boss_mark_color(), vig),
                );
            }
            if actor.is_some_and(|ev| ev.nemesis) {
                let mark = nemesis_mark_rect(px, py, tile_px);
                painter.rect(
                    mark.x,
                    mark.y,
                    mark.w,
                    mark.h,
                    at_level(hud::palette::glyph(GlyphColor::Cyan), vig),
                );
            }
            // The base-space mirror of that outline: an armed bump is a mode
            // with no other trace on the screen — the log said so once, at
            // the moment it was toggled, and a player who walked away and
            // came back has nothing left to read it off. A ring rather than
            // a colour on the `@` itself, so the sprite that stands in for
            // that glyph carries the cue too.
            if cutting && actor.is_some_and(|ev| ev.is_player) {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, CUTTING_OUTLINE);
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
                // The attention colour as well as still: colour and motion
                // say the same thing at once, so a stranded machine is
                // legible from a paused screenshot and not only from
                // watching it. It wears the same role its glyph does,
                // because they are saying the same thing — what separates
                // this from a staffed mark is the blink and the green, and
                // what separates it from a clogged machine was never the
                // mark's hue but the fact that a clogged one still has a
                // worker to bob.
                let (lift, alpha, base) = if stranded {
                    (0.0, fx.stranded_blink(), hud::palette::ATTENTION)
                } else {
                    (fx.staffed_bob(marked), 1.0, hud::palette::HEALTHY)
                };
                let m = staffed_mark_rect(px, py, tile_px, lift);
                painter.rect(m.x, m.y, m.w, m.h, Color { a: alpha, ..base });
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
        // Behind the same gate as the debris, and for a sharper version of
        // the same reason: a walk names base-space cells, and a squad drawn
        // filing out of the anchor while the pane is showing the zone
        // surface would be walking across open ground the party is standing
        // on. Over the tiles, since a body walks on top of the floor.
        fx.draw_walkers(painter, tile_px, glyph_px, |world| {
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
    entities
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
    tile(plan.cursor, None, Some((2.0, hud::palette::EMPHASIS)));
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

/// The bouncing green `+` a program wears while a Repair Bay is mending it,
/// or nothing at all for every other cell on the map.
///
/// **The gate lives in here rather than at the call site**, so a test can
/// hold both halves: a body being mended draws the mark and everything else
/// — the Bay doing the mending included — draws nothing.
/// `EntityView::recovering` is the engine's own answer, derived from the same
/// `Bays::serving` the heal runs through, so the mark cannot claim a
/// recovery the heal is not performing.
///
/// **`palette::HEALTHY`, and the colour moved with the mark.** On the Bay it
/// was `THREAT`, which stretched that role's reservation — hostility and
/// inbound harm — over a building doing the player a favour. On the body it
/// is Integrity climbing, which is the bar fill's own green and the one
/// thing this map ever paints in it.
///
/// It rides `Fx::centred_bob`, the build caret's curve: the caret's argument
/// applies unchanged here — the rest position is the middle of the tile with
/// room on both sides, where `staffed_bob`'s upward-only form would sit the
/// mark high in the cell for its whole cycle. Sharing the curve is also what
/// keeps a base with a build site and a mending body in it reading as one map
/// rather than two animations, and the phase key being the *entity* spreads
/// two patients out of step.
fn draw_recovery_mark(
    painter: &Painter,
    actor: Option<&EntityView>,
    fx: &Fx,
    cell: Rect,
    glyph_px: u16,
    vig: f32,
) {
    let Some(ev) = actor.filter(|ev| ev.recovering) else {
        return;
    };
    let glyph = RECOVERY_MARK.to_string();
    let dims = painter.measure_map(&glyph, glyph_px);
    let lift = fx.centred_bob(ev.entity);
    painter.map(
        &glyph,
        cell.x + (cell.w - dims.width) / 2.0,
        cell.y + (cell.h + dims.height) / 2.0 - lift,
        glyph_px,
        at_level(hud::palette::HEALTHY, vig),
    );
}

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
        MachineStatus::Running => hud::palette::HEALTHY,
        MachineStatus::Starved | MachineStatus::Unstaffed => hud::palette::WARN,
        // The louder yellow rather than the dimmer one: unlike `Unstaffed`,
        // waiting does not fix this one, so it belongs with the states that
        // are asking for you — which is what `palette::ATTENTION` means, and
        // is the same colour `Game::attention` puts in the status bar for
        // them. `Unpowered` joins them because a dark machine never resolves
        // itself either, only a Recharger Node fixes it.
        //
        // Not red. Red is `palette::THREAT`, reserved for hostility and
        // inbound harm, and a clogged Mining Node is neither.
        MachineStatus::Clogged | MachineStatus::Stranded | MachineStatus::Unpowered => {
            hud::palette::ATTENTION
        }
        MachineStatus::Idle => hud::palette::FAINT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::SpriteTable;
    use crate::paint::{painted_images, painted_text, with_painter, with_sprites};
    use crate::text::ui_metrics;
    use feral_processes_engine::MessageSource;
    use feral_processes_engine::components::{GlyphColor, MachineStatus};
    use feral_processes_engine::{CharacterChoice, DifficultyMode, Game};

    fn test_assets() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// One tile of the map, at the origin, big enough for the mark to have
    /// somewhere to bounce.
    const CELL: f32 = 20.0;
    const CELL_GLYPH_PX: u16 = 16;

    /// A staff program as the map sees one, mending or not.
    fn patient_view(recovering: bool) -> EntityView {
        EntityView {
            entity: Entity::PLACEHOLDER,
            pos: (0, 0),
            glyph: 'd',
            color: GlyphColor::Cyan,
            label: "Scrapper".into(),
            is_player: false,
            look: None,
            is_tamed: true,
            is_companion: false,
            is_hostile: false,
            is_structure: false,
            is_anchor: false,
            is_home: false,
            tier: None,
            ceiling: None,
            max_tier: None,
            is_boss: false,
            nemesis: false,
            difficulty: None,
            can_work: false,
            can_trade: false,
            issues_contracts: false,
            structure_worker: None,
            wears_job_mark: false,
            position_is_honest: true,
            structure_attended: false,
            recovering,
            build: None,
            output_stranded: false,
            hp_fraction: None,
            level: None,
            durability: None,
            fusions: 0,
            rarity: Rarity::Ordinary,
            machine_status: None,
            linked_edges: Vec::new(),
        }
    }

    /// Where the `+` was painted this frame, or `None` if it was not.
    fn mark_y(shapes: &[bevy_egui::egui::epaint::ClippedShape]) -> Option<f32> {
        shapes.iter().find_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Text(t) if t.galley.text() == "+" => Some(t.pos.y),
            _ => None,
        })
    }

    fn recovery_mark_shapes(
        fx: &Fx,
        ev: &EntityView,
    ) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
        let (_, shapes) = with_painter(|p| {
            draw_recovery_mark(
                p,
                Some(ev),
                fx,
                Rect::new(0.0, 0.0, CELL, CELL),
                CELL_GLYPH_PX,
                1.0,
            )
        });
        shapes
    }

    fn recovery_mark_at(fx: &Fx, ev: &EntityView) -> Option<f32> {
        mark_y(&recovery_mark_shapes(fx, ev))
    }

    /// The whole of what this mark ships, in the three states that matter:
    /// it is drawn on a program a Bay is mending, it is drawn on *nothing*
    /// else, and it moves.
    ///
    /// The motion is not decoration and is asserted rather than assumed —
    /// `draw_recovery_mark` reads `Fx::centred_bob`, and a mark pinned to the
    /// middle of the tile would paint an identical shape every frame and pass
    /// a test that only looked for the glyph. Sampled across a spread of
    /// frame times rather than at one hand-picked half period, so retuning
    /// the bob's rate cannot turn this into a failure that means nothing.
    #[test]
    fn a_recovering_program_wears_a_bouncing_mark_and_nothing_else_does() {
        let mut fx = Fx::new();
        fx.begin_frame(0.0, Vec::new(), Vec::new(), false);

        assert!(
            recovery_mark_at(&fx, &patient_view(true)).is_some(),
            "a program a Bay is mending must wear the mark"
        );
        assert!(
            recovery_mark_at(&fx, &patient_view(false)).is_none(),
            "a program that is not recovering must draw nothing at all"
        );

        let busy = patient_view(true);
        let ys: Vec<f32> = [0.0, 0.15, 0.3, 0.45, 0.6, 0.75]
            .into_iter()
            .map(|now| {
                fx.begin_frame(now, Vec::new(), Vec::new(), false);
                recovery_mark_at(&fx, &busy).expect("the mark is drawn every frame")
            })
            .collect();
        assert!(
            ys.iter().any(|y| (y - ys[0]).abs() > 0.5),
            "the mark must bounce, not sit still: {ys:?}"
        );
    }

    /// **The mark is green, and the colour is half of what it says.** It was
    /// `palette::THREAT` while it sat on the Bay, and a red `+` over a body
    /// whose Integrity is climbing reads as the harm rather than the cure —
    /// that role is reserved for hostility and inbound harm, and a raid's
    /// flash and a structure taking a hit are the map's only other spenders
    /// of it. Asserted against both roles rather than only the one it must
    /// be, so a revert to the old colour fails here rather than in play.
    #[test]
    fn the_recovery_mark_is_painted_in_the_healthy_role_and_not_the_threat_one() {
        let mut fx = Fx::new();
        fx.begin_frame(0.0, Vec::new(), Vec::new(), false);

        let shapes = recovery_mark_shapes(&fx, &patient_view(true));
        let painted: Vec<Color> = crate::paint::painted_map_glyphs(&shapes)
            .into_iter()
            .filter(|(text, _)| text == "+")
            .map(|(_, c)| c)
            .collect();
        assert_eq!(painted.len(), 1, "exactly one `+` is painted");

        // A distance rather than an equality: `at_level` multiplies the role
        // by the vignette and egui rounds the result to eight bits, so what
        // comes back is the role's colour within a rounding step and never
        // bit-identical to it.
        let gap = |a: Color, b: Color| {
            ((a.r - b.r).powi(2) + (a.g - b.g).powi(2) + (a.b - b.b).powi(2)).sqrt()
        };
        let healthy = gap(painted[0], hud::palette::HEALTHY);
        let threat = gap(painted[0], hud::palette::THREAT);
        assert!(
            healthy < 0.01,
            "the mark must be painted in `palette::HEALTHY`: off by {healthy}"
        );
        assert!(
            threat > healthy,
            "and must not have drifted back toward `palette::THREAT`"
        );
    }

    /// A fresh `App` with a game already in progress, for a test that draws
    /// `draw_playing_base` directly rather than one of its pieces. `game` is
    /// assigned by hand instead of walking `App::handle_key` through the new
    /// game flow — this only needs *a* game standing on the surface, not the
    /// menu path that produces one.
    fn playing_app() -> feral_processes_app_core::App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp =
            std::env::temp_dir().join(format!("fp_gui_map_frame_census_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut app = feral_processes_app_core::App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        );
        app.game = Some(
            Game::new(7, DifficultyMode::Forgiving, &test_assets())
                .expect("the shipped assets must load"),
        );
        app
    }

    /// What the map frame's left mount actually draws for `app`'s game right
    /// now — `Game::terrain_row` through the same `ground_pieces` the
    /// renderer calls, rather than a hardcoded string that would drift the
    /// moment the ground readout's wording changed. Read before
    /// `draw_playing_base` runs, since `terrain_row` needs `&mut Game`.
    fn ground_label(app: &mut feral_processes_app_core::App) -> String {
        let row = app
            .game
            .as_mut()
            .and_then(|g| g.terrain_row())
            .expect("a fresh surface game has ground for the border to read");
        hud::map_frame::ground_pieces(&row)[0].0.clone()
    }

    /// **The trap Task 4 exists to close.** `border_strip` paints its own
    /// background quad, so calling `draw_map_frame` before the pane's
    /// contents lets the map's own fill paint straight over the frame's
    /// strips — the design handoff's own recorded failure.
    /// `a_strip_paints_its_background_before_its_glyphs` in `hud::strip`
    /// only proves the order *inside* `border_strip`; this is the half only
    /// visible a level up, where the map pane's own background rect and the
    /// frame's title text are two separate calls inside `draw_playing_base`.
    /// The `@` is the one glyph on the map whose colour is a **role** and not
    /// the hue its entity authored. `GlyphColor::Cyan` is what the player
    /// spawns with and what a Mainframe-blue structure may author too; br
    /// cyan is the player's alone, and reading it off `is_player` is what
    /// keeps it that way.
    ///
    /// Distance rather than equality, because the map multiplies everything
    /// it draws by a vignette and a per-tile shade — the player stands at the
    /// centre of its own view, where both are all but 1.0, but "all but" is
    /// what rules equality out. The second assertion is the load-bearing one:
    /// the authored cyan is what this used to paint.
    #[test]
    fn the_players_glyph_wears_the_player_role() {
        let mut app = playing_app();
        let mut fx = Fx::new();
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| {
            draw_playing_base(&mut app, &mut fx, None, p, &m);
        });
        let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
        let at = crate::paint::painted_map_glyphs(&shapes);
        let (_, drawn) = at
            .iter()
            .find(|(g, _)| g == "@")
            .expect("the map draws the player");
        assert!(
            dist(*drawn, hud::palette::PLAYER) < 0.06,
            "the player's @ painted {drawn:?}, which is {:.3} from PLAYER",
            dist(*drawn, hud::palette::PLAYER)
        );
        assert!(
            dist(*drawn, hud::palette::PLAYER) < dist(*drawn, glyph_color(GlyphColor::Cyan)),
            "the @ is nearer its authored hue than the player role — it is \
             still being drawn through `glyph_color`"
        );
    }

    /// The palette's two reservations, asserted where they are easiest to
    /// break: br red means hostility and inbound harm, so a clogged machine —
    /// an ordinary, self-inflicted, entirely fixable state — may not wear it,
    /// and br yellow means the player must act, which is exactly what the
    /// three states that will not resolve themselves are.
    ///
    /// The split the old red/yellow pair carried is kept, not flattened:
    /// waiting fixes a starved machine and does not fix a clogged one, so
    /// they stay two colours.
    #[test]
    fn a_machine_asks_for_attention_and_never_reads_as_a_threat() {
        for status in [
            MachineStatus::Clogged,
            MachineStatus::Stranded,
            MachineStatus::Unpowered,
        ] {
            assert_eq!(
                machine_color(status),
                hud::palette::ATTENTION,
                "{status:?} will not fix itself and is what ATTENTION means"
            );
        }
        for status in [MachineStatus::Starved, MachineStatus::Unstaffed] {
            assert_eq!(
                machine_color(status),
                hud::palette::WARN,
                "{status:?} resolves itself, so it is the dimmer yellow"
            );
        }
        assert_eq!(machine_color(MachineStatus::Running), hud::palette::HEALTHY);
        // Walked rather than listed: a status added without a colour of its
        // own is exactly the one that would inherit red by accident.
        for status in MachineStatus::ALL {
            assert_ne!(
                machine_color(status),
                hud::palette::THREAT,
                "{status:?} is not hostility, and THREAT is reserved for it"
            );
        }
    }

    /// A plan is the player having acted; the attention yellow is the base
    /// asking them to. Drawn in the same family they were two different
    /// pieces of news in one colour — the washes sat 0.11 from `ATTENTION`,
    /// which is nothing at 18% alpha over tinted rock.
    ///
    /// All four are checked against the same rule, because the plan is one
    /// hue at four alphas and a retune that moves only the one being read
    /// here is exactly what this is for.
    #[test]
    fn a_plan_cannot_be_read_as_a_machine_asking_for_you() {
        let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
        for (name, c) in [
            ("MARK_FILL", MARK_FILL),
            ("MARK_EDGE", MARK_EDGE),
            ("PREVIEW_FILL", PREVIEW_FILL),
            ("CUTTING_OUTLINE", CUTTING_OUTLINE),
        ] {
            assert!(
                dist(c, hud::palette::ATTENTION) > 0.25,
                "{name} is only {:.2} from ATTENTION",
                dist(c, hud::palette::ATTENTION)
            );
            assert!(
                dist(c, hud::palette::WARN) > 0.25,
                "{name} is only {:.2} from WARN",
                dist(c, hud::palette::WARN)
            );
            assert_eq!(
                (c.r, c.g, c.b),
                (
                    hud::palette::PLAN.r,
                    hud::palette::PLAN.g,
                    hud::palette::PLAN.b
                ),
                "{name} is not the plan's hue — a plan reads as one thing or \
                 it reads as several"
            );
        }
    }

    #[test]
    fn the_map_frame_draws_after_the_map() {
        let mut app = playing_app();
        let label = ground_label(&mut app);
        let mut fx = Fx::new();
        let m = ui_metrics(900.0);
        let (regions, shapes) = with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let regions =
                hud::layout::regions(p.screen_w(), p.screen_h(), char_w, &m, app.log_expanded);
            draw_playing_base(&mut app, &mut fx, None, p, &m);
            regions
        });

        let pane = regions.map_pane;
        let map_bg = shapes
            .iter()
            .position(|cs| match &cs.shape {
                // `fill.a() > 0` is load-bearing: `draw_map_frame` also
                // paints a rect the exact size of the pane — its border, via
                // `rect_lines`, which is a stroke with a *transparent* fill.
                // Matching on geometry alone would find that border instead
                // of the map's own `painter.rect(...)` fill and pass by
                // accident whichever one drew first.
                bevy_egui::egui::Shape::Rect(r) => {
                    r.fill.a() > 0
                        && (r.rect.min.x - pane.x).abs() < 0.5
                        && (r.rect.min.y - pane.y).abs() < 0.5
                        && (r.rect.width() - pane.w).abs() < 0.5
                        && (r.rect.height() - pane.h).abs() < 0.5
                }
                _ => false,
            })
            .expect("the map pane paints a background rect the size of the pane");
        let title = shapes
            .iter()
            .position(|cs| {
                matches!(&cs.shape, bevy_egui::egui::Shape::Text(t) if t.galley.text().contains(&label))
            })
            .unwrap_or_else(|| panic!("the map frame never painted its ground readout {label:?}"));

        assert!(
            title > map_bg,
            "the map's background painted at {map_bg}, the {label:?} readout at {title} — \
             the frame drew before the map, so its own fill painted over the label"
        );
    }

    /// **Bug A.** `border_strip` centres its background quad *on* the border
    /// line it mounts to, so a strip riding the map pane's top border
    /// reaches upward past `map_pane.y` by `size/2 + pad/2` — and
    /// `map_pane.y` used to sit exactly on the status bar's own bottom edge.
    /// `draw_status_bar` paints its opaque fill *after* `draw_map_frame` has
    /// already run, so the top of that quad — and the top of the ground
    /// readout's and the THREAT readout's glyph caps riding on it — painted
    /// straight through the bar.
    ///
    /// Asserted against the real painted quad rather than a region number:
    /// `the_playing_screen_draws_inside_its_regions` already checks
    /// `map_pane.y >= status_bar.y + status_bar.h`, and that alone passes
    /// against this bug, since the quad reaches *above* `map_pane.y` by a
    /// margin that number says nothing about.
    #[test]
    fn the_map_frames_top_strips_clear_the_status_bar() {
        let mut app = playing_app();
        let ground = ground_label(&mut app);
        let mut fx = Fx::new();
        let m = ui_metrics(900.0);
        let (regions, shapes) = with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let regions =
                hud::layout::regions(p.screen_w(), p.screen_h(), char_w, &m, app.log_expanded);
            draw_playing_base(&mut app, &mut fx, None, p, &m);
            regions
        });

        let status_bottom = regions.status_bar.y + regions.status_bar.h;

        // The ground readout (Mount::TopLeft) and the THREAT readout
        // (Mount::TopRight) both ride the map pane's top border. Each paints
        // its background quad immediately before its own glyphs
        // (`strip::border_strip`'s ordering rule), so the nearest preceding
        // filled rect *is* that quad.
        for label in [ground.as_str(), "THREAT"] {
            let text_idx = shapes
                .iter()
                .position(|cs| {
                    matches!(&cs.shape, bevy_egui::egui::Shape::Text(t) if t.galley.text().contains(label))
                })
                .unwrap_or_else(|| panic!("the map frame never painted {label:?}"));
            let quad = shapes[..text_idx]
                .iter()
                .rev()
                .find_map(|cs| match &cs.shape {
                    bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 => Some(r.rect),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{label} has no background quad ahead of it"));
            assert!(
                quad.min.y >= status_bottom - 0.01,
                "{label}'s background quad starts at y={} against the status \
                 bar's {status_bottom}px bottom edge — it paints into the bar",
                quad.min.y
            );
        }
    }

    /// **Bug C's other half.** The expanded log pane is an *overlay*: it
    /// keeps the collapsed pane's bottom edge and grows upward over the
    /// bottom of the map, which is what stops SPACE re-laying the grid out
    /// under the player. That costs the layout nothing only because the log
    /// is drawn **after** the map and fills opaquely — move
    /// `draw_log_pane` above the map and the expanded rows are painted over
    /// by the very pane they are supposed to cover, with nothing failing to
    /// compile.
    #[test]
    fn the_expanded_log_pane_draws_over_the_map() {
        let mut app = playing_app();
        app.log_expanded = true;
        let mut fx = Fx::new();
        let m = ui_metrics(900.0);
        let (regions, shapes) = with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let regions =
                hud::layout::regions(p.screen_w(), p.screen_h(), char_w, &m, app.log_expanded);
            draw_playing_base(&mut app, &mut fx, None, p, &m);
            regions
        });

        assert!(
            regions.log_pane.y < regions.map_pane.y + regions.map_pane.h,
            "the expanded log does not reach the map, so nothing is overlaid"
        );

        // `fill.a() > 0` for `the_map_frame_draws_after_the_map`'s reason:
        // both panes also paint a `rect_lines` border of the same geometry
        // with a transparent fill.
        let fill_of = |pane: crate::paint::Rect| {
            shapes
                .iter()
                .position(|cs| match &cs.shape {
                    bevy_egui::egui::Shape::Rect(r) => {
                        r.fill.a() > 0
                            && (r.rect.min.x - pane.x).abs() < 0.5
                            && (r.rect.min.y - pane.y).abs() < 0.5
                            && (r.rect.width() - pane.w).abs() < 0.5
                            && (r.rect.height() - pane.h).abs() < 0.5
                    }
                    _ => false,
                })
                .expect("both panes paint a background fill the size of the pane")
        };
        let map_bg = fill_of(regions.map_pane);
        let log_bg = fill_of(regions.log_pane);
        assert!(
            log_bg > map_bg,
            "the map's fill painted at {map_bg} and the log's at {log_bg} — \
             the overlay is under the pane it overlays"
        );
    }

    /// The first filled rect painted after `index` that lands on `quad`.
    ///
    /// A strip's own background quad is painted immediately before its
    /// glyphs (`strip::border_strip`'s ordering rule), so it is *not* a
    /// candidate when `index` is that strip's text — which is what makes
    /// "nothing opaque lands on top of it afterwards" the question this
    /// answers.
    fn covering_rect_after(
        shapes: &[bevy_egui::egui::epaint::ClippedShape],
        index: usize,
        quad: bevy_egui::egui::Rect,
    ) -> Option<bevy_egui::egui::Rect> {
        // A pane whose edge lands exactly on the quad's is the clearance
        // holding, not a rect painted over it, so the overlap has to be an
        // area rather than a touch — the same 0.01 slack every other
        // geometry assertion on this screen carries.
        let covers = |r: &bevy_egui::egui::Rect| {
            r.min.x < quad.max.x - EDGE_SLACK
                && r.max.x > quad.min.x + EDGE_SLACK
                && r.min.y < quad.max.y - EDGE_SLACK
                && r.max.y > quad.min.y + EDGE_SLACK
        };
        shapes[index + 1..].iter().find_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 && covers(&r.rect) => Some(r.rect),
            _ => None,
        })
    }

    /// Two edges this close are the same edge.
    const EDGE_SLACK: f32 = 0.01;

    /// The vitals strip's glyphs and the background quad they sit on.
    ///
    /// The whole strip is one galley (`strip::draw_pieces` lays its pieces
    /// out together), so any segment identifies it; MIT is the one the
    /// report named.
    fn vitals_strip(
        shapes: &[bevy_egui::egui::epaint::ClippedShape],
    ) -> (usize, bevy_egui::egui::Rect) {
        let index = shapes
            .iter()
            .position(|cs| {
                matches!(&cs.shape, bevy_egui::egui::Shape::Text(t) if t.galley.text().contains("MIT "))
            })
            .expect("the vitals strip was never painted");
        let quad = shapes[..index]
            .iter()
            .rev()
            .find_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Rect(r) if r.fill.a() > 0 => Some(r.rect),
                _ => None,
            })
            .expect("the vitals strip has no background quad ahead of it");
        (index, quad)
    }

    /// Draws the map screen and returns the shapes, with the log collapsed
    /// or expanded.
    fn playing_shapes(expanded: bool) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
        let mut app = playing_app();
        app.log_expanded = expanded;
        let mut fx = Fx::new();
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| {
            draw_playing_base(&mut app, &mut fx, None, p, &m);
        });
        shapes
    }

    /// **The vitals strip is the last thing painted over its own line, and
    /// this asserts that against *every* quad that follows it.**
    ///
    /// The strip rides a pane border, and `border_strip` centres its
    /// background quad *on* that line — so it reaches `size/2 + pad/2` past
    /// the border on both sides, into whatever the next pane fills.
    ///
    /// **The predecessor of this test named one such quad and was
    /// effectively vacuous.** It compared the strip's own quad against
    /// `log_pane.y`, the log pane's *body fill*, and knew nothing about the
    /// filter strip that used to ride the log pane's top border — whose
    /// quad reached `size/2 + pad/2` *above* that line and covered the
    /// lower half of the vitals glyphs, baseline included, while the
    /// arithmetic it asserted still held. So the question is not "does one
    /// named rect clear it" but "does anything painted after it land on
    /// it", which is what this walks.
    #[test]
    fn nothing_paints_over_the_vitals_strip() {
        let shapes = playing_shapes(false);
        let (index, quad) = vitals_strip(&shapes);
        let covered = covering_rect_after(&shapes, index, quad);
        assert!(
            covered.is_none(),
            "an opaque rect at {covered:?} painted over the vitals strip's \
             {quad:?} — the strip is cut by whatever fills that box"
        );
    }

    /// **The second complaint, and the reason the strip moved.** With the
    /// log expanded (SPACE, `App::log_expanded`) the pane grows upward over
    /// the bottom of the map as an overlay — so a strip riding the *map*
    /// pane's bottom border vanished entirely for as long as the log was
    /// open. Riding the *log* pane's top border instead, it travels with
    /// the pane and stays on screen in both states.
    ///
    /// Asserted as "painted, and nothing painted over it": the text alone
    /// is drawn either way, so a test that only looked for it passes
    /// against the bug it exists to catch.
    #[test]
    fn the_vitals_strip_survives_an_expanded_log() {
        let shapes = playing_shapes(true);
        let text = painted_text(&shapes).join(" ");
        assert!(
            text.contains("MIT "),
            "the vitals strip is not on screen with the log expanded: {text:?}"
        );
        let (index, quad) = vitals_strip(&shapes);
        let covered = covering_rect_after(&shapes, index, quad);
        assert!(
            covered.is_none(),
            "with the log expanded, an opaque rect at {covered:?} painted over \
             the vitals strip's {quad:?}"
        );
    }

    /// **The census the design names.** The badge, the tab marker and the
    /// collapsed bar are three readouts of one call, so they cannot
    /// disagree — and both halves are asserted, because either alone passes
    /// against a surface drawing a constant.
    ///
    /// The nagging half is reached through the save round-trip rather than
    /// by writing a component: `Game`'s `world` is private from out here and
    /// deliberately stays that way (`CLAUDE.md`'s architectural rule), so a
    /// renderer test reaches game state the way a player would — through a
    /// save.
    #[test]
    fn attention_drives_all_three_markers() {
        let m = ui_metrics(900.0);
        let mut fx = Fx::new();

        let mut calm = playing_app();
        assert!(
            calm.game.as_mut().unwrap().attention().is_empty(),
            "the calm fixture has something to say"
        );
        let (_, shapes) = with_painter(|p| {
            draw_playing_base(&mut calm, &mut fx, None, p, &m);
        });
        let text = painted_text(&shapes).join(" ");
        assert!(text.contains("ALL NOMINAL"), "no calm badge: {text:?}");
        assert!(
            crate::paint::painted_runs_in(&shapes, hud::palette::ATTENTION, true).is_empty(),
            "a calm base wears a mark with nothing to mark"
        );

        let path = std::env::temp_dir().join(format!(
            "fp_gui_attention_census_{}.sav",
            std::process::id()
        ));
        calm.game.as_mut().unwrap().save(&path).unwrap();
        let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
        data.player.perk_points = 4;
        feral_processes_engine::save::save_to_file(&path, &data).unwrap();
        let mut nagged = playing_app();
        nagged.game = Some(Game::load(&path, &test_assets()).unwrap());
        let _ = std::fs::remove_file(&path);

        let row = nagged
            .game
            .as_mut()
            .unwrap()
            .attention()
            .into_iter()
            .next()
            .expect("four unspent perk points is something to say");
        // A perk point lands in CREW, so open BASE and read the *closed*
        // pane's marker and bar — the state the whole design is about.
        nagged.info_tab = feral_processes_app_core::InfoTab::Base;

        let (_, shapes) = with_painter(|p| {
            draw_playing_base(&mut nagged, &mut fx, None, p, &m);
        });
        let text = painted_text(&shapes).join(" ");
        assert!(
            text.contains(&row.text.to_uppercase()),
            "the badge is silent: {text:?}"
        );
        assert!(
            text.contains(&row.text),
            "the closed pane hid the condition: {text:?}"
        );
        assert!(
            !crate::paint::painted_runs_in(&shapes, hud::palette::ATTENTION, true).is_empty(),
            "no tab wears a mark"
        );
        assert!(
            !text.contains("ALL NOMINAL"),
            "the bar claims calm while nagging: {text:?}"
        );
    }

    /// The column does not scroll, so a row past the bottom is dropped in
    /// silence — `the_tallest_gear_page_fits_its_popup`'s trap in a taller
    /// box. Measured at the smallest supported window, against the fixed
    /// head of the panel: the bars, the stat block, the party and pet
    /// headings and every party row. The inventory list below them clips
    /// itself against the same floor and is not part of this.
    #[test]
    fn the_tallest_column_pane_fits_its_column() {
        let m = ui_metrics(720.0);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let r = hud::layout::regions(1280.0, 720.0, char_w, &m, false);
            let body = hud::column::regions(r.info_column, &m).body;

            // Two bars, the level/zone/position block, four stat rows, the
            // mining row, the two headings, and a full party under them.
            let head =
                2.0 + 3.0 + 4.0 + 1.0 + 2.0 + feral_processes_engine::tuning::MAX_PARTY_SIZE as f32;
            let rows = (body.h - m.inset) / m.line_height;
            assert!(
                rows >= head,
                "the column fits {rows:.1} rows and the panel's head is {head} —                  the overflow is dropped in silence"
            );
        });
    }

    /// `draw_playing_base` no longer computes its own rects — it reads them
    /// from `hud::layout::regions` once, at the top. This is the trap named
    /// in the module's own doc comment: a literal `0.0` for a y-origin draws
    /// under the status bar and no test sees it, so the regions are asserted
    /// against each other here rather than trusted by eye.
    #[test]
    fn the_playing_screen_draws_inside_its_regions() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let r = hud::layout::regions(1440.0, 900.0, char_w, &m, false);
            assert!(
                r.map_pane.x + r.map_pane.w <= r.info_column.x,
                "the map pane runs into the info column"
            );
            assert!(
                r.map_pane.y >= r.status_bar.y + r.status_bar.h,
                "the map pane starts above the status bar's bottom edge"
            );
        });
    }

    /// Draws one surface map and reports what landed: the textured meshes,
    /// and the text of every glyph painted.
    fn drawn_map(sprites: SpriteTable) -> (usize, Vec<String>) {
        drawn_map_with(sprites, 0)
    }

    /// How far the party has to walk before the anchor it spawned on top of
    /// is a cell of its own. One step is enough; the constant is named so
    /// the assertion above can say what zero means.
    const STEPS_OFF_THE_ANCHOR: usize = 1;

    /// `drawn_map`, with the party walked `steps` tiles east first.
    fn drawn_map_with(sprites: SpriteTable, steps: usize) -> (usize, Vec<String>) {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
            .expect("the shipped assets must load");
        for _ in 0..steps {
            game.move_player(1, 0);
        }
        let mut fx = Fx::new();
        let (tile_px, glyph_px) = crate::text::map_cell(1);
        let (_, shapes) = with_sprites(sprites, |p| {
            let status = game.player_status();
            draw_surface_map(
                &mut game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                status.position,
            );
        });
        (painted_images(&shapes).len(), painted_text(&shapes))
    }

    /// `drawn_map`, but the game starts from a character-creation `choice`
    /// rather than `CharacterChoice::default()` — the sprite-name and
    /// colour tests need a player whose `look` carries something other than
    /// the empty string and zero every other test's default game has.
    fn drawn_map_with_choice(
        sprites: SpriteTable,
        choice: &CharacterChoice,
    ) -> (usize, Vec<String>) {
        let (images, glyphs) = drawn_map_images(sprites, choice);
        (images.len(), glyphs)
    }

    /// `drawn_map_with_choice`, keeping the textured meshes themselves
    /// rather than counting them — which texture was drawn, and in what
    /// tint, is the whole question for the drawn icon's two rules.
    #[allow(clippy::type_complexity)]
    fn drawn_map_images(
        sprites: SpriteTable,
        choice: &CharacterChoice,
    ) -> (
        Vec<(
            bevy_egui::egui::TextureId,
            bevy_egui::egui::Rect,
            bevy_egui::egui::Color32,
        )>,
        Vec<String>,
    ) {
        let mut game = Game::new_with(7, DifficultyMode::Forgiving, &test_assets(), choice)
            .expect("the shipped assets must load");
        let mut fx = Fx::new();
        let (tile_px, glyph_px) = crate::text::map_cell(1);
        let (_, shapes) = with_sprites(sprites, |p| {
            let status = game.player_status();
            draw_surface_map(
                &mut game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                status.position,
            );
        });
        (painted_images(&shapes), painted_text(&shapes))
    }

    /// A drawing with one lit pixel — non-blank, which is all the map cares
    /// about, since the pixels themselves live in the texture the upload
    /// built and never reach this code.
    fn a_drawing() -> feral_processes_engine::PlayerIcon {
        let mut icon = feral_processes_engine::PlayerIcon::default();
        icon.set(3, 4, 7);
        icon
    }

    /// **The overdraw rule, for the rung most exposed to it.** A drawn icon
    /// is transparent wherever the player left the canvas bare — every icon
    /// is — so a renderer that painted the texture over an `@` that was
    /// still there would show the glyph through its own avatar. Against the
    /// opaque placeholder art that failure is invisible, which is why both
    /// halves are asserted in one test: the mesh landed, **and** the `@`
    /// did not.
    #[test]
    fn the_drawn_icon_stands_in_for_the_at_sign() {
        let choice = CharacterChoice {
            icon: Some(a_drawing()),
            ..CharacterChoice::default()
        };
        let mut table = SpriteTable::default();
        table.insert(
            crate::sprites::DRAWN_ICON_KEY,
            bevy_egui::egui::TextureId::User(9),
        );

        let (images, glyphs) = drawn_map_with_choice(table, &choice);

        assert_eq!(images, 1, "exactly one sprite, the player's drawing");
        assert!(
            !glyphs.iter().any(|g| g == "@"),
            "the '@' must give way to the drawing, not sit under it: {glyphs:?}"
        );
    }

    /// The drawing is the **top** rung: a player who drew one and also
    /// carries a named sprite sees the drawing. Both keys are in the table,
    /// so nothing here can pass by a lookup that merely missed.
    #[test]
    fn the_drawn_icon_outranks_the_named_sprite() {
        let choice = CharacterChoice {
            sprite: "hero".to_string(),
            icon: Some(a_drawing()),
            ..CharacterChoice::default()
        };
        let mut table = SpriteTable::default();
        table.insert(
            crate::sprites::DRAWN_ICON_KEY,
            bevy_egui::egui::TextureId::User(9),
        );
        table.insert("hero", bevy_egui::egui::TextureId::User(4));

        let (images, _) = drawn_map_images(table, &choice);

        assert_eq!(images.len(), 1, "one tile draws one sprite");
        assert_eq!(
            images[0].0,
            bevy_egui::egui::TextureId::User(9),
            "the drawing must win the tile from the named sprite"
        );
    }

    /// **The drawn icon is the one sprite in the game drawn untinted.**
    /// Every other sprite is authored near-white and multiplied by the hue
    /// its tile would have worn; a drawing carries its own fifteen colours,
    /// and multiplying those by an indigo swatch turns most of them black.
    /// The tint is therefore neutral — grey at the vignette's own value, so
    /// depth shading is kept and the hue is dropped.
    #[test]
    fn the_drawn_icon_is_drawn_untinted() {
        let choice = CharacterChoice {
            colour: Some(3),
            icon: Some(a_drawing()),
            ..CharacterChoice::default()
        };
        let mut table = SpriteTable::default();
        table.insert(
            crate::sprites::DRAWN_ICON_KEY,
            bevy_egui::egui::TextureId::User(9),
        );

        let (images, _) = drawn_map_images(table, &choice);

        let tint = images
            .iter()
            .find(|(id, _, _)| *id == bevy_egui::egui::TextureId::User(9))
            .expect("the drawing must be painted")
            .2;
        assert_eq!(
            tint.r(),
            tint.g(),
            "a neutral tint has equal channels, not a hue: {tint:?}"
        );
        assert_eq!(
            tint.g(),
            tint.b(),
            "a neutral tint has equal channels, not a hue: {tint:?}"
        );
        assert!(
            tint.r() > 0,
            "the vignette must scale the tint, not erase it: {tint:?}"
        );
    }

    /// The rung under the drawing must not regress: a player who drew
    /// nothing keeps their named sprite, **even when the table still holds
    /// a drawing**. Which rung runs is read off the player's own look, not
    /// off whatever the sprite table happens to be carrying.
    #[test]
    fn a_player_with_no_drawing_still_draws_the_named_sprite() {
        let choice = CharacterChoice {
            sprite: "hero".to_string(),
            icon: None,
            ..CharacterChoice::default()
        };
        let mut table = SpriteTable::default();
        table.insert(
            crate::sprites::DRAWN_ICON_KEY,
            bevy_egui::egui::TextureId::User(9),
        );
        table.insert("hero", bevy_egui::egui::TextureId::User(4));

        let (images, glyphs) = drawn_map_images(table, &choice);

        assert_eq!(images.len(), 1, "one tile draws one sprite");
        assert_eq!(
            images[0].0,
            bevy_egui::egui::TextureId::User(4),
            "no drawing means the named sprite still owns the tile"
        );
        assert!(
            !glyphs.iter().any(|g| g == "@"),
            "the named sprite still stands in for the glyph: {glyphs:?}"
        );
    }

    /// The bottom rung: neither a drawing nor a sprite the table knows
    /// leaves the `@`. `assets/sprites/` deleted is still the glyph map.
    #[test]
    fn a_player_with_neither_still_draws_the_glyph() {
        let choice = CharacterChoice {
            sprite: "no-such-sprite".to_string(),
            icon: None,
            ..CharacterChoice::default()
        };

        let (images, glyphs) = drawn_map_with_choice(SpriteTable::default(), &choice);

        assert_eq!(images, 0, "nothing in the table must paint no texture");
        assert!(
            glyphs.iter().any(|g| g == "@"),
            "the glyph is what is left when neither rung above it draws: {glyphs:?}"
        );
    }

    /// The player's sprite stands in for the player's glyph — it does not
    /// draw beside it. Both halves are asserted in one test on purpose: the
    /// sprite half alone passes against a renderer that paints the texture
    /// over an '@' that is still there, which on white placeholder art looks
    /// exactly right and is wrong the moment the sprite has any transparency.
    ///
    /// Deliberately runs against a **bare `Game::new`** — no explicit
    /// choice — because that is what every save written before the wizard
    /// existed, every `dev-saves/` template, and any run that skips the
    /// Look step loads as. Handing it a choice that names `"player"` is the
    /// accommodation that let the default lose the shipped art in silence
    /// once already; `the_player_sprite_comes_from_the_choice` is what
    /// covers a wizard-authored name.
    #[test]
    fn the_player_sprite_stands_in_for_the_at_sign() {
        let mut table = SpriteTable::default();
        table.insert(
            feral_processes_engine::DEFAULT_PLAYER_SPRITE,
            bevy_egui::egui::TextureId::User(1),
        );

        let (images, glyphs) = drawn_map(table);

        assert_eq!(images, 1, "exactly one sprite, the player's");
        assert!(
            !glyphs.iter().any(|g| g == "@"),
            "the '@' must give way to the sprite, not sit under it: {glyphs:?}"
        );
    }

    /// The sprite name is not tied to the literal `"player"` — any name the
    /// wizard wrote into `PlayerIdentity::sprite` must reach the map the
    /// same way, or the wizard's other icon options would be dead choices.
    #[test]
    fn the_player_sprite_comes_from_the_choice() {
        let choice = CharacterChoice {
            sprite: "hero".to_string(),
            ..CharacterChoice::default()
        };
        let mut table = SpriteTable::default();
        table.insert("hero", bevy_egui::egui::TextureId::User(4));

        let (images, glyphs) = drawn_map_with_choice(table, &choice);

        assert_eq!(images, 1, "exactly one sprite, the chosen one");
        assert!(
            !glyphs.iter().any(|g| g == "@"),
            "the '@' must give way to the chosen sprite, not sit under it: {glyphs:?}"
        );
    }

    /// A name the sprite table has nothing under returns `false` and the
    /// caller draws the glyph — the fallback that keeps `assets/sprites/`
    /// optional, exercised here against a *chosen* name rather than the
    /// empty one a default game carries, so this cannot pass by accident on
    /// an empty-string lookup that was never attempted.
    #[test]
    fn a_missing_sprite_falls_back_to_the_chosen_glyph() {
        let choice = CharacterChoice {
            sprite: "no-such-sprite".to_string(),
            ..CharacterChoice::default()
        };

        let (images, glyphs) = drawn_map_with_choice(SpriteTable::default(), &choice);

        assert_eq!(
            images, 0,
            "a name the table has nothing under must paint no texture"
        );
        assert!(
            glyphs.iter().any(|g| g == "@"),
            "the glyph must still draw when the chosen sprite is missing: {glyphs:?}"
        );
    }

    /// The chosen colour comes off `PlayerLook`, **0-indexed** —
    /// `PLAYER_CHOICES[colour]` — and is what the map actually draws, not
    /// just state nothing reads: `the_players_glyph_wears_the_player_role`
    /// already covers `None` staying `PLAYER`, so this is the other half —
    /// an explicit pick has to move the drawn colour off it.
    #[test]
    fn the_players_glyph_wears_the_chosen_colour() {
        let choice = CharacterChoice {
            colour: Some(2),
            ..CharacterChoice::default()
        };
        let mut game = Game::new_with(7, DifficultyMode::Forgiving, &test_assets(), &choice)
            .expect("the shipped assets must load");
        let mut fx = Fx::new();
        let (tile_px, glyph_px) = crate::text::map_cell(1);
        let (_, shapes) = with_painter(|p| {
            let status = game.player_status();
            draw_surface_map(
                &mut game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                status.position,
            );
        });
        let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
        let at = crate::paint::painted_map_glyphs(&shapes);
        let (_, drawn) = at
            .iter()
            .find(|(g, _)| g == "@")
            .expect("the map draws the player");
        let want = hud::palette::PLAYER_CHOICES[2];
        assert!(
            dist(*drawn, want) < 0.06,
            "colour Some(2) painted {drawn:?}, which is {:.3} from PLAYER_CHOICES[2] {want:?}",
            dist(*drawn, want)
        );
        assert!(
            dist(*drawn, hud::palette::PLAYER) > 0.06,
            "a chosen colour must not still read as PLAYER"
        );
    }

    /// **The whole reason `colour` is an `Option` and not a reserved zero.**
    /// The first swatch on the wizard's own screen is index `0`, and under a
    /// one-indexed `u8` picking it stored the same value "no choice was
    /// made" carries — so the player's first pick painted `PLAYER` and read
    /// as the key doing nothing. Both halves are asserted: the drawn colour
    /// is `PLAYER_CHOICES[0]`, *and* it is not `PLAYER`.
    #[test]
    fn the_first_swatch_is_not_the_player_role() {
        let choice = CharacterChoice {
            colour: Some(0),
            ..CharacterChoice::default()
        };
        let mut game = Game::new_with(7, DifficultyMode::Forgiving, &test_assets(), &choice)
            .expect("the shipped assets must load");
        let mut fx = Fx::new();
        let (tile_px, glyph_px) = crate::text::map_cell(1);
        let (_, shapes) = with_painter(|p| {
            let status = game.player_status();
            draw_surface_map(
                &mut game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                status.position,
            );
        });
        let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
        let at = crate::paint::painted_map_glyphs(&shapes);
        let (_, drawn) = at
            .iter()
            .find(|(g, _)| g == "@")
            .expect("the map draws the player");
        let want = hud::palette::PLAYER_CHOICES[0];
        assert!(
            dist(*drawn, want) < 0.06,
            "the first swatch painted {drawn:?}, which is {:.3} from PLAYER_CHOICES[0] {want:?}",
            dist(*drawn, want)
        );
        assert!(
            dist(*drawn, hud::palette::PLAYER) > 0.06,
            "picking the first swatch must not read as no choice at all"
        );
    }

    /// The anchor gets the same treatment, and it needs its own test: it is
    /// the one map fixture that is neither a creature nor a `Structure`, so
    /// nothing the player's case exercises picks it out.
    ///
    /// The party has to step off it first. `Game::new` spawns the anchor
    /// *under* the player, and the player wins a shared tile, so a fresh run
    /// draws no anchor at all — asserted here rather than assumed, or the
    /// test would pass against a renderer that never selected the sprite.
    #[test]
    fn the_anchor_sprite_stands_in_for_the_hash() {
        let mut table = SpriteTable::default();
        table.insert("anchor", bevy_egui::egui::TextureId::User(2));

        let (under, _) = drawn_map_with(table.clone(), 0);
        assert_eq!(
            under, 0,
            "the party stands on the anchor at spawn and hides it, so this \
             fixture proves nothing until it moves"
        );

        let (images, glyphs) = drawn_map_with(table, STEPS_OFF_THE_ANCHOR);

        assert_eq!(images, 1, "exactly one sprite, the anchor's");
        assert!(
            !glyphs.iter().any(|g| g == "#"),
            "the '#' must give way to the sprite, not sit under it: {glyphs:?}"
        );
    }

    /// ...and with nothing loaded the map is exactly what it was. This is
    /// what makes `assets/sprites/` deletable, the same supported way
    /// `assets/sectors/` is.
    #[test]
    fn an_empty_sprite_table_leaves_the_glyph_map_alone() {
        let (images, glyphs) = drawn_map(SpriteTable::default());

        assert_eq!(images, 0, "nothing loaded must paint no texture at all");
        assert!(
            glyphs.iter().any(|g| g == "@"),
            "the player must still be drawn as a glyph: {glyphs:?}"
        );
    }

    /// The zone map drawn at `at` seconds, with effects live. The party is
    /// left standing where a new run puts it, on the surface — which is the
    /// half of the cloud gate this fixture exists to see.
    fn drawn_surface_at(at: f64) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
            .expect("the shipped assets must load");
        let mut fx = Fx::new();
        fx.begin_frame(at, Vec::new(), Vec::new(), false);
        let (tile_px, glyph_px) = crate::text::map_cell(1);
        let (_, shapes) = with_painter(|p| {
            let status = game.player_status();
            draw_surface_map(
                &mut game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                status.position,
            );
        });
        shapes
    }

    /// The two halves of the cloud gate, and they are asserted together on
    /// purpose. The zone-map half alone passes against a renderer that
    /// shadows base space too — where a drifting patch would read as the
    /// ceiling of a pocket underground doing something it cannot do.
    #[test]
    fn clouds_drift_over_the_zone_map_and_never_over_base_space() {
        let (surface_early, _) = painted_fills_and_strokes(&drawn_surface_at(0.0));
        let (surface_later, _) = painted_fills_and_strokes(&drawn_surface_at(60.0));
        assert_ne!(
            surface_early, surface_later,
            "the ground must be shaded differently a minute of wind later"
        );

        let (base_early, _) = painted_fills_and_strokes(&drawn_base_at(0.0, false, true));
        let (base_later, _) = painted_fills_and_strokes(&drawn_base_at(60.0, false, true));
        assert_eq!(
            base_early, base_later,
            "base space has no sky and must not change with the clock"
        );
    }

    /// A base with one request filed on the cell east of the party, drawn
    /// once. Returns the shapes so a caller can ask what landed.
    ///
    /// The party is stood **in base space**, which is the whole point: the
    /// site's `Position` is a base-space cell, and `view_entities` selects
    /// on `Game::stands_in_base_space` — drawn from the surface, nothing
    /// here would be on the pane at all.
    fn drawn_base(at: f64, request: bool) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
        drawn_base_at(at, request, true)
    }

    /// `drawn_base` with the animation switch, so a test can ask where
    /// something *rests* as well as where it is this frame.
    fn drawn_base_at(
        at: f64,
        request: bool,
        animated: bool,
    ) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets())
            .expect("the shipped assets must load");
        game.place_structure("home", 0, 0)
            .expect("a Home founds it");
        game.enter_base().expect("the party steps inside");
        if request {
            // No materials are given, and none are needed: filing charges
            // nothing. That is not incidental to this fixture — it is why a
            // renderer test can reach a build site at all without the engine
            // exposing its `World`.
            game.place_structure("mining_node", 1, 0)
                .expect("a request is filed beside the party");
        }

        let mut fx = Fx::new();
        fx.enabled = animated;
        fx.begin_frame(at, Vec::new(), Vec::new(), false);
        let (tile_px, glyph_px) = crate::text::map_cell(1);
        let (_, shapes) = with_painter(|p| {
            let status = game.player_status();
            draw_surface_map(
                &mut game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                status.position,
            );
        });
        shapes
    }

    /// Every rect fill and every rect stroke a frame painted.
    fn painted_fills_and_strokes(
        shapes: &[bevy_egui::egui::epaint::ClippedShape],
    ) -> (Vec<bevy_egui::egui::Color32>, Vec<bevy_egui::egui::Color32>) {
        let mut fills = Vec::new();
        let mut strokes = Vec::new();
        for cs in shapes {
            if let bevy_egui::egui::Shape::Rect(r) = &cs.shape {
                fills.push(r.fill);
                if r.stroke.width > 0.0 {
                    strokes.push(r.stroke.color);
                }
            }
        }
        (fills, strokes)
    }

    /// A request draws as a slab, an edge and a caret — all three, because
    /// each carries a different half of the message and any one alone reads
    /// as something else.
    ///
    /// The slab alone is an unlit tile; the caret alone is a glyph standing
    /// on bare ground, which is what every *creature* on this map looks like.
    ///
    /// **Differential against the same base with nothing on order**, and
    /// that is load-bearing rather than tidy. Written as "is there a greyish
    /// rect somewhere", this test passed with the slab draw deleted
    /// outright — the map paints plenty of grey. What only a build site can
    /// produce is a fill and a stroke the identical frame without one never
    /// paints at all.
    #[test]
    fn a_pending_build_site_draws_a_slab_an_edge_and_a_caret() {
        let with = drawn_base(0.0, true);
        let without = drawn_base(0.0, false);
        let (with_fills, with_strokes) = painted_fills_and_strokes(&with);
        let (bare_fills, bare_strokes) = painted_fills_and_strokes(&without);

        assert!(
            with_fills.iter().any(|c| !bare_fills.contains(c)),
            "the slab is a fill the same frame without a request never paints"
        );
        assert!(
            with_strokes.iter().any(|c| !bare_strokes.contains(c)),
            "and its edge is a stroke that frame never paints either"
        );
        assert!(
            crate::paint::painted_text(&with).iter().any(|g| g == "^"),
            "the caret is what says work is happening here: {:?}",
            crate::paint::painted_text(&with)
        );
        assert!(
            !crate::paint::painted_text(&without)
                .iter()
                .any(|g| g == "^"),
            "and it is drawn only where a request stands"
        );
    }

    /// ...and the caret bounces.
    ///
    /// **The motion is the point**, not decoration: a build site is the one
    /// cell on this map where "nothing is happening yet" is the wrong
    /// reading, and a still caret says exactly that. Two frames at different
    /// times, comparing where the glyph landed — a test that only checks the
    /// caret exists passes against a renderer that pinned it.
    /// Where the caret landed in a frame.
    fn caret_y(shapes: &[bevy_egui::egui::epaint::ClippedShape]) -> f32 {
        shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Text(t) if t.galley.text() == "^" => Some(t.pos.y),
                _ => None,
            })
            .expect("the caret is drawn")
    }

    #[test]
    fn the_caret_over_a_build_site_bounces() {
        // A quarter of the bob's period apart, so the raised cosine is at
        // genuinely different heights rather than at two points that happen
        // to share one.
        let (a, b) = (
            caret_y(&drawn_base(0.0, true)),
            caret_y(&drawn_base(0.25, true)),
        );
        assert_ne!(
            a, b,
            "the caret must sit at different heights across frames — a still one reads as a \
             glyph standing on the ground"
        );
    }

    /// ...and it bounces *around* the middle of its slab, not on top of it.
    ///
    /// The caret rides the same raised cosine the staffed mark does, and
    /// that one is upward-only by design — its rest position is an inset off
    /// the tile's bottom edge that a down-swing would spend. The caret's
    /// rest position is the centre of its own slab, so anchored the same way
    /// it spent its whole cycle at or above centre and read as sitting high
    /// in the tile.
    ///
    /// Two frames **half a period** apart, whose mean is the resting
    /// position for any phase, against a frame with animation off — which is
    /// what puts the caret at rest. Asserting only that the two frames
    /// straddle *each other* would pass against the old upward-only bob.
    #[test]
    fn the_caret_bounces_around_the_middle_of_its_slab() {
        let rest = caret_y(&drawn_base_at(0.0, true, false));
        // Half a period apart at the bob's 1 Hz, which is what makes the
        // mean below the rest position for *any* entity phase.
        let (a, b) = (
            caret_y(&drawn_base_at(0.0, true, true)),
            caret_y(&drawn_base_at(0.5, true, true)),
        );

        assert!(
            ((a + b) / 2.0 - rest).abs() < 0.01,
            "the swing must be centred on the caret's rest position: {a} and {b} about {rest}"
        );
        assert!(
            (a - rest) * (b - rest) < 0.0,
            "and it must genuinely cross that position rather than sit to one side of it: \
             {a} and {b} about {rest}"
        );
    }

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

    fn entry(text: &str, repeats: usize) -> LogEntry {
        LogEntry {
            kind: MessageKind::Info,
            source: MessageSource::Field,
            text: text.to_string(),
            repeats,
        }
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
        assert_eq!(vignette(0.0, 0.0, 400.0, 300.0, VIGNETTE_FLOOR_FULL), 1.0);
    }

    #[test]
    fn the_vignette_bottoms_out_at_its_floor_and_never_below() {
        let f = VIGNETTE_FLOOR_FULL;
        assert!((vignette(400.0, 0.0, 400.0, 300.0, f) - f).abs() < 1e-6);
        // The corners sit past the unit radius and must clamp rather than
        // keep darkening.
        assert!(vignette(400.0, 300.0, 400.0, 300.0, f) >= f);
        assert!(vignette(9999.0, 9999.0, 400.0, 300.0, f) >= f);
    }

    #[test]
    fn the_vignette_darkens_monotonically_outward() {
        let mut previous = f32::MAX;
        for i in 0..=20 {
            let v = vignette(i as f32 * 20.0, 0.0, 400.0, 300.0, VIGNETTE_FLOOR_FULL);
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
        let small = vignette(100.0, 75.0, 400.0, 300.0, VIGNETTE_FLOOR_FULL);
        let large = vignette(200.0, 150.0, 800.0, 600.0, VIGNETTE_FLOOR_FULL);
        assert!((small - large).abs() < 1e-6, "{small} vs {large}");
    }

    /// The whole of what Power buys: a full reserve is the vignette this map
    /// has always drawn, and draining it deepens the corners and nothing
    /// else.
    #[test]
    fn the_vignette_floor_deepens_as_the_reserve_drains() {
        assert_eq!(vignette_floor(POWER_MAX), VIGNETTE_FLOOR_FULL);
        assert_eq!(vignette_floor(0.0), VIGNETTE_FLOOR_EMPTY);

        let mut previous = f32::MAX;
        for i in 0..=10 {
            let f = vignette_floor(POWER_MAX * (10 - i) as f32 / 10.0);
            assert!(
                f <= previous,
                "brightened at step {i}: {f} after {previous}"
            );
            previous = f;
        }
    }

    /// `PowerReserve` clamps itself, but this reads a `PlayerStatus` field
    /// rather than the type, and a floor that ran past either constant would
    /// either blow out the centre or drive the corners toward black.
    #[test]
    fn the_vignette_floor_is_clamped_at_both_ends() {
        assert_eq!(vignette_floor(POWER_MAX * 4.0), VIGNETTE_FLOOR_FULL);
        assert_eq!(vignette_floor(-40.0), VIGNETTE_FLOOR_EMPTY);
    }

    /// It stays a *vignette* at every reserve: the pane's middle is untouched
    /// whatever the floor, so a drained reserve costs the player nothing at
    /// the centre of their own attention. A version that dimmed the whole
    /// pane would be a wash, and would read as the renderer having faulted
    /// rather than as the reserve running down.
    #[test]
    fn a_drained_reserve_darkens_the_pane_edge_and_not_its_centre() {
        let full = vignette_floor(POWER_MAX);
        let empty = vignette_floor(0.0);

        assert_eq!(vignette(0.0, 0.0, 400.0, 300.0, full), 1.0);
        assert_eq!(vignette(0.0, 0.0, 400.0, 300.0, empty), 1.0);
        assert!(
            vignette(400.0, 0.0, 400.0, 300.0, empty) < vignette(400.0, 0.0, 400.0, 300.0, full),
            "an empty reserve must darken the edge"
        );
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
        let staffed_y = staffed_mark_rect(px, py, tile_px, 0.0).y;

        assert!(
            nemesis.y + nemesis.h < staffed_y,
            "the nemesis mark's bottom ({}) reaches into the staffed mark's \
             row (starts at {staffed_y})",
            nemesis.y + nemesis.h
        );
    }

    /// **The whole camera feature is one value.** `draw_surface_map` takes
    /// the tile it centres on rather than deriving it, so the watched
    /// program's cell moves the tile window, the entity window and the
    /// camera's own slide together — and the `@`, which is the one glyph
    /// that must end up drawn *off* centre.
    ///
    /// Asserted as a shift rather than an absolute position: the pane's
    /// centre is a function of the window size and the strip insets, and a
    /// test that recomputed it would be re-deriving the layout it is meant
    /// to be checking.
    #[test]
    fn the_map_draws_around_the_tile_it_is_handed() {
        let mut app = playing_app();
        let pane = Rect::new(0.0, 0.0, 800.0, 500.0);
        let (tile_px, glyph_px) = map_cell(app.zoom);
        let home = app.game.as_ref().unwrap().player_status().position;

        let mut at = |center: (i32, i32)| {
            let mut fx = Fx::new();
            let game = app.game.as_mut().unwrap();
            let status = game.player_status();
            let (_, shapes) = with_painter(|p| {
                // Twice, because the camera eases toward its target: the
                // first call seeds it and the second is drawn from a camera
                // that has arrived.
                for _ in 0..2 {
                    draw_surface_map(
                        game, &mut fx, p, pane, tile_px, glyph_px, &status, None, center,
                    );
                }
            });
            crate::paint::painted_text_boxes(&shapes)
                .into_iter()
                .find(|(_, text, _)| text == "@")
                .map(|(_, _, rect)| rect)
                .expect("the map draws the player")
        };

        let anchored = at(home);
        let shifted = at((home.0 + 3, home.1));

        assert!(
            (anchored.x - shifted.x - tile_px * 3.0).abs() < 1.0,
            "centring three tiles east must push the `@` three tiles west: \
             {} then {}, against a {tile_px}px tile",
            anchored.x,
            shifted.x
        );
        assert!(
            (anchored.y - shifted.y).abs() < 1.0,
            "an axis that was not moved must not drift: {} then {}",
            anchored.y,
            shifted.y
        );
    }

    /// The watch line replaces the ground readout rather than claiming a
    /// border of its own. `map_pane`'s bottom border carries nothing by
    /// design — a strip there would either cover the map's bottom row of
    /// tiles or make the grid re-lay itself the moment `w` was pressed — and
    /// the ground readout is ambient, where "you are looking somewhere else,
    /// and here is the way back" is not.
    #[test]
    fn the_frame_says_who_is_being_watched_instead_of_the_ground() {
        let mut app = playing_app();
        let m = ui_metrics(900.0);
        let ground = ground_label(&mut app);
        let row = app.game.as_mut().unwrap().terrain_row();
        let threat = hud::map_frame::Threat {
            hostiles: 0,
            shielded: false,
        };
        let pane = Rect::new(0.0, 0.0, 1200.0, 600.0);

        let (_, shapes) = with_painter(|p| {
            hud::map_frame::draw_map_frame(pane, row, threat, Some("Ivy"), p, &m);
        });
        let text = crate::paint::painted_text(&shapes).join("");
        assert!(
            text.contains("Ivy"),
            "the strip must name who the camera is on: {text:?}"
        );
        assert!(
            text.contains("Esc"),
            "and the way back, or the camera is a trap: {text:?}"
        );
        assert!(
            !text.contains(&ground),
            "the ground readout gives up the mount rather than sharing it: \
             {text:?} still holds {ground:?}"
        );

        let (_, shapes) = with_painter(|p| {
            hud::map_frame::draw_map_frame(pane, row, threat, None, p, &m);
        });
        let text = crate::paint::painted_text(&shapes).join("");
        assert!(
            text.contains(&ground) && !text.contains("Esc"),
            "and takes it straight back when the watch ends: {text:?}"
        );
    }

    /// The con read's bar is the rarity bar turned upside down: same
    /// thickness, same full width, opposite edge. Two bars framing the tile
    /// is what lets the glyph between them go back to saying *what* the
    /// program is — so they must never meet, at any zoom.
    #[test]
    fn the_difficulty_bar_hugs_the_bottom_edge_and_never_meets_the_rarity_bar() {
        for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
            let bar = difficulty_bar_rect(100.0, 200.0, tile_px);

            assert_eq!(bar.h, RARITY_BAR_PX, "at tile_px={tile_px}");
            assert_eq!(bar.w, tile_px - 1.0, "at tile_px={tile_px}");
            assert_eq!(
                bar.y + bar.h,
                200.0 + tile_px - 1.0,
                "at tile_px={tile_px} the bar must sit flush on the bottom edge"
            );
            assert!(
                bar.y > 200.0 + RARITY_BAR_PX,
                "at tile_px={tile_px} the two bars reach each other"
            );
        }
    }

    /// `nemesis_mark_rect` drops below the rarity bar; this is the same
    /// clearance at the other end. **The bob is what makes it worth a
    /// test**: `fx.staffed_bob` lifts the mark *away* from the bottom edge,
    /// so a missing offset is invisible while a machine is worked and shows
    /// only at rest — and a stranded mark, which never bobs at all, would
    /// sit under the bar for its whole stall.
    #[test]
    fn the_staffed_mark_clears_the_difficulty_bar_at_rest_and_mid_bob() {
        let tile_px = 40.0_f32;
        let (px, py) = (10.0_f32, 20.0_f32);
        let bar = difficulty_bar_rect(px, py, tile_px);

        for lift in [0.0_f32, 1.0, 3.0] {
            let mark = staffed_mark_rect(px, py, tile_px, lift);
            assert!(
                mark.y + mark.h <= bar.y,
                "at lift={lift} the staffed mark's bottom ({}) reaches into \
                 the difficulty bar (starts at {})",
                mark.y + mark.h,
                bar.y
            );
            assert!(mark.y > py, "at lift={lift} the mark left the tile");
        }
    }

    /// Where the con bar was painted this frame, or `None` if it was not.
    fn con_bar(
        shapes: &[bevy_egui::egui::epaint::ClippedShape],
    ) -> Option<(bevy_egui::egui::Rect, bevy_egui::egui::Color32)> {
        shapes.iter().find_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Rect(r)
                if r.fill.a() > 0 && (r.rect.height() - RARITY_BAR_PX).abs() < 0.01 =>
            {
                Some((r.rect, r.fill))
            }
            _ => None,
        })
    }

    /// `None` is *no reading*, not a reading worth nothing — the engine
    /// hands a con colour only for a hostile, so anything else must leave
    /// the bottom edge bare rather than paint a bar the player would read
    /// as "you can beat your own companion".
    #[test]
    fn nothing_draws_a_con_bar_without_a_con_read() {
        let (_, shapes) = with_painter(|p| {
            draw_difficulty_bar(p, None, 0.0, 0.0, CELL, 1.0);
        });

        assert!(
            con_bar(&shapes).is_none(),
            "a tile with no con read must paint no bar"
        );
    }

    /// The bar lands where `difficulty_bar_rect` says and wears the rung it
    /// was handed. Both in one test because either alone passes against a
    /// bar drawn in the wrong place *or* in the wrong colour, and the whole
    /// point of the channel is that a player reads position and hue
    /// together.
    #[test]
    fn a_con_read_draws_its_rung_along_the_bottom_edge() {
        let (_, shapes) = with_painter(|p| {
            draw_difficulty_bar(p, Some(GlyphColor::Red), 0.0, 0.0, CELL, 1.0);
        });

        let (rect, fill) = con_bar(&shapes).expect("a con read must paint a bar");
        let want = difficulty_bar_rect(0.0, 0.0, CELL);

        assert!(
            (rect.min.y - want.y).abs() < 0.01 && (rect.width() - want.w).abs() < 0.01,
            "the bar was painted at {rect:?}, not at {want:?}"
        );

        let red = hud::palette::glyph(GlyphColor::Red);
        let want_fill = bevy_egui::egui::Color32::from_rgb(
            (red.r * 255.0) as u8,
            (red.g * 255.0) as u8,
            (red.b * 255.0) as u8,
        );
        assert_eq!(
            (fill.r(), fill.g(), fill.b()),
            (want_fill.r(), want_fill.g(), want_fill.b()),
            "the bar must wear the con rung's own hue, drawn from the one \
             palette table `glyph_color` reads"
        );
    }

    /// The other end of `nemesis_mark_rect`'s clearance: the bottom-right
    /// corner has a bar under it now too, and a mark flush into a corner
    /// reads as painting back an edge `outline_open` left off.
    #[test]
    fn the_boss_mark_clears_the_difficulty_bar_and_stays_inside_the_tile() {
        for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
            let (px, py) = (100.0_f32, 200.0_f32);
            let mark = boss_mark_rect(px, py, tile_px);
            let bar = difficulty_bar_rect(px, py, tile_px);

            assert!(
                mark.y + mark.h <= bar.y,
                "at tile_px={tile_px} the boss mark reaches into the con bar"
            );
            assert!(
                mark.x + mark.w < px + tile_px - 1.0 && mark.x > px,
                "at tile_px={tile_px} the boss mark touches a side edge"
            );
            assert!(mark.w > 0.0, "at tile_px={tile_px} the mark has no size");
        }
    }

    /// **A creature can now be both.** The old `difficulty_color` returned a
    /// reserved hue for a nemesis *or* a boss and the nemesis won, so being
    /// both was one fact drawn and one dropped. They are two marks in two
    /// corners now, and nothing may make them fight for pixels.
    #[test]
    fn a_boss_that_is_also_a_nemesis_wears_both_marks_without_them_colliding() {
        let tile_px = 40.0_f32;
        let boss = boss_mark_rect(0.0, 0.0, tile_px);
        let nemesis = nemesis_mark_rect(0.0, 0.0, tile_px);

        assert!(
            boss.y > nemesis.y + nemesis.h,
            "the two identity marks overlap: boss at {boss:?}, nemesis at \
             {nemesis:?}"
        );
    }

    /// **The census.** A tile can wear the con bar, a rarity bar, a staffed
    /// or stranded mark, a nemesis mark and now a boss mark all at once, and
    /// every one of them is a different fact. The boss mark is the newest
    /// and so the one that has to prove it is not any of the others —
    /// against the same 0.10 channel-distance margin the con ladder's own
    /// census uses.
    #[test]
    fn the_boss_mark_is_distinguishable_from_every_other_mark_a_tile_can_wear() {
        fn dist(a: Color, b: Color) -> f32 {
            (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs()
        }

        let boss = boss_mark_color();
        let others: &[(&str, Color)] = &[
            ("the nemesis mark", hud::palette::glyph(GlyphColor::Cyan)),
            ("a staffed mark", hud::palette::HEALTHY),
            ("a stranded mark", hud::palette::ATTENTION),
            ("the cutting outline", CUTTING_OUTLINE),
            ("con rung green", hud::palette::glyph(GlyphColor::Green)),
            ("con rung yellow", hud::palette::glyph(GlyphColor::Yellow)),
            ("con rung orange", hud::palette::glyph(GlyphColor::Orange)),
            ("con rung red", hud::palette::glyph(GlyphColor::Red)),
            ("rarity silver", SILVER),
            ("rarity gold", GOLD),
            ("rarity platinum", PLATINUM),
            ("rarity prismatic", PRISMATIC),
        ];

        for (what, c) in others {
            let d = dist(boss, *c);
            assert!(
                d >= 0.10,
                "the boss mark is {d:.3} from {what} — a player reading one \
                 tile cannot tell them apart"
            );
        }
    }
}
