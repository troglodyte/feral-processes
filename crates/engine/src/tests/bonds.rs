//! Bands, avoidance, witnessing and departures: what one program's memories
//! of another do to the base.

use super::support::*;
use crate::bonds::Bond;
use crate::components::{Memories, Memory, MemorySubject, ProgramId};
use crate::memories::MemoryId;
use crate::resources::Brawl;
use crate::*;

/// Writes a memory straight into the store, bypassing `remember` — the
/// catalogue and the strike cap are not what these tests are about.
fn implant(game: &mut Game, who: Entity, def: &str, subject: MemorySubject) {
    let now = game.current_tick();
    game.world
        .get_mut::<Memories>(who)
        .expect("an owned program holds a store")
        .0
        .push(Memory {
            def: MemoryId::from(def),
            subject,
            subject_name: None,
            reinforced: now,
            strikes: 1,
        });
}

fn id_of(game: &Game, who: Entity) -> ProgramId {
    *game.world.get::<ProgramId>(who).unwrap()
}

#[test]
fn bond_reads_the_holders_opinion_of_the_subject() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    let about = id_of(&game, other);
    assert_eq!(
        game.bond(holder, about),
        Bond::Neutral,
        "no memories, no bond"
    );

    implant(
        &mut game,
        holder,
        "turned_on_me",
        MemorySubject::Program(about),
    );
    implant(
        &mut game,
        holder,
        "turned_on_me",
        MemorySubject::Program(about),
    );
    let opinion = game.opinion_of(holder, &MemorySubject::Program(about));
    assert_eq!(game.bond(holder, about), crate::bonds::band(opinion));
    assert!(game.bond(holder, about).avoids(), "{opinion}");
}

// ---------------------------------------------------------------------
// Avoidance
// ---------------------------------------------------------------------

/// A worker and one neighbour, the worker on a beat that offers it a real
/// step. The neighbour stands on the far side of that offered tile, so the
/// tile is 8-adjacent to it and the worker is not (yet).
///
/// Standing well inside the starting pocket, `memories.rs`'s
/// `a_base_with_idle_staff` reason: the bond must be the only thing that can
/// refuse the candidate.
fn a_worker_beside_a_neighbour(game: &mut Game) -> (Vec<Entity>, Position) {
    place_home(game);
    let worker = spawn_tamed(game, 10, 3);
    let other = spawn_tamed(game, 10, 3);
    let mut staff = vec![worker, other];
    staff.sort();
    let (worker, other) = (staff[0], staff[1]);
    {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = 3;
        pos.y = 0;
    }
    let here = *game.world.get::<Position>(worker).unwrap();
    let mut tick = game.current_tick();
    let candidate = loop {
        let step = tick / crate::tuning::IDLE_STAFF_STEP_TICKS;
        if let Some(t) = crate::game::base::work_orders::wander_step(here, 0, step) {
            game.world
                .resource_mut::<crate::resources::GameClock>()
                .tick = tick;
            break t;
        }
        tick += 1;
    };
    let mut pos = game.world.get_mut::<Position>(other).unwrap();
    pos.x = candidate.x * 2 - here.x;
    pos.y = candidate.y * 2 - here.y;
    (staff, candidate)
}

fn drift(game: &mut Game, staff: &[Entity]) {
    let amenities = game.amenities();
    let bays = game.repair_bays();
    game.drift_idle_staff_for_test(staff, &amenities, &bays);
}

fn position(game: &Game, who: Entity) -> (i32, i32) {
    let p = game.world.get::<Position>(who).unwrap();
    (p.x, p.y)
}

#[test]
fn a_worker_will_not_step_beside_a_rival() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, candidate) = a_worker_beside_a_neighbour(&mut game);
    let rival = id_of(&game, staff[1]);
    implant(
        &mut game,
        staff[0],
        "turned_on_me",
        MemorySubject::Program(rival),
    );

    let before = position(&game, staff[0]);
    drift(&mut game, &staff);

    assert_eq!(
        position(&game, staff[0]),
        before,
        "declined, so it stays put"
    );
    assert_ne!(position(&game, staff[0]), (candidate.x, candidate.y));
}

