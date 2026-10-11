//! Where every box on the manifest screen goes.
//!
//! Kept free of `Painter` for the same reason `popup_layout` is: what fits is
//! settled in whole rows before any of it becomes pixels, and that arithmetic
//! is then testable at every window size without a window.

use crate::paint::Rect;
use crate::text::Metrics;
use feral_processes_engine::RankedStat;

/// One titled box. Built by the draw function and handed here, so a section
/// with no data is simply absent from the slice and the boxes after it move
/// up — rather than the layout having to know which subject it is drawing.
pub(in crate::render) struct Section {
    pub(in crate::render) title: String,
    pub(in crate::render) column: LedgerColumn,
    pub(in crate::render) rows: Vec<SectionRow>,
    /// Keys the box answers, drawn on the title line after the title: the
    /// page's footer has no room left for them. Empty for none.
    pub(in crate::render) hint: String,
}

/// Which of the ledger's three columns a STATS section stacks in.
///
/// Data on the section, so the builders state where a box goes and the draw
/// step never has to know which subject it is drawing. The columns are
/// FIGHT, GROWTH and, by subject, RUN (player) or DUTY (program).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) enum LedgerColumn {
    Fight,
    Growth,
    Duty,
}

impl LedgerColumn {
    pub(in crate::render) fn index(self) -> usize {
        match self {
            LedgerColumn::Fight => 0,
            LedgerColumn::Growth => 1,
            LedgerColumn::Duty => 2,
        }
    }
}

/// `Clone`/`PartialEq`/`Debug` so a test can state the rows a box must draw
/// and compare them whole. A census that reached into `Section` field by
/// field would pass a box that had silently gained a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::render) enum SectionRow {
    /// A label on the left, its value right-aligned against the box's inner
    /// edge.
    Stat(String, String),
    /// One run of free text across the box.
    Note(String),
    /// A `Stat` the ledger also ranks against the roster
    /// (`Game::subject_ranks`). A plain `Stat` where there is no roster to rank against.
    Ranked(String, String, RankedStat),
    /// A `Stat` whose value is something to act on, drawn in
    /// `hud-attention`.
    Alert(String, String),
    /// A line of free text that is something to act on, in `hud-attention`.
    Callout(String),
}

/// The most rows any one box draws. A section is bounded so the worst-case
/// page height is bounded — otherwise a modded species with twenty moves
/// would silently push the footer off the bottom.
///
/// 6 is what 720px (the tightest supported window, where the UI font is
/// 19px) has room for, for the box set this constant originally protected —
/// see `the_stats_face_stays_in_its_frame_and_overlaps_nothing`. Which height
/// actually binds is a property of the *current* box set, not a fixed fact
/// about 720px: `MAX_MOVE_ROWS`'s doc records a configuration (a fifth
/// columned box at `MOVES` = 6) where 900/1000/1080px failed while 720px
/// passed. Don't assume 720px is always the worst case — the sweep is what
/// decides it, for whatever the worst-case fixtures in `manifest::tests`
/// currently model. 6 is also the routine-slot cap, so a full kit is never
/// trimmed.
pub(in crate::render) const MAX_SECTION_ROWS: usize = 6;

/// The AFFINITIES box's own cap, tighter than `MAX_SECTION_ROWS`. Above 2,
/// clearance at the tightest window shrinks fast enough that a mod pushing
/// this constant up is a real layout risk —
/// `manifest::tests::the_stats_face_stays_in_its_frame_and_overlaps_nothing`
/// is what fails if a value no longer fits; no doc comment quotes pixel
/// figures, so none can go stale. 2 costs nothing shipped regardless of the margin: every
/// species Task 5 gave affinities to carries exactly one strength and one
/// weakness. A modded species naming three or more only ever loses rows
/// to the "+N more" note below, never crashes the page.
pub(in crate::render) const MAX_AFFINITY_ROWS: usize = 2;

