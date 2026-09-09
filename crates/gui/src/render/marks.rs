//! The per-tile marks a program or a machine wears — identity, difficulty,
//! staffed and recovery — and a machine's status colour.

use super::terrain::at_level;
use super::*;

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

/// An identity mark's side, as a fraction of the tile — the shape a
/// nemesis and a boss both wear, in opposite corners, and how far it sits
/// off the tile's edges. Smaller than `STAFFED_MARK` and placed in the
/// opposite corner (top-right rather than bottom-left), so a marked program
/// standing on a machine-adjacent tile can never collide with either a
/// staffed mark or the outline `outline_open` drops along a chained pair's
/// shared edge.
const IDENTITY_MARK: f32 = 0.22;
const IDENTITY_MARK_INSET: f32 = 2.0;

/// The con earmark's leg, as a fraction of the tile — the right-angled
/// wedge folded into the **top-left** corner that answers "can I win this
/// fight".
///
/// Larger than `IDENTITY_MARK` on purpose: the con read is the one a player
/// scans a screenful of tiles for, where a nemesis or a boss mark is a
/// detail confirmed on the tile they have already stopped at. At this leg
/// the wedge's area is within a few percent of the full-width bar it
/// replaced, so the channel keeps the weight it had.
pub(super) const CON_MARK: f32 = 0.34;

/// Where the nemesis mark sits on a tile — the top-right corner, dropped
/// below `RARITY_BAR_PX` so it never overlaps the bar running the width of
/// the top edge, and inset from both remaining edges for the same reason
/// `STAFFED_MARK_INSET` is: flush into a corner it would read as painting
/// back in an edge `outline_open` deliberately left off.
///
/// A free function rather than inlined at the call site so the geometry is
/// unit-testable without a `Painter` — see the `nemesis_mark_clears_the_
/// rarity_bar` test.
pub(super) fn nemesis_mark_rect(px: f32, py: f32, tile_px: f32) -> Rect {
    let size = (tile_px - 1.0) * IDENTITY_MARK;
    Rect::new(
        px + tile_px - 1.0 - IDENTITY_MARK_INSET - size,
        py + RARITY_BAR_PX + IDENTITY_MARK_INSET,
        size,
        size,
    )
}

/// Where a town's patrol wears its mark — the **bottom-right** corner, the
/// one corner of a tile nothing else claims: the rarity bar owns the top
/// edge, the con earmark the top-left, a nemesis the top-right and the
/// staffed mark the bottom-left.
///
/// Sized and inset like `nemesis_mark_rect` rather than like the staffed
/// mark, because what it says is a fact about the program and not about a
/// job — and inset from both edges for `STAFFED_MARK_INSET`'s reason, so it
/// cannot read as painting back in a line `outline_open` left off.
///
/// **A fourth channel that spends none of the other three.** The glyph's
/// authored hue still says what the program is, the con read still says how
/// dangerous, the rarity bar still says how rare.
///
/// A free function for `nemesis_mark_rect`'s reason: the geometry is
/// unit-testable without a `Painter`.
pub(super) fn patrol_mark_rect(px: f32, py: f32, tile_px: f32) -> Rect {
    let size = (tile_px - 1.0) * IDENTITY_MARK;
    Rect::new(
        px + tile_px - 1.0 - IDENTITY_MARK_INSET - size,
        py + tile_px - 1.0 - IDENTITY_MARK_INSET - size,
        size,
        size,
    )
}

/// The con read's earmark — a right triangle folded into the **top-left**
/// corner, its right angle at the corner and its hypotenuse running down
/// into the tile, so the reading is a shape and not a fifth coloured strip
/// competing with the glyph for the tile's interior.
///
/// **Dropped below `RARITY_BAR_PX` and inset from the left**, exactly as
/// `nemesis_mark_rect` is: the rarity bar owns the full width of the top
/// edge and is painted first, so a wedge flush into the corner would cover
/// one end of a channel that means something else. The left inset is
/// `nemesis_mark_rect`'s reason again — flush against the edge it reads as
/// painting back in a line `outline_open` deliberately left off.
///
/// A free function for that mark's other reason: the geometry is
/// unit-testable without a `Painter`.
pub(super) fn difficulty_mark_points(px: f32, py: f32, tile_px: f32) -> [(f32, f32); 3] {
    let leg = (tile_px - 1.0) * CON_MARK;
    let x = px + IDENTITY_MARK_INSET;
    let y = py + RARITY_BAR_PX + IDENTITY_MARK_INSET;
    [(x, y), (x + leg, y), (x, y + leg)]
}

