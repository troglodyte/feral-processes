//! The Stock screen: what the base holds against what work orders ask for.
//! Every figure comes from `Game::stock_report`; this only formats.

use super::staff::elide;
use super::*;
use feral_processes_engine::{OrderState, StockReportRow};

/// The widest an item's name column grows.
const NAME_CHARS: usize = 24;

fn state_text(state: OrderState) -> &'static str {
    match state {
        OrderState::Working => "being made",
        OrderState::Queued => "queued",
        OrderState::Dormant => "met",
        OrderState::Stalled => "stalled",
    }
}

pub(super) fn stock_rows(report: &[StockReportRow], selected: usize) -> Vec<Row> {
    if report.is_empty() {
        return vec![text_row(
            "The base holds nothing and no order asks for anything.",
        )];
    }
    let name_w = report
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(NAME_CHARS);
    let qty_w = report
        .iter()
        .map(|r| r.qty.to_string().len())
        .max()
        .unwrap_or(1);
    report
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let target = match r.order {
                Some((target, state)) => format!("  of {target} ordered - {}", state_text(state)),
                None => String::new(),
            };
            let color = if r.is_stalled_short() {
                RED
            } else if r.is_short() {
                YELLOW
            } else {
                TEXT
            };
            colored_item_row(
                format!(
                    "{:<name_w$}  {:>qty_w$}{target}",
                    elide(&r.name, name_w),
                    r.qty
                ),
                i == selected,
                color,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::fit::{assert_rows_fit, text_of};
    use super::*;
    use feral_processes_engine::items::ItemId;

    fn row(qty: u32, order: Option<(u32, OrderState)>) -> StockReportRow {
        StockReportRow {
            item: ItemId::from("core_fragment"),
            tag: "CF".into(),
            name: "Core Fragment".into(),
            qty,
            order,
        }
    }

    #[test]
    fn a_short_row_shows_its_target_and_the_state_of_its_order() {
        let rows = stock_rows(&[row(2, Some((5, OrderState::Stalled)))], 0);
        let t = text_of(&rows[0]);
        assert!(t.contains("Core Fragment") && t.contains("2"), "{t}");
        assert!(t.contains("of 5 ordered - stalled"), "{t}");
        assert!(matches!(&rows[0], Row::Item { color, .. } if *color == RED));
        let rows = stock_rows(&[row(9, None)], 0);
        assert!(!text_of(&rows[0]).contains("ordered"));
    }

    #[test]
    fn the_widest_stock_row_fits_the_content_rect() {
        let mut worst = row(u32::MAX, Some((u32::MAX, OrderState::Working)));
        worst.name = "N".repeat(40);
        assert_rows_fit(&stock_rows(&[worst], 0), "Stock");
    }
}
