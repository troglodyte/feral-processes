use super::*;

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
        with_painter(|p| draw_tactical_map(&view, &Overlay::default(), &mut fx, p, wide, 32.0, 24));

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
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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
        draw_tactical_map(
            &view,
            &Overlay {
                placeable: placeable.clone(),
                ..Default::default()
            },
            &mut fx,
            p,
            wide,
            32.0,
            24,
        )
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
/// `render/base/` hands this function exactly what that call returns.
#[test]
fn an_empty_placeable_set_is_not_outlined() {
    use crate::paint::painted_line_count_in;

    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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
        draw_tactical_map(
            &view,
            &Overlay {
                placeable: placeable.clone(),
                ..Default::default()
            },
            &mut fx,
            p,
            pane(),
            32.0,
            24,
        )
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
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
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
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    view.player_turn = false;
    let mut fx = Fx::new();
    let (_, theirs) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });

    let ours = painted_rect_fill_count(&mine, wash);
    assert!(ours > 0, "the party's own reach wash painted nothing");
    assert_eq!(
        painted_rect_fill_count(&theirs, wash),
        ours,
        "the wild side's turn drew a different reach wash from the party's"
    );
}

/// A view whose acting body can reach, shelter at and be swung at on
/// exactly its own cell — each wash one cell, so a count is a census.
fn one_cell_fields() -> (TacticalView, (i32, i32)) {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let actor = acting_body(&view).expect("someone is acting");
    assert_eq!(actor.footprint, 1, "the fixture's actor is a squad");
    let cell = actor.cell;
    view.reachable = vec![cell];
    view.covered = vec![cell];
    view.provoking = vec![cell];
    (view, cell)
}

/// Bug 13: the cells a routine would hit were washed in `THREAT`, the
/// danger field's own red, so the two read as one wherever they met.
#[test]
fn the_area_preview_is_washed_in_white_and_not_in_danger_red() {
    let (view, cell) = one_cell_fields();
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(
            &view,
            &Overlay {
                preview: vec![cell],
                ..Default::default()
            },
            &mut fx,
            p,
            pane(),
            32.0,
            24,
        )
    });
    let at = |c: Color| Color::new(c.r, c.g, c.b, 0.22);
    assert_eq!(
        painted_rect_fill_count(&shapes, at(palette::EMPHASIS)),
        1,
        "the preview cell was not washed in the cursor's white"
    );
    assert_eq!(
        painted_rect_fill_count(&shapes, at(palette::THREAT)),
        0,
        "the preview is still drawn in the danger field's red"
    );
}

/// The danger field is `THREAT` at the reach wash's alpha, one wash a
/// cell — nothing pinned it before.
#[test]
fn the_danger_field_is_washed_in_threat() {
    let (view, _) = one_cell_fields();
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let c = palette::THREAT;
    assert_eq!(
        painted_rect_fill_count(&shapes, Color::new(c.r, c.g, c.b, REACH_WASH_ALPHA)),
        1
    );
}

/// The legend is what the map drew, not a second guess at it: the map
/// returns the washes it laid down, in draw order, and nothing else.
#[test]
fn the_map_reports_exactly_the_washes_it_drew() {
    let (mut view, cell) = one_cell_fields();
    let draw = |view: &TacticalView, preview: &[(i32, i32)], placeable: &[(i32, i32)]| {
        let mut fx = Fx::new();
        with_painter(|p| {
            draw_tactical_map(
                view,
                &Overlay {
                    preview: preview.to_vec(),
                    placeable: placeable.to_vec(),
                    ..Default::default()
                },
                &mut fx,
                p,
                pane(),
                32.0,
                24,
            )
        })
        .0
    };
    assert_eq!(
        draw(&view, &[cell], &[]),
        vec![Wash::Danger, Wash::Hits],
        "a cell in all three movement fields wears danger alone"
    );
    assert_eq!(
        draw(&view, &[cell], &[cell]),
        vec![Wash::Aim, Wash::Hits],
        "the aim field replaces the three movement washes"
    );
    view.provoking.clear();
    assert_eq!(draw(&view, &[], &[]), vec![Wash::Cover]);
    view.covered.clear();
    assert_eq!(draw(&view, &[], &[]), vec![Wash::Move]);
    view.reachable.clear();
    assert!(draw(&view, &[], &[]).is_empty());
}

