//! One Depot's allow/deny list: what this shelf will take in, a row per
//! item in the catalogue.
//!
//! The table reads `item | denied | allowed | held`, and that order is the
//! screen's only instruction: **an arrow moves the row toward the column it
//! points at**, which is the transfer picker's own convention read one
//! screen further in.

use super::popup::*;
use super::*;
use feral_processes_app_core::DepotFilterScreen;
use feral_processes_engine::DepotFilterView;

/// The lead `draw_row` puts in front of every `Row::Item` label. The column
/// header is a `Row::Text` and gets none, so it carries this itself or the
/// heading sits two cells left of the table it names — `render/transfer.rs`
/// documents the same hazard.
const HEADER_LEAD: &str = "  ";

/// What separates one column from the next, spelled in whole monospace
/// cells so the header and the figures under it land on the same x.
const COLUMN_GAP: &str = "  ";

/// The mark that sits in whichever of the two standing columns applies.
const MARK: &str = "[x]";

const HEADINGS: [&str; 4] = ["item", "denied", "allowed", "held"];

/// The width of every column but the first, which is measured from the rows.
///
/// The two standing columns are fixed because their only content is `MARK`,
/// so nothing a mod adds can widen them; `held` is measured, since nothing
/// bounds what a modded Depot's `capacity` may hold.
const DENIED_W: usize = 6;
const ALLOWED_W: usize = 7;

pub(super) fn draw_depot_filter(
    screen: Option<&DepotFilterScreen>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let body = match screen {
        Some(screen) => body_rows(screen, selected),
        // Unreachable through `App`, which closes the screen the moment the
        // Depot stops answering — drawn rather than asserted because a
        // renderer that panics takes the window with it.
        None => vec![text_row("There is no Depot here.")],
    };
    draw_popup("Depot filter", PopupSize::Large, &body, refusal, painter, m);
}

/// The header lines, the column header, and then one row per item.
///
/// The `Tab` line is drawn only when there is somewhere to go: up to four
/// Depots can be orthogonally adjacent, and a key offered beside a single
/// shelf reads as doing nothing.
fn body_rows(screen: &DepotFilterScreen, selected: usize) -> Vec<Row> {
    let view = &screen.view;
    let (x, y) = view.tile;
    let mut body = vec![text_row(format!(
        "Depot at ({x}, {y}) — room for {} more",
        view.room
    ))];
    if screen.of > 1 {
        body.push(text_row(format!(
            "One of {} beside you; Tab for the next",
            screen.of
        )));
    }
    body.extend([
        text_row("Up/Down pick a row; Left denies it, Right allows it"),
        text_row("[A] allow all  [D] deny all  Esc back to the transfer screen"),
        text_row("Denying takes nothing off the shelf — it only stops more arriving"),
        text_row(""),
    ]);
    let cols = Columns::of(view);
    body.push(text_row(cols.header()));
    for (i, row) in view.rows.iter().enumerate() {
        body.push(item_row(
            cols.row(&row.name, row.allowed, row.held),
            i == selected,
        ));
    }
    body
}

/// The two measured widths. The other two are `DENIED_W` and `ALLOWED_W`.
///
/// **Each is at least as wide as its own heading**, or the header is the
/// thing that overhangs the table. A name longer than its column pushes its
/// own cells right and leaves the rest of the table alone, rather than
/// being truncated — losing characters off an item's name to keep a column
/// straight is the worse of the two failures, and
/// `no_depot_filter_row_overflows_its_popup` is what says the shipped set
/// has room for the widest of them.
struct Columns {
    name: usize,
    held: usize,
}

impl Columns {
    fn of(view: &DepotFilterView) -> Self {
        let mut cols = Columns {
            name: HEADINGS[0].len(),
            held: HEADINGS[3].len(),
        };
        for row in &view.rows {
            cols.name = cols.name.max(row.name.chars().count());
            cols.held = cols.held.max(row.held.to_string().len());
        }
        cols
    }

    /// Four cells: the name left-aligned, the two marks centred under their
    /// headings, and `held` right-aligned so its digits line up.
    ///
    /// `COLUMN_GAP` goes in ahead of the padding rather than being absorbed
    /// by it, so a name wider than its column keeps the gap intact instead
    /// of butting straight into the mark beside it.
    fn line(&self, name: &str, denied: &str, allowed: &str, held: &str) -> String {
        let mut out = pad_right(name, self.name);
        out.push_str(COLUMN_GAP);
        out.push_str(&centre(denied, DENIED_W));
        out.push_str(COLUMN_GAP);
        out.push_str(&centre(allowed, ALLOWED_W));
        out.push_str(COLUMN_GAP);
        out.push_str(&pad_left(held, self.held));
        out
    }

