use super::*;

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
        with_painter(|p| {
            draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
        })
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
            draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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

fn test_prop(cell: (i32, i32), hp: Option<u32>, volatile: bool, decoration: bool) -> PropView {
    PropView {
        cell,
        name: "test piece".into(),
        sprite: "prop_test_piece".into(),
        hp,
        max_hp: hp.map(|_| 10),
        volatile,
        decoration,
    }
}

/// With no art loaded, each class of prop draws its one glyph, the
/// decoration dimmer than the cover, and an empty board draws none.
#[test]
fn props_fall_back_to_one_glyph_per_class_and_decoration_is_dimmer() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let at = acting_body(&view).expect("somebody is acting").cell;
    view.props = vec![
        test_prop(at, None, false, false),
        test_prop((at.0 + 1, at.1), Some(10), false, false),
        test_prop((at.0 + 2, at.1), Some(5), true, false),
        test_prop((at.0 + 3, at.1), None, false, true),
    ];
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let glyphs = crate::paint::painted_map_glyphs(&shapes);
    // Each prop stands in its own light, so the ink is compared with
    // that light taken back off — the class is what is under test.
    let lighting = Lighting::of(&view.board);
    let ink = |g: &str, cell: (i32, i32)| {
        let c = glyphs
            .iter()
            .find(|(t, _)| t == g)
            .unwrap_or_else(|| panic!("no {g:?} among {glyphs:?}"))
            .1;
        let lit = lighting.at(cell);
        let unlit = |ch: f32, cast: f32| ch / (lit.level * cast);
        Color::new(
            unlit(c.r, lit.cast.r),
            unlit(c.g, lit.cast.g),
            unlit(c.b, lit.cast.b),
            c.a,
        )
    };
    let (wall, debris, volatile, decor) = (
        ink("#", at),
        ink("%", (at.0 + 1, at.1)),
        ink("*", (at.0 + 2, at.1)),
        ink(",", (at.0 + 3, at.1)),
    );
    let same = |a: Color, b: Color| {
        (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
    };
    assert!(same(wall, debris), "{wall:?} {debris:?}");
    assert!(same(wall, volatile), "{wall:?} {volatile:?}");
    assert!(decor.g < wall.g, "decoration must read dimmer than cover");

    let mut fx = Fx::new();
    view.props.clear();
    let (_, bare) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert!(
        !crate::paint::painted_map_glyphs(&bare)
            .iter()
            .any(|(t, _)| ["#", "%", "*", ","].contains(&t.as_str())),
        "a board with no props drew a prop glyph"
    );
}

/// A prop under half its hit points tries its cracked art first, and one
/// at full strength never does.
#[test]
fn a_damaged_prop_tries_its_cracked_sprite_first() {
    let hurt = PropView {
        hp: Some(4),
        ..test_prop((0, 0), Some(10), false, false)
    };
    assert_eq!(
        prop_sprite_keys(&hurt),
        ["prop_test_piece_cracked", "prop_test_piece"]
    );
    let whole = test_prop((0, 0), Some(10), false, false);
    assert_eq!(prop_sprite_keys(&whole), ["prop_test_piece"]);
    let wall = test_prop((0, 0), None, false, false);
    assert_eq!(prop_sprite_keys(&wall), ["prop_test_piece"]);
}

/// The cursor is drawn last, so it is never under a body it points at.
#[test]
fn the_cursor_is_drawn_over_the_bodies() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let mut fx = Fx::new();
    let cell = acting_body(&view).expect("somebody is acting").cell;
    let (_, with) = with_painter(|p| {
        draw_tactical_map(
            &view,
            &Overlay {
                cursor: Some(cell),
                ..Default::default()
            },
            &mut fx,
            p,
            pane(),
            32.0,
            24,
        )
    });
    let mut fx = Fx::new();
    let (_, without) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert!(
        with.len() > without.len(),
        "the cursor painted nothing at all"
    );
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
            (base_l.0 + base_r.0) / 2.0 == point.0 && (point.0 - (100.0 + tile / 2.0)).abs() < 0.01,
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
            draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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

/// The cover sunrise is drawn, and only for a body the view marks: one
/// horizon line and one half-disc standing on it, in the top-right
/// corner rather than the con earmark's top-left.
#[test]
fn a_body_in_cover_wears_a_mark() {
    use crate::paint::painted_poly_points;

    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    for body in &mut view.bodies {
        body.in_cover = false;
    }
    let mut fx = Fx::new();
    let (_, bare) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let bare_polys = painted_poly_points(&bare, palette::HEALTHY);
    let before = bare_polys.len();

    view.bodies[0].in_cover = true;
    let mut fx = Fx::new();
    let (_, marked) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let after = painted_poly_points(&marked, palette::HEALTHY);
    assert_eq!(
        after.len(),
        before + 1,
        "one body in cover should add exactly one sun"
    );
    assert_eq!(
        crate::paint::painted_line_count_in(&marked, palette::HEALTHY),
        crate::paint::painted_line_count_in(&bare, palette::HEALTHY) + 1,
        "one body in cover should add exactly one horizon"
    );
    // A sunrise: a dome whose flat side is its base. The two ends of
    // that base are the lowest points and share a y, and nothing of the
    // dome hangs below it.
    let sun = after
        .iter()
        .find(|pts| !bare_polys.contains(pts))
        .expect("the new mark is not among the shapes drawn");
    let left = sun
        .iter()
        .copied()
        .fold((f32::MAX, 0.0), |a, p| if p.0 < a.0 { p } else { a });
    let right = sun
        .iter()
        .copied()
        .fold((f32::MIN, 0.0), |a, p| if p.0 > a.0 { p } else { a });
    let horizon = sun.iter().map(|&(_, y)| y).fold(f32::MIN, f32::max);
    assert!(
        (left.1 - horizon).abs() < 0.01 && (right.1 - horizon).abs() < 0.01,
        "the sun's ends must sit on its horizon: {sun:?}"
    );
    // Which corner, told against the body's own glyph rather than a
    // colour: up and to the right of it, where the con earmark is up and
    // to the left.
    let mid = (left.0 + right.0) / 2.0;
    let glyph = crate::paint::painted_text_boxes(&marked)
        .into_iter()
        .map(|(_, _, b)| (b.x + b.w / 2.0, b.y + b.h / 2.0))
        .min_by(|a, b| {
            let d = |c: &(f32, f32)| (c.0 - mid).powi(2) + (c.1 - horizon).powi(2);
            d(a).total_cmp(&d(b))
        })
        .expect("the marked body drew a glyph");
    assert!(
        mid > glyph.0 && horizon < glyph.1,
        "the sun at ({mid}, {horizon}) is not top-right of its body at {glyph:?}"
    );
}
