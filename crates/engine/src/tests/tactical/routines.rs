use super::*;

/// Full friendly fire, and the assertion is a *heal*: a patch centred on the
/// player mends the hostile standing beside them, because `recipients` never
/// reads `Hostile`. A blast would prove the same thing through a roll that
/// can miss.
#[test]
fn a_routine_lands_on_everyone_inside_its_shape_whichever_side_they_are_on() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));

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
    game.world.get_mut::<Stats>(pack[0]).unwrap().hp = 10;
    let before = hp_of(&game, pack[0]);

    assert!(
        game.tactical_use_routine(0, at),
        "a patch aimed at the caster's own cell was refused"
    );
    assert!(
        hp_of(&game, pack[0]) > before,
        "the patch spared the hostile standing inside it — recipients read a side"
    );
}

/// The action ends the turn, whatever the action was.
#[test]
fn running_a_routine_hands_the_turn_on() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();

    assert!(game.tactical_use_routine(0, at));
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "the routine ran and the player kept the turn"
    );
}

/// Every refusal lands before anything is spent — the Power, the cooldown
/// and the turn alike.
#[test]
fn a_routine_aimed_out_of_range_is_refused_before_it_is_charged() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    // `WholeParty` derives a range of 0..0: it is aimed at the invoker's own
    // cell and nowhere else.
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();
    let beside = free_neighbour(&game, at);
    let power = game
        .world
        .get::<crate::components::PowerReserve>(player)
        .map(|r| r.get());

    assert!(
        !game.tactical_use_routine(0, beside),
        "an out-of-range aim ran"
    );
    assert_eq!(
        game.world
            .get::<crate::components::PowerReserve>(player)
            .map(|r| r.get()),
        power,
        "a refused routine was charged anyway"
    );
    assert!(
        game.world
            .get::<crate::components::AbilityCooldowns>(player)
            .is_none_or(|c| c.0.is_empty()),
        "a refused routine armed its cooldown"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a refused routine cost the turn"
    );
}

/// A field-only routine has nothing to resolve against a body on a battle
/// map, and a passive is never chosen at all — `battle_special_options`'
/// two exclusions, applied at the other end.
#[test]
fn a_routine_that_is_never_run_in_a_fight_is_refused_on_a_battle_map() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    let field_only = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .find(|d| d.effect.field_only())
        .expect("no field-only routine ships")
        .id
        .clone();
    only_routine(&mut game, player, &field_only);
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();

    assert!(!game.tactical_use_routine(0, at));
    assert_eq!(game.tactical_actor(), Some(player));
}

/// A capture on a battle map is aimed at a body, and it is the same capture:
/// `decompile_body`, reached through a cell rather than a group index.
#[test]
fn a_capture_on_a_battle_map_turns_the_program_it_was_aimed_at() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "decompile");
    game.world.get_mut::<Stats>(pack[0]).unwrap().hp = 1;
    game.world
        .get_mut::<crate::components::Decompiler>(player)
        .unwrap()
        .skill = 50;
    crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);

    for _ in 0..50 {
        if game
            .world
            .get::<crate::components::Tamed>(pack[0])
            .is_some()
        {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(pack[0])
            .expect("the target left the board");
        // Walk into reach: a capture is a `Single` at arm's length.
        while game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .is_some_and(|from| crate::tactical::reach::distance(from, at) > 1)
        {
            let from = game
                .world
                .resource::<TacticalBattle>()
                .cell_of(player)
                .unwrap();
            let dir = ((at.0 - from.0).signum(), (at.1 - from.1).signum());
            if game.tactical_step(dir) != StepOutcome::Moved {
                break;
            }
        }
        // Out of reach with the turn's movement spent: hand the turn on and
        // close the rest of the gap on the next one.
        if !game.tactical_use_routine(0, at) {
            game.tactical_end_turn();
        }
    }

    assert!(
        game.world
            .get::<crate::components::Tamed>(pack[0])
            .is_some(),
        "the program was never captured"
    );
    assert!(
        game.world.get::<Hostile>(pack[0]).is_none(),
        "a captured program is still hostile"
    );
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the last hostile left the board and the fight stayed open"
    );
    assert!(
        game.tactical_result_view()
            .expect("a closing board")
            .fallen
            .is_empty(),
        "a captured program did not fall"
    );
}

