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

/// The recipe chains, read-only — every conversion a structure runs, walked
/// back to the raw inputs it bottoms out in (see `Game::recipe_chains`).
///
/// The product line is the selectable row and its steps are unselectable
/// sub-rows beneath it, which is what lets app-core scroll this screen by
/// chain without knowing how many lines a chain draws. `Row::Item` rather
/// than `Row::Text` for the sub-rows carries `draw_structures`' fix: the
/// popup body ends at the *last* Item and pins whatever follows as a footer,
/// so a step drawn as Text would stay on screen while the list scrolled past
/// the product it belongs to.
///
/// Columns are padded per chain rather than across the whole screen. A single
/// global width would push a Mining Node's one-word line out past the longest
/// row in the game, and the thing worth reading at a glance is one chain's
/// arrows lining up, not every chain's.
pub(super) fn draw_recipes(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let chains = game.recipe_chains();
    let mut rows = vec![
        text_row(format!(
            "{} conversion{} your base can run.",
            chains.len(),
            if chains.len() == 1 { "" } else { "s" }
        )),
        text_row(""),
    ];
    for (i, chain) in chains.iter().enumerate() {
        rows.push(colored_item_row(
            format!("Product: {}", chain.product),
            i == selected,
            TEXT,
        ));
        let cells: Vec<(String, &str)> = chain
            .steps
            .iter()
            .map(|s| (inputs_text(s), s.maker.as_deref().unwrap_or("by hand")))
            .collect();
        let in_w = cells
            .iter()
            .map(|(a, _)| a.chars().count())
            .max()
            .unwrap_or(0);
        let maker_w = cells
            .iter()
            .map(|(_, b)| b.chars().count())
            .max()
            .unwrap_or(0);
        for ((inputs, maker), step) in cells.iter().zip(&chain.steps) {
            rows.push(colored_item_row(
                format!("  {inputs:in_w$} -> {maker:maker_w$} -> {}", step.output),
                false,
                TEXT_DIM,
            ));
        }
        rows.push(colored_item_row("", false, TEXT_DIM));
    }
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("Recipes", PopupSize::Large, &rows, painter, m);
}

/// A step's ingredient list. An extractor has none — it is a tap, and saying
/// so beats an empty column the eye reads as a missing value.
fn inputs_text(step: &RecipeStep) -> String {
    if step.inputs.is_empty() {
        return "(nothing)".to_string();
    }
    step.inputs
        .iter()
        .map(|(name, qty)| format!("{name} x{qty}"))
        .collect::<Vec<_>>()
        .join(" + ")
}
