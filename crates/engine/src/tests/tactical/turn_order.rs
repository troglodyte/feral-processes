use super::*;

/// A hostile's turn hands the turn on **once**.
///
/// `tactical_attack` and `tactical_use_routine` both end the turn
/// themselves — the action ends the turn — so an AI turn that ends it again
/// at the tail spends two rungs of the order and skips whoever came next.
/// With one hostile and one party body that is a fight the player never
/// gets a turn in: the hostile takes every turn, for ever.
#[test]
fn a_hostiles_turn_costs_exactly_one_rung_of_the_order() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let hostile = pack[0];
    assert!(wait_for_turn(&mut game, hostile), "the hostile gets a turn");

    let order: Vec<Entity> = game
        .world
        .resource::<TacticalBattle>()
        .initiative()
        .to_vec();
    let at = order
        .iter()
        .position(|&e| e == hostile)
        .expect("the hostile is in the order");
    let next = order[(at + 1) % order.len()];

    assert!(game.tactical_ai_turn(), "the AI drove the hostile's turn");

    assert_eq!(
        game.tactical_actor(),
        Some(next),
        "the hostile's turn skipped the body that came after it"
    );
}

/// The player is handed control after a lone hostile has swung, which is
/// the shape every wandering encounter has.
#[test]
fn a_one_on_one_fight_hands_control_back_to_the_player() {
    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();

    for _ in 0..8 {
        if game.tactical_awaits_input() {
            assert_eq!(game.tactical_actor(), Some(player));
            return;
        }
        assert!(game.tactical_ai_turn(), "the AI has a turn to drive");
    }
    panic!("the wild side never handed the turn back");
}

/// A walkable, empty cell at least `clear` away from every body on the
/// board — somewhere a blast can be aimed with nobody but its invoker in it.
pub(super) fn lonely_cell(game: &Game, clear: u32) -> (i32, i32) {
    let battle = game.world.resource::<TacticalBattle>();
    let bodies: Vec<(i32, i32)> = battle.bodies().map(|(_, cell)| cell).collect();
    battle
        .board
        .cells()
        .map(|(cell, _)| cell)
        .find(|&(x, y)| {
            battle.board.walkable(x, y)
                && battle.occupant((x, y)).is_none()
                && bodies
                    .iter()
                    .all(|&b| crate::tactical::reach::distance(b, (x, y)) > clear)
        })
        .expect("no cell on the board is clear of every body")
}

/// The order as it stands, and who follows `who` around it.
fn after(game: &mut Game, who: Entity) -> Entity {
    let order: Vec<Entity> = game
        .tactical_view()
        .expect("no fight is open")
        .order
        .iter()
        .map(|row| row.entity)
        .collect();
    let idx = order
        .iter()
        .position(|&e| e == who)
        .expect("that body is not in the order");
    order[(idx + 1) % order.len()]
}

/// A body killed by its own action has already left the order, and
/// `TacticalBattle::remove` handed the turn on when it went. Ending the turn
/// again on top of that skips whoever was standing behind it — a companion
/// who fumbles fatally costs the player their turn.
#[test]
fn a_body_that_kills_itself_with_its_own_action_hands_the_turn_on_once() {
    let mut game = game();
    // A companion rather than a hostile: a hostile holds no `PowerReserve`,
    // so the player's door refuses it every priced routine there is.
    let actor = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, actor);
    tactical_fight(&mut game, 1, 400);
    assert!(wait_for_turn(&mut game, actor));
    only_routine(&mut game, actor, "cascade_overflow");

    // Alone in its own blast: the roll is forced for one recipient, so the
    // one that matters has to be the only one there is.
    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(actor, alone)
    );
    game.world.get_mut::<Stats>(actor).unwrap().hp = 1;
    let next = after(&mut game, actor);
    crate::tests::support::force_the_next_attack_to_land(&mut game);

    assert!(
        game.tactical_use_routine(0, alone),
        "the blast was refused before it could land"
    );
    assert!(
        !game.creature_alive(actor),
        "the blast spared its own invoker — this fixture needs a lethal roll"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(next),
        "the turn was handed on twice: the body behind the one that died never acted"
    );
}

/// Nothing carries a `Squad` yet, so every body — the player's own included —
/// gets exactly one action. Task 4's whole reason `actions_per_turn` takes an
/// entity at all.
#[test]
fn every_body_gets_one_action_without_a_squad() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert_eq!(game.actions_per_turn(player), 1);
    assert_eq!(game.actions_per_turn(pack[0]), 1);
}

