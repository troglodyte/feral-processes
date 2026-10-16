//! One machine's recipe menu: what an Extractor, Factory, Assembler or
//! Printer beside the party makes.
//!
//! `render/rig_tool.rs`'s shape — a header naming the machine, the `Tab`
//! line only when there is somewhere to go, then one row per recipe the
//! player has researched.

use super::popup::*;
use super::*;
use feral_processes_app_core::MachineRecipeScreen;
use feral_processes_engine::MachineRecipeRow;

const CURRENT_MARK: &str = "  (making this)";

pub(super) fn draw_machine_recipe(
    screen: Option<&MachineRecipeScreen>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let body = match screen {
        Some(screen) => body_rows(screen, selected),
        // Unreachable through `App`, which closes the screen the moment the
        // machine stops answering — drawn rather than asserted, because a
        // renderer that panics takes the window with it.
        None => vec![text_row("There is no machine here.")],
    };
    draw_popup("Recipe", PopupSize::Large, &body, refusal, painter, m);
}

pub(super) fn draw_machine_recipe_confirm(
    screen: Option<&MachineRecipeScreen>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let body = match screen.and_then(|s| s.confirm_prompt()) {
        Some(prompt) => vec![
            text_row(prompt),
            text_row(""),
            item_row("[y] Yes", selected == 0),
            item_row("[n] No", selected == 1),
        ],
        None => vec![text_row("There is nothing to confirm.")],
    };
    draw_popup(
        "Switch recipe",
        PopupSize::Large,
        &body,
        refusal,
        painter,
        m,
    );
}

fn body_rows(screen: &MachineRecipeScreen, selected: usize) -> Vec<Row> {
    let view = &screen.view;
    let (x, y) = view.tile;
    let mut body = vec![text_row(format!("{} at ({x}, {y})", view.name))];
    if screen.of > 1 {
        body.push(text_row(format!(
            "One of {} beside you; Tab for the next",
            screen.of
        )));
    }
    body.push(text_row(match view.current {
        Some(_) => "Pick a row to switch; a switch empties what the machine holds",
        None => "Nothing set yet — the machine makes nothing until you pick a row",
    }));
    body.push(text_row("Esc to go back"));
    body.push(text_row(""));
    if view.rows.is_empty() {
        body.push(text_row(
            "You have not researched anything this machine makes.",
        ));
        return body;
    }
    for (i, row) in view.rows.iter().enumerate() {
        body.push(item_row(row_text(i, row), i == selected));
    }
    body
}

/// One recipe's row, a function of its own so the width census measures the
/// text the screen draws and not a copy of it.
fn row_text(index: usize, row: &MachineRecipeRow) -> String {
    let mut text = format!("[{}] {}", menu_shortcut(index), row.name);
    if !row.inputs.is_empty() {
        let inputs: Vec<String> = row
            .inputs
            .iter()
            .map(|(name, qty)| format!("{qty} {name}"))
            .collect();
        text.push_str(&format!("  {}", inputs.join(", ")));
    }
    text.push_str(&format!("  {}", row.pace.label()));
    if row.current {
        text.push_str(CURRENT_MARK);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DifficultyMode, Game, MachineRecipeView, RecipePace};

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    /// `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
    fn body_room(m: &Metrics) -> f32 {
        1440.0 * 0.88 - m.pad * 2.0
    }

    /// Every recipe a shipped machine lists, as the row `machine_recipe_view`
    /// would build for an Assembler (inputs from the item's own cost), so the
    /// census measures the catalogue and not a fixture.
    fn shipped_rows(game: &Game) -> Vec<MachineRecipeRow> {
        let items = game.item_defs();
        game.structure_defs()
            .iter()
            .filter_map(|d| d.machine.as_ref())
            .flat_map(|machine| machine.recipes.iter())
            .map(|recipe| MachineRecipeRow {
                item: recipe.produces.clone(),
                name: game.item_name(&recipe.produces).to_string(),
                inputs: items
                    .iter()
                    .find(|d| d.id == recipe.produces)
                    .and_then(|d| d.craftable.as_ref())
                    .map(|c| {
                        c.cost
                            .iter()
                            .map(|(item, qty)| (game.item_name(item).to_string(), *qty))
                            .collect()
                    })
                    .unwrap_or_default(),
                pace: RecipePace::VerySlow,
                current: true,
            })
            .collect()
    }

    /// **The widest recipe row the shipped catalogue can print fits the
    /// popup.** `draw_row` never clips horizontally, so an over-wide row is
    /// drawn off the panel in silence. `hardened_shell` has the most
    /// ingredients; it is asserted to be in the census so a rename cannot
    /// quietly leave the test measuring something shorter.
    #[test]
    fn the_widest_recipe_row_fits_the_popup() {
        let game = shipped_game();
        let rows = shipped_rows(&game);
        assert!(
            rows.iter().any(|r| r.item.as_str() == "hardened_shell"),
            "the census must reach hardened_shell"
        );
        let (index, widest) = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i % 9, r))
            .max_by_key(|(i, r)| row_text(*i, r).chars().count())
            .expect("the shipped assets define craftables");
        let text = row_text(index, widest);

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = body_room(&m);
            let drawn = p.measure_ui_advance(format!("  {text}"), m.font_size);
            assert!(drawn > 0.0, "the census measured nothing");
            assert!(
                drawn <= room,
                "the widest recipe row overflows by {:.0}px:\n{text}",
                drawn - room
            );
        });
    }

    #[test]
    fn a_row_reads_name_inputs_pace_and_marks_the_current_one() {
        let row = MachineRecipeRow {
            item: ItemId::from("bytecode_block"),
            name: "Bytecode Block".to_string(),
            inputs: vec![("Core Fragment".to_string(), 4)],
            pace: RecipePace::Steady,
            current: false,
        };
        assert_eq!(
            row_text(0, &row),
            "[1] Bytecode Block  4 Core Fragment  steady"
        );
        let current = MachineRecipeRow {
            current: true,
            ..row
        };
        assert!(row_text(0, &current).ends_with(CURRENT_MARK));
    }

    #[test]
    fn an_unset_machine_says_so_and_a_set_one_warns_of_the_switch() {
        let mut screen = MachineRecipeScreen {
            machine: Entity::PLACEHOLDER,
            of: 1,
            pending: None,
            view: MachineRecipeView {
                tile: (1, 2),
                name: "Factory".to_string(),
                current: None,
                held: 0,
                rows: Vec::new(),
            },
        };
        let text = |screen: &MachineRecipeScreen| {
            body_rows(screen, 0)
                .iter()
                .map(row_label_text)
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(text(&screen).contains("Nothing set yet"));
        screen.view.current = Some(ItemId::from("bytecode_block"));
        assert!(text(&screen).contains("empties what the machine holds"));
    }
}
