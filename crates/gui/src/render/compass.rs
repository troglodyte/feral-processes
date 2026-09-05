//! The destination picker: where the run's known places lie, and which one
//! the compass is pointing at.

use feral_processes_engine::CompassRow;
use feral_processes_engine::settlements::{CompassTarget, SettlementKind};

use super::popup::*;
use super::*;

/// What a destination reads as, in one place — the picker's row and the map
/// strip are the same sentence, and a second formatter is how the two would
/// come to disagree about a place.
///
/// One form, and no branch. The tier lives entirely in `label` — a row the
/// party has not reached says `a settlement` where a reached one says
/// `Lowport`, and both carry a heading and a figure — so there is nothing
/// here for this function to decide.
pub(in crate::render) fn destination_line(row: &CompassRow) -> String {
    format!("{} · {} · {}", row.label, row.bearing, row.distance)
}

/// One row per `CompassRow`, in the order the engine hands them over — home,
/// then settlements, then Stack entrances, each group nearest-first.
///
/// The tiering is read and never re-decided: `label` already withholds a
/// town's name until the party has walked to it. A renderer that decided
/// that for itself would be a second statement of the rule that the strip on
/// the map could then disagree with.
fn compass_rows(
    rows: &[CompassRow],
    selected: Option<CompassTarget>,
    highlight: usize,
) -> Vec<Row> {
    if rows.is_empty() {
        return vec![text_row(
            "Nothing known out here. The compass reads the zone surface only.",
        )];
    }
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let mark = if Some(row.target) == selected {
                " «"
            } else {
                ""
            };
            let text = format!("{}{mark}", destination_line(row));
            let (glyph, color) = target_glyph(row.target);
            with_icon(item_row(text, i == highlight), glyph, color)
        })
        .collect()
}

/// The glyph the zone map draws for a target, in the hue
/// `hud::palette::glyph` gives it — so a place reads as the same thing on
/// the map and on this screen.
///
/// A settlement's kind is not on the row, so both kinds take the Mainframe's
/// `M`: the picker is about *where*, and the scale cue is something the map
/// already carries at the tile itself.
fn target_glyph(target: CompassTarget) -> (char, Color) {
    match target {
        CompassTarget::Home => ('#', hud::palette::glyph(GlyphColor::Gray)),
        CompassTarget::Town(_) => (
            SettlementKind::Mainframe.glyph(),
            hud::palette::glyph(GlyphColor::Orange),
        ),
        CompassTarget::Link(_) => ('>', hud::palette::glyph(GlyphColor::Magenta)),
    }
}

pub(super) fn draw_compass(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let targets = game.compass_targets();
    let pointing = game.compass_bearing().map(|r| r.target);
    let mut rows = compass_rows(&targets, pointing, selected);
    rows.push(text_row(""));
    rows.push(text_row(
        "Somewhere you have been gives its name; somewhere only recorded is \
         still a heading and a distance away.",
    ));
    rows.push(text_row(
        "Up/Down to scroll, Enter to point the compass, X to clear it, Esc to close.",
    ));
    draw_popup("Compass", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &str, distance: i32, visited: bool) -> CompassRow {
        CompassRow {
            target: CompassTarget::Town(feral_processes_engine::settlements::SettlementKey {
                rx: 0,
                ry: 0,
            }),
            label: label.to_string(),
            bearing: "south",
            distance,
            visited,
        }
    }

    fn text_of(row: &Row) -> String {
        match row {
            Row::Item { text, .. } => text.clone(),
            Row::Text(t) => t.clone(),
            _ => String::new(),
        }
    }

    #[test]
    fn the_screen_draws_one_row_per_target() {
        let targets = vec![
            row("home", 4, true),
            row("a settlement", 143, false),
            row("Lowport", 219, true),
        ];
        assert_eq!(compass_rows(&targets, None, 0).len(), targets.len());
    }

    /// The one tier, as it reaches the eye. Both rows are navigable — a
    /// heading and a figure each — and the only thing reaching a place buys
    /// is being able to call it by name.
    #[test]
    fn every_row_is_navigable_and_only_the_name_is_earned() {
        let rows = compass_rows(
            &[row("Lowport", 219, true), row("a settlement", 143, false)],
            None,
            0,
        );
        let reached = text_of(&rows[0]);
        assert!(
            reached.contains("Lowport") && reached.contains("219"),
            "{reached:?}"
        );

        let unreached = text_of(&rows[1]);
        assert!(unreached.contains("a settlement"), "{unreached:?}");
        assert!(unreached.contains("south"), "{unreached:?}");
        assert!(
            unreached.contains("143"),
            "an unreached place still says how far off it is — a bearing with \
             no figure is a direction to wander in: {unreached:?}"
        );
    }

    /// Which row the compass is *pointing at* is not the same as which row
    /// the highlight is on, and the screen has to say both.
    #[test]
    fn the_pointed_at_row_is_marked_wherever_the_highlight_is() {
        let target = row("Lowport", 219, true).target;
        let rows = compass_rows(&[row("Lowport", 219, true)], Some(target), 0);
        assert!(text_of(&rows[0]).contains('«'), "{:?}", text_of(&rows[0]));

        let rows = compass_rows(&[row("Lowport", 219, true)], None, 0);
        assert!(!text_of(&rows[0]).contains('«'));
    }

    #[test]
    fn an_empty_compass_says_so_rather_than_drawing_nothing() {
        let rows = compass_rows(&[], None, 0);
        assert_eq!(rows.len(), 1);
        assert!(!text_of(&rows[0]).is_empty());
    }
}
