use super::*;

/// Everything app-core lays over the board this frame that is not the
/// fight itself: the aim cursor and the routine it previews, the walk the
/// pointer is hovering, and the hostile a click has marked. All of it is
/// app-core state read before the `game` borrow in `render/base/`; the
/// board draws it and decides nothing about it.
#[derive(Default)]
pub(crate) struct Overlay {
    /// Where the aim cursor is, in `Mode::TacticalAim`.
    pub(crate) cursor: Option<(i32, i32)>,
    /// The cells the aimed routine would hit.
    pub(crate) preview: Vec<(i32, i32)>,
    /// The cells a `Radius` routine may be aimed at.
    pub(crate) placeable: Vec<(i32, i32)>,
    /// The walk a click on the hovered cell would commit.
    pub(crate) path: Vec<(i32, i32)>,
    /// The marked hostile's cell, read off the live board.
    pub(crate) target: Option<(i32, i32)>,
}

/// Draws the whole battle map.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tactical_map(
    view: &TacticalView,
    overlay: &Overlay,
    fx: &mut Fx,
    painter: &Painter,
    pane: Rect,
    tile_px: f32,
    glyph_px: u16,
) -> Vec<Wash> {
    let cursor = overlay.cursor;
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
    let target = fx
        .battle_center(acting_body(view).map(|body| (body.entity, body.cell, body.footprint)))
        .unwrap_or_else(|| {
            let middle = (view.board.side / 2) as f32;
            (middle, middle)
        });
    // The whole cell the tiles are laid out around; the target's fraction
    // — a body part-way through a glide — rides in the offset instead.
    let center = (target.0.round() as i32, target.1.round() as i32);
    // **No lag clamp, unlike the surface map.** `CAMERA_MAX_LAG` buys that
    // map a trailing edge its one extra ring of tiles can cover; this loop
    // walks the whole board and skips what falls outside the pane, so there
    // is no blank edge to expose — and at one tile the move between two
    // bodies standing apart is a teleport rather than a pan.
    let (off_x, off_y) = fx.camera_offset_toward(target, center, painter.delta(), None);
    // Stashed for the pointer system exactly as `draw_surface_map` stashes
    // its own: these are `tile_origin_px`'s arguments for this frame, with
    // the offset already eased, so a click inverts the frame it saw.
    fx.set_map_click(Some(super::super::base::MapClickLayout {
        pane,
        center,
        half: (half_w, half_h),
        off: (off_x, off_y),
        tile_px,
    }));
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

    // `reachable`/`covered`/`provoking` are all candidate anchors for the
    // body whose turn it is (`TacticalView`'s own doc on each), so the one
    // footprint to expand them by is the actor's — `expand_to_footprint`'s
    // own doc. `1` without a `Squad`, so an ordinary body's wash is these
    // same three vectors unchanged.
    let acting_footprint = acting_body(view).map(|b| b.footprint).unwrap_or(1);
    let reachable = expand_to_footprint(&view.reachable, acting_footprint);
    let covered = expand_to_footprint(&view.covered, acting_footprint);
    let provoking = expand_to_footprint(&view.provoking, acting_footprint);
    let charge_cells = |party_side: bool| -> Vec<(i32, i32)> {
        view.charge_aims
            .iter()
            .filter(|a| a.party_side == party_side)
            .flat_map(|a| a.cells.iter().copied())
            .collect()
    };
    let washes = board_washes(
        &reachable,
        &covered,
        &provoking,
        overlay,
        &charge_cells(true),
        &charge_cells(false),
    );

    let props: std::collections::HashMap<(i32, i32), &PropView> =
        view.props.iter().map(|p| (p.cell, p)).collect();
    let lighting = Lighting::of(&view.board);
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
        let lit = lighting.at(cell);
        draw_ground(painter, view.ground, kind, lit, px, py, tile_px - 1.0);
        if let Some(prop) = props.get(&cell) {
            draw_prop(
                painter,
                view.ground,
                lit,
                prop,
                (px, py),
                tile_px - 1.0,
                glyph_px,
            );
        }

        // What the legend reports is this list, so a key entry exists
        // exactly when its wash is drawn — `board_washes`' own doc.
        for &(wash, ref field) in &washes {
            if matches!(wash, Wash::Hits | Wash::Path) {
                // No edge: a preview is a handful of cells under a cursor
                // that already outlines its target, and a second boundary
                // around it would compete with the cursor's own. The path
                // likewise: its boundary is the reach's, already traced.
                if field.contains(&cell) {
                    let c = wash.color();
                    painter.rect(
                        px,
                        py,
                        tile_px - 1.0,
                        tile_px - 1.0,
                        Color::new(c.r, c.g, c.b, PREVIEW_ALPHA),
                    );
                }
            } else {
                // The move wash fills only the cells nothing louder claims,
                // but its edge traces everything reachable: traced round the
                // plain cells alone, a cover or danger pocket inside the
                // reach would be ringed in blue as if it were a hole in it.
                let edge: &[(i32, i32)] = if wash == Wash::Move {
                    &reachable
                } else {
                    field
                };
                draw_cell_field(painter, field, edge, cell, (px, py), tile_px, wash.color());
            }
        }
        // A body's own hit, last of the tile's washes — `render/base/`'s
        // own ordering for a raid flash, and the same call: a tactical hit
        // reuses `EffectKind::Hit`'s wash and spark burst rather than
        // inventing a second red.
        if let Some(flash) = fx.tactical_tile_flash(cell) {
            painter.rect(px, py, tile_px - 1.0, tile_px - 1.0, flash);
        }
    }

    // Where the dead fell, first of everything standing on the board: a
    // mark is nothing a body cannot stand on, and the body is what the
    // cell is about when one does. Scaled to the footprint the way
    // `draw_body` is, so a squad's mark covers the squad.
    for fallen in &view.fallen {
        let (px, py) = tile_origin_px(
            fallen.cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if !on_pane(px, py) {
            continue;
        }
        let footprint = fallen.footprint.max(1) as f32;
        let cell_px = tile_px * footprint;
        let mark_px = (glyph_px as f32 * footprint).round() as u16;
        let mark = FALLEN_MARK.0.to_string();
        let dims = painter.measure_map(&mark, mark_px);
        let tx = px + (cell_px - dims.width) / 2.0;
        let ty = py + (cell_px + dims.height) / 2.0;
        painter.map(&mark, tx, ty, mark_px, FALLEN_MARK.1);
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
            super::super::glyph_color(decoy.color)
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

    // Drawn where each body's glide has it this frame rather than on its
    // cell — `Fx::slid_cell` — so a walk moves rather than hops. Read once
    // here for the turn arrow below, which has to ride the same glide.
    let slid_px = |fx: &mut Fx, body: &TacticalBody| {
        let (sx, sy) = fx.slid_cell(body.entity, body.cell);
        let (px, py) = tile_origin_px(
            body.cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        (
            px + (sx - body.cell.0 as f32) * tile_px,
            py + (sy - body.cell.1 as f32) * tile_px,
        )
    };
    for body in &view.bodies {
        let (px, py) = slid_px(fx, body);
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
        let (px, py) = slid_px(fx, body);
        if on_pane(px, py) {
            let cell_px = tile_px * body.footprint.max(1) as f32;
            painter.poly(
                &turn_arrow(px, py, cell_px, fx.staffed_bob(body.entity)),
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

    // The struck body's own debris — `render/base/`'s spark burst, reused
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

    // A red `-N` rising off anyone just hit, a green `+N` off anyone just
    // healed, a `!` over anyone who just took an opportunity swing.
    fx.draw_cell_marks(painter, tile_px, glyph_px, |cell| {
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
    //
    // **Spans a squad's whole footprint, not the one cell the cursor
    // landed on.** The cursor names a body by any of its footprint's
    // cells — `Game::tactical_occupant`'s own rule from Task 1 — so a
    // player aiming at any corner of a 2x2 squad must see the same box
    // the swing will actually resolve against, not a quarter of it.
    if let Some(cell) = cursor {
        let anchor = body_at(view, cell)
            .map(|b| (b.cell, b.footprint.max(1) as f32))
            .unwrap_or((cell, 1.0));
        let (px, py) = tile_origin_px(
            anchor.0,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if on_pane(px, py) {
            let cell_px = tile_px * anchor.1;
            painter.rect_lines(px, py, cell_px - 1.0, cell_px - 1.0, 2.0, palette::EMPHASIS);
        }
    }
    // The marked hostile, outlined at the cursor's own weight and in its
    // own white: a mark is where the next click's swing lands, which is
    // what that white already means on this board. Bounds-checked for the
    // cursor's reason.
    if let Some(cell) = overlay.target {
        let footprint = view
            .bodies
            .iter()
            .find(|b| b.cell == cell)
            .map_or(1.0, |b| b.footprint.max(1) as f32);
        let (px, py) = tile_origin_px(
            cell,
            center,
            (half_w, half_h),
            (off_x, off_y),
            tile_px,
            pane,
        );
        if on_pane(px, py) {
            let cell_px = tile_px * footprint;
            painter.rect_lines(px, py, cell_px - 1.0, cell_px - 1.0, 2.0, palette::EMPHASIS);
        }
    }
    // How far the acting body and the aimed one swing, so the player can
    // step into — or out of — reach. Last, so no bolt or mark covers it,
    // and bounds-checked for the arrow's reason.
    for body in ranged_bodies(view, cursor) {
        let (px, py) = slid_px(fx, body);
        if on_pane(px, py) {
            let cell_px = tile_px * body.footprint.max(1) as f32;
            draw_range_tag(painter, body.swing_range, px, py, cell_px, glyph_px);
        }
    }
    washes.into_iter().map(|(wash, _)| wash).collect()
}
