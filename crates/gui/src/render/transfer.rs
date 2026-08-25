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
/// **The columns line up because the name is padded to the widest one on the
/// screen**, not because `draw_row` knows anything about tables. `suffix_x`
/// places a suffix one inset past the *advance* of the row's own label, so a
/// name padded out to a common width puts every suffix at the same x — the
/// UI face is monospace, and a trailing space advances exactly as a glyph
/// does. `PowerCell` reaches the same straight edge the other way, by
/// reserving a fixed cell inside the label; that shape is for a column
/// between the lead and the name, and these figures sit after it.
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
    let cols = Columns::of(game, entries);
    for (i, (item, amount, put, take)) in entries.iter().enumerate() {
        body.push(annotated_item_row(
            cols.name_cell(game.item_name(item)),
            Some(cols.suffix(*amount, *put, *take)),
            i == selected,
            TEXT,
        ));
    }
    body
}

/// How wide each of this screen's four columns runs: the item name, then the
/// three figures after it.
///
/// **Measured from the rows actually listed rather than fixed**, so a shelf
/// of short names draws a narrow table instead of a wide one full of empty space.
/// Every row is built in one pass over `entries`, so the widths cannot shift
/// under a row while the screen is open — typing an amount rebuilds the whole
/// body, and a wider figure widens the column for every row at once rather
/// than knocking one out of line.
///
/// A name longer than the column is not truncated: it pushes its own figures
/// right and leaves the rest of the table alone. Losing characters off an
/// item's name to keep a column straight is the worse of the two failures,
/// and `no_transfer_row_overflows_its_popup` is what says the shipped set has
/// room for the widest of them.
struct Columns {
    name: usize,
    amount: usize,
    put: usize,
    take: usize,
}

impl Columns {
    fn of(game: &Game, entries: &[(ItemId, i64, u32, u32)]) -> Self {
        let mut cols = Columns {
            name: 0,
            amount: 0,
            put: 0,
            take: 0,
        };
        for (item, amount, put, take) in entries {
            cols.name = cols.name.max(game.item_name(item).chars().count());
            cols.amount = cols.amount.max(amount.to_string().len());
            cols.put = cols.put.max(put_cell(*put).len());
            cols.take = cols.take.max(take_cell(*take).len());
        }
        cols
    }

    /// The name padded out to the column, which is what puts every suffix at
    /// the same x — see the module doc.
    fn name_cell(&self, name: &str) -> String {
        format!("{name:<width$}", width = self.name)
    }

    /// What a row's suffix column reads: the signed amount, then how far the
    /// row may still go in each direction.
    ///
    /// The availables are **live** rather than the row's raw figures: a row
    /// reading `-0` while the pack still holds units is the screen saying the
    /// other rows have spent the Depot's room. Its own function so the width
    /// census measures the string the screen draws rather than a hand-written
    /// stand-in for it.
    fn suffix(&self, amount: i64, put: u32, take: u32) -> String {
        let (put, take) = (put_cell(put), take_cell(take));
        format!(
            "{amount:>aw$} / {put:>pw$} .. {take:>tw$}",
            aw = self.amount,
            pw = self.put,
            tw = self.take,
        )
    }
}

/// The two ends of a row's range, spelled once each so the width measured for
/// a column and the string drawn into it cannot disagree.
fn put_cell(put: u32) -> String {
    format!("-{put}")
}

fn take_cell(take: u32) -> String {
    format!("+{take}")
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
        let cols = Columns::of(
            &game,
            &[(
                ItemId::from("core_fragment"),
                -(u32::MAX as i64),
                u32::MAX,
                u32::MAX,
            )],
        );
        let suffix = cols.suffix(-(u32::MAX as i64), u32::MAX, u32::MAX);

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

    /// **Every row's figures start at the same x, whatever its name is
    /// worth.** That is the whole of the column: `suffix_x` places a suffix
    /// one inset past the *advance* of the row's own label, so two rows agree
    /// only if their labels measure the same — which is what the padding in
    /// `name_cell` buys. Drop it and the figures step in and out with the
    /// names above them.
    ///
    /// Measured rather than asserted on the string, because the advance is
    /// what `draw_row` actually places against; and the suffixes are held to
    /// one width besides, since a row whose figures are internally ragged is
    /// a table only at its left edge.
    #[test]
    fn every_rows_figures_start_at_the_same_x() {
        let game = shipped_game();
        let mut names: Vec<_> = game
            .item_defs()
            .into_iter()
            .map(|def| (def.id.clone(), game.item_name(&def.id).chars().count()))
            .collect();
        names.sort_by_key(|(_, len)| *len);
        let short = names
            .first()
            .expect("the shipped assets define items")
            .clone();
        let long = names
            .last()
            .expect("the shipped assets define items")
            .clone();
        assert!(
            short.1 < long.1,
            "the census needs two names of different lengths to say anything"
        );

        // Figures of different widths too, or the suffix column would line up
        // by luck rather than by being one.
        let entries = vec![(short.0, 5i64, 7u32, 9u32), (long.0, -1234i64, 99u32, 1u32)];
        let rows = body_rows(&game, &entries, None, 0);
        let items: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Item { text, suffix, .. } => Some((text.clone(), suffix.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 2, "one row per entry");

        let widths: Vec<_> = items
            .iter()
            .map(|(_, suffix)| suffix.as_ref().expect("every row carries figures").len())
            .collect();
        assert_eq!(widths[0], widths[1], "the figures are ragged: {items:?}");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `draw_row`'s own label for an unselected, untagged, iconless
            // row — the string `suffix_x` measures.
            let at = |text: &str| p.measure_ui_advance(format!("  {text}"), m.font_size);
            let (a, b) = (at(&items[0].0), at(&items[1].0));
            assert!(
                (a - b).abs() < 0.5,
                "the figures step with the name: {a} against {b}\n{items:?}"
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
