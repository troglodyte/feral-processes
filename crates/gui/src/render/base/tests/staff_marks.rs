use super::*;

/// A staff program an amenity is refilling, `patient_view`'s shape.
fn served_view() -> EntityView {
    EntityView {
        serviced: true,
        ..patient_view(false)
    }
}

/// Where `mark` was painted this frame, or `None` if it was not.
fn mark_y(shapes: &[bevy_egui::egui::epaint::ClippedShape], mark: &str) -> Option<f32> {
    shapes.iter().find_map(|cs| match &cs.shape {
        bevy_egui::egui::Shape::Text(t) if t.galley.text() == mark => Some(t.pos.y),
        _ => None,
    })
}

fn recovery_mark_shapes(fx: &Fx, ev: &EntityView) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
    let (_, shapes) = with_painter(|p| {
        draw_rising_mark(
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
    mark_y(&recovery_mark_shapes(fx, ev), "+")
}

/// **The amenity mark is the recovery mark's shape with a different
/// glyph**: drawn on a program an amenity is refilling, drawn on
/// nothing else, and it moves — sampled across frame times for the
/// same reason the `+` test is. The `+` is also asserted absent, since
/// a body being refilled is not being mended and the two must read
/// apart.
#[test]
fn a_serviced_program_wears_a_floating_caret_and_never_the_cross() {
    let mut fx = Fx::new();
    fx.begin_frame(0.0, FrameCues::default(), false);
    let served = served_view();

    assert!(
        mark_y(&recovery_mark_shapes(&fx, &served), "^").is_some(),
        "a program an amenity is refilling must wear the `^`"
    );
    assert!(
        mark_y(&recovery_mark_shapes(&fx, &served), "+").is_none(),
        "and never the `+`, which is Integrity's"
    );
    assert!(
        mark_y(&recovery_mark_shapes(&fx, &patient_view(false)), "^").is_none(),
        "a program nothing is refilling draws no `^`"
    );

    let ys: Vec<f32> = [0.0, 0.15, 0.3, 0.45, 0.6, 0.75]
        .into_iter()
        .map(|now| {
            fx.begin_frame(now, FrameCues::default(), false);
            mark_y(&recovery_mark_shapes(&fx, &served), "^").expect("drawn every frame")
        })
        .collect();
    assert!(
        ys.iter().any(|y| (y - ys[0]).abs() > 0.5),
        "the `^` must float, not sit still: {ys:?}"
    );
}

/// The Refit Bench's `+` is purple, and a body a Bay is also mending
/// keeps the Bay's green one.
#[test]
fn a_refitting_body_wears_the_mending_plus_and_recovery_outranks_it() {
    use crate::render::hud::palette::{HEALTHY, MENDING};
    use crate::render::marks::rising_mark;
    let refitting = EntityView {
        refitting: true,
        ..patient_view(false)
    };
    assert_eq!(rising_mark(&refitting), Some(('+', MENDING)));
    let both = EntityView {
        refitting: true,
        ..patient_view(true)
    };
    assert_eq!(rising_mark(&both), Some(('+', HEALTHY)));
}

/// A downed program in reach of both wears the `+` alone: recovery
/// outranks the amenity, and two glyphs on one tile would paint through
/// each other.
#[test]
fn recovery_outranks_the_amenity_mark_on_one_body() {
    let mut fx = Fx::new();
    fx.begin_frame(0.0, FrameCues::default(), false);
    let both = EntityView {
        serviced: true,
        ..patient_view(true)
    };
    let shapes = recovery_mark_shapes(&fx, &both);
    assert!(mark_y(&shapes, "+").is_some(), "the `+` is drawn");
    assert!(mark_y(&shapes, "^").is_none(), "and the `^` is not");
}

/// The whole of what this mark ships, in the three states that matter:
/// it is drawn on a program a Bay is mending, it is drawn on *nothing*
/// else, and it moves.
///
/// The motion is not decoration and is asserted rather than assumed —
/// `draw_rising_mark` reads `Fx::recovery_float`, and a mark pinned to the
/// middle of the tile would paint an identical shape every frame and pass
/// a test that only looked for the glyph. Sampled across a spread of
/// frame times rather than at one hand-picked half period, so retuning
/// the bob's rate cannot turn this into a failure that means nothing.
#[test]
fn a_recovering_program_wears_a_floating_mark_and_nothing_else_does() {
    let mut fx = Fx::new();
    fx.begin_frame(0.0, FrameCues::default(), false);

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
            fx.begin_frame(now, FrameCues::default(), false);
            recovery_mark_at(&fx, &busy).expect("the mark is drawn every frame")
        })
        .collect();
    assert!(
        ys.iter().any(|y| (y - ys[0]).abs() > 0.5),
        "the mark must move, not sit still: {ys:?}"
    );

    // It floats *up* off its rest position and never below it — the
    // rest position, which a disabled fx draws at, is the lowest the
    // mark ever sits.
    let mut still = Fx::new();
    still.enabled = false;
    still.begin_frame(0.0, FrameCues::default(), false);
    let rest = recovery_mark_at(&still, &busy).expect("drawn with animations off");
    assert!(
        ys.iter().all(|y| *y <= rest + 0.01),
        "the mark sank below its rest position {rest}: {ys:?}"
    );

    // And it fades as it goes, so it disappears rather than snapping back.
    let alphas: Vec<f32> = [0.0, 0.15, 0.3, 0.45, 0.6, 0.75]
        .into_iter()
        .map(|now| {
            fx.begin_frame(now, FrameCues::default(), false);
            crate::paint::painted_map_glyphs(&recovery_mark_shapes(&fx, &busy))
                .into_iter()
                .find(|(text, _)| text == "+")
                .expect("the mark is drawn every frame")
                .1
                .a
        })
        .collect();
    assert!(
        alphas.iter().any(|a| *a < 0.5),
        "the mark must fade out as it rises: {alphas:?}"
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
    // Animations off, so the mark is fully opaque: egui premultiplies
    // alpha into the colour, and the float's fade would read as a hue drift.
    let mut fx = Fx::new();
    fx.enabled = false;
    fx.begin_frame(0.0, FrameCues::default(), false);

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

/// **The mark floats above the patient it belongs to, and does not sit
/// on top of it.** `eb6183ef` moved the mark from the Bay onto the
/// patient but kept the Bay's own centring formula — the same baseline
/// the patient's own glyph draws at — so the two painted over each
/// other and, next to a Repair Bay one tile away, read as an
/// indicator on the *Bay* rather than a mark on the body being healed.
/// Pinned against the patient's own glyph, drawn through the tile
/// loop's own centring formula, so a regression back to the shared
/// baseline fails here rather than in play.
#[test]
fn the_recovery_mark_floats_clear_above_the_patients_own_glyph() {
    let mut fx = Fx::new();
    fx.begin_frame(0.0, FrameCues::default(), false);
    let busy = patient_view(true);
    let own_glyph = busy.glyph.to_string();

    // The patient's own glyph, drawn through the exact formula the tile
    // loop's `ch` path uses — centred in the cell, no lift.
    let (_, glyph_shapes) = with_painter(|p| {
        let dims = p.measure_map(&own_glyph, CELL_GLYPH_PX);
        p.map(
            &own_glyph,
            (CELL - dims.width) / 2.0,
            (CELL + dims.height) / 2.0,
            CELL_GLYPH_PX,
            hud::palette::HEALTHY,
        );
    });
    let glyph_top = glyph_shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Text(t) if t.galley.text() == own_glyph => Some(t.pos.y),
            _ => None,
        })
        .expect("the patient's own glyph is drawn");

    let (mark_dims, mark_shapes) = with_painter(|p| {
        let dims = p.measure_map("+", CELL_GLYPH_PX);
        draw_rising_mark(
            p,
            Some(&busy),
            &fx,
            Rect::new(0.0, 0.0, CELL, CELL),
            CELL_GLYPH_PX,
            1.0,
        );
        dims
    });
    let mark_top = mark_y(&mark_shapes, "+").expect("the mark is drawn");
    let mark_bottom = mark_top + mark_dims.height;

    assert!(
        mark_bottom <= glyph_top + 0.01,
        "the mark's ink (top {mark_top}, bottom {mark_bottom}) must sit \
         entirely above the patient's own glyph (top {glyph_top}), not \
         overlap it"
    );
}

/// A body can be sulking and being serviced at once, so the mood pip
/// and the floating `+`/`^` both draw on one tile and must not share
/// pixels. Pinned against the mark's own measured ink at rest, its
/// lowest frame.
#[test]
fn mood_mark_does_not_overlap_rising_mark() {
    let mut fx = Fx::new();
    fx.begin_frame(0.0, FrameCues::default(), false);
    let busy = patient_view(true);
    let (mark_dims, mark_shapes) = with_painter(|p| {
        let dims = p.measure_map("+", CELL_GLYPH_PX);
        draw_rising_mark(
            p,
            Some(&busy),
            &fx,
            Rect::new(0.0, 0.0, CELL, CELL),
            CELL_GLYPH_PX,
            1.0,
        );
        dims
    });
    let mark_bottom = mark_y(&mark_shapes, "+").expect("the mark is drawn") + mark_dims.height;
    let largest = mood_mark(feral_processes_engine::MoodStep::LashingOut)
        .unwrap()
        .0;
    let pip = mood_mark_rect(0.0, 0.0, CELL, largest);
    assert!(
        pip.y >= mark_bottom,
        "the mood pip (top {}) reaches into the rising mark (bottom {mark_bottom})",
        pip.y
    );
}
