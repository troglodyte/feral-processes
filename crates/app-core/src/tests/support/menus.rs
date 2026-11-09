//! Fixtures shared by the app-core tests: menus.

use super::*;
use crate::*;

/// Every party command row on offer right now, Programs then Routines.
pub(crate) fn party_rows(app: &mut App) -> Vec<GroupMenuRow> {
    PARTY_COMMAND_PAGES
        .iter()
        .flat_map(|&page| app.command_rows(page).unwrap_or_default())
        .collect()
}

/// Opens a party screen the way a player has to: through the hub.
///
/// Tests go through this instead of hard-coding a row number — the rows are
/// filtered by what is currently possible (see `App::command_rows`), so a
/// row's position depends on the fixture and pinning it would make these
/// tests break for the wrong reason. "Companions" and "Perks" are pages, not
/// rows, and are reached by walking the rail.
pub(crate) fn open_via_menu(app: &mut App, group: char, label: &str) {
    assert_eq!(group, 'p', "{group} is not a group menu key");
    match label {
        "Companions" => return open_hub_screen(app, Mode::Companion),
        "Perks" => return open_hub_screen(app, Mode::Perks),
        _ => {}
    }
    let mut offered = Vec::new();
    for page in PARTY_COMMAND_PAGES {
        let rows = app.command_rows(page).unwrap_or_default();
        offered.push((page, rows.iter().map(|r| r.label).collect::<Vec<_>>()));
        if let Some(idx) = rows.iter().position(|r| r.label == label) {
            open_hub_screen(app, page);
            pick_row(app, idx);
            return;
        }
    }
    panic!("{label:?} is not offered right now; pages: {offered:?}");
}

/// Moves the highlight to `row` the way a player does, with Up or Down from
/// wherever the screen opened it. Wrapping makes any row reachable, and the
/// cap catches a `row` the list does not have rather than looping.
pub(crate) fn highlight_row(app: &mut App, row: usize) {
    for _ in 0..row + 64 {
        match app.menu_selected.cmp(&row) {
            std::cmp::Ordering::Equal => return,
            std::cmp::Ordering::Less => app.handle_key(GameKey::Down),
            std::cmp::Ordering::Greater => app.handle_key(GameKey::Up),
        }
    }
    panic!(
        "row {row} is unreachable; highlight is on {}",
        app.menu_selected
    );
}

/// Highlights `row` and presses Enter.
pub(crate) fn pick_row(app: &mut App, row: usize) {
    highlight_row(app, row);
    app.handle_key(GameKey::Enter);
}

/// The base's command pages: the Structures stopgap alone since the hub
/// merge made the other launchers' rows into tabs.
pub(crate) const COMMAND_PAGES: [Mode; 1] = [Mode::BaseStructures];

/// Every base command row on offer right now.
pub(crate) fn command_labels_all(app: &mut App) -> Vec<&'static str> {
    COMMAND_PAGES
        .iter()
        .flat_map(|&page| app.command_rows(page).unwrap_or_default())
        .map(|r| r.label)
        .collect()
}

/// Opens the screen a base launcher row `label` opened, the way a player
/// reaches it now: a retired row through its tab (`open_hub_screen`), a
/// structure verb through the Structures page, highlighted and picked.
/// Those rows are filtered by what is currently possible, so they are found
/// by label rather than by position.
pub(crate) fn open_command(app: &mut App, label: &str) {
    let tab = match label {
        "Deploy a structure" => Some(Mode::Build),
        "Research" => Some(Mode::Research),
        "Routine research" => Some(Mode::RoutineResearch),
        "Affix research" => Some(Mode::AffixResearch),
        "Weapon research" => Some(Mode::WeaponResearch),
        "Study a program" => Some(Mode::PinSubject),
        "Compile an item" => Some(Mode::Craft),
        "Contracts" => Some(Mode::Contracts),
        "Caravan" => Some(Mode::Caravan),
        "Dispatch" => Some(Mode::Dispatch),
        _ => None,
    };
    if let Some(tab) = tab {
        return open_hub_screen(app, tab);
    }
    // Hold lost its launcher with the Build page and is the Holding Cell's
    // Hold tab; its screen's own tests open it directly.
    let unhoused = match label {
        "Hold a program" => Some(Mode::PinPrisoner),
        _ => None,
    };
    if let Some(mode) = unhoused {
        app.mode = mode;
        app.menu_selected = 0;
        return;
    }
    let mut offered = Vec::new();
    for page in COMMAND_PAGES {
        let rows = app.command_rows(page).unwrap_or_default();
        offered.push((page, rows.iter().map(|r| r.label).collect::<Vec<_>>()));
        if let Some(idx) = rows.iter().position(|r| r.label == label) {
            open_hub_screen(app, page);
            pick_row(app, idx);
            return;
        }
    }
    panic!("{label:?} is not offered right now; pages: {offered:?}");
}

