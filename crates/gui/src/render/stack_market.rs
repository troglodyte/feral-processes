//! A Stack market's shelf, and the holder picker the narrowest routine
//! rung opens.

use super::popup::*;
use super::*;

/// Everything a market has, in the order `app::stack_market::market_row`
/// resolves a picked row against — offers, then what the stall will take.
///
/// Both lists come from one `Game::stack_market` call, which is what stops
/// the drawn rows and the handler's rows disagreeing about which row number
/// is which. A renderer that rebuilt either list itself would be right
/// until the first purchase.
pub(super) fn draw_stack_market(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let Some(view) = game.stack_market() else {
        return;
    };
    let money = &view.currency;
    let mut rows = vec![
        text_row(format!("You have: {} {money}", view.credits)),
        Row::TextColored(
            "For sale — and once it's gone, it's gone:".to_string(),
            TEXT,
        ),
    ];

    let mut idx = 0;
    if view.offers.is_empty() {
        rows.push(text_row("(the shelf is bare)"));
    }
    for offer in &view.offers {
        // Priced rows the player cannot afford stay listed and stay dim: a
        // shelf you have to come back for is a reason to go deeper, where a
        // hidden row is a shelf that looks smaller than it is.
        let label = format!(
            "[{}] {}  ({} {money})",
            menu_shortcut(idx),
            offer.name,
            offer.price
        );
        rows.push(if offer.affordable {
            item_row(label, idx == selected)
        } else {
            spent_item_row(label, idx == selected)
        });
        if !offer.detail.is_empty() {
            rows.push(Row::TextColored(
                format!("      {}", offer.detail),
                TEXT_DIM,
            ));
        }
        idx += 1;
    }

    rows.push(text_row(""));
    rows.push(Row::TextColored(
        "They'll take (no buyback — they won't remember you):".to_string(),
        TEXT,
    ));
    if view.sells.is_empty() {
        rows.push(text_row("(nothing they want)"));
    }
    for row in &view.sells {
        rows.push(tier_row(
            format!(
                "[{}] {}  Sell {} x{} ({} {money} each)",
                menu_shortcut(idx),
                game.item_category(&row.copy.item).short_label(),
                row.name,
                row.qty,
                row.unit_price
            ),
            idx == selected,
            row.copy.tier,
            row.copy.rarity,
        ));
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(text_row("[S] sells the whole stack  ·  Esc to walk on"));
    draw_popup(
        "A stall in the corridor",
        PopupSize::Large,
        &rows,
        painter,
        m,
    );
}
