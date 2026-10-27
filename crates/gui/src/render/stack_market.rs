//! A Stack market's shelf, and the holder picker the narrowest routine
//! rung opens.

use super::list_filter::{filter_line, no_match_row};
use super::popup::*;
use super::*;
use feral_processes_app_core::market_visible;

/// Everything a market has, in the order `app::stack_market::market_row`
/// resolves a picked row against — offers, then what the stall will take.
///
/// Both lists come from one `Game::stack_market` call, which is what stops
/// the drawn rows and the handler's rows disagreeing about which row number
/// is which. A renderer that rebuilt either list itself would be right
/// until the first purchase.
pub(super) fn draw_stack_market(
    game: &mut Game,
    filter: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(view) = game.stack_market() else {
        return;
    };
    let money = &view.currency;
    let mut rows = vec![text_row(format!("You have: {} {money}", view.credits))];
    rows.extend(filter_line(filter));
    // The handler resolves a highlight through the same call, so `idx`
    // counts visible rows and a section with none is skipped, not drawn bare.
    let vis = market_visible(&view, filter);
    let filtering = !filter.is_empty();
    if filtering && vis.total() == 0 {
        rows.push(no_match_row(filter));
    }
    if !filtering || !vis.offers.is_empty() {
        rows.push(Row::TextColored(
            "For sale — and once it's gone, it's gone:".to_string(),
            TEXT,
        ));
    }

    let mut idx = 0;
    if !filtering && view.offers.is_empty() {
        rows.push(text_row("(the shelf is bare)"));
    }
    for &src in &vis.offers {
        let offer = &view.offers[src];
        // Priced rows the player cannot afford stay listed and stay dim: a
        // shelf you have to come back for is a reason to go deeper, where a
        // hidden row is a shelf that looks smaller than it is.
        let label = format!("{}  ({} {money})", offer.name, offer.price);
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

    if !filtering || !vis.sells.is_empty() {
        rows.push(text_row(""));
        rows.push(Row::TextColored(
            "They'll take (no buyback — they won't remember you):".to_string(),
            TEXT,
        ));
    }
    if !filtering && view.sells.is_empty() {
        rows.push(text_row("(nothing they want)"));
    }
    for &src in &vis.sells {
        let row = &view.sells[src];
        rows.push(with_tag(
            tier_row(
                format!("Sell {} ({} {money} each)", row.name, row.unit_price),
                idx == selected,
                row.copy.tier,
                row.copy.rarity,
            ),
            row_lead(Some(row.qty)),
            game.item_category(&row.copy.item).short_label(),
            Some(row.copy.quality),
            PowerCell::of_copy(game, &row.copy),
        ));
        for line in effect_lines(game, &row.copy.item, ROW_WRAP_COLUMNS) {
            rows.push(tier_row(line, false, row.copy.tier, row.copy.rarity));
        }
        idx += 1;
    }
    rows.push(text_row(""));
    rows.push(text_row(key_hints(&[
        "[S] sell the whole stack",
        "Type to filter",
        "[I] inspect",
        "Esc back (clears the filter first)",
    ])));
    draw_popup(
        "A stall in the corridor",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}
