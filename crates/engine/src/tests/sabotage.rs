//! A sulker spoiling output at a machine it resents or a rival works, and
//! the staff who see it.

use super::respite::sulk;
use super::support::*;
use crate::base_ledger::BaseLedger;
use crate::components::{
    Memories, MemorySubject, Position, PostedAt, ProgramId, Stock, Structure, Task, TaskKind,
};
use crate::game::base::sabotage::sabotage_seed;
use crate::tuning::{
    BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES, INTERACTION_PERIOD, SABOTAGE_CHANCE,
    SABOTAGE_SALT,
};
use crate::*;

const FRAGMENT: &str = ids::CORE_FRAGMENT;

/// An established base: eight staff parked far from everything, eight
/// structures, and a Mining Node at (20, 20) holding `qty` fragments.
fn a_base_with_a_full_machine(game: &mut Game, qty: u32) -> (Vec<Entity>, Entity) {
    stand_in_base(game);
    place_home(game);
    for i in 1..BASE_ESTABLISHED_STRUCTURES {
        spawn_structure_at(game, "depot", -20 - i as i32, 20);
    }
    let mut staff: Vec<Entity> = (0..BASE_ESTABLISHED_STAFF)
        .map(|_| spawn_tamed(game, 10, 3))
        .collect();
    staff.sort();
    for (i, &who) in staff.iter().enumerate() {
        place_at(game, who, -40 + 3 * i as i32, -40);
    }
    let machine = spawn_structure_at(game, "mining_node", 20, 20);
    stock_output(game, machine, qty);
    (staff, machine)
}

fn stock_output(game: &mut Game, machine: Entity, qty: u32) {
    let mut stock = Stock::default();
    if qty > 0 {
        stock.output.insert(ItemId::from(FRAGMENT), qty);
    }
    game.world.entity_mut(machine).insert(stock);
}

fn place_at(game: &mut Game, who: Entity, x: i32, y: i32) {
    let mut pos = game.world.get_mut::<Position>(who).unwrap();
    (pos.x, pos.y) = (x, y);
}

fn output(game: &Game, machine: Entity) -> Vec<(ItemId, u32)> {
    game.world
        .get::<Stock>(machine)
        .unwrap()
        .output
        .iter()
        .map(|(i, q)| (i.clone(), *q))
        .collect()
}

fn resent(game: &mut Game, who: Entity, kind: &str) {
    for _ in 0..3 {
        game.remember(who, "jammed_here", MemorySubject::Structure(kind.into()));
    }
    assert!(game.resents_structure(who, &kind.to_string()), "fixture");
}

fn avoid(game: &mut Game, who: Entity, rival: Entity) {
    let about = *game.world.get::<ProgramId>(rival).unwrap();
    game.remember(who, "turned_on_me", MemorySubject::Program(about));
    assert!(game.bond(who, about).avoids(), "fixture");
}

fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

/// The first period tick at which every one of `who` rolls under the chance.
fn rolling_tick(game: &Game, who: &[Entity]) -> u64 {
    let ids: Vec<ProgramId> = who
        .iter()
        .map(|&e| *game.world.get::<ProgramId>(e).unwrap())
        .collect();
    (1..2000)
        .map(|n| n * INTERACTION_PERIOD)
        .find(|&t| {
            ids.iter()
                .all(|&id| crate::derive::unit(sabotage_seed(t, id)) < SABOTAGE_CHANCE)
        })
        .expect("a rolling tick within 2000 periods")
}

/// Runs the pass on every period tick up to a bound; true if `machine`'s
/// output changed. Stops at the first spoil so a test sees exactly one.
fn spoils_within_a_while(game: &mut Game, machine: Entity) -> bool {
    let before = output(game, machine);
    for n in 1..400 {
        set_tick(game, n * INTERACTION_PERIOD);
        game.note_sabotage();
        if output(game, machine) != before {
            return true;
        }
    }
    false
}

