//! The info column's shell: the frame, the tab row, and the two collapsed
//! bars.
//!
//! Three panes — BASE, CREW, PACK — one open and two collapsed to a single
//! live row each. **A closed pane must never hide an actionable state**, and
//! the tab markers and the collapsed bars are two of the three readouts of
//! `Game::attention` that make that true; the status bar's badge is the
//! third. None of them derives anything: `draw_playing_base` calls once and
//! hands the same slice to all of them.
//!
//! What goes *inside* the open pane is not this module's business. It
//! returns the body rect and the caller fills it — today with the status
//! panel this column is replacing, from phase 5 with the real BASE/CREW/PACK
//! contents.
//!
//! The column does not scroll, the same as the gear inspect and memories
//! pages, so the body rect's height is a layout constraint rather than a
//! starting point. See `docs/superpowers/archive/specs/2026-08-27-paned-command-hud-design.md`.

use feral_processes_app_core::InfoTab;
use feral_processes_engine::{AttentionKind, AttentionRow};

use super::palette;
use super::panes::{PaneData, summary};
use crate::paint::{Color, Painter, Rect, TextRun};
use crate::text::Metrics;

/// Between one tab cell and the next.
const TAB_GAP: &str = "   ";
/// How many collapsed bars the column holds: every tab but the open one.
/// Derived from `InfoTab::ALL` rather than written down, or a fifth tab
/// would lose its bar to `draw`'s `zip` with nothing failing to compile.
const ALL_BUT_ONE: usize = InfoTab::ALL.len() - 1;
/// A tab wearing this needs the player.
const MARK_ACT: &str = "!";
/// A tab with nothing to say. The handoff reserves a cyan `·` for "merely
/// notable" as well; there is no notable-but-not-actionable condition in the
/// model, so that state is not drawn rather than invented.
const MARK_CALM: &str = "·";

/// What the column draws around whatever is in its open pane.
pub(in crate::render) struct ColumnState<'a> {
    pub tab: InfoTab,
    /// `Game::attention`, called once by the caller and shared with the
    /// status bar.
    pub attention: &'a [AttentionRow],
    /// What the closed tabs summarise. The same struct the open pane's
    /// rows are built from, so a collapsed bar and the pane it stands for
    /// can never disagree about what the base is holding.
    pub pane: &'a PaneData<'a>,
}

/// Where the column's three fixed pieces sit. Pure arithmetic, so the tiling
/// is testable without drawing anything — `hud::layout`'s reason at the next
/// scale down.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(in crate::render) struct ColumnRegions {
    pub tabs: Rect,
    /// What the open pane draws into. The one figure phase 5 lays five
    /// blocks against.
    pub body: Rect,
    /// The closed tabs' summary rows, in `InfoTab::ALL` order with the open
    /// one skipped. One per tab that is not open, so the length is
    /// `InfoTab::ALL.len() - 1` — `draw` zips the closed tabs against these,
    /// and a short array would drop the last tab's bar in silence.
    pub bars: [Rect; ALL_BUT_ONE],
}

pub(in crate::render) fn regions(at: Rect, m: &Metrics) -> ColumnRegions {
    let row = m.line_height;
    let tabs = Rect::new(at.x, at.y + m.inset, at.w, row);
    // Pinned to the bottom edge, which is what makes the body's height fall
    // out of subtraction rather than out of however much the open pane drew.
    let bars_top = at.y + at.h - m.inset - row * ALL_BUT_ONE as f32;
    let body = Rect::new(
        at.x,
        tabs.y + row,
        at.w,
        (bars_top - m.gap - (tabs.y + row)).max(0.0),
    );
    ColumnRegions {
        tabs,
        body,
        bars: std::array::from_fn(|i| Rect::new(at.x, bars_top + row * i as f32, at.w, row)),
    }
}

/// Which pane a condition belongs to.
///
/// Exhaustive, `cell_mark`'s rule: as a `_ =>` arm a new condition ships
/// with no marker on any tab and no line in any collapsed bar, which is the
/// exact failure the tabbed column exists to prevent. Nothing routes to
/// `Pack` — the pack has no capacity and so nothing to ask for — and that is
/// a fact about the four conditions rather than a gap.
fn tab_of(kind: AttentionKind) -> InfoTab {
    match kind {
        AttentionKind::StructureDamaged | AttentionKind::IdleStructures => InfoTab::Base,
        AttentionKind::PerkPoints | AttentionKind::RosterFull => InfoTab::Crew,
    }
}

