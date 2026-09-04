//! `Mode::DownedPrograms`: the store reached from the pack, and the
//! tool-and-yield picker for one held program — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`, section
//! 6.
//!
//! One popup, two pages — `App::pending_downed_program_index` decides which
//! — the shape `draw_develop`/`draw_develop_program` take across two
//! `Mode`s collapsed into one: there is no per-tier ladder here to keep
//! apart from the list the way a Kernel Ring's page needs.

use super::popup::*;
use super::*;

pub(super) fn draw_downed_programs(
    game: &Game,
    pending_index: Option<usize>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    match pending_index {
        Some(index) => draw_extraction_options(game, index, selected, refusal, painter, m),
        None => draw_downed_program_list(game, selected, refusal, painter, m),
    }
}

/// Page one: every held program, `Game::downed_program_rows`' own order.
///
/// The row does not print `grade` — it is an unbounded internal fold of the
/// three fields already on the line (condition, rarity, level) and adds no
/// figure a player can act on, only a scaleless number. It stays on
/// `views::DownedProgramRow` for the engine's own use (and so a screen that
/// does want it later doesn't have to widen the interface).
pub(super) fn downed_program_list_rows(game: &Game, selected: usize) -> Vec<Row> {
    let programs = game.downed_program_rows();
    let mut rows = vec![text_row(
        "Downed programs held for extraction. Pick one to see what a tool would give.",
    )];
    if programs.is_empty() {
        rows.push(text_row(
            "Nothing held — a kill sometimes leaves a program behind.",
        ));
    }
    for (i, p) in programs.iter().enumerate() {
        let mut label = format!(
            "[{}] {} Lv{}  cond {}%",
            menu_shortcut(i),
            p.name,
            p.level,
            p.condition,
        );
        if let Some(tier) = p.rarity.label() {
            label.push_str(&format!("  {tier}"));
        }
        if p.boss {
            label.push_str("  (Boss)");
        }
        rows.push(tier_row(label, i == selected, 0, p.rarity));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to go back"));
    rows
}

fn draw_downed_program_list(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = downed_program_list_rows(game, selected);
    draw_popup(
        "Downed Programs",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// Page two: the program named by `index`, and every installed tool's own
/// preview yield for it — `Game::extraction_options`' order and figures,
/// never re-derived here. `Game::installed_tools` supplies the tool's
/// display name, which `extraction_options` doesn't carry (it names tools
/// by `ToolId` alone), joined by **position** rather than by looking each
/// id back up: `extraction_options` builds its `Vec` by mapping
/// `installed_tools()` in order (see its own doc), so the two are already
/// the same sequence and a positional `zip` is structural where a
/// find-by-id join would only be resting on ids happening to be unique.
pub(super) fn extraction_options_rows(game: &Game, index: usize, selected: usize) -> Vec<Row> {
    let programs = game.downed_program_rows();
    let Some(program) = programs.get(index) else {
        return vec![text_row("That program is gone.")];
    };
    let tools = game.installed_tools();
    let options = game.extraction_options(index);

    let mut rows = vec![
        text_row(format!(
            "Extracting the level {} {} (condition {}%).",
            program.level, program.name, program.condition
        )),
        text_row(""),
    ];
    if options.is_empty() {
        rows.push(text_row("No tool is installed."));
    }
    for (i, (tool, (_, yield_rows))) in tools.iter().zip(options.iter()).enumerate() {
        let yield_text = if yield_rows.is_empty() {
            "nothing usable".to_string()
        } else {
            yield_rows
                .iter()
                .map(|(item, qty)| format!("{qty} {}", game.item_name(item)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let label = format!("[{}] {}: {yield_text}", menu_shortcut(i), tool.name);
        rows.push(item_row(label, i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to go back"));
    rows
}

fn draw_extraction_options(
    game: &Game,
    index: usize,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = extraction_options_rows(game, index, selected);
    draw_popup(
        "Downed Programs",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::DifficultyMode;
    use feral_processes_engine::items::DownedProgram;
    use feral_processes_engine::save;
    use feral_processes_engine::tools::ToolId;
    use feral_processes_engine::tuning::{self, MAX_DOWNED_PROGRAMS, TOOL_SLOT_CAP};

    fn assets_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn program(species: &str, level: u32, rarity: Rarity) -> DownedProgram {
        DownedProgram {
            species: species.to_string(),
            level,
            rarity,
            boss: true,
            condition: 100,
        }
    }

    /// The species id whose display name is longest in the shipped
    /// catalogue — derived rather than hand-picked, so a renamed or added
    /// species becomes the new worst case automatically instead of quietly
    /// leaving the census measuring a shorter string than the screen can
    /// actually draw.
    fn widest_species_id(game: &Game) -> String {
        game.species_defs()
            .into_iter()
            .max_by_key(|def| def.name.chars().count())
            .expect("the shipped catalogue defines at least one species")
            .id
    }

    /// The `Rarity` whose `label()` is longest — `widest_species_id`'s
    /// reason, and walked over `Rarity::ALL` rather than assumed to be the
    /// top rung: it isn't — `Gold`'s "Overclocked" (11) outruns
    /// `Prismatic`'s "Bare-Metal" (10).
    fn widest_rarity() -> Rarity {
        Rarity::ALL
            .into_iter()
            .max_by_key(|r| r.label().map(str::len).unwrap_or(0))
            .expect("Rarity::ALL is non-empty")
    }

    /// A real `Game` holding `held`, with `tools` installed if given — through
    /// a save/edit/load round trip, `app_holding_downed_programs`'s reason
    /// (`crates/app-core/src/tests/support.rs`): the engine exposes no way to
    /// hand-place a `DownedProgram` from outside itself, and `Game::world` is
    /// private to it besides — this crate could not reach in even if it
    /// wanted to.
    ///
    /// The path is keyed on an atomic counter, not just `seed` — the test
    /// binary runs cases as concurrent threads, and two calls sharing a
    /// seed shared one file and raced (`scratch_path`'s own reason, in
    /// app-core's fixtures).
    fn game_with_state(seed: u32, held: Vec<DownedProgram>, tools: Option<Vec<ToolId>>) -> Game {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let assets = assets_dir();
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &assets).unwrap();
        let path = std::env::temp_dir().join(format!(
            "fp_gui_downed_programs_{seed}_{unique}_{}.sav",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        data.player.downed_programs = held;
        if let Some(tools) = tools {
            data.player.tools = tools;
        }
        save::save_to_file(&path, &data).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);
        loaded
    }

    fn game_holding_downed_programs(seed: u32, held: Vec<DownedProgram>) -> Game {
        game_with_state(seed, held, None)
    }

    /// The list page's worst case: `MAX_DOWNED_PROGRAMS` rows, every one the
    /// longest-named species at the longest-labelled rarity and a boss (the
    /// only longer of the two tag options) — every axis derived from the
    /// shipped catalogue, not hand-picked, so a new species or a renamed
    /// rarity is covered automatically.
    fn tallest_and_widest_list_game() -> Game {
        let probe = Game::new(9698, DifficultyMode::Forgiving, &assets_dir()).unwrap();
        let species = widest_species_id(&probe);
        let rarity = widest_rarity();
        let held = vec![program(&species, 999, rarity); MAX_DOWNED_PROGRAMS];
        game_holding_downed_programs(9700, held)
    }

    /// The tool page's worst case: the same widest program above, run
    /// through `TOOL_SLOT_CAP` filled slots rather than the single starter
    /// tool a fresh run installs — spec section 6 names that cap as the
    /// asserted constraint, so a page that only ever sees one tool never
    /// exercises it. Only two tools ship (`salvage_clamp`, `core_tap`), so
    /// the slots repeat rather than naming four distinct ones — the cap on
    /// *row count* is what this fixture is for, not a fourth tool file.
    fn tallest_and_widest_options_game() -> Game {
        let probe = Game::new(9698, DifficultyMode::Forgiving, &assets_dir()).unwrap();
        let species = widest_species_id(&probe);
        let rarity = widest_rarity();
        let held = vec![program(&species, 999, rarity)];
        let tools = (0..TOOL_SLOT_CAP)
            .map(|i| {
                ToolId(if i % 2 == 0 {
                    tuning::STARTER_TOOL_ID.to_string()
                } else {
                    "core_tap".to_string()
                })
            })
            .collect();
        game_with_state(9702, held, Some(tools))
    }

    /// This page has no scroll (spec section 6), so its height is a layout
    /// constraint — `the_tallest_memory_page_fits_its_popup`'s shape.
    /// `PopupSize::Large` at 720px has real headroom (28 rows, against 13
    /// for this store at the shipped cap of 10), so a `+1` mutation stays
    /// green: checked by hand, `MAX_DOWNED_PROGRAMS` had to reach 26 before
    /// this failed, and reverted — the constant itself is the thing under
    /// test, not a second copy of it here.
    #[test]
    fn the_tallest_downed_program_list_fits_its_popup_at_1280x720() {
        let game = tallest_and_widest_list_game();
        let rows = downed_program_list_rows(&game, 0).len();
        let m = ui_metrics(720.0);
        let cap = popup_max_rows(720.0, PopupSize::Large, &m);
        assert!(
            rows <= cap,
            "a full store ({MAX_DOWNED_PROGRAMS} programs) builds a {rows}-row page into a \
             {cap}-row popup at 1280x720"
        );
    }

    /// The other axis, `no_memory_row_overflows_its_popup`'s shape but over
    /// `Row::Item` rows rather than `Row::Text` — this page's list is
    /// exactly the row kind that trap warns a naive width census skips.
    #[test]
    fn no_downed_program_row_overflows_the_popup_body_at_1280x720() {
        let game = tallest_and_widest_list_game();
        let rows = downed_program_list_rows(&game, 0);
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        crate::paint::with_painter(|p| {
            for row in &rows {
                let label = row_label_text(row);
                let width = p.measure_ui_advance(&label, m.font_size);
                assert!(
                    width <= body,
                    "a downed-program row draws {width}px into a {body}px body at 1280x720: \
                     {label:?}"
                );
            }
        });
    }

    /// The tool page's own worst case: the widest program, so the header
    /// line is included too, run through `TOOL_SLOT_CAP` filled slots.
    #[test]
    fn the_tallest_extraction_options_page_fits_its_popup_at_1280x720() {
        let game = tallest_and_widest_options_game();
        let rows = extraction_options_rows(&game, 0, 0).len();
        let m = ui_metrics(720.0);
        let cap = popup_max_rows(720.0, PopupSize::Large, &m);
        assert!(
            rows <= cap,
            "the tool page builds a {rows}-row page into a {cap}-row popup at 1280x720"
        );
    }

    #[test]
    fn no_extraction_options_row_overflows_the_popup_body_at_1280x720() {
        let game = tallest_and_widest_options_game();
        let rows = extraction_options_rows(&game, 0, 0);
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        crate::paint::with_painter(|p| {
            for row in &rows {
                let label = row_label_text(row);
                let width = p.measure_ui_advance(&label, m.font_size);
                assert!(
                    width <= body,
                    "an extraction-options row draws {width}px into a {body}px body at \
                     1280x720: {label:?}"
                );
            }
        });
    }

    /// The list and the tool page must agree with what the engine actually
    /// counts — `Game::downed_program_rows().len()` — rather than the
    /// renderer keeping its own idea of how many rows there are.
    #[test]
    fn the_list_row_count_agrees_with_the_engine() {
        let game = game_holding_downed_programs(
            9701,
            vec![
                program("scrapper", 1, Rarity::Ordinary),
                program("scrapper", 2, Rarity::Ordinary),
                program("scrapper", 3, Rarity::Ordinary),
            ],
        );
        let rows = downed_program_list_rows(&game, 0);
        let item_rows = rows
            .iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .count();
        assert_eq!(item_rows, game.downed_program_rows().len());
    }
}
