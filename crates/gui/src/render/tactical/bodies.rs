use super::*;

/// The bodies that float their swing range: the acting one, and the one
/// under the aim cursor when that is someone else.
pub(super) fn ranged_bodies(
    view: &TacticalView,
    cursor: Option<(i32, i32)>,
) -> impl Iterator<Item = &TacticalBody> {
    let acting = acting_body(view);
    let aimed = cursor
        .and_then(|cell| body_at(view, cell))
        .filter(|b| acting.is_none_or(|a| a.entity != b.entity));
    acting.into_iter().chain(aimed)
}

/// The body standing on `cell`, by any cell of its footprint —
/// `Game::tactical_occupant`'s own rule.
pub(super) fn body_at(view: &TacticalView, cell: (i32, i32)) -> Option<&TacticalBody> {
    view.bodies.iter().find(|b| {
        let f = b.footprint.max(1) as i32;
        (b.cell.0..b.cell.0 + f).contains(&cell.0) && (b.cell.1..b.cell.1 + f).contains(&cell.1)
    })
}

/// A body's swing range, floated off the top-right corner of its footprint.
///
/// **Above the tile rather than in its corner**, the turn arrow's own
/// reason: the cover sunrise owns the top-right corner inside the tile. The
/// aim yellow because a range is a targeting fact, outlined in the bar
/// trough's dark so it reads over any ground or sprite.
pub(super) fn draw_range_tag(
    painter: &Painter,
    range: u32,
    px: f32,
    py: f32,
    cell_px: f32,
    glyph_px: u16,
) {
    let text = range.to_string();
    let size = (glyph_px * 2 / 3).max(1);
    let dims = painter.measure_map(&text, size);
    let x = px + cell_px - dims.width / 2.0;
    let y = py - TURN_ARROW_GAP;
    for (dx, dy) in [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ] {
        painter.map(&text, x + dx, y + dy, size, palette::BAR_TROUGH);
    }
    painter.map(&text, x, y, size, palette::AIM);
}

/// The body whose turn it is — the one derivation of that, since the camera,
/// the arrow over its head and every caller asking where it stands must all
/// name the same body.
///
/// `TacticalView::active` indexes `order`, which is the initiative roll and
/// not the board, so this is a lookup and not a subscript.
pub(super) fn acting_body(view: &TacticalView) -> Option<&TacticalBody> {
    let acting = view.order.get(view.active?)?.entity;
    view.bodies.iter().find(|b| b.entity == acting)
}

