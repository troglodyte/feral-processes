//! The dev keypad's two pickers, `Mode::DevItemPicker` and
//! `Mode::DevZonePicker`. One file because they share a layout: a hint row,
//! then one row per entry app-core hands over. Up/Down move the highlight,
//! Enter picks it, and the popup's window follows the highlighted row.

use super::list_filter::{filter_line, no_match_row};
use super::popup::*;
use super::*;

const HEADER: &str = "Dev build only";

pub(super) fn item_picker_rows(rows: &[(String, u32)], filter: &str, selected: usize) -> Vec<Row> {
    let mut out = vec![text_row(HEADER)];
    out.extend(filter_line(filter));
    if rows.is_empty() {
        out.push(no_match_row(filter));
    }
    out.extend(
        rows.iter()
            .enumerate()
            .map(|(i, (name, held))| item_row(format!("{name}  x{held}"), i == selected)),
    );
    push_key_footer(
        &mut out,
        &[
            "Type to filter",
            "Enter give",
            "Esc back (clears the filter first)",
        ],
    );
    out
}

pub(super) fn zone_picker_rows(rows: &[u32], selected: usize) -> Vec<Row> {
    let mut out = vec![text_row(HEADER)];
    out.extend(
        rows.iter()
            .enumerate()
            .map(|(i, zone)| item_row(format!("Zone {zone}"), i == selected)),
    );
    push_key_footer(&mut out, &["Enter warp", "Esc back"]);
    out
}

pub(super) fn draw_dev_item_picker(
    rows: &[(String, u32)],
    filter: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Give item (x10)",
        PopupSize::Large,
        &item_picker_rows(rows, filter, selected),
        refusal,
        painter,
        m,
    );
}

pub(super) fn draw_dev_zone_picker(
    rows: &[u32],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Warp to zone",
        PopupSize::Large,
        &zone_picker_rows(rows, selected),
        refusal,
        painter,
        m,
    );
}

#[cfg(test)]
mod tests {
    use super::super::test_support::test_assets_dir;
    use super::*;
    use crate::paint::{painted_text, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::{DifficultyMode, Game};

    /// Every shipped item, each with a count wider than any stack the game
    /// reaches.
    fn full_item_list() -> Vec<(String, u32)> {
        let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.dev_plain_items()
            .into_iter()
            .map(|(_, name, _)| (name, 99_999))
            .collect()
    }

    /// The widest shipped item name, at the last selector, in the picker's
    /// popup: the 1440x900 geometry `ui_metrics` is calibrated for, against
    /// `PopupSize::Large`'s 0.88 width fraction.
    #[test]
    fn the_widest_item_row_fits_its_popup() {
        let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let widest = game
            .dev_plain_items()
            .into_iter()
            .map(|(_, name, _)| name)
            .max_by_key(|n| n.chars().count())
            .unwrap();
        let rows = item_picker_rows(&[(widest, 99_999)], "", 0);
        let Some(Row::Item { text: label, .. }) =
            rows.iter().rfind(|r| matches!(r, Row::Item { .. }))
        else {
            panic!("the last row is the item");
        };
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(label, m.font_size);
            assert!(drawn <= room, "{label} overflows by {:.0}px", drawn - room);
        });
    }

    /// Both pickers draw their rows without labels: the name and the count
    /// for an item, the zone for a warp.
    #[test]
    fn both_pickers_draw_their_rows() {
        let m = ui_metrics(900.0);
        let (_, shapes) =
            with_painter(|p| draw_dev_item_picker(&full_item_list()[..3], "", 1, None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("x99999"), "{text}");
        assert!(!text.contains("[1]"), "{text}");

        let (_, shapes) = with_painter(|p| draw_dev_zone_picker(&[4, 5], 1, None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("Zone 5"), "{text}");
        assert!(!text.contains("[2]"), "{text}");
    }

    /// The highlight is on the row `selected` names, so a list deeper than
    /// the popup can follow it.
    #[test]
    fn the_highlight_is_on_the_selected_row() {
        let rows = item_picker_rows(&full_item_list(), "", 7);
        let lit: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, Row::Item { selected: true, .. }))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(lit, vec![8], "hint row first, then item 7");
    }

    /// A typed filter shows above the rows; with none left, the notice
    /// replaces them.
    #[test]
    fn the_filter_and_the_empty_notice_are_drawn() {
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| draw_dev_item_picker(&[], "zzz", 0, None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("filter: zzz"), "{text}");
        assert!(text.contains("Nothing matches"), "{text}");
    }
}
