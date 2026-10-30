use super::*;
use crate::paint::with_painter;
use crate::text::ui_metrics;
use feral_processes_engine::duties::Duty;
use feral_processes_engine::views::WorkColumnKey;

fn work_row(label: &str, doing: &str, section: WorkSection, cells: [bool; 4]) -> WorkRow {
    let mut program = super::view(1, 1, 1);
    program.label = label.to_string();
    program.is_structure = false;
    program.is_tamed = true;
    WorkRow {
        program,
        section,
        role: Some(if section == WorkSection::Staff {
            ProgramRole::Staff
        } else {
            ProgramRole::InParty
        }),
        doing: doing.to_string(),
        cells: cells.to_vec(),
        rank: 0,
    }
}

fn columns(unworked: [usize; 4]) -> Vec<WorkColumn> {
    Duty::ALL
        .iter()
        .zip(unworked)
        .map(|(&duty, unworked)| WorkColumn {
            key: WorkColumnKey::Duty(duty),
            label: duty.label(),
            unworked,
        })
        .collect()
}

fn table(rows: Vec<WorkRow>) -> WorkTable {
    WorkTable {
        columns: columns([0, 0, 2, 0]),
        on_shift: rows
            .iter()
            .filter(|r| r.section == WorkSection::Staff)
            .count(),
        jobs: rows.len(),
        unworked_total: 2,
        rows,
    }
}

/// The widest row the shipped content can produce: the longest species
/// name at a high level with a zone tag, and the longest activity —
/// "guarding the Contract Broker" over the longest structure name in
/// `assets/structures/`.
fn widest_row() -> WorkRow {
    work_row(
        "Gold Sub-Process Lv18 [z9]",
        "guarding the Contract Broker",
        WorkSection::Staff,
        [true, true, false, true],
    )
}

/// Nothing clamps a popup row horizontally, so a table row wider than the
/// Base Staff popup's body runs off its right edge and takes a duty
/// column with it.
///
/// **Both budgets, because only one of them discriminates.** In pixels
/// the widest row clears the reference geometry either way, so that half
/// would pass just as happily with a column dropped — it is here for
/// `no_roster_row_overflows_its_popup`'s reason, to catch the day
/// `ROW_WRAP_COLUMNS` stops being the right budget.
///
/// With the Drop Trooper column drawn, since a base with a terminal is the
/// widest the table gets.
#[test]
fn the_widest_base_staff_row_stays_inside_the_content_rect() {
    let mut row = widest_row();
    row.cells.push(true);
    let mut t = table(vec![row]);
    t.columns.push(WorkColumn {
        key: WorkColumnKey::DropTrooper,
        label: "POD",
        unworked: 0,
    });
    for row in base_staff_menu_rows(&t, &[], 0, Some(0)) {
        let text = match &row {
            Row::Text(text) | Row::TextColored(text, _) => text.clone(),
            Row::Item { text, .. } => text.clone(),
        };
        assert!(
            text.chars().count() <= ROW_WRAP_COLUMNS,
            "a {} char Base Staff row runs past the {ROW_WRAP_COLUMNS} \
             column body: {text:?}",
            text.chars().count()
        );
    }
    with_painter(|p| {
        let rows = base_staff_menu_rows(&t, &[], 0, Some(0));
        for (w, h) in [(1280.0, 720.0), (1440.0, 900.0), (1920.0, 1080.0)] {
            let m = ui_metrics(h);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let content = crate::render::base_report::content_rect_at(w, h, char_w, &m);
            let room = content.w - m.pad * 2.0;
            for row in &rows {
                let text = match row {
                    Row::Text(text) | Row::TextColored(text, _) => text.clone(),
                    Row::Item { text, .. } => format!("     {text}"),
                };
                let drawn = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    drawn <= room,
                    "the widest Base Staff row overflows the content rect by \
                     {:.0}px at {w}x{h} ({drawn:.0} into {room:.0}):\n{text}",
                    drawn - room
                );
            }
        }
    });
}