/// `hand_on_turn` hands the turn on only once no actions are left, and the
/// existing "still the one acting" guard stays alongside the new gate — a
/// body cannot be exercised this way for real yet, since no body carries
/// more than one action, so `actions_left` is driven directly.
#[test]
fn a_body_with_two_actions_keeps_its_turn_after_the_first() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    assert!(game.tactical_defend(), "the first brace was refused");
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a second action still owed was handed to the next body early"
    );
    assert_eq!(
        game.world.resource::<TacticalBattle>().actions_left(),
        1,
        "the first of two actions did not spend itself"
    );

    assert!(game.tactical_defend(), "the second brace was refused");
    assert_eq!(
        game.tactical_actor(),
        Some(pack[0]),
        "the last of two actions did not hand the turn on"
    );
}

/// `tactical_attack` reads `actions_left == 0` where it used to read
/// `acted` — a body with nothing left to spend may not swing.
#[test]
fn a_body_with_no_actions_left_may_not_attack() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    // Beside the player, not wherever deployment put it — the refusal has
    // to be `actions_left`'s, not a range or sight refusal that would pass
    // whether or not the gate this test names is even there.
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    let beside = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[0], beside)
    );
    let before = hp_of(&game, pack[0]);
    game.world.resource_mut::<TacticalBattle>().spend_action();

    assert!(!game.tactical_attack(pack[0]), "a spent body swung anyway");
    assert_eq!(
        hp_of(&game, pack[0]),
        before,
        "the refusal still landed a blow"
    );
}

/// The movement allowance is the *turn's*, not one action's — a body walks
/// once for the whole turn however many actions it buys, so what it has
/// already spent must carry from one action into the next and reset only
/// once the turn actually ends.
#[test]
fn two_actions_share_one_turns_movement_allowance() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);
    game.world.resource_mut::<TacticalBattle>().spend(2);

    assert!(game.tactical_defend(), "the first brace was refused");
    assert_eq!(
        game.world.resource::<TacticalBattle>().spent(),
        2,
        "movement already spent this turn must carry into its second action"
    );

    assert!(game.tactical_defend(), "the second brace was refused");
    assert_eq!(
        game.world.resource::<TacticalBattle>().spent(),
        0,
        "the next body's own turn must start with nothing spent"
    );
}

/// After landing an action with another still owed, `run_tactical_beat`
/// clears the walk rather than ending the turn, so the next action plans a
/// fresh one from wherever this one left the body standing.
#[test]
fn a_second_action_plans_a_fresh_walk_rather_than_ending_the_turn() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild));
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    // Closing the deployment gap can cost a walk of its own beats before the
    // first action lands — every one of them must report `Stepped`, since
    // none of them may end the turn early.
    let mut beats = 0;
    loop {
        assert_eq!(
            game.tactical_ai_beat(),
            AiBeat::Stepped,
            "neither a walk step nor a landed first action may end the turn"
        );
        beats += 1;
        assert!(beats < 20, "the hostile never closed on the player to act");
        if game.world.resource::<TacticalBattle>().actions_left() < 2 {
            break;
        }
    }
    assert_eq!(
        game.world.resource::<TacticalBattle>().actions_left(),
        1,
        "the first action was not spent"
    );
    assert!(
        !game.world.resource::<TacticalBattle>().walk_planned(),
        "the walk was not cleared for the second action to plan its own"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(wild),
        "the body with an action left lost its turn early"
    );

    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Acted,
        "the second of two actions must end the turn"
    );
    assert_eq!(
        game.world.resource::<TacticalBattle>().actions_left(),
        1,
        "the next body's own fresh budget must not read as the first's leftover"
    );
}

/// A routine's cooldown arms the moment it runs, not when the turn ends —
/// otherwise a body with two actions could run the same one-shot routine
/// twice in the same turn.
#[test]
fn a_routines_cooldown_arms_the_action_it_runs_in_not_the_turn_it_ends() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    only_routine(&mut game, wild, "acid_wash");
    let def = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("acid_wash")
        .cloned()
        .expect("acid_wash ships");
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    assert_eq!(
        log_texts(&game)
            .iter()
            .filter(|l| l.contains(&def.name))
            .count(),
        1,
        "the routine ran twice in one turn — its cooldown did not arm until \
         the turn ended"
    );
}

