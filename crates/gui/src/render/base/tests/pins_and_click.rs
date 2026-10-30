use super::*;

/// **`draw_pin_marks`' own `hide_top_left` parameter**, unit-tested
/// directly the way `nothing_draws_a_con_mark_without_a_con_read` tests
/// `draw_difficulty_mark` — a body under study is by definition owned,
/// and `EntityView::unseen_routine`'s doc says that reads `false` for
/// anything the player owns, so `marker` and `view_pinned_at` can never
/// both be true on any tile real play reaches. What this pins is the
/// drawing primitive's own contract, which the corner-marker rule at the
/// `draw_surface_map` call site depends on holding.
#[test]
fn the_top_left_pin_mark_yields_to_the_alt_marker() {
    let (_, hidden) = with_painter(|p| {
        draw_pin_marks(p, 0.0, 0.0, CELL, true, 1.0);
    });
    let (_, shown) = with_painter(|p| {
        draw_pin_marks(p, 0.0, 0.0, CELL, false, 1.0);
    });
    assert_eq!(
        painted_rect_fill_count(&hidden, marks::PIN_MARK_COLOR),
        3,
        "three corners drawn, with the top-left suppressed"
    );
    assert_eq!(
        painted_rect_fill_count(&shown, marks::PIN_MARK_COLOR),
        4,
        "all four corners draw when nothing borrows the top-left"
    );
}

/// **The corner census — a proof, not a suppression.** A body under
/// study can wear none of the four corner marks a tile can carry: the
/// con earmark (`ConRead::of` answers `None` for anything that is not
/// hostile), the nemesis mark and the patrol mark (both wild-only), and
/// the staffed mark (`wears_job_mark` is about a posted program, and
/// `UnderStudy` is not `Staff`). All four are unreachable today for this
/// reason alone — this test fails loudly the day a con read starts
/// applying to an owned program, which is exactly when this ring needs
/// re-examining.
/// `game_with_a_research_station` plus a program settled in its pen, and
/// optionally a project running — the two things `Game::view_pinned_at`
/// reads, built through the save rather than through `select_research`,
/// whose zone, prereq and chain gates are not what either caller is
/// testing.
fn game_with_a_pinned_subject(
    seed: u32,
    anchor: (i32, i32),
    active_research: Option<&str>,
) -> (Game, Entity) {
    let mut game = game_with_a_research_station(seed, anchor);
    let pen = (anchor.0 + 1, anchor.1 + 1);
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    let path = crate::render::test_support::scratch_path("pinned_subject", seed);
    let _cleanup = crate::render::test_support::RemoveOnDrop(&path);
    game.save(&path).unwrap();
    let mut data = feral_processes_engine::save::load_from_file(&path).unwrap();
    data.active_research = active_research.map(|id| id.to_string());
    data.creatures
        .push(feral_processes_engine::save::CreatureSave {
            sortie_index: None,
            boss: false,
            species,
            position: pen,
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 2,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
            wielded: false,
            zone: 1,
            custom_name: Some("Census Subject".to_string()),
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            assembly_roll: 1.0,
            extraction_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            bought_stats: Default::default(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
            nest_position: None,
            patrol_position: None,
            study_station: None,
            outpost: None,
            siphon: None,
            jailed: None,
            pursuing: false,
            carrying: None,
            carrying_program: None,
            tidying: None,
            clearing: false,
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 0,
            disposition: None,
            disgruntled: None,
            disgruntled_stranded: false,
            memories: Vec::new(),
            conversations: Vec::new(),
            needs: Default::default(),
            attributes: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
            siege_cell: None,
            siege_order: None,
            besieger: false,
            stolen_from: None,
            off_duties: Vec::new(),
            staff_rank: None,
            drop_trooper: false,
            base: crate::render::test_support::tamed_base(),
            stat_points: 0,
            generation: 0,
            breed_ready_at: 0,
            hold_points: false,
        });
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    let mut game = Game::load(&path, &test_assets()).unwrap();

    let station = game
        .view_entities_at(anchor, 4, 4)
        .into_iter()
        .find(|e| e.is_structure && e.pos == anchor)
        .expect("the Station stands at its anchor")
        .entity;
    let staff = game.base_staff();
    assert_eq!(
        staff.len(),
        1,
        "the census subject is the base's only staff"
    );
    let program = staff[0];

    game.pin_subject(program, station)
        .expect("a Staff program standing on its own pen must be pinnable");
    assert_eq!(
        game.program_role(program),
        Some(feral_processes_engine::ProgramRole::UnderStudy),
        "pinning must actually land the fifth role, or this census proves nothing"
    );
    (game, program)
}

/// **The body rattles and the pen does not.** The whole of
/// `views::PinMark::Strained`'s render contract, asserted against two
/// frames of the same scene at two `Fx` clocks: the subject's ink lands
/// somewhere different, and every line and every rect the map painted —
/// the four pin marks among them — lands in exactly the same place.
///
/// Both halves matter. Offsetting `px`/`py` for the whole cell instead of
/// for the ink alone compiles, looks right against one screenshot, and
/// walks the pen off its own tile-edge ring into the neighbouring
/// cell — which reads as the pen being loose rather than the body being
/// worked.
#[test]
fn a_strained_subject_rattles_inside_a_still_pen() {
    let anchor = (3, 0);
    let pen = (anchor.0 + 1, anchor.1 + 1);
    let (mut game, program) = game_with_a_pinned_subject(560_104, anchor, Some("paging"));
    assert_eq!(
        game.view_pinned_at(pen, 1, 1)[1][1],
        PinMark::Strained,
        "the fixture is vacuous unless the project is actually working this body"
    );
    let glyph = game
        .view_entities_at(pen, 4, 4)
        .into_iter()
        .find(|e| e.entity == program)
        .expect("the subject is drawn at its pen")
        .glyph
        .to_string();

    let (tile_px, glyph_px) = crate::text::map_cell(1);
    let frame = |game: &mut Game, now: f64| {
        let mut fx = Fx::new();
        fx.begin_frame(now, FrameCues::default(), false);
        let (_, shapes) = with_painter(|p| {
            let status = game.player_status();
            draw_surface_map(
                game,
                &mut fx,
                p,
                Rect::new(0.0, 0.0, 800.0, 600.0),
                tile_px,
                glyph_px,
                &status,
                None,
                None,
                status.position,
                false,
            );
        });
        shapes
    };
    // Every line segment and every rect the map painted, by position
    // alone — the pin marks are among the rects, and the claim is the
    // stronger one that *nothing* on the map moves but the ink. Colours
    // are deliberately left out of the key: the cloud field drifts with
    // the clock, which changes what tiles are shaded and not where
    // anything sits.
    let geometry = |shapes: &[bevy_egui::egui::epaint::ClippedShape]| {
        let q = |v: f32| (v * 16.0).round() as i32;
        let mut points: Vec<[i32; 4]> = shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::LineSegment { points, .. } => Some([
                    q(points[0].x),
                    q(points[0].y),
                    q(points[1].x),
                    q(points[1].y),
                ]),
                bevy_egui::egui::Shape::Rect(r) => Some([
                    q(r.rect.min.x),
                    q(r.rect.min.y),
                    q(r.rect.max.x),
                    q(r.rect.max.y),
                ]),
                _ => None,
            })
            .collect();
        points.sort_unstable();
        points
    };
    // Where the subject's own ink landed.
    let ink = |shapes: &[bevy_egui::egui::epaint::ClippedShape]| {
        let mut found: Vec<(i32, i32)> = crate::paint::painted_text_boxes(shapes)
            .into_iter()
            .filter(|(_, text, _)| *text == glyph)
            .map(|(_, _, r)| ((r.x * 16.0).round() as i32, (r.y * 16.0).round() as i32))
            .collect();
        found.sort_unstable();
        found
    };

    // Two clocks far enough apart to be different `STRAIN_STEP_HZ` steps.
    let early = frame(&mut game, 0.0);
    let late = frame(&mut game, 1.37);

    assert!(
        !geometry(&early).is_empty(),
        "the map drew nothing at all, so this proves nothing"
    );
    assert_eq!(
        geometry(&early),
        geometry(&late),
        "the pen must not move with the body it is holding, and nor must \
         anything else on the map"
    );
    assert_eq!(ink(&early).len(), 1, "exactly one tile draws this glyph");
    assert_ne!(
        ink(&early),
        ink(&late),
        "a body a project is spending must not sit still"
    );
}

