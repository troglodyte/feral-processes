//! Drop pods phase B2 — the Drop Trooper flag, the `call_reinforcements`
//! routine and its teardown
//! (`docs/superpowers/specs/2026-09-27-drop-pods-design.md`).

use super::support::*;
use crate::components::{DropTrooper, StaffRank};
use crate::*;

/// A real save and load, never RON alone —
/// `ron-round-trip-cannot-catch-a-skipped-field`.
#[test]
fn the_drop_trooper_flag_survives_a_save_and_load() {
    let dir = scratch_assets_dir("drop_trooper_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20260927, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let trooper = spawn_tamed(&mut game, 10, 3);
    let other = spawn_tamed(&mut game, 10, 3);
    game.set_drop_trooper(trooper, true).unwrap();
    let trooper_id = game.world.get::<ProgramId>(trooper).unwrap().0;
    let other_id = game.world.get::<ProgramId>(other).unwrap().0;
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&ProgramId, Option<&DropTrooper>)>();
    let flag_of = |id: u32, world: &World, query: &mut QueryState<_>| {
        query
            .iter(world)
            .find(|(p, _): &(&ProgramId, Option<&DropTrooper>)| p.0 == id)
            .map(|(_, t)| t.is_some())
            .expect("the program should survive the round trip")
    };
    assert!(flag_of(trooper_id, &loaded.world, &mut query));
    assert!(!flag_of(other_id, &loaded.world, &mut query));
}

/// Absent in the file means not a trooper — opt-in, the reason the flag is
/// not a `Duty` (whose absent set means every column checked).
#[test]
fn a_creature_record_without_the_field_loads_as_no_trooper() {
    let record: save::CreatureSave = ron::from_str(&{
        let mut game = Game::new(20260928, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let text = ron::to_string(&game.creature_save_for(worker).unwrap()).unwrap();
        assert!(text.contains("drop_trooper:false"), "{text}");
        text.replace("drop_trooper:false,", "")
            .replace(",drop_trooper:false", "")
    })
    .unwrap();
    assert!(!record.drop_trooper);
}

/// `drop_troopers()` is table order — `StaffRank`, never entity id.
#[test]
fn drop_troopers_are_listed_in_staff_rank_order() {
    let mut game = Game::new(20260929, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 10, 3);
    let bystander = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(first).insert(StaffRank(9));
    game.world.entity_mut(second).insert(StaffRank(2));
    game.world.entity_mut(bystander).insert(StaffRank(5));
    game.set_drop_trooper(first, true).unwrap();
    game.set_drop_trooper(second, true).unwrap();
    assert_eq!(game.drop_troopers(), vec![second, first]);

    game.set_drop_trooper(second, false).unwrap();
    assert_eq!(game.drop_troopers(), vec![first]);
}

/// The shipped routine is battle-map only — `tactical_only` is what keeps it
/// out of the group model's pickers and the arena's `SwingOnly` party — and
/// researchable through an ordinary synthesised node with no structure gate
/// (decided 2026-09-27: `NoPodReady` already makes it inert without a pod).
#[test]
fn call_reinforcements_is_tactical_only_and_gets_an_ungated_node() {
    let game = Game::new(20260930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("call_reinforcements")
        .expect("the shipped routine should load")
        .clone();
    assert!(matches!(
        def.effect,
        crate::abilities::AbilityEffect::Reinforce
    ));
    assert!(def.effect.tactical_only());
    let node = crate::routine_tree::node_id(&def.id);
    assert!(
        game.world
            .resource::<crate::research::ResearchDb>()
            .get(&node)
            .is_some(),
        "a synthesised routine node should exist"
    );
}