/// The MOVES box's own cap, separate from `MAX_SECTION_ROWS` because `MOVES`
/// is the one box a mod can genuinely grow past what any shipped species
/// needs, and the ledger's column fit changed how much of that growth the
/// layout can absorb before a section is dropped —
/// `manifest::tests::the_stats_face_stays_in_its_frame_and_overlaps_nothing`
/// is what fails if it no longer fits. Kept deliberately below
/// `MAX_SECTION_ROWS` even where `fit_column` alone would clear today's
/// shipped worst case: this is the only defence against a mod-maximal
/// `MOVES` list, and the headroom has value on its own,
/// at the owner's explicit call. A separate constant rather than lowering
/// `MAX_SECTION_ROWS` itself: at 6 rows the ROUTINES box already spends its
/// last line on a "+N more" note for a kit near
/// `COMPANION_ROUTINE_SLOT_CAP`, and lowering the shared cap would take that
/// from every box — nothing shipped has more than 2 moves, so this trims
/// nothing that exists today, only a mod.
///
/// **Lowered from 4 to 3 to pay for the need rows in WORK.** That is the
/// trade this page always makes: the program page has the least clearance in
/// the renderer, so a new reading is bought out of an existing cap rather
/// than out of the frame. MOVES was the one place with a row nothing shipped
/// uses.
///
/// **Was `MAX_MOVE_ROWS`, and it is not a band any more.** MOVES became a
/// columned box to pay for MEMORIES: a full-width band and two columned boxes
/// side by side occupy the same grid row, so demoting it bought a whole new
/// box for nothing — the program page held its measured clearance exactly.
/// The name moved with the shape rather than being left describing one the
/// page no longer has. The **player** page keeps the renderer's only band,
/// EQUIPMENT, which is capped by its own slot count and not by this.
/// **Lowered again, from 3 to 1, to pay for POTENTIAL's two build rolls.**
/// That is the same trade one paragraph up, made a second time and at a real
/// cost this time: a species with two moves names only the first, and its
/// title carried the count — a "+1 more" *row* would
/// have spent the box's only line and named neither, which it did until the
/// count moved up. The WORK trade cost nothing shipped. Measured
/// rather than guessed — `MAX_POTENTIAL_ROWS` at 7 clears the 10px floor at
/// **1** and does not at 2, and the alternative on the table was folding the
/// two build rolls onto one line to keep MOVES at 2. The owner took the cost
/// on MOVES so both rolls could be named in full on their own rows.
pub(in crate::render) const MAX_MOVE_ROWS: usize = 1;

/// The NEEDS box's own cap, tighter than `MAX_SECTION_ROWS` for
/// `MAX_AFFINITY_ROWS`' reason: the program page has the least clearance of
/// anything in the renderer, and this box was added to a layout that was
/// already close. `assets/needs/` is data, so a mod can grow it without
/// bound; the shipped catalogue is two, and a third costs nothing. Above
/// that a row is traded for the "+N more" note rather than for a page that
/// escapes the frame —
/// `tests::the_stats_face_stays_in_its_frame_and_overlaps_nothing` is what
/// fails if a value no longer fits.
pub(in crate::render) const MAX_NEED_ROWS: usize = 2;

/// POTENTIAL's own cap, and the one per-box constant here that is **wider**
/// than `MAX_SECTION_ROWS` rather than narrower. Seven because the box lists
/// one row per roll plus the overall tier, and `components::Potential`'s two
/// build rolls are the sixth and seventh.
///
/// Held here rather than by raising `MAX_SECTION_ROWS`, which would hand the
/// extra row to every box — and the ROUTINES box's sixth line is already
/// spent on its "+N more" note, so raising the shared cap would change what
/// a full kit shows for a box that does not need the room. What paid for it
/// is `MAX_MOVE_ROWS`; see that constant.
pub(in crate::render) const MAX_POTENTIAL_ROWS: usize = 7;

