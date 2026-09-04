//! `Mode::SpritePicker` and `Mode::SpriteEditor` — the subject list, both
//! gates, the editing screen, the write cue and the pointer entry point.
//!
//! Both gates are fields read once in `App::new` (or set by
//! `install_sprite_dir`), not a live env lookup, so a test can open them
//! without touching an environment the parallel suite shares — the same
//! reasoning `dev_arena_enabled` records.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use feral_processes_engine::icon::Canvas;

use super::support::test_app;
use crate::*;

/// An app sitting on the main menu with both gates open — the flag and a
/// sprite dir installed.
fn app_with_sprite_forge(seed: u32) -> App {
    let mut app = test_app(seed);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.sprite_forge_flag = true;
    app.install_sprite_dir(PathBuf::from("/does/not/matter"));
    app
}

#[test]
fn neither_gate_open_leaves_the_row_absent_and_the_mode_unreachable() {
    let mut app = test_app(1);
    app.game = None;
    app.mode = Mode::MainMenu;
    // The default `test_app` state, made explicit: no flag, no dir.
    app.sprite_forge_flag = false;

    assert!(!app.sprite_forge_enabled());
    app.handle_key(GameKey::Char('d'));

    assert_eq!(
        app.mode,
        Mode::MainMenu,
        "the row must not be reachable with neither gate open"
    );
}

#[test]
fn the_flag_alone_is_not_enough_without_a_checkout() {
    let mut app = test_app(2);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.sprite_forge_flag = true;
    // No `install_sprite_dir` call — an installed build's whole story:
    // there is no repo to write art into, so the row must stay off even
    // with the flag set.

    assert!(!app.sprite_forge_enabled());
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn a_dir_alone_is_not_enough_without_the_flag() {
    let mut app = test_app(3);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.install_sprite_dir(PathBuf::from("/does/not/matter"));
    // `sprite_forge_flag` left false.

    assert!(!app.sprite_forge_enabled());
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn both_gates_open_d_opens_the_picker_and_esc_returns() {
    let mut app = app_with_sprite_forge(4);
    assert!(app.sprite_forge_enabled());

    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::SpritePicker);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::MainMenu);
}

#[test]
fn sprite_subjects_is_every_species_and_structure_plus_player_and_anchor() {
    let mut app = app_with_sprite_forge(5);

    let subjects = app.sprite_subjects();

    assert_eq!(
        subjects.len(),
        49,
        "17 species + 30 structures + player + anchor, with no shipped \
         sprite_name overlaps to de-duplicate away"
    );
    let names: Vec<&str> = subjects.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"player"),
        "the engine's DEFAULT_PLAYER_SPRITE"
    );
    assert!(
        names.contains(&"anchor"),
        "the name hardcoded at render/base.rs:1379"
    );

    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        names, sorted,
        "sprite_subjects must already be sorted and de-duplicated by name"
    );
}

/// Fix round 1: a subject's colour must be the def's own `GlyphColor`, not a
/// placeholder — checked against two real shipped defs (`assets/species/
/// cipher.ron`, `assets/structures/annealing_node.ron`, both `color: Cyan`)
/// rather than trusting the loader wired the field through — and the two
/// hardcoded subjects must land on exactly the values `render/base.rs`
/// actually draws them in: `None` for the player (a role colour, not a
/// `GlyphColor`) and `Some(Gray)` for the anchor (its literal spawned
/// colour, `game/lifecycle.rs`).
#[test]
fn sprite_subjects_carry_the_defs_own_colour_and_the_player_has_none() {
    use feral_processes_engine::components::GlyphColor;

    let mut app = app_with_sprite_forge(7);
    let subjects = app.sprite_subjects();
    let by_name = |name: &str| subjects.iter().find(|s| s.name == name).unwrap();

    assert_eq!(
        by_name("player").color,
        None,
        "the player wears the PLAYER role colour, not an authored GlyphColor"
    );
    assert_eq!(
        by_name("anchor").color,
        Some(GlyphColor::Gray),
        "the anchor has no role override, so its colour is an ordinary lookup"
    );
    assert_eq!(by_name("cipher").color, Some(GlyphColor::Cyan));
    assert_eq!(by_name("annealing_node").color, Some(GlyphColor::Cyan));
}

