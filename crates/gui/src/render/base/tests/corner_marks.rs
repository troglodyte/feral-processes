use super::*;

/// `a_wild_creature`, narrowed to one the Alt marker would flag — seed
/// 7's opening population always carries at least one species whose
/// level-1 kit is undiscovered, which is the common case a fresh run
/// starts in.
fn an_unseen_wild_creature(game: Game) -> (Game, EntityView) {
    let mut game = game;
    let far = game
        .view_entities(64, 64)
        .into_iter()
        .find(|ev| ev.unseen_routine)
        .expect("seed 7's opening population must hold an unseen carrier");
    near_the_player(game, far)
}

#[test]
fn a_stockpile_cell_draws_its_wash() {
    let pane = Rect::new(0.0, 0.0, 200.0, 200.0);
    let (_, shapes) = with_painter(|p| {
        draw_excavation_plan(
            p,
            &[(1, 1)],
            &[],
            None,
            |w| (w.0 as f32 * CELL, w.1 as f32 * CELL),
            CELL,
            pane,
        )
    });
    let want = bevy_egui::egui::Color32::from_rgba_unmultiplied(
        (STOCKPILE_FILL.r * 255.0).round() as u8,
        (STOCKPILE_FILL.g * 255.0).round() as u8,
        (STOCKPILE_FILL.b * 255.0).round() as u8,
        (STOCKPILE_FILL.a * 255.0).round() as u8,
    );
    assert!(
        shapes.iter().any(|cs| matches!(
            &cs.shape,
            bevy_egui::egui::Shape::Rect(r) if r.fill == want
        )),
        "{shapes:?}"
    );
}