/// Hands the fight round until `bound` turns have passed, driving every
/// hostile through the AI and ending every party body's turn at once.
/// Bounded, so a model that stops handing the turn on fails rather than
/// hangs.
fn run_ai_rounds(game: &mut Game, bound: usize) {
    for _ in 0..bound {
        if game.tactical_actor().is_none() {
            return;
        }
        if !game.tactical_ai_turn() {
            game.tactical_end_turn();
        }
    }
}

pub(super) fn cell_of(game: &Game, body: Entity) -> Option<(i32, i32)> {
    game.world.resource::<TacticalBattle>().cell_of(body)
}

/// The whole point of the file: a hostile left to itself closes the
/// deployment gap and swings, so a tactical fight finishes without the
/// player driving both sides.
#[test]
fn a_hostile_closes_the_deployment_gap_on_its_own_and_swings() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let opened = crate::tactical::reach::distance(
        cell_of(&game, player).expect("the player is on the board"),
        cell_of(&game, pack[0]).expect("the hostile is on the board"),
    );
    let before = hp_of(&game, player);

    run_ai_rounds(&mut game, 40);

    assert!(
        hp_of(&game, player) < before,
        "a hostile that crossed {opened} cells never swung: {before} HP untouched"
    );
}

/// A companion's turn is the player's turn, and the screen has to say so.
///
/// **The third predicate.** `tactical_ai_actor` and
/// `Game::tactical_awaits_input` are deliberately one definition, gated on
/// `Hostile`; `TacticalView::player_turn` was a third, gated on `Player` —
/// so on a companion's turn app-core sat waiting for a key while the screen
/// said the wild side was moving and offered no keys to press.
#[test]
fn a_companion_s_turn_reads_as_the_player_s_on_the_screen() {
    let mut game = game();
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    tactical_fight(&mut game, 1, 40);
    assert!(
        wait_for_turn(&mut game, companion),
        "the companion never got a turn"
    );

    assert!(
        game.tactical_awaits_input(),
        "the engine is waiting on a key for this body"
    );
    let view = game.tactical_view().expect("the fight is open");
    assert!(
        view.player_turn,
        "...and the screen must agree: a companion's turn is the player's, so \
         the keybar owes it the action keys rather than `the wild side is moving`"
    );
}

/// A hostile's turn is nobody's to command, on both surfaces.
#[test]
fn a_hostile_s_turn_reads_as_the_wild_side_s() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    assert!(wait_for_turn(&mut game, hostile), "the hostile never acted");

    assert!(!game.tactical_awaits_input());
    let view = game.tactical_view().expect("the fight is open");
    assert!(!view.player_turn, "the screen offered keys for a wild body");
}

/// The AI drives one side. A party body's turn is the player's to spend, and
/// `false` is what tells a driver to wait for input rather than to hand the
/// turn on.
#[test]
fn a_party_body_s_turn_is_not_the_ai_s_to_drive() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the player never acted");
    let stood = cell_of(&game, player);

    assert!(
        !game.tactical_ai_turn(),
        "the AI claimed a turn that belongs to the player"
    );
    assert_eq!(cell_of(&game, player), stood, "the AI moved the player");
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a refused turn must not be handed on"
    );
}

