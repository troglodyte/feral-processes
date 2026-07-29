//! Where every box on the manifest screen goes.
//!
//! Kept free of `Painter` for the same reason `popup_layout` is: what fits is
//! settled in whole rows before any of it becomes pixels, and that arithmetic
//! is then testable at every window size without a window.

use super::bars::bar_row_height;
use crate::paint::Rect;
use crate::text::Metrics;

/// One titled box. Built by the draw function and handed here, so a section
/// with no data is simply absent from the slice and the boxes after it move
/// up — rather than the layout having to know which subject it is drawing.
pub(super) struct Section {
    pub(super) title: &'static str,
    pub(super) rows: Vec<SectionRow>,
    /// Spans both columns, below the grid. The Moves band.
    pub(super) full_width: bool,
}

pub(super) enum SectionRow {
    /// A label on the left, its value right-aligned against the box's inner
    /// edge.
    Stat(String, String),
    /// One run of free text across the box.
    Note(String),
}

/// The most rows any one box draws. A section is bounded so the worst-case
/// page height is bounded — otherwise a modded species with twenty moves
/// would silently push the footer off the bottom.
///
/// 6 is what the tightest supported window (720px, where the UI font is 19px)
/// has room for once the header, four meters and the footer are paid for —
/// see `the_real_worst_case_pages_fit_the_tightest_window`. It is also the
/// routine-slot cap, so a full kit is never trimmed.
pub(super) const MAX_SECTION_ROWS: usize = 6;

/// The AFFINITIES box's own cap, tighter than `MAX_SECTION_ROWS`. Measured at
/// 720px (the tightest supported window): a fifth columned box — COMBAT,
/// POTENTIAL, AFFINITIES, SPECIES, ROUTINES, plus the full-width MOVES band —
/// has room for 2 affinity rows; 3 overlaps the footer by ~20px and 4-5
/// escape the frame outright (see the git history of
/// `the_real_worst_case_pages_fit_the_tightest_window` for the measurements).
/// 2 costs nothing shipped: every species Task 5 gave affinities to carries
/// exactly one strength and one weakness. A modded species naming three or
/// more only ever loses rows to the "+N more" note below, never crashes the
/// page.
pub(super) const MAX_AFFINITY_ROWS: usize = 2;

/// The full-width band's own cap. `MAX_SECTION_ROWS` covers the columned
/// boxes, but the band is what actually overflows once a fifth columned box
/// (AFFINITIES) exists: at `MAX_SECTION_ROWS` (6) it overlapped the footer
/// by 2.00px/10.00px/3.33px at 900px/1000px/1080px (720px and every other
/// swept height were fine — the failure is height-band-specific, not
/// universal). 4 clears all of it with margin. Deliberately a separate
/// constant from `MAX_SECTION_ROWS` rather than lowering that one: 6 is also
/// `COMPANION_ROUTINE_SLOT_CAP`, so shrinking `MAX_SECTION_ROWS` would trim
/// a player's full 6-slot routine kit, which is a real regression the band
/// cap doesn't have to cause — nothing shipped has more than 2 moves, so 4
/// trims nothing that exists today, only a mod.
pub(super) const MAX_BAND_ROWS: usize = 4;

/// Trims `rows` to `MAX_SECTION_ROWS`, spending the last line on a count of
/// what was dropped. A silent truncation would read as "that's all of them".
pub(super) fn section_rows(rows: Vec<SectionRow>) -> Vec<SectionRow> {
    section_rows_capped(rows, MAX_SECTION_ROWS)
}

/// `section_rows` at an arbitrary cap, for the one box (AFFINITIES) whose
/// real worst case is narrower than `MAX_SECTION_ROWS` — see
/// `MAX_AFFINITY_ROWS`.
pub(super) fn section_rows_capped(mut rows: Vec<SectionRow>, cap: usize) -> Vec<SectionRow> {
    if rows.len() <= cap {
        return rows;
    }
    let hidden = rows.len() - (cap - 1);
    rows.truncate(cap - 1);
    rows.push(SectionRow::Note(format!("+{hidden} more")));
    rows
}