#[test]
fn a_worker_steps_beside_a_friend() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, candidate) = a_worker_beside_a_neighbour(&mut game);
    let friend = id_of(&game, staff[1]);
    implant(
        &mut game,
        staff[0],
        "bonded_in_battle",
        MemorySubject::Program(friend),
    );
    implant(
        &mut game,
        staff[0],
        "bonded_in_battle",
        MemorySubject::Program(friend),
    );
    assert!(!game.bond(staff[0], friend).avoids());

    drift(&mut game, &staff);

    assert_eq!(position(&game, staff[0]), (candidate.x, candidate.y));
}

#[test]
fn a_worker_steps_beside_a_stranger() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, candidate) = a_worker_beside_a_neighbour(&mut game);

    drift(&mut game, &staff);

    assert_eq!(
        position(&game, staff[0]),
        (candidate.x, candidate.y),
        "control: with no bond the same fixture takes the same tile"
    );
}

// ---------------------------------------------------------------------
// Departures
// ---------------------------------------------------------------------

const DEPARTURE_DEFS: [&str; 4] = ["lost_in_battle", "let_go", "became_part_of", "rid_of"];

fn implant_strikes(game: &mut Game, who: Entity, def: &str, about: ProgramId, strikes: u32) {
    implant(game, who, def, MemorySubject::Program(about));
    let mut store = game.world.get_mut::<Memories>(who).unwrap();
    store.0.last_mut().unwrap().strikes = strikes;
}

fn befriend(game: &mut Game, holder: Entity, about: ProgramId) {
    implant_strikes(game, holder, "bonded_in_battle", about, 3);
    assert_eq!(game.bond(holder, about), Bond::Friend);
}

fn sour_on(game: &mut Game, holder: Entity, about: ProgramId) {
    implant_strikes(game, holder, "turned_on_me", about, 1);
    assert!(game.bond(holder, about).relieved());
}

/// The departure defs `who` holds about `about`, in store order.
fn departures(game: &Game, who: Entity, about: ProgramId) -> Vec<String> {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.subject == MemorySubject::Program(about))
                .filter(|m| DEPARTURE_DEFS.contains(&m.def.as_str()))
                .map(|m| m.def.as_str().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// One program about to leave and three who have an opinion of it.
struct Cast {
    friend: Entity,
    rival: Entity,
    stranger: Entity,
}

fn cast_around(game: &mut Game, leaving: Entity) -> Cast {
    let id = id_of(game, leaving);
    let friend = spawn_tamed(game, 500, 3);
    let rival = spawn_tamed(game, 500, 3);
    let stranger = spawn_tamed(game, 500, 3);
    befriend(game, friend, id);
    sour_on(game, rival, id);
    Cast {
        friend,
        rival,
        stranger,
    }
}

/// What every door must do: a friend grieves in the door's own def, a rival
/// is relieved, and a holder with no bond is left alone.
fn assert_grief_by(game: &Game, cast: &Cast, about: ProgramId, friend_def: &str) {
    assert_eq!(departures(game, cast.friend, about), vec![friend_def]);
    assert_eq!(departures(game, cast.rival, about), vec!["rid_of"]);
    assert!(departures(game, cast.stranger, about).is_empty());
}

#[test]
fn a_sale_is_let_go() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let market = spawn_market(&mut game);
    let leaving = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);

    game.sell_companion(market, leaving).unwrap();

    assert_grief_by(&game, &cast, id, "let_go");
}

#[test]
fn an_extraction_is_let_go() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let leaving = spawn_tamed(&mut game, 10, 3);
    let prize = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(def, _)| def.id.clone())
        .next()
        .expect("some shipped ability is wild-poolable");
    game.world
        .get_mut::<crate::components::Routines>(leaving)
        .unwrap()
        .0 = vec![prize];
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);

    game.extract_routine(leaving, 0).unwrap();

    assert_grief_by(&game, &cast, id, "let_go");
}

