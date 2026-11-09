//! The Recipes page: every item the base can make right now, A-Z, with what
//! it is for, what it takes and where it is made. Every figure comes from
//! `Game::recipe_chains`; this only formats.

use super::*;
use feral_processes_engine::{RecipeChain, RecipeMaker, RecipeStep};

/// The rows of the page: per entry a selectable product row, unselectable
/// sub-rows, then a blank row.
///
/// `Row::Item` rather than `Row::Text` for the sub-rows: the body ends at the
/// *last* Item and pins whatever follows as a footer, so a step drawn as Text
/// would stay on screen while the list scrolled past the product it belongs
/// to. app-core scrolls this page by entry (`recipe_chains().len()`) without
/// knowing how many lines an entry draws, which is what the sub-rows allow.
pub(super) fn recipe_rows(chains: &[RecipeChain], selected: usize) -> Vec<Row> {
    if chains.is_empty() {
        return vec![text_row("Nothing to make yet. Research unlocks recipes.")];
    }
    let mut rows = Vec::new();
    for (i, chain) in chains.iter().enumerate() {
        rows.push(colored_item_row(chain.product.clone(), i == selected, TEXT));
        for line in chain_rows(chain) {
            rows.push(Row::Detail(line, TEXT_DIM));
        }
        rows.push(Row::Detail(String::new(), TEXT_DIM));
    }
    rows
}

/// One entry's sub-rows: what the item is *for*, then how it is made.
///
/// The steps answer "how do I make this" and nothing else on the page
/// answers "why would I", so the authored prose leads, wrapped like the
/// inventory's describe page so the two cannot describe one item differently.
/// An entry with no prose (`description` is `None`) opens on its
/// `Used in:` line, or its steps.
fn chain_rows(chain: &RecipeChain) -> Vec<String> {
    chain
        .description
        .iter()
        .flat_map(|text| wrap_text(text, DESCRIBE_WRAP_COLUMNS))
        .map(|line| format!("  {line}"))
        .chain(used_in_rows(&chain.used_in))
        .chain(step_rows(chain))
        .collect()
}

/// `Used in: A, B, C`, wrapped with a hanging indent; nothing for an item no
/// unlocked recipe takes.
fn used_in_rows(used_in: &[String]) -> Vec<String> {
    if used_in.is_empty() {
        return Vec::new();
    }
    let line = format!("Used in: {}", used_in.join(", "));
    let mut wrapped = wrap_text(&line, HUB_WRAP_COLUMNS - USED_IN_INDENT.len()).into_iter();
    let first = wrapped.next().unwrap_or_default();
    std::iter::once(format!("  {first}"))
        .chain(wrapped.map(|rest| format!("  {USED_IN_INDENT}{rest}")))
        .collect()
}

/// Past `Used in: `, so a continuation reads as part of the list above it.
const USED_IN_INDENT: &str = "        ";

/// One entry's step lines, arrow columns aligned.
///
/// Columns are padded per entry rather than across the whole page. A single
/// global width would push an Extractor's one-word line out past the longest
/// row in the game, and the thing worth reading at a glance is one entry's
/// arrows lining up.
///
/// Split out of `recipe_rows` so the width these rows reach is measurable
/// without a window: `draw_row` clamps a row vertically and nothing clamps
/// it horizontally, so an over-wide row runs off the page and takes the
/// product of the deepest chain in the game with it.
fn step_rows(chain: &RecipeChain) -> Vec<String> {
    let cells: Vec<(String, String)> = chain
        .steps
        .iter()
        .map(|s| (inputs_text(s), makers_text(&s.makers)))
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
        .map(|((inputs, makers), step)| {
            format!(
                "{inputs:in_w$} -> {makers:maker_w$} -> {}",
                output_text(step)
            )
        })
        .flat_map(wrap_step_line)
        .collect()
}

/// What a wrapped step line's continuations are indented by - past the two
/// spaces every step line already carries, so a continuation reads as part of
/// the step above rather than as a step of its own.
const STEP_ROW_INDENT: &str = "      ";

/// A step line, indented, and split if it would otherwise run off the page.
///
/// A step that fits is returned untouched, which is what preserves the arrow
/// alignment `step_rows` pads for: `wrap_text` splits on whitespace and so
/// collapses that padding, and a row wide enough to need wrapping has lost the
/// alignment anyway. So the cost of wrapping falls only on the row that needs
/// it.
fn wrap_step_line(line: String) -> Vec<String> {
    const PREFIX: &str = "  ";
    if line.chars().count() + PREFIX.len() <= HUB_WRAP_COLUMNS {
        return vec![format!("{PREFIX}{line}")];
    }
    let mut wrapped = wrap_text(&line, HUB_WRAP_COLUMNS - STEP_ROW_INDENT.len()).into_iter();
    let first = wrapped.next().unwrap_or_default();
    std::iter::once(format!("{PREFIX}{first}"))
        .chain(wrapped.map(|rest| format!("{STEP_ROW_INDENT}{rest}")))
        .collect()
}

