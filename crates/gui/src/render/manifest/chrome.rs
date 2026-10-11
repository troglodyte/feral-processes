//! What every face of the sheet shares: the frame, the roster rail and the
//! summary band. A face paints its own body into the layout this returns.

use super::band::{BandHead, Tabs, draw_band};
use super::layout::{Face, ManifestLayout};
use super::rail::{draw_rail, rail_rows};
use super::*;
use feral_processes_engine::{CallToAct, RosterRow};

/// Who the sheet is about and what the band says of them.
pub(super) struct Head<'a> {
    pub(super) view: &'a ManifestView,
    pub(super) roster: &'a [RosterRow],
    pub(super) tags: &'a [String],
    pub(super) tabs: Tabs<'a>,
    /// STATS alone carries a call to act.
    pub(super) call: Option<CallToAct>,
}

/// Paints the frame, rail and band into `frame` and returns where the rest
/// goes. `band_rows` is the bands' row count under the body; zero for none.
pub(super) fn paint_chrome(
    frame: Rect,
    face: Face,
    band_rows: usize,
    head: &Head,
    painter: &Painter,
    m: &Metrics,
) -> ManifestLayout {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let selected = head
        .roster
        .iter()
        .position(|r| r.entity == head.view.entity);
    let rail_rows_n = selected.map_or(0, |_| rail_rows(head.roster).len());
    let l = ManifestLayout::new(frame, m, char_w, face, rail_rows_n, band_rows);

    painter.rect(frame.x, frame.y, frame.w, frame.h, hud::palette::STATUS_BG);
    painter.rect_lines(
        frame.x,
        frame.y,
        frame.w,
        frame.h,
        2.0,
        hud::palette::PANE_BORDER,
    );
    if let (Some(rail), Some(selected)) = (&l.rail, selected) {
        draw_rail(rail, head.roster, selected, char_w, painter, m);
    }
    let view = head.view;
    let kind = subject_kind(view);
    draw_band(
        l.band,
        &BandHead {
            glyph: view.glyph,
            glyph_color: glyph_color(view.color),
            name: &view.name,
            kind: kind.as_deref(),
            boss: is_boss(view),
            rarity: header_rarity(view),
            tags: head.tags,
            tabs: head.tabs,
            call: head.call,
        },
        painter,
        m,
    );
    l
}