/// The plan pass draws the wash for every mark and a cut meter for the
/// ones that still have a wall — and the cull the meter rides has to go
/// on culling the wash with it.
#[test]
fn a_marked_wall_wears_its_cut_and_a_floor_it_mark_does_not() {
    let pane = Rect::new(0.0, 0.0, 200.0, 200.0);
    let rects = |marks: &[DigMark]| {
        let (_, shapes) = with_painter(|p| {
            draw_excavation_plan(
                p,
                &[],
                marks,
                None,
                |w| (w.0 as f32 * CELL, w.1 as f32 * CELL),
                CELL,
                pane,
            )
        });
        shapes
            .iter()
            .filter(|cs| matches!(cs.shape, bevy_egui::egui::Shape::Rect(_)))
            .count()
    };

    let floor_it = rects(&[DigMark {
        pos: (1, 1),
        cut: None,
    }]);
    let part_cut = rects(&[DigMark {
        pos: (1, 1),
        cut: Some(0.5),
    }]);
    assert!(floor_it > 0, "every mark draws its wash");
    assert_eq!(
        part_cut,
        floor_it + 2,
        "a wall part-way down adds a track and a fill and nothing else"
    );
    assert_eq!(
        rects(&[DigMark {
            pos: (99, 99),
            cut: Some(0.5)
        }]),
        0,
        "a mark off the pane is culled, meter and all"
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

/// The fourth corner, and the census that keeps it the fourth: a patrol
/// member that is also rare, also a nemesis and standing on ground a
/// staffed mark claims must show every one of them without any being
/// spent to make room for another.
#[test]
fn the_patrol_mark_shares_no_pixels_with_any_other_mark_on_the_tile() {
    let tile_px = 40.0_f32;
    let (px, py) = (0.0, 0.0);
    let patrol = patrol_mark_rect(px, py, tile_px);
    let nemesis = nemesis_mark_rect(px, py, tile_px);
    let staffed = staffed_mark_rect(px, py, tile_px, 0.0);
    let [_, (con_right, _), (_, con_bottom)] = difficulty_mark_points(px, py, tile_px);

    assert!(
        patrol.y > nemesis.y + nemesis.h,
        "the patrol mark reaches up into the nemesis mark's row"
    );
    assert!(
        patrol.x > staffed.x + staffed.w,
        "the patrol mark reaches left into the staffed mark's column"
    );
    assert!(
        patrol.x > con_right && patrol.y > con_bottom,
        "the patrol mark reaches into the con earmark's wedge"
    );
    assert!(
        patrol.y > py + RARITY_BAR_PX,
        "the patrol mark reaches up into the rarity bar"
    );
}

/// `nemesis_mark_rect`'s rule, from the other two edges: flush against
/// one it would read as painting back in a wall `outline_open` left off.
#[test]
fn the_patrol_mark_stays_inside_the_tile_and_off_every_edge() {
    let (px, py, tile_px) = (50.0_f32, 60.0_f32, 40.0_f32);
    let mark = patrol_mark_rect(px, py, tile_px);

    assert!(mark.x > px, "the mark touches the tile's left edge");
    assert!(mark.y > py, "the mark touches the tile's top edge");
    assert!(
        mark.x + mark.w < px + tile_px - 1.0,
        "the mark touches or crosses the tile's right edge"
    );
    assert!(
        mark.y + mark.h < py + tile_px - 1.0,
        "the mark touches or crosses the tile's bottom edge"
    );
    assert!(mark.w > 0.0 && mark.h > 0.0, "the mark must have real size");
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
                    game, &mut fx, p, pane, tile_px, glyph_px, &status, None, None, center, false,
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
/// border of its own. `map_pane`'s bottom border belongs to the compass
/// strip, whose band is bought unconditionally — a mount bought only
/// while it is in use re-lays the grid the moment `w` is pressed — and
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

/// The con earmark is a right angle folded into the top-left corner,
/// and the rarity bar owns the full width of the edge above it. The
/// wedge is painted *after* that bar, so an earmark that reached the
/// top of the tile would silently cover one end of a channel that means
/// something else — at every zoom, since the inset is absolute and the
/// leg is not.
#[test]
fn the_con_earmark_folds_into_the_top_left_and_clears_the_rarity_bar() {
    for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
        let (px, py) = (100.0_f32, 200.0_f32);
        let [(cx, cy), (ax, ay), (dx, dy)] = difficulty_mark_points(px, py, tile_px);
        let leg = (tile_px - 1.0) * CON_MARK;

        assert!(
            cy > py + RARITY_BAR_PX,
            "at tile_px={tile_px} the earmark reaches into the rarity bar"
        );
        assert!(
            cx > px,
            "at tile_px={tile_px} the earmark is flush against the left edge"
        );
        assert!(
            (ax - cx - leg).abs() < 0.01 && ay == cy,
            "at tile_px={tile_px} the top leg must run along the edge"
        );
        assert!(
            (dy - cy - leg).abs() < 0.01 && dx == cx,
            "at tile_px={tile_px} the left leg must run down the edge"
        );
        assert!(
            cy + leg < py + tile_px - 1.0,
            "at tile_px={tile_px} the earmark hangs off the bottom of the tile"
        );
    }
}

/// The top edge carries two things now — the con earmark on the left and
/// the nemesis mark on the right — and a nemesis is exactly the creature
/// whose con read the player most wants beside it. They must never meet,
/// at any zoom: the earmark's leg is a fraction of the tile and the
/// mark's inset is not, so the narrowest gap is at the deepest zoom.
#[test]
fn the_con_earmark_never_meets_the_nemesis_mark() {
    for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
        let (px, py) = (100.0_f32, 200.0_f32);
        let [(_, cy), (ax, _), _] = difficulty_mark_points(px, py, tile_px);
        let nemesis = nemesis_mark_rect(px, py, tile_px);

        assert!(
            ax < nemesis.x,
            "at tile_px={tile_px} the earmark reaches {ax} and the nemesis \
             mark starts at {}",
            nemesis.x
        );
        assert!(
            (cy - nemesis.y).abs() < 0.01,
            "at tile_px={tile_px} the two top-edge marks hang at different \
             depths, which reads as one of them being misplaced"
        );
    }
}

/// **The bob is what makes this worth a test**: `fx.staffed_bob` lifts
/// the mark away from the bottom edge, so a resting place that has
/// drifted off the tile is invisible while a machine is worked and shows
/// only at rest — and for a stranded mark, which never bobs at all.
#[test]
fn the_staffed_mark_stays_inside_the_tile_at_rest_and_mid_bob() {
    let tile_px = 40.0_f32;
    let (px, py) = (10.0_f32, 20.0_f32);

    for lift in [0.0_f32, 1.0, 3.0] {
        let mark = staffed_mark_rect(px, py, tile_px, lift);
        assert!(
            mark.y + mark.h < py + tile_px - 1.0,
            "at lift={lift} the staffed mark's bottom ({}) hangs off the \
             tile (ends at {})",
            mark.y + mark.h,
            py + tile_px - 1.0
        );
        assert!(mark.y > py, "at lift={lift} the mark left the tile");
    }
}

/// The triangle the con read was painted as this frame, or `None` if it
/// was not. A `poly` rather than a `rect`, which is the whole reason the
/// earmark can share the top edge with the rarity bar without either
/// being mistaken for the other.
fn con_mark(
    shapes: &[bevy_egui::egui::epaint::ClippedShape],
) -> Option<(Vec<bevy_egui::egui::Pos2>, bevy_egui::egui::Color32)> {
    shapes.iter().find_map(|cs| match &cs.shape {
        bevy_egui::egui::Shape::Path(p) if p.fill.a() > 0 && p.points.len() == 3 => {
            Some((p.points.clone(), p.fill))
        }
        _ => None,
    })
}

/// `None` is *no reading*, not a reading worth nothing — the engine
/// hands a con colour only for a hostile, so anything else must leave
/// the corner bare rather than paint an earmark the player would read as
/// "you can beat your own companion".
#[test]
fn nothing_draws_a_con_mark_without_a_con_read() {
    let (_, shapes) = with_painter(|p| {
        draw_difficulty_mark(p, None, 0.0, 0.0, CELL, 1.0);
    });

    assert!(
        con_mark(&shapes).is_none(),
        "a tile with no con read must paint no earmark"
    );
}

/// The earmark lands where `difficulty_mark_points` says and wears the
/// rung it was handed. Both in one test because either alone passes
/// against a wedge drawn in the wrong place *or* in the wrong colour,
/// and the whole point of the channel is that a player reads position
/// and hue together.
#[test]
fn a_con_read_draws_its_rung_in_the_top_left_corner() {
    let (_, shapes) = with_painter(|p| {
        draw_difficulty_mark(p, Some(GlyphColor::Red), 0.0, 0.0, CELL, 1.0);
    });

    let (points, fill) = con_mark(&shapes).expect("a con read must paint an earmark");
    let want = difficulty_mark_points(0.0, 0.0, CELL);

    for (got, (wx, wy)) in points.iter().zip(want) {
        assert!(
            (got.x - wx).abs() < 0.01 && (got.y - wy).abs() < 0.01,
            "the earmark was painted at {points:?}, not at {want:?}"
        );
    }

    let red = hud::palette::glyph(GlyphColor::Red);
    let want_fill = bevy_egui::egui::Color32::from_rgb(
        (red.r * 255.0) as u8,
        (red.g * 255.0) as u8,
        (red.b * 255.0) as u8,
    );
    assert_eq!(
        (fill.r(), fill.g(), fill.b()),
        (want_fill.r(), want_fill.g(), want_fill.b()),
        "the earmark must wear the con rung's own hue, drawn from the one \
         palette table `glyph_color` reads"
    );
}

/// The text the Alt marker was painted as this frame, or `None` if it
/// was not — `con_mark`'s own shape, but for a run of text rather than
/// a filled triangle.
fn unseen_marker_drawn(shapes: &[bevy_egui::egui::epaint::ClippedShape]) -> bool {
    painted_text(shapes).iter().any(|t| t == "?")
}

/// **The property `corner_marker` exists for.** With `reveal` down, or
/// with nothing to reveal, the earmark alone lands in the corner — the
/// map before this feature existed. With both up, the marker takes the
/// corner and the earmark it would otherwise have carried is gone, on
/// every kind of con read an earmark can come from: a rung already on
/// the glyph (no earmark to begin with), a real earmark, and no con
/// read at all.
#[test]
fn corner_marker_hands_the_corner_to_the_earmark_unless_reveal_flags_an_unseen_tile() {
    for con in [
        ConRead::None,
        ConRead::Glyph(GlyphColor::Red),
        ConRead::Earmark(GlyphColor::Red),
    ] {
        assert_eq!(
            corner_marker(con, false, true),
            (con.earmark(), false),
            "{con:?}: nothing to reveal, so the earmark alone stands"
        );
        assert_eq!(
            corner_marker(con, true, false),
            (con.earmark(), false),
            "{con:?}: unseen but not asked for, so the earmark alone stands"
        );
        assert_eq!(
            corner_marker(con, true, true),
            (None, true),
            "{con:?}: reveal held over an unseen tile must take the \
             corner and clear whatever the earmark would have drawn"
        );
    }
}

/// The paint-level twin of the test above: the earmark and the marker
/// are mutually exclusive on the canvas, not just in the tuple
/// `corner_marker` returns.
#[test]
fn the_marker_and_the_earmark_never_share_a_frame() {
    let (_, revealed) = with_painter(|p| {
        let (earmark, marker) = corner_marker(ConRead::Earmark(GlyphColor::Red), true, true);
        draw_difficulty_mark(p, earmark, 0.0, 0.0, CELL, 1.0);
        if marker {
            draw_unseen_marker(p, 0.0, 0.0, CELL_GLYPH_PX, 1.0);
        }
    });
    assert!(
        unseen_marker_drawn(&revealed) && con_mark(&revealed).is_none(),
        "reveal must draw the marker and suppress the earmark"
    );

    let (_, hidden) = with_painter(|p| {
        let (earmark, marker) = corner_marker(ConRead::Earmark(GlyphColor::Red), true, false);
        draw_difficulty_mark(p, earmark, 0.0, 0.0, CELL, 1.0);
        if marker {
            draw_unseen_marker(p, 0.0, 0.0, CELL_GLYPH_PX, 1.0);
        }
    });
    assert!(
        !unseen_marker_drawn(&hidden) && con_mark(&hidden).is_some(),
        "without reveal the earmark alone must stand"
    );
}

/// Regression: `Painter::map`'s `y` is a baseline (`paint.rs`'s own
/// module docs — top = y − ascent), and the marker used to pass
/// `py + RARITY_BAR_PX + IDENTITY_MARK_INSET` straight through as if it
/// were a top edge, floating most of the glyph's ink into the tile
/// *above*. This draws the marker at a non-zero tile origin — the
/// origin-zero fixtures above cannot distinguish "inside this tile"
/// from "inside the tile above", since both start at y = 0 — and checks
/// the drawn ink's own bounds land inside `[py, py + CELL]` and
/// `[px, px + CELL]`.
#[test]
fn the_unseen_marker_sits_inside_its_own_tile() {
    let (px, py) = (100.0, 140.0);
    let (_, shapes) = with_painter(|p| {
        draw_unseen_marker(p, px, py, CELL_GLYPH_PX, 1.0);
    });
    let (_, text, ink) = crate::paint::painted_text_boxes(&shapes)
        .into_iter()
        .find(|(_, t, _)| t == "?")
        .expect("the marker must draw the \"?\" glyph");
    assert_eq!(text, "?");
    assert!(
        ink.y >= py && ink.y + ink.h <= py + CELL,
        "the marker's ink (y={}, h={}) must stay inside its tile (py={py}, CELL={CELL}) \
         instead of floating into the tile above",
        ink.y,
        ink.h
    );
    assert!(
        ink.x >= px && ink.x + ink.w <= px + CELL,
        "the marker's ink (x={}, w={}) must stay inside its tile (px={px}, CELL={CELL})",
        ink.x,
        ink.w
    );
}

/// The full pipeline, not just the extracted gate: a real wild
/// creature the engine itself flags draws the marker with Alt held and
/// draws no marker without it.
#[test]
fn the_map_draws_the_alt_marker_only_while_reveal_is_held() {
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets())
        .expect("the shipped assets must load");
    let (mut game, creature) = an_unseen_wild_creature(game);

    let (_, revealed) =
        drawn_map_centered_on_revealed(&mut game, SpriteTable::default(), creature.pos, true);
    assert!(
        revealed.iter().any(|t| t == "?"),
        "an unseen carrier must draw the marker while reveal is held: {revealed:?}"
    );

    let (_, hidden) =
        drawn_map_centered_on_revealed(&mut game, SpriteTable::default(), creature.pos, false);
    assert!(
        !hidden.iter().any(|t| t == "?"),
        "the marker must not draw without reveal: {hidden:?}"
    );
}

