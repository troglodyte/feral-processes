use super::*;

/// A covered destination is washed, and a board with none is not.
#[test]
fn a_covered_destination_is_washed() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.covered = Vec::new();
    view.provoking = Vec::new();
    let c = palette::HEALTHY;
    let cover_wash = Color::new(c.r, c.g, c.b, REACH_WASH_ALPHA);
    let mut fx = Fx::new();
    let (_, bare) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert_eq!(painted_rect_fill_count(&bare, cover_wash), 0);

    view.covered = view.reachable.iter().copied().take(1).collect();
    assert_eq!(view.covered.len(), 1, "the acting body reaches nowhere");
    let mut fx = Fx::new();
    let (_, washed) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert_eq!(
        painted_rect_fill_count(&washed, cover_wash),
        1,
        "a covered cell drew no wash"
    );
}

/// A finished board is a result screen: `frozen` clears both marks, so
/// neither can be drawn over one.
#[test]
fn a_finished_board_draws_neither() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.covered = view.reachable.clone();
    for body in &mut view.bodies {
        body.in_cover = true;
    }
    let frozen = view.frozen();
    assert!(frozen.covered.is_empty());
    assert!(frozen.bodies.iter().all(|b| !b.in_cover));
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
    bare.begin_frame(0.0, FrameCues::default(), true);
    let (_, quiet) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut bare, p, pane(), 32.0, 24)
    });

    let mut fx = Fx::new();
    fx.begin_frame(
        0.0,
        FrameCues {
            bolts: vec![feral_processes_engine::BoltCue {
                from: view.bodies[0].cell,
                to: view.bodies[1].cell,
                fx: None,
                color: feral_processes_engine::components::GlyphColor::Cyan,
            }],
            ..Default::default()
        },
        true,
    );
    let (_, lit) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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
        FrameCues {
            tactical_fx: vec![TacticalFxCue {
                pos: cell,
                kind: TacticalFxKind::Hit(7),
            }],
            ..Default::default()
        },
        true,
    );
    let flash = fx
        .tactical_tile_flash(cell)
        .expect("a hit cue queued this frame is live");
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert_eq!(
        painted_rect_fill_count(&shapes, flash),
        1,
        "a landed blow must wash the cell it landed on"
    );
    assert!(
        painted_text(&shapes).iter().any(|t| t == "-7"),
        "a landed blow must float what it dealt: {:?}",
        painted_text(&shapes)
    );
}

/// A heal on a battle map floats `+N` over the recipient's own cell —
/// the same real-map assertion the hit test above makes.
#[test]
fn a_tactical_heal_floats_its_amount_over_the_recipients_cell() {
    use feral_processes_engine::{TacticalFxCue, TacticalFxKind};

    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let cell = view.bodies[0].cell;

    let mut fx = Fx::new();
    fx.begin_frame(
        0.0,
        FrameCues {
            tactical_fx: vec![TacticalFxCue {
                pos: cell,
                kind: TacticalFxKind::Heal(5),
            }],
            ..Default::default()
        },
        true,
    );
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert!(
        painted_text(&shapes).iter().any(|t| t == "+5"),
        "a heal cue must float what it restored over the healed body: {:?}",
        painted_text(&shapes)
    );
}

/// A reaction draws a `!` over the *reactor's* cell, `Heal`'s mark
/// with its own glyph — the one thing that tells an interrupt from an
/// ordinary swing on the board.
#[test]
fn a_reaction_draws_a_bang_over_the_reactors_cell() {
    use feral_processes_engine::{TacticalFxCue, TacticalFxKind};

    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let cell = view.bodies[0].cell;

    let mut fx = Fx::new();
    fx.begin_frame(
        0.0,
        FrameCues {
            tactical_fx: vec![TacticalFxCue {
                pos: cell,
                kind: TacticalFxKind::Reaction,
            }],
            ..Default::default()
        },
        true,
    );
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let text = painted_text(&shapes);
    assert!(
        text.iter().any(|t| t == "!"),
        "a reaction cue must draw a ! over the reactor: {text:?}"
    );
    assert!(
        !text.iter().any(|t| t.starts_with('+')),
        "and not a heal's mark: {text:?}"
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
            fx.begin_frame(i as f64 / 8.0, FrameCues::default(), true);
            let (_, shapes) = with_painter(|p| {
                draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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

/// A wrap's banner names the new round, and lands whole inside the pane
/// — above the middle, where the camera holds the body that is acting.
#[test]
fn a_wrap_banner_names_the_round_inside_the_pane() {
    let mut fx = Fx::new();
    fx.begin_frame(0.0, FrameCues::default(), false);
    let m = ui_metrics(720.0);
    let (_, quiet) = with_painter(|p| draw_round_banner(&fx, pane(), p, &m));
    assert!(painted_text(&quiet).is_empty(), "no wrap, no banner");

    fx.observe_round(Some(2));
    fx.observe_round(Some(3));
    let (_, shapes) = with_painter(|p| draw_round_banner(&fx, pane(), p, &m));
    let boxes = crate::paint::painted_text_boxes(&shapes);
    let (_, _, ink) = boxes
        .iter()
        .find(|(_, text, _)| text == "ROUND 3")
        .unwrap_or_else(|| panic!("the banner drew no round: {boxes:?}"));
    let p = pane();
    assert!(
        ink.x >= p.x && ink.x + ink.w <= p.x + p.w && ink.y >= p.y,
        "{ink:?} spills out of {p:?}"
    );
    assert!(
        ink.y + ink.h < p.y + p.h / 2.0,
        "{ink:?} sits over the acting body"
    );
}
