use super::*;

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
        fx.begin_frame(at, FrameCues::default(), true);
        let (_, shapes) =
            with_painter(|p| draw_tactical_map(v, &Overlay::default(), fx, p, pane(), 32.0, 24));
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
        fx.begin_frame(at, FrameCues::default(), true);
        let (_, shapes) =
            with_painter(|p| draw_tactical_map(v, &Overlay::default(), fx, p, pane(), 32.0, 24));
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

/// A body that steps is drawn part-way between its two cells while it
/// glides, not on the cell it has already reached — the renderer half
/// of `Fx::slid_cell`, which drawing at `body.cell` would undo without
/// any `Fx` test noticing. A body other than the acting one, so the
/// camera stays still and the glyph's own movement is all that shows.
#[test]
fn a_stepping_body_is_drawn_mid_glide() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let acting = acting_body(&view).expect("somebody is acting").entity;
    let walker = view
        .bodies
        .iter()
        .find(|b| {
            b.entity != acting && view.bodies.iter().filter(|o| o.glyph == b.glyph).count() == 1
        })
        .expect("a body other than the acting one wears a glyph of its own");
    let (entity, glyph) = (walker.entity, walker.glyph);
    let mut stepped = view.clone();
    let body = stepped
        .bodies
        .iter_mut()
        .find(|b| b.entity == entity)
        .expect("the walker is still on the board");
    body.cell.0 += 1;

    let mut fx = Fx::new();
    let frame = |fx: &mut Fx, at: f64, v: &TacticalView| {
        fx.begin_frame(at, FrameCues::default(), true);
        let (_, shapes) =
            with_painter(|p| draw_tactical_map(v, &Overlay::default(), fx, p, pane(), 32.0, 24));
        glyph_box(&shapes, glyph).expect("the walker was drawn")
    };
    let before = frame(&mut fx, 1.0, &view);
    frame(&mut fx, 1.0 + 1e-3, &stepped);
    let mid = frame(
        &mut fx,
        1.0 + 1e-3 + crate::fx::SLIDE_SECONDS / 2.0,
        &stepped,
    );
    let after = frame(&mut fx, 1.0 + 1e-3 + crate::fx::SLIDE_SECONDS, &stepped);

    assert!(
        (mid.x - (before.x + 16.0)).abs() < 1.0,
        "halfway through the glide the body was drawn at {} (from {} to {})",
        mid.x,
        before.x,
        after.x
    );
    assert!(
        (after.x - (before.x + 32.0)).abs() < 1.0,
        "the glide never landed"
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
    fx.begin_frame(0.0, FrameCues::default(), true);
    with_painter(|p| draw_tactical_map(&middle, &Overlay::default(), &mut fx, p, tight, 32.0, 24));

    fx.begin_frame(0.02, FrameCues::default(), true);
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(
            &far_away,
            &Overlay {
                cursor: Some(far),
                ..Default::default()
            },
            &mut fx,
            p,
            tight,
            32.0,
            24,
        )
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
