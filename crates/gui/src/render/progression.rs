//! The perk and research pickers.

use super::popup::*;
use super::*;
use feral_processes_engine::ResearchStatus;
use feral_processes_engine::perks::{Perk, PerkDef};

/// What a description is indented by under the entry it belongs to. Four
/// columns rather than `continuation_lines`' seven: that indent is a fact
/// about `with_icon`'s glyph slot, and neither picker's rows carry an icon.
const DESCRIPTION_INDENT: &str = "    ";

/// One entry's description, wrapped to the popup and indented under the row
/// it belongs to. Both pickers print prose the same way, and the shipped
/// assets carry up to about 240 characters of it against a
/// `PopupSize::Large` body of roughly 114 — printed raw it ran off the
/// right edge in silence, since `draw_row` clamps a row vertically and
/// nothing clamps it horizontally.
///
/// `DESCRIBE_WRAP_COLUMNS` rather than `ROW_WRAP_COLUMNS` because this is
/// prose, which reads better narrow than wide — the same width the Recipes
/// screen wraps a product's description to, so the two screens cannot
/// disagree about how wide the game's prose runs.
///
/// The lines stay `Row::Item`, which is what `perks_menu_rows` documents as
/// load-bearing: `popup_layout` cuts the scrollable body at the last
/// `Row::Item`, so a description made of `Row::Text` would be torn off the
/// entry it describes and pinned to the foot of the box.
fn description_rows(description: &str) -> impl Iterator<Item = Row> + '_ {
    wrap_text(
        description,
        DESCRIBE_WRAP_COLUMNS - DESCRIPTION_INDENT.chars().count(),
    )
    .into_iter()
    .map(|line| colored_item_row(format!("{DESCRIPTION_INDENT}{line}"), false, TEXT_DIM))
}

/// The perk picker's rows. A perk's description is a *dim item row* rather
/// than a `Row::Text`, and the help line sits in the header rather than under
/// the list, for the reason `build_menu_rows` does the same: `popup_layout`
/// cuts the scrollable body at the last `Row::Item` and pins everything after
/// it, so a trailing `Row::Text` description is torn off the perk it belongs
/// to and drawn at the foot of the box, where it neither scrolls nor sits
/// under its own row. See `every_progression_description_stays_inside_the_
/// scrollable_body`.
pub(super) fn perks_menu_rows(
    points: u32,
    defs: &[PerkDef],
    held: &[Perk],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(format!("Perk Points: {points}"), CYAN),
        text_row("Pick a row's key to buy another level. Esc to close"),
        text_row(""),
    ];
    for (i, def) in defs.iter().enumerate() {
        let level = held.iter().filter(|p| **p == def.id).count();
        let tag = if level > 0 {
            format!(" (level {level})")
        } else {
            String::new()
        };
        rows.push(item_row(
            format!(
                "[{}] {} - {} Perk Points{}",
                menu_shortcut(i),
                def.name,
                def.cost,
                tag
            ),
            i == selected,
        ));
        rows.extend(description_rows(&def.description));
    }
    rows
}

pub(super) fn draw_perks_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let rows = perks_menu_rows(
        status.perk_points,
        &game.perk_defs(),
        &status.unlocked_perks,
        selected,
    );
    draw_popup("Perks", PopupSize::Large, &rows, painter, m);
}

/// What a research row says about itself after its name and price. This is
/// the only place a locked node is labelled, so a node held up by both a
/// prerequisite and a breach has to read as held up by both — the engine
/// reports the two reasons separately (`ResearchState::Locked`) precisely so
/// that neither is dropped here, and joining them is this function's whole
/// job.
///
/// A function rather than an inline `match` for the reason
/// `render/party.rs::companion_help` is one: it is the only way a test can
/// hold the string without standing a window up.
fn state_tag(state: &ResearchState) -> String {
    match state {
        ResearchState::Unlocked => " (researched)".to_string(),
        ResearchState::Available => String::new(),
        ResearchState::Locked { missing, min_zone } => {
            let mut reasons = missing.clone();
            reasons.extend(min_zone.map(|z| format!("Zone {z}")));
            format!(" (needs {})", reasons.join(", "))
        }
    }
}

