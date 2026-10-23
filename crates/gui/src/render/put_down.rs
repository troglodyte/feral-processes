//! The put-down picker: one row per thing in the pack that can lie on the
//! floor. The rows are `App::put_down_rows`, the same list the keys pick
//! from.

use super::popup::*;
use super::*;
use feral_processes_engine::PutDownOption;

pub(super) fn draw_put_down(
    rows: &[PutDownOption],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Put down",
        PopupSize::Large,
        &body_rows(rows, selected),
        refusal,
        painter,
        m,
    );
}

fn body_rows(rows: &[PutDownOption], selected: usize) -> Vec<Row> {
    let mut body = vec![
        text_row("Pick what to set on the floor. Esc to go back."),
        text_row(""),
    ];
    body.extend(
        rows.iter()
            .enumerate()
            .map(|(i, row)| item_row(row.label.to_string(), i == selected)),
    );
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::PutDownChoice;
    use feral_processes_engine::items::ItemId;

    /// Every row prints the key that picks it, since `selected_index` honours
    /// it whether or not it is shown.
    #[test]
    fn every_row_prints_the_key_that_picks_it() {
        let rows = [
            PutDownOption {
                label: "Core Fragment x2".to_string(),
                choice: PutDownChoice::Item(ItemId::from("core_fragment")),
            },
            PutDownOption {
                label: "The level 3 Scrapper".to_string(),
                choice: PutDownChoice::Program(0),
            },
        ];
        let texts: Vec<_> = body_rows(&rows, 0)
            .into_iter()
            .filter_map(|row| match row {
                Row::Item { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                "Core Fragment x2".to_string(),
                "The level 3 Scrapper".to_string()
            ]
        );
    }
}
