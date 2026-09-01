//! The transfer picker: what the adjacent shelves are holding, what the pack
//! is carrying, and where each unit would sit once the basket is committed.

use super::popup::*;
use super::*;

/// The lead `draw_row` puts in front of every `Row::Item` label — `"  "`
/// unselected, `"> "` selected, and the same advance either way because the
/// UI face is monospace.
///
/// The column header is a `Row::Text`, which gets **no** lead at all, so it
/// carries this one itself. Without it the whole header sits two cells left
/// of the table it names, which reads as the figures being misaligned rather
/// than as the heading being.
const HEADER_LEAD: &str = "  ";

/// What separates one column from the next. A whole number of cells, which
/// is the point: the header cannot use `suffix_x`'s pixel inset, so every
/// gap on this screen has to be spelled in monospace cells for the heading
/// and the figure under it to land on the same x.
const COLUMN_GAP: &str = "  ";

/// The three headings, in the order they are drawn. `you` is the pack,
/// `container` is the adjacent shelves — so the arrows read off the table:
/// Left pulls toward `you`, Right pushes toward `container`.
///
/// **There is no `change` column.** The two figures are what the transfer
/// would leave behind rather than what each side is holding now, so a unit
/// asked for is one the player watches leave one column and land in the
/// other. A signed delta beside them said the same thing a second time, in a
/// notation the two moving numbers do not need.
const HEADINGS: [&str; 3] = ["item", "you", "container"];

/// One row per item: the name, then the two figures, laid out as a table
/// under a header naming each column.
///
/// **No shortcut lead.** Every other list opens its rows with `[1] `, but a
/// digit here is a quantity — advertising a key that sets an amount instead
/// of picking a row would be a menu that lies about its own keys.
///
/// **The whole row is one padded string, and the suffix column is
/// deliberately unused.** A suffix is placed by `suffix_x`, one `m.inset` —
/// a *pixel* gap — past the advance of the row's own label. A header is a
/// `Row::Text` drawn flat at `x + m.pad` with no suffix of its own, so there
/// is no way to reproduce that pixel gap in a heading; the column would name
/// a position it does not sit over. Padding all three columns into the label
/// puts every boundary at a whole number of monospace cells, which the
/// header can match exactly. The hazard the suffix column exists to close —
/// a row measured without part of what it draws — does not arise, because
/// `draw_row` measures the one string it lays down.
///
/// **The columns line up because every cell is padded to the widest on the
/// screen**, not because `draw_row` knows anything about tables. The UI face
/// is monospace, so a trailing space advances exactly as a glyph does.
///
/// **The hint says which arrow does which**, because a modifier is invisible
/// until named, so Shift and Ctrl ride the same line.
///
/// **The page needs no height census.** The cursor drives `menu_selected` and
/// `popup_layout` keeps the selected row visible, so a long shelf scrolls —
/// and the header is a text row *above* the first item row, which is what
/// puts it in `popup_layout`'s pinned header rather than in the scrolling
/// body. It does need a width census: `draw_row` clips vertically only.
///
/// `entries` is `(item, amount, carried, on_shelves)` per row, zipped by the
/// caller. The two figures are the row's **holdings**, not `App`'s two
/// ceilings: what the player may still move is what the keys clamp against,
/// and stating it as a column is what made the screen report a Depot's
/// shared budget as the size of the player's own pack.
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

/// The hint lines, the column header, and then the item rows.
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
        text_row("Left takes out, Right puts in; Shift for the end, Ctrl halves the gap"),
        text_row("[A] take everything  [N] clear  Enter to transfer  Esc to leave"),
        text_row(""),
    ]);
    let cols = Columns::of(game, entries);
    body.push(text_row(cols.header()));
    for (i, (item, amount, carried, on_shelves)) in entries.iter().enumerate() {
        body.push(item_row(
            cols.row(game.item_name(item), *carried, *on_shelves, *amount),
            i == selected,
        ));
    }
    body
}

