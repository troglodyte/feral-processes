use super::*;

/// The bottom edge belongs to the progress bar, exactly as the top edge
/// belongs to the rarity bar — so the two bottom-corner marks lift above
/// it the way the two top-corner marks drop below that one. Without the
/// lift a worked machine's bar is drawn straight through its own
/// "somebody is on this job" mark.
#[test]
fn the_bottom_corner_marks_clear_the_progress_bar() {
    for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
        let (px, py) = (100.0_f32, 200.0_f32);
        let bar = progress_bar_rect(px, py, tile_px);
        let staffed = staffed_mark_rect(px, py, tile_px, 0.0);
        let patrol = patrol_mark_rect(px, py, tile_px);

        assert!(
            staffed.y + staffed.h <= bar.y,
            "at tile_px={tile_px} the staffed mark's bottom ({}) reaches \
             into the bar's row (starts at {})",
            staffed.y + staffed.h,
            bar.y
        );
        assert!(
            patrol.y + patrol.h <= bar.y,
            "at tile_px={tile_px} the patrol mark's bottom ({}) reaches \
             into the bar's row (starts at {})",
            patrol.y + patrol.h,
            bar.y
        );
    }
}

/// The bar sits *inside* the status outline. `outline_open` draws a
/// machine's bottom wall 2 px thick along `py + tile_px - 1`, and it is
/// drawn after the tile's fills — a bar flush to that edge is painted
/// over by the outline of the very machine it belongs to.
#[test]
fn the_progress_bar_sits_inside_the_status_outline() {
    for tile_px in [24.0_f32, 32.0, 48.0, 64.0] {
        let (px, py) = (0.0_f32, 0.0_f32);
        let bar = progress_bar_rect(px, py, tile_px);
        let wall = py + tile_px - 1.0;

        assert!(
            bar.y + bar.h < wall - 1.0,
            "at tile_px={tile_px} the bar's bottom ({}) is under the \
             outline's bottom wall",
            bar.y + bar.h
        );
        assert!(bar.x > px, "the bar touches the tile's left edge");
        assert!(
            bar.x + bar.w < px + tile_px - 1.0,
            "the bar touches the tile's right edge"
        );
        assert!(bar.w > 0.0 && bar.h > 0.0, "the bar must have real size");
    }
}

/// Every rectangle `draw_progress_bar` painted this frame, widest last.
fn bar_widths(progress: Option<f32>) -> Vec<f32> {
    let (_, shapes) = with_painter(|p| {
        draw_progress_bar(p, progress, 0.0, 0.0, CELL, hud::palette::HEALTHY, 1.0)
    });
    let mut widths: Vec<f32> = shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            bevy_egui::egui::Shape::Rect(r) => Some(r.rect.width()),
            _ => None,
        })
        .collect();
    widths.sort_by(|a, b| a.partial_cmp(b).expect("no NaN widths"));
    widths
}

/// Three states, and the empty one is the reason there is a track at
/// all: a bar with nothing in it still has to say *a job is running
/// here*, which a zero-width fill on bare tile cannot.
#[test]
fn a_bar_draws_a_track_it_never_overflows_and_nothing_at_all_with_no_job() {
    let track = progress_bar_rect(0.0, 0.0, CELL).w;

    assert_eq!(bar_widths(None), Vec::<f32>::new(), "no job, no bar");
    assert_eq!(
        bar_widths(Some(0.0)),
        vec![track],
        "a cycle just begun is a track with no fill"
    );
    assert_eq!(
        bar_widths(Some(1.0)),
        vec![track, track],
        "a full cycle fills its track exactly"
    );
    let half = bar_widths(Some(0.5));
    assert_eq!(half.len(), 2, "a part-done cycle is a track and a fill");
    assert!(
        half[0] > 0.0 && half[0] < track,
        "half a cycle is neither empty nor full: {half:?}"
    );
}

