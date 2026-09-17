//! The battle map, drawn into the map pane.
//!
//! **The map screen's grid with a different tile source.** It takes its
//! origin from `tile_origin_px` and its cell size from `map_cell` exactly as
//! the surface map does, so the camera, the zoom and the pane's own inset
//! are the ones already in use; what it does not share is the surface map's
//! loop, because a battle map has no biomes, no structures, no build sites
//! and no haul marks. The rules the two genuinely share — `ConRead::of` for
//! the con read, `glyph_color` for a body's hue, `Painter::sprite` for its
//! art, `marks::draw_rarity_bar` for a rare-spawn tier's bar — are called
//! here rather than restated.
//!
//! No vignette. The surface map dims with the player's Power because the
//! world is seen through a failing signal; a battle map is a discrete arena
//! the fight is lit by, and dimming it would hide the one thing the screen
//! exists to show.

use feral_processes_engine::tactical::map::BattleCell;
use feral_processes_engine::tactical::view::{TacticalBody, TacticalView, TamperTag, TurnRow};

use super::RARITY_BAR_PX;
use super::base::{ConRead, tile_origin_px};
use super::history::history_rows;
use super::hud::layout::strip_inset;
use super::hud::palette;
use super::marks::draw_rarity_bar;
use super::popup::{PopupSize, Row, draw_popup, item_row, spent_item_row, text_row};
use crate::fx::{BOLT_THICKNESS_PX, Fx, cell_centers};
use crate::paint::{Color, Painter, Rect};
use crate::text::Metrics;
use feral_processes_app_core::{Mode, menu_shortcut};
use feral_processes_engine::LogEntry;
use feral_processes_engine::battle::SpecialOption;

/// The ground, by kind. Brightness and nothing else carries passability,
/// `RockDb`'s rule from base space: hue is spoken for by what is standing
/// on a cell, and a kind may not spend it.
fn cell_color(kind: BattleCell) -> Color {
    match kind {
        BattleCell::Open => Color::new(0.13, 0.15, 0.18, 1.0),
        BattleCell::Rough => Color::new(0.19, 0.18, 0.14, 1.0),
        BattleCell::Cover => Color::new(0.30, 0.31, 0.34, 1.0),
        BattleCell::Blocked => Color::new(0.05, 0.06, 0.08, 1.0),
    }
}

/// How far into a tile a body's art is inset, matching the surface map's own
/// centring of a glyph in its cell.
fn sprite_inset(tile_px: f32, glyph_px: u16) -> f32 {
    (tile_px - glyph_px as f32) / 2.0
}

/// How much ink a cloaked body or a Hallucination decoy draws with — see
/// `components::Cloaked` and `tactical::Decoy`. One constant for both: they
/// mean the same thing, *not really there*.
///
/// One multiply at the draw site fades **both** halves of a cloaked body:
/// `Painter::sprite`'s tint multiplies, and `ConRead::glyph_ink` carries the
/// authored alpha through, so a body's art and its glyph dim together with
/// no sixteenth `Painter` operation and no change to `paint.rs`. A decoy
/// draws through `painter.map` alone — see `Painter::sprite`'s own doc for
/// why nothing masquerading as a body may take that door.
///
/// Faded rather than hidden: a cloaked body is still a wall in
/// `reach::movement_field`, so the cell it stands on is already a tell, and
/// a tell is the right amount of information. A decoy is faded for the same
/// reason a cloaked body is — the player is meant to notice it is not real.
const FADED_ALPHA: f32 = 0.35;

/// The arrow that hangs over whoever is acting: its width and its height as
/// fractions of a tile, and how far its point is held off the tile's top
/// edge at rest.
///
/// **It hangs *above* the tile rather than sitting in it**, which is what
/// keeps it out of every channel a tile already spends: the rarity bar owns
/// the top edge, the con earmark the top-left corner below it, the HP bar
/// the bottom edge, and the middle is the glyph or the sprite this arrow
/// exists to point at. Drawn inside the cell it would have to be small
/// enough to dodge all four, and an arrow that small is not the thing a
/// player finds by glancing.
///
/// The gap is what the bob swings out of: the arrow's rest position is its
/// *lowest*, so a lift can never carry it down onto the body.
const TURN_ARROW_WIDTH: f32 = 0.44;
const TURN_ARROW_HEIGHT: f32 = 0.30;
const TURN_ARROW_GAP: f32 = 2.0;

/// How heavily a cell the acting body can still step to is washed — and, in
/// `AIM` rather than `PLAN`, a cell a `Radius` routine's centre could legally
/// land on. One constant for both, and that is safe **only** because the two
/// fields never draw on the same cell at once: `draw_tactical_map` suppresses
/// the reach field entirely while `placeable` is non-empty, so the yellow
/// outline *replaces* the blue one for as long as a splash is being aimed
/// rather than sitting over it. The two drawing at once was the first cut of
/// this feature's own bug — two washes stacked read as a colour nobody
/// authored, not as "you may both step here and throw there".
///
/// Under the aim preview's own 0.22 so a shaped routine reads over it, and
/// faint enough that the terrain under it stays legible: the wash says a
/// cell is *available*, and a cell whose kind it hid would make it say
/// something it does not know.
const REACH_WASH_ALPHA: f32 = 0.13;

/// How thickly the reach field's outer edge is drawn — the placeable
/// field's too, `REACH_WASH_ALPHA`'s reason.
///
/// **The boundary is what makes the field findable, and the wash above is
/// what makes it readable.** At 0.13 of one hue over a near-black tile the
/// wash is legible only to somebody already looking at the right part of the
/// board; raising it far enough to catch the eye is the thing its own comment
/// refuses, since the terrain kind underneath is the other half of what a
/// cell is worth stepping to. A line at full strength spends no ink inside
/// the field at all.
///
/// Drawn along the field's **boundary** and not around each of its cells:
/// every cell outlined whole is a grid of boxes, which reads as sixty marks
/// rather than as one area, and the eye has to count them to find the edge.
const REACH_EDGE_PX: f32 = 1.5;

/// The three points of that arrow, given the top-left of the acting body's
/// tile and how far this frame's bob has lifted it.
///
/// A free function for `marks::nemesis_mark_rect`'s reason — the geometry is
/// the thing worth holding, and holding it needs no `Painter`.
fn turn_arrow(px: f32, py: f32, tile_px: f32, lift: f32) -> [(f32, f32); 3] {
    let cx = px + tile_px / 2.0;
    let half = tile_px * TURN_ARROW_WIDTH / 2.0;
    let point = py - TURN_ARROW_GAP - lift;
    let base = point - tile_px * TURN_ARROW_HEIGHT;
    [(cx - half, base), (cx + half, base), (cx, point)]
}

/// Washes and outlines one cell of a field — the reach field and the
/// placeable field are the same shape drawn in different colours, so this is
/// the one place that shape is written.
///
/// Draws nothing when `cell` is not itself in `field`. The wash is a flat
/// tint at `REACH_WASH_ALPHA`; the boundary is drawn a side at a time and
/// only where the neighbour on that side is *not* in `field` — a body and a
/// `Blocked` cell are both walls in `reach::movement_field`, so a gap in the
/// field is a cell that genuinely cannot be reached, and outlining it is the
/// same answer the field's outer edge gives.
fn draw_cell_field(
    painter: &Painter,
    field: &[(i32, i32)],
    cell: (i32, i32),
    px: f32,
    py: f32,
    tile_px: f32,
    color: Color,
) {
    if !field.contains(&cell) {
        return;
    }
    painter.rect(
        px,
        py,
        tile_px - 1.0,
        tile_px - 1.0,
        Color::new(color.r, color.g, color.b, REACH_WASH_ALPHA),
    );
    let far = tile_px - 1.0;
    for (dx, dy, from, to) in [
        (0, -1, (0.0, 0.0), (far, 0.0)),
        (0, 1, (0.0, far), (far, far)),
        (-1, 0, (0.0, 0.0), (0.0, far)),
        (1, 0, (far, 0.0), (far, far)),
    ] {
        if field.contains(&(cell.0 + dx, cell.1 + dy)) {
            continue;
        }
        painter.line(
            px + from.0,
            py + from.1,
            px + to.0,
            py + to.1,
            REACH_EDGE_PX,
            color,
        );
    }
}