/// The trap this phase was shaped around. A hostile holds no `PowerReserve`
/// by design, so `ability_unavailable` refuses it every priced routine there
/// is — and every routine that can be run is priced. Routed through the
/// player's door the AI's routine arm would compile, test green and never
/// once fire.
///
/// Asserted in both halves deliberately: that the player's gate really does
/// refuse this hostile this routine, and that it runs anyway.
///
/// Read off the log and the cooldown after **one** turn rather than off a
/// finished fight: teardown wipes every battle-scoped component, so a
/// cooldown asked about after the last blow is gone whether it was armed or
/// not.
#[test]
fn a_hostile_runs_a_routine_the_player_s_own_gate_would_refuse_it() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    // The hostile acts first, so its turn does not close the round: closing
    // it ages cooldowns and runs a world tick, and where a hostile lands in
    // the rolled order is seed luck.
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![wild, player]);
    only_routine(&mut game, wild, "acid_wash");
    let def = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("acid_wash")
        .cloned()
        .expect("acid_wash ships");
    assert!(
        game.ability_unavailable(wild, &def).is_some(),
        "a hostile with no reserve must be refused a priced routine, or this \
         test is not covering the trap it was written for"
    );

    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    assert!(
        log_texts(&game).iter().any(|l| l.contains(&def.name)),
        "the hostile never ran the routine it was carrying: {:?}",
        log_texts(&game)
    );
    assert_eq!(
        game.world
            .get::<crate::components::AbilityCooldowns>(wild)
            .and_then(|c| c.0.get("acid_wash").copied()),
        Some(crate::abilities::armed_cooldown(
            def.cooldown,
            crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN
        )),
        "a routine that ran must be cooled at the enemy side's own floor"
    );
}

/// `RoutineRefusal::Power` is the variant `ability_unavailable` returns when
/// the reserve is the only thing standing in the way — Phase 1 of #103, a
/// pure refactor from `Option<String>` to `Option<RoutineRefusal>` with no
/// behaviour change, so this is the one new assertion the phase adds.
#[test]
fn an_unaffordable_routine_refuses_with_the_power_variant() {
    let mut game = game();
    let player = game.player_entity();
    let def = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("acid_wash")
        .cloned()
        .expect("acid_wash ships");
    game.world
        .entity_mut(player)
        .insert(crate::components::PowerReserve::new(
            0.0,
            crate::components::POWER_MAX,
        ));
    assert_eq!(
        game.ability_unavailable(player, &def),
        Some(crate::game::combat::RoutineRefusal::Power { cost: 8.0 }),
        "an empty reserve short of the routine's cost must refuse as Power"
    );
}

/// A hostile's routine is floored at `ENEMY_ROUTINE_MIN_COOLDOWN`, so one
/// whose file authors no cooldown cannot be run every turn of the fight.
///
/// The fixture is a shipped routine with its cooldown edited to zero,
/// because **no shipped routine can reach this branch**: `field_only_dead_
/// fields` warns about a cooldown on a field-only effect, so every shipped
/// `cooldown: 0` routine is field-only and `wild_routine_ready` excludes it.
/// The branch guards a mod, and the edit is what stands in for one — driven
/// through a real turn, since the floor the AI *passes* is the half a call
/// to `run_tactical_routine` would not cover.
#[test]
fn a_hostile_s_routine_is_floored_even_when_its_file_authors_no_cooldown() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    // The hostile acts first, so its turn does not close the round: closing
    // it ages cooldowns and runs a world tick, and where a hostile lands in
    // the rolled order is seed luck.
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![wild, player]);
    let mut free = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("acid_wash")
        .cloned()
        .expect("acid_wash ships");
    free.cooldown = 0;
    game.world
        .resource_mut::<crate::abilities::AbilityDb>()
        .insert(free.clone());
    only_routine(&mut game, wild, &free.id);

    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    assert_eq!(
        game.world
            .get::<crate::components::AbilityCooldowns>(wild)
            .and_then(|c| c.0.get(&free.id).copied()),
        Some(crate::abilities::armed_cooldown(
            0,
            crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN
        )),
        "a routine its file left uncooled must still be floored for a hostile"
    );
}
