//! **A hub tab's registered `Row` ordinals equal app-core's selection index**
//! (`b`, `i` and the game menu: YOU, a real PARTY member, PROGRAMS and the
//! BESIDE YOU entries). Whatever a tab draws under an entry — a description, a
//! heading, a wrapped line — is not an `Item`, or a click (or the "n more"
//! count) lands on the wrong entry. Driven through the real `render::draw`
//! into a recording painter, over the live rail of the `contracts` template,
//! which has a Broker, a staffed base and every research node, so most tabs
//! have rows to count.

use super::*;
use feral_processes_app_core::{GameKey, Hub, HubPageKey};

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

/// The `Row` ordinals and every other target the **real** draw of the current
/// screen registers: `render::draw` into a recording painter, so a tab whose
/// renderer skips `draw_rows_in`, passes it no targets, or draws its own rows
/// registers nothing here and fails the census.
fn drawn_targets(app: &mut App) -> (Vec<usize>, Vec<feral_processes_app_core::PointerHit>) {
    let mut fx = Fx::new();
    let hits = crate::paint::with_targets(|p| draw(app, &mut fx, p, false, false));
    let ordinals = hits
        .iter()
        .filter_map(|(_, hit)| match hit {
            feral_processes_app_core::PointerHit::Row(i) => Some(*i),
            _ => None,
        })
        .collect();
    (ordinals, hits.into_iter().map(|(_, h)| h).collect())
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

/// Whether `ordinals` are what the draw must register for `selectable`
/// rows with the cursor on `at`: numbered from zero without a gap, the
/// window holding the cursor's row, and every row there when no marker says
/// more lie below.
fn ordinals_agree(
    ordinals: &[usize],
    selectable: usize,
    at: usize,
    more_below: bool,
    mode: Mode,
) -> Result<(), String> {
    // A member is the page's key, not a highlight: nothing on the page is a
    // row, and the cursor's row is the roster's.
    if mode == Mode::PartyMember {
        return if ordinals.is_empty() {
            Ok(())
        } else {
            Err(format!("registered {ordinals:?}"))
        };
    }
    // Zero rows and one both leave the cursor at the top.
    if ordinals.is_empty() && selectable == 1 {
        return Ok(());
    }
    let first = ordinals.first().copied().unwrap_or(0);
    let contiguous = ordinals.windows(2).all(|w| w[1] == w[0] + 1);
    let last = ordinals.last().copied();
    let holds_cursor = ordinals.contains(&at.min(selectable.saturating_sub(1)));
    let end_ok = last.is_some_and(|l| l + 1 == selectable) || more_below;
    if contiguous && holds_cursor && end_ok && last.is_none_or(|l| l < selectable) && first <= at {
        Ok(())
    } else {
        Err(format!(
            "registered {ordinals:?} with the cursor on {at} of {selectable}"
        ))
    }
}

/// Walks every tab of every page on `hub`'s live rail, returning how many it
/// saw and the tabs whose registered rows and selectable rows disagree, with
/// the cursor at the top and at the last row.
fn walk_hub(app: &mut App, hub: Hub, open: GameKey) -> (usize, Vec<String>) {
    app.handle_key(open);
    app.hub_on_rail = true;
    let pages = hub.kind().pages(app).len();
    let mut seen = 0;
    let mut wrong = Vec::new();
    for _ in 0..pages {
        let (_, page, _) = app.hub_at().unwrap_or_else(|| panic!("in {hub:?}"));
        let tabs = hub.kind().pages(app)[page].tabs.len();
        for _ in 0..tabs {
            let mode = app.mode;
            let selectable = selection_rows(app);
            assert_eq!(app.mode, mode, "Up left {mode:?}");
            for at in [0, selectable - 1] {
                app.hub_on_rail = false;
                app.menu_selected = at;
                let (ordinals, hits) = drawn_targets(app);
                let more_below = hits.contains(&feral_processes_app_core::PointerHit::More(
                    feral_processes_app_core::Direction::Down,
                ));
                if let Err(why) = ordinals_agree(&ordinals, selectable, at, more_below, mode) {
                    wrong.push(format!("{mode:?} at {at}: {why}"));
                }
            }
            app.menu_selected = 0;
            app.hub_on_rail = true;
            seen += 1;
            app.handle_key(GameKey::Tab);
        }
        app.handle_key(GameKey::Char(']'));
    }
    (seen, wrong)
}

#[test]
fn every_b_tab_registers_one_row_per_selectable_row() {
    let mut app = contracts_app();
    app.research_graph_view = false;
    let (seen, wrong) = walk_hub(&mut app, Hub::Base, GameKey::Char('b'));
    assert!(seen > 20, "the census walked {seen} tabs");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `i` as well: YOU, a real PARTY member, PROGRAMS, and the BESIDE YOU
/// entries (the template's party stands by a Broker and a Printer).
#[test]
fn every_i_tab_registers_one_row_per_selectable_row() {
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
    let (seen, wrong) = walk_hub(&mut app, Hub::Inventory, GameKey::Char('i'));
    assert!(seen > 10, "the census walked {seen} tabs");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Esc's game menu is a hub too: its pages are lists of options.
#[test]
fn every_game_menu_tab_registers_one_row_per_selectable_row() {
    let mut app = contracts_app();
    let (seen, wrong) = walk_hub(&mut app, Hub::GameMenu, GameKey::Esc);
    assert!(seen >= 4, "the census walked {seen} tabs");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A list too long for its rect registers only the rows on screen, numbered
/// from where the window starts, and a marker for each hidden side that
/// scrolls it; the highlighted row is not also tinted for hover.
#[test]
fn a_scrolling_list_registers_its_window_and_its_markers() {
    use crate::render::popup::{RowTargets, draw_rows_in, item_row};
    use feral_processes_app_core::{Direction, PointerHit};
    let m = crate::text::ui_metrics(720.0);
    let rows: Vec<Row> = (0..40)
        .map(|i| item_row(format!("row {i}"), i == 20))
        .collect();
    let rect = crate::paint::Rect::new(0.0, 0.0, 600.0, 10.0 * m.line_height);
    let draw = |hover| {
        crate::paint::with_painter(|p| {
            let hover = Some(RowTargets { hover });
            draw_rows_in(rect, &rows, true, hover, p, &m)
        })
        .1
    };
    let hits = crate::paint::with_targets(|p| {
        draw_rows_in(rect, &rows, true, Some(RowTargets { hover: None }), p, &m)
    });
    let ordinals: Vec<usize> = hits
        .iter()
        .filter_map(|(_, h)| match h {
            PointerHit::Row(i) => Some(*i),
            _ => None,
        })
        .collect();
    assert!(ordinals.contains(&20), "the cursor's row is on screen");
    assert!(!ordinals.contains(&0) && !ordinals.contains(&39));
    assert!(
        ordinals.windows(2).all(|w| w[1] == w[0] + 1),
        "{ordinals:?}"
    );
    let hit_list: Vec<PointerHit> = hits.iter().map(|(_, h)| *h).collect();
    assert!(hit_list.contains(&PointerHit::More(Direction::Up)));
    assert!(hit_list.contains(&PointerHit::More(Direction::Down)));
    // Hover tints a row that is not the cursor's, and the marker too.
    let tint = crate::render::hud::palette::HOVER_BG;
    let tinted = |shapes: &[_]| crate::paint::painted_rect_fill_count(shapes, tint);
    assert_eq!(tinted(&draw(None)), 0);
    assert_eq!(tinted(&draw(Some(PointerHit::Row(ordinals[0])))), 1);
    assert_eq!(tinted(&draw(Some(PointerHit::Row(20)))), 0, "the cursor's");
    assert_eq!(tinted(&draw(Some(PointerHit::More(Direction::Down)))), 1);
}