/// Bug 13: move, cover and danger stacked on one cell read as a colour
/// nobody authored. Each reachable cell wears exactly one of them.
#[test]
fn a_reachable_cell_wears_one_movement_wash() {
    let (open, sheltered, exposed, both) = ((0, 0), (1, 0), (2, 0), (3, 0));
    let reachable = [open, sheltered, exposed, both];
    let covered = [sheltered, both];
    let provoking = [exposed, both];
    assert_eq!(
        board_washes(
            &reachable,
            &covered,
            &provoking,
            &Overlay::default(),
            &[],
            &[]
        ),
        vec![
            (Wash::Move, vec![open]),
            (Wash::Cover, vec![sheltered]),
            (Wash::Danger, vec![exposed, both]),
        ]
    );
}

/// A locked charge washes its cells in the charger's side's own tint,
/// and the legend reads the same list, so both words exist exactly when
/// the cells do.
#[test]
fn locked_charge_cells_get_their_own_washes() {
    let washes = board_washes(
        &[],
        &[],
        &[],
        &Overlay::default(),
        &[(1, 1)],
        &[(2, 2), (2, 3)],
    );
    assert_eq!(
        washes,
        vec![
            (Wash::ChargeParty, vec![(1, 1)]),
            (Wash::ChargeHostile, vec![(2, 2), (2, 3)]),
        ]
    );
    assert_ne!(Wash::ChargeParty.color(), Wash::Danger.color());
    assert_ne!(Wash::ChargeHostile.color(), Wash::Danger.color());
}

/// A charging body wears `CHG k/N`, and a body that is not charging does
/// not.
#[test]
fn a_charging_body_wears_its_progress_tag() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let mut body = view.bodies[0].clone();
    body.charge = Some((2, 3));
    body.statuses = Vec::new();
    view.bodies = vec![body];
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let drawn = crate::paint::painted_map_glyphs(&shapes);
    assert!(
        drawn.iter().any(|(t, _)| t == "CHG 2/3"),
        "no CHG tag in {drawn:?}"
    );
    view.bodies[0].charge = None;
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert!(
        !crate::paint::painted_map_glyphs(&shapes)
            .iter()
            .any(|(t, _)| t.starts_with("CHG"))
    );
}

/// Whether a run of `text` was painted in the aim yellow — the range
/// number's own ink, as against its dark outline.
fn painted_in_aim(drawn: &[(String, Color)], text: &str) -> bool {
    drawn.iter().any(|(t, c)| {
        t == text
            && (c.r - palette::AIM.r).abs() < 0.01
            && (c.g - palette::AIM.g).abs() < 0.01
            && (c.b - palette::AIM.b).abs() < 0.01
    })
}

/// The acting body floats its swing range; a body that is neither
/// acting nor aimed at does not.
#[test]
fn the_acting_body_floats_its_swing_range() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let acting = acting_body(&view).expect("someone acts").entity;
    for b in &mut view.bodies {
        b.swing_range = if b.entity == acting { 7 } else { 8 };
    }
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert!(
        painted_in_aim(&crate::paint::painted_map_glyphs(&shapes), "7"),
        "the acting body's range is missing"
    );
    assert!(
        !painted_in_aim(&crate::paint::painted_map_glyphs(&shapes), "8"),
        "a bystander floated its range"
    );
}

/// The body under the aim cursor floats its range too, so the player
/// can read a hostile's reach before stepping into it.
#[test]
fn the_aimed_body_floats_its_swing_range() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    let acting = acting_body(&view).expect("someone acts").entity;
    let aimed = view
        .bodies
        .iter()
        .find(|b| b.entity != acting)
        .expect("a second body")
        .cell;
    for b in &mut view.bodies {
        b.swing_range = if b.entity == acting { 7 } else { 8 };
    }
    let overlay = Overlay {
        cursor: Some(aimed),
        ..Overlay::default()
    };
    let mut fx = Fx::new();
    let (_, shapes) =
        with_painter(|p| draw_tactical_map(&view, &overlay, &mut fx, p, pane(), 32.0, 24));
    assert!(
        painted_in_aim(&crate::paint::painted_map_glyphs(&shapes), "7"),
        "the acting body's range is missing"
    );
    assert!(
        painted_in_aim(&crate::paint::painted_map_glyphs(&shapes), "8"),
        "the aimed body's range is missing"
    );
}

/// A finished board has nobody acting, so nothing floats a range.
#[test]
fn a_frozen_board_floats_no_range() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open").frozen();
    for b in &mut view.bodies {
        b.swing_range = 7;
    }
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    assert!(!painted_in_aim(
        &crate::paint::painted_map_glyphs(&shapes),
        "7"
    ));
}