/// The manifest's boxes. `sections` is index-aligned with the slice passed to
/// `manifest_layout`.
pub(super) struct ManifestLayout {
    pub(super) frame: Rect,
    pub(super) header: Rect,
    pub(super) meters: Vec<Rect>,
    pub(super) sections: Vec<Rect>,
    pub(super) footer: Rect,
}

/// How much of the window the sheet claims. Not the full window: the status
/// banner (see `draw_status_banner`) lives in the bottom strip, and a refusal
/// drawn over the footer would be unreadable.
const FRAME_W_PCT: f32 = 0.92;
const FRAME_H_PCT: f32 = 0.90;

/// Header rows: a name line and a subtitle line. The glyph is drawn to their
/// left at twice the title size and spans both, so it costs no rows of its
/// own — which the 720px budget needs it not to.
const HEADER_ROWS: f32 = 2.0;

pub(super) fn manifest_layout(
    screen_w: f32,
    screen_h: f32,
    meters: usize,
    sections: &[Section],
    m: &Metrics,
) -> ManifestLayout {
    let w = screen_w * FRAME_W_PCT;
    let h = screen_h * FRAME_H_PCT;
    let frame = Rect::new((screen_w - w) / 2.0, (screen_h - h) / 2.0, w, h);

    let inner_x = frame.x + m.pad;
    let inner_w = frame.w - m.pad * 2.0;
    let mut y = frame.y + m.pad;

    let header = Rect::new(inner_x, y, inner_w, m.line_height * HEADER_ROWS);
    y += header.h + m.gap;

    let meter_rects: Vec<Rect> = (0..meters)
        .map(|i| {
            Rect::new(
                inner_x,
                y + i as f32 * bar_row_height(m),
                inner_w,
                bar_row_height(m),
            )
        })
        .collect();
    y += meters as f32 * bar_row_height(m) + m.gap;

    let footer = Rect::new(
        inner_x,
        frame.y + frame.h - m.pad - m.line_height,
        inner_w,
        m.line_height,
    );

    let col_gap = m.pad;
    let col_w = (inner_w - col_gap) / 2.0;
    // Running bottom edge of each column. A box lands under whichever side is
    // currently shorter, so an uneven set of boxes still fills evenly instead
    // of leaving one column short.
    let mut col_y = [y, y];
    // `None` for a full-width band, which can't be placed until every
    // columned box is down and the grid's true bottom is known.
    let mut placed: Vec<Option<Rect>> = Vec::with_capacity(sections.len());

    for section in sections {
        if section.full_width {
            placed.push(None);
            continue;
        }
        let side = usize::from(col_y[0] > col_y[1]);
        let box_h = section_height(section, m);
        placed.push(Some(Rect::new(
            inner_x + side as f32 * (col_w + col_gap),
            col_y[side],
            col_w,
            box_h,
        )));
        col_y[side] += box_h + m.gap;
    }

    let mut band_y = col_y[0].max(col_y[1]);
    for (slot, section) in placed.iter_mut().zip(sections) {
        if slot.is_some() {
            continue;
        }
        let box_h = section_height(section, m);
        *slot = Some(Rect::new(inner_x, band_y, inner_w, box_h));
        band_y += box_h + m.gap;
    }

    ManifestLayout {
        frame,
        header,
        meters: meter_rects,
        sections: placed.into_iter().flatten().collect(),
        footer,
    }
}

/// A stat row's height inside a box — tighter than `m.line_height`, which is
/// tuned for prose the eye reads left to right rather than a column of
/// label/value pairs it scans down.
///
/// Derived from `font_size` rather than from `m.small()`: `small()` is
/// `font_size - 4`, so it closes on the body size as the font grows, and a
/// box would be *relatively taller* at 1440px than at 720px. That inverts
/// which window is the tight one, which is exactly the kind of thing the
/// height sweep exists to catch.
pub(super) fn section_row_h(m: &Metrics) -> f32 {
    m.font_size as f32
}