fn sabotage_lines(game: &Game) -> Vec<String> {
    game.message_history(500)
        .into_iter()
        .filter(|r| r.text.contains("spoiled a"))
        .map(|r| r.text)
        .collect()
}

fn consumed(game: &Game) -> u32 {
    game.world.resource::<BaseLedger>().lifetime[&ItemId::from(FRAGMENT)].consumed
}

fn saw(game: &Game, who: Entity) -> usize {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.def.as_str() == "saw_sabotage")
                .count()
        })
        .unwrap_or(0)
}

/// A resenting sulker standing beside the machine.
fn beside_and_resenting(game: &mut Game, staff: &[Entity], machine: Entity) -> Entity {
    let who = staff[0];
    place_at(game, who, 21, 20);
    sulk(game, who);
    let kind = game.world.get::<Structure>(machine).unwrap().kind.clone();
    resent(game, who, &kind);
    who
}

#[test]
fn a_resenting_sulker_spoils_one_unit_and_the_staff_in_reach_see_it() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
    let who = beside_and_resenting(&mut game, &staff, machine);
    place_at(&mut game, staff[1], 22, 22);
    place_at(&mut game, staff[2], 23, 20);

    let tick = rolling_tick(&game, &[who]);
    set_tick(&mut game, tick);
    game.note_sabotage();

    assert_eq!(output(&game, machine), vec![(ItemId::from(FRAGMENT), 4)]);
    assert_eq!(consumed(&game), 1);
    let label = game.creature_label(who);
    assert_eq!(
        sabotage_lines(&game),
        vec![format!(
            "{label} spoiled a {} at the {}.",
            game.item_name(&ItemId::from(FRAGMENT)),
            game.structure_name(&"mining_node".to_string()),
        )]
    );
    assert_eq!(saw(&game, staff[1]), 1, "in reach of the machine");
    assert_eq!(saw(&game, staff[2]), 0, "out of reach");
}

#[test]
fn an_amenity_in_the_base_does_not_stop_it() {
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "defrag_bay", 4, 0).expect("a Defrag Bay is buildable");
    beside_and_resenting(&mut game, &staff, machine);
    assert!(spoils_within_a_while(&mut game, machine));
}

#[test]
fn a_machine_a_rival_works_is_a_candidate_though_not_resented() {
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
    let who = staff[0];
    place_at(&mut game, who, 21, 20);
    sulk(&mut game, who);
    avoid(&mut game, who, staff[1]);
    place_at(&mut game, staff[1], 30, 30);
    game.world.entity_mut(staff[1]).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 1,
    });
    assert!(!game.resents_structure(who, &"mining_node".to_string()));
    assert!(spoils_within_a_while(&mut game, machine));
}

/// A resenting sulker beside a full machine, on a game of its own so no
/// case inherits another's clock or its decayed grudges.
fn a_resenting_sulker_beside_a_full_machine(seed: u32) -> (Game, Entity, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
    let who = beside_and_resenting(&mut game, &staff, machine);
    assert!(game.sulks(who), "fixture");
    assert!(
        game.resents_structure(who, &"mining_node".to_string()),
        "fixture"
    );
    (game, who, machine)
}

#[test]
fn a_program_that_is_not_sulking_spoils_nothing() {
    let (mut game, who, machine) = a_resenting_sulker_beside_a_full_machine(9004);
    game.world
        .entity_mut(who)
        .remove::<crate::components::Disgruntled>();
    assert!(!spoils_within_a_while(&mut game, machine));
    // The loop left the clock far ahead; `sulk` remembers, which prunes a
    // decayed grudge, so rewind first.
    set_tick(&mut game, 0);
    sulk(&mut game, who);
    assert!(spoils_within_a_while(&mut game, machine), "control");
}

#[test]
fn a_sulker_with_a_task_spoils_nothing() {
    let (mut game, who, machine) = a_resenting_sulker_beside_a_full_machine(9011);
    game.world.entity_mut(who).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 1,
    });
    assert!(!spoils_within_a_while(&mut game, machine));
    game.world.entity_mut(who).remove::<Task>();
    assert!(spoils_within_a_while(&mut game, machine), "control");
}

