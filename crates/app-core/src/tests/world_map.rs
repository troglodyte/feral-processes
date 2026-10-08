//! `Mode::WorldMap`: pan with the arrows, pick a row with a letter or a
//! click, `C` to steer the compass, `P` to come back, Esc to close.

use super::support::*;
use crate::*;
use feral_processes_engine::views::WorldMapMarkKind;

fn open(seed: u32) -> App {
    let mut app = test_app(seed);
    app.handle_key(GameKey::Char('g'));
    assert_eq!(app.mode, Mode::WorldMap);
    app
}

fn party_chunk(app: &mut App) -> (i32, i32) {
    app.game
        .as_mut()
        .unwrap()
        .world_map((0, 0), 0)
        .unwrap()
        .party
}

#[test]
fn opening_centres_on_the_party_chunk() {
    let mut app = open(910);
    let party = party_chunk(&mut app);
    assert_eq!(app.world_map_center, party);
    assert_eq!(app.menu_selected, 0);
}

#[test]
fn arrows_pan_and_p_recentres() {
    let mut app = open(911);
    let home = app.world_map_center;
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Down);
    assert_eq!(app.world_map_center, (home.0 + 1, home.1 + 1));
    app.handle_key(GameKey::Left);
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Up);
    assert_eq!(app.world_map_center, (home.0, home.1 - 1));
    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.world_map_center, home);
    assert_eq!(app.mode, Mode::WorldMap);
}

#[test]
fn esc_returns_to_playing() {
    let mut app = open(912);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn a_letter_selects_a_row_and_arrows_do_not() {
    let mut app = open(913);
    assert!(app.world_map_marks().len() >= 2, "home and a town at least");
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.menu_selected, 1);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.menu_selected, 0);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 0, "Down pans, it does not move the row");
}

#[test]
fn c_on_a_target_row_sets_the_compass() {
    let mut app = open(914);
    let marks = app.world_map_marks();
    let (idx, target) = marks
        .iter()
        .enumerate()
        .find_map(|(i, m)| m.target.map(|t| (i, t)))
        .expect("a mark with a target");
    app.menu_selected = idx;
    app.handle_key(GameKey::Char('C'));
    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .compass_bearing()
            .map(|r| r.target),
        Some(target)
    );
    assert_eq!(app.mode, Mode::WorldMap);
}

#[test]
fn c_on_a_nest_refuses_and_leaves_the_bearing() {
    let mut app = open(915);
    let before = app
        .game
        .as_mut()
        .unwrap()
        .compass_bearing()
        .map(|r| r.target);
    let nest = WorldMapMark {
        chunk: (0, 0),
        kind: WorldMapMarkKind::Nest,
        label: "a nest".into(),
        target: None,
    };
    app.steer_compass_at(&nest);
    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .compass_bearing()
            .map(|r| r.target),
        before
    );
    assert!(app.status_line.as_deref().unwrap_or("").contains("nest"));
}

#[test]
fn clicking_a_marked_chunk_selects_its_row() {
    let mut app = open(916);
    let marks = app.world_map_marks();
    let last = marks.len() - 1;
    app.pick_world_map_chunk(marks[last].chunk);
    assert_eq!(
        app.world_map_marks()[app.menu_selected].chunk,
        marks[last].chunk
    );
    assert_eq!(app.mode, Mode::WorldMap);
}

#[test]
fn clicking_an_empty_chunk_keeps_the_row() {
    let mut app = open(917);
    app.menu_selected = 1;
    let marks = app.world_map_marks();
    let empty = (i32::MIN / 2, i32::MIN / 2);
    assert!(marks.iter().all(|m| m.chunk != empty));
    app.pick_world_map_chunk(empty);
    assert_eq!(app.menu_selected, 1);
}

#[test]
fn clicking_a_shared_chunk_again_steps_through_its_marks() {
    let mut app = open(901);
    let marks = app.world_map_marks();
    let chunk = marks[0].chunk;
    let shared: Vec<usize> = (0..marks.len())
        .filter(|&i| marks[i].chunk == chunk)
        .collect();
    assert!(shared.len() >= 2, "seed 901 stacks marks on home's chunk");
    let mut seen = Vec::new();
    for _ in 0..shared.len() {
        app.pick_world_map_chunk(chunk);
        seen.push(app.menu_selected);
    }
    seen.sort_unstable();
    assert_eq!(seen, shared, "each mark on the chunk once per lap");
}

#[test]
fn clicking_a_row_selects_it_and_past_the_list_does_nothing() {
    let mut app = open(919);
    let len = app.world_map_marks().len();
    app.pick_world_map_row(len - 1);
    assert_eq!(app.menu_selected, len - 1);
    app.pick_world_map_row(len);
    assert_eq!(app.menu_selected, len - 1);
}