/// A charger's bar offers the decision and nothing the engine refuses.
#[test]
fn a_chargers_action_bar_is_hold_and_release() {
    let mut game = fighting();
    let mut view = game.tactical_view().expect("the fight is open");
    view.player_turn = true;
    view.acted = false;
    let acting = acting_body(&view).expect("someone acts").entity;
    for b in view.bodies.iter_mut().filter(|b| b.entity == acting) {
        b.charge = Some((1, 3));
    }
    let keys: Vec<String> = action_bar(Mode::TacticalBattle, &view, false)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(&keys[..2], ["H", "X"]);
    assert!(!keys.iter().any(|k| k == "a" || k == "s"));
}

/// The move edge is the reach's boundary, not the plain cells': a
/// covered cell at the end of a row of three leaves the blue outline
/// round all three (8 sides), not round the two blue-filled ones (6).
#[test]
fn the_move_edge_runs_round_cells_a_louder_wash_fills() {
    use crate::paint::painted_line_count_in;

    let (mut view, _) = one_cell_fields();
    let cells: Vec<(i32, i32)> = view.board.cells().map(|(c, _)| c).collect();
    let row = cells
        .iter()
        .map(|&(x, y)| [(x, y), (x + 1, y), (x + 2, y)])
        .find(|r| r.iter().all(|c| cells.contains(c)))
        .expect("the board has a row of three");
    view.reachable = row.to_vec();
    view.covered = vec![row[2]];
    view.provoking = Vec::new();
    let wide = Rect::new(0.0, 0.0, 1400.0, 1000.0);
    let mut fx = Fx::new();
    let (_, shapes) =
        with_painter(|p| draw_tactical_map(&view, &Overlay::default(), &mut fx, p, wide, 32.0, 24));
    assert_eq!(painted_line_count_in(&shapes, palette::PLAN), 8);
}

/// The draw follows the list: the one cell in all three fields is
/// washed once, in danger red, with no blue or green under it.
#[test]
fn a_cell_in_every_movement_field_is_washed_once() {
    let (view, _) = one_cell_fields();
    let mut fx = Fx::new();
    let (_, shapes) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let wash = |c: Color| Color::new(c.r, c.g, c.b, REACH_WASH_ALPHA);
    assert_eq!(painted_rect_fill_count(&shapes, wash(palette::PLAN)), 0);
    assert_eq!(painted_rect_fill_count(&shapes, wash(palette::HEALTHY)), 0);
    assert_eq!(painted_rect_fill_count(&shapes, wash(palette::THREAT)), 1);
}

const ALL_WASHES: [Wash; 7] = [
    Wash::Move,
    Wash::Cover,
    Wash::Danger,
    Wash::Aim,
    Wash::Hits,
    Wash::ChargeParty,
    Wash::ChargeHostile,
];

#[test]
fn the_legend_draws_one_swatch_and_word_per_wash() {
    let m = ui_metrics(720.0);
    let (_, shapes) = with_painter(|p| draw_wash_legend(&ALL_WASHES, pane(), p, &m));
    let words = painted_text(&shapes);
    for wash in ALL_WASHES {
        assert!(
            words.contains(&wash.label().to_string()),
            "{wash:?} has no word in {words:?}"
        );
        assert_eq!(
            painted_rect_fill_count(&shapes, wash.color()),
            1,
            "{wash:?} has no swatch"
        );
    }
    let (_, none) = with_painter(|p| draw_wash_legend(&[], pane(), p, &m));
    assert!(none.is_empty(), "an empty board drew a legend");
}

/// A pane too narrow for one row wraps the key onto more rows rather
/// than dropping it, and every row stays inside the pane.
#[test]
fn a_narrow_pane_wraps_the_legend_instead_of_hiding_it() {
    use crate::paint::painted_fills;

    let m = ui_metrics(720.0);
    let narrow = Rect::new(0.0, 0.0, 160.0, 400.0);
    let (_, shapes) = with_painter(|p| draw_wash_legend(&ALL_WASHES, narrow, p, &m));
    assert_eq!(painted_text(&shapes).len(), ALL_WASHES.len());
    let fills = painted_fills(&shapes);
    let swatch = f32::from(m.small()) * 0.7;
    let rows: std::collections::BTreeSet<i32> = fills
        .iter()
        .filter(|(_, r)| (r.w - swatch).abs() < 0.01 && (r.h - swatch).abs() < 0.01)
        .map(|(_, r)| r.y.round() as i32)
        .collect();
    assert!(rows.len() > 1, "one row of five words in a 160px pane");
    for (_, r) in &fills {
        assert!(
            r.x >= narrow.x && r.y >= narrow.y && r.x + r.w <= narrow.x + narrow.w,
            "{r:?} leaves the pane"
        );
    }
}

