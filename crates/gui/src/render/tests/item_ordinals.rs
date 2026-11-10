//! **`Row::Item` ordinal == app-core selection index on every hub tab** (`b`
//! and `i`: YOU, a real PARTY member, PROGRAMS and the BESIDE YOU entries).
//! Whatever a tab draws under an entry — a description, a hint, a wrapped
//! line — is not an `Item`, or a click (or the "n more" count) lands on the
//! wrong entry. Walked over the live rail of the `contracts` template, which
//! has a Broker, a staffed base and every research node, so most tabs have
//! rows to count.

use super::*;
use crate::render::base_report::{AppScreens, screen_rows};
use crate::render::building::{build_menu_page_rows, pin_subject_page_rows};
use crate::render::progression::research_page_rows;
use feral_processes_app_core::{GameKey, Hub, HubPageKey, TransferEntry};
use feral_processes_engine::ResearchTree;

fn contracts_app() -> App {
    let mut app = census_app();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ron = std::fs::read_to_string(root.join("dev-saves/contracts.ron")).unwrap();
    let data = feral_processes_engine::save::from_ron(&ron).unwrap();
    // Two tests build this app on parallel threads, so the pid alone is not
    // a unique name.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_gui_item_ordinals_{}_{nth}.bin",
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
    let (active, offers) = app.contract_sections();
    let reach = app.broker_reach();
    let site = app.held_site();
    let member = app.held_member();
    let at_bench = app.craft_at_bench();
    let bay = app.breeding_bay();
    let dispatch = app.dispatch_sites();
    let trips = app.dispatch_trip_reports();
    let build_category = app.build_category;
    let machine_recipe = app.machine_recipe.clone();
    let depot_filter = app.depot_filter.clone();
    let structure = app.pending_structure_manifest;
    let room = app.basket_room;
    let transfer_entries: Vec<(String, i64, u32, u32)> = app
        .basket_rows
        .iter()
        .enumerate()
        .map(|(row, entry)| {
            let amount = app.basket_amounts.get(row).copied().unwrap_or(0);
            match entry {
                TransferEntry::Item(r) => (
                    app.game.as_ref().unwrap().item_name(&r.item).to_string(),
                    amount,
                    r.carried,
                    r.on_shelves,
                ),
                TransferEntry::Carrier(c) => (
                    c.label.clone(),
                    amount,
                    u32::from(!c.racked),
                    u32::from(c.racked),
                ),
            }
        })
        .collect();
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
        Mode::Transfer => crate::render::transfer::body_rows(&transfer_entries, room, 0),
        Mode::MachineRecipe => crate::render::machine_recipe::page_rows(machine_recipe.as_ref(), 0),
        Mode::DepotFilter => crate::render::depot_filter::page_rows(depot_filter.as_ref(), 0),
        Mode::StructureManifest => {
            crate::render::structure_manifest::structure_rows(game, structure)?
        }
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
                member,
                list_filter: "",
                commands: commands.as_deref(),
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

/// Walks every tab of every page on `hub`'s live rail, returning how many it
/// saw and the tabs whose drawn `Item` count and selectable count disagree.
fn walk_hub(app: &mut App, hub: Hub, key: char) -> (usize, Vec<String>) {
    app.handle_key(GameKey::Char(key));
    app.hub_on_rail = true;
    let pages = hub.kind().pages(app).len();
    let mut seen = 0;
    let mut wrong = Vec::new();
    for _ in 0..pages {
        let (_, page, _) = app.hub_at().unwrap_or_else(|| panic!("in {hub:?}"));
        let tabs = hub.kind().pages(app)[page].tabs.len();
        for _ in 0..tabs {
            let mode = app.mode;
            let Some(rows) = tab_rows(app) else {
                wrong.push(format!("teach the census {mode:?}"));
                app.handle_key(GameKey::Tab);
                continue;
            };
            let items = rows
                .iter()
                .filter(|r| matches!(r, Row::Item { .. }))
                .count();
            let selectable = selection_rows(app);
            assert_eq!(app.mode, mode, "Up left {mode:?}");
            let agrees = match mode {
                // A member is the page's key, not a highlight: nothing on the
                // page is an `Item`, and the cursor's row is the roster's.
                Mode::PartyMember => items == 0,
                // These three draw an entry over several `Item` lines (a
                // wrapped continuation, a perk group's heading and spacer) and
                // `draw_popup` pages them by span rather than by ordinal, so
                // the count can only be bounded: every selectable entry has a
                // row.
                Mode::Inventory | Mode::Perks | Mode::Companion => items >= selectable,
                // Zero rows and one both leave the cursor at the top.
                _ => items == selectable || (items == 0 && selectable == 1),
            };
            if !agrees {
                wrong.push(format!("{mode:?}: {items} items, {selectable} selectable"));
            }
            seen += 1;
            app.handle_key(GameKey::Tab);
        }
        app.handle_key(GameKey::Char(']'));
    }
    (seen, wrong)
}

#[test]
fn every_b_tab_counts_one_item_per_selectable_row() {
    let mut app = contracts_app();
    app.research_graph_view = false;
    let (seen, wrong) = walk_hub(&mut app, Hub::Base, 'b');
    assert!(seen > 20, "the census walked {seen} tabs");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `i` as well: YOU, a real PARTY member, PROGRAMS, and the BESIDE YOU
/// entries (the template's party stands by a Broker and a Printer).
#[test]
fn every_i_tab_counts_one_item_per_selectable_row() {
    let mut app = contracts_app();
    let members = Hub::Inventory
        .kind()
        .pages(&app)
        .iter()
        .filter(|p| matches!(p.key, HubPageKey::Member(_)))
        .count();
    assert!(members > 0, "the census needs a real party member");
    assert!(
        !app.game.as_ref().unwrap().adjacent_structures().is_empty(),
        "the census needs a structure beside the party"
    );
    let (seen, wrong) = walk_hub(&mut app, Hub::Inventory, 'i');
    assert!(seen > 10, "the census walked {seen} tabs");
    assert!(wrong.is_empty(), "{wrong:#?}");
}