/// The player's RUN column holds seven readings (Credits, Portal Fragments,
/// Contracts, Cargo carried, Difficulty, Cycle, Position), one more than
/// `MAX_SECTION_ROWS`, which would swallow the last into a `+N more`.
pub(in crate::render) const MAX_RUN_ROWS: usize = 7;

/// WORK's cap: Speed, Analysis, Base job, Work aptitude, the post, and
/// `MAX_NEED_ROWS` needs with their own `+N more` note. Wider than
/// `MAX_SECTION_ROWS` for `MAX_POTENTIAL_ROWS`' reason: the ledger's DUTY
/// column has the height for it, and a cap of 6 would put the `+N more` note
/// where a reading used to be.
pub(in crate::render) const MAX_WORK_ROWS: usize = 8;

/// Trims `rows` to `MAX_SECTION_ROWS`, spending the last line on a count of
/// what was dropped. A silent truncation would read as "that's all of them".
pub(in crate::render) fn section_rows(rows: Vec<SectionRow>) -> Vec<SectionRow> {
    section_rows_capped(rows, MAX_SECTION_ROWS)
}

/// `section_rows` at an arbitrary cap, for the boxes (AFFINITIES, MOVES)
/// whose real worst case is narrower than `MAX_SECTION_ROWS` — see
/// `MAX_AFFINITY_ROWS` and `MAX_MOVE_ROWS`.
pub(in crate::render) fn section_rows_capped(
    mut rows: Vec<SectionRow>,
    cap: usize,
) -> Vec<SectionRow> {
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

/// How much of the window the sheet claims. Not the full window: the status
/// banner (see `draw_status_banner`) lives in the bottom strip, and a refusal
/// drawn over the footer would be unreadable.
const FRAME_W_PCT: f32 = 0.92;
const FRAME_H_PCT: f32 = 0.90;

/// Header rows: a name line and a subtitle line. The glyph is drawn to their
/// left at twice the title size and spans both, so it costs no rows of its
/// own — which the 720px budget needs it not to.
const HEADER_ROWS: f32 = 2.0;

/// The sheet's frame, centred in the window.
pub(in crate::render) fn frame_rect(screen_w: f32, screen_h: f32) -> Rect {
    let w = screen_w * FRAME_W_PCT;
    let h = screen_h * FRAME_H_PCT;
    Rect::new((screen_w - w) / 2.0, (screen_h - h) / 2.0, w, h)
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
pub(in crate::render) fn section_row_h(m: &Metrics) -> f32 {
    m.font_size as f32
}

/// The full roster rail's width, in cells: shortcut, glyph, name, `Lv N`
/// and the `!` mark.
const RAIL_FULL_CELLS: f32 = 30.0;
/// The collapsed rail's width, in cells: the glyph and the `!` mark.
const RAIL_COLLAPSED_CELLS: f32 = 4.0;
/// The narrowest a ledger column may be for the rail to draw full. Rows wrap
/// now, so this is a readability floor rather than the width a label, value
/// and compare cell need side by side.
const LEDGER_MIN_CELLS: f32 = 22.0;
/// The narrowest frame, in pixels, that draws the full rail. In pixels and
/// not cells because the UI font scales with window *height*: 1280x720 and
/// 1440x900 leave the same number of cells for the columns, so only the
/// width tells them apart. 1440 wide (a 1325px frame) is the first window
/// the full rail is for.
pub(in crate::render) const RAIL_FULL_MIN_FRAME_W: f32 = 1300.0;
const LEDGER_COLUMNS: usize = 3;

/// Which kind of page the frame holds. Only STATS has a vitals line and may
/// draw the rail full; SCORE, SOCIAL, TALK and the dossier are `Other`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) enum Face {
    Stats,
    Other,
}

/// The roster rail down the sheet's left edge. Its divider is drawn on
/// `rect`'s right edge.
pub(in crate::render) struct RailGeom {
    /// The 30-cell rail with names, or the 4-cell glyph column.
    pub(in crate::render) full: bool,
    pub(in crate::render) rect: Rect,
    /// The `ROSTER` title row. A collapsed rail has no room for one.
    pub(in crate::render) title: Option<Rect>,
    /// Where the subject rows, the party rule and the `↑`/`↓ N more` rows go.
    pub(in crate::render) rows: Rect,
    pub(in crate::render) row_h: f32,
}

impl RailGeom {
    /// How many rail rows `rows` holds, the overflow rows included.
    pub(in crate::render) fn fits(&self) -> usize {
        (self.rows.h / self.row_h).floor().max(0.0) as usize
    }
}

/// Where every part of the redesigned sheet goes. Pure over `Metrics` and
/// the caller's measured `char_w` (see `hud::layout` for why that is a
/// parameter), so the census tests run without a window.
pub(in crate::render) struct ManifestLayout {
    /// `None` for a wild subject, which is not on the roster.
    pub(in crate::render) rail: Option<RailGeom>,
    /// The summary band: name and tier, tags, face tabs and call to act.
    pub(in crate::render) band: Rect,
    /// The vitals line, STATS only. A divider rule sits a `gap` above and
    /// below it.
    pub(in crate::render) vitals: Option<Rect>,
    /// FIGHT, GROWTH and DUTY/RUN: equal widths, `pad × 2` apart.
    pub(in crate::render) ledger: [Rect; LEDGER_COLUMNS],
    /// The full-width bands below the ledger (PARTY, LATELY, TOTAL). Placed
    /// before the ledger is given its height, so the ledger is what yields.
    pub(in crate::render) bands: Rect,
    /// A divider rule sits `gap / 2` above it.
    pub(in crate::render) keybar: Rect,
}

impl ManifestLayout {
    /// The ledger's three columns as one rectangle, for the faces that lay
    /// their body out by their own rule.
    pub(in crate::render) fn body(&self) -> Rect {
        let [first, _, last] = &self.ledger;
        Rect::new(first.x, first.y, last.x + last.w - first.x, first.h)
    }

    /// `rail_rows` is how many rows the rail would draw (subjects plus the
    /// party rule); zero means a wild subject and no rail. `band_rows` is the
    /// bands' row count under their one subhead; zero means no bands.
    pub(in crate::render) fn new(
        frame: Rect,
        m: &Metrics,
        char_w: f32,
        face: Face,
        rail_rows: usize,
        band_rows: usize,
    ) -> ManifestLayout {
        let inner = Rect::new(
            frame.x + m.pad,
            frame.y + m.pad,
            frame.w - m.pad * 2.0,
            frame.h - m.pad * 2.0,
        );
        let keybar = Rect::new(
            inner.x,
            inner.y + inner.h - m.line_height,
            inner.w,
            m.line_height,
        );
        let body_bottom = keybar.y - m.gap;
        let row_h = section_row_h(m);

        let full = face == Face::Stats
            && frame.w >= RAIL_FULL_MIN_FRAME_W
            && ledger_col_w(inner.w - RAIL_FULL_CELLS * char_w - m.pad, m)
                >= LEDGER_MIN_CELLS * char_w;
        let rail = (rail_rows > 0).then(|| {
            let cells = if full {
                RAIL_FULL_CELLS
            } else {
                RAIL_COLLAPSED_CELLS
            };
            let rect = Rect::new(inner.x, inner.y, cells * char_w, body_bottom - inner.y);
            let title = full.then(|| Rect::new(rect.x, rect.y, rect.w, m.line_height));
            let rows_y = title.map_or(rect.y, |t| t.y + t.h);
            RailGeom {
                full,
                rect,
                title,
                rows: Rect::new(rect.x, rows_y, rect.w, body_bottom - rows_y),
                row_h,
            }
        });

        let main_x = rail
            .as_ref()
            .map_or(inner.x, |r| r.rect.x + r.rect.w + m.pad);
        let main_w = inner.x + inner.w - main_x;
        let band = Rect::new(main_x, inner.y, main_w, m.line_height * HEADER_ROWS);
        let mut y = band.y + band.h + m.gap;
        let vitals = (face == Face::Stats).then(|| {
            let r = Rect::new(main_x, y, main_w, m.line_height);
            y += r.h + m.gap;
            r
        });

        let bands_h = if band_rows == 0 {
            0.0
        } else {
            m.line_height + band_rows as f32 * row_h
        };
        let bands = Rect::new(main_x, body_bottom - bands_h, main_w, bands_h);
        let ledger_bottom = if band_rows == 0 {
            body_bottom
        } else {
            bands.y - m.gap
        };
        let ledger_h = (ledger_bottom - y).max(0.0);
        let col_w = ledger_col_w(main_w, m);
        let ledger = std::array::from_fn(|i| {
            Rect::new(
                main_x + i as f32 * (col_w + ledger_gap(m)),
                y,
                col_w,
                ledger_h,
            )
        });

        ManifestLayout {
            rail,
            band,
            vitals,
            ledger,
            bands,
            keybar,
        }
    }
}

/// The space between two ledger columns.
pub(in crate::render) fn ledger_gap(m: &Metrics) -> f32 {
    m.pad * 2.0
}

/// One ledger column's width when the three share `main_w`.
fn ledger_col_w(main_w: f32, m: &Metrics) -> f32 {
    (main_w - ledger_gap(m) * (LEDGER_COLUMNS - 1) as f32) / LEDGER_COLUMNS as f32
}

/// Which slice of the rail is drawn, and how many rows each overflow row
/// reports.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::render) struct RailWindow {
    pub(in crate::render) start: usize,
    pub(in crate::render) end: usize,
    /// Rows cut above the window, reported by a `↑ N more` row when not zero.
    pub(in crate::render) above: usize,
    /// Rows cut below the window, reported by a `↓ N more` row when not zero.
    pub(in crate::render) below: usize,
}

