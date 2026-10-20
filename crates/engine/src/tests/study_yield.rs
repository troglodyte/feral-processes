//! A finished study's program: held at the Research Station
//! (`components::StudyYield`), carried to a Quarantine Rack by an idle body
//! (`game::base::study_yield`), and the room accounting that gates the next
//! completion.

use super::support::*;
use crate::components::{CarryingProgram, DownedPrograms, Racked, StudyYield, Task, TaskKind};
use crate::items::DownedProgram;
use crate::*;

fn program(level: u32) -> DownedProgram {
    DownedProgram {
        species: "scrapper".to_string(),
        level,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    }
}

/// A floored base with a Research Node and one rack.
fn base_with_station_and_rack() -> (Game, Entity, Entity) {
    let mut game = Game::new(5100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    lay_floor_box(&mut game, (-12, -12), (12, 12));
    let station = base_with_a_research_node(&mut game);
    let rack = spawn_machine_at(&mut game, "quarantine_rack", -6, -6);
    (game, station, rack)
}

fn a_task(target: Entity) -> Task {
    Task {
        kind: TaskKind::GatherResource,
        target,
        progress: 0,
        required: 1000,
    }
}

fn hold(game: &mut Game, station: Entity, level: u32) {
    game.hold_at_station(station, program(level));
}

fn held_at(game: &Game, station: Entity) -> Option<DownedProgram> {
    game.world
        .get::<StudyYield>(station)
        .and_then(|y| y.0.clone())
}

fn racked_levels(game: &Game, rack: Entity) -> Vec<u32> {
    game.world
        .get::<Racked>(rack)
        .unwrap()
        .0
        .iter()
        .map(|p| p.level)
        .collect()
}

/// Every place a program from the station can be standing, as levels.
fn all_programs(game: &mut Game, station: Entity, rack: Entity) -> Vec<u32> {
    let mut levels = racked_levels(game, rack);
    levels.extend(held_at(game, station).map(|p| p.level));
    levels.extend(
        game.world
            .query::<&CarryingProgram>()
            .iter(&game.world)
            .map(|c| c.0.level),
    );
    levels.extend(
        game.world
            .get::<DownedPrograms>(game.player_entity())
            .unwrap()
            .0
            .iter()
            .map(|p| p.level),
    );
    levels.sort();
    levels
}

fn tick_until(game: &mut Game, what: &str, mut done: impl FnMut(&mut Game) -> bool) {
    for _ in 0..600 {
        if done(game) {
            return;
        }
        game.tick();
    }
    panic!("never happened: {what}");
}

#[test]
fn an_idle_staffer_carries_the_held_program_to_a_rack_with_room() {
    let (mut game, station, rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);
    let worker = spawn_tamed(&mut game, 100, 3);

    let mut carried = false;
    tick_until(&mut game, "program reaches the rack", |g| {
        carried |= g.world.get::<CarryingProgram>(worker).is_some();
        assert_eq!(
            all_programs(g, station, rack),
            vec![7],
            "the kill is somewhere at every instant"
        );
        racked_levels(g, rack) == vec![7]
    });

    assert!(carried, "a body walked it rather than it teleporting");
    assert!(held_at(&game, station).is_none(), "the slot is emptied");
    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert!(game.world.get::<Task>(worker).is_none());
}

#[test]
fn nothing_leaves_the_station_when_no_rack_has_room() {
    let (mut game, station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    hold(&mut game, station, 7);
    let worker = spawn_tamed(&mut game, 100, 3);

    for _ in 0..120 {
        game.tick();
        assert!(
            game.world.get::<CarryingProgram>(worker).is_none(),
            "picked up with nowhere to go"
        );
    }
    assert_eq!(held_at(&game, station).map(|p| p.level), Some(7));
}

#[test]
fn a_second_rack_takes_it_when_the_first_is_full() {
    let (mut game, station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    let second = spawn_machine_at(&mut game, "quarantine_rack", 6, -6);
    hold(&mut game, station, 7);
    spawn_tamed(&mut game, 100, 3);

    tick_until(&mut game, "program reaches the second rack", |g| {
        racked_levels(g, second) == vec![7]
    });
    assert_eq!(racked_levels(&game, rack).len(), 8);
}

#[test]
fn the_station_never_reads_as_a_rack() {
    let (mut game, station, _rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);
    assert!(game.world.get::<Racked>(station).is_none());
    assert!(
        game.quarantine_racks()
            .iter()
            .all(|&(_, _, e)| e != station),
        "a held program does not make the station a rack"
    );
}

#[test]
fn a_station_still_holding_a_program_stalls_completion_with_its_own_text() {
    let (mut game, station, _rack) = base_with_station_and_rack();
    set_zone(&mut game, 2);
    let subject = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, subject, station);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");
    hold(&mut game, station, 3);

    assert_eq!(
        game.research_material_shortfall(),
        Some("crew to carry the last program to a rack".to_string()),
        "the slot is reported ahead of rack room, in the gate's own order"
    );
    game.tick();
    assert!(!game.is_researched("paging"));
    assert!(
        game.world
            .get::<crate::components::UnderStudy>(subject)
            .is_some(),
        "nothing was spent"
    );
}

#[test]
fn rack_room_counts_programs_held_and_in_transit() {
    let (mut game, station, rack) = base_with_station_and_rack();
    assert_eq!(game.free_rack_room(), 8);
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 5];
    assert_eq!(game.free_rack_room(), 3, "racked");

    hold(&mut game, station, 7);
    assert_eq!(game.free_rack_room(), 2, "held at a station");

    let carrier = spawn_tamed(&mut game, 100, 3);
    game.world
        .entity_mut(carrier)
        .insert(CarryingProgram(program(8)));
    assert_eq!(game.free_rack_room(), 1, "in transit to a rack");

    // A teardown rig's fetcher carries one too, but it is posted and is not
    // heading to a rack.
    let fetcher = spawn_tamed(&mut game, 100, 3);
    game.world
        .entity_mut(fetcher)
        .insert((CarryingProgram(program(9)), a_task(station)));
    assert_eq!(game.free_rack_room(), 1, "a rig fetch is not a rack trip");
}

#[test]
fn rack_room_respects_tier() {
    let (mut game, _station, rack) = base_with_station_and_rack();
    game.world
        .entity_mut(rack)
        .insert(crate::components::StructureTier(2));
    assert_eq!(game.free_rack_room(), 16);
}

#[test]
fn a_full_rack_blocks_completion_and_reports_room_on_a_rack() {
    let (mut game, station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    assert_eq!(
        game.study_yield_blocker(),
        Some("room on a Quarantine Rack")
    );
    game.world.get_mut::<Racked>(rack).unwrap().0.pop();
    assert_eq!(game.study_yield_blocker(), None);
    hold(&mut game, station, 7);
    assert_eq!(
        game.study_yield_blocker(),
        Some("crew to carry the last program to a rack")
    );
}

#[test]
fn a_posted_staffer_may_not_start_the_errand() {
    let (mut game, station, _rack) = base_with_station_and_rack();
    let worker = spawn_tamed(&mut game, 100, 3);
    let amenities = game.amenities_for_test();
    assert!(game.may_rack(worker, &amenities));
    game.world.entity_mut(worker).insert(a_task(station));
    assert!(!game.may_rack(worker, &amenities));
}

#[test]
fn an_idle_carrier_is_not_posted_to_a_job() {
    let (mut game, station, rack) = base_with_station_and_rack();
    let node_worker = spawn_tamed(&mut game, 100, 3);
    game.world
        .entity_mut(node_worker)
        .insert(CarryingProgram(program(4)));
    // Work for it to be handed: an extractor with a recipe.
    spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, -4);

    for _ in 0..40 {
        game.tick();
        if game.world.get::<CarryingProgram>(node_worker).is_none() {
            break;
        }
        assert!(
            game.world.get::<Task>(node_worker).is_none(),
            "a body holding a kill is never handed a post"
        );
    }
    assert_eq!(all_programs(&mut game, station, rack), vec![4]);
}

#[test]
fn a_rack_lost_mid_trip_puts_the_program_back_on_the_station() {
    let (mut game, station, rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);
    let worker = spawn_tamed(&mut game, 100, 3);
    tick_until(&mut game, "pickup", |g| {
        g.world.get::<CarryingProgram>(worker).is_some()
    });

    game.world.despawn(rack);
    game.tick();

    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert_eq!(
        held_at(&game, station).map(|p| p.level),
        Some(7),
        "nowhere to deliver: it goes back where it came from"
    );
}

#[test]
fn demolishing_a_station_holding_a_program_shelves_it() {
    let (mut game, station, rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);

    game.remove_structure(station).unwrap();

    assert!(
        all_programs(&mut game, station, rack) == vec![7],
        "the kill survives the building"
    );
    assert_eq!(racked_levels(&game, rack), vec![7]);
}

#[test]
fn a_station_destroyed_by_damage_shelves_its_program() {
    let (mut game, station, rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);
    game.world
        .entity_mut(station)
        .insert(crate::components::Durability { hp: 10, max_hp: 10 });

    game.damage_structure(station, 100_000, "The Research Node", "a siege");

    assert_eq!(racked_levels(&game, rack), vec![7]);
}

#[test]
fn a_downed_carrier_shelves_what_it_holds() {
    let (mut game, station, rack) = base_with_station_and_rack();
    let worker = spawn_tamed(&mut game, 100, 3);
    game.world
        .entity_mut(worker)
        .insert((CarryingProgram(program(4)), crate::components::Downed));

    game.tick();

    assert!(game.world.get::<CarryingProgram>(worker).is_none());
    assert_eq!(all_programs(&mut game, station, rack), vec![4]);
}

fn round_trip(game: &mut Game, tag: &str) -> Game {
    let path = std::env::temp_dir().join(format!(
        "feral_study_yield_{tag}_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}

#[test]
fn a_save_keeps_the_program_a_station_is_holding() {
    let (mut game, station, _rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);

    let mut loaded = round_trip(&mut game, "held");

    let station = loaded.study_station().expect("the station stands");
    assert_eq!(held_at(&loaded, station).map(|p| p.level), Some(7));
    let _ = &mut loaded;
}

#[test]
fn a_save_keeps_a_carrier_mid_trip_and_it_finishes_the_delivery() {
    let (mut game, station, _rack) = base_with_station_and_rack();
    hold(&mut game, station, 7);
    let worker = spawn_tamed(&mut game, 100, 3);
    tick_until(&mut game, "pickup", |g| {
        g.world.get::<CarryingProgram>(worker).is_some()
    });

    let mut loaded = round_trip(&mut game, "transit");

    let station = loaded.study_station().unwrap();
    let rack = loaded.quarantine_racks()[0].2;
    assert_eq!(
        all_programs(&mut loaded, station, rack),
        vec![7],
        "the carried program survived the reload"
    );
    tick_until(&mut loaded, "delivery after reload", |g| {
        racked_levels(g, rack) == vec![7]
    });
}