/// The `chains` template: a base whose machines are already set.
fn chains_game() -> Game {
    let ron = std::fs::read_to_string(test_assets().join("../dev-saves/chains.ron")).unwrap();
    let data = feral_processes_engine::save::from_ron(&ron).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_processes_gui_alt_chains_{}.bin",
        std::process::id()
    ));
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    let game = Game::load(&path, &test_assets()).unwrap();
    let _ = std::fs::remove_file(&path);
    game
}

#[test]
fn the_map_names_what_a_machine_makes_only_while_reveal_is_held() {
    let mut game = chains_game();
    let (_, revealed) =
        drawn_map_centered_on_revealed(&mut game, SpriteTable::default(), (3, 0), true);
    assert!(
        revealed.iter().any(|t| t == "Bytecode Block"),
        "a set Factory must be labelled while reveal is held: {revealed:?}"
    );
    let (_, hidden) =
        drawn_map_centered_on_revealed(&mut game, SpriteTable::default(), (3, 0), false);
    assert!(!hidden.iter().any(|t| t == "Bytecode Block"), "{hidden:?}");
}

/// The four rungs `difficulty_color` can answer with.
const RUNGS: [GlyphColor; 4] = [
    GlyphColor::Green,
    GlyphColor::Yellow,
    GlyphColor::Orange,
    GlyphColor::Red,
];