/// Every place a step can be done, comma-joined; a field recipe naming no
/// structure reads `by hand`.
fn makers_text(makers: &[RecipeMaker]) -> String {
    makers
        .iter()
        .map(|m| match m {
            RecipeMaker::Structure(name) => name.as_str(),
            RecipeMaker::ByHand => "by hand",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A step's ingredient list. An extractor has none - it is a tap, and saying
/// so beats an empty column the eye reads as a missing value.
///
/// An ingredient no recipe makes leads with the structure that taps it, so a
/// chain read top to bottom is the build order: `Extractor (Core Fragment
/// x4) -> Factory` is the whole answer to "what do I put down to get one".
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
    use super::super::fit::{assert_rows_fit, text_of};
    use super::*;
    use feral_processes_engine::{DifficultyMode, RecipeInput};

    /// A game with every shipped research node done, so the page shows the
    /// widest content the assets can produce.
    fn game_with_all_research() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let mut game =
            Game::new(7, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        for def in game.research_defs() {
            // Already-researched nodes answer Err; only the grant matters.
            let _ = game.dev_unlock_research(&def.id);
        }
        game
    }

    fn step(makers: Vec<RecipeMaker>) -> RecipeStep {
        RecipeStep {
            inputs: vec![RecipeInput {
                item: "Core Fragment".to_string(),
                qty: 3,
                source: None,
            }],
            makers,
            output: "Breach Kit".to_string(),
            output_qty: Some(1),
        }
    }

    fn chain(description: Option<&str>, used_in: &[&str], steps: Vec<RecipeStep>) -> RecipeChain {
        RecipeChain {
            product: "Breach Kit".to_string(),
            description: description.map(str::to_string),
            used_in: used_in.iter().map(|s| s.to_string()).collect(),
            steps,
        }
    }

    /// A chain whose one step has more, and longer-named, ingredients than any
    /// shipped recipe. Built rather than named off the assets: the shipped
    /// chains all fit today, so the wrap here is defensive, and a fixture that
    /// merely tracked the current widest would stop testing it the moment the
    /// assets moved.
    fn a_very_wide_chain() -> RecipeChain {
        chain(
            None,
            &[],
            vec![RecipeStep {
                inputs: (0..4)
                    .map(|n| RecipeInput {
                        item: format!("Rather Long Ingredient Name {n}"),
                        qty: 20,
                        source: Some(format!("Some Extractor With A Long Name {n}")),
                    })
                    .collect(),
                makers: vec![RecipeMaker::Structure(
                    "Overwrought Assembly Bench".to_string(),
                )],
                output: "Overwrought Assembly".to_string(),
                output_qty: Some(1),
            }],
        )
    }

    /// `draw_row` clips a row vertically and never horizontally, so a Recipes
    /// row wider than the page silently loses its right edge - the product
    /// column, the one thing every row exists to name. Measured against the
    /// real shipped assets with everything researched: what sets the width is
    /// the longest ingredient list plus its makers, the longest description or
    /// the longest `Used in:` line, and a fixture would go stale the first
    /// time any of them moved.
    #[test]
    fn the_widest_recipe_row_fits_the_page_it_is_drawn_in() {
        let game = game_with_all_research();
        let chains = game.recipe_chains();
        assert!(!chains.is_empty(), "the shipped assets declare recipes");
        assert_rows_fit(&recipe_rows(&chains, 0), "Recipes");
    }

    #[test]
    fn a_step_too_wide_for_the_page_wraps_instead_of_running_off() {
        let rows = recipe_rows(&[a_very_wide_chain()], 0);
        assert!(rows.len() > 3, "a step this wide should have been split");
        assert_rows_fit(&rows, "wrapped Recipes");
    }

    #[test]
    fn a_long_used_in_line_wraps_inside_the_page() {
        let names: Vec<String> = (0..30)
            .map(|n| format!("Rather Long Product {n}"))
            .collect();
        let used: Vec<&str> = names.iter().map(String::as_str).collect();
        let rows = recipe_rows(
            &[chain(None, &used, vec![step(vec![RecipeMaker::ByHand])])],
            0,
        );
        assert!(
            rows.iter()
                .filter(|r| text_of(r).contains("Rather"))
                .count()
                > 1,
            "thirty names should not fit one line"
        );
        assert_rows_fit(&rows, "Used in");
    }

    #[test]
    fn a_step_names_every_maker() {
        let rows = recipe_rows(
            &[chain(
                None,
                &[],
                vec![step(vec![
                    RecipeMaker::Structure("Printer".to_string()),
                    RecipeMaker::ByHand,
                ])],
            )],
            0,
        );
        assert!(
            rows.iter().any(|r| text_of(r).contains("Printer, by hand")),
            "{:?}",
            rows.iter().map(text_of).collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_entry_with_no_description_draws_no_description_line() {
        let rows = recipe_rows(
            &[chain(
                None,
                &["Honeypot"],
                vec![step(vec![RecipeMaker::ByHand])],
            )],
            0,
        );
        let texts: Vec<&str> = rows.iter().map(text_of).collect();
        assert_eq!(texts[0], "Breach Kit");
        assert_eq!(texts[1], "  Used in: Honeypot", "{texts:?}");
    }

    #[test]
    fn a_described_entry_leads_with_its_prose_then_its_uses() {
        let rows = recipe_rows(
            &[chain(
                Some("A taming catalyst."),
                &["Honeypot"],
                vec![step(vec![RecipeMaker::ByHand])],
            )],
            0,
        );
        let texts: Vec<&str> = rows.iter().map(text_of).collect();
        assert_eq!(texts[1], "  A taming catalyst.");
        assert_eq!(texts[2], "  Used in: Honeypot");
    }

    #[test]
    fn an_item_nothing_takes_has_no_used_in_line() {
        let rows = recipe_rows(
            &[chain(None, &[], vec![step(vec![RecipeMaker::ByHand])])],
            0,
        );
        assert!(!rows.iter().any(|r| text_of(r).contains("Used in")));
    }

    #[test]
    fn an_empty_list_says_research_unlocks_recipes() {
        let rows = recipe_rows(&[], 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            text_of(&rows[0]),
            "Nothing to make yet. Research unlocks recipes."
        );
    }
}
