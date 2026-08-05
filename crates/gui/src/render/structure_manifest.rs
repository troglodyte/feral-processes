//! The inspector's single-structure sheet — what `i` opens when the thing
//! it found in the direction you pointed is a structure rather than a
//! program.
//!
//! Deliberately not a variant of `render/manifest.rs`: that screen's whole
//! body is HP, level, XP, stats and equipment, none of which a structure
//! has. What this screen shows is the `B` roster's row for one machine, and
//! it is built by *calling* the roster's own line builders rather than
//! restating them — see `building::structure_detail_lines`.

use super::building::{structure_detail_lines, structure_headline, structure_is_idle};
use super::popup::*;
use super::*;

pub(super) fn draw_structure_manifest(
    game: &mut Game,
    entity: Option<Entity>,
    painter: &Painter,
    m: &Metrics,
) {
    // A raid can bring a structure down while its sheet is open, so the
    // missing case is a message rather than an empty popup — the same
    // contract `draw_manifest` keeps for a program that has died.
    let Some(report) = entity.and_then(|e| game.structure_manifest(e)) else {
        draw_popup(
            "Structure",
            PopupSize::Small,
            &[text_row("That structure is gone. Esc to go back.")],
            painter,
            m,
        );
        return;
    };

    let mut rows = vec![Row::TextColored(
        structure_headline(&report),
        if structure_is_idle(&report) {
            YELLOW
        } else {
            TEXT
        },
    )];
    let detail = structure_detail_lines(&report);
    if detail.is_empty() {
        rows.push(text_row("  nothing staged, nobody posted"));
    }
    for (line, color) in detail {
        rows.push(Row::TextColored(line, color));
    }
    rows.push(text_row(""));
    rows.push(text_row("Any key to close."));

    draw_popup("Structure", PopupSize::Small, &rows, painter, m);
}
