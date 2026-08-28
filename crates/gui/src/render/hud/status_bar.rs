//! The status bar: one row across the top of every screen that draws the
//! world behind it.
//!
//! Three zones. The **left** says who and where you are — identity, zone,
//! position, tick. The **centre** is what the base is holding, which is the
//! stock strip this bar absorbed rather than reimplemented: `stock::fits` is
//! still the one answer to how many piles fit, and it is simply handed a
//! narrower budget than the whole window. The **right** carries the attention
//! badge — the first row of `Game::attention` and its keycap, or
//! `ALL NOMINAL` when nothing holds. It is one of that call's three
//! readouts, alongside the info column's tab markers and its collapsed
//! bars, and it derives nothing of its own.
//!
//! The centre is the only elastic zone. The identity block is measured and
//! subtracted first and the badge zone is reserved at a fixed fraction, so
//! the badge appearing does not re-lay the bar out.
//!
//! The row has no wrap and no clip — `Painter` clips vertically and never
//! horizontally — so what does not fit is **counted**, not drawn off the
//! end. That is `stock::fits`' rule and the reason this module has a width
//! census.

use feral_processes_engine::{AttentionRow, StockRow};

use super::{palette, strip};
use crate::paint::{Color, Painter, Rect, TextRun};
use crate::render::stock;
use crate::text::Metrics;

/// Between one field of the identity block and the next.
const SEP: &str = " · ";
/// The share of the bar held back for the attention badge.
const BADGE_FRAC: f32 = 0.22;
/// Between the identity block and the first stock pile.
const ZONE_GAP: &str = "   ";

/// What the bar reads, gathered by the caller before the `Game` borrow.
pub(in crate::render) struct StatusBarState<'a> {
    pub zone: u32,
    pub position: (i32, i32),
    pub tick: u64,
    pub stock: &'a [StockRow],
    /// `Game::attention`, called once by the caller and shared with the
    /// info column. This never derives its own.
    pub attention: &'a [AttentionRow],
}

/// The identity block, as coloured runs. Pure, so the census can measure
/// what will actually be drawn rather than an estimate of it.
fn identity_runs(state: &StatusBarState) -> Vec<(String, Color, bool)> {
    let (x, y) = state.position;
    vec![
        ("feral".to_string(), palette::EMPHASIS, true),
        ("-processes".to_string(), palette::LABEL, false),
        (SEP.to_string(), palette::FAINT, false),
        ("ZONE ".to_string(), palette::FIELD_LABEL, false),
        (state.zone.to_string(), palette::BODY, false),
        (SEP.to_string(), palette::FAINT, false),
        (format!("({x}, {y})"), palette::BODY, false),
        (SEP.to_string(), palette::FAINT, false),
        ("tick ".to_string(), palette::FIELD_LABEL, false),
        (state.tick.to_string(), palette::BODY, false),
    ]
}

/// The plain text of the identity block — what gets measured.
fn identity_text(state: &StatusBarState) -> String {
    identity_runs(state)
        .into_iter()
        .map(|(t, _, _)| t)
        .collect()
}

/// The badge, as coloured pieces: the leading condition upper-cased, its
/// keycap, and a dim `+N` for the rest — or `ALL NOMINAL` when nothing
/// holds.
///
/// The calm state is a real state and is drawn, not an empty gap. The count
/// rides as a suffix rather than as a list because this is one row on a bar
/// that already has two other zones on it; the column is where the rest of
/// the conditions are read.
///
/// The keycap stays `palette::EMPHASIS` in both colourings — a keycap is a
/// keycap, and running it in the row's own colour would make a reservation
/// decorative.
fn badge_pieces(attention: &[AttentionRow]) -> Vec<strip::Piece> {
    let Some(first) = attention.first() else {
        return vec![("ALL NOMINAL".to_string(), palette::HEALTHY, true)];
    };
    let color = if first.threat {
        palette::THREAT
    } else {
        palette::ATTENTION
    };
    let mut pieces = vec![
        (first.text.to_uppercase(), color, true),
        (" ".to_string(), palette::FAINT, false),
        (format!("[{}]", first.key), palette::EMPHASIS, false),
    ];
    let rest = attention.len() - 1;
    if rest > 0 {
        pieces.push((format!(" +{rest}"), palette::FAINT, false));
    }
    pieces
}