/// The rows `start..end` of a `len`-row rail that fit `fits` rows with the
/// `selected` row among them.
///
/// **`fitting_rows`' rule:** an overflow row costs one of the `fits` rows,
/// and is reserved only when something is cut. The window is the topmost one
/// holding the selection, so moving down the rail scrolls it only once the
/// selection reaches its last row. With fewer than three rows there is no
/// room for both overflow rows and the selection, so the selection alone is
/// returned and no overflow row is promised: `above` and `below` are zero.
pub(in crate::render) fn rail_window(len: usize, selected: usize, fits: usize) -> RailWindow {
    let window = |start: usize, end: usize| RailWindow {
        start,
        end,
        above: start,
        below: len - end,
    };
    if len <= fits {
        return window(0, len);
    }
    let selected = selected.min(len - 1);
    if fits < 3 {
        return RailWindow {
            start: selected,
            end: selected + 1,
            above: 0,
            below: 0,
        };
    }
    // One row goes to `↓ N more`.
    if selected < fits - 1 {
        return window(0, fits - 1);
    }
    // One row goes to `↑ N more`.
    if selected >= len - (fits - 1) {
        return window(len - (fits - 1), len);
    }
    // Both overflow rows, and the selection on the window's last row.
    window(selected + 3 - fits, selected + 1)
}

