//! The collect picker: what the adjacent machines are offering, and how much
//! of each the player has asked for.

use super::popup::*;
use super::*;

/// One row per item on offer, each carrying `taken / available` in the row's
/// own suffix column.
///
/// **No shortcut lead.** Every other list opens its rows with `[1] `, but a
/// digit here is a quantity — advertising a key that sets an amount instead
/// of picking a row would be a menu that lies about its own keys.
///
/// **The figures go in the suffix column** rather than being `format!`ed into
/// the name. Six screens made that mistake with the category tag: measuring a
/// row without its column makes `suffix_x` drop the suffix on the row's own
/// tail, and a wrap then budgets for a row narrower than it draws.
///
/// **The page needs no height census.** The cursor drives `menu_selected` and
/// `popup_layout` keeps the selected row visible, so a long shelf scrolls —
/// unlike the memories and gear-inspect pages, whose rows are `Text` and so
/// have no `Item` span for the popup to page. It does need a width one:
/// `draw_row` clips vertically only.
pub(super) fn draw_collect(
    game: &Game,
    rows: &[(ItemId, u32)],
    basket: &[u32],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut body = vec![
        text_row("Up/Down pick a row; digits, Backspace and Left/Right set the amount"),
        text_row("[A] take everything  [N] take nothing  Enter to collect  Esc to leave"),
        text_row(""),
    ];
    for (i, (item, available)) in rows.iter().enumerate() {
        let taken = basket.get(i).copied().unwrap_or(0);
        body.push(annotated_item_row(
            game.item_name(item),
            Some(collect_suffix(taken, *available)),
            i == selected,
            TEXT,
        ));
    }
    draw_popup("Collect", PopupSize::Large, &body, painter, m);
}

/// What a row's suffix column reads: how much has been asked for, out of how
/// much is there.
///
/// Its own function so the width census measures the string the screen draws
/// rather than a hand-written stand-in for it.
fn collect_suffix(taken: u32, available: u32) -> String {
    format!("{taken} / {available}")
}
