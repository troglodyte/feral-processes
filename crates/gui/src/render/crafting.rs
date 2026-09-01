//! The recipe picker and its quantity prompt.

use super::popup::*;
use super::*;
use feral_processes_engine::tuning::{QUALITY_CAREFUL_BONUS, QUALITY_CAREFUL_COST_PERCENT};

pub(super) fn draw_craft_menu(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let status = game.player_status();
    let recipes = game.craft_recipes();
    let mut rows = vec![
        text_row("Esc to cancel; Up/Down + Enter also work"),
        text_row(""),
    ];
    for (i, recipe) in recipes.iter().enumerate() {
        let (lead, tag, power, lines) = craft_rows(game, recipe, i, &status.inventory);
        let mut lines = lines.into_iter();
        let head = lines.next().expect("craft_rows always emits the head line");
        rows.push(with_tag(
            item_row(head, i == selected),
            lead,
            tag,
            // A recipe's result, not a copy: what it will compile at is a roll
            // the player has not made yet.
            None,
            power,
        ));
        // Dim, unselected continuations, the same shape `draw_recipes` gives a
        // product and its steps: the highlight belongs on the line carrying the
        // shortcut, and the popup's scroll anchor is the *first* selected Item,
        // so these cannot disturb it.
        for line in lines {
            rows.push(colored_item_row(line, false, TEXT_DIM));
        }
    }
    draw_popup("Compile", PopupSize::Large, &rows, refusal, painter, m);
}

/// What a wrapped cost line is indented by: the width of `"[x] MOD  "`, so the
/// continuation sits under the item's name rather than under its shortcut.
const CRAFT_ROW_INDENT: &str = "         ";

/// One recipe's row: its kind, its name, what it does, and what it costs.
///
/// The kind leads in a column of its own, the same two-space column and the
/// same `Game::item_category` call the inventory list and a trader's shelf
/// print — the tag is what says whether the thing you are about to compile
/// goes on your back or into a machine, and `item_blurb` deliberately no
/// longer names a slot now that this column does.
///
/// The column reads as a heading for the run of rows beneath it rather than
/// as noise repeated at random, which is `Game::craft_recipes`' job: it hands
/// the list back already grouped by category.
///
/// A row past `ROW_WRAP_COLUMNS` sheds its cost list onto continuation lines
/// rather than running off the popup, which `draw_row` would otherwise let it
/// do — it clamps a row vertically and nothing clamps it horizontally. The
/// split is at the ` - ` seam rather than a word wrap of the whole row, so the
/// shortcut, kind and name stay together on the line the eye scans.
///
/// Returns the lines rather than drawing them so the width they reach is
/// measurable without a window — see
/// `a_row_too_wide_for_the_popup_wraps_instead_of_running_off`. The head line
/// is always present, so a caller may take it unconditionally.
fn craft_rows(
    game: &Game,
    recipe: &CraftRecipe,
    i: usize,
    inventory: &[InventoryRow],
) -> (String, &'static str, PowerCell, Vec<String>) {
    let blurb = game
        .item_blurb(&recipe.result)
        .map(|b| format!(" ({b})"))
        .unwrap_or_default();
    let lead = row_lead(menu_shortcut(i), None);
    let tag = game.item_category(&recipe.result).short_label();
    // What a plain copy of the result rates — the roll has not been made, so
    // this is what compiling one is worth before quality moves it.
    let power = PowerCell::of_item(game, &recipe.result);
    let head = format!("{}{}", game.item_name(&recipe.result), blurb);
    let cost = cost_display(game, &recipe.cost, inventory).join(", ");

    // Measured with the tag column joined back on, not without it: the row
    // that has to fit is the one the screen draws, column and all.
    let one_line = format!("{head} - {cost}");
    let lines = if tagged_text(&lead, tag, power, &one_line).chars().count() <= ROW_WRAP_COLUMNS {
        vec![one_line]
    } else {
        std::iter::once(head)
            .chain(
                wrap_text(&cost, ROW_WRAP_COLUMNS - CRAFT_ROW_INDENT.len())
                    .into_iter()
                    .map(|line| format!("{CRAFT_ROW_INDENT}{line}")),
            )
            .collect()
    };
    (lead, tag, power, lines)
}

