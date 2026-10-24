//! The Workbench's routine-crafting pages: Build routine, Subroutines and
//! Built routines, and installing what they make.

use super::support::*;
use crate::*;
use feral_processes_engine::routine_craft::{AreaKind, Core, Subroutine, Version};
use feral_processes_engine::routine_tree::part_node_id;

fn heal() -> Subroutine {
    Subroutine::Core(Core::Heal, Version::V1)
}

fn group() -> Subroutine {
    Subroutine::Area(AreaKind::Group)
}

/// A base with a Workbench beside the party and Automation researched.
fn app_at_a_workbench(seed: u32) -> App {
    let mut app = app_owning_a_program_and_a_station_of("routine_craft", "workbench", seed);
    stand_beside_the_extractor(&mut app);
    let game = app.game.as_mut().unwrap();
    let _ = game.dev_unlock_research(&"automation".to_string());
    app
}

fn know(app: &mut App, part: &Subroutine) {
    let game = app.game.as_mut().unwrap();
    let _ = game.dev_unlock_research(&part_node_id(part));
}

fn hold(app: &mut App, part: &Subroutine, n: u32) {
    let game = app.game.as_mut().unwrap();
    game.dev_give_item(&feral_processes_engine::items::ItemId::part(part), n)
        .unwrap();
}

/// `c`, down the rail to the page that shows `target`, and Right onto it.
fn open_page(app: &mut App, target: Mode) {
    app.handle_key(GameKey::Char('c'));
    for _ in 0..12 {
        if app.mode == target {
            break;
        }
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.mode, target, "the rail reaches {target:?}");
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, target);
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(GameKey::Char(c));
    }
}

#[test]
fn a_workbench_hub_has_the_three_routine_pages() {
    let mut app = app_at_a_workbench(80_001);
    app.handle_key(GameKey::Char('c'));
    let hub = app.structure_hub.as_ref().expect("the hub opens");
    let pages = &hub.current().pages;
    for page in [
        StructurePage::Build,
        StructurePage::Subroutines,
        StructurePage::Built,
    ] {
        assert!(pages.contains(&page), "{page:?} in {pages:?}");
    }
}

#[test]
fn building_a_routine_through_the_screen_compiles_and_installs_it() {
    let mut app = app_at_a_workbench(80_002);
    for part in [heal(), group()] {
        know(&mut app, &part);
        hold(&mut app, &part, 1);
    }
    open_page(&mut app, Mode::RoutineBuild);

    // Core row -> pick Heal v1.0 (first option that is Heal).
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::RoutineBuildPick);
    let options = app
        .game
        .as_ref()
        .unwrap()
        .build_part_options("Core", &app.routine_draft.parts);
    let at = options.iter().position(|o| o.part == heal()).unwrap();
    pick_row(&mut app, at);
    assert_eq!(app.mode, Mode::RoutineBuild);
    assert_eq!(app.routine_draft.parts, vec![heal()]);

    // Area row -> Group.
    pick_row(&mut app, 2);
    let options = app
        .game
        .as_ref()
        .unwrap()
        .build_part_options("Area", &app.routine_draft.parts);
    let at = options.iter().position(|o| o.part == group()).unwrap();
    pick_row(&mut app, at);
    assert_eq!(app.routine_draft.parts, vec![heal(), group()]);

    // Name row, type, Enter back.
    pick_row(&mut app, NAME_ROW);
    assert_eq!(app.mode, Mode::RoutineBuildName);
    type_text(&mut app, "Mend All");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RoutineBuild);
    assert_eq!(app.routine_draft.name, "Mend All");

    pick_row(&mut app, COMPILE_ROW);
    assert!(app.routine_draft.parts.is_empty(), "the draft resets");
    let game = app.game.as_mut().unwrap();
    let built = game.built_routine_rows();
    assert_eq!(built.len(), 1);
    assert_eq!(built[0].name, "Mend All");
    assert_eq!(game.parts_held(&heal()), 0, "the parts are spent");
}