/// How much room the stock piles have, once the identity block and the
/// reserved badge zone have taken theirs.
fn stock_avail(at: Rect, identity_w: f32, painter: &Painter, m: &Metrics) -> f32 {
    let gap = painter.measure_ui_advance(ZONE_GAP, m.font_size);
    (at.w - m.inset * 2.0 - identity_w - gap - at.w * BADGE_FRAC).max(0.0)
}

pub(in crate::render) fn draw_status_bar(
    at: Rect,
    state: &StatusBarState,
    painter: &Painter,
    m: &Metrics,
) {
    painter.rect(at.x, at.y, at.w, at.h, palette::STATUS_BG);
    painter.line(
        at.x,
        at.y + at.h,
        at.x + at.w,
        at.y + at.h,
        2.0,
        palette::PANE_BORDER,
    );

    let baseline = at.y + m.inset + m.font_size as f32 / 2.0;
    let owned = identity_runs(state);
    let runs: Vec<TextRun> = owned
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect();
    painter.ui_runs(&runs, at.x + m.inset, baseline, m.font_size);

    // Above the stock half's early return, for the identity block's reason
    // one zone along: whether anything needs the player is not conditional
    // on the base holding cargo.
    draw_badge(at, state.attention, baseline, painter, m);

    // Drawn unconditionally, and before any early return the stock half
    // might want: the identity block is not conditional on the base holding
    // anything, and `draw_stock_strip` used to return early on an empty
    // base.
    let identity_w = painter.measure_ui_advance(identity_text(state), m.font_size);
    let gap = painter.measure_ui_advance(ZONE_GAP, m.font_size);
    let stock_x = at.x + m.inset + identity_w + gap;
    let avail = stock_avail(at, identity_w, painter, m);

    if state.stock.is_empty() {
        painter.ui(
            "base stock: none",
            stock_x,
            baseline,
            m.font_size,
            palette::FAINT,
        );
        return;
    }

    let pieces = stock::pieces(state.stock);
    let shown = stock::fits(&pieces, avail, painter, m);
    let mut stock_runs: Vec<TextRun> = Vec::new();
    let pair_gap = "   ".to_string();
    let tail;
    for (i, (tag, qty)) in pieces.iter().take(shown).enumerate() {
        if i > 0 {
            stock_runs.push(TextRun {
                text: &pair_gap,
                bold: false,
                color: palette::FAINT,
            });
        }
        stock_runs.push(TextRun {
            text: tag,
            bold: false,
            color: palette::FIELD_LABEL,
        });
        stock_runs.push(TextRun {
            text: qty,
            bold: false,
            color: palette::BODY,
        });
    }
    let dropped = pieces.len() - shown;
    if dropped > 0 {
        tail = format!("   +{dropped}");
        stock_runs.push(TextRun {
            text: &tail,
            bold: false,
            color: palette::FAINT,
        });
    }
    painter.ui_runs(&stock_runs, stock_x, baseline, m.font_size);
}