#[test]
fn sprite_subjects_reads_art_state_off_the_installed_library() {
    let mut app = app_with_sprite_forge(6);
    let mut enabled = HashMap::new();
    enabled.insert("anchor".to_string(), Canvas::new(16));
    let mut disabled = HashSet::new();
    disabled.insert("player".to_string());
    app.install_sprite_library(enabled, disabled);

    let subjects = app.sprite_subjects();
    let by_name = |name: &str| subjects.iter().find(|s| s.name == name).unwrap();

    assert_eq!(by_name("anchor").art, SpriteArt::On);
    assert_eq!(by_name("player").art, SpriteArt::Off);
    let neither = subjects
        .iter()
        .find(|s| s.name != "anchor" && s.name != "player")
        .expect("at least one subject with no installed or disabled art");
    assert_eq!(neither.art, SpriteArt::None);
}

/// The regression a naive whole-row cache would introduce: the
/// name/label/glyph triples are parsed once and reused, but `art` must stay
/// live, or a sprite saved mid-session (Task 8's whole point — the map
/// updating without a restart) would never show up on this screen either.
#[test]
fn the_static_list_is_cached_but_art_state_stays_live() {
    let mut app = app_with_sprite_forge(7);

    let first = app.sprite_subjects();
    let second = app.sprite_subjects();
    assert_eq!(
        first, second,
        "two reads before anything changes must agree, cache or no cache"
    );

    // Installed *between* the two reads above and the one below — this is
    // what a cache that captured `art` alongside the static fields would
    // get wrong.
    let mut enabled = HashMap::new();
    enabled.insert("anchor".to_string(), Canvas::new(16));
    app.install_sprite_library(enabled, HashSet::new());

    let third = app.sprite_subjects();
    assert_eq!(
        third.len(),
        second.len(),
        "the cached static list must not silently change shape"
    );
    let anchor = third.iter().find(|s| s.name == "anchor").unwrap();
    assert_eq!(
        anchor.art,
        SpriteArt::On,
        "art must be read live even though the name/label/glyph list is cached"
    );
}

// ---- Mode::SpriteEditor -----------------------------------------------

/// Opens `Mode::SpriteEditor` on `name` through the picker's own `Enter`
/// key, the one real door into it — mirrors how a player reaches it.
fn open_editor(app: &mut App, name: &str) {
    let subjects = app.sprite_subjects();
    let idx = subjects
        .iter()
        .position(|s| s.name == name)
        .unwrap_or_else(|| panic!("no subject named {name}"));
    app.mode = Mode::SpritePicker;
    app.menu_selected = idx;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SpriteEditor, "Enter must open the editor");
}

#[test]
fn opening_a_subject_with_art_loads_the_installed_canvas_and_one_without_art_opens_blank() {
    let mut app = app_with_sprite_forge(20);
    let mut art = Canvas::new(16);
    art.set(0, 0, 5);
    let mut enabled = HashMap::new();
    enabled.insert("anchor".to_string(), art);
    app.install_sprite_library(enabled, HashSet::new());

    let subjects = app.sprite_subjects();
    let blank_name = subjects
        .iter()
        .find(|s| s.name != "anchor")
        .expect("at least one subject with no installed art")
        .name
        .clone();

    open_editor(&mut app, "anchor");
    let view = app.sprite_editor_view().expect("editor open");
    assert_eq!(view.subject, "anchor");
    assert_eq!(
        view.canvas.cells[0], 5,
        "the installed canvas loaded, not a blank one"
    );

    open_editor(&mut app, &blank_name);
    let view = app.sprite_editor_view().expect("editor open");
    assert_eq!(view.subject, blank_name);
    assert!(
        view.canvas.cells.iter().all(|&c| c == 0),
        "a subject with no installed art opens on a blank canvas"
    );
}

