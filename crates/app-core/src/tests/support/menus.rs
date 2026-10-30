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

/// The four command pages, in rail order.
pub(crate) const COMMAND_PAGES: [Mode; 4] = [
    Mode::BaseBuild,
    Mode::BaseResearch,
    Mode::BaseTrade,
    Mode::BaseCompile,
];

/// Every command row on offer right now, across all four pages.
pub(crate) fn command_labels_all(app: &mut App) -> Vec<&'static str> {
    COMMAND_PAGES
        .iter()
        .flat_map(|&page| app.command_rows(page).unwrap_or_default())
        .map(|r| r.label)
        .collect()
}

/// Opens the command page that offers `label` through the hub and selects its
/// row, the way a player does. Rows are filtered by what is currently
/// possible, so the row is found by label rather than by position.
pub(crate) fn open_command(app: &mut App, label: &str) {
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

/// `[c]` and into the first page: the hub opens on its rail, so a test
/// about a page's own keys needs the `Right` a player would press.
pub(crate) fn open_structure_page(app: &mut App) {
    app.handle_key(GameKey::Char('c'));
    // A refused `c` leaves the map up, where Right would walk the player.
    if app.structure_hub.is_some() && app.hub_on_rail {
        app.handle_key(GameKey::Right);
    }
}

/// Opens `target`'s hub with its own key, walks `]` to `target`'s page and
/// Right off the rail and along its screens to `target`, the way a player
/// reaches a screen that no longer has a menu row of its own.
pub(crate) fn open_hub_screen(app: &mut App, target: Mode) {
    let (hub, page, screen) = hub_of(target).expect("target is a hub screen");
    app.handle_key(GameKey::Char(match hub {
        Hub::Base => 'b',
        Hub::Inventory => 'i',
        Hub::Structure | Hub::GameMenu => unreachable!("hub_of names the shared hubs only"),
    }));
    for _ in 0..hub.kind().table().len() {
        if hub_of(app.mode).map(|(_, p, _)| p) == Some(page) {
            break;
        }
        app.handle_key(GameKey::Char(']'));
    }
    app.handle_key(GameKey::Right);
    // Right walks the page's screens; the duty table spends it on its
    // columns first, hence the bound rather than exactly `screen` presses.
    for _ in 0..screen * 16 {
        if app.mode == target {
            break;
        }
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
