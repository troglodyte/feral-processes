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
/// **The hint says which arrow does which.** Left adds and Right removes here,
/// against every other Left/Right in the game, so a player who guesses from
/// the rest of the UI guesses wrong — and a modifier is invisible until
/// named, so Shift and Ctrl ride the same line. The three hint lines stay no
/// wider than the `[A]`/`[N]` line, which is the widest text the screen has
/// always drawn — the width census below covers item rows only, and
/// `draw_row` clips vertically alone.
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
        text_row("Up/Down pick a row; digits and Backspace type an amount"),
        text_row("Left adds one, Right removes one; Shift for all, Ctrl halves the gap"),
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DifficultyMode, Game};

    /// **The widest collect row the shipped assets can build still fits.**
    ///
    /// `draw_row` clips a row vertically and nothing clips it horizontally, so
    /// an over-wide row is drawn off the panel in silence — taking the figures
    /// that say how much is on the shelf with it.
    ///
    /// The name comes from the real `ItemDb` rather than a hand-written
    /// string, which is the difference between a census and a fixture: a
    /// longer item name dropped into `assets/items/` has to fail here rather
    /// than be caught by eye. The figures are the widest `u32` can print,
    /// since nothing bounds what a modded `capacity` may hold.
    #[test]
    fn no_collect_row_overflows_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let widest = game
            .item_defs()
            .into_iter()
            .map(|def| game.item_name(&def.id).to_string())
            .max_by_key(|name| name.chars().count())
            .expect("the shipped assets define items");
        let suffix = collect_suffix(u32::MAX, u32::MAX);

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            // What `draw_row` actually lays down: the row's label, then the
            // suffix one inset past the end of it (see `suffix_x`).
            let label = p.measure_ui_advance(format!("  {widest}"), m.font_size);
            let drawn = label + m.inset + p.measure_ui_advance(&suffix, m.font_size);
            assert!(
                label > 0.0,
                "the census measured nothing — the shipped set has to reach here"
            );
            assert!(
                drawn <= room,
                "the widest collect row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}  {suffix}",
                drawn - room
            );
        });
    }
}