#[test]
fn a_permadeath_death_is_a_fall() {
    let mut game = Game::new(41, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let leaving = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);

    game.bench_or_dissolve(leaving);

    assert_grief_by(&game, &cast, id, "lost_in_battle");
}

/// A Forgiving "death" leaves the program `Downed` on the roster: it is
/// still there to be mourned or not, and nobody grieves a body that is
/// coming back.
#[test]
fn a_forgiving_death_writes_nothing() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let leaving = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);

    game.bench_or_dissolve(leaving);

    assert!(game.world.get_entity(leaving).is_ok());
    for who in [cast.friend, cast.rival, cast.stranger] {
        assert!(departures(&game, who, id).is_empty());
    }
}

#[test]
fn each_fusion_parent_is_grieved_as_part_of_another() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "program_refactoring");
    let a = spawn_tamed(&mut game, 20, 10);
    let b = spawn_tamed(&mut game, 10, 6);
    let (id_a, id_b) = (id_of(&game, a), id_of(&game, b));
    let friend = spawn_tamed(&mut game, 500, 3);
    let rival = spawn_tamed(&mut game, 500, 3);
    let stranger = spawn_tamed(&mut game, 500, 3);
    befriend(&mut game, friend, id_a);
    befriend(&mut game, friend, id_b);
    sour_on(&mut game, rival, id_a);
    // The co-parent's own opinion of the other parent dies with it; nothing
    // is written to a body that is about to be despawned.
    befriend(&mut game, a, id_b);
    let before: Vec<Entity> = game
        .world
        .query_filtered::<Entity, With<crate::components::Tamed>>()
        .iter(&game.world)
        .collect();

    game.fuse_companions(a, b, None).unwrap();

    assert_eq!(departures(&game, friend, id_a), vec!["became_part_of"]);
    assert_eq!(departures(&game, friend, id_b), vec!["became_part_of"]);
    assert_eq!(departures(&game, rival, id_a), vec!["rid_of"]);
    assert!(departures(&game, stranger, id_a).is_empty());
    let child = *game
        .world
        .query_filtered::<Entity, With<crate::components::Tamed>>()
        .iter(&game.world)
        .collect::<Vec<_>>()
        .iter()
        .find(|e| !before.contains(e))
        .expect("fusion leaves a child");
    assert!(
        departures(&game, child, id_a).is_empty() && departures(&game, child, id_b).is_empty(),
        "the child is nobody's co-parent's mourner"
    );
}

fn a_base_with_a_crew(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<crate::components::Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    game
}

#[test]
fn a_finished_build_lets_its_spent_program_go() {
    let mut game = a_base_with_a_crew(1102);
    let leaving = tame_at_zone(&mut game, 1);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);

    game.place_structure("mining_node", 1, 0, Some(leaving))
        .unwrap();
    assert!(
        departures(&game, cast.friend, id).is_empty(),
        "committing is reversible, so committing grieves nothing"
    );
    let (px, py) = game.base_pos().unwrap();
    for _ in 0..600 {
        if game.build_site_at(px + 1, py).is_none() {
            break;
        }
        game.tick();
    }
    assert!(
        game.build_site_at(px + 1, py).is_none(),
        "the crew finished it"
    );

    assert_grief_by(&game, &cast, id, "let_go");
}

#[test]
fn a_committed_then_refunded_program_grieves_nobody() {
    let mut game = a_base_with_a_crew(1103);
    let leaving = tame_at_zone(&mut game, 1);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);

    game.place_structure("mining_node", 1, 0, Some(leaving))
        .unwrap();
    let (px, py) = game.base_pos().unwrap();
    let site = game.build_site_at(px + 1, py).unwrap();
    game.cancel_build_request(site).unwrap();

    for who in [cast.friend, cast.rival, cast.stranger] {
        assert!(departures(&game, who, id).is_empty());
    }
}

