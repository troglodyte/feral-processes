//! Drop pods phase B2 — the Drop Trooper flag, the `call_reinforcements`
//! routine and its teardown
//! (`docs/superpowers/specs/2026-09-27-drop-pods-design.md`).

use super::support::*;
use super::tactical::{only_routine, tactical_fight, wait_for_turn};
use crate::components::{DropPod, DropTrooper, Reinforcement, StaffRank};
use crate::game::combat::RoutineRefusal;
use crate::resources::Party;
use crate::tactical::TacticalBattle;
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

// ---------------------------------------------------------------------------
// The call — `Game::call_reinforcement` and its four refusals.
// ---------------------------------------------------------------------------

/// A terminal standing on base-space `(x, y)`, raised straight through
/// `spawn_structure` — so charged, by that door's own rule.
fn terminal_at(game: &mut Game, x: i32, y: i32) -> Entity {
    let def = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get("drop_pod_terminal")
        .cloned()
        .expect("the terminal ships");
    game.spawn_structure(&def, x, y, None)
}

/// A base staff program marked as a Drop Trooper, standing on base-space
/// `(x, y)` at table rank `rank`.
fn trooper_at(game: &mut Game, x: i32, y: i32, rank: u32) -> Entity {
    let body = spawn_tamed(game, 60, 3);
    game.world
        .entity_mut(body)
        .insert((Position { x, y }, StaffRank(rank)));
    game.set_drop_trooper(body, true).unwrap();
    body
}

/// A battle map with one hostile, the player holding `call_reinforcements`
/// alone and it their turn.
fn fight(game: &mut Game) -> Entity {
    tactical_fight(game, 1, 40);
    let player = game.player_entity();
    only_routine(game, player, "call_reinforcements");
    assert!(wait_for_turn(game, player));
    player
}

fn call(game: &mut Game) -> bool {
    let player = game.player_entity();
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player is seated");
    game.tactical_use_routine(0, at)
}

fn routine(game: &Game) -> crate::abilities::AbilityDef {
    game.world
        .resource::<crate::abilities::AbilityDb>()
        .get("call_reinforcements")
        .cloned()
        .unwrap()
}

fn charged(game: &Game, pod: Entity) -> bool {
    game.world.get::<DropPod>(pod).unwrap().charged
}

fn power(game: &Game) -> f32 {
    game.world
        .get::<PowerReserve>(game.player_entity())
        .unwrap()
        .get()
}

/// What a refusal must leave untouched — asserted per refusal, since one
/// test over one path passes against every path that never spends anyway.
fn refused_spending_nothing(game: &mut Game, pods: &[Entity], expected: RoutineRefusal) {
    let player = game.player_entity();
    let def = routine(game);
    assert_eq!(game.ability_unavailable(player, &def), Some(expected));
    let before_power = power(game);
    let before_charge: Vec<bool> = pods.iter().map(|&p| charged(game, p)).collect();
    let before_party = game.world.resource::<Party>().0.clone();
    assert!(!call(game), "a refused call ran anyway");
    assert_eq!(power(game), before_power, "Power was spent");
    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| c.0.is_empty()),
        "the cooldown was armed"
    );
    assert_eq!(
        pods.iter().map(|&p| charged(game, p)).collect::<Vec<_>>(),
        before_charge,
        "a pod was spent"
    );
    assert_eq!(game.world.resource::<Party>().0, before_party);
    assert_eq!(game.tactical_actor(), Some(player), "the turn was spent");
}

#[test]
fn no_charged_pod_refuses_the_call() {
    let mut game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pod = terminal_at(&mut game, 10, 10);
    game.world.get_mut::<DropPod>(pod).unwrap().charged = false;
    trooper_at(&mut game, 11, 10, 0);
    fight(&mut game);
    refused_spending_nothing(&mut game, &[pod], RoutineRefusal::NoPodReady);
}

#[test]
fn no_trooper_on_shift_refuses_the_call() {
    let mut game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pod = terminal_at(&mut game, 10, 10);
    let trooper = trooper_at(&mut game, 11, 10, 0);
    game.world
        .entity_mut(trooper)
        .insert(crate::components::Downed);
    // A marked program that is not base staff is no candidate either.
    let away = trooper_at(&mut game, 12, 10, 1);
    game.world.resource_mut::<Party>().0.push(away);
    fight(&mut game);
    refused_spending_nothing(&mut game, &[pod], RoutineRefusal::NoTrooperAvailable);
}

#[test]
fn a_siege_refuses_the_call() {
    let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pod = terminal_at(&mut game, 10, 10);
    trooper_at(&mut game, 11, 10, 0);
    fight(&mut game);
    game.world.resource_mut::<TacticalBattle>().siege_pack = 1;
    refused_spending_nothing(&mut game, &[pod], RoutineRefusal::InSiege);
}