/// **The property the type exists for.** A rung shown on the glyph
/// *and* in the corner reads as two different creatures; a rung shown
/// in neither reads as harmless. Both are silent faults, and two
/// conditions agreeing at two draw sites is how either arrives — so the
/// whole matrix is swept rather than the interesting corner of it.
#[test]
fn a_con_read_lands_in_exactly_one_place_and_no_reading_lands_in_none() {
    for is_boss in [false, true] {
        for drew_sprite in [false, true] {
            for rung in RUNGS {
                let con = ConRead::of(Some(rung), is_boss, drew_sprite);
                let on_glyph = con.glyph_ink(WHITE, 1.0) != WHITE;
                let in_corner = con.earmark().is_some();
                assert!(
                    on_glyph != in_corner,
                    "{con:?} (is_boss={is_boss}, drew_sprite={drew_sprite}) \
                     puts the {rung:?} rung in {} places",
                    on_glyph as u8 + in_corner as u8
                );
            }

            let none = ConRead::of(None, is_boss, drew_sprite);
            assert_eq!(none, ConRead::None);
            assert_eq!(none.earmark(), None, "a non-hostile drew an earmark");
            assert_eq!(
                none.glyph_ink(WHITE, 1.0),
                WHITE,
                "a non-hostile had its authored hue taken"
            );
        }
    }
}