#[test]
fn g_toggles_the_brush_and_the_view_reports_it() {
    let mut app = app_with_sprite_forge(21);
    open_editor(&mut app, "anchor");

    assert_eq!(app.sprite_editor_view().unwrap().canvas.brush, 1);
    app.handle_key(GameKey::Char('g'));
    assert_eq!(app.sprite_editor_view().unwrap().canvas.brush, 2);
    app.handle_key(GameKey::Char('g'));
    assert_eq!(app.sprite_editor_view().unwrap().canvas.brush, 1);
}

#[test]
fn s_queues_exactly_one_save_carrying_the_edited_canvas() {
    let mut app = app_with_sprite_forge(22);
    open_editor(&mut app, "anchor");
    app.handle_key(GameKey::Char(' ')); // paints the cursor cell (0, 0)
    app.handle_key(GameKey::Char('s'));

    let writes = app.take_sprite_writes();
    assert_eq!(writes.len(), 1, "exactly one cue, not one per keystroke");
    assert_eq!(writes[0].name, "anchor");
    match &writes[0].op {
        SpriteOp::Save(canvas) => assert_eq!(canvas.get(0, 0), 1, "the edit is in the cue"),
        other => panic!("expected SpriteOp::Save, got {other:?}"),
    }
}

#[test]
fn esc_queues_nothing_and_returns_to_the_picker() {
    let mut app = app_with_sprite_forge(23);
    open_editor(&mut app, "anchor");
    app.handle_key(GameKey::Char(' ')); // an edit Esc must discard silently
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::SpritePicker);
    assert!(
        app.take_sprite_writes().is_empty(),
        "Esc must not queue a write for the edit it is discarding"
    );
}

#[test]
fn picker_t_queues_disable_for_an_on_subject_and_enable_for_an_off_one() {
    let mut app = app_with_sprite_forge(24);
    let mut enabled = HashMap::new();
    enabled.insert("anchor".to_string(), Canvas::new(16));
    let mut disabled = HashSet::new();
    disabled.insert("player".to_string());
    app.install_sprite_library(enabled, disabled);

    let subjects = app.sprite_subjects();
    let anchor_idx = subjects.iter().position(|s| s.name == "anchor").unwrap();
    let player_idx = subjects.iter().position(|s| s.name == "player").unwrap();
    let none_idx = subjects
        .iter()
        .position(|s| s.art == SpriteArt::None)
        .expect("at least one subject with no art at all");

    app.mode = Mode::SpritePicker;
    app.menu_selected = anchor_idx;
    app.handle_key(GameKey::Char('t'));
    app.menu_selected = player_idx;
    app.handle_key(GameKey::Char('t'));
    app.menu_selected = none_idx;
    app.handle_key(GameKey::Char('t'));

    let writes = app.take_sprite_writes();
    assert_eq!(
        writes.len(),
        2,
        "the SpriteArt::None subject has nothing to toggle and queues nothing"
    );
    assert_eq!(writes[0].name, "anchor");
    assert_eq!(writes[0].op, SpriteOp::Disable);
    assert_eq!(writes[1].name, "player");
    assert_eq!(writes[1].op, SpriteOp::Enable);
}

#[test]
fn take_sprite_writes_drains_so_a_second_call_is_empty() {
    let mut app = app_with_sprite_forge(25);
    open_editor(&mut app, "anchor");
    app.handle_key(GameKey::Char('s'));

    assert_eq!(app.take_sprite_writes().len(), 1);
    assert!(app.take_sprite_writes().is_empty());
}

