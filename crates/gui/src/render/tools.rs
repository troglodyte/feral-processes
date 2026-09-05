//! `Mode::Tools`: the tool kit, reached from the party menu — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`, section
//! 6. A flat list, `Game::tool_rows`'s own order — there is no per-row
//! drill-down page the way `Mode::DownedPrograms` has one.

use super::popup::*;
use super::*;
use feral_processes_engine::tools;

/// Every row of the screen: a header naming slots used against the level
/// cap (plan decision 3), then one row per `Game::tool_rows` entry, then the
/// footer every read-and-act popup in this family closes with.
///
/// The status column reads one of three ways depending on the row's own
/// figures — never re-derived here beyond formatting, `message_history`'s
/// rule that a per-row transform belongs in the engine and this only joins
/// strings.
pub(super) fn tools_rows(game: &Game, selected: usize) -> Vec<Row> {
    let rows_data = game.tool_rows();
    let slots_used = game.installed_tools().len();
    let slots_total = tools::player_tool_slots(game.player_status().level);
    let mut rows = vec![
        text_row(format!(
            "Tool slots: {slots_used}/{slots_total}. F forges a carrier, I installs one, X \
             pulls a slot."
        )),
        text_row(""),
    ];
    if rows_data.is_empty() {
        rows.push(text_row("Nothing known or installed yet."));
    }
    for (i, row) in rows_data.iter().enumerate() {
        let status = match row.slot {
            Some(slot) => format!("slot {}", slot + 1),
            None if row.carriers_held > 0 => format!("{} held", row.carriers_held),
            None => "not forged".to_string(),
        };
        let label = format!(
            "[{}] {}  T{}  {}  {status}",
            menu_shortcut(i),
            row.name,
            row.tier,
            row.category.as_str()
        );
        rows.push(item_row(label, i == selected));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to go back"));
    rows
}

pub(super) fn draw_tools(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = tools_rows(game, selected);
    draw_popup("Tools", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::DifficultyMode;
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::save;
    use feral_processes_engine::tools::{ToolDb, ToolId};
    use feral_processes_engine::tuning::{self, TOOL_SLOT_PER_LEVEL};

    fn assets_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// The real shipped tool catalogue, loaded directly rather than through
    /// a `Game` — the only way to see every def regardless of what any one
    /// fixture happens to know, `research_db_with_tool_unlocks`'s own
    /// reason for loading straight off disk in the engine's own tests.
    fn shipped_tools() -> ToolDb {
        let (db, warnings) = ToolDb::load_dir(&assets_dir().join("tools")).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped tool files must load clean: {warnings:?}"
        );
        db
    }

    /// Recursively copies `src` into `dst`, which must not yet exist — the
    /// gui crate's own version of the engine's `tests::support::copy_
    /// shipped_assets`, which is `pub(super)` inside that crate and so
    /// cannot be reused from here.
    fn copy_assets_dir(src: &std::path::Path, dst: &std::path::Path) {
        std::fs::create_dir_all(dst).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let target = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_assets_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    /// A game where `tuning::MAX_TOOL_ROWS` tools have a row — the real
    /// bound (Minor 9 of the review), not "however many tools this build
    /// ships" (three, today, well under the cap). `Game::tool_rows` trims
    /// to that ceiling itself, so this fixture pads the shipped catalogue
    /// with synthetic tool files up to it, on a scratch copy of the whole
    /// asset tree (`Game::new` needs every directory present to start
    /// clean). The shipped tools fill as many slots as fit (exercising the
    /// `slot N` status), the padding tools carry a three-digit carrier
    /// count (exercising `N held`), so both status strings the row builder
    /// can produce sit on screen at once alongside `not forged`.
    fn worst_case_game() -> Game {
        // Keyed on an atomic counter, not just the process id — the test
        // binary runs cases as concurrent threads, and every caller of this
        // fixture shares one seed, so two calls raced on one file without
        // it (`scratch_path`'s own reason, in app-core's fixtures).
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let shipped_ids: Vec<ToolId> = shipped_tools().all().map(|d| d.id.clone()).collect();
        assert!(
            !shipped_ids.is_empty(),
            "the census walked no shipped tools at all"
        );

        let scratch = std::env::temp_dir().join(format!(
            "fp_gui_tools_worst_case_assets_{}_{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        copy_assets_dir(&assets_dir(), &scratch);
        let extra_needed = tuning::MAX_TOOL_ROWS.saturating_sub(shipped_ids.len());
        let extra_ids: Vec<ToolId> = (0..extra_needed)
            .map(|i| {
                let id = format!("worst_case_padding_tool_{i}");
                std::fs::write(
                    scratch.join("tools").join(format!("{id}.ron")),
                    format!(
                        r#"(
                            id: "{id}",
                            name: "Worst Case Padding Tool {i}",
                            description: "d",
                            category: Materials,
                            yields: [("core_fragment", 1.0)],
                            tier: 1,
                            ticks: 1,
                        )"#
                    ),
                )
                .unwrap();
                ToolId(id)
            })
            .collect();
        let ids: Vec<ToolId> = shipped_ids.into_iter().chain(extra_ids).collect();
        assert_eq!(
            ids.len(),
            tuning::MAX_TOOL_ROWS,
            "the fixture must reach exactly the row ceiling to test it"
        );

        let mut game = Game::new(9698, DifficultyMode::Forgiving, &scratch).unwrap();

        let path = std::env::temp_dir().join(format!(
            "fp_gui_tools_worst_case_{}_{unique}.sav",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        data.known_tools = ids.clone();
        data.player.level = TOOL_SLOT_PER_LEVEL * 3; // TOOL_SLOT_CAP reached
        let cap = tools::player_tool_slots(data.player.level);
        data.player.tools = ids.iter().take(cap).cloned().collect();
        data.player
            .inventory
            .extend(ids.iter().skip(cap).map(|id| (ItemId::tool(id), 999u32)));
        save::save_to_file(&path, &data).unwrap();
        let loaded = Game::load(&path, &scratch).unwrap();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&scratch);
        loaded
    }

    /// This page has no scroll (spec section 6), so its height is a layout
    /// constraint — `the_tallest_downed_program_list_fits_its_popup`'s
    /// shape.
    #[test]
    fn the_tallest_tools_list_fits_its_popup_at_1280x720() {
        let game = worst_case_game();
        let rows = tools_rows(&game, 0).len();
        let m = ui_metrics(720.0);
        let cap = popup_max_rows(720.0, PopupSize::Large, &m);
        assert!(
            rows <= cap,
            "every shipped tool known builds a {rows}-row page into a {cap}-row popup at \
             1280x720"
        );
    }

    #[test]
    fn no_tools_row_overflows_the_popup_body_at_1280x720() {
        let game = worst_case_game();
        let rows = tools_rows(&game, 0);
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        crate::paint::with_painter(|p| {
            for row in &rows {
                let label = row_label_text(row);
                let width = p.measure_ui_advance(&label, m.font_size);
                assert!(
                    width <= body,
                    "a tools row draws {width}px into a {body}px body at 1280x720: {label:?}"
                );
            }
        });
    }

    /// The list must agree with what the engine actually counts —
    /// `Game::tool_rows().len()` — rather than the renderer keeping its own
    /// idea of how many rows there are.
    #[test]
    fn the_row_count_agrees_with_the_engine() {
        let game = worst_case_game();
        let rows = tools_rows(&game, 0);
        let item_rows = rows
            .iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .count();
        assert_eq!(item_rows, game.tool_rows().len());
    }
}