/// A ledger section's height: its subhead (and the rule under it), its
/// rows, and a gap before the next section.
pub(in crate::render) fn ledger_section_h(rows: usize, m: &Metrics) -> f32 {
    m.line_height + rows as f32 * section_row_h(m) + m.gap
}

/// What one ledger column keeps.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::render) struct ColumnFit {
    /// The leading sections drawn.
    pub(in crate::render) kept: usize,
    /// The trailing sections cut, reported in one `+N more` row when not
    /// zero.
    pub(in crate::render) dropped: usize,
}

/// How many of a column's sections fit `height`, given each one's row count
/// after its own cap (`section_rows_capped`).
///
/// Whole trailing sections go, never a section's tail, so a section either
/// reads complete or is counted. **`fitting_rows`' rule:** the `+N more` row
/// is reserved only when something is cut. The bands below the ledger are
/// not this function's business: `ManifestLayout` places them first.
pub(in crate::render) fn fit_column(rows: &[usize], height: f32, m: &Metrics) -> ColumnFit {
    let total: f32 = rows.iter().map(|&r| ledger_section_h(r, m)).sum();
    if total <= height {
        return ColumnFit {
            kept: rows.len(),
            dropped: 0,
        };
    }
    let budget = height - section_row_h(m);
    let mut used = 0.0;
    let mut kept = 0;
    for &r in rows {
        let h = ledger_section_h(r, m);
        if used + h > budget {
            break;
        }
        used += h;
        kept += 1;
    }
    ColumnFit {
        kept,
        dropped: rows.len() - kept,
    }
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

    #[test]
    fn the_frame_always_fits_the_window() {
        for window_h in WINDOW_HEIGHTS {
            for window_w in WINDOW_WIDTHS {
                let frame = frame_rect(window_w, window_h);
                let window = Rect::new(0.0, 0.0, window_w, window_h);
                assert!(
                    contains(&window, &frame),
                    "at {window_w}x{window_h} the frame {frame:?} runs off the window",
                );
            }
        }
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

    /// A modded species naming more moves than `MAX_MOVE_ROWS` must not push
    /// the MOVES box into the footer — it renders the cap plus an honest
    /// count of what's hidden, same shape as the affinity truncation above.
    #[test]
    fn a_move_list_past_the_move_cap_is_capped_with_an_honest_count() {
        let rows: Vec<SectionRow> = (0..MAX_MOVE_ROWS + 3)
            .map(|i| SectionRow::Stat(format!("Move {i}"), format!("pow {i}")))
            .collect();
        let trimmed = section_rows_capped(rows, MAX_MOVE_ROWS);
        assert_eq!(trimmed.len(), MAX_MOVE_ROWS);
        let SectionRow::Note(last) = &trimmed[MAX_MOVE_ROWS - 1] else {
            panic!("the trailing row is a note");
        };
        assert_eq!(last, "+4 more");
    }

    /// The UI face's measured advance at `m`'s body size, as the drawers will
    /// pass it.
    fn char_w(m: &Metrics) -> f32 {
        crate::paint::with_painter(|p| p.measure_ui_advance("M", m.font_size)).0
    }

    /// The tallest bands a sheet carries: the PARTY band's column header,
    /// your row and a full party.
    const WORST_BAND_ROWS: usize = 2 + feral_processes_engine::tuning::MAX_PARTY_SIZE;

    /// Every swept window, with its metrics and measured advance.
    fn sweep() -> Vec<(f32, f32, Metrics, f32)> {
        let mut out = Vec::new();
        for h in WINDOW_HEIGHTS {
            let m = ui_metrics(h);
            let cw = char_w(&m);
            for w in WINDOW_WIDTHS {
                out.push((w, h, ui_metrics(h), cw));
            }
        }
        out
    }

    fn sheet(w: f32, h: f32, m: &Metrics, cw: f32, face: Face) -> ManifestLayout {
        ManifestLayout::new(frame_rect(w, h), m, cw, face, 41, WORST_BAND_ROWS)
    }

    #[test]
    fn the_rail_is_full_from_1440_wide_and_never_under_a_readable_column() {
        let min = LEDGER_MIN_CELLS;
        let (mut full, mut collapsed) = (0, 0);
        for (w, h, m, cw) in sweep() {
            let l = sheet(w, h, &m, cw, Face::Stats);
            let rail = l.rail.as_ref().expect("an owned subject has a rail");
            let wide = frame_rect(w, h).w >= RAIL_FULL_MIN_FRAME_W;
            assert!(
                wide || !rail.full,
                "{w}x{h}: a narrow frame has no full rail"
            );
            if wide && !rail.full {
                // Only the column floor may refuse a wide frame.
                let swing = (RAIL_FULL_CELLS - RAIL_COLLAPSED_CELLS) * cw / 3.0;
                assert!(
                    l.ledger[0].w - swing < min * cw,
                    "{w}x{h}: collapsed though a full rail leaves {:.1}-cell columns",
                    (l.ledger[0].w - swing) / cw
                );
            }
            if rail.full {
                full += 1;
                assert!(rail.title.is_some(), "{w}x{h}: a full rail is titled");
                assert!(
                    l.ledger[0].w >= min * cw,
                    "{w}x{h}: full rail but a {:.1}-cell column",
                    l.ledger[0].w / cw
                );
            } else {
                collapsed += 1;
            }
            let other = sheet(w, h, &m, cw, Face::Other);
            assert!(
                !other.rail.unwrap().full,
                "{w}x{h}: only STATS has a full rail"
            );
        }
        assert!(full > 0 && collapsed > 0, "the sweep must see both forms");
    }

    /// The two sizes the user named: 1440x900 shows the name rail, 1280x720
    /// keeps the glyph column.
    #[test]
    fn the_rail_is_full_at_1440x900_and_collapsed_at_1280x720() {
        for ((w, h), want) in [((1440.0, 900.0), true), ((1280.0, 720.0), false)] {
            let m = ui_metrics(h);
            let l = sheet(w, h, &m, char_w(&m), Face::Stats);
            assert_eq!(l.rail.unwrap().full, want, "{w}x{h}");
        }
    }

    #[test]
    fn a_wild_subject_gets_no_rail_and_the_ledger_starts_at_the_frame() {
        let m = ui_metrics(900.0);
        let frame = frame_rect(1440.0, 900.0);
        let l = ManifestLayout::new(frame, &m, char_w(&m), Face::Stats, 0, 2);
        assert!(l.rail.is_none());
        assert_eq!(l.ledger[0].x, frame.x + m.pad);
    }

    #[test]
    fn the_ledger_columns_are_equal_and_two_pads_apart() {
        for (w, h, m, cw) in sweep() {
            for face in [Face::Stats, Face::Other] {
                let l = sheet(w, h, &m, cw, face);
                for pair in l.ledger.windows(2) {
                    assert!((pair[0].w - pair[1].w).abs() < 0.01, "{w}x{h}: unequal");
                    let gap = pair[1].x - (pair[0].x + pair[0].w);
                    assert!((gap - m.pad * 2.0).abs() < 0.01, "{w}x{h}: gap {gap}");
                    assert_eq!((pair[0].y, pair[0].h), (pair[1].y, pair[1].h));
                }
            }
        }
    }

    #[test]
    fn every_part_of_the_sheet_sits_inside_the_frame_without_overlap() {
        for (w, h, m, cw) in sweep() {
            for face in [Face::Stats, Face::Other] {
                let l = sheet(w, h, &m, cw, face);
                let frame = frame_rect(w, h);
                let rail = l.rail.as_ref().unwrap();
                let mut parts = vec![rail.rect, l.band, l.bands, l.keybar];
                parts.extend(l.ledger);
                parts.extend(l.vitals);
                for (i, a) in parts.iter().enumerate() {
                    assert!(
                        contains(&frame, a),
                        "{w}x{h} {face:?}: {a:?} leaves the frame"
                    );
                    for b in &parts[i + 1..] {
                        assert!(!overlaps(a, b), "{w}x{h} {face:?}: {a:?} overlaps {b:?}");
                    }
                }
                // The bands are placed first; what yields is the ledger,
                // and it still holds a working column at every swept size.
                assert!(
                    l.ledger[0].h >= ledger_section_h(MAX_SECTION_ROWS, &m),
                    "{w}x{h} {face:?}: a {:.0}px ledger holds not one full section",
                    l.ledger[0].h
                );
                assert_eq!(l.vitals.is_some(), face == Face::Stats);
                assert!(contains(&rail.rect, &rail.rows));
            }
        }
    }

    #[test]
    fn the_rail_window_keeps_forty_subjects_selection_in_view() {
        const SUBJECTS: usize = 40;
        for (w, h, m, cw) in sweep() {
            let fits = sheet(w, h, &m, cw, Face::Stats).rail.unwrap().fits();
            for selected in 0..SUBJECTS {
                let win = rail_window(SUBJECTS, selected, fits);
                assert!(
                    (win.start..win.end).contains(&selected),
                    "{w}x{h}: {selected} outside {win:?}"
                );
                let drawn =
                    win.end - win.start + usize::from(win.above > 0) + usize::from(win.below > 0);
                assert!(drawn <= fits, "{w}x{h}: {drawn} rows into {fits}");
                assert_eq!(win.above, win.start);
                assert_eq!(win.below, SUBJECTS - win.end);
            }
        }
    }

    #[test]
    fn a_rail_that_fits_shows_everything_and_reports_nothing() {
        let all = RailWindow {
            start: 0,
            end: 5,
            above: 0,
            below: 0,
        };
        assert_eq!(rail_window(5, 4, 5), all);
        assert_eq!(rail_window(0, 0, 5).end, 0);
    }

    #[test]
    fn the_rail_window_scrolls_only_once_the_selection_reaches_its_edge() {
        // Ten rows into five: the top window gives one row to `↓ 6 more`.
        assert_eq!(
            rail_window(10, 3, 5),
            RailWindow {
                start: 0,
                end: 4,
                above: 0,
                below: 6
            }
        );
        // The selection one past it: both overflow rows, selection last.
        assert_eq!(
            rail_window(10, 4, 5),
            RailWindow {
                start: 2,
                end: 5,
                above: 2,
                below: 5
            }
        );
        // Near the end: the bottom window gives one row to `↑ 6 more`.
        assert_eq!(
            rail_window(10, 7, 5),
            RailWindow {
                start: 6,
                end: 10,
                above: 6,
                below: 0
            }
        );
    }

    #[test]
    fn too_short_a_rail_still_shows_the_selection() {
        assert_eq!(rail_window(10, 6, 2).start, 6);
        for fits in 0..3 {
            let win = rail_window(10, 6, fits);
            assert_eq!((win.above, win.below), (0, 0), "fits {fits}");
        }
        assert_eq!(rail_window(10, 6, 0).end, 7);
        assert_eq!(rail_window(10, 99, 1).start, 9);
    }

    #[test]
    fn a_column_that_fits_keeps_every_section() {
        let m = ui_metrics(900.0);
        let rows = [6, 4, 2];
        let total: f32 = rows.iter().map(|&r| ledger_section_h(r, &m)).sum();
        assert_eq!(
            fit_column(&rows, total, &m),
            ColumnFit {
                kept: 3,
                dropped: 0
            }
        );
    }

    #[test]
    fn a_column_too_tall_drops_whole_trailing_sections_and_counts_them() {
        let m = ui_metrics(900.0);
        let rows = [6, 4, 2, 1];
        let first_two = ledger_section_h(6, &m) + ledger_section_h(4, &m);
        // Room for two sections and the `+N more` row, not three.
        let fit = fit_column(&rows, first_two + section_row_h(&m), &m);
        assert_eq!(
            fit,
            ColumnFit {
                kept: 2,
                dropped: 2
            }
        );
        // Exactly the first three plus not quite the `+N more` row: the
        // reserved row costs the third section.
        let first_three = first_two + ledger_section_h(2, &m);
        let fit = fit_column(&rows, first_three + section_row_h(&m) - 1.0, &m);
        assert_eq!(
            fit,
            ColumnFit {
                kept: 2,
                dropped: 2
            }
        );
    }
}