/// Draws the whole battle map.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tactical_map(
    view: &TacticalView,
    cursor: Option<(i32, i32)>,
    preview: &[(i32, i32)],
    placeable: &[(i32, i32)],
    fx: &mut Fx,
    painter: &Painter,
    pane: Rect,
    tile_px: f32,
    glyph_px: u16,
) {
    let half_w = ((pane.w / tile_px) / 2.0).max(1.0) as i32;
    let half_h = ((pane.h / tile_px) / 2.0).max(1.0) as i32;
    // The camera follows whoever is acting — the "watch" override's job,
    // done here off the view rather than through `App::watch_center`,
    // which reads a world `Position` a body on a battle map does not have.
    //
    // **Through `Fx::battle_center` and not off the view directly.** A turn
    // is handed on inside the same call that resolves the blow, so the acting
    // body *is* the next one the instant an attack lands; aimed at that, the
    // camera walks away from the attacker while its streak is still in flight
    // and its hit flash still lit. The dwell lives in `Fx` because it is
    // state across frames, which a renderer has none of.
    let center = fx
        .battle_center(acting_body(view).map(|body| (body.entity, body.cell)))
        .unwrap_or((view.board.side / 2, view.board.side / 2));
    // **No lag clamp, unlike the surface map.** `CAMERA_MAX_LAG` buys that
    // map a trailing edge its one extra ring of tiles can cover; this loop
    // walks the whole board and skips what falls outside the pane, so there
    // is no blank edge to expose — and at one tile the move between two
    // bodies standing apart is a teleport rather than a pan.
    let (off_x, off_y) = fx.camera_offset(center, painter.delta(), None);
    // Every draw below is gated on this. The camera is held on the body that
    // acted *last*, so the acting body, the arrow over its head and a cursor
    // opened on its cell can each be an arbitrary distance off centre while
    // the pan runs — and the pane is a region of a shared screen, so
    // anything drawn outside it lands on the HUD.
    let on_pane = |px: f32, py: f32| {
        px < pane.x + pane.w
            && py < pane.y + pane.h
            && px + tile_px > pane.x
            && py + tile_px > pane.y
    };

    painter.rect(
        pane.x,
        pane.y,
        pane.w,
        pane.h,
        Color::new(0.02, 0.03, 0.04, 1.0),
    );

    for (cell, kind) in view.board.cells() {
        let (px, py) = tile_origin_px(
            cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if !on_pane(px, py) {
            continue;
        }
        painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, cell_color(kind));

        // Where the body whose turn it is may still step — **either side**.
        // `TacticalView::reachable` is `battle.actor()`'s own field and has
        // never had a notion of sides, so this is drawn off it alone and
        // never off `player_turn`: a wash that appeared only for bodies the
        // player commands hid the half of the board a fight is planned
        // *against*.
        //
        // **One colour for both, and `PLAN` rather than `ATTENTION`.** The
        // wash answers *where*, and whose turn it is is already answered a
        // few lines down by the arrow bobbing over that body's head, in
        // `PLAN` against `THREAT`. Tinting the wash by side too would put
        // one answer on two channels and make neither the place to read it.
        //
        // **Suppressed while `placeable` is showing.** The request this
        // outline shipped for was "the same as the movement outline, but
        // yellow" — a replacement, not a second field stacked on the first.
        // Drawing both while aiming a splash washed every reachable cell
        // twice and read as a colour nobody authored.
        if placeable.is_empty() {
            draw_cell_field(
                painter,
                &view.reachable,
                cell,
                px,
                py,
                tile_px,
                palette::PLAN,
            );
        }
        // Where a `Radius` routine's centre may legally land. `placeable` is
        // already empty for every shape but `Radius`
        // (`Game::tactical_placeable_cells`'s own gate), so this draws
        // nothing while aiming a swing, a `Single` routine or a directional
        // one, and nothing outside `Mode::TacticalAim` at all.
        draw_cell_field(painter, placeable, cell, px, py, tile_px, palette::AIM);
        // What the aim would land on. Over the reach wash, because a routine
        // resolves wherever it is aimed whether or not the body could walk
        // there.
        if preview.contains(&cell) {
            let c = palette::THREAT;
            painter.rect(
                px,
                py,
                tile_px - 1.0,
                tile_px - 1.0,
                Color::new(c.r, c.g, c.b, 0.22),
            );
        }
        // A body's own hit, last of the tile's washes — `render/base.rs`'s
        // own ordering for a raid flash, and the same call: a tactical hit
        // reuses `EffectKind::Hit`'s wash and spark burst rather than
        // inventing a second red.
        if let Some(flash) = fx.tactical_tile_flash(cell) {
            painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
        }
    }

    // A Hallucination's fakes, **before** the bodies: a hallucinating body
    // may stand on its own decoy's cell (decision 3, "every decoy goes on a
    // free cell" — free of *other* bodies, not of the caster once it walks),
    // and the body drawn on top is the one a swing can actually hit.
    for decoy in &view.decoys {
        let (px, py) = tile_origin_px(
            decoy.cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if !on_pane(px, py) {
            continue;
        }
        let base = if decoy.of_player {
            palette::PLAYER
        } else {
            super::glyph_color(decoy.color)
        };
        let ink = Color::new(base.r, base.g, base.b, base.a * FADED_ALPHA);
        let glyph = decoy.glyph.to_string();
        let dims = painter.measure_map(&glyph, glyph_px);
        let tx = px + (tile_px - dims.width) / 2.0;
        let ty = py + (tile_px + dims.height) / 2.0;
        // `painter.map`, never `sprite` — a decoy is a fake glyph, not a
        // fake body, and `sprite`'s tint is authored for near-white art.
        painter.map(&glyph, tx, ty, glyph_px, ink);
    }

    for body in &view.bodies {
        let (px, py) = tile_origin_px(
            body.cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if !on_pane(px, py) {
            continue;
        }
        draw_body(body, painter, px, py, tile_px, glyph_px);
    }

    // A profiled hostile's published plan, over the bodies so a walk
    // crossing a body's tile still reads. `Game::tactical_forecast` is
    // exact only at the moment the turn begins — see its own doc — so this
    // is read straight off `TurnRow::forecast` rather than re-derived here.
    for row in &view.order {
        let Some(forecast) = &row.forecast else {
            continue;
        };
        if forecast.walk.is_empty() {
            continue;
        }
        let Some(body) = view.bodies.iter().find(|b| b.entity == row.entity) else {
            continue;
        };
        let to_px = |cell: (i32, i32)| {
            tile_origin_px(
                cell,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        };
        let mut from = body.cell;
        for &step in &forecast.walk {
            let (tpx, tpy) = to_px(step);
            if on_pane(tpx, tpy) {
                let ((ax, ay), (bx, by)) = cell_centers(to_px, tile_px, from, step);
                // `FORECAST` and its own role — never `THREAT` (the wild
                // side's inbound harm, not a preview of it), never `AIM` (the
                // player's own cursor), and no longer `PLAN`: the reach wash
                // a few blocks up and the arrow over the acting body's head
                // are both `PLAN`, and a telegraphed walk drawn in it was a
                // third meaning on that one channel. See the role's own doc.
                painter.line(ax, ay, bx, by, BOLT_THICKNESS_PX, palette::FORECAST);
            }
            from = step;
        }
        if let Some(target) = forecast.target {
            let (px, py) = to_px(target);
            if on_pane(px, py) {
                painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, palette::FORECAST);
            }
        }
    }

    // Whose turn it is, hung over that body's head and bouncing.
    //
    // **The one channel on this grid that says *now*** — the turn strip in
    // the corner already names the order and is the wrong place to find the
    // answer to "where am I": a fight is played by looking at the board.
    // Blue for a body the player commands and red for one the wild side
    // does, which is `palette::PLAN` against `palette::THREAT` rather than
    // the HP bar's green: this arrow is the player having a choice, the same
    // reading the reach wash above is drawn with, and a hostile taking its
    // turn is inbound harm.
    //
    // **`staffed_bob` and not `centred_bob`.** The shared raised cosine
    // either way, so this bounce and the base's agree rather than being two
    // invented curves — but this one is anchored at its rest position and
    // lifts only, because that rest position is `TURN_ARROW_GAP` off the
    // body's head and a down-swing would spend it.
    //
    // Bounds-checked like the loops above, and it was not always: `center`
    // used to be this body's own cell within one tile of camera lag, so the
    // arrow could never reach an edge to hang off. `Fx::battle_center` holds
    // the camera on the body that acted *last*, which is exactly what makes
    // the acting body's own tile reachable from anywhere on the board.
    if let Some(body) = acting_body(view) {
        let (px, py) = tile_origin_px(
            body.cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if on_pane(px, py) {
            painter.poly(
                &turn_arrow(px, py, tile_px, fx.staffed_bob(body.entity)),
                if body.is_hostile {
                    palette::THREAT
                } else {
                    palette::PLAN
                },
            );
        }
    }

    // Over the bodies, because a blow travelling to a body passes in front
    // of it, and because a streak under a glyph is invisible at three cells.
    fx.draw_bolts(
        painter,
        |cell| {
            tile_origin_px(
                cell,
                center,
                (half_w, half_h),
                (off_x, off_y),
                tile_px,
                pane,
            )
        },
        tile_px,
    );

    // The struck body's own debris — `render/base.rs`'s spark burst, reused
    // rather than restated, cued at the body's cell instead of a raided
    // structure's world tile.
    fx.draw_tactical_bursts(painter, tile_px, |cell| {
        tile_origin_px(
            cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        )
    });

    // A green `+` bouncing over anyone just healed.
    fx.draw_heal_marks(painter, tile_px, glyph_px, |cell| {
        tile_origin_px(
            cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        )
    });

    // Last, so the cursor is never under a body it is pointing at — and
    // bounds-checked for the arrow's reason: it opens on the acting body's
    // own cell, which the camera need not be looking at yet.
    if let Some(cell) = cursor {
        let (px, py) = tile_origin_px(
            cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if on_pane(px, py) {
            painter.rect_lines(px, py, tile_px - 1.0, tile_px - 1.0, 2.0, palette::EMPHASIS);
        }
    }
}

/// The body whose turn it is — the one derivation of that, since the camera,
/// the arrow over its head and every caller asking where it stands must all
/// name the same body.
///
/// `TacticalView::active` indexes `order`, which is the initiative roll and
/// not the board, so this is a lookup and not a subscript.
fn acting_body(view: &TacticalView) -> Option<&TacticalBody> {
    let acting = view.order.get(view.active?)?.entity;
    view.bodies.iter().find(|b| b.entity == acting)
}

/// One body: its art or its glyph, its con read, and what is left of it.
fn draw_body(
    body: &TacticalBody,
    painter: &Painter,
    px: f32,
    py: f32,
    tile_px: f32,
    glyph_px: u16,
) {
    let authored = super::glyph_color(body.color);
    // The player's `@` is a role, read off `is_player` and never off the
    // hue they happen to have spawned with.
    let mut ink = if body.is_player {
        palette::PLAYER
    } else {
        authored
    };
    if body.cloaked {
        ink.a *= FADED_ALPHA;
    }
    let inset = sprite_inset(tile_px, glyph_px);
    // **The sprite call's own answer**, never `sprite.is_some()`: a name the
    // table has nothing under falls back to the glyph, and that glyph is
    // free to carry the con rung.
    let drew_sprite = body
        .sprite
        .as_deref()
        .is_some_and(|name| painter.sprite(name, px + inset, py + inset, glyph_px as f32, ink));
    let con = ConRead::of(body.difficulty, body.is_boss, drew_sprite);
    if !drew_sprite {
        let glyph = body.glyph.to_string();
        let dims = painter.measure_map(&glyph, glyph_px);
        let tx = px + (tile_px - dims.width) / 2.0;
        let ty = py + (tile_px + dims.height) / 2.0;
        painter.map(&glyph, tx, ty, glyph_px, con.glyph_ink(ink, 1.0));
    }
    // The rare-spawn tier's own bar — see `marks::draw_rarity_bar`. Drawn
    // before the earmark below, which drops clear of it exactly as the
    // surface map's does.
    draw_rarity_bar(painter, body.rarity, px, py, tile_px, 1.0);
    if let Some(rung) = con.earmark() {
        let c = super::glyph_color(rung);
        let leg = tile_px * 0.28;
        let y = py + RARITY_BAR_PX;
        painter.poly(&[(px, y), (px + leg, y), (px, y + leg)], c);
    }
    if let Some(fraction) = body.hp_fraction {
        let h = (tile_px * 0.09).max(2.0);
        let y = py + tile_px - 1.0 - h;
        painter.rect(px, y, tile_px - 1.0, h, palette::BAR_TROUGH);
        painter.rect(
            px,
            y,
            (tile_px - 1.0) * fraction.clamp(0.0, 1.0),
            h,
            if body.is_hostile {
                palette::THREAT
            } else {
                palette::HEALTHY
            },
        );
    }
}

/// The turn order, as a block inside the map pane.
///
/// **The compass block's slot, and its rules.** It starts at
/// `layout::strip_inset` below the pane's top edge because the THREAT strip's
/// quad hangs down into the pane, and it is a block rather than a strip on
/// the pane's bottom border because a strip that appears only during a fight
/// buys the map a band it can never draw tiles in and re-lays the whole grid
/// on the keypress that opens one — which at the keyboard reads as the
/// camera lurching.
pub(super) fn draw_turn_strip(view: &TacticalView, pane: Rect, painter: &Painter, m: &Metrics) {
    if view.order.is_empty() {
        return;
    }
    let pad = m.line_height * 0.35;
    let size = m.small();
    let cell = painter.measure_ui_advance("M", size) * 2.0;
    let head = format!("ROUND {}", view.round);
    let head_w = painter.measure_ui_advance(&head, size);
    let w = pad * 2.0 + head_w.max(cell * view.order.len() as f32);
    let h = pad * 2.0 + m.line_height * 2.0;

    let x = pane.x + pane.w - m.inset - w;
    let y = pane.y + strip_inset(m);
    if x < pane.x + m.inset || y + h > pane.y + pane.h {
        return;
    }
    painter.rect(x, y, w, h, Color::new(0.04, 0.06, 0.08, 0.88));
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);
    painter.ui(
        &head,
        x + pad,
        y + pad + size as f32 * 0.8,
        size,
        palette::PANE_TITLE,
    );

    let baseline = y + pad + m.line_height + size as f32 * 0.8;
    for (i, row) in view.order.iter().enumerate() {
        let gx = x + pad + cell * i as f32;
        // The acting body wears the highlight, which is the whole of what a
        // turn order is for.
        if Some(i) == view.active {
            let c = palette::EMPHASIS;
            painter.rect(
                gx - 1.0,
                y + pad + m.line_height,
                cell,
                m.line_height,
                Color::new(c.r, c.g, c.b, 0.18),
            );
        }
        let glyph = row.glyph.to_string();
        painter.map(
            &glyph,
            gx,
            baseline,
            size,
            // `WARN` before `PLAYER`: a taken-over companion is still on the
            // party's side of the initiative order (`is_hostile` is false),
            // but its rung is the AI's for as long as the entry lasts, and
            // that is worth a colour of its own — `WARN` rather than
            // `THREAT`, since the party still wins this fight together.
            if row.is_hostile {
                super::glyph_color(row.color)
            } else if row.taken_over {
                palette::WARN
            } else {
                palette::PLAYER
            },
        );
    }

    draw_tamper_block(view, pane, x + w, y + h, w, painter, m);
}

/// A rung's own tag text, decision 7's exhaustive vocabulary — `cell_mark`'s
/// rule, so a sixth `TamperTag` fails to compile here rather than drawing a
/// blank word.
fn tamper_tag_text(tag: TamperTag) -> &'static str {
    match tag {
        TamperTag::Hot => "HOT",
        TamperTag::Cold => "COLD",
        TamperTag::Profiled => "PROF",
        TamperTag::Injected => "INJ",
        TamperTag::Hallucinating => "HALL",
    }
}

/// One rung's own line in the tamper block, decision 7's format —
/// `<glyph> TAG TAG ▸ <routine name>` — or `None` for a rung with nothing
/// to say: no active `Tampered` slot, no live forecast, and not taken over.
///
/// A free function rather than inlined in `draw_tamper_block`, so the width
/// census below builds the exact line production draws instead of a second
/// copy of the format.
fn tamper_line(row: &TurnRow) -> Option<String> {
    if row.tags.is_empty() && row.forecast.is_none() && !row.taken_over {
        return None;
    }
    let mut words = vec![row.glyph.to_string()];
    words.extend(row.tags.iter().map(|&tag| tamper_tag_text(tag).to_string()));
    if row.taken_over {
        words.push("HIJACK".to_string());
    }
    if let Some(forecast) = &row.forecast {
        words.push(format!("▸ {}", forecast.action));
    }
    Some(words.join(" "))
}

/// The tamper block, decision 7: one line per rung that carries a tag, a
/// forecast or a hijack, beneath the strip and in initiative order.
///
/// `strip_right`/`strip_bottom`/`strip_w` are the strip's own box, read
/// rather than a literal: the block re-anchors to the strip's right edge and
/// starts one gap below its bottom. `pane` is the map pane itself, for the
/// same off-screen guard `draw_turn_strip` applies to its own box.
///
/// **Does not scroll**, per decision 7 — its height is however many lines
/// the initiative order asks for, and
/// `the_widest_tamper_line_fits_the_map_pane` is what keeps a single line
/// from ever demanding more width than the pane has to give.
fn draw_tamper_block(
    view: &TacticalView,
    pane: Rect,
    strip_right: f32,
    strip_bottom: f32,
    strip_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let lines: Vec<String> = view.order.iter().filter_map(tamper_line).collect();
    if lines.is_empty() {
        return;
    }
    let pad = m.line_height * 0.35;
    let size = m.small();
    let widest = lines
        .iter()
        .map(|l| painter.measure_ui_advance(l, size))
        .fold(0.0_f32, f32::max);
    let w = (pad * 2.0 + widest).max(strip_w);
    let h = pad * 2.0 + m.line_height * lines.len() as f32;
    // Right-aligned on the strip's own right edge, so the two read as one
    // fixture rather than two boxes that happen to sit near each other.
    let x = strip_right - w;
    let y = strip_bottom + m.gap;
    if x < pane.x + strip_inset(m) || y + h > pane.y + pane.h {
        return;
    }
    painter.rect(x, y, w, h, Color::new(0.04, 0.06, 0.08, 0.88));
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);
    for (i, line) in lines.iter().enumerate() {
        painter.ui(
            line,
            x + pad,
            y + pad + m.line_height * i as f32 + size as f32 * 0.8,
            size,
            palette::LABEL,
        );
    }
}

/// What the acting body may do, for the keybar.
///
/// Built here rather than as a strip of its own: the keybar already rides
/// `log_pane`'s bottom border and already degrades through
/// `strip::fitting`, so a fight's action list is a content swap and costs
/// no layout at all.
///
/// **The mode is an argument because a fight is two screens.** The cursor
/// is not the board: the three action keys do nothing while it is open, and
/// the two that commit and cancel it appear nowhere else. Taken here rather
/// than branched on at `draw_playing_base`, so which bar a screen gets is
/// one derivation a test can ask rather than a condition in a renderer.
///
/// **And `auto` is an argument because it is app-core's**, not the engine's:
/// `TacticalView::player_turn` still says the party's turn is the player's
/// while auto-attack spends it, which is the whole shape of that feature. It
/// is read before `player_turn` because it outranks it — a board being driven
/// end to end has one thing worth saying at every instant of it, and "the
/// wild side is moving" is not it.
pub(super) fn action_bar(mode: Mode, view: &TacticalView, auto: bool) -> Vec<(String, String)> {
    if mode == Mode::TacticalAim {
        // Movement first because it is what the player is doing; `fitting`
        // drops from the end, and Esc is the row that may go — a cursor
        // backed out of by the key every other screen backs out with is the
        // one thing here nobody has to be told twice.
        return vec![
            ("↑↓←→ numpad".to_string(), "aim".to_string()),
            ("Enter".to_string(), "confirm".to_string()),
            ("Esc".to_string(), "cancel".to_string()),
        ];
    }
    if auto {
        return vec![("any key".to_string(), "stop auto-attack".to_string())];
    }
    if !view.player_turn {
        return vec![(String::new(), "the wild side is moving".to_string())];
    }
    let mut rows = vec![
        (
            "↑↓←→ numpad".to_string(),
            format!("move ({} left)", view.allowance),
        ),
        ("a".to_string(), "attack".to_string()),
        // Between attack and special, so the bar reads in the order the
        // abstract fight's own action menu does: a, d, s.
        ("d".to_string(), "defend".to_string()),
        // `special`, not `routine`: this is the word the abstract fight's
        // own action menu builds (`Game::battle_action_options`), and one
        // fight model naming the same verb differently is what makes the
        // shared `s` stop reading as the same key.
        ("s".to_string(), "special".to_string()),
        ("E".to_string(), "end turn".to_string()),
        // Before `A`, which stays last per its own comment below.
        ("R".to_string(), "resolve".to_string()),
        // Last, because `strip::fitting` drops from the end and this is the
        // row that may go — but it does not go at 1280x720, which is
        // `the_action_bar_fits_the_log_pane`'s measurement and not a hope.
        ("A".to_string(), "auto-attack".to_string()),
    ];
    if view.acted {
        // The auto and resolve rows survive a spent turn: arming either is
        // not an action, and a turn with nothing left to spend is exactly
        // when a player decides they would rather watch the rest —
        // `tactical_drive_turn` on a spent turn is safe (`tactical_attack`
        // and `tactical_step` both refuse when `acted`, so `[R]` just ends
        // the turn and carries on), and hiding it here would hide a key
        // that is not actually refused.
        rows.retain(|(k, _)| k == "E" || k == "R" || k == "A");
    }
    rows
}

/// The routine picker, drawn as a popup over the battle map.
///
/// `draw_field_routine`'s shape, and greyed rather than hidden for its
/// reason: the engine refuses an unavailable routine again on commit with
/// the reason on the status line, and a row that vanished would leave the
/// player wondering where a routine they installed went.
pub(super) fn draw_tactical_routines(
    options: &[SpecialOption],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("Run which routine?")];
    if options.is_empty() {
        rows.push(text_row("(nothing this body can run in a fight)"));
    }
    for (i, option) in options.iter().enumerate() {
        let label = format!("[{}] {} — {}", menu_shortcut(i), option.name, option.detail);
        rows.push(match &option.unavailable {
            None => item_row(label, i == selected),
            Some(reason) => spent_item_row(format!("{label} — {reason}"), i == selected),
        });
    }
    draw_popup(
        "Run a Routine",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// A finished tactical fight's results, drawn as a popup over the board it
/// was fought on — see `Mode::TacticalResult`.
pub(super) fn draw_tactical_result(
    outcomes: &[LogEntry],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Fight Over",
        PopupSize::Large,
        &tactical_result_rows(outcomes),
        refusal,
        painter,
        m,
    );
}

/// The popup's rows: one per folded result, through the history screen's
/// own row builder so a result reads the same there and here.
///
/// Items and not text rows, because `draw_popup` pages an item span and
/// nothing else — a salvage tally and an XP line per fighter can outrun the
/// box, and a text-row page loses its tail in silence. Nothing is selected:
/// the page opens on its first line, and every line is in the log pane
/// once the popup is gone.
pub(super) fn tactical_result_rows(outcomes: &[LogEntry]) -> Vec<Row> {
    let mut rows = if outcomes.is_empty() {
        vec![text_row("The fight is over.")]
    } else {
        history_rows(outcomes, usize::MAX)
    };
    rows.push(text_row(""));
    rows.push(text_row("[any key] continue"));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_rect_fill_count, painted_text, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::{DifficultyMode, Game};

    fn assets() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// A game standing in a tactical fight, opened the way the router opens
    /// one.
    fn fighting() -> Game {
        for seed in 0..200u32 {
            let mut game =
                Game::new(seed, DifficultyMode::Forgiving, &assets()).expect("the assets parse");
            let mut profile = game.profile().clone();
            profile.tactical_battles = true;
            game.install_profile(profile);
            let at = game.player_status().position;
            let target = game
                .view_entities(12, 12)
                .into_iter()
                .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
                .find(|e| (e.pos.0 - at.0).abs() + (e.pos.1 - at.1).abs() == 1);
            let Some(target) = target else { continue };
            game.move_player(target.pos.0 - at.0, target.pos.1 - at.1);
            if game.in_tactical_battle() {
                return game;
            }
        }
        panic!("no seed under 200 put a lone wild program next to the player");
    }

    fn pane() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    /// The grid draws a mark for every body standing on it. Without this a
    /// silently-empty battle map passes every census in the file.
    #[test]
    fn every_body_on_the_board_is_drawn() {
        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        let painted = painted_text(&shapes);
        for body in &view.bodies {
            assert!(
                painted.iter().any(|t| t == &body.glyph.to_string()),
                "{:?} was not drawn on the battle map",
                body.glyph
            );
        }
    }

    /// A finished tactical fight ends in a popup over its own board, and
    /// never on the group model's battle screen — the bug this screen fixed.
    #[test]
    fn a_finished_fight_draws_its_results_over_the_board() {
        let mut game = fighting();
        for _ in 0..4000 {
            if !game.has_active_battle() {
                break;
            }
            game.tactical_auto_beat();
        }
        assert!(!game.has_active_battle(), "the fight never resolved");
        assert!(
            game.tactical_result_view().is_some(),
            "the fight left no board"
        );
        let outcomes = game.battle_outcomes();
        let first = outcomes
            .first()
            .expect("the fight reported nothing")
            .text
            .clone();

        let mut app = crate::render::test_support::playing_app_around(game);
        app.mode = Mode::TacticalResult;
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| crate::render::draw(&mut app, &mut fx, p, false));
        let painted = painted_text(&shapes);

        assert!(
            painted.iter().any(|t| t == "Fight Over"),
            "no results popup was drawn"
        );
        assert!(
            painted.iter().any(|t| t.contains(first.as_str())),
            "the popup does not list the fight's first result, {first:?}"
        );
        // The ground's own fill and not a body's glyph: the player's `@` is
        // drawn by the surface map too, so a glyph passes with no board.
        assert!(
            painted_rect_fill_count(&shapes, cell_color(BattleCell::Open)) > 0,
            "the board the fight ended on is not under the popup"
        );
        assert!(
            !painted.iter().any(|t| t.starts_with("Hostile programs")),
            "the fight ended on the group model's battle screen"
        );
    }

    /// A cloaked body fades, and an uncloaked one does not.
    ///
    /// Read off the **alpha** rather than the colour: the map dims every
    /// glyph it draws by a vignette and a per-tile shade, so rgb moves for
    /// reasons that have nothing to do with a cloak — and alpha is the one
    /// channel `ConRead::glyph_ink` carries through untouched, which is what
    /// makes the same multiply fade a sprite's tint too.
    #[test]
    fn a_cloaked_body_draws_faded_and_an_uncloaked_one_does_not() {
        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let subject = view
            .bodies
            .iter()
            .find(|b| b.is_hostile)
            .cloned()
            .expect("a hostile stands on the board");

        let alpha_of = |cloaked: bool| {
            let mut view = view.clone();
            for body in &mut view.bodies {
                if body.entity == subject.entity {
                    body.cloaked = cloaked;
                }
            }
            let mut fx = Fx::new();
            let (_, shapes) = with_painter(|p| {
                draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
            });
            crate::paint::painted_map_glyphs(&shapes)
                .into_iter()
                .find(|(text, _)| text == &subject.glyph.to_string())
                .map(|(_, c)| c.a)
                .expect("the body was not drawn at all")
        };

        let plain = alpha_of(false);
        let faded = alpha_of(true);
        assert!(plain > 0.9, "an uncloaked body drew faded already: {plain}");
        assert!(
            (faded - plain * FADED_ALPHA).abs() < 0.02,
            "a cloaked body drew at alpha {faded}, not {} — one multiply at the draw site",
            plain * FADED_ALPHA
        );
    }

    /// A Hallucination's fake draws the invoker's own glyph at `FADED_ALPHA`
    /// — one constant with the cloak fade above, because a decoy means the
    /// same thing a cloaked body does: not really there.
    ///
    /// **(M)** Dropping the `* FADED_ALPHA` multiply at the decoy draw site
    /// makes this fail the same way removing the cloak's own multiply would
    /// fail the test above.
    #[test]
    fn a_decoy_draws_the_invokers_glyph_faded() {
        use feral_processes_engine::components::GlyphColor;
        use feral_processes_engine::tactical::view::DecoyView;

        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        let occupied: std::collections::HashSet<(i32, i32)> =
            view.bodies.iter().map(|b| b.cell).collect();
        let free = view
            .board
            .cells()
            .find(|(cell, kind)| *kind != BattleCell::Blocked && !occupied.contains(cell))
            .map(|(cell, _)| cell)
            .expect("the board has a free cell for a decoy");
        view.decoys = vec![DecoyView {
            cell: free,
            glyph: 'Q',
            color: GlyphColor::Red,
            of_player: false,
        }];
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        let (_, color) = crate::paint::painted_map_glyphs(&shapes)
            .into_iter()
            .find(|(text, _)| text == "Q")
            .expect("the decoy was not drawn at all");
        assert!(
            (color.a - FADED_ALPHA).abs() < 0.02,
            "a decoy drew at alpha {}, not {FADED_ALPHA}",
            color.a
        );
    }

    /// A profiled hostile's forecast draws its walk as a run of `FORECAST`
    /// segments and marks its target — and an empty walk, which is every
    /// forecast above temperature zero (`Game::tactical_forecast`'s own
    /// rule), draws none.
    ///
    /// **The reach wash is left standing**, which is the point of the role:
    /// the wash is `PLAN` and the forecast is not, so the two are counted
    /// apart on a board that is drawing both at once. This test used to have
    /// to `view.reachable.clear()` to see the forecast at all, and a player
    /// cannot clear the wash.
    #[test]
    fn the_forecast_draws_its_walk_and_marks_its_target() {
        use feral_processes_engine::tactical::view::ForecastView;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        assert!(
            !view.reachable.is_empty(),
            "fixture: the acting body has a reach wash for the forecast to be told from"
        );
        let subject = view.order[0].entity;
        let from = view
            .bodies
            .iter()
            .find(|b| b.entity == subject)
            .expect("the first rung stands on the board")
            .cell;
        let steps = vec![(from.0 + 1, from.1), (from.0 + 2, from.1)];

        let draw = |forecast: Option<ForecastView>| {
            let mut view = view.clone();
            for row in &mut view.order {
                if row.entity == subject {
                    row.forecast = forecast.clone();
                }
            }
            let mut fx = Fx::new();
            with_painter(|p| draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24))
                .1
        };

        let quiet = draw(Some(ForecastView {
            action: "swing".to_string(),
            walk: Vec::new(),
            target: None,
        }));
        assert_eq!(
            crate::paint::painted_line_count_in(&quiet, palette::FORECAST),
            0,
            "a forecast with no walk drew a FORECAST line anyway"
        );

        let planned = draw(Some(ForecastView {
            action: "swing".to_string(),
            walk: steps.clone(),
            target: Some(steps[1]),
        }));
        assert!(
            crate::paint::painted_line_count_in(&planned, palette::FORECAST) > 0,
            "a planned walk drew no FORECAST line"
        );
        assert!(
            crate::paint::painted_rect_stroke_count(&planned, palette::FORECAST) > 0,
            "the forecast's target went unmarked"
        );
        // The forecast's own channel, asserted against the wash it is drawn
        // over: the reach field is still showing, and neither the walk nor
        // the target mark may be the colour it is painted in.
        assert_eq!(
            crate::paint::painted_line_count_in(&planned, palette::PLAN),
            crate::paint::painted_line_count_in(&quiet, palette::PLAN),
            "the forecast's walk drew PLAN lines, which is the reach wash's colour"
        );
        assert_eq!(
            crate::paint::painted_rect_stroke_count(&planned, palette::PLAN),
            crate::paint::painted_rect_stroke_count(&quiet, palette::PLAN),
            "the forecast's target mark drew in PLAN, the reach wash's colour"
        );
    }

    /// A rare-spawn tier draws a bar along the tile's top edge — through
    /// `marks::draw_rarity_bar`, the surface map's own function, so a silver
    /// or gold body reads the same colour on both grids rather than being
    /// redrawn from a second copy of `rarity_color`.
    #[test]
    fn a_rare_bodys_tile_wears_the_rarity_bar() {
        use feral_processes_engine::components::Rarity;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let subject = view.bodies[0].entity;
        let gold = crate::render::rarity_color(Rarity::Gold).expect("gold has a colour");

        let bar_count = |rarity: Rarity| {
            let mut view = view.clone();
            for body in &mut view.bodies {
                if body.entity == subject {
                    body.rarity = rarity;
                }
            }
            let mut fx = Fx::new();
            let (_, shapes) = with_painter(|p| {
                draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
            });
            painted_rect_fill_count(&shapes, gold)
        };

        assert_eq!(
            bar_count(Rarity::Ordinary),
            0,
            "an ordinary body must draw no rarity bar"
        );
        assert!(
            bar_count(Rarity::Gold) > bar_count(Rarity::Ordinary),
            "a gold body's tile drew no rarity bar — the player can't tell it apart on the board"
        );
    }

    /// The cursor is drawn last, so it is never under a body it points at.
    #[test]
    fn the_cursor_is_drawn_over_the_bodies() {
        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let mut fx = Fx::new();
        let cell = acting_body(&view).expect("somebody is acting").cell;
        let (_, with) = with_painter(|p| {
            draw_tactical_map(&view, Some(cell), &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        let mut fx = Fx::new();
        let (_, without) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        assert!(
            with.len() > without.len(),
            "the cursor painted nothing at all"
        );
    }

    /// The downward-pointing, horizontally symmetric triangles among a set
    /// of polygons, as `(apex, width, height)`.
    ///
    /// The discriminator is the *apex being centred between the other two
    /// points*, which is what tells this arrow from the con earmark — the
    /// only other polygon on this grid, a right-angled wedge folded into a
    /// corner whose third point sits directly under one of its neighbours.
    /// It has to be told apart by shape and not by colour: `palette::glyph`
    /// resolves `GlyphColor::Red` to `THREAT` and `Blue` to `PLAN`, so an
    /// earmark can be painted in either of the two colours this arrow uses.
    fn arrows(polys: &[Vec<(f32, f32)>]) -> Vec<((f32, f32), f32, f32)> {
        polys
            .iter()
            .filter(|p| p.len() == 3)
            .filter_map(|p| {
                let (a, b, c) = (p[0], p[1], p[2]);
                let centred = ((a.0 + b.0) / 2.0 - c.0).abs() < 0.01;
                (a.1 == b.1 && c.1 > a.1 && centred).then(|| (c, (b.0 - a.0).abs(), c.1 - a.1))
            })
            .collect()
    }

    /// The arrow clears the head of the body it points at, at rest and at
    /// the top of its bounce alike.
    ///
    /// **Geometry and not a screenshot**: what this is really asserting is
    /// that the arrow spends none of the four channels a tile already has
    /// — the top-edge rarity bar, the top-left earmark, the bottom HP bar,
    /// and the glyph in the middle — and the whole of that is the shape
    /// sitting above `py`.
    #[test]
    fn the_turn_arrow_hangs_clear_of_the_body_it_points_at() {
        let tile = 32.0;
        for lift in [0.0, 2.0, 4.0, 12.0] {
            let a = turn_arrow(100.0, 200.0, tile, lift);
            let (base_l, base_r, point) = (a[0], a[1], a[2]);
            assert!(
                point.1 <= 200.0 - TURN_ARROW_GAP,
                "the point touched the tile at lift {lift}: {a:?}"
            );
            assert!(
                base_l.1 < point.1,
                "the arrow is not pointing down at lift {lift}: {a:?}"
            );
            assert!(
                (base_l.0 + base_r.0) / 2.0 == point.0
                    && (point.0 - (100.0 + tile / 2.0)).abs() < 0.01,
                "the arrow is off the middle of its tile at lift {lift}: {a:?}"
            );
            assert!(
                (base_r.0 - base_l.0 - tile * TURN_ARROW_WIDTH).abs() < 0.01
                    && (point.1 - base_l.1 - tile * TURN_ARROW_HEIGHT).abs() < 0.01,
                "the arrow is not the authored size at lift {lift}: {a:?}"
            );
        }
    }

    /// Blue over a body the player commands, red over one the wild side
    /// does — and exactly one arrow, because two would be two claims about
    /// whose turn it is.
    #[test]
    fn the_turn_arrow_names_the_side_whose_turn_it_is() {
        use crate::paint::painted_poly_points;

        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        for hostile in [false, true] {
            let rung = view
                .order
                .iter()
                .position(|r| r.is_hostile == hostile)
                .unwrap_or_else(|| panic!("the fixture fields no hostile == {hostile} body"));
            view.active = Some(rung);
            let mut fx = Fx::new();
            let (_, shapes) = with_painter(|p| {
                draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
            });

            let (mine, theirs) = if hostile {
                (palette::THREAT, palette::PLAN)
            } else {
                (palette::PLAN, palette::THREAT)
            };
            assert_eq!(
                arrows(&painted_poly_points(&shapes, mine)).len(),
                1,
                "a hostile == {hostile} turn must wear exactly one arrow in its own colour"
            );
            assert!(
                arrows(&painted_poly_points(&shapes, theirs)).is_empty(),
                "a hostile == {hostile} turn drew the other side's arrow"
            );
        }
    }

    /// A blow in flight is drawn in the pane, and it is drawn *by the map*
    /// — asserted through the real `draw_tactical_map` rather than by
    /// calling `draw_bolts` itself, which would pass with the call site
    /// deleted.
    #[test]
    fn a_bolt_in_flight_is_drawn_over_the_battle_map() {
        use crate::paint::painted_line_count;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");

        let mut bare = Fx::new();
        bare.begin_frame(0.0, Vec::new(), Vec::new(), Vec::new(), Vec::new(), true);
        let (_, quiet) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut bare, p, pane(), 32.0, 24)
        });

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            Vec::new(),
            Vec::new(),
            vec![feral_processes_engine::BoltCue {
                from: view.bodies[0].cell,
                to: view.bodies[1].cell,
                color: feral_processes_engine::components::GlyphColor::Cyan,
            }],
            Vec::new(),
            true,
        );
        let (_, lit) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });

        assert!(
            painted_line_count(&lit) > painted_line_count(&quiet),
            "a live bolt painted no line the same frame without one did not: \
             {} vs {}",
            painted_line_count(&lit),
            painted_line_count(&quiet)
        );
    }

    /// A landed blow on a battle map washes the defender's own cell with
    /// exactly the flash a raid draws on a structure — asserted through the
    /// real `draw_tactical_map`, `a_bolt_in_flight_is_drawn_over_the_battle_
    /// map`'s reason: calling `draw_tactical_bursts` or `tactical_tile_flash`
    /// directly would pass with the call site inside the map deleted.
    #[test]
    fn a_tactical_hit_washes_the_defenders_cell() {
        use feral_processes_engine::{TacticalFxCue, TacticalFxKind};

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let cell = view.bodies[0].cell;

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![TacticalFxCue {
                pos: cell,
                kind: TacticalFxKind::Hit,
            }],
            true,
        );
        let flash = fx
            .tactical_tile_flash(cell)
            .expect("a hit cue queued this frame is live");
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        assert_eq!(
            painted_rect_fill_count(&shapes, flash),
            1,
            "a landed blow must wash the cell it landed on"
        );
    }

    /// A heal on a battle map draws a `+` over the recipient's own cell —
    /// the same real-map assertion the hit test above makes.
    #[test]
    fn a_tactical_heal_draws_a_plus_over_the_recipients_cell() {
        use feral_processes_engine::{TacticalFxCue, TacticalFxKind};

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let cell = view.bodies[0].cell;

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![TacticalFxCue {
                pos: cell,
                kind: TacticalFxKind::Heal,
            }],
            true,
        );
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        assert!(
            painted_text(&shapes).iter().any(|t| t == "+"),
            "a heal cue must draw a + over the healed body: {:?}",
            painted_text(&shapes)
        );
    }

    /// ...and it bounces.
    ///
    /// **Eight frames spanning the bob's full cycle**, for
    /// `the_caret_bounces_around_the_middle_of_its_slab`'s reason: the phase
    /// is keyed off an `Entity` this test can neither see nor choose, and
    /// for some of the sixty-four buckets a two-sample probe half a period
    /// apart lands on the identical pixel both times.
    #[test]
    fn the_turn_arrow_bounces() {
        use crate::paint::painted_poly_points;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let side = if view.order[view.active.expect("somebody is acting")].is_hostile {
            palette::THREAT
        } else {
            palette::PLAN
        };
        // A fresh `Fx` per frame, so the camera glide — which is stateful
        // across frames — contributes the same offset to every sample and
        // the only thing moving is the bob.
        let ys: Vec<f32> = (0..8)
            .map(|i| {
                let mut fx = Fx::new();
                fx.begin_frame(
                    i as f64 / 8.0,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    true,
                );
                let (_, shapes) = with_painter(|p| {
                    draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
                });
                arrows(&painted_poly_points(&shapes, side))
                    .first()
                    .expect("the arrow is drawn")
                    .0
                    .1
            })
            .collect();
        let min = ys.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max - min > 1.0,
            "the arrow must bounce, not sit still: {ys:?}"
        );
    }

    /// The strip names the round and one mark per body in the order.
    #[test]
    fn the_turn_strip_names_the_round() {
        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_turn_strip(&view, pane(), p, &m));
        let painted = painted_text(&shapes);
        assert!(
            painted.iter().any(|t| t.contains("ROUND")),
            "the turn strip drew no round: {painted:?}"
        );
    }

    /// A hijacked companion's own rung wears `WARN`, not `PLAYER` — its side
    /// of the initiative order (`is_hostile`) does not change, but its turn
    /// is the AI's for as long as the entry lasts.
    #[test]
    fn a_hijacked_companion_s_rung_is_drawn_in_warn() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        let idx = view
            .order
            .iter()
            .position(|r| !r.is_hostile)
            .expect("a party rung stands in the initiative order");
        view.order[idx].taken_over = true;
        let glyph = view.order[idx].glyph;
        assert_eq!(
            view.order.iter().filter(|r| r.glyph == glyph).count(),
            1,
            "two rungs wear {glyph:?}, so a glyph box cannot name one of them"
        );

        let m = ui_metrics(720.0);
        let (_, shapes) = with_painter(|p| draw_turn_strip(&view, pane(), p, &m));
        let (_, color) = crate::paint::painted_map_glyphs(&shapes)
            .into_iter()
            .find(|(text, _)| text == &glyph.to_string())
            .expect("the hijacked rung was not drawn");
        let close = |a: Color, b: Color| {
            (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
        };
        assert!(
            close(color, palette::WARN),
            "a taken-over companion's rung drew {color:?}, not WARN"
        );
    }

    /// The width census: the widest line the tamper block can ever draw —
    /// every tag, `HIJACK`, and the longest shipped routine's own display
    /// name — leaves `draw_tamper_block`'s own guard satisfied.
    ///
    /// **Measured against what the guard measures**, not against the pane.
    /// The block is right-aligned on the strip, so it is dropped whole when
    /// `x < pane.x + strip_inset` — that is, when its `pad * 2 + widest`
    /// exceeds `strip_right - (pane.x + strip_inset)`, and `strip_right` is
    /// `pane.x + pane.w - m.inset` whatever the strip's own width. A census
    /// that measured the bare line against the whole pane could pass while
    /// the block vanished in silence, which is the failure this exists to
    /// catch.
    ///
    /// **(M)** The shipped worst case measures 496.2px into a 766.5px
    /// budget, so the headroom is real and a sixth tag does not spend it —
    /// the mutation that fails this is padding the fixture's tag list out
    /// until the line is wider than the budget, which it does at around
    /// forty extra rungs. Verified and restored.
    #[test]
    fn the_widest_tamper_line_fits_the_map_pane() {
        use crate::render::hud::layout;
        use feral_processes_engine::abilities::AbilityDb;

        let dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
        let (db, warnings) = AbilityDb::load_dir(&dir).expect("the abilities load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let longest = db
            .all()
            .map(|d| d.name.as_str())
            .max_by_key(|name| name.len())
            .expect("at least one routine ships");

        let mut game = fighting();
        let entity = game.tactical_view().expect("the fight is open").order[0].entity;
        let row = TurnRow {
            entity,
            glyph: 'W',
            color: feral_processes_engine::components::GlyphColor::White,
            label: String::new(),
            is_hostile: false,
            hp_fraction: None,
            tags: vec![
                TamperTag::Hot,
                TamperTag::Cold,
                TamperTag::Profiled,
                TamperTag::Injected,
                TamperTag::Hallucinating,
            ],
            forecast: Some(feral_processes_engine::tactical::view::ForecastView {
                action: longest.to_string(),
                walk: Vec::new(),
                target: None,
            }),
            taken_over: true,
        };
        let line = tamper_line(&row).expect("a row wearing every tag says something");

        let m = ui_metrics(900.0);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let map_pane = layout::regions(1280.0, 720.0, char_w, &m, false).map_pane;
            // `draw_turn_strip`'s own right edge, which the block hangs off
            // and which is independent of how wide the strip itself is.
            let strip_right = map_pane.x + map_pane.w - m.inset;
            let budget = strip_right - (map_pane.x + strip_inset(&m));
            // `draw_tamper_block`'s own two figures, in its own spelling.
            let pad = m.line_height * 0.35;
            let width = pad * 2.0 + p.measure_ui_advance(&line, m.small());
            assert!(
                width <= budget,
                "the widest tamper block would be dropped by its own guard, \
                 overflowing by {:.1}px ({width:.1}px into a {budget:.1}px budget): {line:?}",
                width - budget
            );
        });
    }

    /// The action bar says what is left to spend, and drops the two keys
    /// that would be refused once the action is gone.
    #[test]
    fn the_action_bar_follows_what_is_left_of_the_turn() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        view.player_turn = true;
        view.acted = false;
        let open = action_bar(Mode::TacticalBattle, &view, false);
        assert!(open.iter().any(|(k, _)| k == "a"));
        assert!(open.iter().any(|(k, _)| k == "E"));
        assert!(
            open.iter().any(|(k, l)| k == "s" && l == "special"),
            "the special key must read the way the abstract fight's does: {open:?}"
        );
        assert!(
            !open.iter().any(|(k, _)| k == "r"),
            "`r` is not a tactical key any more: {open:?}"
        );

        view.acted = true;
        let spent = action_bar(Mode::TacticalBattle, &view, false);
        // `R` and `A` both stay: arming auto-attack or driving the rest of
        // the fight are not actions and are not refused on a spent turn —
        // and a turn with nothing left to spend is exactly when a player
        // decides they would rather watch the rest of the fight.
        assert_eq!(
            spent.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            vec!["E", "R", "A"],
            "a spent turn still offered an action"
        );
    }

    /// **The bar is one row on a border with no wrap and no clip**, so
    /// widening the movement cell to name the numpad is a measured question
    /// and not a guess — `the_keybar_fits_the_log_pane`'s rule, asked of
    /// the content that replaces that bar in a fight. Measured through
    /// `keybar_segments` and `fitting` rather than against a hand-built
    /// list, so the thing measured is the thing drawn.
    #[test]
    fn the_action_bar_fits_the_log_pane() {
        use crate::render::hud::{layout, strip::fitting};

        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        // The fixture may open on the wild side, whose bar is one short
        // sentence — measuring that would pass against any width at all.
        view.player_turn = true;
        view.acted = false;
        // Three bars: the cursor's is its own content at its own width, a
        // census over one of them passes against the other overflowing, and
        // the board's bar is one row longer while auto-attack is on offer —
        // which is the row that has to be measured rather than assumed to fit.
        for (mode, auto) in [
            (Mode::TacticalBattle, false),
            (Mode::TacticalBattle, true),
            (Mode::TacticalAim, false),
        ] {
            let actions = action_bar(mode, &view, auto);
            assert!(
                auto || actions.len() >= 3,
                "not a full bar for {mode:?}: {actions:?}"
            );
            for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0)] {
                let m = ui_metrics(h);
                with_painter(|p| {
                    let char_w = p.measure_ui_advance("M", m.font_size);
                    let pane = layout::regions(w, h, char_w, &m, false).log_pane;
                    let avail = pane.w - m.inset * 2.0;
                    let segments = crate::render::hud::log_frame::keybar_segments(Some(&actions));
                    let taken = fitting(&segments, avail, p, &m);
                    let drawn: String = taken.iter().map(|(t, _, _)| t.as_str()).collect();
                    let slack = avail - p.measure_ui_advance(&drawn, m.small());

                    // Against the bar as it would be with unlimited room, so
                    // this cannot drift as segments are added: `fitting`
                    // drops from the end, so anything short of the whole is
                    // a key the player cannot see.
                    let whole = fitting(&segments, f32::INFINITY, p, &m);
                    assert_eq!(
                        taken.len(),
                        whole.len(),
                        "{mode:?}'s bar (auto {auto}) dropped a key at {w}x{h} — slack {slack:.1}px: {drawn:?}"
                    );
                    assert!(
                        slack >= 0.0,
                        "{mode:?}'s bar (auto {auto}) overhangs its pane by {slack:.1}px"
                    );
                });
            }
        }
    }

    /// The bar is the only place auto-attack is discoverable, and while it is
    /// running it is the only thing the bar has to say: every action key on it
    /// would stop it rather than do what it names.
    #[test]
    fn the_action_bar_offers_auto_attack_and_says_when_it_is_running() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        view.player_turn = true;
        view.acted = false;

        let offered = action_bar(Mode::TacticalBattle, &view, false);
        assert!(
            offered.iter().any(|(k, _)| k == "A"),
            "auto-attack is bound to nothing the player can see: {offered:?}"
        );

        let running = action_bar(Mode::TacticalBattle, &view, true);
        for action in ["a", "d", "s", "E"] {
            assert!(
                !running.iter().any(|(k, _)| k == action),
                "{action:?} would stop auto-attack rather than act, so the bar must not offer it: {running:?}"
            );
        }
        assert!(
            running.iter().any(|(_, l)| l.contains("auto-attack")),
            "a running auto-attack went unsaid: {running:?}"
        );
    }

    /// The cursor is a different screen from the board, and offering the
    /// board's keys on it names three that do nothing and hides the two
    /// that commit and cancel.
    #[test]
    fn the_aim_cursor_gets_its_own_keys() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        view.player_turn = true;
        view.acted = false;

        let rows = action_bar(Mode::TacticalAim, &view, false);

        assert!(
            rows.iter().any(|(k, _)| k == "Enter"),
            "the cursor's commit key is not on the bar: {rows:?}"
        );
        assert!(
            rows.iter().any(|(k, _)| k == "Esc"),
            "the cursor's cancel key is not on the bar: {rows:?}"
        );
        for dead in ["a", "s", "E"] {
            assert!(
                !rows.iter().any(|(k, _)| k == dead),
                "{dead:?} does nothing while aiming, so the bar must not offer it: {rows:?}"
            );
        }
    }

    /// The wild side's turn says so rather than offering keys that would be
    /// swallowed.
    #[test]
    fn the_action_bar_waits_out_the_wild_side() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        view.player_turn = false;
        let rows = action_bar(Mode::TacticalBattle, &view, false);
        assert!(rows.iter().all(|(k, _)| k.is_empty()));
    }

    /// Where one body's glyph was drawn, for a caller comparing the framing
    /// of two frames.
    ///
    /// The glyph and not the tile under it: a cell's own rect is one of
    /// several hundred identical boxes, and a body's ink is the only thing on
    /// this grid that can be named.
    fn glyph_box(shapes: &[bevy_egui::egui::epaint::ClippedShape], glyph: char) -> Option<Rect> {
        crate::paint::painted_text_boxes(shapes)
            .into_iter()
            .find(|(_, text, _)| text == &glyph.to_string())
            .map(|(_, _, r)| r)
    }

    /// A finished fight's board has nobody acting, and its results popup is
    /// read for as long as the player likes — so the camera stays where the
    /// fight left it rather than letting the hold lapse and swinging to the
    /// middle of the board under the popup.
    #[test]
    fn a_frozen_board_keeps_the_camera_where_the_fight_left_it() {
        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let acting = acting_body(&view).expect("somebody is acting");
        let middle = (view.board.side / 2, view.board.side / 2);
        assert_ne!(
            acting.cell, middle,
            "the acting body stands mid-board, so no framing could differ"
        );
        let glyph = acting.glyph;
        assert_eq!(
            view.bodies.iter().filter(|b| b.glyph == glyph).count(),
            1,
            "two bodies wear {glyph:?}, so a glyph box cannot name one of them"
        );
        let frozen = view.clone().frozen();

        let mut fx = Fx::new();
        let frame = |fx: &mut Fx, at: f64, v: &TacticalView| {
            fx.begin_frame(at, Vec::new(), Vec::new(), Vec::new(), Vec::new(), true);
            let (_, shapes) =
                with_painter(|p| draw_tactical_map(v, None, &[], &[], fx, p, pane(), 32.0, 24));
            shapes
        };

        let fought = frame(&mut fx, 0.0, &view);
        let mut t = 0.0;
        // Frame by frame, as a player reading the popup draws it, well past
        // the dwell that would release a hold nobody refreshes.
        while t < crate::fx::CAMERA_DWELL_SECONDS * 10.0 {
            t += 0.016;
            frame(&mut fx, t, &frozen);
        }
        let read = frame(&mut fx, t + 0.016, &frozen);

        assert!(
            glyph_box(&fought, glyph).is_some(),
            "the acting body was not drawn at all"
        );
        assert_eq!(
            glyph_box(&fought, glyph),
            glyph_box(&read, glyph),
            "the camera left the fight's last framing while its results were read"
        );
    }

    /// The camera stays on the body that just acted, and moves once the blow
    /// has been read.
    ///
    /// **The whole point of the dwell.** `Game::hand_on_turn` fires inside
    /// the same call that resolves an attack, so the acting body is the next
    /// one the instant a blow lands; a camera aimed at that walks off the
    /// attacker while the streak is still in flight.
    ///
    /// Measured by where an *unrelated* body's glyph lands, which moves only
    /// when the framing does — the held frame must be pixel-identical to the
    /// one before it, and the frame past the dwell must not be.
    #[test]
    fn the_camera_holds_the_body_that_just_acted_and_then_moves() {
        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let acting = view.active.expect("somebody is acting");
        let next = (acting + 1) % view.order.len();
        let (first, second) = (view.order[acting].entity, view.order[next].entity);
        assert_ne!(first, second, "the order has one rung, so this is vacuous");
        let cell_of = |e| {
            view.bodies
                .iter()
                .find(|b| b.entity == e)
                .expect("a body in the order stands on the board")
                .cell
        };
        assert_ne!(
            cell_of(first),
            cell_of(second),
            "both bodies stand on one cell, so no framing could differ"
        );
        // The framing is read off the body that *acted*: it is drawn in all
        // three frames and is where the camera is supposed to stay. Its
        // glyph has to be the only one of its kind on the board, or another
        // body wearing it answers instead.
        let glyph = view
            .bodies
            .iter()
            .find(|b| b.entity == first)
            .expect("the acting body stands on the board")
            .glyph;
        assert_eq!(
            view.bodies.iter().filter(|b| b.glyph == glyph).count(),
            1,
            "two bodies wear {glyph:?}, so a glyph box cannot name one of them"
        );

        let mut handed_on = view.clone();
        handed_on.active = Some(next);

        let mut fx = Fx::new();
        let frame = |fx: &mut Fx, at: f64, v: &TacticalView| {
            fx.begin_frame(at, Vec::new(), Vec::new(), Vec::new(), Vec::new(), true);
            let (_, shapes) =
                with_painter(|p| draw_tactical_map(v, None, &[], &[], fx, p, pane(), 32.0, 24));
            shapes
        };

        let acted = frame(&mut fx, 0.0, &view);
        let held = frame(&mut fx, 0.02, &handed_on);
        let released = frame(&mut fx, crate::fx::CAMERA_DWELL_SECONDS + 0.01, &handed_on);

        let (a, b, c) = (
            glyph_box(&acted, glyph),
            glyph_box(&held, glyph),
            glyph_box(&released, glyph),
        );
        assert!(a.is_some(), "the body that acted was not drawn at all");
        assert_eq!(
            a, b,
            "the camera moved off the body that acted before the blow could be read"
        );
        assert_ne!(
            b, c,
            "the camera never left the body that acted, so the dwell is a freeze"
        );
    }

    /// Nothing is drawn outside the map pane, even mid-pan.
    ///
    /// The acting body's tile used to be within one tile of pane centre by
    /// construction, so the arrow over its head and the aim cursor on its
    /// cell were both drawn unchecked. Holding the camera on the body that
    /// acted *last* makes that tile reachable from anywhere on the board,
    /// and the pane is a region of a shared screen: a mark drawn outside it
    /// lands on the HUD.
    #[test]
    fn nothing_is_drawn_outside_the_pane_while_the_camera_pans() {
        use crate::paint::{painted_poly_points, painted_rect_stroke_boxes};

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let acting = view.active.expect("somebody is acting");
        // A pane three tiles across, so a body a few cells off centre is
        // unambiguously outside it.
        let tight = Rect::new(40.0, 40.0, 200.0, 200.0);
        let far = (view.board.side - 1, view.board.side - 1);

        let mut far_away = view.clone();
        let acting_entity = far_away.order[acting].entity;
        for body in &mut far_away.bodies {
            if body.entity == acting_entity {
                body.cell = far;
            }
        }

        let mut fx = Fx::new();
        // Latch the camera on the board's middle, then hand the turn to a
        // body standing in the far corner inside the dwell.
        let mut middle = view.clone();
        for body in &mut middle.bodies {
            if body.entity == acting_entity {
                body.cell = (view.board.side / 2, view.board.side / 2);
            }
        }
        fx.begin_frame(0.0, Vec::new(), Vec::new(), Vec::new(), Vec::new(), true);
        with_painter(|p| draw_tactical_map(&middle, None, &[], &[], &mut fx, p, tight, 32.0, 24));

        fx.begin_frame(0.02, Vec::new(), Vec::new(), Vec::new(), Vec::new(), true);
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&far_away, Some(far), &[], &[], &mut fx, p, tight, 32.0, 24)
        });

        let inside = |x: f32, y: f32| {
            x >= tight.x - 1.0
                && y >= tight.y - 1.0
                && x <= tight.x + tight.w + 1.0
                && y <= tight.y + tight.h + 1.0
        };
        for colour in [palette::THREAT, palette::PLAN] {
            for tri in painted_poly_points(&shapes, colour) {
                for (x, y) in tri {
                    assert!(
                        inside(x, y),
                        "an arrow point landed at {x},{y}, outside the pane"
                    );
                }
            }
        }
        for r in painted_rect_stroke_boxes(&shapes, palette::EMPHASIS) {
            assert!(
                inside(r.min.x, r.min.y) && inside(r.max.x, r.max.y),
                "the aim cursor was drawn at {r:?}, outside the pane"
            );
        }
    }

    /// The reach field is *outlined*, and the outline is its boundary rather
    /// than a box around every cell.
    ///
    /// The wash alone is 0.13 of one hue over a near-black tile, which is
    /// legible only to somebody already looking at the right part of the
    /// board; the border at full strength is what makes it findable at a
    /// glance. Counted against the field's own perimeter rather than
    /// asserted to be non-zero, because outlining every reachable cell whole
    /// draws lines too and is the failure this is written against.
    ///
    /// A pane wide enough for the whole board, deliberately: the draw loop
    /// skips a cell outside it, so a tighter pane would make the expected
    /// count depend on where the acting body happens to stand.
    #[test]
    fn the_reach_field_is_outlined_along_its_boundary() {
        use crate::paint::painted_line_count_in;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        assert!(
            !view.reachable.is_empty(),
            "the acting body can reach nowhere, so this test would be vacuous"
        );
        let wide = Rect::new(0.0, 0.0, 1400.0, 1000.0);
        let mut fx = Fx::new();
        let (_, shapes) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &[], &mut fx, p, wide, 32.0, 24));

        let expected: usize = view
            .reachable
            .iter()
            .map(|&(x, y)| {
                [(0, -1), (0, 1), (-1, 0), (1, 0)]
                    .iter()
                    .filter(|&&(dx, dy)| !view.reachable.contains(&(x + dx, y + dy)))
                    .count()
            })
            .sum();
        assert_eq!(
            painted_line_count_in(&shapes, palette::PLAN),
            expected,
            "the boundary drawn is not the field's own perimeter"
        );
    }

    /// ...and nothing is outlined when the acting body can reach nowhere, so
    /// the census above cannot be passing on a border drawn unconditionally.
    #[test]
    fn an_empty_reach_field_is_not_outlined() {
        use crate::paint::painted_line_count_in;

        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        view.reachable.clear();
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        assert_eq!(painted_line_count_in(&shapes, palette::PLAN), 0);
    }

    /// The placeable field — where a `Radius` routine's centre may legally
    /// land — is outlined the same way the reach field is: along its
    /// boundary, tinted `AIM` rather than `PLAN`.
    ///
    /// `view.reachable` stands in for the placeable set here: `draw_
    /// tactical_map` takes it as its own parameter and never reads
    /// `TacticalView` for it, so any non-empty cell set proves the same
    /// boundary rule, and reusing the field this file's own reach test
    /// already establishes is non-empty keeps the fixture from inventing a
    /// second one.
    #[test]
    fn the_placeable_field_is_outlined_along_its_boundary() {
        use crate::paint::painted_line_count_in;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        assert!(
            !view.reachable.is_empty(),
            "the acting body can reach nowhere, so this test would be vacuous"
        );
        let placeable = view.reachable.clone();
        let wide = Rect::new(0.0, 0.0, 1400.0, 1000.0);
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &placeable, &mut fx, p, wide, 32.0, 24)
        });

        let expected: usize = placeable
            .iter()
            .map(|&(x, y)| {
                [(0, -1), (0, 1), (-1, 0), (1, 0)]
                    .iter()
                    .filter(|&&(dx, dy)| !placeable.contains(&(x + dx, y + dy)))
                    .count()
            })
            .sum();
        assert_eq!(
            painted_line_count_in(&shapes, palette::AIM),
            expected,
            "the boundary drawn is not the placeable field's own perimeter"
        );
    }

    /// The state every call site but the one above passes — no routine being
    /// aimed, or one aimed that is not a `Radius` shape — draws nothing in
    /// `AIM`: `Game::tactical_placeable_cells` is already empty then, and
    /// `render/base.rs` hands this function exactly what that call returns.
    #[test]
    fn an_empty_placeable_set_is_not_outlined() {
        use crate::paint::painted_line_count_in;

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        let mut fx = Fx::new();
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        assert_eq!(painted_line_count_in(&shapes, palette::AIM), 0);
    }

    /// The placeable field *replaces* the reach field rather than stacking
    /// on it — the request this outline shipped for was "the same as the
    /// movement outline, but yellow", not a second wash on top of the first.
    ///
    /// Both halves in one test: the reach field's wash and boundary paint
    /// nothing while `placeable` is non-empty, and paint exactly what
    /// `the_reach_wash_is_drawn_for_either_side` and `the_reach_field_is_
    /// outlined_along_its_boundary` already establish once it is empty again
    /// — so a fix that only ever suppresses, or only ever restores, is
    /// still caught.
    #[test]
    fn a_placeable_field_suppresses_the_reach_field() {
        use crate::paint::{painted_line_count_in, painted_rect_fill_count};

        let mut game = fighting();
        let view = game.tactical_view().expect("the fight is open");
        assert!(
            !view.reachable.is_empty(),
            "the acting body can reach nowhere, so this test would be vacuous"
        );
        let c = palette::PLAN;
        let wash = Color::new(c.r, c.g, c.b, REACH_WASH_ALPHA);
        let placeable = view.reachable.clone();

        let mut fx = Fx::new();
        let (_, aiming) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &placeable, &mut fx, p, pane(), 32.0, 24)
        });
        assert_eq!(
            painted_rect_fill_count(&aiming, wash),
            0,
            "the movement wash painted while a placeable field was showing"
        );
        assert_eq!(
            painted_line_count_in(&aiming, palette::PLAN),
            0,
            "the movement boundary painted while a placeable field was showing"
        );

        let mut fx = Fx::new();
        let (_, not_aiming) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        assert!(
            painted_rect_fill_count(&not_aiming, wash) > 0,
            "the movement wash stayed suppressed once placeable went back to empty"
        );
    }

    /// The reach wash is drawn off `reachable` alone, so the wild side's
    /// turn wears it exactly as the party's does.
    ///
    /// Gated on `player_turn` it was invisible for every body the player
    /// does not command, which is the half of the board a player plans
    /// *against* — and the gate is one `&&` that reads as deliberate, so
    /// nothing but this test says the two sides are drawn alike.
    #[test]
    fn the_reach_wash_is_drawn_for_either_side() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        assert!(
            !view.reachable.is_empty(),
            "the acting body can reach nowhere, so this test would be vacuous"
        );
        let c = palette::PLAN;
        let wash = Color::new(c.r, c.g, c.b, REACH_WASH_ALPHA);

        view.player_turn = true;
        let mut fx = Fx::new();
        let (_, mine) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });
        view.player_turn = false;
        let mut fx = Fx::new();
        let (_, theirs) = with_painter(|p| {
            draw_tactical_map(&view, None, &[], &[], &mut fx, p, pane(), 32.0, 24)
        });

        let ours = painted_rect_fill_count(&mine, wash);
        assert!(ours > 0, "the party's own reach wash painted nothing");
        assert_eq!(
            painted_rect_fill_count(&theirs, wash),
            ours,
            "the wild side's turn drew a different reach wash from the party's"
        );
    }
}
