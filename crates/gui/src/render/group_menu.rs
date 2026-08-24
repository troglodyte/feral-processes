//! The two group menus, `b` and `p`. One draw function for both: they differ
//! only in their title and the rows app-core hands over.

use super::popup::*;
use super::*;

/// Rows come from `App::base_menu_rows`/`party_menu_rows` — the same call
/// the handler dispatches from. Rows are hidden when the screen behind them
/// would be empty, so building the list here instead would drift out of
/// index with the handler and row 2 would open a different screen from the
/// one under the highlight.
/// The dev keypad. Same popup shape as the group menus and drawn beside
/// them deliberately — it is one more list of labelled actions, and a file
/// of its own would only separate it from the helpers it shares.
///
/// The rows come from `App::dev_console_rows`, the same table the handler
/// dispatches, so a label here can never name an action the press does not
/// take.
pub(super) fn draw_dev_console(
    rows: &[DevConsoleRow],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut out = vec![text_row("Dev build only. Esc to close.")];
    for (i, row) in rows.iter().enumerate() {
        out.push(item_row(
            format!("[{}] {}", menu_shortcut(i), row.label),
            i == selected,
        ));
    }
    draw_popup("Dev Console", PopupSize::Large, &out, refusal, painter, m);
}

pub(super) fn draw_group_menu(
    rows: &[GroupMenuRow],
    title: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut out = vec![text_row("Esc to close; Up/Down + Enter also work")];
    if rows.is_empty() {
        out.push(text_row("(nothing available here right now)"));
    }
    for (i, row) in rows.iter().enumerate() {
        out.push(item_row(
            format!("[{}] {}", menu_shortcut(i), row.label),
            i == selected,
        ));
    }
    draw_popup(title, PopupSize::Large, &out, refusal, painter, m);
}