    /// The header, carrying `HEADER_LEAD` itself — see that constant.
    fn header(&self) -> String {
        format!(
            "{HEADER_LEAD}{}",
            self.line(HEADINGS[0], HEADINGS[1], HEADINGS[2], HEADINGS[3])
        )
    }

    /// One item's row. Exactly one of the two standing columns is marked,
    /// which is what makes the pair readable as one axis rather than as two
    /// independent flags.
    fn row(&self, name: &str, allowed: bool, held: u32) -> String {
        let (denied_cell, allowed_cell) = if allowed { ("", MARK) } else { (MARK, "") };
        self.line(name, denied_cell, allowed_cell, &held.to_string())
    }
}

fn pad_right(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    out.extend(std::iter::repeat_n(
        ' ',
        width.saturating_sub(text.chars().count()),
    ));
    out
}

fn pad_left(text: &str, width: usize) -> String {
    let mut out: String =
        std::iter::repeat_n(' ', width.saturating_sub(text.chars().count())).collect::<String>();
    out.push_str(text);
    out
}

/// Centred, with the odd cell going to the right — so `[x]` sits under the
/// middle of both `denied` and `allowed` whichever of them it is in.
fn centre(text: &str, width: usize) -> String {
    let slack = width.saturating_sub(text.chars().count());
    let left = slack / 2;
    let mut out: String = std::iter::repeat_n(' ', left).collect();
    out.push_str(text);
    out.extend(std::iter::repeat_n(' ', slack - left));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DepotFilterRow, DifficultyMode, Game};

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    /// `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
    fn body_room(m: &Metrics) -> f32 {
        1440.0 * 0.88 - m.pad * 2.0
    }

    /// Every shipped item, at the widest figure a column can print.
    fn widest_view(game: &Game) -> DepotFilterView {
        let rows = game
            .item_defs()
            .into_iter()
            .map(|def| DepotFilterRow {
                name: game.item_name(&def.id).to_string(),
                item: def.id.clone(),
                held: u32::MAX,
                allowed: false,
            })
            .collect();
        DepotFilterView {
            tile: (0, 0),
            room: 0,
            rows,
        }
    }

    /// **The widest row the shipped catalogue can build still fits, and so
    /// does the header over it.**
    ///
    /// `draw_row` clips a row vertically and nothing clips it horizontally,
    /// so an over-wide row is drawn off the panel in silence — taking the
    /// two columns that say what the Depot will take with it.
    #[test]
    fn no_depot_filter_row_overflows_its_popup() {
        let game = shipped_game();
        let view = widest_view(&game);
        let cols = Columns::of(&view);
        let widest = view
            .rows
            .iter()
            .max_by_key(|r| r.name.chars().count())
            .expect("the shipped assets define items");
        let row = cols.row(&widest.name, false, u32::MAX);
        let header = cols.header();

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = body_room(&m);
            let drawn = p.measure_ui_advance(format!("  {row}"), m.font_size);
            assert!(
                drawn > 0.0,
                "the census measured nothing — the shipped set has to reach here"
            );
            assert!(
                drawn <= room,
                "the widest Depot filter row overflows by {:.0}px \
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
    /// worth.** The cells are padded to a width measured across the whole
    /// screen and the UI face is monospace, so two rows agree only if every
    /// cell before a boundary measures the same.
    #[test]
    fn a_short_name_and_a_long_one_line_their_marks_up() {
        let game = shipped_game();
        let view = widest_view(&game);
        let cols = Columns::of(&view);
        let short = view
            .rows
            .iter()
            .min_by_key(|r| r.name.chars().count())
            .expect("the shipped assets define items");
        let long = view
            .rows
            .iter()
            .max_by_key(|r| r.name.chars().count())
            .expect("the shipped assets define items");

        let a = cols.row(&short.name, true, 1);
        let b = cols.row(&long.name, true, 1);
        assert_eq!(
            a.chars().count(),
            b.chars().count(),
            "two rows of the same table must be the same width:\n{a}\n{b}"
        );
        assert_eq!(
            a.find(MARK).map(|i| a[..i].chars().count()),
            b.find(MARK).map(|i| b[..i].chars().count()),
            "and the mark must sit in the same column on both"
        );
    }

    /// Exactly one of the two standing columns carries the mark, which is
    /// what makes them one axis rather than two flags that can disagree.
    #[test]
    fn a_row_is_marked_in_exactly_one_column() {
        let cols = Columns { name: 4, held: 4 };
        for allowed in [true, false] {
            let row = cols.row("item", allowed, 0);
            assert_eq!(
                row.matches(MARK).count(),
                1,
                "allowed={allowed} drew {row:?}"
            );
        }
    }
}