/// The leading row a tab is wearing a mark for, if any. `Game::attention` is
/// already sorted most urgent first, so this is the first match and never a
/// second sort.
fn leading(attention: &[AttentionRow], tab: InfoTab) -> Option<&AttentionRow> {
    attention.iter().find(|r| tab_of(r.kind) == tab)
}

fn mark_color(row: &AttentionRow) -> Color {
    if row.threat {
        palette::THREAT
    } else {
        palette::ATTENTION
    }
}

/// Frames the column, draws the tab row and the two collapsed bars, and
/// returns the body rect of the open pane.
///
/// Fill, then contents, then the frame — `hud::strip`'s ordering rule at the
/// scale of a pane, so nothing this draws is painted over by the fill it
/// sits on.
pub(in crate::render) fn draw_info_column(
    at: Rect,
    state: &ColumnState,
    painter: &Painter,
    m: &Metrics,
) -> Rect {
    let r = regions(at, m);
    painter.rect(at.x, at.y, at.w, at.h, palette::STATUS_BG);

    draw_tab_row(r.tabs, state, painter, m);

    // A hairline between the open pane and the two rows summarising the
    // closed ones, so the bars do not read as more of the pane above them.
    let rule_y = r.bars[0].y - m.gap / 2.0;
    painter.line(
        at.x + m.inset,
        rule_y,
        at.x + at.w - m.inset,
        rule_y,
        1.0,
        palette::DIVIDER,
    );
    let closed = InfoTab::ALL.iter().filter(|t| **t != state.tab);
    for (tab, rect) in closed.zip(r.bars.iter()) {
        draw_collapsed_bar(*rect, *tab, state, painter, m);
    }

    painter.rect_lines(at.x, at.y, at.w, at.h, 2.0, palette::PANE_BORDER);
    r.body
}

/// The tab row's pieces, in draw order.
///
/// Split from [`draw_tab_row`] so a census can measure exactly the string
/// that is drawn, `contracts.rs`' `contract_footer` for the same reason: the
/// row grows with every tab and with the *length of its label*, and
/// `Painter` clips nothing horizontally — so it would run off the column in
/// silence rather than failing anything.
fn tab_row_pieces(state: &ColumnState) -> Vec<(String, Color, bool)> {
    let mut owned: Vec<(String, Color, bool)> = Vec::new();
    for (i, tab) in InfoTab::ALL.iter().enumerate() {
        if i > 0 {
            owned.push((TAB_GAP.to_string(), palette::FAINT, false));
        }
        let open = *tab == state.tab;
        owned.push((format!("{} ", i + 1), palette::EMPHASIS, false));
        owned.push((
            tab.label().to_string(),
            if open {
                palette::PANE_TITLE
            } else {
                palette::LABEL
            },
            open,
        ));
        match leading(state.attention, *tab) {
            Some(row) => owned.push((format!(" {MARK_ACT}"), mark_color(row), true)),
            None => owned.push((format!(" {MARK_CALM}"), palette::LABEL, false)),
        }
    }
    owned
}

fn draw_tab_row(at: Rect, state: &ColumnState, painter: &Painter, m: &Metrics) {
    let size = m.small();
    let owned = tab_row_pieces(state);
    let runs: Vec<TextRun> = owned
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect();
    painter.ui_runs(&runs, at.x + m.inset, at.y + size as f32 / 2.0, size);
}

