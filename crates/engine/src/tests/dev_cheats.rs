//! The dev keypad's engine half: each `dev_*` cheat calls the real game
//! code, so these assert what a player would get after cheating.

use super::support::*;
use crate::resources::{ActiveResearch, WorkOrders};
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

/// The counterpart that makes the program assertion above mean something:
/// the program is spent either way, and it is the structure standing at once
/// that free build adds.
#[test]
fn rebuild_without_free_build_spends_the_program_but_raises_nothing_yet() {
    let mut game = bare_base(9006);
    let (px, py) = game.base_pos().unwrap();
    let mut site = BuildSite::new(StructureId::from("extractor"), vec![]);
    site.awaiting_program = true;
    let site = game.world.spawn((site, Position { x: px + 1, y: py })).id();
    let program = build_program(&mut game, &"extractor".to_string(), 1).expect("a program");

    game.commit_rebuild_program(site, program).unwrap();

    assert!(game.world.get_entity(program).is_err(), "program spent");
    assert!(structure_at(&mut game, 1, 0).is_none(), "not raised yet");
    assert!(game.world.get::<BuildSite>(site).is_some(), "still a site");
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

fn research_node(game: &Game, want: impl Fn(&crate::research::ResearchDef) -> bool) -> ResearchId {
    game.world
        .resource::<ResearchDb>()
        .all()
        .find(|d| want(d))
        .map(|d| d.id.clone())
        .expect("a shipped node of that shape")
}

#[test]
fn unlock_research_grants_a_blocked_node_without_a_bill() {
    let mut game = bare_base(9301);
    let id = research_node(&game, |d| {
        !d.requires.is_empty() && d.teaches.is_none() && d.cost > 0
    });
    assert!(!game.is_researched(id.as_str()));

    let said = game.dev_unlock_research(&id).unwrap();

    assert!(game.is_researched(id.as_str()), "{said}");
}

#[test]
fn unlock_research_clears_the_active_node_it_grants() {
    let mut game = bare_base(9302);
    let id = research_node(&game, |d| d.teaches.is_none() && d.cost > 0);
    let other = research_node(&game, |d| d.teaches.is_none() && d.id != id);
    game.world.resource_mut::<ActiveResearch>().id = Some(id.clone());

    game.dev_unlock_research(&id).unwrap();
    assert!(game.world.resource::<ActiveResearch>().id.is_none());

    game.world.resource_mut::<ActiveResearch>().id = Some(id.clone());
    game.dev_unlock_research(&other).unwrap();
    assert_eq!(
        game.world.resource::<ActiveResearch>().id,
        Some(id),
        "unlocking some other node leaves the active one alone"
    );
}

#[test]
fn unlocking_another_node_keeps_the_active_projects_material_orders() {
    let mut game = bare_base(9305);
    let id = research_node(&game, |d| d.teaches.is_none() && d.cost > 0);
    let other = research_node(&game, |d| d.teaches.is_none() && d.id != id);
    game.world.resource_mut::<ActiveResearch>().id = Some(id.clone());
    let order = WorkOrder::batch(ItemId::from("core_fragment"), 3).with_research();
    game.world.resource_mut::<WorkOrders>().0.push(order);

    game.dev_unlock_research(&other).unwrap();
    assert!(
        game.work_orders().iter().any(|o| o.for_research),
        "the active project's orders are filed once, at selection"
    );

    game.dev_unlock_research(&id).unwrap();
    assert!(
        game.work_orders().iter().all(|o| !o.for_research),
        "granting the active node itself withdraws them"
    );
}

#[test]
fn unlock_research_refuses_a_granted_or_unknown_node() {
    let mut game = bare_base(9303);
    let id = research_node(&game, |d| d.teaches.is_none());
    game.dev_unlock_research(&id).unwrap();

    assert!(game.dev_unlock_research(&id).is_err(), "already granted");
    assert!(
        game.dev_unlock_research(&ResearchId::from("no_such_node"))
            .is_err()
    );
}

#[test]
fn unlock_research_puts_a_routine_node_in_known_routines() {
    let mut game = bare_base(9304);
    let id = research_node(&game, |d| d.teaches.is_some());
    let ability = game
        .world
        .resource::<ResearchDb>()
        .get(id.as_str())
        .unwrap()
        .teaches
        .clone()
        .unwrap();

    game.dev_unlock_research(&id).unwrap();

    assert!(game.knows_routine(&ability));
    assert!(!game.world.resource::<Research>().0.contains(&id));
}

/// The species of the one program `dev_spawn_companion` added.
fn spawn_companion(game: &mut Game) -> Result<String, (String, String)> {
    let before: Vec<Entity> = game.owned_pets().iter().map(|p| p.entity).collect();
    let said = game.dev_spawn_companion().map_err(|e| (e, String::new()))?;
    let added: Vec<Entity> = game
        .owned_pets()
        .iter()
        .map(|p| p.entity)
        .filter(|e| !before.contains(e))
        .collect();
    assert_eq!(added.len(), 1, "exactly one companion joined: {said}");
    Ok(game
        .world
        .get::<Creature>(added[0])
        .unwrap()
        .species
        .clone())
}

fn on_a_walkable_field_tile(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let at = *game.world.get::<Position>(player).unwrap();
    assert!(
        game.world
            .resource_mut::<WorldMap>()
            .tile(at.x, at.y)
            .walkable
    );
    game
}

#[test]
fn spawn_companion_draws_from_the_tiles_pool_and_joins_the_roster() {
    let mut game = on_a_walkable_field_tile(9401);
    let at = *game.world.get::<Position>(game.player_entity()).unwrap();
    let (pool, _) = game.habitat_pools(at.x, at.y, None, 0).unwrap();

    let species = spawn_companion(&mut game).unwrap();

    assert!(pool.contains(&species), "{species} not in {pool:?}");
}

#[test]
fn spawn_companion_is_deterministic_per_seed() {
    let first = spawn_companion(&mut on_a_walkable_field_tile(9402)).unwrap();
    let second = spawn_companion(&mut on_a_walkable_field_tile(9402)).unwrap();

    assert_eq!(first, second);
}

#[test]
fn spawn_companion_refuses_an_unwalkable_tile() {
    let mut game = on_a_walkable_field_tile(9403);
    let player = game.player_entity();
    let blocked = (-40..40)
        .flat_map(|x| (-40..40).map(move |y| (x, y)))
        .find(|&(x, y)| !game.world.resource_mut::<WorldMap>().tile(x, y).walkable)
        .expect("the zone has some wall");
    game.world.entity_mut(player).insert(Position {
        x: blocked.0,
        y: blocked.1,
    });
    let pets = game.owned_pets().len();

    assert!(game.dev_spawn_companion().is_err());
    assert_eq!(game.owned_pets().len(), pets);
}

#[test]
fn spawn_companion_refuses_a_full_roster() {
    let mut game = on_a_walkable_field_tile(9404);
    let player = game.player_entity();
    while game.roster_room() > 0 {
        game.world.spawn(Tamed { owner: player });
    }
    let pets = game.pet_count();

    let err = game.dev_spawn_companion().unwrap_err();

    assert_eq!(err, "Your roster is full.");
    assert_eq!(game.pet_count(), pets);
}
