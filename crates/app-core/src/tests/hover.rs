//! `App::hover_lines` — what the map's delayed hover label says about a
//! tile: whatever is drawn there, then the ground under it.

use super::support::*;
use crate::*;

fn player_pos(app: &App) -> (i32, i32) {
    app.game.as_ref().unwrap().player_status().position
}

fn biome_at(app: &mut App, tile: (i32, i32)) -> String {
    let tiles = app.game.as_mut().unwrap().view_tiles_at(tile, 0, 0);
    tiles[0][0].biome.name().to_string()
}

#[test]
fn a_hostiles_tile_names_the_hostile_then_the_ground() {
    let mut app = test_app(2801);
    place_wild_program_east(&mut app, 4);
    let species = app
        .game
        .as_ref()
        .unwrap()
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .name
        .clone();
    let start = player_pos(&app);
    let tile = (start.0 + 4, start.1);

    let lines = app.hover_lines(tile.0, tile.1);

    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].contains(&species),
        "{lines:?} does not name {species}"
    );
    assert_eq!(lines[1], biome_at(&mut app, tile));
}

#[test]
fn open_ground_names_only_the_ground() {
    let mut app = test_app(2802);
    clear_the_area_around_player(&mut app);
    let start = player_pos(&app);
    let tile = (start.0 + 3, start.1);

    let lines = app.hover_lines(tile.0, tile.1);

    assert_eq!(lines, vec![biome_at(&mut app, tile)]);
}

/// `surface_map_game`, the gate `travel_to` shares: over a popup the map
/// behind it is not what the pointer is pointing at.
#[test]
fn hover_off_the_playing_screen_says_nothing() {
    let mut app = test_app(2803);
    app.mode = Mode::Inventory;
    let start = player_pos(&app);

    assert!(app.hover_lines(start.0, start.1).is_empty());
}

#[test]
fn hover_underground_says_nothing() {
    let mut app = app_underground(2804);
    let start = player_pos(&app);

    assert!(app.hover_lines(start.0, start.1).is_empty());
}

/// A renamed staff member's name is the player's own and says nothing of
/// what it is, so the hover label adds the species after it.
#[test]
fn a_renamed_staff_member_names_its_species_too() {
    let mut app = app_inside_a_small_base_with_programs(2805, false, 1);
    let start = player_pos(&app);
    let game = app.game.as_mut().unwrap();
    let staff = game
        .view_entities_at(start, 0, 0)
        .into_iter()
        .find(|e| e.is_tamed && !e.is_player)
        .expect("the fixture's program stands on the player's tile")
        .entity;
    game.rename_companion(staff, Some("Bob".to_string()))
        .unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .name
        .clone();

    let lines = app.hover_lines(start.0, start.1);

    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Bob") && l.ends_with(&format!("({species})"))),
        "{lines:?}"
    );
}

/// A staff member still on its handle is named just as opaquely — a hex
/// number says nothing of what the program is either.
#[test]
fn a_staff_member_on_its_handle_names_its_species_too() {
    let mut app = app_inside_a_small_base_with_programs(2806, false, 1);
    let start = player_pos(&app);
    let species = app
        .game
        .as_ref()
        .unwrap()
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .name
        .clone();

    let lines = app.hover_lines(start.0, start.1);

    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("0x") && l.ends_with(&format!("({species})"))),
        "{lines:?}"
    );
}