/// The hue a boss's glyph wears — the magenta `difficulty_color` used to
/// return for one, and wears again.
///
/// A named function rather than a literal at the draw site, so the census
/// that holds it apart from every con rung and every mark a tile can wear
/// has something to name. It is also the whole reason a boss cannot say its
/// own con read with its ink: this hue *is* that ink.
pub(super) fn boss_color() -> Color {
    hud::palette::glyph(GlyphColor::Magenta)
}

/// Paints the con read into the top-left corner, or nothing at all when
/// there is no reading to paint.
///
/// Extracted the way `draw_recovery_mark` is: the map's tile loop is far too
/// big to reach with a test, and what has to be pinned here is that `None`
/// paints *nothing* — an earmark on a companion would say the player can
/// beat their own program.
///
/// `vig` multiplies the way the rarity bar's does, so a tile darkened at the
/// edge of the light doesn't leave its marks burning at full brightness.
pub(super) fn draw_difficulty_mark(
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
    painter.poly(
        &difficulty_mark_points(px, py, tile_px),
        Color::new(c.r * vig, c.g * vig, c.b * vig, c.a),
    );
}

/// Where the "someone is on this job" mark sits, `lift` px up from its
/// resting place — `Fx::staffed_bob` while a machine is worked, zero at rest
/// and for a stranded mark, which blinks in place instead.
///
/// Extracted rather than left inline so the resting place is one
/// expression: the test that pins it used to hand-copy this arithmetic,
/// which is exactly the copy that drifts when the bottom edge changes.
pub(super) fn staffed_mark_rect(px: f32, py: f32, tile_px: f32, lift: f32) -> Rect {
    let size = (tile_px - 1.0) * STAFFED_MARK;
    Rect::new(
        px + STAFFED_MARK_INSET,
        py + tile_px - 1.0 - STAFFED_MARK_INSET - size - lift,
        size,
        size,
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
pub(super) fn draw_recovery_mark(
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
pub(super) fn outline_open(
    painter: &Painter,
    px: f32,
    py: f32,
    size: f32,
    color: Color,
    open: &[(i32, i32)],
) {
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
pub(super) fn machine_color(status: MachineStatus) -> Color {
    match status {
        MachineStatus::Running => hud::palette::HEALTHY,
        MachineStatus::Starved | MachineStatus::Unstaffed => hud::palette::WARN,
        // The louder yellow rather than the dimmer one: unlike `Unstaffed`,
        // waiting does not fix this one, so it belongs with the states that
        // are asking for you — which is what `palette::ATTENTION` means, and
        // is the same colour `Game::attention` puts in the status bar for
        // them. `Unpowered` joins them because a dark machine never resolves
        // itself either; what fixes it is more supply on the grid, which is
        // not always another Recharger Node — a base whose Rechargers are all
        // `Dry` already has the capacity and needs fuel next to them.
        //
        // Not `palette::THREAT`. Br red is reserved for hostility and inbound
        // harm, and a clogged Mining Node is neither.
        MachineStatus::Clogged | MachineStatus::Stranded | MachineStatus::Unpowered => {
            hud::palette::ATTENTION
        }
        // The one machine state that gets a red, and it is `palette::OFFLINE`
        // rather than THREAT — see that constant for why a second red exists.
        // A dry supplier is the *cause* of every dark machine on the map, so
        // it is the tile worth walking to, and it reads hotter than the
        // effects it produced.
        MachineStatus::Dry => hud::palette::OFFLINE,
        MachineStatus::Idle => hud::palette::FAINT,
    }
}
