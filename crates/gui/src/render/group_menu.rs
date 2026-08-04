//! The two group menus, `b` and `p`. One draw function for both: they differ
//! only in their title and the rows app-core hands over.

use super::popup::*;
use super::*;

/// Rows come from `App::base_menu_rows`/`party_menu_rows` — the same call
/// the handler dispatches from. Rows are hidden when the screen behind them
/// would be empty, so building the list here instead would drift out of
/// index with the handler and row 2 would open a different screen from the
/// one under the highlight.
pub(super) fn draw_group_menu(
    rows: &[GroupMenuRow],
    title: &str,
    selected: usize,
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
    draw_popup(title, PopupSize::Large, &out, painter, m);
}