#[test]
fn a_machine_out_of_reach_is_left_alone() {
    let (mut game, who, machine) = a_resenting_sulker_beside_a_full_machine(9012);
    place_at(&mut game, who, 23, 20);
    assert!(!spoils_within_a_while(&mut game, machine));
    place_at(&mut game, who, 21, 20);
    assert!(spoils_within_a_while(&mut game, machine), "control");
}

#[test]
fn a_machine_nobody_resents_or_rival_works_is_left_alone() {
    let (mut game, who, machine) = a_resenting_sulker_beside_a_full_machine(9013);
    let other = spawn_structure_at(&mut game, "depot", 20, 21);
    stock_output(&mut game, other, 5);
    stock_output(&mut game, machine, 0);
    assert!(
        !game.resents_structure(who, &"depot".to_string()),
        "fixture"
    );
    assert!(!spoils_within_a_while(&mut game, other));
    assert!(sabotage_lines(&game).is_empty());
    stock_output(&mut game, machine, 5);
    assert!(spoils_within_a_while(&mut game, machine), "control");
}

#[test]
fn an_empty_output_is_never_asked_for_a_unit() {
    let mut game = Game::new(9005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 0);
    beside_and_resenting(&mut game, &staff, machine);
    assert!(!spoils_within_a_while(&mut game, machine));
    assert!(sabotage_lines(&game).is_empty());
}

/// RF1: two sulkers roll together over one unit. The first reaches only the
/// one-unit machine A; the second reaches A and a full machine B. Built once
/// before the loop, the second's candidates would still list the emptied A,
/// and a tick is chosen where it would pick it, so only a per-body rebuild
/// lets it spoil at B.
#[test]
fn the_second_of_two_sulkers_sees_the_stock_the_first_left() {
    let mut game = Game::new(9006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, a) = a_base_with_a_full_machine(&mut game, 1);
    let b = spawn_structure_at(&mut game, "mining_node", 20, 22);
    stock_output(&mut game, b, 5);
    let id_of = |game: &Game, e: Entity| *game.world.get::<ProgramId>(e).unwrap();
    let (first, second) = if id_of(&game, staff[0]) < id_of(&game, staff[1]) {
        (staff[0], staff[1])
    } else {
        (staff[1], staff[0])
    };
    let (first_id, second_id) = (id_of(&game, first), id_of(&game, second));
    let mut by_entity = [a, b];
    by_entity.sort();
    let a_slot = by_entity.iter().position(|&e| e == a).unwrap();
    let tick = (1..4000)
        .map(|n| n * INTERACTION_PERIOD)
        .find(|&t| {
            let rolls = |id| crate::derive::unit(sabotage_seed(t, id)) < SABOTAGE_CHANCE;
            let pick = crate::derive::index(
                crate::derive::fold(sabotage_seed(t, second_id), &[1, SABOTAGE_SALT]),
                2,
            );
            rolls(first_id) && rolls(second_id) && pick == a_slot
        })
        .expect("a tick where the second would pick the emptied machine");
    // Grudges are written at the tick they are asked about, so they have
    // not decayed by the time the pass reads them.
    set_tick(&mut game, tick);
    for (who, x, y) in [(first, 20, 19), (second, 20, 21)] {
        place_at(&mut game, who, x, y);
        sulk(&mut game, who);
        resent(&mut game, who, "mining_node");
    }

    game.note_sabotage();

    assert!(output(&game, a).is_empty(), "no zero-qty entry left");
    assert_eq!(output(&game, b), vec![(ItemId::from(FRAGMENT), 4)]);
    assert_eq!(consumed(&game), 2);
    assert_eq!(sabotage_lines(&game).len(), 2);
}

