//! The perk and research pickers.

use super::popup::*;
use super::*;
use feral_processes_engine::ResearchStatus;
use feral_processes_engine::perks::{Perk, PerkDef};

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
        rows.push(colored_item_row(
            format!("    {}", def.description),
            false,
            TEXT_DIM,
        ));
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
        rows.push(colored_item_row(
            format!("    {}", node.description),
            false,
            TEXT_DIM,
        ));
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
}