/// `actions_left` must not resurrect a turn `TacticalBattle::remove` already
/// handed on: a body with two actions that kills itself with the first still
/// does not get a second — the body behind it acts next, exactly as it does
/// with one action.
///
/// **The body dies on the *last* rung, and the round's upkeep is the
/// assertion.** Who acts next cannot tell the identity check from its
/// absence: `remove` has already handed the turn on either way, so the
/// cursor names the same body whichever branch `hand_on_turn` takes. What
/// the identity check is *for* is that the branch it guards reads the dead
/// body's turn off the next body's fresh `actions_left` and returns early —
/// silently skipping the round upkeep, which is exactly the case
/// `hand_on_turn`'s own doc names. On the last rung `TacticalBattle::remove`
/// wraps the round itself, so the upkeep is owed and its world tick is what
/// makes the skip visible.
#[test]
fn a_body_with_two_actions_that_kills_itself_on_the_first_gets_no_second() {
    let mut game = game();
    // A companion rather than a hostile: a hostile holds no `PowerReserve`,
    // so the player's door refuses it every priced routine there is.
    let actor = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, actor);
    tactical_fight(&mut game, 1, 400);

    // Seated last, so its death wraps the round inside `remove`.
    let mut order: Vec<Entity> = game
        .world
        .resource::<TacticalBattle>()
        .initiative()
        .to_vec();
    order.retain(|&e| e != actor);
    order.push(actor);
    let next = order[0];
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(order);

    assert!(wait_for_turn(&mut game, actor));
    assert_eq!(
        game.tactical_actor(),
        Some(actor),
        "fixture: the dying body must be the one acting"
    );
    only_routine(&mut game, actor, "cascade_overflow");
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    // Alone in its own blast: the roll is forced for one recipient, so the
    // one that matters has to be the only one there is.
    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(actor, alone)
    );
    game.world.get_mut::<Stats>(actor).unwrap().hp = 1;
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    let round_before = game.world.resource::<TacticalBattle>().round;
    let tick_before = game.world.resource::<crate::resources::GameClock>().tick;

    assert!(
        game.tactical_use_routine(0, alone),
        "the blast was refused before it could land"
    );
    assert!(
        !game.creature_alive(actor),
        "the blast spared its own invoker — this fixture needs a lethal roll"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(next),
        "the dead body's second action resurrected its turn"
    );
    let battle = game.world.resource::<TacticalBattle>();
    assert_eq!(
        battle.round,
        round_before + 1,
        "fixture: a death on the last rung must wrap the round"
    );
    assert_eq!(
        game.world.resource::<crate::resources::GameClock>().tick,
        tick_before + 1,
        "the round's upkeep was skipped — the dead body's hand-on read the \
         next body's fresh action budget as its own open turn"
    );
}

/// A round on a battle map spends the upkeep an abstract round spends —
/// cooldowns and status effects tick, and the world clock moves. Without it
/// every routine is once per fight and a fight costs the world no time at
/// all.
#[test]
fn a_round_on_a_battle_map_cools_a_routine_and_spends_a_world_tick() {
    let mut game = game();
    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    only_routine(&mut game, player, "cascade_overflow");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    assert!(
        game.tactical_use_routine(0, at),
        "the blast was refused before it could be charged"
    );
    let armed = cooldown_of(&game, player, "cascade_overflow");
    assert!(armed > 0, "a routine with a cooldown was not put on one");

    let round = game.tactical_view().expect("the fight closed").round;
    let tick = game.current_tick();
    for _ in 0..64 {
        if game.tactical_actor().is_none() {
            break;
        }
        if game.tactical_view().is_some_and(|v| v.round > round) {
            break;
        }
        game.tactical_end_turn();
    }

    assert!(
        cooldown_of(&game, player, "cascade_overflow") < armed,
        "a full round passed and the routine never cooled"
    );
    assert!(
        game.current_tick() > tick,
        "a round of a battle map cost the world no time"
    );
}

fn cooldown_of(game: &Game, body: Entity, routine: &str) -> u32 {
    game.world
        .get::<crate::components::AbilityCooldowns>(body)
        .and_then(|c| c.0.get(routine).copied())
        .unwrap_or(0)
}

