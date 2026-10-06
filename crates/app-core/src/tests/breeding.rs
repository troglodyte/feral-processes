//! The three-page breeding flow, from the bay's sheet to `Game::breed`.

use super::support::*;
use crate::*;

fn seeds(app: &mut App) -> u32 {
    let path = scratch_path("breeding_seeds", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let data = feral_processes_engine::save::load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    data.player
        .inventory
        .iter()
        .find(|(id, _)| id.as_str() == "breeding_seed")
        .map_or(0, |(_, n)| *n)
}

#[test]
fn b_on_a_bay_sheet_opens_the_first_parent_page() {
    let mut app = app_with_a_breeding_bay(8101, 1, false);
    let bay = open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Breed);
    assert_eq!(app.pending_breed_bay, Some(bay));
}

#[test]
fn a_lowercase_b_does_not_open_it() {
    let mut app = app_with_a_breeding_bay(8102, 1, false);
    open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn b_on_a_sheet_that_is_not_a_bay_just_closes_it() {
    let mut app = app_inside_a_small_base_with_programs(8103, false, 0);
    let node = app
        .game
        .as_mut()
        .unwrap()
        .structure_report()
        .into_iter()
        .find(|s| s.label != "Home")
        .unwrap()
        .entity;
    app.pending_structure_manifest = Some(node);
    app.mode = Mode::StructureManifest;
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_breed_bay, None);
}

#[test]
fn a_resting_program_is_refused_with_the_engines_reason() {
    let mut app = app_with_a_breeding_bay(8104, 1, true);
    open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    let resting = app
        .game
        .as_mut()
        .unwrap()
        .owned_pets()
        .iter()
        .position(|p| p.entity == app_first_resting(&mut app))
        .unwrap();
    app.handle_key(GameKey::Char(menu_shortcut(resting)));
    assert_eq!(app.mode, Mode::Breed, "a resting program is not picked");
    assert_eq!(app.status_line.as_deref(), Some("ready later"));
    assert_eq!(app.pending_breed_first, None);
}

fn app_first_resting(app: &mut App) -> Entity {
    let game = app.game.as_mut().unwrap();
    game.owned_pets()
        .into_iter()
        .map(|p| p.entity)
        .find(|e| game.breed_refusal(*e).is_some())
        .expect("one program is resting")
}

#[test]
fn esc_backs_out_one_page_at_a_time_and_spends_nothing() {
    let mut app = app_with_a_breeding_bay(8105, 1, false);
    open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::BreedSecond);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::BreedConfirm);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BreedSecond);
    assert_eq!(app.pending_breed_second, None);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Breed);
    assert_eq!(app.pending_breed_first, None);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_breed_bay, None);
    assert_eq!(seeds(&mut app), 1, "backing out is free");
}

#[test]
fn confirming_breeds_and_consumes_the_seed() {
    let mut app = app_with_a_breeding_bay(8106, 1, false);
    let bay = open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line, None);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(seeds(&mut app), 0);
    let slots = app.game.as_mut().unwrap().incubations(bay);
    assert!(slots[0].child.is_some(), "the child is incubating");
    assert_eq!(app.pending_breed_first, None);
}

#[test]
fn a_missing_seed_is_refused_on_the_confirm_page() {
    let mut app = app_with_a_breeding_bay(8107, 0, false);
    open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::BreedConfirm);
    assert_eq!(app.status_line.as_deref(), Some("no breeding seed"));
}

#[test]
fn a_full_bay_offers_no_breeding() {
    let mut app = app_with_a_breeding_bay(8108, 2, false);
    let bay = open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);
    app.pending_structure_manifest = Some(bay);
    app.mode = Mode::StructureManifest;
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Playing, "no free slot, so B only closes");
}
