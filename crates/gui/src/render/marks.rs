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

/// The progress bar's height, and how far it is held off the tile's edges.
///
/// **The bottom edge belongs to the bar the way the top edge belongs to the
/// rarity bar**, which is what the two bottom-corner marks lifting above it
/// mirrors — `nemesis_mark_rect` and `difficulty_mark_points` already drop
/// below `RARITY_BAR_PX` for exactly this reason at the other end.
///
/// The inset is doing a second job here that it is not doing on the marks.
/// `outline_open` draws a machine's bottom wall two pixels thick along
/// `py + tile_px - 1` and draws it *after* the tile's fills, so a bar flush
/// to that edge is painted over by the outline of the very machine it
/// belongs to. Held clear, the bar reads as a gauge sitting inside the box.
const PROGRESS_BAR_PX: f32 = 3.0;
const PROGRESS_BAR_INSET: f32 = 2.0;

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

/// A rare-spawn tier's bar along a tile's top edge — the surface map's own
/// channel for "how rare", shared with the tactical board so a silver or
/// gold body reads the same colour on both grids rather than being drawn
/// from a second copy of this arithmetic (`render/base.rs` used to inline
/// it, and `render/tactical.rs` drew nothing at all).
///
/// Paints nothing for `Rarity::Ordinary`, `rarity_color`'s own "no reading"
/// case — the same channel-optional shape `draw_difficulty_mark` and
/// `draw_recovery_mark` already follow.
///
/// `vig` multiplies the way every other mark's does; the tactical board has
/// no vignette of its own, so it always passes `1.0`.
pub(super) fn draw_rarity_bar(
    painter: &Painter,
    rarity: Rarity,
    px: f32,
    py: f32,
    tile_px: f32,
    vig: f32,
) {
    let Some(bar) = rarity_color(rarity) else {
        return;
    };
    painter.rect(px, py, tile_px - 1.0, RARITY_BAR_PX, at_level(bar, vig));
}

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
    // `staffed_mark_rect`'s floor, and its reason: the bottom edge is the
    // progress bar's.
    let floor = progress_bar_rect(px, py, tile_px).y;
    Rect::new(
        px + tile_px - 1.0 - IDENTITY_MARK_INSET - size,
        floor - IDENTITY_MARK_INSET - size,
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

/// The Alt marker — a `?` glyph in the con earmark's own corner, drawn
/// **in place of** the earmark for a frame `reveal` is held (spec §4 "The
/// Alt marker"). Text and not a shape, so the two can never be confused for
/// each other even at a glance that only catches form: the earmark answers
/// "how dangerous", this answers "is there something to learn here", and
/// they trigger on unrelated conditions.
///
/// Every corner is already taken (`difficulty_mark_points`'s own doc), so
/// holding Alt is a deliberate question that borrows the earmark's slot for
/// the duration — `draw_difficulty_mark`'s own caller is what suppresses the
/// earmark while this draws, not a check made here.
pub(super) fn draw_unseen_marker(painter: &Painter, px: f32, py: f32, glyph_px: u16, vig: f32) {
    let glyph = "?";
    painter.map(
        glyph,
        px + IDENTITY_MARK_INSET,
        py + RARITY_BAR_PX + IDENTITY_MARK_INSET,
        glyph_px,
        at_level(WHITE, vig),
    );
}

/// Which of a tile's two occupants owns the progress bar, and the hue it
/// wears — or nothing at all where no work is being done on this cell.
///
/// **A machine's own cycle wins over a pending upgrade's row**, which is the
/// same precedence the engine applies in `EntityView::job_progress` and the
/// same one the tile loop applies to the glyph: a machine being upgraded is
/// still producing, and the cell is drawing the machine.
///
/// **No hue of its own** — each case takes the colour that cell already
/// wears for this job, so the bar adds a quantity to a reading the tile is
/// already giving. A machine takes `machine_color`, so a starved one's
/// frozen bar agrees with its own outline instead of contradicting it; a
/// site the crew has not raised yet takes its caret's orange, the slab
/// itself being deliberately colourless.
///
/// Extracted rather than left inline for `staffed_mark_rect`'s reason: the
/// map's tile loop is far too big to reach with a test, and the precedence
/// is the half of this that a copy would get wrong.
pub(super) fn cell_bar(
    structure: Option<&EntityView>,
    building: Option<&EntityView>,
) -> Option<(f32, Color)> {
    structure
        .and_then(|ev| Some((ev.job_progress?, machine_color(ev.machine_status?))))
        .or_else(|| building.and_then(|ev| Some((ev.job_progress?, ORANGE))))
}

/// Where a cell's progress bar sits — the full width of the tile's bottom
/// edge, held clear of the status outline and of both bottom-corner marks.
///
/// A free function for `nemesis_mark_rect`'s reason: the geometry is
/// unit-testable without a `Painter`, and it is what `staffed_mark_rect` and
/// `patrol_mark_rect` read to find their own floor, so the three cannot
/// drift into each other's pixels.
pub(super) fn progress_bar_rect(px: f32, py: f32, tile_px: f32) -> Rect {
    let size = tile_px - 1.0;
    Rect::new(
        px + PROGRESS_BAR_INSET,
        py + size - PROGRESS_BAR_INSET - PROGRESS_BAR_PX,
        size - 2.0 * PROGRESS_BAR_INSET,
        PROGRESS_BAR_PX,
    )
}

/// How far along the work on this cell is, or nothing at all where no work
/// is being done on it — see `EntityView::job_progress`.
///
/// **A track and a fill, and the track is why an empty bar is still drawn.**
/// A cycle that has just turned over has to go on saying *a job is running
/// here*, which a zero-width fill on bare tile cannot; `palette::BAR_TROUGH`
/// is the role the rest of the HUD already spends on exactly that.
///
/// **No hue of its own.** The caller passes the colour the cell already
/// wears for this job — a machine's `machine_color`, a build site's caret
/// orange, a dig mark's plan blue — so the bar adds a quantity to a reading
/// the tile is already giving rather than opening a channel that has to be
/// learned.
///
/// Clamped here as well as at the engine's end: nothing in the compiler
/// holds a second caller to a range, and an unclamped figure draws off the
/// tile in silence.
pub(super) fn draw_progress_bar(
    painter: &Painter,
    progress: Option<f32>,
    px: f32,
    py: f32,
    tile_px: f32,
    color: Color,
    vig: f32,
) {
    let Some(done) = progress else {
        return;
    };
    let bar = progress_bar_rect(px, py, tile_px);
    painter.rect(
        bar.x,
        bar.y,
        bar.w,
        bar.h,
        at_level(hud::palette::BAR_TROUGH, vig),
    );
    let filled = bar.w * done.clamp(0.0, 1.0);
    if filled > 0.0 {
        painter.rect(bar.x, bar.y, filled, bar.h, at_level(color, vig));
    }
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
    // The bar owns the bottom edge, so the mark's floor is the bar's top and
    // not the tile's. A worked machine wears both at once — the mark says
    // somebody is on this and the bar says how far through — so sharing the
    // pixels would have drawn each through the other.
    let floor = progress_bar_rect(px, py, tile_px).y;
    Rect::new(
        px + STAFFED_MARK_INSET,
        floor - STAFFED_MARK_INSET - size - lift,
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
/// **Anchored above the patient's own glyph, not on it.** The shipped Bay's
/// `radius: 0` and `hauling::step_to_post`'s structure-tiles block mean the
/// patient stands *beside* the Bay, never on it — `a_downed_program_at_a_bay`
/// places it one tile off — so this was never actually a mark on the Bay's
/// own ink. It reads as one anyway when the mark shares the tile loop's own
/// centring formula: at rest a `+` at the same baseline as the patient's own
/// glyph paints on top of it, hiding what is being healed and leaving only a
/// green mark floating next to the Bay. `map`'s `y` is a **baseline**, and a
/// font's own ascent already carries a glyph's ink well clear above it, so
/// planting that baseline at the same top band `nemesis_mark_rect` and
/// `difficulty_mark_points` already keep clear of `RARITY_BAR_PX` — with no
/// further push down for the mark's own measured height — is what leaves the
/// patient's centred glyph, well below, untouched.
///
/// It still rides `Fx::centred_bob`, the build caret's curve — the same
/// rate, the same entity phase, so a base with a build site and a mending
/// body in it still reads as one map rather than two animations, and two
/// patients still bounce out of step.
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
        cell.y + RARITY_BAR_PX + IDENTITY_MARK_INSET - lift,
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
