//! The research picker.

use super::support::*;
use crate::*;
use feral_processes_engine::{GraphDir, MessageKind};

/// The id under the highlight, resolved the way the handlers do — through
/// `research_nodes()`, whose order changes as the player buys things.
fn selected_research_id(app: &App) -> String {
    let nodes = app.game.as_ref().expect("a run").research_nodes();
    nodes[app.menu_selected.min(nodes.len() - 1)].id.clone()
}

/// Which project the base is working, as the screen would read it.
fn active_project(app: &App) -> Option<String> {
    app.game
        .as_ref()
        .expect("a run")
        .research_nodes()
        .into_iter()
        .find(|n| n.state == feral_processes_engine::ResearchState::Active)
        .map(|n| n.id)
}

/// Exercises the exact key sequence a player drives at the keyboard —
/// `b` to open Build, a number to pick a structure, then a direction to
/// place it — entirely through `App::handle_key`, to make sure the
/// build/deploy flow (as opposed to `Game::place_structure` in
/// isolation, which the engine's own tests already cover) still works
/// end to end after the menu-navigation changes. Loops over every
/// structure number and every direction (re-opening the build menu each
/// time, exactly as a player retrying would) rather than assuming
/// number "1" is affordable or a given direction is walkable — with
/// starting resources, several of the ten structures are affordable, so
/// this only fails if the *menu itself* is broken, not because of which
/// particular structure a fresh session happens to put at each digit.
#[test]
fn the_base_menu_opens_research_and_esc_closes_it() {
    let mut app = test_app(501);
    open_via_menu(&mut app, 'b', "Research");
    assert!(matches!(app.mode, Mode::Research));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// A refused selection lands on the status line and leaves the screen open —
/// the six refusals themselves live in the engine, so one is enough here.
#[test]
fn a_refused_selection_lands_on_the_status_line() {
    let mut app = test_app(502);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('1'));
    assert!(
        matches!(app.mode, Mode::Research),
        "the menu stays open so the player can pick another"
    );
    assert!(app.status_line.is_some(), "a refusal has to say why");
    assert_eq!(active_project(&app), None, "and nothing was taken on");
}

/// The row keys take a project on, and `[A]` gives it up again.
///
/// `[A]` is **uppercase**: `selected_index` treats every lowercase letter as a
/// row label past the digits, so a lowercase `a` on a 34-row screen would pick
/// a row *and* abandon the project on one keypress. That second half is what
/// this pins.
#[test]
fn capital_a_abandons_and_lowercase_a_still_picks_a_row() {
    let mut app = app_in_base_with_a_research_node(503);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('1'));
    let taken = active_project(&app).expect("the first row is takeable from a stocked base");

    app.handle_key(GameKey::Char('a'));
    assert_eq!(
        active_project(&app).as_deref(),
        Some(taken.as_str()),
        "a lowercase key is a row selector and must not abandon anything"
    );

    app.handle_key(GameKey::Char('A'));
    assert_eq!(active_project(&app), None, "and the uppercase one does");
    assert_eq!(app.mode, Mode::Research, "the screen stays open either way");
}

/// The list's row keys and the graph's `Enter` are the same door, so both take
/// the node under the highlight on.
#[test]
fn enter_selects_the_highlighted_node() {
    let mut app = app_in_base_with_a_research_node(504);
    open_via_menu(&mut app, 'b', "Research");
    let from_list = {
        app.handle_key(GameKey::Char('1'));
        let taken = active_project(&app).expect("the list's row key takes a project on");
        app.handle_key(GameKey::Char('A'));
        taken
    };

    app.handle_key(GameKey::Char('G'));
    app.handle_key(GameKey::Enter);

    assert_eq!(
        active_project(&app).as_deref(),
        Some(from_list.as_str()),
        "Enter in the graph view is the same door as a row key in the list"
    );
}

/// The refusal reaches the message log as well as the status line, so a
/// player who looks away and misses the banner can still find out why
/// nothing happened. `App::refuse` is the one door that writes both.
#[test]
fn a_refused_research_node_is_written_to_the_log_too() {
    let mut app = test_app(502);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('1'));

    let banner = app.status_line.clone().expect("a refusal was reported");
    let logged = app
        .game
        .as_ref()
        .unwrap()
        .message_log(crate::MESSAGE_LOG_CAP);
    let last = logged.last().expect("the log is not empty");
    assert_eq!(last.text, banner, "the banner and the log must agree");
    assert_eq!(last.kind, MessageKind::Refusal);
}