/// Where `mode` sits in the base report's or the inventory's fixed table, as
/// `(hub, page, screen)`. The rails that depend on the `App` are not here.
pub(crate) fn hub_of(mode: Mode) -> Option<(Hub, usize, usize)> {
    [
        (Hub::Base, BASE_REPORT_PAGES),
        (Hub::Inventory, INVENTORY_PAGES),
    ]
    .into_iter()
    .find_map(|(hub, table)| {
        table.iter().enumerate().find_map(|(p, page)| {
            page.tabs
                .iter()
                .position(|t| t.mode == mode)
                .map(|s| (hub, p, s))
        })
    })
}

/// `[c]` and into the first page: the hub opens on its rail, so a test
/// about a page's own keys needs the `Right` a player would press.
pub(crate) fn open_structure_page(app: &mut App) {
    app.handle_key(GameKey::Char('c'));
    // A refused `c` leaves the map up, where Right would walk the player.
    if app.structure_hub.is_some() && app.hub_on_rail {
        app.handle_key(GameKey::Right);
    }
}

/// Steps a research tab out of its tree view (the default), whose Esc
/// leaves the tree before the tab.
pub(crate) fn leave_the_research_tree_view(app: &mut App) {
    if app.research_graph_view {
        app.handle_key(GameKey::Esc);
    }
    assert!(!app.research_graph_view);
}

/// Opens `target`'s hub with its own key, walks `]` to the first page on
/// the rail as it stands that offers `target`, and Right off the rail and
/// `Tab` along its tabs to it, the way a player reaches a screen that has
/// no menu row of its own. The page is found on the live rail, since a page
/// the rail slots in (Breed, Contracts, a site) moves the ones after it. A
/// screen in neither fixed table is looked for on `b`'s rail.
pub(crate) fn open_hub_screen(app: &mut App, target: Mode) {
    let hub = hub_of(target).map_or(Hub::Base, |(hub, ..)| hub);
    app.handle_key(GameKey::Char(match hub {
        Hub::Base => 'b',
        Hub::Inventory => 'i',
        Hub::Structure | Hub::GameMenu => {
            unreachable!("hub_of names the shared hubs only")
        }
    }));
    let pages = hub.kind().pages(app);
    let page = pages
        .iter()
        .position(|p| p.offers(target))
        .unwrap_or_else(|| panic!("no page of {hub:?} offers {target:?} right now"));
    for _ in 0..=pages.len() {
        if app.hub_page() == Some(&pages[page].key) {
            break;
        }
        app.handle_key(GameKey::Char(']'));
    }
    // From the tab the page opened (or was left) on, from the rail: a
    // research tree view keeps `Tab` once the cursor is in it.
    for _ in 0..pages[page].tabs.len() {
        if app.mode == target {
            break;
        }
        app.handle_key(GameKey::Tab);
    }
    if app.hub_on_rail {
        app.handle_key(GameKey::Right);
    }
    assert_eq!(app.mode, target, "the hub reached {target:?}");
    assert!(!app.hub_on_rail, "the cursor is on the screen's rows");
}

/// A base the player is standing inside: a Home one tile north and an
/// ordinary Extractor one tile east, with nothing to the south or west.
///
/// Built by editing a save for the same reason `app_owning_a_program_and_an_
/// extractor` is — deploying a second structure through the build flow needs
/// materials the player does not start with, and the layout is the whole
/// point here: the direct demolish key has to tell a Home from anything else
/// and both from an empty tile, which is three neighbours of one tile.
pub(crate) fn app_inside_a_small_base(seed: u32, underground: bool) -> App {
    app_inside_a_small_base_with_programs(seed, underground, 0)
}