/// A checked cell reads `[x]`, an unchecked one `[ ]` — except under the
/// column cursor, on the selected row alone, where it reads `<x>`/`< >`
/// instead. No other row's cell changes shape.
#[test]
fn the_cursor_marks_exactly_the_selected_cell() {
    let t = table(vec![
        work_row("A", "idle", WorkSection::Staff, [true, false, true, true]),
        work_row("B", "idle", WorkSection::Staff, [true, false, true, true]),
    ]);
    let rows = base_staff_menu_rows(&t, &[], 0, Some(1));
    let text = |i: usize| match &rows[i + 2] {
        // +2: the header counts line and the column header sit above row 0.
        Row::Item { text, .. } => text.clone(),
        _ => panic!("expected an item row"),
    };
    assert!(
        text(0).contains("< >"),
        "row 0's Guard cell is off and under the cursor: {}",
        text(0)
    );
    assert!(
        !text(1).contains("< >") && !text(1).contains("<x>"),
        "row 1 is not selected, so its Guard cell stays [ ]: {}",
        text(1)
    );
}

/// With the base report's cursor on its rail the table has no column
/// cursor, so no cell on any row changes shape.
#[test]
fn no_column_means_no_cell_cursor() {
    let t = table(vec![work_row(
        "A",
        "idle",
        WorkSection::Staff,
        [true, false, true, true],
    )]);
    let rows = base_staff_menu_rows(&t, &[], 0, None);
    let Row::Item { text, .. } = &rows[2] else {
        panic!("expected an item row");
    };
    assert!(!text.contains('<'), "{text}");
}

/// A duty column's header names its unworked count and nothing when
/// there is none — `DIG` in `columns` above is given 2.
#[test]
fn a_duty_columns_header_shows_its_unworked_count() {
    let t = table(vec![widest_row()]);
    let header = match &base_staff_menu_rows(&t, &[], 0, Some(0))[1] {
        Row::Text(s) => s.clone(),
        _ => panic!("expected the column header"),
    };
    assert!(header.contains("DIG 2!"), "{header}");
    assert!(
        header.contains("OPERATE") && !header.contains("OPERATE 0"),
        "an unworked count of zero draws nothing: {header}"
    );
}

/// The away section reads dim rather than tier-coloured — it is still
/// editable, and a tier colour on an away row would read as "cannot be
/// changed" the way it does nowhere else on this screen.
#[test]
fn away_rows_follow_a_divider_and_read_dim() {
    let t = table(vec![
        work_row("Staffer", "idle", WorkSection::Staff, [true; 4]),
        work_row("Traveller", "in party", WorkSection::Away, [true; 4]),
    ]);
    let rows = base_staff_menu_rows(&t, &[], 0, Some(0));
    let dividers = rows
        .iter()
        .filter(|r| matches!(r, Row::Text(s) if s.contains("away")))
        .count();
    assert_eq!(dividers, 1, "exactly one divider between the two sections");

    let away_color = rows
        .iter()
        .find_map(|r| match r {
            Row::Item { text, color, .. } if text.contains("Traveller") => Some(*color),
            _ => None,
        })
        .expect("the away row is an item row");
    assert_eq!(away_color, TEXT_DIM);
}

/// **The height finding.** `tuning::ROSTER_HARD_CAP` (200) is a refusal
/// ceiling, not a size anyone plays with — a developed base's roster is
/// realistically a few dozen at most. Unlike the character wizard, this
/// screen does not promise "no scroll" — it is a trade/deploy-shaped
/// list, and `draw_rows_in` pages it in the hub's content rect with the
/// pinned header and the scroll indicators. This asserts the fixture
/// really is past what that rect fits at 1280x720, so the paging is what
/// carries the overflow.
#[test]
fn a_roster_past_what_fits_at_1280x720_still_scrolls_rather_than_overflowing() {
    let rows: Vec<WorkRow> = (0..40)
        .map(|i| {
            work_row(
                &format!("Program {i}"),
                "idle",
                if i < 30 {
                    WorkSection::Staff
                } else {
                    WorkSection::Away
                },
                [true; 4],
            )
        })
        .collect();
    let t = table(rows);
    let body = base_staff_menu_rows(&t, &[], 0, Some(0));
    let m = ui_metrics(720.0);
    with_painter(|p| {
        let char_w = p.measure_ui_advance("M", m.font_size);
        let content = crate::render::base_report::content_rect_at(1280.0, 720.0, char_w, &m);
        let fits = (content.h / m.line_height).floor() as usize;
        assert!(
            body.len() > fits,
            "the fixture should be past what fits, or this test proves nothing: \
             {} rows against {fits} in the content rect",
            body.len()
        );
    });
}