/// The quantity page's key line, and the widest row that page can draw.
///
/// A named constant so the width test measures the string the screen
/// actually prints: `draw_row` clips a row vertically and never
/// horizontally, so a footer past the popup body is lost in silence.
const CRAFT_QUANTITY_KEYS: &str =
    "[C] Careful   [F] Compile 5   [M] Compile max affordable   Esc to go back";

/// The arrow line, named for `CRAFT_QUANTITY_KEYS`' reason — it is the only
/// other row on this page that can grow by a word, and the width test
/// measures both.
const CRAFT_QUANTITY_ARROWS: &str =
    "Left/Right adjust  ·  Shift jumps to the end  ·  Ctrl halves the gap";

/// The quantity prompt, and the careful-compile toggle that decides what
/// the batch is worth.
///
/// Every figure on it is asked *at the price the batch will be charged* —
/// the cost line and the max-affordable line both take `careful`, because a
/// screen quoting the plain price beside a careful compile is the exact
/// disagreement `Game::craft_cost` exists to prevent.
pub(super) fn draw_craft_quantity(
    game: &mut Game,
    pending: Option<ItemId>,
    quantity: u32,
    careful: bool,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(result) = pending else { return };
    let status = game.player_status();
    let cost = game.craft_cost(&result, careful);
    let mut rows = vec![
        text_row(format!("Compile how many {}?", game.item_name(&result))),
        text_row(""),
    ];
    if !cost.is_empty() {
        let cost = cost_display(game, &cost, &status.inventory);
        rows.push(text_row(format!("Cost per unit: {}", cost.join(", "))));
        rows.push(text_row(""));
    }
    rows.push(text_row(format!("Quantity: {quantity}")));
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "Max affordable right now: {}",
        game.max_craftable(&result, careful)
    )));
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "Careful compile: {}  (+{} quality, +{}% materials)",
        if careful { "on" } else { "off" },
        QUALITY_CAREFUL_BONUS,
        QUALITY_CAREFUL_COST_PERCENT
    )));
    rows.push(text_row(""));
    rows.push(text_row(CRAFT_QUANTITY_ARROWS));
    rows.push(text_row("Type digits, Enter to compile"));
    rows.push(text_row(CRAFT_QUANTITY_KEYS));
    draw_popup("Compile", PopupSize::Large, &rows, refusal, painter, m);
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
pub(super) fn draw_recipes(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
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
    draw_popup("Recipes", PopupSize::Large, &rows, refusal, painter, m);
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
                "{inputs:in_w$} -> {maker:maker_w$} -> {}",
                output_text(step)
            )
        })
        .flat_map(wrap_step_line)
        .collect()
}

/// What a wrapped step line's continuations are indented by — past the two
/// spaces every step line already carries, so a continuation reads as part of
/// the step above rather than as a step of its own.
const STEP_ROW_INDENT: &str = "      ";

