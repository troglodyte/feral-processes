//! A program's points from its Manifest: `H` holds them, `S` spends them on
//! the Points screen, and both go back to the same sheet.

use feral_processes_engine::attributes::AttributeId;
use feral_processes_engine::{ManifestSubject, StatOwner};

use super::support::*;
use crate::*;

fn open_stats_tab(app: &mut App, program: Entity) {
    app.pending_manifest = Some(program);
    app.manifest_origin = ManifestOrigin::Roster;
    app.manifest_tab = ManifestTab::Stats;
    app.mode = Mode::Manifest;
}

fn banked(app: &App, program: Entity) -> u32 {
    app.game
        .as_ref()
        .unwrap()
        .stat_points_of(StatOwner::Program(program))
}

fn holding(app: &mut App, program: Entity) -> bool {
    let view = app.game.as_ref().unwrap().manifest(program).unwrap();
    match view.subject {
        ManifestSubject::Program(p) => p.points.expect("seated").holding,
        ManifestSubject::Player(_) => unreachable!(),
    }
}

#[test]
fn h_toggles_holding_on_the_sheets_program() {
    let (mut app, program) = app_owning_a_program_with_points(7401, 0, false);
    open_stats_tab(&mut app, program);

    app.handle_key(GameKey::Char('H'));
    assert!(holding(&mut app, program));
    app.handle_key(GameKey::Char('H'));
    assert!(!holding(&mut app, program));
    assert_eq!(app.mode, Mode::Manifest);
}

#[test]
fn lowercase_h_does_nothing_on_a_sheet() {
    let (mut app, program) = app_owning_a_program_with_points(7402, 0, false);
    open_stats_tab(&mut app, program);
    app.handle_key(GameKey::Char('h'));
    assert!(!holding(&mut app, program));
}

#[test]
fn s_opens_the_points_screen_for_the_program() {
    let (mut app, program) = app_owning_a_program_with_points(7403, 3, true);
    open_stats_tab(&mut app, program);

    app.handle_key(GameKey::Char('S'));

    assert_eq!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    assert_eq!(app.allocation_pool(), 3);
    assert_eq!(
        app.stat_allocation.as_ref().unwrap().purpose(),
        AllocationFor::Owned(StatOwner::Program(program))
    );
}

#[test]
fn s_is_refused_with_no_points() {
    let (mut app, program) = app_owning_a_program_with_points(7404, 0, true);
    open_stats_tab(&mut app, program);

    app.handle_key(GameKey::Char('S'));

    assert_eq!(app.mode, Mode::Manifest);
    assert!(app.status_line.is_some(), "a refused key says why");
}

#[test]
fn esc_keeps_the_points_and_returns_to_the_same_sheet() {
    let (mut app, program) = app_owning_a_program_with_points(7405, 3, true);
    open_stats_tab(&mut app, program);
    app.menu_selected = 2;
    app.handle_key(GameKey::Char('S'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, Some(program));
    assert_eq!(app.manifest_tab, ManifestTab::Stats);
    assert_eq!(app.menu_selected, 2, "the roster's parked row survives");
    assert_eq!(banked(&app, program), 3);
}

#[test]
fn committing_spends_on_the_program_and_not_the_player() {
    let (mut app, program) = app_owning_a_program_with_points(7406, 3, true);
    let player_points = app.game.as_ref().unwrap().player_status().stat_points;
    let analysis = AttributeId::from("analysis");
    let before = app
        .game
        .as_ref()
        .unwrap()
        .attributes_of(program)
        .get(&analysis)
        .unwrap();
    open_stats_tab(&mut app, program);
    app.handle_key(GameKey::Char('S'));
    let row = app
        .allocation_rows()
        .iter()
        .position(|r| matches!(r, CreationRow::Attribute { id, .. } if *id == analysis))
        .unwrap();
    app.menu_selected = row;
    app.handle_key(GameKey::ShiftRight);

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, Some(program));
    assert_eq!(banked(&app, program), 0);
    let after = app
        .game
        .as_ref()
        .unwrap()
        .attributes_of(program)
        .get(&analysis)
        .unwrap();
    assert_eq!(after, before + 3);
    assert_eq!(
        app.game.as_ref().unwrap().player_status().stat_points,
        player_points
    );
}

#[test]
fn the_preview_starts_from_the_programs_own_base() {
    let (mut app, program) = app_owning_a_program_with_points(7407, 3, true);
    open_stats_tab(&mut app, program);
    app.handle_key(GameKey::Char('S'));
    let game = app.game.as_ref().unwrap();
    let live_atk = game.manifest(program).unwrap().atk;
    let atk_row = app
        .allocation_rows()
        .into_iter()
        .find_map(|r| match r {
            CreationRow::Attribute { id, effects, .. } if id == AttributeId::from("analysis") => {
                effects
                    .into_iter()
                    .find(|(s, _, _)| *s == feral_processes_engine::attributes::DerivedStat::Atk)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        atk_row.1 as i32, live_atk,
        "before-figure is the live attack"
    );
}