/// RF2: the saboteur is not its own witness, and only staff witness.
#[test]
fn a_saboteur_is_no_witness_to_itself_and_a_non_staff_body_sees_nothing() {
    let mut game = Game::new(9007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
    let who = beside_and_resenting(&mut game, &staff, machine);
    // A ninth body, so the base is still established with one off the staff.
    let away = spawn_tamed(&mut game, 10, 3);
    place_at(&mut game, away, 21, 21);
    game.world.entity_mut(away).insert(PostedAt((0, 0)));
    assert_ne!(
        game.program_role(away),
        Some(crate::game::party::ProgramRole::Staff),
        "fixture: the body is off the staff"
    );
    let near = staff[2];
    place_at(&mut game, near, 19, 20);

    let tick = rolling_tick(&game, &[who]);
    set_tick(&mut game, tick);
    game.note_sabotage();

    assert_eq!(saw(&game, who), 0, "the saboteur does not witness itself");
    assert_eq!(saw(&game, away), 0, "not staff");
    assert_eq!(saw(&game, near), 1, "control: staff in reach");
}

/// RF3: modded content removed after the fact.
#[test]
fn a_missing_item_or_structure_def_falls_back_to_the_id() {
    let mut game = Game::new(9008, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 0);
    let who = beside_and_resenting(&mut game, &staff, machine);
    game.world.get_mut::<Structure>(machine).unwrap().kind = "ghost_machine".to_string();
    resent(&mut game, who, "ghost_machine");
    let mut stock = Stock::default();
    stock.output.insert(ItemId::from("ghost_item"), 2);
    game.world.entity_mut(machine).insert(stock);

    let tick = rolling_tick(&game, &[who]);
    set_tick(&mut game, tick);
    game.note_sabotage();

    let label = game.creature_label(who);
    assert_eq!(
        sabotage_lines(&game),
        vec![format!(
            "{label} spoiled a ghost_item at the ghost_machine."
        )]
    );
}

/// `rng_unadvanced_by` builds its games bare and applies only its closure to
/// one of them, but staging a base spends the stream (every `spawn_tamed`),
/// so the same proof is made here with both games staged identically and
/// only one asked to run the pass.
#[test]
fn sabotage_draws_nothing_from_the_game_rng() {
    let stage = || {
        let mut game = Game::new(9009, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
        beside_and_resenting(&mut game, &staff, machine);
        (game, staff, machine)
    };
    let (mut touched, staff, machine) = stage();
    let (mut untouched, _, _) = stage();
    let tick = rolling_tick(&touched, &[staff[0]]);
    set_tick(&mut touched, tick);
    touched.note_sabotage();
    assert_eq!(
        output(&touched, machine),
        vec![(ItemId::from(FRAGMENT), 4)],
        "the pass must actually have spoiled, or this proves nothing"
    );
    let after: u64 = touched.world.resource_mut::<GameRng>().0.random();
    let baseline: u64 = untouched.world.resource_mut::<GameRng>().0.random();
    assert_eq!(after, baseline);
}

/// The witness's `remember` is a no-op on a def that does not ship, as every
/// `remember` on a missing def already is. (The saboteur's own grudge needs
/// the catalogue, so a wholly empty one has nothing to resent; this removes
/// only the witness def.)
#[test]
fn a_catalogue_without_the_witness_def_still_spoils_and_logs() {
    let mut game = Game::new(9010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (staff, machine) = a_base_with_a_full_machine(&mut game, 5);
    let who = beside_and_resenting(&mut game, &staff, machine);
    place_at(&mut game, staff[1], 21, 21);

    let dir = std::env::temp_dir().join(format!("feral_sabotage_memories_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(test_assets_dir().join("memories")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ron")
            && path.file_stem().is_some_and(|n| n != "saw_sabotage")
        {
            std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
        }
    }
    let (db, _) = crate::memories::MemoryDb::load_dir(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    game.world.insert_resource(db);

    let tick = rolling_tick(&game, &[who]);
    set_tick(&mut game, tick);
    game.note_sabotage();

    assert_eq!(output(&game, machine), vec![(ItemId::from(FRAGMENT), 4)]);
    assert_eq!(sabotage_lines(&game).len(), 1);
    assert_eq!(saw(&game, staff[1]), 0);
}
