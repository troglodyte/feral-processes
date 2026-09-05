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
            "[{}] {}  T{}  {:?}  {status}",
            menu_shortcut(i),
            row.name,
            row.tier,
            row.category
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
    use feral_processes_engine::tuning::TOOL_SLOT_PER_LEVEL;

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

    /// A game where every shipped tool has a row: every id marked known,
    /// as many installed as fit (exercising the `slot N` status), the rest
    /// carrying a three-digit carrier count (exercising `N held`) — so the
    /// three status strings the row builder can produce are all on screen
    /// at once, not just the count.
    ///
    /// Row *count* here is "every tool this build ships", not
    /// `tuning::TOOL_SLOT_CAP`: decision 3 lists every *known* tool, and
    /// nothing bounds how many a modded research tree can teach — the
    /// shipped catalogue is the only real worst case there is today, the
    /// same footing `every_shipped_tool_id_is_unique` stands on.
    fn worst_case_game() -> Game {
        // Keyed on an atomic counter, not just the process id — the test
        // binary runs cases as concurrent threads, and every caller of this
        // fixture shares one seed, so two calls raced on one file without
        // it (`scratch_path`'s own reason, in app-core's fixtures).
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let assets = assets_dir();
        let mut game = Game::new(9698, DifficultyMode::Forgiving, &assets).unwrap();
        let ids: Vec<ToolId> = shipped_tools().all().map(|d| d.id.clone()).collect();
        assert!(!ids.is_empty(), "the census walked no shipped tools at all");

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
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);
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
