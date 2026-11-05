use super::*;
use feral_processes_engine::items::ItemId;

/// The page at its worst: both sections full to the report's own cap,
/// the widest name a shipped item could carry, and every row
/// `Game::attention` can produce at once.
fn tallest_base_output_page() -> Vec<Row> {
    use feral_processes_engine::tuning::{BASE_OUTPUT_MAX_ROWS, BASE_OUTPUT_SPARK_BUCKETS};
    use feral_processes_engine::{AttentionKind, AttentionRow, BaseOutputRow};

    let row = |i: usize| BaseOutputRow {
        item: ItemId(format!("item_{i}")),
        // Longer than the column, so the cut is measured rather than
        // assumed: a modded item set is the case that reaches it.
        name: "Singularity Matrix Mk4 Prototype".to_string(),
        sector: 999_999,
        run: 999_999,
        machine: 999_999,
        hand: 999_999,
        used: 999_999,
        // Every sink at once, the longest labels: the line must be cut,
        // so the cut is what gets measured.
        used_by: [
            "into Singularity Matrix Mk4 Prototype",
            "lost in breach",
            "burnt as fuel",
            "sabotaged",
            "installed",
            "crew jobs",
            "repairs",
        ]
        .iter()
        .map(|l| (l.to_string(), 999_999))
        .collect(),
        spark: vec![999_999; BASE_OUTPUT_SPARK_BUCKETS],
    };
    let attention = |kind, text: &str, threat| AttentionRow {
        kind,
        text: text.to_string(),
        key: 'b',
        threat,
    };
    base_output_rows(&BaseOutputReport {
        zone: 10,
        mined: (0..BASE_OUTPUT_MAX_ROWS).map(row).collect(),
        compiled: (0..BASE_OUTPUT_MAX_ROWS).map(row).collect(),
        // All four rows `Game::attention` can hold at once, at the
        // widest wording each of them builds.
        attention: vec![
            attention(
                AttentionKind::StructureDamaged,
                "Recompiler Bay damaged",
                true,
            ),
            attention(AttentionKind::IdleStructures, "12 machines idle", false),
            attention(AttentionKind::PerkPoints, "12 perk points unspent", false),
            attention(AttentionKind::Unslotted, "2 unslotted (14/12)", false),
        ],
    })
}

/// **The page has no scroll.** The hub pages a `Row::Item` span and this
/// page has none, so a row past the bottom is dropped in silence —
/// `the_tallest_memory_page_fits_its_popup`'s trap. Raising
/// `BASE_OUTPUT_MAX_ROWS` past what fits means giving the page a scroll
/// first.
///
/// Swept across window heights rather than measured at one: `ui_metrics`
/// clamps the font at both ends, so below the clamp the box keeps
/// shrinking while the line height stops.
#[test]
fn the_tallest_base_output_page_fits_its_content_rect() {
    let rows = tallest_base_output_page().len();
    crate::paint::with_painter(|p| {
        for h in (600..=2160).step_by(60) {
            let m = crate::text::ui_metrics(h as f32);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let content = crate::render::base_report::content_rect_at(
                h as f32 * 16.0 / 9.0,
                h as f32,
                char_w,
                &m,
            );
            // One line is the refusal, when there is one.
            let cap = (content.h / m.line_height).floor() as usize - 1;
            assert!(
                rows <= cap,
                "a full base builds a {rows}-row page into a {cap}-row content rect at {h}px"
            );
        }
    });
}

/// The other axis, and the one nothing clamps at all: `draw_row` clips a
/// row vertically and never horizontally, so a line past the right edge
/// is simply lost — and on this page the tail of a row is the run total
/// and the sparkline.
#[test]
fn no_base_output_row_overflows_its_content_rect() {
    let rows = tallest_base_output_page();
    crate::paint::with_painter(|p| {
        for (w, h) in [(1280.0, 720.0), (1440.0, 900.0), (1920.0, 1080.0)] {
            let m = crate::text::ui_metrics(h);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let content = crate::render::base_report::content_rect_at(w, h, char_w, &m);
            let room = content.w - m.pad * 2.0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    _ => continue,
                };
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a base output row overflows the page by {:.0}px at {w}x{h} \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        }
    });
}

