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

/// An extractor with an order behind it: the scheduler wants it, which the
/// control body proves by being posted within a few beats. Without that the
/// carrier's "never posted" would pass whether or not anything held it back.
fn wanted_job(game: &mut Game) -> Entity {
    stand_in_base(game);
    let machine = spawn_machine_with_recipe_at(game, "extractor", "core_fragment", 2, -4);
    game.queue_work_order(WorkOrder::batch(ItemId::from("core_fragment"), 5))
        .unwrap();
    machine
}

#[test]
fn the_scheduler_wants_the_job_a_carrier_is_kept_from() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    wanted_job(&mut game);
    let free = spawn_tamed(&mut game, 100, 3);
    game.tick();
    assert!(
        game.world.get::<Task>(free).is_some(),
        "the fixture's job is wanted, so an idle body is posted to it"
    );
}

#[test]
fn an_idle_carrier_is_not_posted_to_a_job() {
    let (mut game, station, rack) = base_with_station_and_rack();
    wanted_job(&mut game);
    let carrier = a_carrier(&mut game);

    // The rack is nine steps off, so delivery cannot land inside these beats.
    for _ in 0..4 {
        game.tick();
        assert!(
            game.world.get::<CarryingProgram>(carrier).is_some(),
            "delivered too soon for the assertion below to mean anything"
        );
        assert!(
            game.world.get::<Task>(carrier).is_none(),
            "a body holding a kill is never handed a post"
        );
    }
    assert_eq!(all_programs(&mut game, station, rack), vec![7]);
}

/// A rig's fetcher is posted while it carries. If the matching ever
/// un-seats it, it loses its `Task` still holding the kill, and then it is
/// neither a fetcher nor a rack carrier by the narrow predicate: uncounted
/// and never delivered. Any body holding a program is held in place.
#[test]
fn a_posted_fetcher_holding_a_program_is_never_re_matched() {
    let (mut game, _station, rack) = base_with_station_and_rack();
    let job = wanted_job(&mut game);
    let fetcher = a_carrier(&mut game);
    // Posted to something the scheduler does not want, so only the forced
    // edge keeps it from being freed or sent to the wanted job.
    game.world.entity_mut(fetcher).insert(a_task(rack));

    for _ in 0..4 {
        game.tick();
        let task = game.world.get::<Task>(fetcher);
        assert_eq!(
            task.map(|t| t.target),
            Some(rack),
            "a fetcher holding a kill kept its post (the wanted job is {job:?})"
        );
        assert!(game.world.get::<CarryingProgram>(fetcher).is_some());
    }
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

/// Every program a base keeps for later, as levels: rack shelves, station
/// slots and the floor. Not a body's hands and not the player's pack: a
/// door that ends a hold must have put the kill somewhere that outlasts the
/// body, and research never uses the pack.
fn kept(game: &mut Game) -> Vec<u32> {
    let mut levels: Vec<u32> = game
        .world
        .query::<&Racked>()
        .iter(&game.world)
        .flat_map(|r| r.0.iter().map(|p| p.level))
        .collect();
    levels.extend(
        game.world
            .query::<&StudyYield>()
            .iter(&game.world)
            .filter_map(|y| y.0.as_ref().map(|p| p.level)),
    );
    levels.extend(
        game.world
            .query::<&crate::components::FloorItem>()
            .iter(&game.world)
            .filter_map(|f| match &f.0 {
                crate::items::FloorThing::Program(p) => Some(p.level),
                _ => None,
            }),
    );
    levels.sort();
    levels
}

fn pack_programs(game: &Game) -> usize {
    game.world
        .get::<DownedPrograms>(game.player_entity())
        .unwrap()
        .0
        .len()
}

/// A task-less body holding a level-7 kill: a rack carrier mid-trip.
fn a_carrier(game: &mut Game) -> Entity {
    let carrier = spawn_tamed(game, 100, 3);
    game.world
        .entity_mut(carrier)
        .insert(CarryingProgram(program(7)));
    carrier
}

fn assert_handed_back(game: &mut Game, carrier: Option<Entity>) {
    if let Some(carrier) = carrier {
        assert!(
            game.world.get::<CarryingProgram>(carrier).is_none(),
            "the body that left the labour pool still holds the kill"
        );
    }
    assert_eq!(kept(game), vec![7], "the kill was lost with the door");
    assert_eq!(pack_programs(game), 0, "research never uses the pack");
}

#[test]
fn pinning_a_carrier_hands_back_its_program() {
    let (mut game, station, _rack) = base_with_station_and_rack();
    let carrier = a_carrier(&mut game);
    game.pin_subject(carrier, station).unwrap();
    assert_handed_back(&mut game, Some(carrier));
}

#[test]
fn spending_a_carrier_on_a_build_hands_back_its_program() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    let carrier = a_carrier(&mut game);
    spawn_tamed(&mut game, 100, 3);
    assert!(game.commit_program(carrier).is_some());
    assert_handed_back(&mut game, None);
}

