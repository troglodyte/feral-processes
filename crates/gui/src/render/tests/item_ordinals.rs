//! **`Row::Item` ordinal == app-core selection index on every `b` tab.**
//! Whatever a tab draws under an entry — a description, a hint, a wrapped
//! line — is not an `Item`, or a click (or the "n more" count) lands on the
//! wrong entry. Walked over the live rail of the `contracts` template, which
//! has a Broker, a staffed base and every research node, so most tabs have
//! rows to count.

use super::*;
use crate::render::base_report::{AppScreens, command_pages, screen_rows};
use crate::render::building::{build_menu_page_rows, pin_subject_page_rows};
use crate::render::progression::research_page_rows;
use feral_processes_app_core::{GameKey, Hub};
use feral_processes_engine::ResearchTree;

fn contracts_app() -> App {
    let mut app = census_app();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ron = std::fs::read_to_string(root.join("dev-saves/contracts.ron")).unwrap();
    let data = feral_processes_engine::save::from_ron(&ron).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_processes_gui_item_ordinals_{}.bin",
        std::process::id()
    ));
    feral_processes_engine::save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &root.join("assets")).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// The rows `draw_mode_overlay` builds for a `b` tab, through the same
/// builders. `None` names a tab this census has not been taught.
fn tab_rows(app: &mut App) -> Option<Vec<Row>> {
    let mode = app.mode;
    let work_orders = app.work_order_rows();
    let base_staff = (mode == Mode::BaseStaff).then(|| app.base_staff_table());
    let commands = app.command_rows(mode);
    let command_counts: Vec<(usize, usize)> = command_pages()
        .filter_map(|(page, mode)| Some((page, app.command_rows(mode)?.len())))
        .collect();
    let (active, offers) = app.contract_sections();
    let reach = app.broker_reach();
    let site = app.held_site();
    let at_bench = app.craft_at_bench();
    let bay = app.breeding_bay();
    let dispatch = app.dispatch_sites();
    let trips = app.dispatch_trip_reports();
    let build_category = app.build_category;
    let game = app.game.as_mut().unwrap();
    let tree = |mode| match mode {
        Mode::Research => Some(ResearchTree::Base),
        Mode::RoutineResearch => Some(ResearchTree::Routines),
        Mode::AffixResearch => Some(ResearchTree::Affixes),
        Mode::WeaponResearch => Some(ResearchTree::Weapons),
        _ => None,
    };
    Some(match mode {
        Mode::Build => build_menu_page_rows(game, build_category, 0),
        Mode::PinSubject => pin_subject_page_rows(game, 0),
        Mode::ContractsHeld => crate::render::contracts::held_rows(&active, 0),
        Mode::Contracts => crate::render::contracts::board_rows(
            &offers,
            active.iter().any(|row| row.tutorial),
            reach,
            0,
        ),
        Mode::Craft => crate::render::crafting::craft_page_rows(game, at_bench, "", 0),
        Mode::Breed => crate::render::breeding::breed_page_rows(game, bay, 0),
        Mode::Dispatch => match dispatch.as_deref() {
            Some(sites) => crate::render::dispatch::dispatch_hub_rows(sites, &trips.0, &trips.1, 0),
            None => Vec::new(),
        },
        // The tab's basket rows need a docked wagon; the census world has
        // none, so its one line is text.
        Mode::Caravan => Vec::new(),
        mode if tree(mode).is_some() => research_page_rows(game, tree(mode)?, 0, false).1,
        mode => {
            let screens = AppScreens {
                site,
                member: None,
                list_filter: "",
                commands: commands.as_deref(),
                command_counts: &command_counts,
                work_orders: &work_orders,
                base_staff: base_staff.as_ref(),
                work_column: 0,
                on_rail: false,
                perk_previews: &[],
            };
            let flags = game.base_report_flags();
            let rows = screen_rows(game, mode, 0, &screens, &flags);
            if matches!(rows.as_slice(), [Row::Text(t)] if t.starts_with("No report for")) {
                return None;
            }
            rows
        }
    })
}

/// How many rows app-core lets the cursor walk: Up from the top wraps to
/// the last. Zero and one rows both leave it at 0.
fn selection_rows(app: &mut App) -> usize {
    app.hub_on_rail = false;
    app.menu_selected = 0;
    app.handle_key(GameKey::Up);
    let last = app.menu_selected;
    app.menu_selected = 0;
    app.hub_on_rail = true;
    last + 1
}

#[test]
fn every_b_tab_counts_one_item_per_selectable_row() {
    let mut app = contracts_app();
    app.research_graph_view = false;
    app.handle_key(GameKey::Char('b'));
    app.hub_on_rail = true;
    let pages = Hub::Base.kind().pages(&app).len();
    let mut seen = 0;
    let mut wrong = Vec::new();
    for _ in 0..pages {
        let (_, page, _) = app.hub_at().expect("in b");
        let tabs = Hub::Base.kind().pages(&app)[page].tabs.len();
        for _ in 0..tabs {
            let mode = app.mode;
            let rows = tab_rows(&mut app).unwrap_or_else(|| panic!("teach the census {mode:?}"));
            let items = rows
                .iter()
                .filter(|r| matches!(r, Row::Item { .. }))
                .count();
            let selectable = selection_rows(&mut app);
            assert_eq!(app.mode, mode, "Up left {mode:?}");
            // Zero rows and one both leave the cursor at the top.
            if !(items == selectable || (items == 0 && selectable == 1)) {
                wrong.push(format!("{mode:?}: {items} items, {selectable} selectable"));
            }
            seen += 1;
            app.handle_key(GameKey::Tab);
        }
        app.handle_key(GameKey::Char(']'));
    }
    assert!(seen > 20, "the census walked {seen} tabs");
    assert!(wrong.is_empty(), "{wrong:#?}");
}