#[test]
fn a_program_under_study_wears_none_of_the_four_corner_marks() {
    let anchor = (3, 0);
    let pen = (anchor.0 + 1, anchor.1 + 1);
    let (mut game, program) = game_with_a_pinned_subject(560_102, anchor, None);

    let view = game
        .view_entities_at(pen, 4, 4)
        .into_iter()
        .find(|e| e.entity == program)
        .expect("the pinned program is still where it was standing");

    assert_eq!(
        view.difficulty, None,
        "a con read must not reach an owned program"
    );
    assert!(!view.nemesis, "a nemesis mark is wild-only");
    assert_eq!(view.patrol, None, "a patrol mark is wild-only");
    assert!(
        !view.wears_job_mark,
        "the staffed mark is about a posted program, and UnderStudy is not Staff"
    );
}

/// `tile_at_px` is `tile_origin_px`'s exact inverse — the round trip the
/// click-to-travel feature stands on. Two window sizes and a non-zero
/// `pane.y` (the status bar claiming the top row) and camera offset:
/// the drawing seam's "literal 0.0" trap is a term that only shows up
/// once one of these stops being zero, so a fixture that leaves any of
/// them at zero would pass with the term dropped.
#[test]
fn tile_at_px_inverts_tile_origin_px_at_the_corner_and_inside_the_cell() {
    let cases = [
        (
            Rect::new(0.0, 32.0, 800.0, 560.0),
            20.0_f32,
            (20, 14),
            (0.35, -0.6),
        ),
        (
            Rect::new(0.0, 48.0, 1600.0, 900.0),
            32.0_f32,
            (26, 15),
            (-0.2, 0.8),
        ),
    ];
    for (pane, tile_px, half, off) in cases {
        let center = (5, -3);
        let layout = MapClickLayout {
            pane,
            center,
            half,
            off,
            tile_px,
        };
        for dx in -6..=6 {
            for dy in -6..=6 {
                let t = (center.0 + dx, center.1 + dy);
                let (ox, oy) = tile_origin_px(t, center, half, off, tile_px, pane);
                assert_eq!(
                    tile_at_px(ox, oy, &layout),
                    t,
                    "the corner of {t:?}, drawn at ({ox}, {oy}), inverted to the wrong tile"
                );
                // Not just the corner: a point in the interior of the
                // cell has to answer the same tile, or a click anywhere
                // but the top-left pixel would miss.
                let (ix, iy) = (ox + tile_px * 0.5, oy + tile_px * 0.5);
                assert_eq!(
                    tile_at_px(ix, iy, &layout),
                    t,
                    "the middle of {t:?}, at ({ix}, {iy}), inverted to the wrong tile"
                );
            }
        }
    }
}