/// The columns are read down, not across, so a name wider than its
/// column is cut rather than allowed to shift the four figures beside
/// it — one modded item would otherwise skew every row under it.
#[test]
fn a_long_name_is_cut_rather_than_moving_the_columns() {
    use feral_processes_engine::BaseOutputRow;
    use feral_processes_engine::tuning::BASE_OUTPUT_SPARK_BUCKETS;

    let wide = BaseOutputRow {
        item: ItemId("wide".to_string()),
        name: "M".repeat(OUTPUT_NAME_COLUMN * 2),
        sector: 1,
        run: 2,
        machine: 2,
        hand: 0,
        used: 0,
        used_by: Vec::new(),
        spark: vec![1; BASE_OUTPUT_SPARK_BUCKETS],
    };
    let narrow = BaseOutputRow {
        name: "Core Fragment".to_string(),
        ..wide.clone()
    };
    let (wide, narrow) = (base_output_line(&wide), base_output_line(&narrow));
    assert!(wide.contains('…'), "the name was not cut: {wide}");
    assert_eq!(
        wide.chars().count(),
        narrow.chars().count(),
        "the cut name moved the columns:\n{wide}\n{narrow}"
    );
}

/// **Both sections carry the machine/hand split**, not the sector/run
/// pair the design sketch gave MINED alone. An item sits in a section on
/// dominant provenance, so a Power Cell whose machines outproduce the
/// player lands under MINED — and dropping the hand column there would
/// hide exactly the units the page was built to count.
#[test]
fn a_mined_row_still_says_what_the_player_made_by_hand() {
    use feral_processes_engine::BaseOutputRow;

    let rows = base_output_rows(&BaseOutputReport {
        zone: 2,
        mined: vec![BaseOutputRow {
            item: ItemId("power_cell".to_string()),
            name: "Power Cell".to_string(),
            sector: 40,
            run: 90,
            machine: 60,
            hand: 30,
            used: 0,
            used_by: Vec::new(),
            spark: Vec::new(),
        }],
        compiled: Vec::new(),
        attention: Vec::new(),
    });
    let line = rows
        .iter()
        .filter_map(|r| match r {
            Row::Text(t) | Row::TextColored(t, _) => Some(t),
            _ => None,
        })
        .find(|t| t.contains("Power Cell"))
        .expect("the mined row is drawn");
    assert!(line.contains("60") && line.contains("30"), "{line}");
}

/// A base that has made nothing says so: the page is always reachable
/// from the hub, so an empty ledger must not read as a broken one.
#[test]
fn an_empty_ledger_draws_a_sentence_rather_than_a_blank_page() {
    let rows = base_output_rows(&BaseOutputReport {
        zone: 1,
        mined: Vec::new(),
        compiled: Vec::new(),
        attention: Vec::new(),
    });
    assert!(rows.iter().any(|r| match r {
        Row::Text(t) | Row::TextColored(t, _) => t.contains("not made anything"),
        _ => false,
    }));
}

fn drawn(rows: &[Row]) -> Vec<&String> {
    rows.iter()
        .filter_map(|r| match r {
            Row::Text(t) | Row::TextColored(t, _) => Some(t),
            _ => None,
        })
        .collect()
}

fn row_named(
    name: &str,
    used: u32,
    used_by: &[(&str, u32)],
) -> feral_processes_engine::BaseOutputRow {
    feral_processes_engine::BaseOutputRow {
        item: ItemId(name.to_lowercase()),
        name: name.to_string(),
        sector: 1,
        run: 9,
        machine: 9,
        hand: 0,
        used,
        used_by: used_by.iter().map(|(l, q)| (l.to_string(), *q)).collect(),
        spark: Vec::new(),
    }
}

/// The usage line sits directly under its own item, and an item nothing
/// spent draws no line at all.
#[test]
fn a_used_item_draws_its_figure_and_what_used_it_beneath() {
    let rows = base_output_rows(&BaseOutputReport {
        zone: 1,
        mined: vec![
            row_named("Core Fragment", 7, &[("built", 5), ("into Power Cell", 2)]),
            row_named("Copper", 0, &[]),
        ],
        compiled: Vec::new(),
        attention: Vec::new(),
    });
    let lines = drawn(&rows);
    let at = lines
        .iter()
        .position(|t| t.contains("Core Fragment"))
        .expect("the item is drawn");
    assert!(
        lines[at].trim_end().ends_with(" 7"),
        "the used figure closes the columns: {}",
        lines[at]
    );
    assert_eq!(lines[at + 1].as_str(), "    ← built 5 · into Power Cell 2");
    assert!(lines[at + 2].contains("Copper"), "{}", lines[at + 2]);
    assert!(
        !lines[at + 3].contains('←'),
        "an unused item drew a usage line: {}",
        lines[at + 3]
    );
}

#[test]
fn a_long_breakdown_is_cut_rather_than_wrapped() {
    let pairs: Vec<(String, u32)> = (0..20).map(|i| (format!("into Thing {i}"), 99)).collect();
    let refs: Vec<(&str, u32)> = pairs.iter().map(|(l, q)| (l.as_str(), *q)).collect();
    let line = base_output_used_line(&row_named("X", 1, &refs)).expect("used");
    assert!(line.ends_with('…'), "{line}");
    assert_eq!(line.chars().count(), OUTPUT_LINE_CHARS);
}