/// The glyph is the home a tile takes whenever it can, and the ink it
/// takes is the rung's own — from the one palette table `glyph_color`
/// reads, so the map and every popup that draws a program agree.
#[test]
fn an_ordinary_hostile_says_its_con_read_with_its_own_ink() {
    for rung in RUNGS {
        let con = ConRead::of(Some(rung), false, false);
        assert_eq!(con, ConRead::Glyph(rung));

        let want = hud::palette::glyph(rung);
        let got = con.glyph_ink(WHITE, 1.0);
        assert!(
            (got.r - want.r).abs() < 0.001
                && (got.g - want.g).abs() < 0.001
                && (got.b - want.b).abs() < 0.001,
            "the {rung:?} rung painted {got:?}, not the palette's {want:?}"
        );
    }
}

/// **Gated on the sprite call's answer, never on the sprite's name.** A
/// name the table has nothing under falls back to the glyph — the whole
/// of what keeps `assets/sprites/` optional — and that glyph is free to
/// carry the rung. Reading `sprite.is_some()` instead compiles, ships,
/// and costs every tile whose art failed to load the better of the two
/// homes, which reads as the con read having moved for no reason.
#[test]
fn a_named_sprite_that_did_not_draw_leaves_the_read_on_the_glyph() {
    assert_eq!(
        ConRead::of(Some(GlyphColor::Red), false, false),
        ConRead::Glyph(GlyphColor::Red)
    );
    assert_eq!(
        ConRead::of(Some(GlyphColor::Red), false, true),
        ConRead::Earmark(GlyphColor::Red)
    );
}

/// The two tiles that cannot pay with their ink, each for its own
/// reason: a sprite is authored near-white and this colour is
/// *multiplied* through it, and a boss's ink is already its magenta.
/// Asserted separately because they are independent — a boss with art
/// is one tile, and either alone must be enough.
#[test]
fn a_sprite_and_a_boss_each_push_the_read_into_the_corner() {
    for (what, is_boss, drew_sprite) in [
        ("a hostile drawn as a sprite", false, true),
        ("a boss drawn as a glyph", true, false),
        ("a boss drawn as a sprite", true, true),
    ] {
        let con = ConRead::of(Some(GlyphColor::Orange), is_boss, drew_sprite);
        assert_eq!(
            con,
            ConRead::Earmark(GlyphColor::Orange),
            "{what} must pay for its con read with a corner"
        );
        assert_eq!(
            con.glyph_ink(WHITE, 1.0),
            WHITE,
            "{what} had the ink taken it was supposed to keep"
        );
    }
}

/// **The census.** A boss's magenta is a *hue* now, sharing the one
/// channel the con rungs use, so it has to prove it is not any of them
/// — a magenta glyph misread as a rung is the map answering "can I win
/// this fight" with a lie. It shares the tile with the marks and bars
/// too. Same 0.10 channel-distance margin the con ladder's own census
/// uses.
#[test]
fn a_boss_glyph_is_distinguishable_from_every_hue_a_tile_can_wear() {
    fn dist(a: Color, b: Color) -> f32 {
        (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs()
    }

    let boss = boss_color();
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
            "a boss's glyph is {d:.3} from {what} — a player reading one \
             tile cannot tell them apart"
        );
    }
}
