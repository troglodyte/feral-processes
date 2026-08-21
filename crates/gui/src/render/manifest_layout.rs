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
/// 6 is what 720px (the tightest supported window, where the UI font is
/// 19px) has room for, for the box set this constant originally protected —
/// see `the_real_worst_case_pages_fit_the_tightest_window`. Which height
/// actually binds is a property of the *current* box set, not a fixed fact
/// about 720px: `MAX_BAND_ROWS`'s doc records a configuration (a fifth
/// columned box at `MOVES` = 6) where 900/1000/1080px failed while 720px
/// passed. Don't assume 720px is always the worst case — the sweep is what
/// decides it, for whatever `worst_case_program`/`worst_case_player`
/// currently model. 6 is also the routine-slot cap, so a full kit is never
/// trimmed.
pub(super) const MAX_SECTION_ROWS: usize = 6;

/// The AFFINITIES box's own cap, tighter than `MAX_SECTION_ROWS`. Above 2,
/// clearance at the tightest window shrinks fast enough that a mod pushing
/// this constant up is a real layout risk — see
/// `tests::the_real_worst_case_pages_fit_the_tightest_window` for the
/// current measured clearance at every value and every window size; that
/// test is the one place this file lets a pixel figure live, precisely so
/// a doc comment quoting numbers can't go stale the way this one has
/// twice already. 2 costs nothing shipped regardless of the margin: every
/// species Task 5 gave affinities to carries exactly one strength and one
/// weakness. A modded species naming three or more only ever loses rows
/// to the "+N more" note below, never crashes the page.
pub(super) const MAX_AFFINITY_ROWS: usize = 2;

/// The full-width band's own cap, separate from `MAX_SECTION_ROWS` because
/// `MOVES` is the one box a mod can genuinely grow past what any shipped
/// species needs, and `best_column_split`'s exact partition (see its doc)
/// changed how much of that growth the layout can absorb before a
/// columned box's overflow reaches the band — again, see
/// `tests::the_real_worst_case_pages_fit_the_tightest_window` for the
/// current clearance figures rather than a restated copy here. Kept
/// deliberately below `MAX_SECTION_ROWS` even where the exact partition
/// alone would clear today's shipped worst case: this is the only defence
/// against a mod-maximal `MOVES` list regardless of packer, and the
/// headroom has value on its own, at the owner's explicit call. A separate
/// constant rather than lowering `MAX_SECTION_ROWS` itself: 6 is also
/// `COMPANION_ROUTINE_SLOT_CAP`, so shrinking it would trim a player's full
/// 6-slot routine kit — nothing shipped has more than 2 moves, so 4 trims
/// nothing that exists today, only a mod.
pub(super) const MAX_BAND_ROWS: usize = 4;

/// Trims `rows` to `MAX_SECTION_ROWS`, spending the last line on a count of
/// what was dropped. A silent truncation would read as "that's all of them".
pub(super) fn section_rows(rows: Vec<SectionRow>) -> Vec<SectionRow> {
    section_rows_capped(rows, MAX_SECTION_ROWS)
}

