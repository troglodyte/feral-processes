//! The roster rail: every subject the manifest can page to, with the one on
//! screen picked out, down the sheet's left edge.

use super::layout::{RailGeom, rail_window};
use super::*;
use feral_processes_app_core::menu_shortcut;
use feral_processes_engine::RosterRow;

/// One drawn line of the rail, before the window cuts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RailRow {
    /// An index into the roster; also the row's shortcut.
    Subject(usize),
    /// The `─ party above ─` rule that follows the last party member.
    PartyRule,
}

/// The rail's lines in roster order, with the party rule after the last
/// member. `manifest_subjects` already puts party members before everyone
/// else, so the rule is a position and not a sort.
pub(super) fn rail_rows(roster: &[RosterRow]) -> Vec<RailRow> {
    let last_party = roster.iter().rposition(|r| r.in_party);
    let mut rows = Vec::with_capacity(roster.len() + 1);
    for i in 0..roster.len() {
        rows.push(RailRow::Subject(i));
        if last_party == Some(i) {
            rows.push(RailRow::PartyRule);
        }
    }
    rows
}

/// Cells before the name: caret, shortcut, a space, the glyph and a space.
const NAME_COL: usize = 5;
/// Cell the `Lv N` readout ends before, leaving room for the `!` mark.
const LEVEL_END_COL: usize = 28;
const ATTENTION_COL: usize = 28;
/// A collapsed rail is a glyph column: the glyph, then the `!`.
const COLLAPSED_GLYPH_COL: usize = 1;
const COLLAPSED_MARK_COL: usize = 2;

/// Draws the rail into `rail`, with `selected` (a roster index) among the
/// rows shown, and its divider on the right edge.
pub(super) fn draw_rail(
    rail: &RailGeom,
    roster: &[RosterRow],
    selected: usize,
    char_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let divider_x = rail.rect.x + rail.rect.w + m.pad / 2.0;
    painter.rect(
        divider_x,
        rail.rect.y,
        1.0,
        rail.rect.h,
        hud::palette::DIVIDER,
    );

    if let Some(title) = rail.title {
        let base = title.y + title.h * 0.75;
        painter.ui_bold("ROSTER", title.x, base, m.small(), hud::palette::PANE_TITLE);
        let x = title.x + painter.measure_ui_advance("ROSTER", m.small()) + char_w;
        painter.ui(
            format!("{} · party first", roster.len()),
            x,
            base,
            m.small(),
            hud::palette::FAINT,
        );
    }

    let rows = rail_rows(roster);
    let at = rows
        .iter()
        .position(|r| *r == RailRow::Subject(selected))
        .unwrap_or(0);
    let w = rail_window(rows.len(), at, rail.fits());
    let mut slot = 0;
    if w.above > 0 {
        draw_more(rail, slot, '↑', w.above, char_w, painter, m);
        slot += 1;
    }
    for row in &rows[w.start..w.end] {
        let top = rail.rows.y + slot as f32 * rail.row_h;
        match *row {
            RailRow::Subject(i) => {
                draw_subject(rail, &roster[i], i, i == selected, top, painter, m)
            }
            RailRow::PartyRule => draw_party_rule(rail, top, char_w, painter, m),
        }
        slot += 1;
    }
    if w.below > 0 {
        draw_more(rail, slot, '↓', w.below, char_w, painter, m);
    }
}

/// Where a row's text sits on its baseline.
fn baseline(top: f32, rail: &RailGeom) -> f32 {
    top + rail.row_h * 0.85
}

fn draw_subject(
    rail: &RailGeom,
    row: &RosterRow,
    index: usize,
    selected: bool,
    top: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let x = |col: usize| rail.rows.x + col as f32 * char_w;
    let base = baseline(top, rail);
    if selected {
        painter.rect(
            rail.rows.x,
            top,
            rail.rows.w,
            rail.row_h,
            hud::palette::PANE_BORDER,
        );
    }
    let glyph = row.glyph.to_string();
    let mark = |col: usize| {
        if row.needs_you.is_some() {
            painter.ui_bold("!", x(col), base, m.font_size, hud::palette::ATTENTION);
        }
    };
    if !rail.full {
        painter.map(
            &glyph,
            x(COLLAPSED_GLYPH_COL),
            base,
            m.font_size,
            glyph_color(row.color),
        );
        mark(COLLAPSED_MARK_COL);
        return;
    }
    if selected {
        painter.ui("▸", x(0), base, m.font_size, hud::palette::PANE_TITLE);
    }
    painter.ui(
        menu_shortcut(index).to_string(),
        x(1),
        base,
        m.font_size,
        hud::palette::EMPHASIS,
    );
    painter.map(&glyph, x(3), base, m.font_size, glyph_color(row.color));

    let level = format!("Lv {}", row.level);
    let level_col = LEVEL_END_COL - level.chars().count();
    painter.ui(
        &level,
        x(level_col),
        base,
        m.font_size,
        hud::palette::FIELD_LABEL,
    );
    let room = (level_col - 1 - NAME_COL) as f32 * char_w;
    painter.ui(
        elided_to_fit(painter, &row.name, m.font_size, room),
        x(NAME_COL),
        base,
        m.font_size,
        if selected {
            hud::palette::EMPHASIS
        } else {
            hud::palette::BODY
        },
    );
    mark(ATTENTION_COL);
}

fn draw_party_rule(rail: &RailGeom, top: f32, char_w: f32, painter: &Painter, m: &Metrics) {
    let text = if rail.full {
        "─ party above ─"
    } else {
        "──"
    };
    painter.ui(
        text,
        rail.rows.x + char_w,
        baseline(top, rail),
        m.font_size,
        hud::palette::FAINT,
    );
}

/// The `↑ N more` / `↓ N more` row. A collapsed rail has four cells, so it
/// says `↑N`.
fn draw_more(
    rail: &RailGeom,
    slot: usize,
    arrow: char,
    n: usize,
    char_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let text = if rail.full {
        format!("{arrow} {n} more")
    } else {
        format!("{arrow}{n}")
    };
    painter.ui(
        text,
        rail.rows.x + char_w,
        baseline(rail.rows.y + slot as f32 * rail.row_h, rail),
        m.font_size,
        hud::palette::FAINT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(in_party: bool) -> RosterRow {
        RosterRow {
            entity: Entity::PLACEHOLDER,
            name: "x".to_string(),
            glyph: 'x',
            color: GlyphColor::White,
            level: 1,
            in_party,
            needs_you: None,
        }
    }

    #[test]
    fn the_party_rule_follows_the_last_party_member_only() {
        let roster = [row(false), row(true), row(true), row(false)];
        assert_eq!(
            rail_rows(&roster),
            vec![
                RailRow::Subject(0),
                RailRow::Subject(1),
                RailRow::Subject(2),
                RailRow::PartyRule,
                RailRow::Subject(3),
            ]
        );
        assert_eq!(
            rail_rows(&[row(false), row(false)]).len(),
            2,
            "no party, no rule"
        );
    }
}
