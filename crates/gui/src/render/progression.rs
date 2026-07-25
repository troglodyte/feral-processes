//! The perk and research pickers.

use super::popup::*;
use super::*;

pub(super) fn draw_perks_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let status = game.player_status();
    let mut rows = vec![
        Row::TextColored(format!("Perk Points: {}", status.perk_points), CYAN),
        text_row(""),
    ];
    for (i, perk) in feral_processes_engine::Perk::all().iter().enumerate() {
        let level = status.unlocked_perks.iter().filter(|p| *p == perk).count();
        let tag = if level > 0 {
            format!(" (level {level})")
        } else {
            String::new()
        };
        rows.push(item_row(
            format!(
                "[{}] {} - {} Perk Points{}",
                menu_shortcut(i),
                perk.display_name(),
                perk.cost(),
                tag
            ),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", perk.description())));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Pick a row's key to buy another level. Esc to close",
    ));
    draw_popup("Perks", PopupSize::Large, &rows, fonts, m);
}

pub(super) fn draw_research_menu(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
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
        rows.push(item_row(
            format!(
                "[{}] {} - {} Research Data{tag}",
                menu_shortcut(i),
                node.name,
                node.cost
            ),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", node.description)));
    }
    rows.push(text_row(""));
    rows.push(text_row("Pick a row's key to research it. Esc to close"));
    draw_popup("Research", PopupSize::Large, &rows, fonts, m);
}