/// `section_rows` at an arbitrary cap, for the boxes (AFFINITIES, MOVES)
/// whose real worst case is narrower than `MAX_SECTION_ROWS` — see
/// `MAX_AFFINITY_ROWS` and `MAX_BAND_ROWS`.
pub(super) fn section_rows_capped(mut rows: Vec<SectionRow>, cap: usize) -> Vec<SectionRow> {
    debug_assert!(
        cap >= 1,
        "a zero cap can't reserve a row for the note below"
    );
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
    // `None` for a full-width band, which can't be placed until every
    // columned box is down and the grid's true bottom is known.
    let mut placed: Vec<Option<Rect>> = vec![None; sections.len()];

    // Indices into `sections` of the columned boxes, in emission order —
    // the partition below decides which column each lands in, but not
    // their relative order within a column.
    let columned: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.full_width)
        .map(|(i, _)| i)
        .collect();
    let heights: Vec<f32> = columned
        .iter()
        .map(|&i| section_height(&sections[i], m) + m.gap)
        .collect();
    let side = best_column_split(&heights);

    let mut col_y = [y, y];
    for (slot, &i) in columned.iter().enumerate() {
        let s = side[slot];
        let box_h = heights[slot] - m.gap;
        placed[i] = Some(Rect::new(
            inner_x + s as f32 * (col_w + col_gap),
            col_y[s],
            col_w,
            box_h,
        ));
        col_y[s] += heights[slot];
    }

    let mut band_y = col_y[0].max(col_y[1]);
    for (i, section) in sections.iter().enumerate() {
        if !section.full_width {
            continue;
        }
        let box_h = section_height(section, m);
        placed[i] = Some(Rect::new(inner_x, band_y, inner_w, box_h));
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

/// Which column (0 or 1) each of `heights` (already each including its own
/// trailing gap) lands in, minimising the taller column's total — an exact
/// 2-partition, not the single-pass "assign to whichever side is currently
/// shorter" greedy this replaced. The greedy could land measurably further
/// from balanced than optimal: with `AFFINITIES` making the program page a
/// fifth columned box, it left 19-38px on the table (see the git history
/// around `MAX_BAND_ROWS`), and on the player page — which has no mod-only
/// box to trim, unlike the program page's `MOVES` — that gap was the whole
/// reason `worst_case_player`'s corrected box order failed at 7 of 9 swept
/// heights before this function existed.
///
/// Brute force over every subset is exact and cheap: a manifest page has at
/// most a handful of columned boxes, so `2^n` is never more than a few
/// dozen iterations. Do not replace this with a heuristic (LPT, sorting by
/// descending height then greedy) — measured against this exact case, LPT
/// produced *zero* improvement over the naive order-dependent greedy,
/// because the greedy's failure here isn't about visitation order.
///
/// Ties are broken by the lowest bitmask found in ascending order (`<`, not
/// `<=`, so the first minimum wins), with box 0 (the first emitted) weighted
/// as the *most* significant bit — so among tied partitions, the ascending
/// scan finds the one that keeps the earliest-emitted boxes on the left
/// first, matching the pre-existing "fill left then right" contract
/// `columned_sections_fill_left_then_right` pins for equal-height boxes.
/// Not by iterating any hash-based structure, either way: this repo has
/// shipped a flaky test before from a `HashMap`'s iteration order leaking
/// into behavior (see the species-habitat-lookup fix), and a layout that
/// could shuffle between runs would be that bug again, one layer up.
fn best_column_split(heights: &[f32]) -> Vec<usize> {
    let n = heights.len();
    // The "a handful" in this function's doc is aspirational unless
    // enforced: at n >= 32, `1u32 << n` panics in debug and silently
    // evaluates to 1 in release, which would put every box in the left
    // column. No shipped or moddable page comes remotely close — sections
    // are hardcoded pushes, not something a mod can add to — so this
    // should never trip; it exists to fail loudly if that assumption ever
    // stops holding, rather than silently mis-laying out a page in release.
    debug_assert!(
        n < 32,
        "best_column_split: {n} columned boxes on one page — 1u32 << n panics past 31 in \
         debug and silently misbehaves in release; this function needs a real bitset before \
         a page can have this many boxes"
    );
    let bit = |i: usize| 1u32 << (n - 1 - i);
    let mut best_mask: u32 = 0;
    let mut best_worst = f32::MAX;
    for mask in 0u32..(1u32 << n) {
        let mut total = [0.0f32; 2];
        for (i, h) in heights.iter().enumerate() {
            total[usize::from(mask & bit(i) != 0)] += h;
        }
        let worst = total[0].max(total[1]);
        if worst < best_worst {
            best_worst = worst;
            best_mask = mask;
        }
    }
    (0..n)
        .map(|i| usize::from(best_mask & bit(i) != 0))
        .collect()
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
    const WINDOW_WIDTHS: [f32; 4] = [1280.0, 1600.0, 1920.0, 2560.0];

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
    /// `MAX_AFFINITY_ROWS` affinity lines, 5 species facts (a non-boss
    /// species carrying a `work_resource` — Habitats, Work aptitude, the two
    /// Decompile rows, Growth), 2 work facts in their own WORK box (Speed
    /// and Analysis, split out of SPECIES so it can hold that fifth row
    /// without hitting `MAX_SECTION_ROWS`), `COMPANION_ROUTINE_SLOT_CAP`
    /// routines, and `MAX_BAND_ROWS` moves. POTENTIAL and AFFINITIES
    /// together are the ordinary case for a tamed Scrapper, not an edge
    /// case — the balance sweep models a mid-grade party as three of them,
    /// and `scrapper.ron` carries both a `Potential` roll and a non-neutral
    /// `damage` affinity.
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
            // Damage, Attack, Mitigation, Power. The damage band joined this
            // box with the combat model, and this fixture has to move with
            // `sections_for` — drifting from what the renderer emits is a
            // failure this project has already shipped once.
            section("COMBAT", 4, false),
            section("POTENTIAL", 5, false),
            section("AFFINITIES", MAX_AFFINITY_ROWS, false),
            section("SPECIES", 5, false),
            // Speed, Analysis, Base job, and the post — a tameable program
            // always has a class, and a posted one names the structure it
            // works, so four is the real worst case. A boss has neither and
            // sits at two.
            section("WORK", 4, false),
            // Rings, ceiling and talents. Emitted only for a developed
            // program, which is exactly what a worst case is.
            section("DEVELOPMENT", 3, false),
            // One row per occupied `EquipmentSlot`, so `EquipmentSlot::ALL`
            // is the cap. Any program the player owns can wear gear as of
            // 0.8.0, so a fully kitted companion is the worst case here and
            // not an edge case.
            section("EQUIPMENT", 3, false),
            section("ROUTINES", 6, false),
            section("MOVES", MAX_BAND_ROWS, true),
        ]
    }

    /// The fullest page the player can produce — six columned boxes, no
    /// band. Perks caps at `MAX_SECTION_ROWS` (there are 12 perk types now,
    /// still well past the cap of 6), party at `MAX_PARTY_SIZE`.
    ///
    /// `manifest_layout`'s column packer is an exact 2-partition
    /// (`best_column_split`), so *whether the page fits* no longer depends
    /// on this list's order — only on the set of box heights. The order
    /// below is still kept matching what `manifest::sections_for` +
    /// `manifest::player_sections` actually emit — COMBAT, PROGRESSION,
    /// EQUIPMENT, PERKS, PARTY, then ROUTINES **last**, since
    /// `sections_for` appends it after `player_sections` returns rather
    /// than as its fourth push — because a fixture that silently drops or
    /// reorders a box relative to reality is still the failure mode this
    /// file exists to catch, even though a wrong *order* alone can no
    /// longer hide an overflow the way it could under the old greedy.
    fn worst_case_player() -> Vec<Section> {
        vec![
            // Damage, Attack, Accuracy, Evasion, Mitigation, Power. Sitting
            // exactly on `MAX_SECTION_ROWS` is safe here in a way it would
            // not be for SPECIES: COMBAT's row list is fixed-length, so it
            // cannot grow at runtime and `section_rows` can never trim it
            // to a "+N more" the player reads as "that's all of them".
            // Player-only — a program's COMBAT stays at 4, because the
            // program page's clearance sweep has no room for a fifth row.
            section("COMBAT", MAX_SECTION_ROWS, false),
            section("PROGRESSION", 4, false),
            section("PERKS", MAX_SECTION_ROWS, false),
            section("PARTY", 5, false),
            // Credits, Portal Fragments, difficulty, cycle, contracts —
            // fixed-length like COMBAT, and one row under the cap.
            section("RUN", 5, false),
            section("EQUIPMENT", 3, false),
            section("ROUTINES", 6, false),
        ]
    }

    /// Meter counts: a program shows Integrity and Experience, the player
    /// adds Power.
    const PROGRAM_METERS: usize = 2;
    const PLAYER_METERS: usize = 3;

    /// Boxes are expected to *touch* — stacked meters are laid out
    /// contiguously, and `y + h` of one is exactly the `y` of the next. In f32
    /// that sum can land a fraction of a pixel past the boundary, so the
    /// tolerance here is what separates "adjacent" from "genuinely on top of
    /// each other". A real overlap is tens of pixels, never a rounding tail.
    const TOUCH_EPSILON: f32 = 0.5;

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

    /// The floor this test holds the gap between the tallest content box and
    /// the footer to, on top of "doesn't overlap". Chosen to be meaningful
    /// rather than trivially true: far past `TOUCH_EPSILON` (0.5px) so this
    /// is never satisfied by a rounding tail, comfortably below today's
    /// measured minimums (see the test's own failure output for the current
    /// numbers — a doc comment restating them would only go stale again,
    /// which is exactly what happened three times on this branch) so
    /// legitimate content changes don't trip it by accident, but tight
    /// enough to catch a real regression while there's still room to see it
    /// coming rather than after it's already an overlap.
    const MIN_CLEARANCE_PX: f32 = 10.0;

    /// The gate this whole module exists for: the fullest page either subject
    /// can produce has to fit the tightest window, at every window size, with
    /// nothing overlapping, nothing escaping the frame, and real clearance
    /// left over — not just "doesn't overlap yet". This is the test to cite
    /// for the current clearance figures at any window size: run it (or read
    /// its `MIN_CLEARANCE_PX` failure message) rather than trusting a comment
    /// elsewhere that copied a number out of it.
    ///
    /// 720px is usually the binding case — the UI font is 19px there, and the
    /// header, four meters and the footer eat most of the box before a
    /// single stat row is drawn — but which height actually binds depends on
    /// the current box set, not a fixed fact about 720px specifically. If
    /// this fails, the fix is content, not the assertion: lower
    /// `MAX_SECTION_ROWS` or `MAX_BAND_ROWS`, or merge two of the player's
    /// boxes.
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

                    let tallest_content_bottom =
                        l.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
                    let clearance = l.footer.y - tallest_content_bottom;
                    assert!(
                        clearance >= MIN_CLEARANCE_PX,
                        "the fullest {who} page at {window_w}x{window_h}: only {clearance:.2}px \
                         between the tallest box and the footer, below the {MIN_CLEARANCE_PX}px \
                         floor this test holds"
                    );
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
            section("COMBAT", 4, false),
            section("POTENTIAL", 4, false),
            section("ROUTINES", 3, false),
            section("SPECIES", 5, false),
        ];
        let without_potential = vec![
            section("COMBAT", 4, false),
            section("ROUTINES", 3, false),
            section("SPECIES", 5, false),
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

    /// The partition search itself, isolated from window metrics: an uneven
    /// set of boxes must land at the true minimum taller-column height, not
    /// whatever the old greedy (assign to whichever side is shorter so far)
    /// happened to find.
    #[test]
    fn best_column_split_finds_the_true_minimum_not_a_greedy_approximation() {
        // Hand-verifiable counterexample: the old greedy (assign each box,
        // in order, to whichever side is currently shorter) processes
        // 1, 1, 1, 3 as side0=1, side1=1, side0=2, side1=4 — columns of 2
        // and 4, a taller column of 4. The true optimum groups the three
        // 1s together against the lone 3: columns of 3 and 3, a taller
        // column of only 3. A function that was secretly still the old
        // greedy under a new name would fail this.
        let heights = [1.0, 1.0, 1.0, 3.0];
        let sides = best_column_split(&heights);
        let mut total = [0.0f32; 2];
        for (i, h) in heights.iter().enumerate() {
            total[sides[i]] += h;
        }
        let worst = total[0].max(total[1]);
        assert_eq!(
            worst, 3.0,
            "the true optimum is 3 and 3, not the greedy's 2 and 4"
        );
    }

    /// Ties resolve deterministically and don't depend on iteration over
    /// any hash-based structure — same input, same output, every call.
    #[test]
    fn best_column_split_is_deterministic_across_repeated_calls() {
        let heights = [4.0, 4.0, 4.0, 4.0, 4.0];
        let first = best_column_split(&heights);
        for _ in 0..20 {
            assert_eq!(best_column_split(&heights), first);
        }
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