#[test]
fn a_down_three_drags_and_an_up_is_one_undo_entry() {
    let mut app = app_with_sprite_forge(26);
    open_editor(&mut app, "anchor");

    app.handle_pointer(
        PointerHit::Cell(0, 0),
        PointerButton::Primary,
        PointerPhase::Down,
    );
    app.handle_pointer(
        PointerHit::Cell(1, 0),
        PointerButton::Primary,
        PointerPhase::Drag,
    );
    app.handle_pointer(
        PointerHit::Cell(2, 0),
        PointerButton::Primary,
        PointerPhase::Drag,
    );
    app.handle_pointer(
        PointerHit::Cell(3, 0),
        PointerButton::Primary,
        PointerPhase::Drag,
    );
    app.handle_pointer(
        PointerHit::Cell(3, 0),
        PointerButton::Primary,
        PointerPhase::Up,
    );

    let view = app.sprite_editor_view().unwrap();
    for x in 0..4u8 {
        assert_eq!(
            view.canvas.cells[x as usize], view.canvas.selected,
            "cell ({x}, 0) painted during the drag"
        );
    }

    app.handle_key(GameKey::Char('u'));
    let view = app.sprite_editor_view().unwrap();
    assert!(
        view.canvas.cells.iter().all(|&c| c == 0),
        "one undo takes back the whole four-cell stroke"
    );
}

#[test]
fn secondary_button_paints_index_zero_erase() {
    let mut app = app_with_sprite_forge(27);
    open_editor(&mut app, "anchor");
    app.handle_pointer(
        PointerHit::Cell(2, 2),
        PointerButton::Primary,
        PointerPhase::Down,
    );
    app.handle_pointer(
        PointerHit::Cell(2, 2),
        PointerButton::Primary,
        PointerPhase::Up,
    );
    assert_ne!(
        app.sprite_editor_view().unwrap().canvas.cells[2 * 16 + 2],
        0
    );

    app.handle_pointer(
        PointerHit::Cell(2, 2),
        PointerButton::Secondary,
        PointerPhase::Down,
    );
    app.handle_pointer(
        PointerHit::Cell(2, 2),
        PointerButton::Secondary,
        PointerPhase::Up,
    );
    assert_eq!(
        app.sprite_editor_view().unwrap().canvas.cells[2 * 16 + 2],
        0,
        "Secondary erases, same as Backspace"
    );
}

#[test]
fn a_swatch_hit_selects_it() {
    let mut app = app_with_sprite_forge(28);
    open_editor(&mut app, "anchor");
    assert_eq!(app.sprite_editor_view().unwrap().canvas.selected, 1);

    app.handle_pointer(
        PointerHit::Swatch(6),
        PointerButton::Primary,
        PointerPhase::Down,
    );
    app.handle_pointer(
        PointerHit::Swatch(6),
        PointerButton::Primary,
        PointerPhase::Up,
    );
    assert_eq!(app.sprite_editor_view().unwrap().canvas.selected, 6);
}

#[test]
fn a_pointer_event_outside_the_sprite_editor_changes_nothing() {
    let mut app = app_with_sprite_forge(29);
    // Not open at all.
    app.mode = Mode::MainMenu;
    app.handle_pointer(
        PointerHit::Cell(0, 0),
        PointerButton::Primary,
        PointerPhase::Down,
    );
    assert!(app.sprite_editor_view().is_none());

    // Open, but the picker rather than the editor is on screen.
    open_editor(&mut app, "anchor");
    app.mode = Mode::SpritePicker;
    app.handle_pointer(
        PointerHit::Cell(0, 0),
        PointerButton::Primary,
        PointerPhase::Down,
    );
    let view = app.sprite_editor_view().unwrap();
    assert!(
        view.canvas.cells.iter().all(|&c| c == 0),
        "a pointer event while any other mode is on screen must be dropped"
    );
    assert!(app.take_sprite_writes().is_empty());
}