/// Where the row's two figures end up once the basket is committed: the
/// amount is signed the way `handle_basket_key` signs it, so a take (positive)
/// moves units out of the container and into the pack and a put (negative)
/// moves them back.
///
/// The clamp is not defensive arithmetic against a state the keys can reach
/// — `edit_row` clamps every amount inside both holdings — it is because
/// `i64 as u32` **wraps** at either end, and a pack and a shelf that between
/// them hold more than `u32::MAX` are what a modded Depot's unbounded
/// `capacity` allows. The one thing a slip must not do is draw four billion
/// units.
fn projected(carried: u32, on_shelves: u32, amount: i64) -> (u32, u32) {
    let cell = |n: i64| n.clamp(0, u32::MAX as i64) as u32;
    (
        cell(carried as i64 + amount),
        cell(on_shelves as i64 - amount),
    )
}

/// How wide each of this screen's three columns runs: the item name, then the
/// two figures after it.
///
/// **Measured from the rows actually listed rather than fixed**, so a shelf
/// of short names draws a narrow table instead of a wide one full of empty
/// space. Every row is built in one pass over `entries`, so the widths cannot
/// shift under a row while the screen is open — typing an amount rebuilds the
/// whole body, and a wider figure widens the column for every row at once
/// rather than knocking one out of line.
///
/// **Each column is at least as wide as its own heading**, or the header
/// would be the thing that overhangs the table.
///
/// A name longer than the column is not truncated: it pushes its own figures
/// right and leaves the rest of the table alone. Losing characters off an
/// item's name to keep a column straight is the worse of the two failures,
/// and `no_transfer_row_overflows_its_popup` is what says the shipped set has
/// room for the widest of them.
struct Columns {
    name: usize,
    you: usize,
    container: usize,
}

impl Columns {
    fn of(game: &Game, entries: &[(ItemId, i64, u32, u32)]) -> Self {
        let mut cols = Columns {
            name: HEADINGS[0].len(),
            you: HEADINGS[1].len(),
            container: HEADINGS[2].len(),
        };
        for (item, amount, carried, on_shelves) in entries {
            let (you, container) = projected(*carried, *on_shelves, *amount);
            cols.name = cols.name.max(game.item_name(item).chars().count());
            cols.you = cols.you.max(you.to_string().len());
            cols.container = cols.container.max(container.to_string().len());
        }
        cols
    }

    /// Where each column ends, as a count of chars into a line built by
    /// `line`. The header's own boundaries sit `HEADER_LEAD` further along,
    /// which is the whole of the offset the header has to carry.
    ///
    /// The **one** definition of the table's geometry: `line` lays every cell
    /// against it, and the two alignment censuses measure the drawn strings at
    /// exactly these offsets. Spelling the edges a second time in a test is
    /// how a table drifts from the ruler that is supposed to be checking it.
    fn boundaries(&self) -> [usize; 3] {
        let gap = COLUMN_GAP.len();
        let name = self.name;
        let you = name + gap + self.you;
        [name, you, you + gap + self.container]
    }

    /// Three cells laid against `boundaries`: the name left-aligned, the two
    /// figures right-aligned so their digits line up under their heading and
    /// against each other.
    ///
    /// `COLUMN_GAP` goes in **before** the padding rather than being absorbed
    /// by it, so a name wider than its column pushes its own figures right
    /// with the gap intact instead of butting straight into them. That is the
    /// one case where a row is wider than the table, and it is the reason
    /// `no_transfer_row_overflows_its_popup` measures the longest shipped name
    /// rather than a fixed width.
    fn line(&self, cells: [&str; 3]) -> String {
        let mut out = String::new();
        for (i, (cell, edge)) in cells.into_iter().zip(self.boundaries()).enumerate() {
            if i > 0 {
                out.push_str(COLUMN_GAP);
                let width = out.chars().count() + cell.chars().count();
                out.extend(std::iter::repeat_n(' ', edge.saturating_sub(width)));
            }
            out.push_str(cell);
            if i == 0 {
                let width = out.chars().count();
                out.extend(std::iter::repeat_n(' ', edge.saturating_sub(width)));
            }
        }
        out
    }

