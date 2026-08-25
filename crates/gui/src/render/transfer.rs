//! The transfer picker: what the adjacent shelves are holding, what the pack
//! could put back, and how much of each the player has asked to move.

use super::popup::*;
use super::*;

/// One row per item, each carrying the signed amount and both live ceilings
/// in the row's own suffix column.
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
/// **The hint says which arrow does which.** Left puts in and Right takes out
/// here, against every other Left/Right in the game, so a player who guesses
/// from the rest of the UI guesses wrong — and a modifier is invisible until
/// named, so Shift and Ctrl ride the same line.
///
/// **The page needs no height census.** The cursor drives `menu_selected` and
/// `popup_layout` keeps the selected row visible, so a long shelf scrolls. It
/// does need a width one: `draw_row` clips vertically only.
///
/// `entries` is `(item, amount, put_available, take_available)` per row,
/// zipped by the caller rather than taken as parallel slices — the two
/// availables are `App::put_available` and `App::take_available`, which
/// borrow the whole `App` and so must be called before `&mut app.game` is
/// taken. Calling them rather than recomputing the budget here keeps the
/// rule stated once, in `basket.rs`.
pub(super) fn draw_transfer(
    game: &Game,
    entries: &[(ItemId, i64, u32, u32)],
    room: Option<u32>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let body = body_rows(game, entries, room, selected);
    draw_popup("Transfer", PopupSize::Large, &body, refusal, painter, m);
}

/// The header lines and then the item rows.
///
/// The room line is **omitted entirely** when there is no Depot beside the
/// party: a Mining Node has no room to report, and a line reading 0 there
/// claims the base is full when it has no shelf at all. `Some(0)` still
/// draws it — that is a Depot with nothing left, which is exactly the thing
/// the player needs told.
fn body_rows(
    game: &Game,
    entries: &[(ItemId, i64, u32, u32)],
    room: Option<u32>,
    selected: usize,
) -> Vec<Row> {
    let mut body = Vec::new();
    if let Some(room) = room {
        let given: u32 = entries.iter().fold(0u32, |acc, (_, n, _, _)| {
            acc.saturating_add((-*n).max(0) as u32)
        });
        let remaining = room.saturating_sub(given);
        body.push(text_row(format!("Depot room remaining: {remaining}")));
    }
    body.extend([
        text_row("Up/Down pick a row; digits and Backspace type an amount"),
        text_row("Left puts in, Right takes out; Shift for the end, Ctrl halves the gap"),
        text_row("[A] take everything  [N] clear  Enter to transfer  Esc to leave"),
        text_row(""),
    ]);
    for (i, (item, amount, put, take)) in entries.iter().enumerate() {
        body.push(annotated_item_row(
            game.item_name(item),
            Some(transfer_suffix(*amount, *put, *take)),
            i == selected,
            TEXT,
        ));
    }
    body
}

/// What a row's suffix column reads: the signed amount, then how far it may
/// still go in each direction.
///
/// The availables are **live** rather than the row's raw figures: a row
/// reading `-0` while the pack still holds units is the screen saying the
/// other rows have spent the Depot's room. Its own function so the width
/// census measures the string the screen draws rather than a hand-written
/// stand-in for it.
fn transfer_suffix(amount: i64, put: u32, take: u32) -> String {
    format!("{amount} / -{put} .. +{take}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DifficultyMode, Game};

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    /// `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
    fn body_room(m: &Metrics) -> f32 {
        1440.0 * 0.88 - m.pad * 2.0
    }

    /// **The widest transfer row the shipped assets can build still fits.**
    ///
    /// `draw_row` clips a row vertically and nothing clips it horizontally,
    /// so an over-wide row is drawn off the panel in silence — taking the
    /// figures that say how far the row may move with it.
    ///
    /// The name comes from the real `ItemDb` rather than a hand-written
    /// string, which is the difference between a census and a fixture. The
    /// figures are the widest each end can print, since nothing bounds what a
    /// modded Depot's `capacity` may hold.
    #[test]
    fn no_transfer_row_overflows_its_popup() {
        let game = shipped_game();
        let widest = game
            .item_defs()
            .into_iter()
            .map(|def| game.item_name(&def.id).to_string())
            .max_by_key(|name| name.chars().count())
            .expect("the shipped assets define items");
        let suffix = transfer_suffix(-(u32::MAX as i64), u32::MAX, u32::MAX);

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = body_room(&m);
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
                "the widest transfer row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{widest}  {suffix}",
                drawn - room
            );
        });
    }

    /// The hint lines are text rows and so are never wrapped or clipped
    /// horizontally either. They stay inside the same body the item census
    /// measures against.
    #[test]
    fn no_transfer_hint_line_overflows_its_popup() {
        let game = shipped_game();
        let entries = vec![(ItemId::from("core_fragment"), 0, 0, 0)];
        let rows = body_rows(&game, &entries, Some(u32::MAX), 0);

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = body_room(&m);
            for line in rows.iter().filter_map(|r| match r {
                Row::Text(t) => Some(t.clone()),
                _ => None,
            }) {
                let drawn = p.measure_ui_advance(&line, m.font_size);
                assert!(drawn <= room, "hint line overflows: {line}");
            }
        });
    }

    /// The two states that must not collapse: no Depot beside you draws no
    /// room line at all, a Depot with nothing left draws one reading 0.
    #[test]
    fn the_room_line_is_absent_without_a_depot_and_reads_zero_when_full() {
        let game = shipped_game();
        let entries = vec![(ItemId::from("core_fragment"), 0, 0, 4)];

        let heads = |room| match &body_rows(&game, &entries, room, 0)[0] {
            Row::Text(t) => t.clone(),
            _ => panic!("the body opens with a text row"),
        };
        assert!(
            !heads(None).starts_with("Depot room"),
            "a Mining Node has no room to report"
        );
        assert_eq!(heads(Some(0)), "Depot room remaining: 0");
    }
}