/// A step line, indented, and split if it would otherwise run off the popup.
///
/// A step that fits is returned untouched, which is what preserves the arrow
/// alignment `step_rows` pads for: `wrap_text` splits on whitespace and so
/// collapses that padding, and a row wide enough to need wrapping has lost the
/// alignment anyway. So the cost of wrapping falls only on the row that needs
/// it, and every shipped chain today takes the first branch.
fn wrap_step_line(line: String) -> Vec<String> {
    const PREFIX: &str = "  ";
    if line.chars().count() + PREFIX.len() <= ROW_WRAP_COLUMNS {
        return vec![format!("{PREFIX}{line}")];
    }
    let mut wrapped = wrap_text(&line, ROW_WRAP_COLUMNS - STEP_ROW_INDENT.len()).into_iter();
    let first = wrapped.next().unwrap_or_default();
    std::iter::once(format!("{PREFIX}{first}"))
        .chain(wrapped.map(|rest| format!("{STEP_ROW_INDENT}{rest}")))
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
    use feral_processes_engine::{DifficultyMode, RecipeInput};

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

    /// The lines `draw_craft_menu` actually draws for a recipe: its head with
    /// the tag column joined back on, then its continuations.
    ///
    /// A width test that measured the head as `craft_rows` returns it would
    /// measure a row the screen never draws — the column is a reserved slot in
    /// the drawn label, so it counts. `item_text` is the one join, so what
    /// this measures cannot drift from what `draw_row` paints.
    fn drawn_craft_lines(
        game: &Game,
        recipe: &CraftRecipe,
        i: usize,
        inventory: &[InventoryRow],
    ) -> Vec<String> {
        let (lead, tag, power, mut lines) = craft_rows(game, recipe, i, inventory);
        lines[0] = tagged_text(&lead, tag, power, &lines[0]);
        lines
    }

    /// The kind column, and the fact that it comes from the same
    /// `Game::item_category` call the inventory and trade screens print — a
    /// second derivation here is how one screen ends up calling a Module a
    /// Weapon.
    #[test]
    fn every_compile_row_leads_with_its_items_category() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let recipes = game.craft_recipes();
        let status = game.player_status();
        assert!(!recipes.is_empty(), "a fresh game can compile something");

        for (i, recipe) in recipes.iter().enumerate() {
            let lines = drawn_craft_lines(&game, recipe, i, &status.inventory);
            let tag = game.item_category(&recipe.result).short_label();
            assert!(
                lines[0].contains(&format!("] {tag}  ")),
                "a Compile row should name its item's kind in a column of its \
                 own, as the inventory does: {}",
                lines[0]
            );
        }
    }

    /// A chain whose one step has more, and longer-named, ingredients than any
    /// shipped recipe. Built rather than named off the assets for
    /// `a_very_wide_recipe`'s reason: the shipped chains all fit today, so the
    /// wrap on this screen is defensive, and a fixture that merely tracked the
    /// current widest would stop testing the wrap the moment the assets moved.
    fn a_very_wide_chain() -> RecipeChain {
        RecipeChain {
            product: "Overwrought Assembly".to_string(),
            description: None,
            steps: vec![RecipeStep {
                inputs: (0..4)
                    .map(|n| RecipeInput {
                        item: format!("Rather Long Ingredient Name {n}"),
                        qty: 20,
                        source: Some(format!("Some Extractor With A Long Name {n}")),
                    })
                    .collect(),
                maker: Some("Overwrought Assembly Bench".to_string()),
                output: "Overwrought Assembly".to_string(),
                output_qty: Some(1),
            }],
        }
    }

    /// The Recipes screen's half of the same rule. Its step lines are arrow-
    /// aligned columns, so an over-wide one wraps with a hanging indent rather
    /// than splitting at a seam the way a Compile row does — the alignment is
    /// what the screen is for, and a continuation indented under the row it
    /// belongs to keeps it readable as one step.
    #[test]
    fn a_step_line_too_wide_for_the_popup_wraps_instead_of_running_off() {
        let chain = a_very_wide_chain();
        let lines = chain_rows(&chain);
        assert!(
            lines.len() > 1,
            "a step this wide should have been split: {lines:?}"
        );

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &lines {
                let drawn = p.measure_ui_advance(&format!("  {line}"), m.font_size);
                assert!(
                    drawn <= room,
                    "a wrapped step line still overflows by {:.0}px \
                     ({drawn:.0} into {room:.0}):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The quantity page has no wrap and no scroll — every row is a
    /// `Row::Text` printed as authored — so its widest line has to fit the
    /// popup body by construction. `[C]` pushed the key line out by a word.
    #[test]
    fn the_compile_quantity_keys_fit_their_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in [CRAFT_QUANTITY_KEYS, CRAFT_QUANTITY_ARROWS] {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a key line overflows by {:.0}px ({drawn:.0} into \
                     {room:.0}):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// A recipe whose cost list runs past `ROW_WRAP_COLUMNS`. Built rather than
    /// named off the shipped assets so it cannot go stale, and so the wrap is
    /// asserted against something wider than anything shipped: the ids resolve
    /// to no `ItemDef`, so `Game::item_name` falls back to the id itself, which
    /// is exactly the long-name case a mod can produce.
    fn a_very_wide_recipe() -> CraftRecipe {
        CraftRecipe {
            requires_structure: None,
            result: ItemId::from("singularity_matrix"),
            cost: (0..5)
                .map(|n| {
                    (
                        ItemId::from(format!("some_rather_long_ingredient_id_{n}")),
                        20,
                    )
                })
                .collect(),
        }
    }

    /// The wrap itself: an over-wide row moves its cost list onto continuation
    /// lines, and *every* line it produces fits the popup. Asserted on the
    /// lines rather than on a count, so a wrap that merely split the row
    /// somewhere and still overflowed would fail here.
    #[test]
    fn a_row_too_wide_for_the_popup_wraps_instead_of_running_off() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let lines = drawn_craft_lines(&game, &a_very_wide_recipe(), 0, &[]);

        assert!(
            lines.len() > 1,
            "a row this wide should have been split: {lines:?}"
        );
        assert!(
            lines[0].starts_with("[1] MOD") && lines[0].contains("  Singularity Matrix"),
            "the shortcut, kind, rating and name are what the eye scans, so they \
             stay together on the first line: {}",
            lines[0]
        );

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &lines {
                let drawn = p.measure_ui_advance(&format!("  {line}"), m.font_size);
                assert!(
                    drawn <= room,
                    "a wrapped line still overflows by {:.0}px \
                     ({drawn:.0} into {room:.0}):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The other half of a conditional wrap, and the half a threshold set too
    /// low would break silently: a row that fits is left alone. Every recipe a
    /// fresh game offers is comfortably inside the limit, so any of them
    /// splitting means the threshold has been mis-set.
    #[test]
    fn a_row_that_fits_is_left_on_one_line() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let status = game.player_status();
        for (i, recipe) in game.craft_recipes().iter().enumerate() {
            let lines = drawn_craft_lines(&game, recipe, i, &status.inventory);
            assert_eq!(
                lines.len(),
                1,
                "no starter recipe is wide enough to need wrapping: {lines:?}"
            );
        }
    }

    /// `draw_row` clamps a row vertically and nothing clamps it horizontally,
    /// so a row past the popup's edge runs off it rather than wrapping — which
    /// is what `craft_rows` now prevents.
    ///
    /// This measures the shipped recipes a fresh game offers. The rest are
    /// gated behind a bench, research or both, and a gui test can reach
    /// neither: `place_structure` wants build materials and `unlock_research`
    /// wants banked Research Data, and both live in the engine's private
    /// `World`. `a_row_too_wide_for_the_popup_wraps_instead_of_running_off`
    /// is what covers them, by building a row wider than any of them.
    #[test]
    fn the_widest_compile_row_fits_the_popup_it_is_drawn_in() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let status = game.player_status();
        let widest = game
            .craft_recipes()
            .iter()
            .enumerate()
            .flat_map(|(i, r)| drawn_craft_lines(&game, r, i, &status.inventory))
            .map(|line| format!("  {line}"))
            .max_by_key(|line| line.chars().count())
            .expect("the shipped assets declare recipes");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let popup_w = 1440.0 * 0.88;
            let drawn = p.measure_ui_advance(&widest, m.font_size);
            let room = popup_w - m.pad * 2.0;
            assert!(
                drawn <= room,
                "the widest Compile row overflows its popup by {:.0}px \
                 ({drawn:.0} drawn into {room:.0} of room):\n{widest}",
                drawn - room
            );
        });
    }
}