/// Right-aligns the badge inside its reserved zone. What does not fit is
/// dropped from the end through `strip::fitting`, never clipped — the row's
/// rule, and `stock::fits`' before it.
fn draw_badge(at: Rect, attention: &[AttentionRow], baseline: f32, painter: &Painter, m: &Metrics) {
    // Piece by piece rather than all or nothing, so a long condition sheds
    // its `+N` and then its keycap rather than vanishing entirely. Not
    // `strip::fitting`, which joins its segments with ` · ` — a badge is one
    // phrase, not a list of them.
    let avail = at.w * BADGE_FRAC - m.inset;
    let mut taken: Vec<strip::Piece> = Vec::new();
    for piece in badge_pieces(attention) {
        let mut next = taken.clone();
        next.push(piece);
        let text: String = next.iter().map(|(t, _, _)| t.as_str()).collect();
        if painter.measure_ui_advance(&text, m.font_size) > avail {
            break;
        }
        taken = next;
    }
    if taken.is_empty() {
        return;
    }
    let text: String = taken.iter().map(|(t, _, _)| t.as_str()).collect();
    let w = painter.measure_ui_advance(&text, m.font_size);
    let runs: Vec<TextRun> = taken
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect();
    painter.ui_runs(&runs, at.x + at.w - m.inset - w, baseline, m.font_size);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::{AttentionKind, AttentionRow};

    fn nagging(kind: AttentionKind, text: &str, threat: bool) -> AttentionRow {
        AttentionRow {
            kind,
            text: text.to_string(),
            key: 'b',
            threat,
        }
    }

    fn stock_rows(piles: &[(String, u32)]) -> Vec<StockRow> {
        piles
            .iter()
            .map(|(tag, qty)| StockRow {
                item: ItemId::from(tag.as_str()),
                tag: tag.clone(),
                name: tag.clone(),
                qty: *qty,
            })
            .collect()
    }

    /// Far more piles than the shipped set holds, each wide enough to be
    /// awkward.
    fn crowded() -> Vec<StockRow> {
        let piles: Vec<(String, u32)> = (0..60)
            .map(|i| (format!("W{i}"), 999_999 - i as u32))
            .collect();
        stock_rows(&piles)
    }

    /// The identity block at its widest plausible values, so the census is
    /// not passing on a short one.
    fn wide_state(stock: &[StockRow]) -> StatusBarState<'_> {
        StatusBarState {
            zone: 16,
            position: (-9999, -9999),
            tick: 9_999_999,
            stock,
            attention: &[],
        }
    }

    /// The one thing this row must never do. It is a single line with no
    /// wrap and no clip, so a line wider than its rect is not cut off — it
    /// is drawn past the edge and the piles at the end simply are not
    /// there, silently.
    #[test]
    fn the_status_bar_never_draws_wider_than_its_rect() {
        let m = ui_metrics(900.0);
        let rows = crowded();
        let state = wide_state(&rows);
        with_painter(|p| {
            let at = Rect::new(0.0, 0.0, p.screen_w(), m.line_height + m.inset);
            let identity_w = p.measure_ui_advance(identity_text(&state), m.font_size);
            let avail = stock_avail(at, identity_w, p, &m);
            let pieces = stock::pieces(state.stock);
            let shown = stock::fits(&pieces, avail, p, &m);
            assert!(shown > 0, "something has to fit");
            assert!(shown < pieces.len(), "the fixture must overflow the row");

            let gap = p.measure_ui_advance(ZONE_GAP, m.font_size);
            let drawn = p.measure_ui_advance(stock::line(&pieces, shown), m.font_size);
            let used = m.inset + identity_w + gap + drawn;
            assert!(
                used <= at.w - at.w * BADGE_FRAC,
                "bar uses {used} of {} with the badge zone reserved",
                at.w - at.w * BADGE_FRAC
            );
        });
    }

    /// The identity block has to actually take its space, and this has to
    /// measure *that* term rather than any other.
    ///
    /// The obvious form — comparing against the whole window — passes on
    /// the badge reservation alone, so it stays green with the identity
    /// term deleted from `stock_avail` and is worth nothing. The baseline
    /// is therefore a budget that has already given the badge zone up, so
    /// the identity block is the only difference left between them.
    #[test]
    fn the_left_zone_survives_a_crowded_base() {
        let m = ui_metrics(900.0);
        let rows = crowded();
        let state = wide_state(&rows);
        with_painter(|p| {
            let at = Rect::new(0.0, 0.0, p.screen_w(), m.line_height + m.inset);
            let pieces = stock::pieces(state.stock);
            let gap = p.measure_ui_advance(ZONE_GAP, m.font_size);
            let without_identity = at.w - m.inset * 2.0 - gap - at.w * BADGE_FRAC;
            let baseline = stock::fits(&pieces, without_identity, p, &m);
            let identity_w = p.measure_ui_advance(identity_text(&state), m.font_size);
            let in_bar = stock::fits(&pieces, stock_avail(at, identity_w, p, &m), p, &m);
            assert!(
                in_bar < baseline,
                "identity block took no space: {in_bar} piles fit either way"
            );
        });
    }

    /// Guards against an early return copied from `draw_stock_strip`, which
    /// returns after writing its empty-base line and would take the whole
    /// identity block with it.
    #[test]
    fn an_empty_base_still_names_itself() {
        let m = ui_metrics(900.0);
        let state = wide_state(&[]);
        let (_, shapes) = with_painter(|p| {
            let at = Rect::new(0.0, 0.0, p.screen_w(), m.line_height + m.inset);
            draw_status_bar(at, &state, p, &m);
        });
        let text = painted_text(&shapes).join("");
        assert!(text.contains("feral"), "identity missing from {text:?}");
        assert!(text.contains("ZONE"), "zone missing from {text:?}");
        assert!(text.contains("16"), "zone number missing from {text:?}");
        assert!(text.contains("tick"), "tick missing from {text:?}");
    }

    /// The calm state is a real state and is drawn, not an empty gap.
    #[test]
    fn a_calm_base_reads_all_nominal() {
        let pieces = badge_pieces(&[]);
        let text: String = pieces.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(text, "ALL NOMINAL");
        assert_eq!(pieces[0].1, palette::HEALTHY);
    }

    #[test]
    fn the_badge_names_the_first_row_and_its_key() {
        let rows = [nagging(
            AttentionKind::IdleStructures,
            "4 nodes without a program",
            false,
        )];
        let text: String = badge_pieces(&rows)
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect();
        assert_eq!(text, "4 NODES WITHOUT A PROGRAM [b]");
    }

    /// One row on a bar that already has two other zones on it: the column
    /// is where the rest of the conditions are read.
    #[test]
    fn a_second_condition_is_counted_not_listed() {
        let rows = [
            nagging(
                AttentionKind::IdleStructures,
                "4 nodes without a program",
                false,
            ),
            nagging(AttentionKind::PerkPoints, "2 perk points unspent", false),
            nagging(AttentionKind::RosterFull, "roster full (3/3)", false),
        ];
        let text: String = badge_pieces(&rows)
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect();
        assert_eq!(text, "4 NODES WITHOUT A PROGRAM [b] +2");
        assert!(
            !text.contains("PERK"),
            "the badge listed a second condition: {text:?}"
        );
    }

    /// Both halves in one test: either alone passes against a badge drawing
    /// one constant colour.
    #[test]
    fn a_threat_badge_is_red() {
        let hostile = [nagging(
            AttentionKind::StructureDamaged,
            "Mining Node damaged",
            true,
        )];
        let calm = [nagging(
            AttentionKind::IdleStructures,
            "1 node without a program",
            false,
        )];
        assert_eq!(badge_pieces(&hostile)[0].1, palette::THREAT);
        assert_eq!(badge_pieces(&calm)[0].1, palette::ATTENTION);
        // The keycap is a keycap either way.
        assert_eq!(badge_pieces(&hostile)[2].1, palette::EMPHASIS);
        assert_eq!(badge_pieces(&calm)[2].1, palette::EMPHASIS);
    }

    /// The row has no wrap and no clip, so a badge wider than its zone is
    /// drawn over the stock piles rather than cut off.
    #[test]
    fn the_badge_stays_inside_its_zone() {
        let m = ui_metrics(720.0);
        let rows: Vec<AttentionRow> = (0..4)
            .map(|i| {
                nagging(
                    AttentionKind::IdleStructures,
                    &format!("{i} interminably named structures without a program"),
                    false,
                )
            })
            .collect();
        let stock = crowded();
        let state = StatusBarState {
            attention: &rows,
            ..wide_state(&stock)
        };
        let at = Rect::new(0.0, 0.0, 1280.0, m.line_height + m.inset);
        let avail = at.w * BADGE_FRAC - m.inset;
        let (_, shapes) = with_painter(|p| {
            let baseline = at.y + m.inset + m.font_size as f32 / 2.0;
            let whole: String = badge_pieces(state.attention)
                .iter()
                .map(|(t, _, _)| t.as_str())
                .collect();
            assert!(
                p.measure_ui_advance(&whole, m.font_size) > avail,
                "the fixture must overflow the badge zone or it proves nothing"
            );
            draw_badge(at, state.attention, baseline, p, &m);
        });
        // Nothing else was drawn into this painter, so every glyph is the
        // badge's.
        let drawn = painted_text(&shapes).join("");
        with_painter(|p| {
            assert!(
                p.measure_ui_advance(&drawn, m.font_size) <= avail,
                "the badge drew {:.1}px into a {avail:.1}px zone: {drawn:?}",
                p.measure_ui_advance(&drawn, m.font_size)
            );
        });
    }

    /// The claim the module doc has been making since phase 1 with nothing
    /// checking it: the badge appearing does not re-lay the bar out.
    #[test]
    fn the_badge_does_not_move_the_stock_strip() {
        let m = ui_metrics(900.0);
        let stock = stock_rows(&[("CF".to_string(), 12)]);
        let rows = [nagging(
            AttentionKind::IdleStructures,
            "4 nodes without a program",
            false,
        )];
        let calm = wide_state(&stock);
        let nagged = StatusBarState {
            attention: &rows,
            ..wide_state(&stock)
        };
        with_painter(|p| {
            let at = Rect::new(0.0, 0.0, p.screen_w(), m.line_height + m.inset);
            let identity_w = p.measure_ui_advance(identity_text(&calm), m.font_size);
            assert_eq!(
                stock_avail(at, identity_w, p, &m),
                stock_avail(
                    at,
                    p.measure_ui_advance(identity_text(&nagged), m.font_size),
                    p,
                    &m
                )
            );
        });
    }
}
