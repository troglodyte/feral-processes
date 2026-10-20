//! The dev keypad's engine half: each `dev_*` cheat calls the real game
//! code, so these assert what a player would get after cheating.

use super::support::*;
use crate::*;

/// A Home on the player's tile and the party in the base, with nothing in
/// the pack: free build has to work from an empty inventory.
fn bare_base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    stand_in_base(&mut game);
    game
}

fn structure_at(game: &mut Game, dx: i32, dy: i32) -> Option<Entity> {
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    let mut query = game.world.query::<(Entity, &Position, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, p, _)| p.x == px + dx && p.y == py + dy)
        .map(|(e, ..)| e)
}

fn build_sites(game: &mut Game) -> usize {
    game.world.query::<&BuildSite>().iter(&game.world).count()
}

#[test]
fn free_build_places_a_complete_structure_with_no_site() {
    let mut game = bare_base(9001);
    game.dev_set_free_build(true);

    file_build(&mut game, "extractor", 1, 0).unwrap();

    assert!(structure_at(&mut game, 1, 0).is_some(), "stood up at once");
    assert_eq!(build_sites(&mut game), 0, "and nothing was filed");
}

#[test]
fn free_build_off_files_a_site_as_before() {
    let mut game = bare_base(9002);
    game.dev_set_free_build(true);
    game.dev_set_free_build(false);
    assert!(!game.dev_free_build());

    file_build(&mut game, "extractor", 1, 0).unwrap();

    assert!(structure_at(&mut game, 1, 0).is_none());
    assert_eq!(build_sites(&mut game), 1);
}

#[test]
fn free_build_upgrades_at_once_and_quotes_nothing() {
    let mut game = bare_base(9003);
    file_build(&mut game, "extractor", 1, 0).unwrap();
    game.dev_set_free_build(true);
    // The first build was filed normally; finish it by switching on and
    // placing a second machine instead.
    file_build(&mut game, "extractor", 2, 0).unwrap();
    let node = structure_at(&mut game, 2, 0).expect("raised at once");
    set_zone(&mut game, 2);

    assert_eq!(game.upgrade_cost(node), Some(vec![]), "the quote is empty");
    file_upgrade(&mut game, node).unwrap();

    assert_eq!(
        game.world.get::<StructureTier>(node).map(|t| t.0),
        Some(2),
        "the tier landed with no crew and no materials"
    );
    assert_eq!(build_sites(&mut game), 1, "only the first, unfinished site");
}

#[test]
fn free_build_rebuild_still_consumes_its_program() {
    let mut game = bare_base(9004);
    game.dev_set_free_build(true);
    // A wreck's rebuild site, awaiting its program.
    let (px, py) = game.base_pos().unwrap();
    let mut site = BuildSite::new(StructureId::from("extractor"), vec![]);
    site.awaiting_program = true;
    let site = game.world.spawn((site, Position { x: px + 1, y: py })).id();
    let program = build_program(&mut game, &"extractor".to_string(), 1).expect("a program");

    game.commit_rebuild_program(site, program).unwrap();

    assert!(structure_at(&mut game, 1, 0).is_some(), "rebuilt at once");
    assert!(
        game.world.get::<BuildSite>(site).is_none(),
        "the site is gone"
    );
    assert!(
        game.world.get_entity(program).is_err(),
        "the program it was rebuilt from is spent"
    );
}

#[test]
fn free_build_does_not_survive_a_save() {
    let mut game = bare_base(9005);
    game.dev_set_free_build(true);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_dev_free_build_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(!loaded.dev_free_build());
}

fn held(game: &Game, id: &str) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(id))
}

fn some_item(game: &Game, gear: bool) -> ItemId {
    game.world
        .resource::<ItemDb>()
        .all()
        .find(|d| d.equipment.is_some() == gear)
        .map(|d| d.id.clone())
        .expect("the shipped items hold both kinds")
}

#[test]
fn give_item_adds_the_quantity() {
    let mut game = bare_base(9101);
    let id = some_item(&game, false);
    let before = held(&game, id.as_str());

    let said = game.dev_give_item(&id, 10).unwrap();

    assert_eq!(held(&game, id.as_str()), before + 10);
    assert!(said.starts_with("10 × "), "{said}");
}

#[test]
fn give_item_refuses_gear_and_unknown_ids() {
    let mut game = bare_base(9102);
    let gear = some_item(&game, true);

    assert!(game.dev_give_item(&gear, 10).is_err());
    assert_eq!(held(&game, gear.as_str()), 0);
    assert!(
        game.dev_give_item(&ItemId::from("no_such_item"), 10)
            .is_err()
    );
}

#[test]
fn plain_items_are_sorted_by_name_and_hold_no_gear() {
    let mut game = bare_base(9103);
    let id = some_item(&game, false);
    game.dev_give_item(&id, 3).unwrap();

    let rows = game.dev_plain_items();

    assert!(!rows.is_empty());
    assert!(rows.windows(2).all(|w| w[0].1 <= w[1].1), "sorted by name");
    let db = game.world.resource::<ItemDb>();
    assert!(
        rows.iter()
            .all(|(id, ..)| db.get(id.as_str()).unwrap().equipment.is_none()),
        "no gear"
    );
    let row = rows.iter().find(|r| r.0 == id).unwrap();
    assert_eq!(
        row.2,
        held(&game, id.as_str()),
        "the held count rides along"
    );
}

#[test]
fn warp_targets_are_the_twenty_zones_ahead() {
    let mut game = bare_base(9201);
    set_zone(&mut game, 3);

    let targets = game.dev_warp_targets();

    assert_eq!(targets.first(), Some(&4));
    assert_eq!(targets.len(), 20);
    assert!(targets.windows(2).all(|w| w[1] == w[0] + 1));
}

#[test]
fn warp_lands_on_the_zone_through_the_real_breach() {
    let mut game = bare_base(9202);
    let from = game.world.resource::<ZoneLevel>().0;

    let said = game.dev_warp_to_zone(from + 2).unwrap();

    assert_eq!(game.world.resource::<ZoneLevel>().0, from + 2);
    assert!(said.contains(&(from + 2).to_string()), "{said}");
    assert!(
        game.dev_warp_to_zone(from).is_err(),
        "warping back stays refused"
    );
}

#[test]
fn warp_is_refused_mid_battle() {
    let mut game = bare_base(9203);
    start_battle_with_a_wild_program(&mut game);
    let from = game.world.resource::<ZoneLevel>().0;

    assert!(game.dev_warp_to_zone(from + 1).is_err());
    assert_eq!(game.world.resource::<ZoneLevel>().0, from);
}
