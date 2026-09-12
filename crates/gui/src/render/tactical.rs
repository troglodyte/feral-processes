//! The battle map, drawn into the map pane.
//!
//! **The map screen's grid with a different tile source.** It takes its
//! origin from `tile_origin_px` and its cell size from `map_cell` exactly as
//! the surface map does, so the camera, the zoom and the pane's own inset
//! are the ones already in use; what it does not share is the surface map's
//! loop, because a battle map has no biomes, no structures, no build sites
//! and no haul marks. The rules the two genuinely share — `ConRead::of` for
//! the con read, `glyph_color` for a body's hue, `Painter::sprite` for its
//! art — are called here rather than restated.
//!
//! No vignette. The surface map dims with the player's Power because the
//! world is seen through a failing signal; a battle map is a discrete arena
//! the fight is lit by, and dimming it would hide the one thing the screen
//! exists to show.

use feral_processes_engine::tactical::map::BattleCell;
use feral_processes_engine::tactical::view::{TacticalBody, TacticalView};

use super::base::{ConRead, tile_origin_px};
use super::hud::layout::strip_inset;
use super::hud::palette;
use super::popup::{PopupSize, draw_popup, item_row, spent_item_row, text_row};
use crate::fx::Fx;
use crate::paint::{Color, Painter, Rect};
use crate::text::Metrics;
use feral_processes_app_core::{Mode, menu_shortcut};
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

/// How much of its ink a cloaked body keeps — see `components::Cloaked`.
///
/// One multiply at the draw site fades **both** halves: `Painter::sprite`'s
/// tint multiplies, and `ConRead::glyph_ink` carries the authored alpha
/// through, so a body's art and its glyph dim together with no sixteenth
/// `Painter` operation and no change to `paint.rs`.
///
/// Faded rather than hidden: a cloaked body is still a wall in
/// `reach::movement_field`, so the cell it stands on is already a tell, and
/// a tell is the right amount of information.
const CLOAKED_ALPHA: f32 = 0.35;

/// The arrow that hangs over whoever is acting: its width and its height as
/// fractions of a tile, and how far its point is held off the tile's top
/// edge at rest.
///
/// **It hangs *above* the tile rather than sitting in it**, which is what
/// keeps it out of every channel a tile already spends: the con earmark owns
/// the top-left corner, the HP bar the bottom edge, and the middle is the
/// glyph or the sprite this arrow exists to point at. Drawn inside the cell
/// it would have to be small enough to dodge all three, and an arrow that
/// small is not the thing a player finds by glancing.
///
/// The gap is what the bob swings out of: the arrow's rest position is its
/// *lowest*, so a lift can never carry it down onto the body.
const TURN_ARROW_WIDTH: f32 = 0.44;
const TURN_ARROW_HEIGHT: f32 = 0.30;
const TURN_ARROW_GAP: f32 = 2.0;

/// How heavily a cell the acting body can still step to is washed.
///
/// Under the aim preview's own 0.22 so a shaped routine reads over it, and
/// faint enough that the terrain under it stays legible: the wash says a
/// cell is *available*, and a cell whose kind it hid would make it say
/// something it does not know.
const REACH_WASH_ALPHA: f32 = 0.13;

/// How thickly the reach field's outer edge is drawn.
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