#[test]
fn a_program_spent_in_the_study_is_let_go() {
    let mut game = Game::new(4501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let leaving = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, leaving);
    pin_subject_at_pen(&mut game, leaving, node);
    // After the pin: a cast member standing in the pen would refuse it.
    let cast = cast_around(&mut game, leaving);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");

    game.tick();

    assert!(game.world.get_entity(leaving).is_err(), "it was spent");
    assert_grief_by(&game, &cast, id, "let_go");
}

/// The name is stored at the write, because the program it names is gone.
#[test]
fn a_departure_keeps_the_departed_programs_name() {
    let mut game = Game::new(41, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let leaving = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, leaving);
    let cast = cast_around(&mut game, leaving);
    let name = game.creature_short_label(leaving);

    game.bench_or_dissolve(leaving);

    let held = game.world.get::<Memories>(cast.friend).unwrap();
    let grief = held
        .0
        .iter()
        .find(|m| m.def.as_str() == "lost_in_battle" && m.subject == MemorySubject::Program(id))
        .expect("the friend grieves");
    assert_eq!(grief.subject_name.as_deref(), Some(name.as_str()));
}

/// A brawl, a witness and a departure draw nothing from the seeded stream.
#[test]
fn a_brawl_and_a_departure_draw_no_rng() {
    assert!(rng_unadvanced_by(77, |game| {
        stand_in_base(game);
        let staff: Vec<Entity> = (0..4).map(|_| spawn_tamed(game, 10, 3)).collect();
        let id = id_of(game, staff[0]);
        befriend(game, staff[1], id);
        game.close_brawl(&Brawl {
            aggressor: staff[0],
            victim: staff[2],
            ticks_left: 0,
            dealt: 3,
            taken: 3,
        });
        // A witness is the third program; it must have written, or the
        // witness path was never part of what drew nothing.
        assert!(
            game.world
                .get::<Memories>(staff[3])
                .unwrap()
                .0
                .iter()
                .any(|m| m.def == MemoryId::from("saw_turn_on")),
            "the witness path did not run"
        );
        // Not `sell_companion`, which ticks the world and so draws.
        game.dissolve_tamed_program(staff[0], crate::bonds::Departure::LetGo);
        assert_eq!(departures(game, staff[1], id), vec!["let_go"]);
    }));
}

/// Deleting `assets/memories/` is a supported install: a departure writes
/// nothing and does not panic.
#[test]
fn with_no_memory_catalogue_a_departure_writes_nothing() {
    let dir = scratch_assets_dir("bonds_no_memories");
    std::fs::create_dir_all(&*dir).unwrap();
    copy_shipped_assets(&dir, &[]);
    assert!(!dir.join("memories").exists());
    let mut game = Game::new(41, DifficultyMode::Permadeath, &dir).unwrap();
    let leaving = spawn_tamed(&mut game, 10, 3);
    let id = id_of(&game, leaving);
    let holder = spawn_tamed(&mut game, 10, 3);
    implant_strikes(&mut game, holder, "bonded_in_battle", id, 3);
    let held = game.world.get::<Memories>(holder).unwrap().0.len();

    game.bench_or_dissolve(leaving);

    assert_eq!(game.world.get::<Memories>(holder).unwrap().0.len(), held);
}

// ---------------------------------------------------------------------
// Known for and the social view
// ---------------------------------------------------------------------

#[test]
fn known_for_reads_what_others_think_not_what_the_program_thinks() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    let (sid, oid) = (id_of(&game, subject), id_of(&game, other));

    // The subject's own grudge against the other is not its reputation.
    implant(
        &mut game,
        subject,
        "turned_on_me",
        MemorySubject::Program(oid),
    );
    assert!(game.known_for(subject).is_empty());
    assert_eq!(game.known_for(other), vec!["a brawler".to_string()]);

    implant(&mut game, other, "idled_with", MemorySubject::Program(sid));
    assert_eq!(game.known_for(subject), vec!["good company".to_string()]);
}