/// A defeat is absorbed inside the fight that lands it, exactly as the
/// abstract model's trailing tick absorbs one. Left to the next idle tick,
/// the player walks off a battle map at zero Integrity and reboots a moment
/// later on the map.
#[test]
fn a_player_dropped_on_a_battle_map_does_not_walk_away_dead() {
    let mut game = game();
    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    only_routine(&mut game, player, "cascade_overflow");
    assert!(wait_for_turn(&mut game, player));

    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, alone)
    );
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_use_routine(0, alone));

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the player went down and the fight stayed open"
    );
    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 0,
        "the player left the battle map dead: the reboot was deferred"
    );
}

/// A capture is aimed at something hostile. Aimed at one of your own it used
/// to spend the catalyst and, on a good roll, hand the companion back
/// through `roster_parts` — a fresh `ProgramId`, level one, no memories.
#[test]
fn a_capture_refuses_a_body_of_your_own() {
    let mut game = game();
    let friend = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, friend);
    let level_before = game.world.get::<Experience>(friend).map(|e| e.level);
    let id_before = game
        .world
        .get::<crate::components::ProgramId>(friend)
        .map(|p| p.0);

    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    only_routine(&mut game, player, "decompile");
    game.world
        .get_mut::<crate::components::Decompiler>(player)
        .unwrap()
        .skill = 50;
    crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);
    assert!(wait_for_turn(&mut game, player));

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(friend)
        .expect("the companion was not seated");
    let beside = beside(&game, at).expect("no cell beside the companion");
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, beside)
    );
    let catalysts = catalysts_held(&game);

    assert!(
        !game.tactical_use_routine(0, at),
        "a capture aimed at your own companion was accepted"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "the refused capture spent the turn"
    );
    assert_eq!(
        catalysts_held(&game),
        catalysts,
        "the refused capture spent a catalyst"
    );
    assert_eq!(
        game.world.get::<Experience>(friend).map(|e| e.level),
        level_before,
        "the companion was handed back through roster_parts"
    );
    assert_eq!(
        game.world
            .get::<crate::components::ProgramId>(friend)
            .map(|p| p.0),
        id_before,
        "the companion was minted a new identity"
    );
}

/// The results page draws from `BattleTimeline::closing`, which only the
/// group model ever filled — so a tactical fight ended on a blank screen
/// with the win, the salvage and the XP written nowhere the player looks.
#[test]
fn a_finished_tactical_fight_leaves_a_results_roster() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);

    let view = game
        .battle_result_view()
        .expect("a finished tactical fight left no results roster: the screen draws nothing");
    let named: Vec<String> = view.party.iter().map(|slot| slot.name.clone()).collect();
    assert!(
        named.iter().any(|name| name == "You"),
        "the results roster names no player: {named:?}"
    );
    assert!(
        view.groups.is_empty(),
        "the board was cleared and the results still list a hostile"
    );
}

/// The results popup is drawn over the board the fight ended on, and
/// `finish_fight` removes `TacticalBattle` — so without a copy taken there,
/// the map pane has nothing to draw under it but the surface.
#[test]
fn a_finished_tactical_fight_leaves_its_board_frozen() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);

    let board = game
        .tactical_result_view()
        .expect("a finished tactical fight left no board to draw under its results");
    assert!(
        board
            .bodies
            .iter()
            .any(|body| body.entity == game.player_entity()),
        "the frozen board lost the player"
    );
    assert!(
        board.bodies.iter().all(|body| !body.is_hostile),
        "a won fight's board still stands a hostile on it"
    );
    // Nothing is acting on a finished board: a turn arrow or a reach wash
    // there offers a move there is no fight left to spend.
    assert_eq!(board.active, None, "the frozen board still names an actor");
    assert!(
        board.reachable.is_empty(),
        "the frozen board still washes reach"
    );
    assert!(!board.player_turn, "the frozen board still awaits input");
}

/// The popup lists what the fight came to, and nothing of the blow-by-blow:
/// the same lines `prune_battle_narration` keeps, read before it runs.
#[test]
fn a_finished_tactical_fight_reports_only_its_outcomes() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);

    let outcomes = game.battle_outcomes();
    assert!(
        outcomes.iter().any(|entry| entry.text == "You won!"),
        "the outcome lines never say the fight was won: {outcomes:?}"
    );
    assert!(
        outcomes
            .iter()
            .all(|entry| entry.kind.survives_battle_prune()),
        "a narration line reached the outcomes: {outcomes:?}"
    );
}

/// A group fight after a tactical one must not inherit its board.
#[test]
fn a_group_fight_leaves_no_frozen_board() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.end_battle(player, None);
    assert!(
        game.tactical_result_view().is_none(),
        "a group fight's results drew the last tactical board"
    );
}
