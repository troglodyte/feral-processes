//! The farm plot's recipe picker: every loaded crop, with what it grows.
//! The rows are `App::farm_recipe_rows`, the same list the keys pick from.

use super::popup::*;
use super::*;
use feral_processes_app_core::CropRow;

pub(super) fn draw_farm_recipe(
    rows: &[CropRow],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Plot crop",
        PopupSize::Large,
        &body_rows(rows, selected),
        refusal,
        painter,
        m,
    );
}

fn body_rows(rows: &[CropRow], selected: usize) -> Vec<Row> {
    let mut body = vec![
        text_row("Pick what this plot grows. Esc to go back."),
        text_row(""),
    ];
    // The shortcut is printed because `selected_index` honours it.
    body.extend(rows.iter().enumerate().map(|(i, row)| {
        item_row(
            format!("[{}] {} — {}", menu_shortcut(i), row.label, row.description),
            i == selected,
        )
    }));
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row prints the key that picks it, then the crop's name and what
    /// it grows.
    #[test]
    fn a_crop_row_prints_its_key_name_and_description() {
        let rows = [CropRow {
            id: "power_cell".into(),
            label: "Power Cell".into(),
            description: "A cell slowly grows.".into(),
        }];
        let texts: Vec<_> = body_rows(&rows, 0)
            .into_iter()
            .filter_map(|row| match row {
                Row::Item { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec!["[1] Power Cell — A cell slowly grows.".to_string()]
        );
    }
}