#[test]
fn a_row_the_core_does_not_take_refuses_with_the_reason() {
    let mut app = app_at_a_workbench(80_003);
    open_page(&mut app, Mode::RoutineBuild);
    pick_row(&mut app, 1);
    assert_eq!(app.mode, Mode::RoutineBuild, "no list opens");
    assert_eq!(app.status_line.as_deref(), Some("Pick a Core first."));
}

#[test]
fn compiling_an_unfinished_build_is_refused_and_keeps_the_draft() {
    let mut app = app_at_a_workbench(80_004);
    open_page(&mut app, Mode::RoutineBuild);
    app.routine_draft.parts = vec![heal()];
    pick_row(&mut app, COMPILE_ROW);
    assert!(app.status_line.is_some());
    assert_eq!(app.routine_draft.parts, vec![heal()]);
}

#[test]
fn the_name_field_stops_at_its_limit_and_backspace_deletes() {
    let mut app = app_at_a_workbench(80_005);
    open_page(&mut app, Mode::RoutineBuild);
    pick_row(&mut app, NAME_ROW);
    type_text(&mut app, &"x".repeat(40));
    assert_eq!(app.routine_draft.name.chars().count(), ROUTINE_NAME_MAX);
    app.handle_key(GameKey::Backspace);
    assert_eq!(app.routine_draft.name.chars().count(), ROUTINE_NAME_MAX - 1);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RoutineBuild);
}

#[test]
fn the_subroutines_page_filters_by_typing_and_enter_crafts() {
    let mut app = app_at_a_workbench(80_006);
    know(&mut app, &heal());
    let disk = feral_processes_engine::items::ItemId::from(
        feral_processes_engine::items::ids::ROUTINE_DISK,
    );
    let game = app.game.as_mut().unwrap();
    game.dev_give_item(&disk, 2).unwrap();
    let (mat, n) = feral_processes_engine::routine_craft::material(&heal());
    game.dev_give_item(&mat, n).unwrap();
    open_page(&mut app, Mode::Subroutines);

    type_text(&mut app, "heal v1");
    let visible = subroutines_visible(app.game.as_ref().unwrap(), app.filter_text());
    assert_eq!(visible.len(), 1);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.game.as_mut().unwrap().parts_held(&heal()), 1);
}

#[test]
fn a_locked_part_refuses_to_craft() {
    let mut app = app_at_a_workbench(80_007);
    open_page(&mut app, Mode::Subroutines);
    type_text(&mut app, "area");
    app.handle_key(GameKey::Enter);
    assert!(app.status_line.is_some());
}

#[test]
fn the_install_list_offers_built_routines_and_installs_one() {
    let mut app = app_at_a_workbench(80_008);
    know(&mut app, &heal());
    hold(&mut app, &heal(), 1);
    let game = app.game.as_mut().unwrap();
    // Free a slot: the starter Patch is in slot 2, so pop it out first.
    let id = game.build_routine("Spare", &[heal()]).unwrap();
    app.handle_key(GameKey::Esc);
    app.mode = Mode::Playing;
    open_via_menu(&mut app, 'p', "Install a routine");
    pick_row(&mut app, 0); // You
    let game = app.game.as_mut().unwrap();
    let player = game.routine_holders()[0].entity;
    let filled = game
        .routine_view(player)
        .iter()
        .position(|s| s.ability.is_some() && !s.fixed);
    if let Some(slot) = filled {
        pick_row(&mut app, slot); // pop out
    }
    let game = app.game.as_mut().unwrap();
    let free = game
        .routine_view(player)
        .iter()
        .position(|s| s.ability.is_none())
        .expect("a free slot");
    pick_row(&mut app, free);
    assert_eq!(app.mode, Mode::RoutineInstall);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::Routines);
    let game = app.game.as_mut().unwrap();
    assert!(
        game.routine_view(player)
            .iter()
            .any(|s| s.ability.as_deref() == Some(id.as_str()))
    );
    assert!(game.built_routine_rows().is_empty());
}