/// Measured, not hoped: every entry draws, and nothing it paints leaves
/// the map pane, at the smallest supported window and a common one.
#[test]
fn the_whole_legend_fits_the_map_pane() {
    use crate::paint::painted_fills;
    use crate::render::hud::layout;

    for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        let m = ui_metrics(h);
        let (map, _) = with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            layout::regions(w, h, char_w, &m, false).map_pane
        });
        let (_, shapes) = with_painter(|p| draw_wash_legend(&ALL_WASHES, map, p, &m));
        assert_eq!(painted_text(&shapes).len(), ALL_WASHES.len(), "at {w}x{h}");
        for (_, r) in painted_fills(&shapes) {
            assert!(
                r.x >= map.x
                    && r.y >= map.y
                    && r.x + r.w <= map.x + map.w
                    && r.y + r.h <= map.y + map.h,
                "{r:?} leaves the map pane {map:?} at {w}x{h}"
            );
        }
    }
}

/// The board stashes the layout it drew with, exactly as the surface
/// map does, so a pixel on a body inverts to the cell that body was
/// drawn on — `tile_at_px` against `tile_origin_px` through the
/// published numbers rather than a recomputed camera.
#[test]
fn the_board_publishes_a_layout_that_inverts_a_bodys_pixel_to_its_cell() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let mut fx = Fx::new();
    with_painter(|p| draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24));
    let layout = fx.map_click().expect("the board published its layout");
    assert_eq!(layout.pane, pane());
    for body in &view.bodies {
        let (px, py) = tile_origin_px(
            body.cell,
            layout.center,
            layout.half,
            layout.off,
            layout.tile_px,
            layout.pane,
        );
        assert_eq!(
            super::super::super::base::tile_at_px(px + 1.0, py + 1.0, &layout),
            body.cell,
            "{:?}'s pixel did not invert to its cell",
            body.label
        );
    }
}

#[test]
fn a_hover_path_is_washed_and_named_in_the_legend() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let actor = acting_body(&view).expect("someone is acting");
    let path = vec![
        (actor.cell.0 + 1, actor.cell.1),
        (actor.cell.0 + 2, actor.cell.1),
    ];
    let overlay = Overlay {
        path: path.clone(),
        ..Default::default()
    };
    let mut fx = Fx::new();
    let (washes, shapes) =
        with_painter(|p| draw_tactical_map(&view, &overlay, &mut fx, p, pane(), 32.0, 24));
    assert!(
        washes.contains(&Wash::Path),
        "the legend does not name the path: {washes:?}"
    );
    let c = Wash::Path.color();
    assert_eq!(
        painted_rect_fill_count(&shapes, Color::new(c.r, c.g, c.b, PREVIEW_ALPHA)),
        path.len(),
        "the path's cells were not washed"
    );
}

#[test]
fn a_marked_target_is_outlined() {
    let mut game = fighting();
    let view = game.tactical_view().expect("the fight is open");
    let hostile = view
        .bodies
        .iter()
        .find(|b| b.is_hostile)
        .expect("a hostile is seated");
    let mut fx = Fx::new();
    let (_, plain) = with_painter(|p| {
        draw_tactical_map(&view, &Overlay::default(), &mut fx, p, pane(), 32.0, 24)
    });
    let overlay = Overlay {
        target: Some(hostile.cell),
        ..Default::default()
    };
    let mut fx = Fx::new();
    let (_, marked) =
        with_painter(|p| draw_tactical_map(&view, &overlay, &mut fx, p, pane(), 32.0, 24));
    let before = crate::paint::painted_rect_stroke_boxes(&plain, palette::EMPHASIS);
    let after = crate::paint::painted_rect_stroke_boxes(&marked, palette::EMPHASIS);
    assert_eq!(after.len(), before.len() + 1, "the mark drew no outline");
    let layout = fx.map_click().expect("the board published its layout");
    let (px, py) = tile_origin_px(
        hostile.cell,
        layout.center,
        layout.half,
        layout.off,
        layout.tile_px,
        layout.pane,
    );
    assert!(
        after
            .iter()
            .any(|r| (r.min.x - px).abs() < 1.0 && (r.min.y - py).abs() < 1.0),
        "no outline at the target's origin ({px}, {py}): {after:?}"
    );
}
