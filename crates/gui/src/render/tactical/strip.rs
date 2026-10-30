use super::*;

/// The turn order, as a block inside the map pane.
///
/// **The compass block's slot, and its rules.** It starts at
/// `layout::strip_inset` below the pane's top edge because the THREAT strip's
/// quad hangs down into the pane, and it is a block rather than a strip on
/// the pane's bottom border because a strip that appears only during a fight
/// buys the map a band it can never draw tiles in and re-lays the whole grid
/// on the keypress that opens one — which at the keyboard reads as the
/// camera lurching.
pub(crate) fn draw_turn_strip(view: &TacticalView, pane: Rect, painter: &Painter, m: &Metrics) {
    if view.order.is_empty() {
        return;
    }
    let pad = m.line_height * 0.35;
    let size = m.small();
    let cell = painter.measure_ui_advance("M", size) * 2.0;
    let head = format!("ROUND {}", view.round);
    let head_w = painter.measure_ui_advance(&head, size);
    let w = pad * 2.0 + head_w.max(cell * view.order.len() as f32);
    let h = pad * 2.0 + m.line_height * 2.0;

    let x = pane.x + pane.w - m.inset - w;
    let y = pane.y + strip_inset(m);
    if x < pane.x + m.inset || y + h > pane.y + pane.h {
        return;
    }
    painter.rect(x, y, w, h, Color::new(0.04, 0.06, 0.08, 0.88));
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);
    painter.ui(
        &head,
        x + pad,
        y + pad + size as f32 * 0.8,
        size,
        palette::PANE_TITLE,
    );

    let baseline = y + pad + m.line_height + size as f32 * 0.8;
    for (i, row) in view.order.iter().enumerate() {
        let gx = x + pad + cell * i as f32;
        // The acting body wears the highlight, which is the whole of what a
        // turn order is for.
        if Some(i) == view.active {
            let c = palette::EMPHASIS;
            painter.rect(
                gx - 1.0,
                y + pad + m.line_height,
                cell,
                m.line_height,
                Color::new(c.r, c.g, c.b, 0.18),
            );
        }
        let glyph = row.glyph.to_string();
        painter.map(
            &glyph,
            gx,
            baseline,
            size,
            // `WARN` before `PLAYER`: a taken-over companion is still on the
            // party's side of the initiative order (`is_hostile` is false),
            // but its rung is the AI's for as long as the entry lasts, and
            // that is worth a colour of its own — `WARN` rather than
            // `THREAT`, since the party still wins this fight together.
            if row.is_hostile {
                super::super::glyph_color(row.color)
            } else if row.taken_over {
                palette::WARN
            } else {
                palette::PLAYER
            },
        );
    }

    draw_tamper_block(view, pane, x + w, y + h, w, painter, m);
}

/// A wrap's `ROUND N`, fading over the pane while the next body acts — the
/// strip's own number changes too, but in a corner nobody is looking at.
///
/// Centred across the pane and a quarter of the way down it, because the
/// camera holds the acting body at the middle and a banner there would
/// cover the very turn it announces.
pub(crate) fn draw_round_banner(fx: &Fx, pane: Rect, painter: &Painter, m: &Metrics) {
    let Some((round, alpha)) = fx.round_banner() else {
        return;
    };
    let text = format!("ROUND {round}");
    let size = m.title() * 2;
    let pad = m.line_height * 0.5;
    let text_w = painter.measure_ui_advance(&text, size);
    let w = text_w + pad * 2.0;
    let h = size as f32 + pad * 2.0;
    let x = pane.x + (pane.w - w) / 2.0;
    let y = pane.y + pane.h * 0.25 - h / 2.0;
    if w > pane.w || y < pane.y {
        return;
    }
    painter.rect(x, y, w, h, Color::new(0.04, 0.06, 0.08, 0.7 * alpha));
    let c = palette::PANE_TITLE;
    painter.ui(
        &text,
        x + pad,
        y + pad + size as f32 * 0.8,
        size,
        Color::new(c.r, c.g, c.b, alpha),
    );
}

/// A rung's own tag text, decision 7's exhaustive vocabulary — `cell_mark`'s
/// rule, so a sixth `TamperTag` fails to compile here rather than drawing a
/// blank word.
pub(super) fn tamper_tag_text(tag: TamperTag) -> &'static str {
    match tag {
        TamperTag::Hot => "HOT",
        TamperTag::Cold => "COLD",
        TamperTag::Profiled => "PROF",
        TamperTag::Injected => "INJ",
        TamperTag::Hallucinating => "HALL",
    }
}

/// One rung's own line in the tamper block, decision 7's format —
/// `<glyph> TAG TAG ▸ <routine name>` — or `None` for a rung with nothing
/// to say: no active `Tampered` slot, no live forecast, and not taken over.
///
/// A free function rather than inlined in `draw_tamper_block`, so the width
/// census below builds the exact line production draws instead of a second
/// copy of the format.
pub(super) fn tamper_line(row: &TurnRow) -> Option<String> {
    if row.tags.is_empty() && row.forecast.is_none() && !row.taken_over {
        return None;
    }
    let mut words = vec![row.glyph.to_string()];
    words.extend(row.tags.iter().map(|&tag| tamper_tag_text(tag).to_string()));
    if row.taken_over {
        words.push("HIJACK".to_string());
    }
    if let Some(forecast) = &row.forecast {
        words.push(format!("▸ {}", forecast.action));
    }
    Some(words.join(" "))
}

/// The tamper block, decision 7: one line per rung that carries a tag, a
/// forecast or a hijack, beneath the strip and in initiative order.
///
/// `strip_right`/`strip_bottom`/`strip_w` are the strip's own box, read
/// rather than a literal: the block re-anchors to the strip's right edge and
/// starts one gap below its bottom. `pane` is the map pane itself, for the
/// same off-screen guard `draw_turn_strip` applies to its own box.
///
/// **Does not scroll**, per decision 7 — its height is however many lines
/// the initiative order asks for, and
/// `the_widest_tamper_line_fits_the_map_pane` is what keeps a single line
/// from ever demanding more width than the pane has to give.
pub(super) fn draw_tamper_block(
    view: &TacticalView,
    pane: Rect,
    strip_right: f32,
    strip_bottom: f32,
    strip_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let lines: Vec<String> = view.order.iter().filter_map(tamper_line).collect();
    if lines.is_empty() {
        return;
    }
    let pad = m.line_height * 0.35;
    let size = m.small();
    let widest = lines
        .iter()
        .map(|l| painter.measure_ui_advance(l, size))
        .fold(0.0_f32, f32::max);
    let w = (pad * 2.0 + widest).max(strip_w);
    let h = pad * 2.0 + m.line_height * lines.len() as f32;
    // Right-aligned on the strip's own right edge, so the two read as one
    // fixture rather than two boxes that happen to sit near each other.
    let x = strip_right - w;
    let y = strip_bottom + m.gap;
    if x < pane.x + strip_inset(m) || y + h > pane.y + pane.h {
        return;
    }
    painter.rect(x, y, w, h, Color::new(0.04, 0.06, 0.08, 0.88));
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);
    for (i, line) in lines.iter().enumerate() {
        painter.ui(
            line,
            x + pad,
            y + pad + m.line_height * i as f32 + size as f32 * 0.8,
            size,
            palette::LABEL,
        );
    }
}
