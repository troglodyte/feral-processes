//! The dev keypad's two pickers, `Mode::DevItemPicker` and
//! `Mode::DevZonePicker`. One file because they share a layout: a hint row,
//! then one `[sel] ...` row per entry of the page app-core hands over.
//! Neither has a highlight — a selector key picks, and Up/Down scroll the
//! page — so every row is drawn unselected.

use super::popup::*;
use super::*;

const HINT: &str = "Dev build only. Up/Down scroll. Esc to go back.";

pub(super) fn item_picker_rows(rows: &[(char, String, u32)]) -> Vec<Row> {
    let mut out = vec![text_row(HINT)];
    out.extend(
        rows.iter()
            .map(|(sel, name, held)| item_row(format!("[{sel}] {name}  x{held}"), false)),
    );
    out
}

pub(super) fn zone_picker_rows(rows: &[(char, u32)]) -> Vec<Row> {
    let mut out = vec![text_row(HINT)];
    out.extend(
        rows.iter()
            .map(|(sel, zone)| item_row(format!("[{sel}] Zone {zone}"), false)),
    );
    out
}

pub(super) fn draw_dev_item_picker(
    rows: &[(char, String, u32)],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Give item (x10)",
        PopupSize::Large,
        &item_picker_rows(rows),
        refusal,
        painter,
        m,
    );
}

pub(super) fn draw_dev_zone_picker(
    rows: &[(char, u32)],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Warp to zone",
        PopupSize::Large,
        &zone_picker_rows(rows),
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
    use feral_processes_app_core::DEV_PICKER_PAGE;
    use feral_processes_engine::{DifficultyMode, Game};

    /// A full page of the shipped items, each with a count wider than any
    /// stack the game reaches.
    fn full_item_page() -> Vec<(char, String, u32)> {
        let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let items = game.dev_plain_items();
        assert!(
            items.len() >= DEV_PICKER_PAGE,
            "the shipped set fills a page"
        );
        items
            .into_iter()
            .take(DEV_PICKER_PAGE)
            .enumerate()
            .map(|(i, (_, name, _))| (feral_processes_app_core::menu_shortcut(i), name, 99_999))
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
        let rows = item_picker_rows(&[('z', widest, 99_999)]);
        let Some(Row::Item { text: label, .. }) = rows.last() else {
            panic!("the last row is the item");
        };
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(label, m.font_size);
            assert!(drawn <= room, "{label} overflows by {:.0}px", drawn - room);
        });
    }

    /// Both pickers draw their rows: the selector, the name and the count
    /// for an item; the selector and the zone for a warp.
    #[test]
    fn both_pickers_draw_their_rows() {
        let m = ui_metrics(900.0);
        let (_, shapes) =
            with_painter(|p| draw_dev_item_picker(&full_item_page()[..3], None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("[1]") && text.contains("x99999"), "{text}");

        let (_, shapes) =
            with_painter(|p| draw_dev_zone_picker(&[('1', 4), ('2', 5)], None, p, &m));
        let text = painted_text(&shapes).join("\n");
        assert!(text.contains("[2] Zone 5"), "{text}");
    }
}