/// A figure out of range is clamped rather than drawn off the tile: the
/// engine clamps its own, and nothing in the compiler holds a second
/// caller to it.
#[test]
fn a_bar_is_clamped_to_its_track() {
    let track = progress_bar_rect(0.0, 0.0, CELL).w;
    assert_eq!(bar_widths(Some(4.0)), vec![track, track]);
    assert_eq!(bar_widths(Some(-1.0)), vec![track]);
}

/// A Depot's bar is the progress bar's own track stood upright on the
/// right edge, so the two read as one kind of mark, and it stops above
/// the job bar so they never share pixels on a Depot being upgraded.
#[test]
fn a_depots_bar_stands_on_the_right_edge_above_the_progress_bar() {
    for tile_px in [CELL, CELL * 2.0, CELL * 3.0] {
        let side = shelf_fill_rect(0.0, 0.0, tile_px);
        let bottom = progress_bar_rect(0.0, 0.0, tile_px);
        assert_eq!(side.w, bottom.h, "same thickness as the job bar");
        assert_eq!(side.y, bottom.x, "same inset from the top edge");
        assert_eq!(
            side.x + side.w,
            bottom.x + bottom.w,
            "same inset from the right edge"
        );
        assert!(side.y + side.h < bottom.y, "the two bars overlap");
        assert!(side.h > side.w, "the bar stands upright");
    }
}

/// Green with room, yellow with a tenth or less left, red when full —
/// the `[GRID]` readout's three roles, so the colours mean one thing
/// on the status bar and the map.
#[test]
fn a_depots_bar_turns_yellow_with_a_tenth_left_and_red_when_full() {
    let at = |held| shelf_fill_color(ShelfFill { held, capacity: 50 });
    assert_eq!(at(0), hud::palette::HEALTHY);
    assert_eq!(at(44), hud::palette::HEALTHY, "six left is over a tenth");
    assert_eq!(at(45), hud::palette::ATTENTION, "five left is a tenth");
    assert_eq!(at(49), hud::palette::ATTENTION);
    assert_eq!(at(50), hud::palette::OFFLINE);
    assert_eq!(
        shelf_fill_color(ShelfFill {
            held: 0,
            capacity: 0
        }),
        hud::palette::OFFLINE,
        "a box that holds nothing has no room"
    );
}

/// The fill is held against capacity, and an empty Depot still draws its
/// track so it reads as a Depot you can read the level of.
#[test]
fn a_depots_bar_fills_in_proportion_and_an_empty_one_keeps_its_track() {
    let heights = |held| {
        let (_, shapes) = with_painter(|p| {
            draw_shelf_fill(
                p,
                Some(ShelfFill { held, capacity: 50 }),
                0.0,
                0.0,
                CELL,
                1.0,
            )
        });
        let mut r: Vec<(f32, f32)> = shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Rect(r) => Some((r.rect.top(), r.rect.height())),
                _ => None,
            })
            .collect();
        r.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("no NaN heights"));
        r
    };
    let track = shelf_fill_rect(0.0, 0.0, CELL);
    let whole = (track.y, track.h);
    assert_eq!(heights(0), vec![whole]);
    assert_eq!(heights(50), vec![whole, whole]);
    let half = heights(25);
    assert!((half[0].1 - track.h / 2.0).abs() < 0.01, "{half:?}");
    assert!(
        (half[0].0 + half[0].1 - (track.y + track.h)).abs() < 0.01,
        "the fill rises from the bottom of its track: {half:?}"
    );
    let (_, none) = with_painter(|p| draw_shelf_fill(p, None, 0.0, 0.0, CELL, 1.0));
    assert!(none.is_empty(), "not a Depot, no bar");
}

