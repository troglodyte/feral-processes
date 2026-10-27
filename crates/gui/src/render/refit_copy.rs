//! `Mode::RefitCopy`: the Refit Bench's bill for restoring one worn copy.
//! Rows are built by a pure function over `Game` so the width test can drive
//! it without a frame.

use super::popup::*;
use super::*;

pub(super) fn refit_rows(game: &Game, copy: &GearCopy) -> Vec<Row> {
    let (left, max) = game.condition_of(copy);
    let mut rows = vec![
        text_row(game.copy_name(copy)),
        text_row(format!("Condition: {left}/{max} fights left.")),
        text_row(""),
    ];
    if !game.refit_bench_standing() {
        rows.push(colored_item_row(
            "Needs a Refit Bench standing in the base.",
            false,
            TEXT_DIM,
        ));
        rows.push(text_row(""));
    }
    if copy.wear == 0 {
        rows.push(text_row("Already in full condition."));
    } else {
        let bill: Vec<String> = game
            .refit_cost(copy)
            .iter()
            .map(|(item, qty)| format!("{qty} {}", game.item_name(item)))
            .collect();
        rows.push(text_row(format!("Cost: {}", bill.join(", "))));
        rows.push(text_row(format!(
            "Refit Bench holds {} Annealed Cores",
            game.refit_bench_cores()
        )));
    }
    rows.push(text_row(""));
    rows.push(text_row(key_hints(&["Enter refit", "Esc back"])));
    rows
}

pub(super) fn draw_refit_copy(
    game: &Game,
    copy: Option<GearCopy>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(copy) = copy else { return };
    let rows = refit_rows(game, &copy);
    draw_popup("Refit Bench", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::super::popup::{popup_body_width, row_label_text};
    use super::*;
    use crate::text::ui_metrics;
    use feral_processes_engine::DifficultyMode;

    fn game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(92, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    #[test]
    fn a_worn_copy_shows_its_fights_left_and_its_bill() {
        let game = game();
        let copy = GearCopy {
            wear: 3,
            ..GearCopy::plain(ItemId::from("nullsteel_plate"))
        };
        let (left, max) = game.condition_of(&copy);
        let labels: Vec<String> = refit_rows(&game, &copy)
            .iter()
            .map(row_label_text)
            .collect();
        assert!(
            labels
                .iter()
                .any(|l| l.contains(&format!("{left}/{max} fights left"))),
            "{labels:?}"
        );
        assert!(labels.iter().any(|l| l.starts_with("Cost: ")), "{labels:?}");
        assert!(
            labels
                .iter()
                .any(|l| l == "Refit Bench holds 0 Annealed Cores"),
            "{labels:?}"
        );
        assert!(
            labels.iter().any(|l| l.contains("Needs a Refit Bench")),
            "no bench stands in a fresh game: {labels:?}"
        );
    }

    /// The widest line is the copy's own name, at the widest the swap
    /// picker's name column is cut for, with the figure appended.
    #[test]
    fn the_widest_name_and_the_bill_fit_the_popup_body_at_1280x720() {
        let game = game();
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        let mut lines: Vec<String> = refit_rows(
            &game,
            &GearCopy {
                wear: 255,
                ..GearCopy::plain(ItemId::from("nullsteel_plate"))
            },
        )
        .iter()
        .map(row_label_text)
        .collect();
        lines.push(
            "Overclocked Reinforced Singularity Matrix of Quiet Handshakes +6 (130%) 0/15!"
                .to_string(),
        );
        crate::paint::with_painter(|p| {
            for line in lines {
                let width = p.measure_ui_advance(&line, m.font_size);
                assert!(width <= body, "{width}px into a {body}px body: {line:?}");
            }
        });
    }
}
