//! The recipe picker and its quantity prompt.

use super::popup::*;
use super::*;

pub(super) fn draw_craft_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let recipes = game.craft_recipes();
    let mut rows = vec![
        text_row("Esc to cancel; Up/Down + Enter also work"),
        text_row(""),
    ];
    for (i, recipe) in recipes.iter().enumerate() {
        let cost = cost_display(game, &recipe.cost, &status.inventory);
        let blurb = game
            .item_blurb(&recipe.result)
            .map(|b| format!(" ({b})"))
            .unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {}{} - {}",
                menu_shortcut(i),
                game.item_name(&recipe.result),
                blurb,
                cost.join(", ")
            ),
            i == selected,
        ));
    }
    draw_popup("Compile", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_craft_quantity(
    game: &mut Game,
    pending: Option<ItemId>,
    quantity_input: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(result) = pending else { return };
    let status = game.player_status();
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == result);
    let mut rows = vec![
        text_row(format!("Compile how many {}?", game.item_name(&result))),
        text_row(""),
    ];
    if let Some(recipe) = &recipe {
        let cost = cost_display(game, &recipe.cost, &status.inventory);
        rows.push(text_row(format!("Cost per unit: {}", cost.join(", "))));
        rows.push(text_row(""));
    }
    let shown = if quantity_input.is_empty() {
        "1"
    } else {
        quantity_input
    };
    rows.push(text_row(format!("Quantity: {shown}")));
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "Max affordable right now: {}",
        game.max_craftable(&result)
    )));
    rows.push(text_row(""));
    rows.push(text_row("Type digits, Enter to compile"));
    rows.push(text_row(
        "[F] Compile 5   [M] Compile max affordable   Esc to go back",
    ));
    draw_popup("Compile", PopupSize::Large, &rows, painter, m);
}