/// One closed tab's summary row.
///
/// **A condition outranks a headcount.** With something to say the bar says
/// it, in the attention colour; calm, it carries `panes::summary` — the
/// closed pane's live figure, built from the same `PaneData` the open pane's
/// rows are, so the two can never disagree.
fn draw_collapsed_bar(at: Rect, tab: InfoTab, state: &ColumnState, painter: &Painter, m: &Metrics) {
    let size = m.small();
    let baseline = at.y + size as f32 / 2.0;
    let (mark, text, color) = match leading(state.attention, tab) {
        Some(row) => (MARK_ACT, row.text.clone(), mark_color(row)),
        None => (MARK_CALM, summary(tab, state.pane), palette::FAINT),
    };
    let label = format!("{} ", tab.label());
    let runs = [
        TextRun {
            text: &label,
            bold: false,
            color: palette::LABEL,
        },
        TextRun {
            text: mark,
            bold: true,
            color,
        },
        TextRun {
            text: " ",
            bold: false,
            color,
        },
        TextRun {
            text: &text,
            bold: false,
            color,
        },
    ];
    painter.ui_runs(&runs, at.x + m.inset, baseline, size);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Painted, paint_order, painted_runs_in, painted_text, with_painter};
    use crate::text::ui_metrics;

    fn column() -> Rect {
        Rect::new(900.0, 30.0, 380.0, 690.0)
    }

    fn row(kind: AttentionKind, text: &str, threat: bool) -> AttentionRow {
        AttentionRow {
            kind,
            text: text.to_string(),
            key: 'b',
            threat,
        }
    }

    fn idle() -> AttentionRow {
        row(
            AttentionKind::IdleStructures,
            "4 nodes without a program",
            false,
        )
    }

    /// The one figure phase 5 lays five blocks against: the body has to sit
    /// clear of the tab row above it and the two bars below it, and the
    /// three pieces have to stay inside the column.
    #[test]
    fn the_column_regions_tile_it() {
        for h in [720.0, 810.0, 1080.0] {
            let m = ui_metrics(h);
            let at = Rect::new(900.0, 30.0, 380.0, h - 30.0);
            let r = regions(at, &m);
            assert!(r.tabs.y >= at.y, "the tab row starts above the column");
            assert!(
                r.body.y >= r.tabs.y + r.tabs.h,
                "the body overlaps the tab row at {h}"
            );
            assert!(
                r.body.y + r.body.h <= r.bars[0].y,
                "the body runs into the collapsed bars at {h}"
            );
            assert!(
                r.bars[1].y + r.bars[1].h <= at.y + at.h,
                "the second collapsed bar hangs off the bottom at {h}"
            );
            assert!(r.body.h > 0.0, "the body has no room at {h}");
        }
    }

    #[test]
    fn the_open_tab_is_the_one_the_state_names() {
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| {
            draw_info_column(
                column(),
                &ColumnState {
                    tab: InfoTab::Crew,
                    attention: &[],
                    pane: &PaneData::default(),
                },
                p,
                &m,
            );
        });
        let open = painted_runs_in(&shapes, palette::PANE_TITLE, true);
        assert_eq!(open, vec!["CREW".to_string()]);
        // Adjacent runs of one style are merged into a section by egui's
        // layout job, so a closed tab's label arrives joined to its calm
        // mark — `contains`, not equality.
        let closed = painted_runs_in(&shapes, palette::LABEL, false);
        assert!(
            closed.iter().any(|s| s.contains("BASE")) && closed.iter().any(|s| s.contains("PACK")),
            "the two closed tabs are not dimmed: {closed:?}"
        );
    }

    /// **The sentence the whole design rests on.** With CREW open, a
    /// condition inside BASE still wears its mark on the tab *and* says what
    /// it is on the collapsed bar.
    #[test]
    fn a_closed_tab_still_wears_its_marker() {
        let m = ui_metrics(900.0);
        let rows = [idle()];
        let (_, shapes) = with_painter(|p| {
            draw_info_column(
                column(),
                &ColumnState {
                    tab: InfoTab::Crew,
                    attention: &rows,
                    pane: &PaneData::default(),
                },
                p,
                &m,
            );
        });
        let marks = painted_runs_in(&shapes, palette::ATTENTION, true);
        assert!(
            marks.iter().any(|s| s.trim() == MARK_ACT),
            "no attention mark drawn: {marks:?}"
        );
        let text = painted_text(&shapes).join(" ");
        assert!(
            text.contains("4 nodes without a program"),
            "the collapsed bar hid the condition: {text:?}"
        );
    }

    /// A calm tab is not an empty tab. With nothing needing the player the
    /// collapsed bars carry `panes::summary` — the closed pane's live
    /// figures, built from the same `PaneData` the open pane's rows are, so a
    /// bar and the pane it stands for cannot disagree.
    #[test]
    fn a_calm_column_reads_its_live_summary() {
        let m = ui_metrics(900.0);
        let pane = PaneData {
            roster: (9, 33),
            carrying: 14,
            ..Default::default()
        };
        let (_, shapes) = with_painter(|p| {
            draw_info_column(
                column(),
                &ColumnState {
                    tab: InfoTab::Base,
                    attention: &[],
                    pane: &pane,
                },
                p,
                &m,
            );
        });
        assert!(
            painted_runs_in(&shapes, palette::ATTENTION, true).is_empty(),
            "a calm column drew an attention mark"
        );
        assert!(
            painted_runs_in(&shapes, palette::THREAT, true).is_empty(),
            "a calm column drew a threat mark"
        );
        let text = painted_text(&shapes).join(" ");
        assert!(
            text.contains("9/33"),
            "the CREW bar hid the roster count: {text:?}"
        );
        assert!(
            text.contains("14 units"),
            "the PACK bar hid what is carried: {text:?}"
        );
    }

    #[test]
    fn a_threat_colours_its_tab_and_its_bar() {
        let m = ui_metrics(900.0);
        let rows = [row(
            AttentionKind::StructureDamaged,
            "Mining Node damaged",
            true,
        )];
        let (_, shapes) = with_painter(|p| {
            draw_info_column(
                column(),
                &ColumnState {
                    tab: InfoTab::Crew,
                    attention: &rows,
                    pane: &PaneData::default(),
                },
                p,
                &m,
            );
        });
        assert!(
            painted_runs_in(&shapes, palette::ATTENTION, true).is_empty(),
            "a threat drew in the act colour"
        );
        let hot = painted_runs_in(&shapes, palette::THREAT, true);
        assert!(
            hot.iter().any(|s| s.trim() == MARK_ACT),
            "the tab mark is not the threat colour: {hot:?}"
        );
        let bar = painted_runs_in(&shapes, palette::THREAT, false);
        assert!(
            bar.iter().any(|s| s.contains("Mining Node damaged")),
            "the collapsed bar is not the threat colour: {bar:?}"
        );
    }

    /// The fill has to land before the text that sits on it — phase 2's
    /// `the_map_frame_draws_after_the_map`, at the scale of one pane.
    #[test]
    fn the_column_fills_before_it_writes() {
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| {
            draw_info_column(
                column(),
                &ColumnState {
                    tab: InfoTab::Base,
                    attention: &[],
                    pane: &PaneData::default(),
                },
                p,
                &m,
            );
        });
        let order = paint_order(&shapes);
        let fill = order
            .iter()
            .position(|k| *k == Painted::Rect)
            .expect("the column fills itself");
        let text = order
            .iter()
            .position(|k| *k == Painted::Text)
            .expect("the column draws its tabs");
        assert!(fill < text, "fill at {fill}, first glyph at {text}");
    }

    /// Every kind has a home. As a `_ =>` arm a fifth condition would ship
    /// with no marker anywhere — the exact failure the tabbed column exists
    /// to prevent — so this walks the whole enum by hand.
    /// **The tab row grows with every tab and with the length of its
    /// label**, and `ui_runs` clips nothing horizontally — so a label one
    /// word too long draws off the column in silence, which is exactly how
    /// the Contracts screen's footer shipped seven characters over its
    /// popup. Measured with every tab wearing its `!` mark, the widest the
    /// row ever draws.
    #[test]
    fn the_tab_row_fits_the_column() {
        let at = column();
        let m = ui_metrics(720.0);
        let r = regions(at, &m);
        // `draw_tab_row` starts at `at.x + m.inset`, so that is the margin
        // it has to live inside on both sides.
        let room = r.tabs.w - m.inset * 2.0;
        let attention: Vec<AttentionRow> = InfoTab::ALL
            .iter()
            .map(|_| row(AttentionKind::IdleStructures, "idle", false))
            .collect();
        let pane = PaneData::default();
        let state = ColumnState {
            tab: InfoTab::Base,
            attention: &attention,
            pane: &pane,
        };
        let drawn: String = tab_row_pieces(&state)
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect();
        with_painter(|p| {
            let width = p.measure_ui_advance(&drawn, m.small());
            assert!(
                width <= room,
                "the tab row overflows the column by {:.0}px \
                 ({width:.0} drawn into {room:.0}):\n{drawn}",
                width - room
            );
        });
    }

    #[test]
    fn every_kind_routes_to_a_tab() {
        for kind in [
            AttentionKind::StructureDamaged,
            AttentionKind::IdleStructures,
            AttentionKind::PerkPoints,
            AttentionKind::RosterFull,
        ] {
            let tab = tab_of(kind);
            assert!(
                InfoTab::ALL.contains(&tab),
                "{kind:?} routes outside the column"
            );
        }
    }
}