/// A box's height: its title line, one `section_row_h` per row, and a gap
/// above and below the rows.
fn section_height(section: &Section, m: &Metrics) -> f32 {
    m.line_height + section_row_h(m) * section.rows.len() as f32 + m.gap * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::ui_metrics;

    /// The same window heights `popup_layout`'s tests sweep — the layout's
    /// row budget is font-dependent, so a bug that misses by one line can
    /// hide at one height and bite at the next.
    const WINDOW_HEIGHTS: [f32; 9] = [
        720.0, 768.0, 800.0, 900.0, 1000.0, 1050.0, 1080.0, 1200.0, 1440.0,
    ];
    const WINDOW_WIDTHS: [f32; 3] = [1280.0, 1600.0, 1920.0];

    fn section(title: &'static str, rows: usize, full_width: bool) -> Section {
        Section {
            title,
            rows: (0..rows)
                .map(|i| SectionRow::Stat(format!("label {i}"), format!("{i}")))
                .collect(),
            full_width,
        }
    }

    /// The fullest page a program can produce, at every cap that actually
    /// bounds `program_sections`' output: 3 combat stats, 5 potential lines,
    /// `MAX_AFFINITY_ROWS` affinity lines, 6 species facts,
    /// `COMPANION_ROUTINE_SLOT_CAP` routines, and `MAX_BAND_ROWS` moves.
    /// POTENTIAL and AFFINITIES together are the ordinary case for a tamed
    /// Scrapper, not an edge case — the balance sweep models a mid-grade
    /// party as three of them, and `scrapper.ron` carries both a
    /// `Potential` roll and a non-neutral `damage` affinity.
    ///
    /// Five affinity rows is *not* a layout state and must not be modeled
    /// here: `program_sections` builds the AFFINITIES box through
    /// `section_rows_capped(_, MAX_AFFINITY_ROWS)`, so a five-category
    /// species renders 2 rows (one plus a "+4 more" note), the same as it
    /// renders here. The real worst case this fixture has to defend is
    /// `MAX_AFFINITY_ROWS` + `MAX_BAND_ROWS` together, which is what made
    /// the band overflow the footer in the first place — restoring either
    /// to a pre-cap literal reintroduces a fixture state the renderer can
    /// no longer produce.
    fn worst_case_program() -> Vec<Section> {
        vec![
            section("COMBAT", 3, false),
            section("POTENTIAL", 5, false),
            section("AFFINITIES", MAX_AFFINITY_ROWS, false),
            section("SPECIES", 6, false),
            section("ROUTINES", 6, false),
            section("MOVES", MAX_BAND_ROWS, true),
        ]
    }

    /// The fullest page the player can produce — six columned boxes, no
    /// band. Perks caps at `MAX_SECTION_ROWS` (there are 7 perk types), party
    /// at `MAX_PARTY_SIZE`.
    fn worst_case_player() -> Vec<Section> {
        vec![
            section("COMBAT", 3, false),
            section("PROGRESSION", 4, false),
            section("EQUIPMENT", 3, false),
            section("ROUTINES", 6, false),
            section("PERKS", MAX_SECTION_ROWS, false),
            section("PARTY", 5, false),
        ]
    }

    /// Meter counts: a program shows Integrity and Experience, the player
    /// adds Power and Fatigue.
    const PROGRAM_METERS: usize = 2;
    const PLAYER_METERS: usize = 4;

    /// Boxes are expected to *touch* — stacked meters are laid out
    /// contiguously, and `y + h` of one is exactly the `y` of the next. In f32
    /// that sum can land a fraction of a pixel past the boundary, so the
    /// tolerance here is what separates "adjacent" from "genuinely on top of
    /// each other". A real overlap is tens of pixels, never a rounding tail.
    ///
    /// Raised from 0.5 to 1.0 when AFFINITIES became a fifth columned box:
    /// five stacked boxes accumulate more float error across a
    /// percentage-based frame than four did, and at the shipped 2-row
    /// affinity cap the resulting gap was a 0.67px rounding tail that 0.5
    /// flagged as an overlap. 1.0 still catches a real one — the tightest
    /// remaining margin between any two real boxes is nowhere near a pixel.
    const TOUCH_EPSILON: f32 = 1.0;

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x + TOUCH_EPSILON < b.x + b.w
            && b.x + TOUCH_EPSILON < a.x + a.w
            && a.y + TOUCH_EPSILON < b.y + b.h
            && b.y + TOUCH_EPSILON < a.y + a.h
    }

    fn contains(outer: &Rect, inner: &Rect) -> bool {
        inner.x >= outer.x - 0.5
            && inner.y >= outer.y - 0.5
            && inner.x + inner.w <= outer.x + outer.w + 0.5
            && inner.y + inner.h <= outer.y + outer.h + 0.5
    }

    /// The gate this whole module exists for: the fullest page either subject
    /// can produce has to fit the tightest window, at every window size, with
    /// nothing overlapping and nothing escaping the frame.
    ///
    /// 720px is the binding case — the UI font is 19px there, and the header,
    /// four meters and the footer eat most of the box before a single stat
    /// row is drawn. If this fails, the fix is content, not the assertion:
    /// lower `MAX_SECTION_ROWS`, then merge two of the player's boxes.
    #[test]
    fn the_real_worst_case_pages_fit_the_tightest_window() {
        for window_h in WINDOW_HEIGHTS {
            for window_w in WINDOW_WIDTHS {
                let m = ui_metrics(window_h);
                for (who, sections, meters) in [
                    ("program", worst_case_program(), PROGRAM_METERS),
                    ("player", worst_case_player(), PLAYER_METERS),
                ] {
                    let l = manifest_layout(window_w, window_h, meters, &sections, &m);

                    let mut boxes = vec![l.header];
                    boxes.extend(l.meters.iter().copied());
                    boxes.extend(l.sections.iter().copied());
                    boxes.push(l.footer);

                    for (i, a) in boxes.iter().enumerate() {
                        assert!(
                            contains(&l.frame, a),
                            "the fullest {who} page at {window_w}x{window_h}: box {i} ({a:?}) \
                             escaped the frame ({:?})",
                            l.frame
                        );
                        for (j, b) in boxes.iter().enumerate().skip(i + 1) {
                            assert!(
                                !overlaps(a, b),
                                "the fullest {who} page at {window_w}x{window_h}: boxes {i} \
                                 ({a:?}) and {j} ({b:?}) overlap"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_frame_always_fits_the_window() {
        for window_h in WINDOW_HEIGHTS {
            for window_w in WINDOW_WIDTHS {
                let m = ui_metrics(window_h);
                let sections = worst_case_player();
                let l = manifest_layout(window_w, window_h, PLAYER_METERS, &sections, &m);
                let window = Rect::new(0.0, 0.0, window_w, window_h);
                assert!(
                    contains(&window, &l.frame),
                    "at {window_w}x{window_h} the frame {:?} runs off the window",
                    l.frame
                );
            }
        }
    }

    /// A subject missing a section (a legacy program with no Potential roll)
    /// must not leave a hole — the boxes after it move up into the space.
    #[test]
    fn a_missing_section_closes_the_gap_rather_than_leaving_a_hole() {
        let m = ui_metrics(1080.0);
        let full = vec![
            section("COMBAT", 3, false),
            section("POTENTIAL", 4, false),
            section("ROUTINES", 3, false),
            section("SPECIES", 6, false),
        ];
        let without_potential = vec![
            section("COMBAT", 3, false),
            section("ROUTINES", 3, false),
            section("SPECIES", 6, false),
        ];
        let a = manifest_layout(1600.0, 1080.0, 2, &full, &m);
        let b = manifest_layout(1600.0, 1080.0, 2, &without_potential, &m);

        assert_eq!(a.sections.len(), 4);
        assert_eq!(b.sections.len(), 3);
        assert_eq!(
            b.sections[1].y, a.sections[1].y,
            "dropping a section must not push the survivors down"
        );
        let a_bottom = a.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
        let b_bottom = b.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
        assert!(
            b_bottom <= a_bottom + 0.5,
            "three sections must not occupy more vertical space than four"
        );
    }

    /// Two columns, so the second section sits beside the first rather than
    /// under it.
    #[test]
    fn columned_sections_fill_left_then_right() {
        let m = ui_metrics(1080.0);
        let sections = vec![section("A", 3, false), section("B", 3, false)];
        let l = manifest_layout(1600.0, 1080.0, 2, &sections, &m);
        assert_eq!(l.sections[0].y, l.sections[1].y, "equal boxes share a row");
        assert!(
            l.sections[1].x > l.sections[0].x,
            "the second box goes to the right column"
        );
    }

    /// A full-width band spans both columns and sits below every columned box.
    #[test]
    fn a_full_width_section_spans_both_columns_below_the_grid() {
        let m = ui_metrics(1080.0);
        let sections = vec![
            section("A", 3, false),
            section("B", 3, false),
            section("MOVES", 2, true),
        ];
        let l = manifest_layout(1600.0, 1080.0, 2, &sections, &m);
        let band = l.sections[2];
        assert!(
            band.y >= l.sections[0].y + l.sections[0].h,
            "the band sits below the grid"
        );
        assert!(
            band.w > l.sections[0].w * 1.5,
            "the band spans both columns, not one"
        );
    }

    /// A section longer than the cap is trimmed with a counted note, so a
    /// modded species with twenty moves can't blow the layout out.
    #[test]
    fn section_rows_trims_past_the_cap_and_says_how_many_it_hid() {
        let rows: Vec<SectionRow> = (0..MAX_SECTION_ROWS + 5)
            .map(|i| SectionRow::Note(format!("row {i}")))
            .collect();
        let trimmed = section_rows(rows);
        assert_eq!(trimmed.len(), MAX_SECTION_ROWS);
        let SectionRow::Note(last) = &trimmed[MAX_SECTION_ROWS - 1] else {
            panic!("the trailing row is a note");
        };
        assert_eq!(last, "+6 more");
    }

    #[test]
    fn section_rows_leaves_a_short_list_alone() {
        let rows: Vec<SectionRow> = (0..3)
            .map(|i| SectionRow::Note(format!("row {i}")))
            .collect();
        assert_eq!(section_rows(rows).len(), 3);
    }

    /// A modded species naming all five `AffinityKind` categories must not
    /// blow out the AFFINITIES box — it renders one row plus an honest
    /// count of what's hidden, same as `section_rows` already does for
    /// MOVES and PERKS at `MAX_SECTION_ROWS`.
    #[test]
    fn a_five_category_affinity_list_is_capped_with_an_honest_count() {
        let rows: Vec<SectionRow> = (0..5)
            .map(|i| SectionRow::Stat(format!("Category {i}"), "1.00x".to_string()))
            .collect();
        let trimmed = section_rows_capped(rows, MAX_AFFINITY_ROWS);
        assert_eq!(trimmed.len(), MAX_AFFINITY_ROWS);
        let SectionRow::Note(last) = &trimmed[MAX_AFFINITY_ROWS - 1] else {
            panic!("the trailing row is a note");
        };
        assert_eq!(last, "+4 more");
    }

    /// A modded species naming more moves than `MAX_BAND_ROWS` must not push
    /// the full-width band into the footer — it renders the cap plus an
    /// honest count of what's hidden, same shape as the affinity truncation
    /// above.
    #[test]
    fn a_move_list_past_the_band_cap_is_capped_with_an_honest_count() {
        let rows: Vec<SectionRow> = (0..MAX_BAND_ROWS + 3)
            .map(|i| SectionRow::Stat(format!("Move {i}"), format!("pow {i}")))
            .collect();
        let trimmed = section_rows_capped(rows, MAX_BAND_ROWS);
        assert_eq!(trimmed.len(), MAX_BAND_ROWS);
        let SectionRow::Note(last) = &trimmed[MAX_BAND_ROWS - 1] else {
            panic!("the trailing row is a note");
        };
        assert_eq!(last, "+4 more");
    }
}