/// The research picker's rows, in the shape `perks_menu_rows` documents and
/// for the same reason: nothing may follow the last `Row::Item`.
pub(super) fn research_menu_rows(held: u32, nodes: &[ResearchStatus], selected: usize) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(format!("Research Data: {held}"), CYAN),
        text_row("Pick a row's key to research it. Esc to close"),
        text_row(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let tag = state_tag(&node.state);
        let label = format!(
            "[{}] {} - {} Research Data{tag}",
            menu_shortcut(i),
            node.name,
            node.cost
        );
        // An unlocked node is kept on the list as a record of what's been
        // bought, so it has to read as spent rather than as an option.
        rows.push(match node.state {
            ResearchState::Unlocked => spent_item_row(label, i == selected),
            _ => item_row(label, i == selected),
        });
        rows.extend(description_rows(&node.description));
    }
    rows
}

pub(super) fn draw_research_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let research_currency = game.research_currency();
    let held = game.banked(&research_currency);
    let nodes = game.research_nodes();
    let rows = research_menu_rows(held, &nodes, selected);
    draw_popup("Research", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::DifficultyMode;

    /// The three questions a playtest of this screen would ask, answered
    /// without standing a window up: a gated node explains itself, a
    /// doubly-held node names both reasons, and an ungated locked node reads
    /// exactly as it did before the gate existed.
    #[test]
    fn a_locked_row_names_every_reason_it_is_locked() {
        assert_eq!(
            state_tag(&ResearchState::Locked {
                missing: Vec::new(),
                min_zone: Some(3),
            }),
            " (needs Zone 3)",
            "a node whose only obstacle is the breach has to say so — this is \
             the row that tells a player the tier is worth breaching for"
        );
        assert_eq!(
            state_tag(&ResearchState::Locked {
                missing: vec!["Neural Interfacing".to_string()],
                min_zone: Some(3),
            }),
            " (needs Neural Interfacing, Zone 3)",
            "both reasons, or clearing one leaves the row saying the same thing"
        );
        assert_eq!(
            state_tag(&ResearchState::Locked {
                missing: vec!["Automation".to_string()],
                min_zone: None,
            }),
            " (needs Automation)",
            "a bootstrap node is untouched by the gate existing"
        );
    }

    #[test]
    fn an_unlocked_row_reads_as_spent_and_an_available_one_says_nothing() {
        assert_eq!(state_tag(&ResearchState::Unlocked), " (researched)");
        assert_eq!(state_tag(&ResearchState::Available), "");
    }

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so a row wider than its popup runs off the right edge
    /// in silence. Both pickers print a description under every entry, and
    /// the prose the assets carry runs to about 240 characters against a
    /// `PopupSize::Large` body of roughly 114 — so the tail of most of them
    /// was drawn outside the box.
    ///
    /// Measured against the real shipped assets rather than a fixture, for
    /// `the_widest_recipe_row_fits_the_popup_it_is_drawn_in`'s reason: what
    /// sets the width here is whichever node or perk carries the longest
    /// prose, and a fixture would go stale the first time one moved.
    /// `with_painter`'s 1440x900 is the geometry `ui_metrics` is calibrated
    /// against, so the font here is the unscaled body size.
    #[test]
    fn the_widest_progression_row_fits_the_popup_it_is_drawn_in() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let nodes = game.research_nodes();
        let defs = game.perk_defs();
        let status = game.player_status();
        assert!(
            !nodes.is_empty() && !defs.is_empty(),
            "the shipped assets declare both a research tree and a perk list"
        );

        let screens = [
            ("Research", research_menu_rows(40, &nodes, 0)),
            (
                "Perks",
                perks_menu_rows(3, &defs, &status.unlocked_perks, 0),
            ),
        ];

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for (screen, rows) in &screens {
                let mut measured = 0usize;
                for row in rows {
                    // `draw_row` prefixes every `Row::Item` with two columns
                    // of its own for the selection caret, which are as much
                    // of the drawn line as the text is.
                    let drawn_text = match row {
                        Row::Text(text) => text.clone(),
                        Row::TextColored(text, _) => text.clone(),
                        Row::Item { text, .. } => format!("  {text}"),
                    };
                    measured += 1;
                    let drawn = p.measure_ui_advance(&drawn_text, m.font_size);
                    assert!(
                        drawn <= room,
                        "the {screen} picker overflows its popup by {:.0}px \
                         ({drawn:.0} drawn into {room:.0} of room):\n{drawn_text}",
                        drawn - room
                    );
                }
                assert!(measured > 0, "the {screen} picker drew no rows to measure");
            }
        });
    }
}