/// Draws the whole battle map.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tactical_map(
    view: &TacticalView,
    cursor: Option<(i32, i32)>,
    preview: &[(i32, i32)],
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
        if view.reachable.contains(&cell) {
            let c = palette::PLAN;
            painter.rect(
                px,
                py,
                tile_px - 1.0,
                tile_px - 1.0,
                Color::new(c.r, c.g, c.b, REACH_WASH_ALPHA),
            );
            // The field's own edge, drawn a side at a time: a side whose
            // neighbour is reachable too is interior and is left alone. The
            // holes get outlined as well — a body and a `Blocked` cell are
            // both walls in `reach::movement_field`, so a gap in the field
            // is a cell that genuinely cannot be stepped on, and saying so
            // is the same answer as the outer edge gives.
            let far = tile_px - 1.0;
            for (dx, dy, from, to) in [
                (0, -1, (0.0, 0.0), (far, 0.0)),
                (0, 1, (0.0, far), (far, far)),
                (-1, 0, (0.0, 0.0), (0.0, far)),
                (1, 0, (far, 0.0), (far, far)),
            ] {
                if view.reachable.contains(&(cell.0 + dx, cell.1 + dy)) {
                    continue;
                }
                painter.line(
                    px + from.0,
                    py + from.1,
                    px + to.0,
                    py + to.1,
                    REACH_EDGE_PX,
                    palette::PLAN,
                );
            }
        }
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
        ink.a *= CLOAKED_ALPHA;
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
    if let Some(rung) = con.earmark() {
        let c = super::glyph_color(rung);
        let leg = tile_px * 0.28;
        painter.poly(&[(px, py), (px + leg, py), (px, py + leg)], c);
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
            if row.is_hostile {
                super::glyph_color(row.color)
            } else {
                palette::PLAYER
            },
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
pub(super) fn action_bar(mode: Mode, view: &TacticalView) -> Vec<(String, String)> {
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
    ];
    if view.acted {
        rows.retain(|(k, _)| k == "E");
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
        let (_, shapes) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));
        let painted = painted_text(&shapes);
        for body in &view.bodies {
            assert!(
                painted.iter().any(|t| t == &body.glyph.to_string()),
                "{:?} was not drawn on the battle map",
                body.glyph
            );
        }
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
            let (_, shapes) =
                with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));
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
            (faded - plain * CLOAKED_ALPHA).abs() < 0.02,
            "a cloaked body drew at alpha {faded}, not {} — one multiply at the draw site",
            plain * CLOAKED_ALPHA
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
            draw_tactical_map(&view, Some(cell), &[], &mut fx, p, pane(), 32.0, 24)
        });
        let mut fx = Fx::new();
        let (_, without) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));
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
    /// that the arrow spends none of the three channels a tile already has
    /// — the top-left earmark, the bottom HP bar, and the glyph in the
    /// middle — and the whole of that is the shape sitting above `py`.
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
            let (_, shapes) =
                with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));

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
        bare.begin_frame(0.0, Vec::new(), Vec::new(), Vec::new(), true);
        let (_, quiet) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut bare, p, pane(), 32.0, 24));

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
            true,
        );
        let (_, lit) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));

        assert!(
            painted_line_count(&lit) > painted_line_count(&quiet),
            "a live bolt painted no line the same frame without one did not: \
             {} vs {}",
            painted_line_count(&lit),
            painted_line_count(&quiet)
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
                fx.begin_frame(i as f64 / 8.0, Vec::new(), Vec::new(), Vec::new(), true);
                let (_, shapes) = with_painter(|p| {
                    draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24)
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

    /// The action bar says what is left to spend, and drops the two keys
    /// that would be refused once the action is gone.
    #[test]
    fn the_action_bar_follows_what_is_left_of_the_turn() {
        let mut game = fighting();
        let mut view = game.tactical_view().expect("the fight is open");
        view.player_turn = true;
        view.acted = false;
        let open = action_bar(Mode::TacticalBattle, &view);
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
        let spent = action_bar(Mode::TacticalBattle, &view);
        assert_eq!(
            spent.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            vec!["E"],
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
        // Both bars: the cursor's is its own content at its own width, and
        // a census over one of them passes against the other overflowing.
        for mode in [Mode::TacticalBattle, Mode::TacticalAim] {
            let actions = action_bar(mode, &view);
            assert!(
                actions.len() >= 3,
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
                        "{mode:?}'s bar dropped a key at {w}x{h} — slack {slack:.1}px: {drawn:?}"
                    );
                    assert!(
                        slack >= 0.0,
                        "{mode:?}'s bar overhangs its pane by {slack:.1}px"
                    );
                });
            }
        }
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

        let rows = action_bar(Mode::TacticalAim, &view);

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
        let rows = action_bar(Mode::TacticalBattle, &view);
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
            fx.begin_frame(at, Vec::new(), Vec::new(), Vec::new(), true);
            let (_, shapes) =
                with_painter(|p| draw_tactical_map(v, None, &[], fx, p, pane(), 32.0, 24));
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
        fx.begin_frame(0.0, Vec::new(), Vec::new(), Vec::new(), true);
        with_painter(|p| draw_tactical_map(&middle, None, &[], &mut fx, p, tight, 32.0, 24));

        fx.begin_frame(0.02, Vec::new(), Vec::new(), Vec::new(), true);
        let (_, shapes) = with_painter(|p| {
            draw_tactical_map(&far_away, Some(far), &[], &mut fx, p, tight, 32.0, 24)
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
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, wide, 32.0, 24));

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
        let (_, shapes) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));
        assert_eq!(painted_line_count_in(&shapes, palette::PLAN), 0);
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
        let (_, mine) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));
        view.player_turn = false;
        let mut fx = Fx::new();
        let (_, theirs) =
            with_painter(|p| draw_tactical_map(&view, None, &[], &mut fx, p, pane(), 32.0, 24));

        let ours = painted_rect_fill_count(&mine, wash);
        assert!(ours > 0, "the party's own reach wash painted nothing");
        assert_eq!(
            painted_rect_fill_count(&theirs, wash),
            ours,
            "the wild side's turn drew a different reach wash from the party's"
        );
    }
}