#[test]
fn wielding_a_carrier_hands_back_its_program() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    let carrier = a_carrier(&mut game);
    game.wield_program(carrier).unwrap();
    assert_handed_back(&mut game, Some(carrier));
}

#[test]
fn enlisting_a_carrier_hands_back_its_program() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    let carrier = a_carrier(&mut game);
    enlist(&mut game, carrier);
    assert_handed_back(&mut game, Some(carrier));
}

#[test]
fn selling_a_carrier_hands_back_its_program() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    let carrier = a_carrier(&mut game);
    game.dissolve_tamed_program(carrier, crate::bonds::Departure::LetGo);
    assert_handed_back(&mut game, None);
}

#[test]
fn a_permadeath_carrier_hands_back_its_program() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    *game.world.resource_mut::<DifficultyMode>() = DifficultyMode::Permadeath;
    let carrier = a_carrier(&mut game);
    game.bench_or_dissolve(carrier);
    assert_handed_back(&mut game, None);
}

#[test]
fn fusing_a_carrier_hands_back_its_program() {
    let (mut game, _station, _rack) = base_with_station_and_rack();
    unlock_research_chain(&mut game, "program_refactoring");
    let carrier = a_carrier(&mut game);
    let other = spawn_tamed(&mut game, 100, 3);
    game.fuse_companions(carrier, other, None).unwrap();
    assert_handed_back(&mut game, None);
}

#[test]
fn dispatching_a_carrier_on_a_sortie_hands_back_its_program() {
    let (mut game, site, staff) = super::sorties::a_base_ready_to_dispatch(5101, 4, 500);
    spawn_machine_at(&mut game, "quarantine_rack", -6, -6);
    game.world
        .entity_mut(staff[0])
        .insert(CarryingProgram(program(7)));
    game.dispatch_sortie(&site, &[staff[0], staff[1]]).unwrap();
    assert_handed_back(&mut game, Some(staff[0]));
}

fn fill_the_pack(game: &mut Game) {
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 =
        vec![program(1); crate::tuning::MAX_DOWNED_PROGRAMS];
}

/// The shelter chain's last rung is the floor, not the player's pack and not
/// a log line saying the kill is gone.
#[test]
fn a_destroyed_station_sets_its_program_on_the_floor_when_no_rack_has_room() {
    let (mut game, station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    hold(&mut game, station, 7);

    game.remove_structure(station).unwrap();

    let mut on_floor = kept(&mut game);
    on_floor.retain(|&l| l == 7);
    assert_eq!(on_floor, vec![7]);
    assert_eq!(pack_programs(&game), 0, "research never uses the pack");
}

#[test]
fn a_full_pack_does_not_lose_a_destroyed_stations_program() {
    let (mut game, station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    fill_the_pack(&mut game);
    hold(&mut game, station, 7);

    game.remove_structure(station).unwrap();

    assert!(
        kept(&mut game).contains(&7),
        "the kill was lost with the machine"
    );
}

#[test]
fn a_rig_carrier_with_no_rack_room_sets_its_program_on_the_floor() {
    let (mut game, _station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    fill_the_pack(&mut game);
    let carrier = a_carrier(&mut game);

    game.return_carried_program(carrier);

    assert!(game.world.get::<CarryingProgram>(carrier).is_none());
    assert!(kept(&mut game).contains(&7), "the kill was lost");
    assert_eq!(pack_programs(&game), crate::tuning::MAX_DOWNED_PROGRAMS);
}

#[test]
fn a_hold_ended_with_the_rack_full_and_the_slot_taken_leaves_the_program_on_the_floor() {
    let (mut game, station, rack) = base_with_station_and_rack();
    game.world.get_mut::<Racked>(rack).unwrap().0 = vec![program(1); 8];
    hold(&mut game, station, 8);
    let carrier = a_carrier(&mut game);

    game.pin_subject(carrier, station).unwrap();

    assert!(kept(&mut game).contains(&7));
    assert_eq!(held_at(&game, station).map(|p| p.level), Some(8));
    assert_eq!(pack_programs(&game), 0);
}