#[test]
fn a_board_with_no_free_cell_refuses_the_call() {
    let mut game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pod = terminal_at(&mut game, 10, 10);
    trooper_at(&mut game, 11, 10, 0);
    fight(&mut game);
    // Fill the board: `board_has_room` searches every cell breadth-first
    // (Summon's own landing), so "no room" is a board with no free cell.
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        let side = battle.board.side;
        for x in 0..side {
            for y in 0..side {
                if battle.occupant((x, y)).is_none() {
                    battle
                        .board
                        .set(x, y, crate::tactical::map::BattleCell::Blocked);
                }
            }
        }
    }
    refused_spending_nothing(&mut game, &[pod], RoutineRefusal::NoLandingCell);
}

/// The trooper that drops is the one nearest the pod that fires, and the
/// table's order is only a tiebreak. Ranks run *against* distance here, so
/// a pairing that read rank first picks the wrong body.
#[test]
fn the_trooper_nearest_a_charged_pod_drops() {
    let mut game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pod = terminal_at(&mut game, 10, 10);
    let far = trooper_at(&mut game, 20, 10, 0);
    let near = trooper_at(&mut game, 12, 11, 5);
    let player = fight(&mut game);

    assert!(call(&mut game));
    assert!(game.world.resource::<Party>().0.contains(&near));
    assert!(!game.world.resource::<Party>().0.contains(&far));
    assert!(game.world.get::<Reinforcement>(near).is_some());
    assert!(!charged(&game, pod), "the pod that fired is spent");
    let battle = game.world.resource::<TacticalBattle>();
    let (me, it) = (
        battle.cell_of(player).unwrap(),
        battle.cell_of(near).expect("the trooper is on the board"),
    );
    assert!((me.0 - it.0).abs() <= 1 && (me.1 - it.1).abs() <= 1);
    assert!(battle.initiative().contains(&near));
}

/// Two pods, one trooper: the pod nearest it is the one spent.
#[test]
fn the_pod_nearest_the_trooper_is_the_one_spent() {
    let mut game = Game::new(4406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let far_pod = terminal_at(&mut game, 0, 20);
    let near_pod = terminal_at(&mut game, 14, 10);
    trooper_at(&mut game, 11, 10, 0);
    fight(&mut game);

    assert!(call(&mut game));
    assert!(!charged(&game, near_pod));
    assert!(charged(&game, far_pod));
}

/// Equal distance goes to the table's order, never to entity id.
#[test]
fn a_tie_goes_to_the_staff_table_order() {
    let mut game = Game::new(4407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    terminal_at(&mut game, 10, 10);
    let spawned_first = trooper_at(&mut game, 13, 10, 8);
    let ranked_first = trooper_at(&mut game, 7, 10, 2);
    fight(&mut game);

    assert!(call(&mut game));
    let party = &game.world.resource::<Party>().0;
    assert!(party.contains(&ranked_first));
    assert!(!party.contains(&spawned_first));
}

/// A reinforcement may exceed the party cap — the point of calling one.
#[test]
fn a_trooper_joins_a_full_party() {
    let mut game = Game::new(4408, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    terminal_at(&mut game, 10, 10);
    let trooper = trooper_at(&mut game, 11, 10, 99);
    for _ in 0..MAX_PARTY_SIZE {
        let member = spawn_tamed(&mut game, 60, 3);
        game.world.resource_mut::<Party>().0.push(member);
    }
    fight(&mut game);

    assert!(call(&mut game));
    let party = &game.world.resource::<Party>().0;
    assert_eq!(party.len(), MAX_PARTY_SIZE + 1);
    assert!(party.contains(&trooper));
}

/// It lands reorienting: the first turn it would have had is passed, and
/// it acts on the one after. Compared by round, since a count of turns is
/// conserved by a skip and passes against the bug.
#[test]
fn a_trooper_loses_exactly_its_first_turn() {
    let mut game = Game::new(4409, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    terminal_at(&mut game, 10, 10);
    let trooper = trooper_at(&mut game, 11, 10, 0);
    fight(&mut game);
    let round = game.world.resource::<TacticalBattle>().round;

    assert!(call(&mut game));
    while game.world.resource::<TacticalBattle>().round == round {
        assert_ne!(
            game.tactical_actor(),
            Some(trooper),
            "the trooper acted in the round it landed"
        );
        game.tactical_end_turn();
    }
    let next = game.world.resource::<TacticalBattle>().round;
    while game.world.resource::<TacticalBattle>().round == next
        && game.tactical_actor() != Some(trooper)
    {
        game.tactical_end_turn();
    }
    assert_eq!(
        game.tactical_actor(),
        Some(trooper),
        "the trooper never got a turn in the round after it landed"
    );
    assert_eq!(game.world.resource::<TacticalBattle>().round, next);
}

/// Calling a posted trooper frees its post: the scheduler treats a
/// non-staff body still holding a `Task` as an outsider and keeps its post
/// covered, and does not run at all while a fight is open.
#[test]
fn a_called_trooper_is_unposted() {
    let mut game = Game::new(4410, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pod = terminal_at(&mut game, 10, 10);
    let trooper = trooper_at(&mut game, 11, 10, 0);
    game.world.entity_mut(trooper).insert(Task {
        target: pod,
        kind: TaskKind::Guard,
        progress: 0,
        required: 1,
    });
    fight(&mut game);

    assert!(call(&mut game));
    assert!(game.world.get::<Task>(trooper).is_none());
}