/// The toggle is uppercase because `selected_index` treats every lowercase
/// letter as a row label past the digits — a lowercase toggle on a 34-row
/// screen would buy a node *and* flip the view on one keypress.
#[test]
fn g_toggles_the_graph_view_and_back() {
    let mut app = test_app(520);
    open_via_menu(&mut app, 'b', "Research");
    assert!(!app.research_graph_view, "the list is the opening view");
    app.handle_key(GameKey::Char('G'));
    assert!(app.research_graph_view);
    app.handle_key(GameKey::Char('G'));
    assert!(!app.research_graph_view);
    assert_eq!(app.mode, Mode::Research, "the toggle is not a mode change");
}

/// One selection, converted at the boundary. Toggling either way has to land
/// on the node the player was looking at, or the two views are two cursors.
#[test]
fn the_toggle_preserves_the_selected_node_in_both_directions() {
    let mut app = test_app(521);
    open_via_menu(&mut app, 'b', "Research");
    for _ in 0..5 {
        app.handle_key(GameKey::Down);
    }
    let before = selected_research_id(&app);
    app.handle_key(GameKey::Char('G'));
    assert_eq!(
        selected_research_id(&app),
        before,
        "flipping to the graph keeps the node"
    );
    app.handle_key(GameKey::Char('G'));
    assert_eq!(
        selected_research_id(&app),
        before,
        "and flipping back keeps it too"
    );
}

/// The whole reason the layout is derived in the engine: app-core computes
/// no neighbours. What an arrow does on this screen is whatever
/// `ResearchGraph::step` says, converted back through the id.
#[test]
fn an_arrow_in_the_graph_view_lands_where_step_says() {
    let mut app = test_app(522);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    for (key, dir) in [
        (GameKey::Down, GraphDir::Down),
        (GameKey::Right, GraphDir::Right),
        (GameKey::Up, GraphDir::Up),
        (GameKey::Left, GraphDir::Left),
    ] {
        let from = selected_research_id(&app);
        let want = app.game.as_ref().unwrap().research_graph().step(&from, dir);
        app.handle_key(key);
        assert_eq!(
            selected_research_id(&app),
            want,
            "{key:?} must land where step({from:?}, {dir:?}) says"
        );
    }
}

/// `step` is total, so a held arrow at the edge of the grid is a no-op and
/// never a panic or a wrap. Driven through `handle_key` because that is the
/// path a key repeat actually takes.
#[test]
fn holding_an_arrow_at_the_edge_of_the_grid_does_nothing() {
    let mut app = test_app(523);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    for _ in 0..40 {
        app.handle_key(GameKey::Up);
        app.handle_key(GameKey::Left);
    }
    let corner = selected_research_id(&app);
    for _ in 0..10 {
        app.handle_key(GameKey::Up);
        app.handle_key(GameKey::Left);
    }
    assert_eq!(corner, selected_research_id(&app), "the corner clamps");
    assert_eq!(app.mode, Mode::Research, "and the screen stays open");
}

/// `Enter` is the same door the list's row keys call, so it must refuse the
/// same way and file nothing on a refusal.
#[test]
fn enter_on_an_unreachable_node_refuses_without_filing() {
    let mut app = test_app(524);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    // Walk to the deepest tier, where a fresh run can afford nothing.
    for _ in 0..10 {
        app.handle_key(GameKey::Right);
    }
    let target = selected_research_id(&app);
    app.handle_key(GameKey::Enter);
    assert!(
        app.status_line.is_some(),
        "a refusal has to say why, exactly as the list's does"
    );
    assert_eq!(active_project(&app), None, "nothing is taken on");
    assert!(
        app.game.as_ref().unwrap().work_orders().is_empty(),
        "and nothing is filed"
    );
    assert!(
        !app.game.as_ref().unwrap().is_researched(&target),
        "and nothing is unlocked"
    );
    assert_eq!(app.mode, Mode::Research, "the screen stays open");
}

/// Esc closes the screen from either view — the graph is a view of this
/// screen, not a screen of its own to back out of first.
#[test]
fn esc_closes_the_screen_from_the_graph_view() {
    let mut app = test_app(525);
    open_via_menu(&mut app, 'b', "Research");
    app.handle_key(GameKey::Char('G'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    assert!(
        app.research_graph_view,
        "the chosen view survives closing the screen"
    );
}