#[test]
fn known_for_sums_across_every_holder_not_the_first_of_each_def() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    let sid = id_of(&game, subject);
    for _ in 0..3 {
        let witness = spawn_tamed(&mut game, 10, 3);
        implant(
            &mut game,
            witness,
            "saw_turn_on",
            MemorySubject::Program(sid),
        );
    }
    let friend = spawn_tamed(&mut game, 10, 3);
    implant(&mut game, friend, "idled_with", MemorySubject::Program(sid));
    // Three witnesses at -3 outweigh one friend at +4.
    assert_eq!(game.known_for(subject)[0], "a brawler");
}

#[test]
fn two_defs_with_one_phrase_are_one_entry() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    let sid = id_of(&game, subject);
    implant(
        &mut game,
        other,
        "turned_on_me",
        MemorySubject::Program(sid),
    );
    implant(&mut game, other, "saw_turn_on", MemorySubject::Program(sid));
    assert_eq!(game.known_for(subject), vec!["a brawler".to_string()]);
}

#[test]
fn known_for_is_capped_at_two_heaviest_first() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    let sid = id_of(&game, subject);
    implant(&mut game, other, "idled_with", MemorySubject::Program(sid));
    implant(
        &mut game,
        other,
        "bonded_in_battle",
        MemorySubject::Program(sid),
    );
    implant(
        &mut game,
        other,
        "turned_on_me",
        MemorySubject::Program(sid),
    );
    let known = game.known_for(subject);
    assert_eq!(known.len(), 2, "{known:?}");
    assert_eq!(known[0], "a brawler", "turned_on_me is the heaviest");
}

#[test]
fn nothing_said_about_a_program_is_nothing_known() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let subject = spawn_tamed(&mut game, 10, 3);
    let _other = spawn_tamed(&mut game, 10, 3);
    assert!(game.known_for(subject).is_empty());
}

#[test]
fn social_is_only_for_an_owned_program() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let owned = spawn_tamed(&mut game, 10, 3);
    let player = game.player_entity();
    assert!(game.social(owned).is_some());
    assert!(game.social(player).is_none());
    let wild = spawn_wild_on_player_tile(&mut game);
    assert!(game.social(wild).is_none());
}

#[test]
fn social_rows_are_strongest_opinion_first() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let holder = spawn_tamed(&mut game, 10, 3);
    let weak = spawn_tamed(&mut game, 10, 3);
    let strong = spawn_tamed(&mut game, 10, 3);
    let (wid, sid) = (id_of(&game, weak), id_of(&game, strong));
    implant(&mut game, holder, "idled_with", MemorySubject::Program(wid));
    implant(
        &mut game,
        holder,
        "turned_on_me",
        MemorySubject::Program(sid),
    );
    implant(
        &mut game,
        holder,
        "turned_on_me",
        MemorySubject::Program(sid),
    );

    let view = game.social(holder).unwrap();
    assert_eq!(view.relationships.len(), 2);
    assert_eq!(
        view.relationships[0].name,
        game.creature_short_label(strong)
    );
    assert_eq!(
        view.relationships[0].bond,
        crate::bonds::band(view.relationships[0].opinion)
    );
    assert!(
        view.relationships[0].opinion.abs() > view.relationships[1].opinion.abs(),
        "{:?}",
        view.relationships
    );
    assert!(view.relationships.iter().all(|r| !r.gone));
}

#[test]
fn a_departed_friend_is_gone_and_keeps_its_name() {
    let mut game = Game::new(41, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let leaving = spawn_tamed(&mut game, 10, 3);
    let cast = cast_around(&mut game, leaving);
    let name = game.creature_short_label(leaving);

    game.bench_or_dissolve(leaving);

    let view = game.social(cast.friend).unwrap();
    let row = view
        .relationships
        .iter()
        .find(|r| r.name == name)
        .expect("the friend still has a row for it");
    assert!(row.gone);
}
