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
        for line in chain_rows(chain) {
            rows.push(colored_item_row(line, false, TEXT_DIM));
        }
        rows.push(colored_item_row("", false, TEXT_DIM));
    }
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("Recipes", PopupSize::Large, &rows, painter, m);
}

/// One chain's sub-rows: what the product is *for*, then how it is made.
///
/// The steps answer "how do I make this" and nothing else on the screen
/// answers "why would I", so the product's own authored prose leads — the
/// same text the inventory's describe page shows, wrapped the same way, so
/// the two cannot describe one item differently.
///
/// A chain whose product has no description (a mod leaving the field blank)
/// simply opens on its steps.
fn chain_rows(chain: &RecipeChain) -> Vec<String> {
    chain
        .description
        .iter()
        .flat_map(|text| wrap_text(text, DESCRIBE_WRAP_COLUMNS))
        .map(|line| format!("  {line}"))
        .chain(step_rows(chain))
        .collect()
}

/// One chain's step lines, arrow columns aligned.
///
/// Columns are padded per chain rather than across the whole screen. A single
/// global width would push a Mining Node's one-word line out past the longest
/// row in the game, and the thing worth reading at a glance is one chain's
/// arrows lining up, not every chain's.
///
/// Split out of `draw_recipes` so the width these rows reach is measurable
/// without a window — `draw_row` clamps a row vertically and nothing clamps
/// it horizontally, so an over-wide row runs off the popup and takes the
/// product of the deepest chain in the game with it. See
/// `the_widest_recipe_row_fits_the_popup_it_is_drawn_in`, which measures
/// `chain_rows` rather than this, so the description lines are held to the
/// same edge as the steps they sit above.
fn step_rows(chain: &RecipeChain) -> Vec<String> {
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
    cells
        .iter()
        .zip(&chain.steps)
        .map(|((inputs, maker), step)| {
            format!(
                "  {inputs:in_w$} -> {maker:maker_w$} -> {}",
                output_text(step)
            )
        })
        .collect()
}

/// A step's ingredient list. An extractor has none — it is a tap, and saying
/// so beats an empty column the eye reads as a missing value.
///
/// An ingredient no recipe makes leads with the structure that taps it, so a
/// chain read top to bottom is the build order: `Mining Node (Core Fragment
/// x4) -> Lathe` is the whole answer to "what do I put down to get one".
/// Which ingredients earn that prefix is `RecipeInput::source`'s call, not
/// this function's.
fn inputs_text(step: &RecipeStep) -> String {
    if step.inputs.is_empty() {
        return "(nothing)".to_string();
    }
    step.inputs
        .iter()
        .map(|i| match &i.source {
            Some(tap) => format!("{tap} ({} x{})", i.item, i.qty),
            None => format!("{} x{}", i.item, i.qty),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// A step's product, quantified where the game will stand behind the number.
/// The suffix is what marks the column as an item rather than another
/// structure; a tap has no fixed yield to quote and so goes bare.
fn output_text(step: &RecipeStep) -> String {
    match step.output_qty {
        Some(qty) => format!("{} x{qty}", step.output),
        None => step.output.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::DifficultyMode;

    /// The chain says how to make the thing; the description is the only
    /// line saying why you would want one. Asserted as `wrap_text`'s own
    /// output against a product whose prose genuinely runs past one row, so
    /// a description printed raw would run off the popup and fail here
    /// rather than only on a wide window nobody tests on.
    #[test]
    fn a_chains_rows_open_on_its_products_description() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let chains = game.recipe_chains();
        let chain = chains
            .iter()
            .find(|c| c.product == "Core Fragment")
            .expect("the Mining Node taps them");

        let description = chain
            .description
            .as_deref()
            .expect("core_fragment.ron carries prose");
        assert!(
            description.chars().count() > DESCRIBE_WRAP_COLUMNS,
            "this product is chosen for being longer than one row: {description}"
        );
        let expected: Vec<String> = wrap_text(description, DESCRIBE_WRAP_COLUMNS)
            .into_iter()
            .map(|line| format!("  {line}"))
            .chain(step_rows(chain))
            .collect();

        assert_eq!(
            chain_rows(chain),
            expected,
            "the prose leads, wrapped, and the steps follow it unchanged"
        );
    }

    /// `draw_row` clamps a row vertically and nothing clamps it horizontally,
    /// so a Recipes row wider than its popup silently runs off the right edge
    /// — taking the product column, the one thing every row exists to name.
    ///
    /// Measured against the real shipped assets rather than a fixture: what
    /// sets the width is the longest ingredient list plus the longest
    /// structure name in the game — or a wrapped line of the longest
    /// description — and a fixture would go stale the first time any of them
    /// moved. `with_painter`'s 1440x900 is the geometry
    /// `ui_metrics` is calibrated against (`REFERENCE_HEIGHT`), so the font
    /// here is the unscaled body size.
    #[test]
    fn the_widest_recipe_row_fits_the_popup_it_is_drawn_in() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");

        let widest = game
            .recipe_chains()
            .iter()
            .flat_map(chain_rows)
            // `draw_row` prefixes every `Row::Item` with two spaces of its
            // own, which are as much of the drawn line as the text is.
            .map(|line| format!("  {line}"))
            .max_by_key(|line| line.chars().count())
            .expect("the shipped assets declare chains");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let popup_w = 1440.0 * 0.88;
            let drawn = p.measure_ui_advance(&widest, m.font_size);
            let room = popup_w - m.pad * 2.0;
            assert!(
                drawn <= room,
                "the widest Recipes row overflows its popup by {:.0}px \
                 ({drawn:.0} drawn into {room:.0} of room):\n{widest}",
                drawn - room
            );
        });
    }
}