/// One body: its art or its glyph, its con read, and what is left of it.
///
/// **Draws over the whole footprint, not one cell** — `cell_px` below is
/// `tile_px` scaled by `body.footprint`, `1` without a `Squad`, so an
/// ordinary body's draw is unchanged pixel for pixel. Every channel this
/// function spends — the glyph or sprite, the rarity bar, the con earmark,
/// the cover mark, the HP bar and now the squad's own mark — scales with
/// it, because a 2x2 body drawn at one cell's size would read as a body
/// standing in the footprint's corner rather than filling it.
pub(super) fn draw_body(
    body: &TacticalBody,
    painter: &Painter,
    px: f32,
    py: f32,
    tile_px: f32,
    glyph_px: u16,
) {
    let footprint = body.footprint.max(1) as f32;
    let cell_px = tile_px * footprint;
    let body_glyph_px = (glyph_px as f32 * footprint).round() as u16;
    // A raised body has already given up what it was — `palette::RESPAWNED`
    // stands in for its authored hue on both the glyph and the sprite tint,
    // since `ink` below is built from this on every path that isn't the
    // player's own role colour or an emulated form.
    let authored = if body.respawned {
        palette::RESPAWNED
    } else {
        super::super::glyph_color(body.color)
    };
    // The player's `@` is a role, read off `is_player` and never off the
    // hue they happen to have spawned with.
    //
    // **`form` outranks it, todo #100 Task 6**: while emulating, the image
    // spends the same boss magenta the surface map's own tile does — see
    // `views::FormLook`'s doc for why this isn't `is_boss: true` instead —
    // checked first so an emulating player's own role colour doesn't win
    // the cell back.
    let mut ink = if body.form.is_some() {
        super::super::marks::boss_color()
    } else if body.is_player {
        palette::PLAYER
    } else {
        authored
    };
    if body.cloaked {
        ink.a *= FADED_ALPHA;
    }
    let inset = sprite_inset(cell_px, body_glyph_px);
    // **The sprite call's own answer**, never `sprite.is_some()`: a name the
    // table has nothing under falls back to the glyph, and that glyph is
    // free to carry the con rung. `form`'s own sprite outranks `body.sprite`
    // for the same reason `ink` above does.
    //
    // The player's own rungs are the map's (`render/base.rs`): their drawing,
    // untinted for that file's reason with only the cloak's fade kept, then
    // the sprite their look names. The player has no species, so without
    // these `body.sprite` is `None` and the `@` was all this board drew.
    let look = body.look.as_ref().filter(|_| body.is_player);
    let drew_drawing = body.is_player
        && super::super::wears_drawing(look, body.form.is_some())
        && painter.sprite(
            crate::sprites::DRAWN_ICON_KEY,
            px + inset,
            py + inset,
            body_glyph_px as f32,
            Color::new(1.0, 1.0, 1.0, ink.a),
            body.entity.to_bits(),
        );
    let sprite = body.form.as_ref().map_or_else(
        || {
            look.and_then(|look| super::super::player_sprite_name(&look.sprite))
                .or(body.sprite.as_deref())
        },
        |f| f.sprite.as_deref(),
    );
    let drew_sprite = drew_drawing
        || sprite.is_some_and(|name| {
            painter.sprite(
                name,
                px + inset,
                py + inset,
                body_glyph_px as f32,
                ink,
                body.entity.to_bits(),
            )
        });
    let con = ConRead::of(
        body.difficulty,
        body.is_boss || body.form.is_some(),
        drew_sprite,
    );
    if !drew_sprite {
        let glyph = body
            .form
            .as_ref()
            .map_or(body.glyph, |f| f.glyph)
            .to_string();
        let dims = painter.measure_map(&glyph, body_glyph_px);
        let tx = px + (cell_px - dims.width) / 2.0;
        let ty = py + (cell_px + dims.height) / 2.0;
        painter.map(&glyph, tx, ty, body_glyph_px, con.glyph_ink(ink, 1.0));
    }
    // The player-colour outline the surface map's own tile keeps while
    // emulating — `render/base.rs`'s doc has the argument for why a corner
    // mark would misread as the nemesis mark on that board; this board has
    // no nemesis mark to be confused with, but the same outline keeps the
    // two grids agreeing about what an image looks like.
    if body.is_player && body.form.is_some() {
        let colour =
            super::super::player_look_color(body.look.as_ref().and_then(|look| look.colour));
        painter.rect_lines(px, py, cell_px - 1.0, cell_px - 1.0, 2.0, colour);
    }
    // The rare-spawn tier's own bar — see `marks::draw_rarity_bar`. Drawn
    // before the earmark below, which drops clear of it exactly as the
    // surface map's does.
    draw_rarity_bar(painter, body.rarity, px, py, cell_px, 1.0);
    if let Some(rung) = con.earmark() {
        let c = super::super::glyph_color(rung);
        let leg = cell_px * 0.28;
        let y = py + RARITY_BAR_PX;
        painter.poly(&[(px, y), (px + leg, y), (px, y + leg)], c);
    }
    // **The top-right corner, which this board alone leaves free** — the
    // surface map spends it on `nemesis_mark_rect`, which is never called
    // here. It drops below the rarity bar as the top-left con earmark does,
    // but is a sunrise — a sun on a horizon — rather than that one's
    // triangle mirrored, so the two corners never read as one mark twice.
    if body.in_cover {
        let Sunrise {
            horizon: [from, to],
            sun,
        } = sunrise(px, py, cell_px);
        painter.line(from.0, from.1, to.0, to.1, REACH_EDGE_PX, palette::HEALTHY);
        painter.poly(&sun, palette::HEALTHY);
    }
    if let Some(fraction) = body.hp_fraction {
        // The squad's own bar, not a sum of five — `marks::tactical_hp_bar_rect`
        // is the one geometry `squad_mark_rect` reads its floor from, so the
        // two cannot drift apart as either changes.
        let bar = marks::tactical_hp_bar_rect(px, py, cell_px);
        painter.rect(bar.x, bar.y, bar.w, bar.h, palette::BAR_TROUGH);
        painter.rect(
            bar.x,
            bar.y,
            bar.w * fraction.clamp(0.0, 1.0),
            bar.h,
            if body.is_hostile {
                palette::THREAT
            } else {
                palette::HEALTHY
            },
        );
    }
    // A charge wind-up: a pulsing outline in the side's colour. The pulse
    // rides the painter's animation clock and holds steady with effects off.
    if body.charge.is_some() {
        let phase = painter
            .anim_now()
            .map_or(1.0, |t| 0.5 + 0.5 * (t as f32 * CHARGE_PULSE_RATE).sin());
        let mut c = if body.is_hostile {
            palette::THREAT
        } else {
            palette::PLAYER
        };
        c.a = CHARGE_PULSE_MIN_ALPHA + (1.0 - CHARGE_PULSE_MIN_ALPHA) * phase;
        painter.rect_lines(px, py, cell_px - 1.0, cell_px - 1.0, 2.0, c);
    }
    // Status tags ride the bottom edge, above the HP bar's row, left-aligned
    // and clipped to the footprint's width. A folded squad's mark owns the
    // bottom-right corner, so the tag stops short of it; the compact form
    // (no rounds left) is what fits.
    let mut tags = super::super::battle::status_tags_compact_text(&body.statuses);
    if let Some((k, n)) = body.charge {
        if !tags.is_empty() {
            tags.push(' ');
        }
        tags.push_str(&charge_tag_text(k, n));
    }
    if !tags.is_empty() {
        let text = tags;
        let size = (glyph_px / 2).max(1);
        let bar = marks::tactical_hp_bar_rect(px, py, cell_px);
        let right = if body.squad.is_some() {
            marks::squad_mark_rect(px, py, cell_px).x
        } else {
            px + bar.w
        };
        let dims = painter.measure_map(&text, size);
        painter.clipped(
            px,
            bar.y - dims.height - 1.0,
            right - px,
            dims.height + 2.0,
            |p| {
                p.map(&text, px + 1.0, bar.y - 1.0, size, palette::EMPHASIS);
            },
        );
    }
    // A folded squad's own mark, bottom-right — see `marks::squad_mark_rect`
    // for why that corner and not the cover mark's. A character and not a
    // filled shape, `tuning::Formation::mark`'s own vocabulary, so it reads
    // through `painter.map` the way the recovery mark and the Alt marker do
    // rather than through a solid poly.
    if let Some(squad) = &body.squad {
        let mark = marks::squad_mark_rect(px, py, cell_px);
        let glyph = squad.mark.to_string();
        let size = (glyph_px / 2).max(1);
        let dims = painter.measure_map(&glyph, size);
        painter.map(
            &glyph,
            mark.x + (mark.w - dims.width) / 2.0,
            mark.y + (mark.h + dims.height) / 2.0,
            size,
            crate::paint::WHITE,
        );
    }
}