    /// The header, carrying `HEADER_LEAD` itself — see that constant.
    fn header(&self) -> String {
        format!("{HEADER_LEAD}{}", self.line(HEADINGS))
    }

    /// One item's row, drawn as the transfer would leave it: pressing Right
    /// takes units off `you` and puts them on `container`, and Left the other
    /// way. That movement is the whole of what the screen says about the
    /// basket, which is why there is no `change` column beside it.
    fn row(&self, name: &str, carried: u32, on_shelves: u32, amount: i64) -> String {
        let (you, container) = projected(carried, on_shelves, amount);
        self.line([name, &you.to_string(), &container.to_string()])
    }
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

    /// The id of the item with the longest shipped name, which is what the
    /// width census has to survive.
    fn widest_named_item(game: &Game) -> ItemId {
        game.item_defs()
            .into_iter()
            .max_by_key(|def| game.item_name(&def.id).chars().count())
            .expect("the shipped assets define items")
            .id
            .clone()
    }

    /// **The widest transfer row the shipped assets can build still fits, and
    /// so does the header over it.**
    ///
    /// `draw_row` clips a row vertically and nothing clips it horizontally,
    /// so an over-wide row is drawn off the panel in silence — taking the
    /// figures that say how far the row may move with it. The header is a
    /// text row and is clipped no more than an item row is, so it is measured
    /// here too: it is exactly as wide as the table plus its lead.
    ///
    /// The name comes from the real `ItemDb` rather than a hand-written
    /// string, which is the difference between a census and a fixture. The
    /// figures are the widest each column can print, since nothing bounds
    /// what a modded Depot's `capacity` may hold.
    #[test]
    fn no_transfer_row_overflows_its_popup() {
        let game = shipped_game();
        let item = widest_named_item(&game);
        let name = game.item_name(&item).to_string();
        // The widest either figure can print: `projected` clamps at
        // `u32::MAX`, so both columns are ten digits whatever the basket asks
        // for.
        let entries = vec![(item, u32::MAX as i64, u32::MAX, u32::MAX)];
        let cols = Columns::of(&game, &entries);
        let row = cols.row(&name, u32::MAX, u32::MAX, u32::MAX as i64);
        let header = cols.header();

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = body_room(&m);
            // What `draw_row` actually lays down for an item row: the two-cell
            // lead, then the row's own text.
            let drawn = p.measure_ui_advance(format!("  {row}"), m.font_size);
            assert!(
                drawn > 0.0,
                "the census measured nothing — the shipped set has to reach here"
            );
            assert!(
                drawn <= room,
                "the widest transfer row overflows by {:.0}px \
                 ({drawn:.0} into {room:.0}):\n{row}",
                drawn - room
            );
            let head = p.measure_ui_advance(&header, m.font_size);
            assert!(
                head <= room,
                "the header over the widest table overflows by {:.0}px:\n{header}",
                head - room
            );
        });
    }

    /// **Every row's columns land on the same x, whatever its name is
    /// worth.** That is the whole of the table: the cells are padded to a
    /// width measured across the screen, and the UI face is monospace, so two
    /// rows agree only if every cell before a boundary measures the same.
    /// Drop the padding and the figures step in and out with the names above
    /// them.
    ///
    /// Measured rather than asserted on the string, because the advance is
    /// what `draw_row` actually places against.
    #[test]
    fn every_rows_columns_line_up() {
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

        // Figures of different widths too, or the columns would line up by
        // luck rather than by being ones.
        let entries = vec![(short.0, 5i64, 7u32, 9u32), (long.0, -1234i64, 99u32, 1u32)];
        let rows = body_rows(&game, &entries, None, 0);
        let items: Vec<String> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 2, "one row per entry");
        assert_eq!(
            items[0].chars().count(),
            items[1].chars().count(),
            "the rows are ragged: {items:?}"
        );

        let cols = Columns::of(&game, &entries);
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // `draw_row`'s own label for an unselected, untagged, iconless
            // row — the string the painter is handed.
            let at = |text: &str, upto: usize| {
                let cut: String = format!("  {text}").chars().take(2 + upto).collect();
                p.measure_ui_advance(&cut, m.font_size)
            };
            for edge in cols.boundaries() {
                let (a, b) = (at(&items[0], edge), at(&items[1], edge));
                assert!(
                    (a - b).abs() < 0.5,
                    "column edge {edge} steps with the name: {a} against {b}\n{items:?}"
                );
            }
        });
    }

    /// **The header sits over the columns it names.**
    ///
    /// The trap is the lead: `draw_row` opens every `Row::Item` label with
    /// `"  "` (or `"> "` when selected) and a `Row::Text` gets nothing, so a
    /// header built from the same padding as a row lands two cells left of
    /// the table it heads. `Columns::header` carries the lead itself.
    ///
    /// Measured rather than compared as strings, because the advance is what
    /// the painter places against.
    #[test]
    fn the_header_sits_over_the_columns_it_names() {
        let game = shipped_game();
        let entries = vec![
            (ItemId::from("core_fragment"), -12i64, 7u32, 300u32),
            (ItemId::from("power_cell"), 4i64, 1u32, 9u32),
        ];
        let rows = body_rows(&game, &entries, None, 0);
        let header = rows
            .iter()
            .filter_map(|r| match r {
                Row::Text(t) => Some(t.clone()),
                _ => None,
            })
            .find(|t| t.contains(HEADINGS[2]))
            .expect("the body carries a column header");
        let items: Vec<String> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 2, "one row per entry");

        let cols = Columns::of(&game, &entries);
        with_painter(|p| {
            let m = ui_metrics(900.0);
            for edge in cols.boundaries() {
                let head: String = header
                    .chars()
                    .take(HEADER_LEAD.chars().count() + edge)
                    .collect();
                let at_header = p.measure_ui_advance(&head, m.font_size);
                for row in &items {
                    let cell: String = format!("  {row}").chars().take(2 + edge).collect();
                    let at_row = p.measure_ui_advance(&cell, m.font_size);
                    assert!(
                        (at_header - at_row).abs() < 0.5,
                        "column edge {edge} is at {at_header} on the header \
                         and {at_row} on a row\n{header}\n  {row}"
                    );
                }
            }
        });
    }

    /// **The two columns are where the units would end up, and that is the
    /// whole of what the screen says about the basket.**
    ///
    /// The `change` column this replaced stated the same movement a second
    /// time; with it gone, a row that draws its raw holdings whatever the
    /// player has asked for is a screen with no feedback at all — the keys
    /// would move a number nothing on the page shows.
    #[test]
    fn the_columns_move_the_units_the_basket_is_holding() {
        assert_eq!(projected(7, 9, 0), (7, 9), "an untouched row is holdings");
        assert_eq!(projected(7, 9, 4), (11, 5), "a take fills the pack");
        assert_eq!(projected(7, 9, -3), (4, 12), "a put fills the container");
    }

    /// Neither figure wraps at the ends `u32` runs out at. `edit_row` cannot
    /// reach either, but `i64 as u32` wraps silently and a column reading
    /// four billion units is the failure that would result.
    #[test]
    fn a_projected_figure_is_clamped_rather_than_wrapped() {
        assert_eq!(projected(0, 0, -1), (0, 1));
        assert_eq!(projected(u32::MAX, 0, 1), (u32::MAX, 0));
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
