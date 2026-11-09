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
    pick_row(&mut app, resting);
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
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::BreedSecond);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::BreedConfirm);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BreedSecond);
    assert_eq!(app.pending_breed_second, None);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Breed);
    assert_eq!(app.pending_breed_first, None);
    // The sheet's B lands on the base report's Breed page, so Esc is that
    // hub's: the rail, then out.
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(seeds(&mut app), 1, "backing out is free");
}

#[test]
fn confirming_breeds_and_consumes_the_seed() {
    let mut app = app_with_a_breeding_bay(8106, 1, false);
    let bay = open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line, None);
    assert_eq!(
        app.mode,
        Mode::Breed,
        "back on the base report's Breed page"
    );
    assert_eq!(seeds(&mut app), 0);
    let slots = app.game.as_mut().unwrap().incubations(bay);
    let child = slots[0].child.as_ref().expect("the slot is reserved");
    assert_eq!(
        child.when(),
        "parents on their way",
        "confirming starts a visit, not an incubation"
    );
    assert_eq!(app.pending_breed_first, None);
}

fn breed_on_base_rail(app: &App) -> bool {
    Hub::Base
        .kind()
        .pages(app)
        .iter()
        .any(|p| p.modes().eq([Mode::Breed]))
}

#[test]
fn the_base_report_offers_breeding_only_once_a_bay_stands() {
    let app = app_inside_a_small_base_with_programs(8113, false, 2);
    assert!(!breed_on_base_rail(&app));
    let app = app_with_a_breeding_bay(8113, 1, false);
    assert!(breed_on_base_rail(&app));
}

/// `b`, then `[` from the first page wraps to the last, the Breed page,
/// and Right moves the cursor into it.
fn open_breeding_in_the_base_report(app: &mut App) {
    app.handle_key(GameKey::Char('b'));
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Right);
}

#[test]
fn breeding_from_the_base_report_stays_on_its_page() {
    let mut app = app_with_a_breeding_bay(8114, 1, false);
    let bay = open_the_bay_sheet(&mut app);
    app.pending_structure_manifest = None;
    app.mode = Mode::Playing;
    open_breeding_in_the_base_report(&mut app);
    assert_eq!(app.mode, Mode::Breed);
    assert!(matches!(app.hub_at(), Some((Hub::Base, ..))));
    assert_eq!(app.breeding_bay(), Some(bay));
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::BreedSecond);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Breed, "Esc steps back to the page");
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line, None);
    assert_eq!(app.mode, Mode::Breed);
    assert!(matches!(app.hub_at(), Some((Hub::Base, ..))));
    assert_eq!(seeds(&mut app), 0);
    let slots = app.game.as_mut().unwrap().incubations(bay);
    assert!(slots[0].child.is_some());
}

#[test]
fn the_c_hub_keeps_its_own_breed_page_with_a_bay_standing() {
    let mut app = app_with_a_breeding_bay(8115, 1, false);
    open_the_bay_in_the_hub(&mut app);
    assert!(matches!(app.hub_at(), Some((Hub::Structure, ..))));
}

#[test]
fn a_missing_seed_is_refused_on_the_confirm_page() {
    let mut app = app_with_a_breeding_bay(8107, 0, false);
    open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::BreedConfirm);
    assert_eq!(app.status_line.as_deref(), Some("no breeding seed"));
}

#[test]
fn a_full_bay_offers_no_breeding() {
    let mut app = app_with_a_breeding_bay(8108, 2, false);
    let bay = open_the_bay_sheet(&mut app);
    app.handle_key(GameKey::Char('B'));
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    app.pending_structure_manifest = Some(bay);
    app.mode = Mode::StructureManifest;
    app.handle_key(GameKey::Char('B'));
    assert_eq!(app.mode, Mode::Playing, "no free slot, so B only closes");
}

/// `[c]` beside the fixture's bay, `Tab` past the Extractor east of the
/// player (east comes before west), then into the page.
fn open_the_bay_in_the_hub(app: &mut App) -> Entity {
    let bay = open_the_bay_sheet(app);
    app.pending_structure_manifest = None;
    app.mode = Mode::Playing;
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Tab);
    assert_eq!(
        app.structure_hub.as_ref().map(|h| h.current().entity),
        Some(bay)
    );
    app.handle_key(GameKey::Right);
    bay
}

#[test]
fn c_beside_a_bay_opens_its_breed_page() {
    let mut app = app_with_a_breeding_bay(8109, 1, false);
    let bay = open_the_bay_in_the_hub(&mut app);
    assert_eq!(app.mode, Mode::Breed);
    assert_eq!(app.pending_breed_bay, Some(bay));
    assert!(app.hub_at().is_some(), "the picker is a page of the hub");
}

#[test]
fn breeding_from_the_hub_lands_on_the_bays_info_page() {
    let mut app = app_with_a_breeding_bay(8110, 1, false);
    let bay = open_the_bay_in_the_hub(&mut app);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::BreedSecond);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line, None);
    assert_eq!(app.mode, Mode::StructureManifest);
    assert_eq!(app.pending_structure_manifest, Some(bay));
    assert!(app.structure_hub.is_some(), "still in the hub");
    assert_eq!(seeds(&mut app), 0);
}

#[test]
fn esc_on_the_second_parent_in_the_hub_steps_back_to_the_breed_page() {
    let mut app = app_with_a_breeding_bay(8111, 1, false);
    open_the_bay_in_the_hub(&mut app);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Breed);
    assert!(app.hub_at().is_some());
}

#[test]
fn a_full_bays_breed_page_refuses_a_pick() {
    let mut app = app_with_a_breeding_bay(8112, 2, false);
    open_the_bay_in_the_hub(&mut app);
    pick_row(&mut app, 0);
    pick_row(&mut app, 0);
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::Breed);
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::Breed, "nothing is picked");
    assert_eq!(app.status_line.as_deref(), Some("no free bay"));
}
