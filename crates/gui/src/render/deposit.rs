//! The deposit picker: what is in the pack, and how much of each the player
//! has asked to put away into an adjacent Depot's `output`.

use super::popup::*;
use super::*;

/// One row per pack item, each carrying `given / available` in the row's own
/// suffix column — mirroring `collect::draw_collect` down to the four rules
/// its doc comment states (no shortcut lead, figures in the suffix column,
/// hint lines naming both arrows and both modifiers, a width census and no
/// height one).
///
/// The one addition is the header line: a Depot's room is a **single**
/// budget shared across every row rather than each row's own shelf, so
/// without it there is nothing on screen saying why a row stopped rising.
///
/// `entries` is `(item, given, available)` per row, zipped by the caller
/// rather than taken as three parallel slices — `draw_deposit` would
/// otherwise carry eight arguments, one past `clippy::too_many_arguments`.
/// The `available` figure in it is `App::basket_available` per row, called by
/// the caller rather than recomputed here: `basket.rs`'s doc comment names
/// that expression as the one place the two pickers genuinely differ, and a
/// second copy of it in `gui` is exactly the drift that function was made
/// `pub` to avoid.
pub(super) fn draw_deposit(
    game: &Game,
    entries: &[(ItemId, u32, u32)],
    room: u32,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let given: u32 = entries.iter().map(|(_, given, _)| given).sum();
    let remaining = room.saturating_sub(given);
    let mut body = vec![
        text_row(format!("Depot room remaining: {remaining}")),
        text_row("Up/Down pick a row; digits and Backspace type an amount"),
        text_row("Left adds one, Right removes one; Shift for all, Ctrl halves the gap"),
        text_row("[A] take everything  [N] take nothing  Enter to put away  Esc to leave"),
        text_row(""),
    ];
    for (i, (item, given, available)) in entries.iter().enumerate() {
        body.push(annotated_item_row(
            game.item_name(item),
            Some(deposit_suffix(*given, *available)),
            i == selected,
            TEXT,
        ));
    }
    draw_popup("Deposit", PopupSize::Large, &body, refusal, painter, m);
}

/// What a row's suffix column reads: how much has been given, out of how
/// much the shared budget will still let this row hold.
///
/// Its own function so the width census measures the string the screen draws
/// rather than a hand-written stand-in for it — `collect_suffix`'s reason.
fn deposit_suffix(given: u32, available: u32) -> String {
    format!("{given} / {available}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DifficultyMode, Game};

    /// **The widest deposit row the shipped assets can build still fits.**
    ///
    /// Mirrors `collect.rs`'s `no_collect_row_overflows_its_popup` exactly —
    /// `draw_row` clips a row vertically and nothing clips it horizontally, so
    /// an over-wide row is drawn off the panel in silence, taking the figures
    /// that say how much of the budget a row is asking for with it.
    ///
    /// The name comes from the real `ItemDb` rather than a hand-written
    /// string, and the figures are the widest a `u32` can print, since
    /// nothing bounds what a modded Depot's `capacity` may hold.
    #[test]
    fn no_deposit_row_overflows_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let widest = game
            .item_defs()
            .into_iter()
            .map(|def| game.item_name(&def.id).to_string())
            .max_by_key(|name| name.chars().count())
            .expect("the shipped assets define items");
        let suffix = deposit_suffix(u32::MAX, u32::MAX);

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
                "the widest deposit row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}  {suffix}",
                drawn - room
            );
        });
    }
}