/// The dot is one square in the top-left quadrant, below the rarity
/// bar's strip and left of the tile's midpoint; a machine holding
/// nothing paints nothing there.
#[test]
fn a_machine_holding_output_wears_one_dot_in_its_top_left_corner() {
    let rects = |holds| {
        let (_, shapes) = with_painter(|p| draw_held_output(p, holds, 0.0, 0.0, CELL, WHITE));
        shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Rect(r) => Some(r.rect),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let drawn = rects(true);
    assert_eq!(drawn.len(), 1, "exactly one dot: {drawn:?}");
    let dot = drawn[0];
    assert!(
        dot.top() >= RARITY_BAR_PX,
        "the dot overlaps the rarity bar: {dot:?}"
    );
    assert!(
        dot.left() > 0.0,
        "the dot is flush to the left edge: {dot:?}"
    );
    assert!(
        dot.right() < CELL / 2.0,
        "the dot leaves the top-left quadrant: {dot:?}"
    );
    assert!(
        dot.bottom() < CELL / 2.0,
        "the dot leaves the top-left quadrant: {dot:?}"
    );
    assert!(rects(false).is_empty(), "holding nothing, no dot");
}

/// The census for the dot: it shares no pixels with the staffed mark,
/// the Depot fill gauge, the progress bar, the rarity bar or the
/// nemesis mark at any real zoom rung, and it stays smaller than the
/// corner badges.
///
/// The staffed mark bobs a fixed four pixels (`fx`'s own constant,
/// private there). At the 20 px rung a 2 px bar, two 2 px insets, a
/// 3 px bar and the mark itself leave the dot less than the bob's
/// reach, so the peak of a bob brushes the dot's row by about a pixel
/// there — the same tightness `the_nemesis_mark_sits_in_the_opposite_
/// corner_from_the_staffed_mark` lives with by measuring at rest. From
/// 40 px up the two are clear at every lift.
#[test]
fn the_held_output_dot_shares_no_pixels_with_any_other_mark_on_the_tile() {
    let disjoint = |a: Rect, b: Rect| {
        a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y
    };
    let (px, py) = (100.0, 200.0);
    for tile_px in [20.0_f32, 40.0, 60.0, 80.0] {
        let dot = held_output_rect(px, py, tile_px);
        let nemesis = nemesis_mark_rect(px, py, tile_px);
        assert!(dot.w < nemesis.w, "at {tile_px}: a dot, not a fourth badge");
        assert!(
            dot.y >= py + RARITY_BAR_PX,
            "at {tile_px}: into the rarity bar"
        );
        let lifts: &[f32] = if tile_px > 20.0 {
            &[0.0, 2.0, 4.0]
        } else {
            &[0.0]
        };
        for &lift in lifts {
            let staffed = staffed_mark_rect(px, py, tile_px, lift);
            assert!(
                disjoint(dot, staffed),
                "at {tile_px}, lift {lift}: {dot:?} vs {staffed:?}"
            );
        }
        for (name, other) in [
            ("depot fill", shelf_fill_rect(px, py, tile_px)),
            ("progress bar", progress_bar_rect(px, py, tile_px)),
            ("nemesis", nemesis),
        ] {
            assert!(
                disjoint(dot, other),
                "at {tile_px}: dot {dot:?} meets {name} {other:?}"
            );
        }
    }
}

/// A structure as the map sees one, running a cycle or not.
fn machine_view(status: Option<MachineStatus>, job_progress: Option<f32>) -> EntityView {
    EntityView {
        glyph: 'n',
        is_tamed: false,
        is_structure: true,
        machine_status: status,
        job_progress,
        ..patient_view(false)
    }
}

/// Which occupant owns the bar, and the hue it wears. The precedence is
/// the half a hand-written copy would get wrong: a machine with an
/// upgrade on order carries that request's row *and* is still producing,
/// and it is the machine the cell is drawing.
#[test]
fn a_machine_owns_its_cells_bar_over_a_request_standing_on_it() {
    let running = machine_view(Some(MachineStatus::Running), Some(0.4));
    let starved = machine_view(Some(MachineStatus::Starved), Some(0.4));
    let idle = machine_view(Some(MachineStatus::Idle), None);
    let site = EntityView {
        is_structure: false,
        job_progress: Some(0.25),
        ..patient_view(false)
    };

    assert_eq!(
        cell_bar(Some(&running), None, None),
        Some((0.4, hud::palette::HEALTHY)),
        "a running machine's bar wears the hue its own outline does"
    );
    assert_eq!(
        cell_bar(Some(&starved), None, None),
        Some((0.4, hud::palette::WARN)),
        "a stalled cycle is frozen, not hidden, and says so in its colour"
    );
    assert_eq!(
        cell_bar(Some(&idle), None, None),
        None,
        "nobody is working here, so there is no bar"
    );
    assert_eq!(
        cell_bar(None, Some(&site), None),
        Some((0.25, ORANGE)),
        "a site the crew has not raised yet wears its caret's orange"
    );
    assert_eq!(
        cell_bar(Some(&running), Some(&site), None),
        Some((0.4, hud::palette::HEALTHY)),
        "a machine with an upgrade on order is still producing, and the \
         cell is drawing the machine"
    );
    assert_eq!(cell_bar(None, None, None), None);
}

/// A hurt member of staff wears its Integrity on the bottom edge, and
/// nobody else does: not a program at full health, not the party, not
/// a wild one.
#[test]
fn only_hurt_staff_wear_a_health_bar() {
    let hurt = EntityView {
        hp_fraction: Some(0.8),
        ..patient_view(false)
    };
    let whole = EntityView {
        hp_fraction: Some(1.0),
        ..patient_view(false)
    };
    let companion = EntityView {
        is_companion: true,
        ..hurt.clone()
    };
    let player = EntityView {
        is_player: true,
        ..hurt.clone()
    };
    let wild = EntityView {
        is_tamed: false,
        is_hostile: true,
        ..hurt.clone()
    };

    assert_eq!(
        cell_bar(None, None, Some(&hurt)),
        Some((0.8, hud::palette::HEALTHY))
    );
    assert_eq!(
        cell_bar(None, None, Some(&whole)),
        None,
        "full health is unmarked"
    );
    assert_eq!(cell_bar(None, None, Some(&companion)), None);
    assert_eq!(cell_bar(None, None, Some(&player)), None);
    assert_eq!(cell_bar(None, None, Some(&wild)), None);
}

/// A worker crossing a machine's floor is passing through; the cell's
/// bar stays the machine's, and the request's over a bare slab.
#[test]
fn a_job_bar_outranks_a_health_bar_on_a_shared_cell() {
    let running = machine_view(Some(MachineStatus::Running), Some(0.4));
    let site = EntityView {
        is_structure: false,
        job_progress: Some(0.25),
        ..patient_view(false)
    };
    let hurt = EntityView {
        hp_fraction: Some(0.3),
        ..patient_view(false)
    };
    assert_eq!(
        cell_bar(Some(&running), None, Some(&hurt)),
        Some((0.4, hud::palette::HEALTHY))
    );
    assert_eq!(
        cell_bar(None, Some(&site), Some(&hurt)),
        Some((0.25, ORANGE))
    );
    let idle = machine_view(Some(MachineStatus::Idle), None);
    assert_eq!(
        cell_bar(Some(&idle), None, Some(&hurt)),
        Some((0.3, hud::palette::WARN)),
        "an idle machine has no bar to give way to"
    );
}

/// Green, then the yellow that fixes itself, then the red of a body the
/// Bay has taken off its job — whose line is the engine's own.
#[test]
fn a_health_bar_steps_down_at_half_and_at_the_bays_admission_line() {
    let admission = feral_processes_engine::tuning::BAY_ADMISSION_HP_FRACTION;
    assert_eq!(health_color(1.0), hud::palette::HEALTHY);
    assert_eq!(health_color(0.51), hud::palette::HEALTHY);
    assert_eq!(health_color(0.5), hud::palette::WARN);
    assert_eq!(health_color(admission + 0.01), hud::palette::WARN);
    assert_eq!(health_color(admission), hud::palette::OFFLINE);
    assert_eq!(health_color(0.0), hud::palette::OFFLINE);
}
