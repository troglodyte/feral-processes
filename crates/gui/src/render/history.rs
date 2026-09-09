//! The message log history screen (`Mode::History`).

use super::*;

/// The message log in full — everything the pane at the bottom of the map
/// has room for a few lines of.
///
/// The footer states the screen's three limits rather than leaving the player
/// to infer them from an absence: the engine keeps `MESSAGE_LOG_CAP` lines,
/// repeats are folded into one row apiece, and
/// `MessageLog::retain_outcomes_since_battle` drops a finished intrusion's
/// blow-by-blow, so an old fight reads as its results.
pub(super) fn draw_history(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let entries = game.message_history(MESSAGE_LOG_CAP);
    let mut rows = history_rows(&entries, selected);
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "The last {MESSAGE_LOG_CAP} lines, repeats folded. A finished intrusion keeps its results, not its blow-by-blow."
    )));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("History", PopupSize::Large, &rows, refusal, painter, m);
}

/// The scrollable body of the history screen: one row per folded entry (see
/// `Game::message_history`), oldest first to match the map's pane, each in its
/// `MessageKind` colour through the same `message_color` that pane's
/// `draw_message_line` calls.
///
/// Rows are `Row::Item` because that is what `popup_layout` scrolls; the
/// highlight is the scroll position, not a selection, and
/// `App::handle_history_key` accepts nothing that would pick one. Which makes
/// the row count load-bearing: app-core counts the same folded entries to
/// bound that highlight, so a row here without one there is a highlight
/// pointing at nothing.
pub(super) fn history_rows(entries: &[LogEntry], selected: usize) -> Vec<Row> {
    if entries.is_empty() {
        return vec![text_row("Nothing has happened yet.")];
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            counted_item_row(
                &entry.text,
                entry.repeats,
                i == selected,
                message_color(entry.kind),
            )
        })
        .collect()
}
