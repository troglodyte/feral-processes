//! The perk and research pickers.

use super::popup::*;
use super::*;

pub(super) fn draw_perks_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(format!("Perk Points: {}", status.perk_points), CYAN),
        text_row(""),
    ];
    for (i, def) in game.perk_defs().iter().enumerate() {
        let level = status
            .unlocked_perks
            .iter()
            .filter(|p| **p == def.id)
            .count();
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
        rows.push(text_row(format!("    {}", def.description)));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Pick a row's key to buy another level. Esc to close",
    ));
    draw_popup("Perks", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_research_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let research_currency = game.research_currency();
    let held = game
        .player_status()
        .inventory
        .iter()
        .find(|(item, _)| *item == research_currency)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let bank_limit = game.bank_limit_of(&research_currency).unwrap_or(0);
    let nodes = game.research_nodes();
    let mut rows = vec![
        Row::TextColored(format!("Research Data: {held}/{bank_limit}"), CYAN),
        text_row(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let tag = match &node.state {
            ResearchState::Unlocked => " (researched)".to_string(),
            ResearchState::Available => String::new(),
            ResearchState::Locked { missing } => format!(" (needs {})", missing.join(", ")),
        };
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
        rows.push(text_row(format!("    {}", node.description)));
    }
    rows.push(text_row(""));
    rows.push(text_row("Pick a row's key to research it. Esc to close"));
    draw_popup("Research", PopupSize::Large, &rows, painter, m);
}
