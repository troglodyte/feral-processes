//! `AbilityEffect::Tamper`: the schema, the load checks, the effect hidden
//! everywhere it cannot run, and what each kind does on a battle map once it
//! lands — temperature, injection, taking a companion over, and a
//! Hallucination's decoys.

use super::support::{
    battle_with_a_pack_of, force_the_next_attack_to_land, generic_species, insert_battle,
    spawn_wild_without_routine, test_assets_dir,
};
use super::tactical::{
    body, free_neighbour, log_texts, marooned, next_draw, only_routine, open_ground, place_one,
    tactical_fight, wait_for_turn,
};
use crate::Game;
use crate::abilities::{
    AbilityDb, AbilityDef, AbilityEffect, AbilityTarget, TamperKind, TamperSlot,
};
use crate::battle::BattleAction;
use crate::components::{
    AbilityCooldowns, ActiveStatus, Creature, GlyphColor, Position, PowerReserve, Routines, Stats,
    StatusEffects, StatusKind, Tampered,
};
use crate::items::ItemId;
use crate::resources::{DifficultyMode, Party, Sorties};
use crate::species::{SpeciesDb, SpeciesDef};
use crate::tactical::ai::{AiBeat, ForecastAction};
use crate::tactical::map::{BattleCell, Board};
use crate::tactical::view::{DecoyView, TamperTag};
use crate::tactical::{Decoy, TacticalBattle, reach};
use crate::tuning::{ENEMY_ROUTINE_MIN_COOLDOWN, TACTICAL_AI_TEMPERATURE, TACTICAL_MOVE_MAX};
use bevy_ecs::prelude::Entity;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A minimal, otherwise-legal Tamper `AbilityDef` — enemy-facing, single
/// target, no authored shape — for a test that mutates exactly one field
/// and calls `tamper_faults` directly, the way `summon_target_mismatch`'s
/// siblings are meant to be exercised.
fn tamper_def(kind: TamperKind, duration: u32) -> AbilityDef {
    AbilityDef {
        id: "test_tamper".into(),
        name: "Test Tamper Single".into(),
        description: "d".into(),
        target: AbilityTarget::OneEnemyGroupFront,
        effect: AbilityEffect::Tamper { kind, duration },
        cooldown: 1,
        power_cost: 1.0,
        accuracy: 0,
        wild_weight: 0,
        research_zone: 0,
        exclusive: false,
        starter: false,
        ranged: false,
        boss_drop: None,
        triggers: None,
        shape: None,
        range: None,
    }
}

/// Decision 8's price table, verbatim: each id's `TamperKind` and `duration`.
#[test]
fn the_five_tamper_routines_load_with_the_kinds_they_author() {
    let game = game(9500);
    let db = game.world.resource::<AbilityDb>();
    let cases: [(&str, TamperKind, u32, u32, f32); 5] = [
        ("cold_sample", TamperKind::Temperature(0.0), 3, 3, 6.0),
        ("heat_injection", TamperKind::Temperature(2.0), 2, 4, 12.0),
        ("inference_probe", TamperKind::Profiled, 3, 3, 5.0),
        ("prompt_injection", TamperKind::Injected, 1, 5, 12.0),
        (
            "hallucination",
            TamperKind::Hallucinating { decoys: 3 },
            3,
            5,
            14.0,
        ),
    ];
    for (id, kind, duration, cooldown, power_cost) in cases {
        let def = db.get(id).unwrap_or_else(|| panic!("{id} ships"));
        match &def.effect {
            AbilityEffect::Tamper {
                kind: found_kind,
                duration: found_duration,
            } => {
                assert_eq!(*found_kind, kind, "{id} authors the wrong TamperKind");
                assert_eq!(*found_duration, duration, "{id} authors the wrong duration");
            }
            other => panic!("{id} is not a Tamper effect: {other:?}"),
        }
        assert_eq!(def.cooldown, cooldown, "{id} authors the wrong cooldown");
        assert_eq!(
            def.power_cost, power_cost,
            "{id} authors the wrong power_cost"
        );
    }
}

/// Spec test 13's other half: `field_runnable` is false and `tactical_only`
/// is true for all five, and `tactical_only` reads false for an ordinary
/// battle routine.
#[test]
fn a_tamper_routine_never_runs_on_the_map() {
    let game = game(9501);
    let db = game.world.resource::<AbilityDb>();
    for id in [
        "cold_sample",
        "heat_injection",
        "inference_probe",
        "prompt_injection",
        "hallucination",
    ] {
        let def = db.get(id).unwrap_or_else(|| panic!("{id} ships"));
        assert!(def.effect.tactical_only(), "{id} must be tactical-only");
        assert!(!def.field_runnable(), "{id} must never appear on the map");
    }
    let deadlock = db.get("deadlock").expect("deadlock ships");
    assert!(
        !deadlock.effect.tactical_only(),
        "an ordinary battle routine is not tactical-only"
    );
}

/// **Every tactical-only routine says so in its own description**, and so
/// does its synthesised routine-tree node — trivially, since
/// `routine_tree::synthesise_nodes` copies a node's description straight off
/// the ability's own (todo #101 moved these off `model_inspection`, which no
/// longer teaches anything, into one node per routine).
///
/// Tactical fights sit behind `profile.tactical_battles`, which is off until
/// a player turns it on. `Game::battle_special_options` filters a
/// tactical-only routine out of the group model's picker, so to a player who
/// never enabled the toggle these five are simply absent with no explanation
/// — and `ability_unavailable`'s `"battle map only"` string has no caller
/// that reaches a screen. The description is the only place they are told,
/// which is why this is a census and not a convention.
///
/// Walked off `tactical_only` rather than off a list of ids, so any further
/// battle-map-only routine shipped without the sentence fails the build —
/// `teleport` is the sixth and was caught by exactly that.
#[test]
fn every_tactical_only_routine_says_it_is_battle_map_only() {
    const SENTENCE: &str = "Battle maps only.";
    let game = game(9502);
    let db = game.world.resource::<AbilityDb>();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let mut checked = 0;
    for def in db.all().filter(|d| d.effect.tactical_only()) {
        assert!(
            def.description.contains(SENTENCE),
            "{} is battle-map only and never says so: {:?}",
            def.id,
            def.description
        );
        let node = research
            .get(&crate::routine_tree::node_id(&def.id))
            .unwrap_or_else(|| panic!("{} should have a synthesised node", def.id));
        assert!(
            node.description.contains(SENTENCE),
            "the node that teaches {} must say so too: {:?}",
            def.id,
            node.description
        );
        checked += 1;
    }
    assert_eq!(
        checked, 6,
        "the five tamper routines and Teleport are what ships today"
    );
}

/// One case per `tamper_faults` refusal, built by hand rather than round-
/// tripped through a scratch `.ron` file.
#[test]
fn a_malformed_tamper_is_refused_at_load() {
    assert!(
        tamper_def(TamperKind::Profiled, 0)
            .tamper_faults()
            .is_some(),
        "duration: 0 must be refused"
    );
    assert!(
        tamper_def(TamperKind::Temperature(f32::NAN), 1)
            .tamper_faults()
            .is_some(),
        "a non-finite temperature must be refused"
    );
    assert!(
        tamper_def(TamperKind::Temperature(-1.0), 1)
            .tamper_faults()
            .is_some(),
        "a negative temperature must be refused"
    );
    assert!(
        tamper_def(TamperKind::Hallucinating { decoys: 0 }, 1)
            .tamper_faults()
            .is_some(),
        "decoys: 0 must be refused"
    );
    let mut ally_facing = tamper_def(TamperKind::Profiled, 1);
    ally_facing.target = AbilityTarget::OneAlly;
    assert!(
        ally_facing.tamper_faults().is_some(),
        "a tamper names the other side, so an ally-facing target must be refused"
    );
    // No authored `shape:`, so this derives `Single` off `OneEnemyGroupFront`
    // — not the `Radius` a Hallucinating entry needs.
    assert!(
        tamper_def(TamperKind::Hallucinating { decoys: 3 }, 1)
            .tamper_faults()
            .is_some(),
        "Hallucinating on a non-Radius shape must be refused"
    );
    assert!(
        tamper_def(TamperKind::Temperature(0.0), 1)
            .tamper_faults()
            .is_none(),
        "a well-formed Tamper must not be refused, or the fixture proves nothing"
    );
}

/// Spec test 13: no hostile carrier ever readies a tamper routine, but an
/// ordinary one still comes up ready.
#[test]
fn no_hostile_readies_a_tamper_routine() {
    let mut game = game(9502);
    let tamperer = game
        .world
        .spawn(Routines(vec!["cold_sample".to_string()]))
        .id();
    assert!(
        game.wild_routine_ready(tamperer).is_none(),
        "cold_sample is tactical-only and no AI may choose it"
    );

    let carrier = game
        .world
        .spawn(Routines(vec!["deadlock".to_string()]))
        .id();
    assert!(
        game.wild_routine_ready(carrier).is_some(),
        "an ordinary carrier routine must still come up ready"
    );
}

/// Spec test 14: the group model's Special menu omits a tamper routine the
/// player knows, and the tactical model's offers it.
#[test]
fn the_group_models_routine_rows_omit_tamper_routines() {
    let mut group = game(9503);
    let player = group.player_entity();
    let enemies = battle_with_a_pack_of(&mut group, 1, 40);
    insert_battle(&mut group, player, enemies);
    group.world.entity_mut(player).insert(Routines(vec![
        "cold_sample".to_string(),
        "deadlock".to_string(),
    ]));
    let group_names: Vec<String> = group
        .battle_special_options(0)
        .into_iter()
        .map(|o| o.name)
        .collect();
    assert_eq!(
        group_names,
        vec!["Hard Lock Single v1.0".to_string()],
        "the group model must offer deadlock alone, never cold_sample"
    );

    let mut tactical = game(9504);
    tactical_fight(&mut tactical, 1, 40);
    let player = tactical.player_entity();
    tactical.world.entity_mut(player).insert(Routines(vec![
        "cold_sample".to_string(),
        "deadlock".to_string(),
    ]));
    assert!(wait_for_turn(&mut tactical, player));
    let tactical_names: Vec<String> = tactical
        .tactical_routine_options()
        .into_iter()
        .map(|o| o.name)
        .collect();
    assert!(
        tactical_names.contains(&"Cold Sample Single".to_string()),
        "a battle map must still offer the tamper routine: {tactical_names:?}"
    );
    assert!(
        tactical_names.contains(&"Hard Lock Single v1.0".to_string()),
        "and the ordinary one alongside it: {tactical_names:?}"
    );
}

/// Spec test 9's door, from the other side: a tamper routine known outside
/// a tactical fight is refused before it can be planned.
#[test]
fn a_tamper_routine_is_refused_outside_a_battle_map() {
    let mut game = game(9505);
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 40);
    insert_battle(&mut game, player, enemies);
    let cold_sample = game
        .world
        .resource::<AbilityDb>()
        .get("cold_sample")
        .cloned()
        .expect("cold_sample ships");

    assert!(
        game.ability_unavailable(player, &cold_sample).is_some(),
        "a tactical-only routine must be refused in a group fight"
    );
}

/// `choose_summon_action`'s exclusion: a forked body whose only routine is
/// a Tamper must fall back to a basic attack rather than ever reach
/// `use_ability`'s `unreachable!` arm for it.
#[test]
fn a_forks_only_tamper_routine_is_never_chosen() {
    let mut game = game(9506);
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
    if let Some(mut stats) = game.world.get_mut::<Stats>(wild) {
        stats.max_hp = 4000;
        stats.hp = 4000;
    }
    insert_battle(&mut game, player, vec![wild]);

    let body = game.fork_programs(player, 1, 0)[0];
    game.world
        .entity_mut(body)
        .insert(Routines(vec!["cold_sample".to_string()]));

    // The fork's slot is left unplanned on purpose — `plan_summons` fills
    // it through `choose_summon_action` when the round resolves. Without
    // the `tactical_only` filter, a lucky pick would reach `use_ability`'s
    // `unreachable!` arm for `Tamper` and this call would panic instead of
    // returning.
    assert!(game.battle_set_action(0, BattleAction::Defend).is_ok());
    game.battle_resolve_round();

    // RNG-independent: whether the fork's fallback attack landed is a
    // separate question from whether it stalled trying to run its only
    // (tactical-only) routine. The round advancing at all is the proof —
    // `battle_round_ready` would still be false, and the round would never
    // resolve, if `plan_summons` left the fork's slot empty.
    assert_eq!(
        game.world.resource::<crate::resources::BattleState>().round,
        2,
        "the round must resolve rather than stall on an unplannable action"
    );
}

/// The sortie exclusion matters most: an off-screen squad reaching
/// `use_ability` with a `Tamper` would hit its `unreachable!` arm. Without
/// the `tactical_only` filter in `swing_for_the_squad` this would panic
/// instead of falling back to a basic attack.
#[test]
fn a_dispatched_squads_only_tamper_routine_is_never_run() {
    let (mut game, squad) = super::sorties::a_dispatched_sortie(9507, DifficultyMode::Forgiving);
    for &member in &squad {
        game.world
            .entity_mut(member)
            .insert(Routines(vec!["cold_sample".to_string()]));
    }
    let total = game.world.resource::<Sorties>().0[0].ticks_total;
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed = total / 2;

    game.run_sorties();

    let record = game.world.resource::<Sorties>().0[0].clone();
    assert!(
        record.battles_done > 0,
        "a battle must have fired, or this proves nothing"
    );
}

/// Clears every cell within `radius` of `at` to `Open`, so a test can place
/// bodies without a procedurally generated obstacle refusing the move.
fn clear_area(game: &mut Game, at: (i32, i32), radius: i32) {
    let battle = &mut *game.world.resource_mut::<TacticalBattle>();
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            let (x, y) = (at.0 + dx, at.1 + dy);
            if battle.board.in_bounds(x, y) {
                battle.board.put(x, y, BattleCell::Open);
            }
        }
    }
}

/// Spec test 9's first half: a radius tamper catches a companion and both
/// hostiles standing beside the player, and skips the player standing beside
/// them too.
#[test]
fn a_tamper_lands_on_every_body_in_its_radius_but_the_player() {
    let mut game = game(9600);
    let pack = tactical_fight(&mut game, 2, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    clear_area(&mut game, at, 2);

    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    let beside_hostile_0 = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[0], beside_hostile_0)
    );
    let beside_hostile_1 = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[1], beside_hostile_1)
    );
    let beside_companion = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .place(companion, beside_companion)
    );

    only_routine(&mut game, player, "heat_injection");
    assert!(
        game.tactical_use_routine(0, at),
        "Heat Injection aimed at the player's own cell must land"
    );

    assert!(
        game.world.get::<Tampered>(player).is_none(),
        "the player must never carry a Tampered entry"
    );
    for &caught in &[pack[0], pack[1], companion] {
        let tampered = game
            .world
            .get::<Tampered>(caught)
            .expect("a body standing in the blast must be tampered");
        assert_eq!(
            tampered.temperature(),
            Some(2.0),
            "Heat Injection authors Temperature(2.0)"
        );
    }
}

/// Spec test 9's other half: a `Single` tamper aimed at the player is
/// refused before Power, the cooldown, or the turn are spent.
#[test]
fn a_single_tamper_at_the_player_is_refused_before_anything_is_spent() {
    let mut game = game(9601);
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "prompt_injection");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    let power_before = game.world.get::<PowerReserve>(player).unwrap().get();

    assert!(
        !game.tactical_use_routine(0, at),
        "a single-target tamper aimed at the player must be refused"
    );

    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        power_before,
        "a refusal must not spend Power"
    );
    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| c.0.is_empty()),
        "a refusal must not arm a cooldown"
    );
    let battle = game.world.resource::<TacticalBattle>();
    assert!(
        battle.actions_left() > 0,
        "a refusal must not spend the turn"
    );
    assert_eq!(
        battle.actor(),
        Some(player),
        "a refusal must leave the turn with the player"
    );
}

/// Reapplying a kind refreshes its slot rather than stacking a second entry,
/// and two different kinds coexist on the same body.
#[test]
fn reapplying_a_kind_refreshes_and_heat_replaces_cold() {
    let mut game = game(9602);
    let pack = tactical_fight(&mut game, 1, 400);
    let hostile = pack[0];
    let player = game.player_entity();
    game.world.entity_mut(player).insert(Routines(vec![
        "cold_sample".to_string(),
        "heat_injection".to_string(),
        "inference_probe".to_string(),
    ]));

    assert!(wait_for_turn(&mut game, player));
    let player_at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    // Default deployment opens the two sides `TACTICAL_DEPLOY_GAP` cells
    // apart, which is past every one of these routines' range — brought
    // beside the player instead, the way `a_heal_queues_a_heal_cue_...` does.
    clear_area(&mut game, player_at, 1);
    let at = free_neighbour(&game, player_at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(hostile, at)
    );
    assert!(game.tactical_use_routine(0, at), "cold_sample must land");
    // Back to the player: the hostile's own pass ages whatever it is
    // carrying by one of its turns, `Tampered`'s ordinary case.
    assert!(wait_for_turn(&mut game, player));
    assert!(game.tactical_use_routine(1, at), "heat_injection must land");
    assert!(wait_for_turn(&mut game, player));
    assert!(
        game.tactical_use_routine(2, at),
        "inference_probe must land"
    );

    let tampered = game
        .world
        .get::<Tampered>(hostile)
        .expect("the hostile must still be tampered");
    assert_eq!(
        tampered.temperature(),
        Some(2.0),
        "heat_injection must have replaced cold_sample's entry rather than stacked"
    );
    assert!(
        tampered.has(TamperSlot::Profiled),
        "Profiled must coexist with Temperature"
    );
    assert_eq!(
        tampered.slots().count(),
        2,
        "exactly the two slots just applied, no more"
    );
}

/// **(M)** Spec test 11: a `duration: 1` Injection landed on a hostile that has
/// already had its turn this round is still live at the start of its next
/// one, and is gone the moment that next turn is handed on.
///
/// Mutation check: moving the age call from `hand_on_turn` into
/// `Game::tick_one_combatant` (a once-per-round age, the wrong cadence) made
/// this fail — the injected entry was gone before the hostile's next turn
/// ever began, because the round wrapped and ran upkeep before that turn
/// started. Restored afterward.
#[test]
fn a_one_turn_injection_on_a_body_that_has_acted_is_live_on_its_next_turn() {
    let mut game = game(9603);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    let player_at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    // Brought within prompt_injection's range 0–3 — default deployment opens
    // the two sides `TACTICAL_DEPLOY_GAP` (6) cells apart.
    clear_area(&mut game, player_at, 1);
    let at = free_neighbour(&game, player_at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(hostile, at)
    );

    // Let the hostile's own turn pass this round before it is tampered.
    assert!(wait_for_turn(&mut game, hostile));
    game.tactical_end_turn();

    assert!(wait_for_turn(&mut game, player));
    only_routine(&mut game, player, "prompt_injection");
    assert!(
        game.tactical_use_routine(0, at),
        "prompt_injection must land"
    );

    assert_eq!(
        game.tactical_actor(),
        Some(hostile),
        "with only two bodies, the hostile's next turn must be current"
    );
    assert!(
        game.world
            .get::<Tampered>(hostile)
            .is_some_and(|t| t.has(TamperSlot::Injected)),
        "Injected must still be live at the start of the hostile's next turn"
    );

    game.tactical_end_turn();
    assert!(
        game.world.get::<Tampered>(hostile).is_none(),
        "Injected must wear off exactly when the tampered turn is handed on"
    );
}

/// Decision 5: a companion's own radius tamper can catch itself, and the
/// hand-on that applied it must not also spend the entry's first turn.
#[test]
fn a_self_applied_entry_is_not_aged_by_the_turn_that_applied_it() {
    let mut game = game(9604);
    let companion = body(&mut game, &generic_species().id);
    game.world
        .entity_mut(companion)
        .insert(PowerReserve::default());
    game.world.resource_mut::<Party>().0.push(companion);
    let pack = tactical_fight(&mut game, 1, 400);
    let hostile = pack[0];
    let player = game.player_entity();

    // Isolate the three bodies so Heat Injection's radius, aimed at the
    // companion's own cell, catches nobody else.
    place_one(&mut game, player, (0, 0));
    place_one(&mut game, hostile, (13, 13));
    let isolated = (6, 6);
    game.world
        .resource_mut::<TacticalBattle>()
        .board
        .put(isolated.0, isolated.1, BattleCell::Open);
    place_one(&mut game, companion, isolated);

    only_routine(&mut game, companion, "heat_injection");
    assert!(wait_for_turn(&mut game, companion));
    assert!(
        game.tactical_use_routine(0, isolated),
        "the companion's self-aimed Heat Injection must land"
    );

    let entry = game
        .world
        .get_mut::<Tampered>(companion)
        .and_then(|mut t| t.remove(TamperSlot::Temperature))
        .expect("the self-applied entry must still be live");
    assert_eq!(
        entry.remaining, 2,
        "the turn that applied the entry must not also have aged it"
    );
}

/// A body's own `Tampered` entry ages on its own turn, and a turn is not the
/// same thing as an action — two actions in one turn must age it once, not
/// twice, or a squad's second action would halve every duration authored
/// against it. `actions_left` is driven directly rather than by seating a
/// real squad, since what is being pinned is the ageing and not the
/// formation.
#[test]
fn a_tampered_body_ages_once_a_turn_however_many_actions_it_spends() {
    let mut game = game(9606);
    let pack = tactical_fight(&mut game, 1, 400);
    let wild = pack[0];
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Temperature(0.0), 5, false);
    game.world.entity_mut(wild).insert(tampered);
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    let entry = game
        .world
        .get_mut::<Tampered>(wild)
        .and_then(|mut t| t.remove(TamperSlot::Temperature))
        .expect("the entry must still be live after one turn");
    assert_eq!(
        entry.remaining, 4,
        "two actions in one turn must age the entry once, not twice"
    );
}

/// Spec test 12's component half: `Tampered` does not survive the fight it
/// was written in, won through the normal door.
#[test]
fn tampered_is_gone_when_the_fight_ends() {
    let mut game = game(9605);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    let pack = tactical_fight(&mut game, 1, 1);
    let hostile = pack[0];
    let player = game.player_entity();

    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    clear_area(&mut game, at, 1);
    let n1 = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(hostile, n1)
    );
    let n2 = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(companion, n2)
    );

    only_routine(&mut game, player, "heat_injection");
    assert!(
        game.tactical_use_routine(0, at),
        "Heat Injection must catch both neighbours"
    );
    assert!(
        game.world.get::<Tampered>(hostile).is_some(),
        "fixture: the hostile must be tampered before it dies"
    );
    assert!(
        game.world.get::<Tampered>(companion).is_some(),
        "fixture: the companion must be tampered before the fight ends"
    );

    assert!(wait_for_turn(&mut game, player));
    force_the_next_attack_to_land(&mut game);
    assert!(
        game.tactical_attack(hostile),
        "the killing blow must land on a 1 HP hostile"
    );

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "fixture: the fight must have ended"
    );
    assert!(
        game.world.get::<Tampered>(player).is_none(),
        "Tampered must not survive the fight ending"
    );
    assert!(
        game.world.get::<Tampered>(companion).is_none(),
        "Tampered must not survive the fight ending"
    );
}

/// `marooned`, with its hostile Cold Sampled directly rather than by
/// running the routine — `decision_temperature`'s read is the door under
/// test, not `apply_tamper`'s.
fn cold_marooned() -> (Game, Entity) {
    let (mut game, wild) = marooned();
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Temperature(0.0), 3, false);
    game.world.entity_mut(wild).insert(tampered);
    (game, wild)
}

/// Spec test 1: a body under `Temperature(0.0)` takes the argmax and draws
/// nothing from `GameRng` over its turn; the same body without it draws.
///
/// **(M)** Mutation check: hardcoding `decision_temperature` to always
/// answer `tuning::TACTICAL_AI_TEMPERATURE` (ignoring `Tampered`) makes the
/// cold hostile's turn draw like the untampered one, and the first
/// `assert_eq!` fails. Verified and restored — see the commit body.
#[test]
fn a_cold_sampled_hostile_draws_nothing_over_its_turn() {
    let (mut cold, _) = cold_marooned();
    let (mut virgin, _) = marooned();
    assert!(
        cold.tactical_ai_turn(),
        "the cold hostile's turn was not run"
    );
    assert_eq!(
        next_draw(&mut cold),
        next_draw(&mut virgin),
        "a Cold Sample argmax turn must draw nothing from GameRng"
    );

    let (mut warm, _) = marooned();
    let (mut warm_virgin, _) = marooned();
    assert!(
        warm.tactical_ai_turn(),
        "the untampered hostile's turn was not run"
    );
    assert_ne!(
        next_draw(&mut warm),
        next_draw(&mut warm_virgin),
        "the untampered twin must still draw over its turn"
    );
}

/// Spec test 2: every tactical AI call site reads `decision_temperature`.
/// The same "drew nothing" assertion from the test above, run through each
/// of the four production doors `decision_temperature` unified.
///
/// **(M)** Mutation check: reverting any one of the four call sites in
/// `tactical/ai.rs` back to the bare `TACTICAL_AI_TEMPERATURE` constant
/// makes exactly that door's `assert_eq!` fail, the other three staying
/// green. Verified per site and restored — see the commit body.
#[test]
fn every_tactical_ai_door_reads_the_temperature_door() {
    let virgin_draw = {
        let (mut game, _) = marooned();
        next_draw(&mut game)
    };

    let (mut via_turn, _) = cold_marooned();
    assert!(
        via_turn.tactical_ai_turn(),
        "tactical_ai_turn did not run the cold hostile's turn"
    );
    assert_eq!(
        next_draw(&mut via_turn),
        virgin_draw,
        "tactical_ai_turn must read decision_temperature"
    );

    let (mut via_beat, _) = cold_marooned();
    for _ in 0..=TACTICAL_MOVE_MAX {
        if via_beat.tactical_ai_beat() == AiBeat::Acted {
            break;
        }
    }
    assert_eq!(
        next_draw(&mut via_beat),
        virgin_draw,
        "tactical_ai_beat must read decision_temperature"
    );

    let (mut via_auto, _) = cold_marooned();
    for _ in 0..=TACTICAL_MOVE_MAX {
        if via_auto.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }
    assert_eq!(
        next_draw(&mut via_auto),
        virgin_draw,
        "tactical_auto_beat must read decision_temperature"
    );

    let (mut via_drive, _) = cold_marooned();
    assert!(
        via_drive.tactical_drive_turn(),
        "tactical_drive_turn did not run the cold hostile's turn"
    );
    assert_eq!(
        next_draw(&mut via_drive),
        virgin_draw,
        "tactical_drive_turn must read decision_temperature"
    );
}

/// Guards against a `decision_temperature` that zeroes every tampered body
/// rather than reading its entry: a Heat-tampered hostile must still draw.
#[test]
fn heat_leaves_the_draw_in_place() {
    let (mut heat, wild) = marooned();
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Temperature(2.0), 3, false);
    heat.world.entity_mut(wild).insert(tampered);

    let (mut virgin, _) = marooned();
    assert!(
        heat.tactical_ai_turn(),
        "the heat-tampered hostile's turn was not run"
    );
    assert_ne!(
        next_draw(&mut heat),
        next_draw(&mut virgin),
        "a Heat-tampered turn must still draw from GameRng"
    );
}

/// Writes a live `TamperKind::Injected` entry directly, the way the tests
/// below want it — `decision_temperature`'s tests' own reason for reading
/// `Tampered` rather than running `prompt_injection`: what is under test is
/// `acts_for_hostiles`, not `apply_tamper`.
fn inject(game: &mut Game, body: Entity) {
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Injected, 5, false);
    game.world.entity_mut(body).insert(tampered);
}

/// **(M)** Spec test 5: an injected hostile's own packmate becomes its
/// target, with the player left out of reach so a swing at the player is
/// the only alternative reading available.
///
/// Mutation check: reverting `acts_for_hostiles` to `Hostile(actor)` alone
/// (ignoring `Injected`) makes the injected body read its packmate as an
/// ally again — `tactical_sides` hands it the unreachable player as its only
/// target, so its turn walks and never swings, and the packmate's Integrity
/// assertion fails. Verified and restored — see the commit body.
#[test]
fn an_injected_hostile_swings_at_a_packmate() {
    let mut game = game(9700);
    let pack = tactical_fight(&mut game, 2, 40);
    open_ground(&mut game, &pack, &[(0, 0), (0, 1)]);
    let injected = pack[0];
    let packmate = pack[1];
    inject(&mut game, injected);

    assert!(wait_for_turn(&mut game, injected));
    let packmate_cell = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(packmate)
        .expect("the packmate was not seated");
    let hp_before = game.world.get::<Stats>(packmate).unwrap().hp;
    force_the_next_attack_to_land(&mut game);

    assert!(
        game.tactical_ai_turn(),
        "the injected hostile's turn was not run"
    );

    assert!(
        game.world.get::<Stats>(packmate).unwrap().hp < hp_before,
        "the packmate's Integrity must have fallen"
    );
    let bolts = game.take_bolts();
    assert!(
        bolts.iter().any(|b| b.to == packmate_cell),
        "the swing's bolt must land on the packmate's cell: {bolts:?}"
    );
}

/// A species whose `equipment_drop` chance is 1.0, minted off whatever
/// species `tactical_fight` already spawns — `extraction.rs`'s `..template`
/// pattern for `SpeciesDb::insert` — so a kill's loot assertion does not
/// depend on a shipped drop table that may or may not land inside a test's
/// lifetime. `random_bool(1.0)` always accepts regardless of where in the
/// stream it falls, which is what makes the drop itself deterministic even
/// though the rarity `grant_gear_drop` rolls afterward is not (see the
/// caller's own comment on that).
fn guaranteed_looter_species(game: &Game) -> SpeciesDef {
    let template = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one shipped species");
    SpeciesDef {
        id: "task4_guaranteed_looter".to_string(),
        equipment_drop: Some((ItemId::from(crate::items::ids::CORE_FRAGMENT), 1.0)),
        ..template
    }
}

/// **(M)** Spec test 5's other half: a hostile killed by an injected
/// packmate's swing pays exactly what any hostile death pays —
/// `reap_tactical_dead`'s `Hostile` check names the victim, never the
/// attacker, but only if the injected body ever gets to swing at it at all —
/// XP and loot alike.
///
/// Compared against the identical kill struck by the player: `kill_xp` is a
/// pure function of the victim's own Integrity ceiling, so the two XP totals
/// must agree — an attacker-keyed XP formula is the gap this pins shut. The
/// two kills' *loot* is compared by item and quantity, not by the full
/// `GearCopy` `award_loot` records: `grant_gear_drop` rolls the rarity from
/// `GameRng` at whatever position the stream happens to be in, and an
/// AI-driven turn spends a walk-decision draw and a move-selection draw the
/// player's own direct `tactical_attack` call does not, so the two runs read
/// different rarities off an identical, deterministic (`chance: 1.0`) drop —
/// comparing the roll would be asserting on incidental stream position,
/// which is unsound (see the `rng-stream-shift` memory entries), not on the
/// invariant this test is for.
///
/// Mutation check: reverting `acts_for_hostiles` leaves the injected body
/// walking toward the unreachable player instead of swinging at its
/// packmate, so the packmate never dies and `fight_rewards_mut` never fills —
/// the `is_none()` Stats assertion and the XP assertion both fail. Separately,
/// commenting out `award_loot`'s `record_drop` call makes the loot assertion
/// fail on its own while the XP assertions stay green, proving it is not
/// riding on the kill assertion above it. Both verified and restored — see
/// the commit body.
#[test]
fn a_packmate_killed_by_an_injected_hostile_pays() {
    let (paid_xp, paid_drops) = {
        let mut game = game(9701);
        let looter = guaranteed_looter_species(&game);
        let looter_id = looter.id.clone();
        game.world.resource_mut::<SpeciesDb>().insert(looter);
        let pack = tactical_fight(&mut game, 2, 40);
        open_ground(&mut game, &pack, &[(0, 0), (0, 1)]);
        let injected = pack[0];
        let packmate = pack[1];
        game.world.get_mut::<Creature>(packmate).unwrap().species = looter_id;
        game.world.get_mut::<Stats>(packmate).unwrap().hp = 1;
        inject(&mut game, injected);

        assert!(wait_for_turn(&mut game, injected));
        force_the_next_attack_to_land(&mut game);
        assert!(
            game.tactical_ai_turn(),
            "the injected hostile's turn was not run"
        );

        assert!(
            game.world.get::<Stats>(packmate).is_none(),
            "the packmate must have died to the blow"
        );
        let rewards = game
            .fight_rewards_mut()
            .expect("the fight is still open with the injected hostile left standing");
        (rewards.player.xp, drop_totals(&rewards.drops))
    };
    assert!(paid_xp > 0, "the kill must pay the player XP");
    assert!(
        paid_drops
            .iter()
            .any(|(item, qty)| item.as_str() == crate::items::ids::CORE_FRAGMENT && *qty > 0),
        "a packmate killed by an injected hostile must still pay loot: {paid_drops:?}"
    );

    // The identical body, killed by the player's own swing instead —
    // `finish_hostile` must not price the two differently.
    let mut by_player = game(9701);
    let looter = guaranteed_looter_species(&by_player);
    let looter_id = looter.id.clone();
    by_player.world.resource_mut::<SpeciesDb>().insert(looter);
    let by_pack = tactical_fight(&mut by_player, 2, 40);
    open_ground(&mut by_player, &by_pack, &[(0, 0), (0, 1)]);
    let player = by_player.player_entity();
    let victim = by_pack[1];
    by_player.world.get_mut::<Creature>(victim).unwrap().species = looter_id;
    by_player.world.get_mut::<Stats>(victim).unwrap().hp = 1;
    let beside = free_neighbour(&by_player, (0, 1));
    place_one(&mut by_player, player, beside);
    assert!(wait_for_turn(&mut by_player, player));
    force_the_next_attack_to_land(&mut by_player);
    assert!(by_player.tactical_attack(victim));

    let rewards = by_player
        .fight_rewards_mut()
        .expect("the fight is still open with the other hostile left standing");
    let player_paid_xp = rewards.player.xp;
    let player_paid_drops = drop_totals(&rewards.drops);
    assert_eq!(
        paid_xp, player_paid_xp,
        "a packmate's kill must pay the same XP as the player's own"
    );
    assert_eq!(
        paid_drops, player_paid_drops,
        "a packmate's kill must pay the same item and quantity as the player's own"
    );
}

/// `(item, quantity)` off a `BattleRewards::drops` tally, dropping the rolled
/// rarity/affix/quality — see `a_packmate_killed_by_an_injected_hostile_pays`'s
/// own doc for why those three are not comparable across two runs.
fn drop_totals(drops: &[(crate::items::GearCopy, u32)]) -> Vec<(ItemId, u32)> {
    drops
        .iter()
        .map(|(copy, qty)| (copy.item.clone(), *qty))
        .collect()
}

/// Spec test 6: an injected hostile's own Heal routine is helpful, and
/// `acts_for_hostiles` flips which side counts as "wanted" for it too — the
/// party, not its own kind.
///
/// `mirror_restore` is `WholeParty`, which derives range 0–0 — aimable only
/// at the invoker's own cell. The walk-scoring `cell_merit` shares with a
/// Swing reads that band as ground to *close on the packmate*, not as "stand
/// here and heal" — a pre-existing property of the model, not something this
/// task changes — so the healer is boxed in by `Blocked` neighbours to keep
/// it from spending the whole turn marching toward its one tracked "enemy"
/// instead of ever reaching the `Routine` branch that runs the heal.
#[test]
fn an_injected_healer_aims_its_heal_at_the_party() {
    let mut game = game(9702);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    let pack = tactical_fight(&mut game, 2, 40);
    let healer = pack[0];
    let packmate = pack[1];
    let healer_at = (1, 1);
    open_ground(&mut game, &pack, &[healer_at, (7, 7)]);
    only_routine(&mut game, healer, "mirror_restore");
    inject(&mut game, healer);
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        for (dx, dy) in [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ] {
            battle
                .board
                .put(healer_at.0 + dx, healer_at.1 + dy, BattleCell::Blocked);
        }
    }
    // Two cells off the healer — inside `TACTICAL_PARTY_RADIUS` (3) but
    // outside the blocked ring, so its placement is untouched by it.
    place_one(&mut game, companion, (healer_at.0, healer_at.1 + 2));
    game.world.get_mut::<Stats>(companion).unwrap().hp = 4;
    game.world.get_mut::<Stats>(packmate).unwrap().hp = 20;

    assert!(wait_for_turn(&mut game, healer));
    assert!(
        game.tactical_ai_turn(),
        "the injected healer's turn was not run"
    );

    assert!(
        game.world.get::<Stats>(companion).unwrap().hp > 4,
        "the companion's Integrity must have risen"
    );
    assert_eq!(
        game.world.get::<Stats>(healer).unwrap().hp,
        40,
        "the healer's own Integrity must not have changed"
    );
    assert_eq!(
        game.world.get::<Stats>(packmate).unwrap().hp,
        20,
        "no hostile's Integrity may have risen — the packmate stands outside the blast"
    );
}

/// An uninjected packmate's own reading of the fight is untouched: it still
/// swings at the player and leaves the injected packmate alone, even though
/// that packmate is now aiming the other way.
#[test]
fn an_uninjected_packmate_still_treats_the_injected_one_as_its_own() {
    let mut game = game(9703);
    let pack = tactical_fight(&mut game, 2, 40);
    // The uninjected one adjacent to the player; the injected one far
    // enough away that it is never a candidate the walk would close on.
    open_ground(&mut game, &pack, &[(0, 0), (3, 4)]);
    let injected = pack[0];
    let uninjected = pack[1];
    inject(&mut game, injected);

    let player = game.player_entity();
    let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;
    let injected_hp_before = game.world.get::<Stats>(injected).unwrap().hp;

    assert!(wait_for_turn(&mut game, uninjected));
    force_the_next_attack_to_land(&mut game);
    assert!(
        game.tactical_ai_turn(),
        "the uninjected hostile's turn was not run"
    );

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < player_hp_before,
        "the uninjected packmate must still swing at the player"
    );
    assert_eq!(
        game.world.get::<Stats>(injected).unwrap().hp,
        injected_hp_before,
        "the uninjected packmate must not swing at its own injected packmate"
    );
}

/// **(M)** Spec test 10, half A: a companion under a live `Temperature`
/// entry is the AI's for the length of its turn, and control returns the
/// instant the entry ages out.
///
/// Mutation check: dropping the `taken_over` arm from `tactical_ai_actor`
/// makes the first `assert!` below fail — `tactical_awaits_input` reads
/// `true` for the companion's turn exactly as an untampered one would,
/// because nothing else in the gate answers for a `Temperature` entry on a
/// non-`Hostile` body. Verified and restored — see the commit body.
#[test]
fn a_heat_caught_companion_stops_awaiting_input_and_returns_when_it_ends() {
    let mut game = game(9800);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    tactical_fight(&mut game, 1, 400);

    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Temperature(0.0), 1, false);
    game.world.entity_mut(companion).insert(tampered);

    assert!(wait_for_turn(&mut game, companion));
    assert!(
        !game.tactical_awaits_input(),
        "a heat-caught companion must not await input"
    );

    let mut acted = false;
    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_ai_beat() == AiBeat::Acted {
            acted = true;
            break;
        }
    }
    assert!(acted, "the AI never finished the companion's turn");
    assert!(
        game.world.get::<Tampered>(companion).is_none(),
        "duration: 1 must have expired on this turn's own hand-on"
    );

    assert!(wait_for_turn(&mut game, companion));
    assert!(
        game.tactical_awaits_input(),
        "control must return once the entry has ended"
    );
}

/// Spec test 10, half B: `Profiled` alone never takes a companion over —
/// only `Temperature` and `Injected` do.
#[test]
fn a_profiled_companion_is_still_commanded() {
    let mut game = game(9801);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    tactical_fight(&mut game, 1, 40);

    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Profiled, 3, false);
    game.world.entity_mut(companion).insert(tampered);

    assert!(wait_for_turn(&mut game, companion));
    assert!(
        game.tactical_awaits_input(),
        "a profiled companion is still the player's to command"
    );
}

/// Spec test 10, half C: an injected companion reads the party as its own
/// target, the same flip `an_injected_hostile_swings_at_a_packmate` proves
/// for the wild side.
#[test]
fn an_injected_companion_swings_at_the_party() {
    let mut game = game(9802);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    let pack = tactical_fight(&mut game, 2, 40);
    // The pack parked out of reach — an injected companion's own kind reads
    // as its ally now, so keeping them close would prove nothing about who
    // it actually swings at.
    let centre = open_ground(&mut game, &pack, &[(0, 0), (0, 1)]);
    let player = game.player_entity();
    let beside_player = free_neighbour(&game, centre);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(companion, beside_player)
    );
    inject(&mut game, companion);

    let hp_before = game.world.get::<Stats>(player).unwrap().hp;
    assert!(wait_for_turn(&mut game, companion));
    force_the_next_attack_to_land(&mut game);

    assert!(
        game.tactical_ai_turn(),
        "the injected companion's turn was not run"
    );

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < hp_before,
        "the injected companion must swing at the party"
    );
}

/// A decoy the party placed on `cell` — the owner a player's Hallucination
/// writes, and the one a hostile sees.
fn party_decoy(cell: (i32, i32)) -> Decoy {
    Decoy {
        cell,
        owner_hostile: false,
        glyph: '@',
        color: GlyphColor::Cyan,
        of_player: true,
    }
}

/// A party-owned decoy on each of `cells` and a live `Hallucinating` entry
/// on `body`, written directly — `inject`'s reason: what is under test below
/// is what a hallucinating body does, not how `apply_tamper` seats one.
fn hallucinate(game: &mut Game, body: Entity, cells: &[(i32, i32)]) {
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        for &cell in cells {
            battle.place_decoy(party_decoy(cell));
        }
    }
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Hallucinating { decoys: 3 }, 3, false);
    game.world.entity_mut(body).insert(tampered);
}

fn decoy_cells(game: &Game) -> Vec<(i32, i32)> {
    game.world
        .resource::<TacticalBattle>()
        .decoys()
        .iter()
        .map(|d| d.cell)
        .collect()
}

fn hallucinating(game: &Game, body: Entity) -> bool {
    game.world
        .get::<Tampered>(body)
        .is_some_and(|t| t.has(TamperSlot::Hallucinating))
}

fn lines_containing(game: &Game, needle: &str) -> usize {
    log_texts(game)
        .iter()
        .filter(|l| l.contains(needle))
        .count()
}

/// Spec test 7's placement half, and Decision 3: the aim cell is taken by
/// the hostile it was aimed at, so the three decoys fill the nearest free
/// cells of the radius, nearest the aim first and then in reading order —
/// and the entry lands on the hostile alone.
#[test]
fn hallucination_places_its_decoys_on_the_nearest_free_cells() {
    let mut game = game(9900);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(4, 6)]);
    only_routine(&mut game, player, "hallucination");
    assert!(wait_for_turn(&mut game, player));

    assert!(
        game.tactical_use_routine(0, (4, 6)),
        "Hallucination aimed at the hostile must run"
    );

    let decoys = game.world.resource::<TacticalBattle>().decoys().to_vec();
    assert_eq!(
        decoys.iter().map(|d| d.cell).collect::<Vec<_>>(),
        vec![(3, 5), (4, 5), (5, 5)],
        "the aim cell is occupied, so the ring one out fills in (y, x) order"
    );
    assert!(
        decoys.iter().all(|d| !d.owner_hostile && d.of_player),
        "every decoy is the player's own: {decoys:?}"
    );
    assert!(
        hallucinating(&game, hostile),
        "the hostile under the blast must be hallucinating"
    );
    assert!(
        game.world.get::<Tampered>(player).is_none(),
        "the player invoked it and must carry nothing"
    );
    assert_eq!(
        lines_containing(&game, "starts seeing decoys"),
        1,
        "the take-hold line fires once, for the one body that took it"
    );
}

/// Asserts that `game`'s acting `body` spent none of its turn: no Power, no
/// cooldown, no action, and the turn still its own.
///
/// One helper rather than one copy per refusal, because "nothing was spent"
/// is four separate reads and a refusal that forgot one of them would pass
/// against three.
fn nothing_was_spent(game: &Game, body: Entity, power_before: f32, what: &str) {
    assert_eq!(
        game.world.get::<PowerReserve>(body).unwrap().get(),
        power_before,
        "{what} must not spend Power"
    );
    assert!(
        game.world
            .get::<AbilityCooldowns>(body)
            .is_none_or(|c| c.0.is_empty()),
        "{what} must not arm a cooldown"
    );
    let battle = game.world.resource::<TacticalBattle>();
    assert!(battle.actions_left() > 0, "{what} must not spend the turn");
    assert_eq!(
        battle.actor(),
        Some(body),
        "{what} must leave the turn with the body that asked"
    );
}

/// Decision 3's other half, corrected by the whole-branch review: with no
/// free cell in the radius there is nowhere to seat a decoy, so the routine
/// is **refused** rather than run for nothing. `board_has_room` is the
/// precedent — a `Summon` with nowhere to put a body refuses for exactly
/// this reason, and spending 14 Power and a five-round cooldown to seat
/// nobody is the same wasted round.
#[test]
fn hallucination_with_no_free_cell_is_refused_before_anything_is_spent() {
    let mut game = game(9901);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(4, 6)]);
    game.world.resource_mut::<TacticalBattle>().board = Board::from_rows(&[
        ".........",
        ".........",
        ".........",
        ".........",
        "..XX.XX..",
        "..XXXXX..",
        "..XX.XX..",
        "..XXXXX..",
        "..XXXXX..",
    ]);
    // (4, 4) is the player's and (4, 6) the hostile's; every other cell of
    // the radius around the aim cannot be stood on.
    only_routine(&mut game, player, "hallucination");
    assert!(wait_for_turn(&mut game, player));
    let power_before = game.world.get::<PowerReserve>(player).unwrap().get();

    assert!(
        !game.tactical_use_routine(0, (4, 6)),
        "a Hallucination with nowhere to seat a decoy must be refused"
    );

    assert!(decoy_cells(&game).is_empty(), "no cell was free");
    assert!(
        game.world.get::<Tampered>(hostile).is_none(),
        "with no decoy placed, nobody may hallucinate"
    );
    assert_eq!(lines_containing(&game, "starts seeing decoys"), 0);
    nothing_was_spent(&game, player, power_before, "a refused Hallucination");
}

/// The second half of the same review finding: decoys seated where no
/// opposing body stands are seen by nobody, so `settle_decoys` drops every
/// one of them inside the very `hand_on_turn` that ended the turn — the
/// player would see them for no frames at all and get no line. Aiming a
/// Hallucination at open ground as bait is exactly that, so it is refused
/// too, and separately: a single test over one of the two passes against a
/// door that only closed the other.
#[test]
fn hallucination_over_nobody_is_refused_before_anything_is_spent() {
    let mut game = game(9903);
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    // The hostile parks in the far corner: (0, 0) is six cells from the aim
    // and so outside the Radius(2), while the aim itself is two from the
    // player and inside the routine's 0..=5 range.
    open_ground(&mut game, &pack, &[(0, 0)]);
    only_routine(&mut game, player, "hallucination");
    assert!(wait_for_turn(&mut game, player));
    let power_before = game.world.get::<PowerReserve>(player).unwrap().get();

    assert!(
        !game.tactical_use_routine(0, (4, 6)),
        "a Hallucination covering no opposing body must be refused"
    );

    assert!(
        decoy_cells(&game).is_empty(),
        "a refusal must seat no decoy: {:?}",
        decoy_cells(&game)
    );
    assert_eq!(lines_containing(&game, "starts seeing decoys"), 0);
    nothing_was_spent(&game, player, power_before, "a refused Hallucination");
}

/// The other side of the two refusals above: with both a free cell and an
/// opposing body under the shape, the routine still runs. Without this the
/// pair could be satisfied by a door that refused every Hallucination.
#[test]
fn a_hallucination_with_a_cell_and_a_target_still_runs() {
    let mut game = game(9904);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(4, 6)]);
    only_routine(&mut game, player, "hallucination");
    assert!(wait_for_turn(&mut game, player));

    assert!(
        game.tactical_use_routine(0, (4, 6)),
        "a Hallucination with room and a target must run"
    );
    assert!(
        hallucinating(&game, hostile),
        "the hostile under the blast must be hallucinating"
    );
}

/// Seating a Hallucination's decoys spends **no** `GameRng` draw — the
/// spec's requirement, and `hallucination_cells`' own doc: the whole
/// placement is a filter and a sort over the board, and a tactical fight's
/// budget is one draw an AI turn.
///
/// The twin-game probe rather than `support::rng_unadvanced_by`, for the
/// reason `a_cold_sampled_hostile_draws_nothing_over_its_turn` uses it too:
/// that fixture builds both games itself and runs the closure on only one,
/// so the fight's own setup draws would land on one side of the comparison
/// and never the other. Here both twins are set up identically and only one
/// is asked to run the routine.
///
/// Measured at `Game::apply_tamper` and not at `tactical_use_routine`,
/// deliberately: the door hands the turn on, and a round's upkeep and the
/// next body's own turn draw for reasons that have nothing to do with where
/// a decoy stands. What the spec asks about is the placement.
///
/// **(M)** Drawing a single `u64` off `GameRng` inside `hallucinate` before
/// the decoys are seated makes this fail. Verified and restored.
#[test]
fn seating_a_hallucination_spends_no_rng_draw() {
    let seated = |run: bool| {
        let mut game = game(9905);
        let pack = tactical_fight(&mut game, 1, 40);
        let player = game.player_entity();
        open_ground(&mut game, &pack, &[(4, 6)]);
        let def = game
            .world
            .resource::<AbilityDb>()
            .get("hallucination")
            .expect("hallucination ships")
            .clone();
        if run {
            game.apply_tamper(
                player,
                &def,
                TamperKind::Hallucinating { decoys: 3 },
                3,
                (4, 6),
            );
            assert_eq!(decoy_cells(&game).len(), 3, "three decoys were seated");
            assert!(
                hallucinating(&game, pack[0]),
                "the hostile under the blast must have taken the entry"
            );
        }
        game
    };

    let mut touched = seated(true);
    let mut untouched = seated(false);
    assert_eq!(
        next_draw(&mut touched),
        next_draw(&mut untouched),
        "seating a Hallucination's decoys moved the seeded stream"
    );
}

/// Spec test 8's first half: a companion caught in the party's own
/// Hallucination is unaffected, because the party ignores its own decoys.
#[test]
fn a_companion_in_the_party_s_own_hallucination_carries_no_entry() {
    let mut game = game(9902);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(4, 6)]);
    place_one(&mut game, companion, (5, 6));
    only_routine(&mut game, player, "hallucination");
    assert!(wait_for_turn(&mut game, player));

    assert!(game.tactical_use_routine(0, (4, 6)));

    assert!(
        hallucinating(&game, hostile),
        "fixture: the hostile beside it must have taken the entry"
    );
    assert!(
        game.world.get::<Tampered>(companion).is_none(),
        "a companion in its own side's Hallucination must carry no entry"
    );
    assert_eq!(
        lines_containing(&game, "starts seeing decoys"),
        1,
        "and the take-hold line must not name it"
    );
}

/// **(M)** Spec test 7: a hallucinating hostile's only target is its nearest
/// decoy, so it walks toward that rather than toward the player a twin run
/// closes on.
///
/// Mutation check: dropping the decoy override from `tactical_sides` sends
/// the tampered hostile at the player exactly as its twin — it ends no
/// nearer the decoy and as near the player. Verified and restored.
#[test]
fn a_hallucinating_hostile_walks_toward_its_nearest_decoy() {
    let start = (4, 0);
    let decoy = (0, 0);
    let run = |tampered: bool| {
        let mut game = game(9903);
        let pack = tactical_fight(&mut game, 1, 40);
        let hostile = pack[0];
        let centre = open_ground(&mut game, &pack, &[start]);
        if tampered {
            hallucinate(&mut game, hostile, &[decoy]);
        }
        assert!(wait_for_turn(&mut game, hostile));
        assert!(
            game.tactical_ai_turn_at(0.0),
            "the hostile's turn was not run"
        );
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(hostile)
            .expect("the hostile left the board");
        (at, centre)
    };
    let (tampered_at, player_at) = run(true);
    let (twin_at, _) = run(false);

    assert!(
        reach::distance(tampered_at, decoy) < reach::distance(start, decoy),
        "the hallucinating hostile must close on its decoy: {start:?} -> {tampered_at:?}"
    );
    assert!(
        reach::distance(tampered_at, decoy) < reach::distance(twin_at, decoy),
        "and end nearer it than the untampered twin: {tampered_at:?} vs {twin_at:?}"
    );
    assert!(
        reach::distance(tampered_at, player_at) >= reach::distance(twin_at, player_at),
        "and no nearer the player than the twin: {tampered_at:?} vs {twin_at:?}"
    );
}

/// **(M)** Spec test 7: a hallucinating hostile beside its decoy swings
/// through it — the decoy is gone, nobody lost Integrity, and the turn moved
/// on. A second decoy far off keeps the entry alive, so this is the strike
/// alone.
///
/// Mutation check: removing the `tactical_strike_decoy` call from
/// `swing_at_best_neighbour` leaves the near decoy standing. Verified and
/// restored.
#[test]
fn striking_a_decoy_destroys_it_and_spends_the_turn() {
    let mut game = game(9904);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(0, 0)]);
    hallucinate(&mut game, hostile, &[(1, 0), (8, 8)]);
    assert!(wait_for_turn(&mut game, hostile));
    let player_hp = game.world.get::<Stats>(player).unwrap().hp;
    let hostile_hp = game.world.get::<Stats>(hostile).unwrap().hp;
    game.take_bolts();

    assert!(game.tactical_ai_turn_at(0.0));

    assert_eq!(
        decoy_cells(&game),
        vec![(8, 8)],
        "the near decoy was struck"
    );
    assert_eq!(game.world.get::<Stats>(player).unwrap().hp, player_hp);
    assert_eq!(game.world.get::<Stats>(hostile).unwrap().hp, hostile_hp);
    assert_ne!(
        game.tactical_actor(),
        Some(hostile),
        "the strike must have spent the turn"
    );
    assert_eq!(lines_containing(&game, "swing passes through a decoy"), 1);
    assert!(
        game.take_bolts().iter().any(|b| b.to == (1, 0)),
        "a strike draws the melee feedback at the decoy's cell"
    );
    assert!(
        hallucinating(&game, hostile),
        "a decoy is still standing, so the entry must be too"
    );
}

/// `tactical_strike_decoy`'s refusals, each on its own and each before
/// anything moves — the decoy, the action and the turn.
#[test]
fn every_decoy_strike_refusal_lands_before_anything_moves() {
    let mut no_fight = game(9905);
    assert!(!no_fight.tactical_strike_decoy((0, 0)), "no fight is open");

    let mut game = game(9905);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    open_ground(&mut game, &pack, &[(0, 0)]);
    // A drone swings at two cells, so a sight line has a cell to cross.
    game.world.get_mut::<Creature>(hostile).unwrap().species = "drone".to_string();
    assert_eq!(game.swing_range(hostile), 2, "fixture: a drone reaches two");
    assert!(wait_for_turn(&mut game, hostile));

    let untouched = |game: &Game, decoys: &[(i32, i32)], why: &str| {
        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(
            battle.decoys().iter().map(|d| d.cell).collect::<Vec<_>>(),
            decoys,
            "{why}: a decoy moved"
        );
        assert!(battle.actions_left() > 0, "{why}: the action was spent");
        assert_eq!(battle.actor(), Some(hostile), "{why}: the turn moved on");
    };

    game.world
        .resource_mut::<TacticalBattle>()
        .place_decoy(party_decoy((1, 0)));
    assert!(!game.tactical_strike_decoy((1, 0)), "not hallucinating");
    untouched(&game, &[(1, 0)], "not hallucinating");

    hallucinate(&mut game, hostile, &[]);
    game.world
        .resource_mut::<TacticalBattle>()
        .place_decoy(Decoy {
            owner_hostile: true,
            ..party_decoy((0, 1))
        });
    assert!(!game.tactical_strike_decoy((0, 1)), "its own side's decoy");
    untouched(&game, &[(1, 0), (0, 1)], "its own side's decoy");
    assert!(!game.tactical_strike_decoy((1, 1)), "an empty cell");
    untouched(&game, &[(1, 0), (0, 1)], "an empty cell");

    game.world
        .resource_mut::<TacticalBattle>()
        .place_decoy(party_decoy((3, 0)));
    assert!(!game.tactical_strike_decoy((3, 0)), "beyond swing range");
    untouched(&game, &[(1, 0), (0, 1), (3, 0)], "beyond swing range");

    game.world
        .resource_mut::<TacticalBattle>()
        .place_decoy(party_decoy((0, 2)));
    game.world
        .resource_mut::<TacticalBattle>()
        .board
        .put(0, 1, BattleCell::Cover);
    assert!(!game.tactical_strike_decoy((0, 2)), "behind cover");
    untouched(&game, &[(1, 0), (0, 1), (3, 0), (0, 2)], "behind cover");

    game.world.resource_mut::<TacticalBattle>().spend_action();
    assert!(!game.tactical_strike_decoy((1, 0)), "already acted");
    let battle = game.world.resource::<TacticalBattle>();
    assert_eq!(battle.decoys().len(), 4, "already acted: a decoy moved");
    assert_eq!(
        battle.actor(),
        Some(hostile),
        "already acted: the turn moved on"
    );
}

/// Spec test 7's routine half: a routine run by a hallucinating body passes
/// through every decoy in its shape and still lands on the real bodies
/// there.
#[test]
fn a_routine_over_a_decoy_destroys_it_and_still_lands_on_real_bodies() {
    let mut game = game(9906);
    let pack = tactical_fight(&mut game, 2, 40);
    let (healer, wounded) = (pack[0], pack[1]);
    open_ground(&mut game, &pack, &[(1, 1), (2, 1)]);
    game.world.get_mut::<Stats>(wounded).unwrap().hp = 10;
    hallucinate(&mut game, healer, &[(1, 2), (0, 1)]);
    let def = game
        .world
        .resource::<AbilityDb>()
        .get("mirror_restore")
        .cloned()
        .expect("mirror_restore ships");
    assert!(wait_for_turn(&mut game, healer));

    game.run_tactical_routine(healer, &def, (1, 1), ENEMY_ROUTINE_MIN_COOLDOWN);

    assert!(
        decoy_cells(&game).is_empty(),
        "both decoys stood in the patch"
    );
    assert!(
        game.world.get::<Stats>(wounded).unwrap().hp > 10,
        "the real body in the patch must still have been mended"
    );
    assert_eq!(
        lines_containing(&game, &format!("{} passes through a decoy", def.name)),
        1,
        "one line per routine, however many decoys it passed through"
    );
}

/// `best_aim` for a hallucinating body scores its decoys, not the party it
/// cannot see: with the player in range too, the aim goes where the decoys
/// are.
#[test]
fn a_hallucinating_hostile_aims_its_routine_at_its_decoys() {
    let mut game = game(9907);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    open_ground(&mut game, &pack, &[(0, 0)]);
    only_routine(&mut game, hostile, "throttle");
    hallucinate(&mut game, hostile, &[(3, 0), (3, 1)]);
    assert!(wait_for_turn(&mut game, hostile));

    assert!(game.tactical_ai_turn_at(0.0));

    assert!(
        game.world
            .get::<AbilityCooldowns>(hostile)
            .is_some_and(|c| c.0.contains_key("throttle")),
        "fixture: the hostile must have run its routine"
    );
    assert!(
        decoy_cells(&game).is_empty(),
        "the aim must have covered both decoys rather than the player"
    );
}

/// **(M)** Spec test 7's last clause: the strike that takes the last decoy
/// ends the entry there and then, two turns before `remaining` would have.
///
/// Mutation check: removing the `settle_decoys` call from `hand_on_turn`
/// leaves the entry live at `remaining: 2`. Verified and restored — with the
/// hostile seated first, since a wrap's upkeep reap settles as well and hid
/// the mutation when it was last in the order.
#[test]
fn with_no_decoys_left_the_entry_goes_at_once() {
    let mut game = game(9908);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(0, 0)]);
    hallucinate(&mut game, hostile, &[(1, 0)]);
    // Seated first, so its hand-on does not wrap the round: the upkeep's reap
    // settles decoys too, and would otherwise answer for the hand-on's own.
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![hostile, player]);

    assert!(game.tactical_ai_turn_at(0.0));

    assert!(
        decoy_cells(&game).is_empty(),
        "fixture: the decoy was struck"
    );
    assert!(
        !hallucinating(&game, hostile),
        "with no decoy left, the entry must be gone before its duration ran out"
    );
    assert_eq!(lines_containing(&game, "sees clearly again"), 1);
}

/// **(M)** Spec test 8: a party body's walk and swing never see the party's
/// own decoys, even one nearer than the hostile it is fighting. (Its aim
/// cannot be reached at all — `run_tactical_beat` offers a routine to a
/// `Hostile` alone.)
///
/// Mutation check: making `sees_decoy` answer `true` for every body and
/// every decoy hands the companion its own decoy as its target — it stands
/// beside it, its strike is refused, and the hostile is never swung at.
/// Verified and restored.
#[test]
fn the_party_s_walk_swing_and_aim_ignore_its_own_decoys() {
    let mut game = game(9909);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    let pack = tactical_fight(&mut game, 1, 400);
    let hostile = pack[0];
    open_ground(&mut game, &pack, &[(0, 2)]);
    place_one(&mut game, companion, (2, 2));
    // The hostile is hallucinating too, or `settle_decoys` would drop a decoy
    // nobody sees at the first hand-on and the companion would have nothing
    // to ignore.
    hallucinate(&mut game, hostile, &[(2, 3)]);
    assert!(wait_for_turn(&mut game, companion));
    game.take_bolts();

    let mut acted = false;
    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            acted = true;
            break;
        }
    }
    assert!(acted, "the companion's turn never finished");

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(companion)
        .expect("the companion left the board");
    assert_eq!(
        reach::distance(at, (0, 2)),
        1,
        "the companion must have walked to the hostile: {at:?}"
    );
    assert!(
        game.take_bolts().iter().any(|b| b.to == (0, 2)),
        "the companion must have swung at the hostile"
    );
    assert_eq!(
        decoy_cells(&game),
        vec![(2, 3)],
        "its own decoy is untouched"
    );
}

/// Decoys are in no list `movement_field`, `line_of_sight` or `recipients`
/// reads: a body walks onto one, a swing crosses one, and a shape over one
/// lands on nobody.
#[test]
fn decoys_block_neither_movement_nor_sight() {
    let mut game = game(9910);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(4, 2)]);
    game.world.get_mut::<Creature>(hostile).unwrap().species = "drone".to_string();
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.place_decoy(party_decoy((4, 3)));
        battle.place_decoy(Decoy {
            owner_hostile: true,
            ..party_decoy((5, 4))
        });
    }

    assert!(wait_for_turn(&mut game, player));
    {
        let allowance = game.movement_allowance(player);
        let battle = game.world.resource::<TacticalBattle>();
        let field = reach::movement_field(battle, player, allowance);
        assert!(field.contains_key(&(5, 4)), "a decoy's cell is walkable");
        assert!(field.contains_key(&(6, 4)), "and so is the cell past it");
        assert!(
            reach::recipients(
                battle,
                player,
                (5, 4),
                crate::abilities::AbilityShape::Single
            )
            .is_empty(),
            "a shape over a decoy lands on nobody"
        );
    }
    assert_eq!(
        game.tactical_step((1, 0)),
        crate::tactical::turn::StepOutcome::Moved,
        "a body steps onto a decoy's cell"
    );
    place_one(&mut game, player, (4, 4));

    assert!(wait_for_turn(&mut game, hostile));
    assert!(
        game.tactical_attack(player),
        "a swing crosses the decoy between the drone and the player"
    );
}

/// Spec test 12's other half: decoys go the moment the last body that could
/// see them dies — here to a Bleed in the round's upkeep, which no hand-on
/// follows — and the fight ending takes the list with it.
#[test]
fn decoys_are_gone_when_the_fight_ends() {
    let mut game = game(9911);
    let pack = tactical_fight(&mut game, 2, 1);
    let (bleeding, other) = (pack[0], pack[1]);
    let player = game.player_entity();
    open_ground(&mut game, &pack, &[(0, 0), (8, 0)]);
    hallucinate(&mut game, bleeding, &[(1, 0)]);
    game.world
        .get_mut::<StatusEffects>(bleeding)
        .unwrap()
        .active = Some(ActiveStatus {
        kind: StatusKind::Bleed,
        remaining: 4,
        power: 20,
        landed_this_round: false,
    });

    let round = game.world.resource::<TacticalBattle>().round;
    while game.world.resource::<TacticalBattle>().round == round {
        game.tactical_end_turn();
    }
    assert!(
        game.world.get::<Stats>(bleeding).is_none_or(|s| s.hp <= 0),
        "fixture: the Bleed must have killed the hallucinating hostile"
    );
    assert!(
        decoy_cells(&game).is_empty(),
        "no living body can see the decoy, so it must be gone"
    );

    hallucinate(&mut game, other, &[(7, 0)]);
    assert!(wait_for_turn(&mut game, player));
    place_one(&mut game, other, (5, 4));
    force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_attack(other), "the last hostile must fall");
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight is over and its decoys went with it"
    );
}

/// `marooned`, with its hostile carrying `kinds` — the forecast tests' own
/// fixture, since what is under test is `tactical_forecast`'s reading of
/// `Tampered` rather than the routines that write it.
fn marooned_with(kinds: &[TamperKind]) -> (Game, Entity) {
    let (mut game, wild) = marooned();
    let mut tampered = Tampered::default();
    for &kind in kinds {
        tampered.apply(kind, 3, false);
    }
    game.world.entity_mut(wild).insert(tampered);
    (game, wild)
}

fn stunned(game: &Game, body: Entity) -> bool {
    game.world
        .get::<StatusEffects>(body)
        .and_then(|s| s.active.as_ref())
        .is_some_and(|a| a.kind == StatusKind::Stun)
}

/// Stands `body` on a random cell that will take it. Bounded, so a board
/// with no room fails rather than hangs.
fn scatter(game: &mut Game, body: Entity, rng: &mut rand::rngs::StdRng) {
    use rand::RngExt;
    let side = game.world.resource::<TacticalBattle>().board.side;
    for _ in 0..10_000 {
        let cell = (rng.random_range(0..side), rng.random_range(0..side));
        if game
            .world
            .resource_mut::<TacticalBattle>()
            .move_to(body, cell)
        {
            return;
        }
    }
    panic!("no cell on the board would take a body");
}

/// **(M)** Spec test 3: a Cold Sampled, Profiled hostile's turn walks to the
/// forecast's destination and lands on the forecast's target — swept over
/// seeded boards, so the claim is about the planner and not one layout.
///
/// Every body but the subject ends its turns without acting, so the board
/// the forecast was read off is the board the turn runs on. A third of the
/// hostiles carry only `deadlock`, a Single Stun, so a routine's aim is
/// observable as the Stun on whoever stands there; a swing's is the first
/// `BoltCue`'s `to`.
///
/// Mutation check: making `tactical_forecast` pick its destination with
/// `sample_scored` at 0.5 over a local `StdRng::seed_from_u64(0)` (`StdRng`
/// is not `Clone`, so not a copy of `GameRng`) instead of `argmax_scored`
/// makes the destinations disagree. Verified and restored —
/// see the commit body.
#[test]
fn a_cold_profiled_hostile_does_what_its_forecast_said() {
    use rand::{RngExt, SeedableRng};

    const BOARDS: u64 = 24;
    let (mut compared, mut skipped, mut unreachable) = (0, 0, 0);
    let (mut swings, mut routines, mut walked) = (0, 0, 0);
    for i in 0..BOARDS {
        let mut rng = rand::rngs::StdRng::seed_from_u64(i);
        let mut game = game(9950 + i as u32);
        let player = game.player_entity();
        if let Some(mut stats) = game.world.get_mut::<Stats>(player) {
            stats.hp = 10_000;
            stats.max_hp = 10_000;
        }
        let companion = body(&mut game, &generic_species().id);
        // `body` spawns no `StatusEffects`, and a Stun has nowhere to land
        // without one — the companion would read as a routine that missed.
        game.world
            .entity_mut(companion)
            .insert(StatusEffects::default());
        game.world.resource_mut::<Party>().0.push(companion);
        let count = rng.random_range(1..=3);
        let pack = tactical_fight(&mut game, count, 1_000);
        for &hostile in &pack {
            if rng.random_range(0..3) == 0 {
                only_routine(&mut game, hostile, "deadlock");
            }
        }
        for body in [player, companion].into_iter().chain(pack.iter().copied()) {
            scatter(&mut game, body, &mut rng);
        }
        let subject = pack[rng.random_range(0..count)];
        let mut tampered = Tampered::default();
        tampered.apply(TamperKind::Temperature(0.0), 3, false);
        tampered.apply(TamperKind::Profiled, 3, false);
        game.world.entity_mut(subject).insert(tampered);

        assert!(wait_for_turn(&mut game, subject), "board {i}: no turn");
        game.take_bolts();
        let start = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(subject)
            .expect("the subject stands on the board");
        let Some(forecast) = game.tactical_forecast(subject) else {
            skipped += 1;
            continue;
        };
        let bodies: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .collect();

        assert!(game.tactical_ai_turn(), "board {i}: the turn was not run");
        let end = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(subject)
            .expect("the subject left the board");
        assert_eq!(
            Some(end),
            Some(forecast.walk.last().copied().unwrap_or(start)),
            "board {i}: the turn ended somewhere the forecast did not walk to"
        );

        match &forecast.action {
            ForecastAction::Swing => {
                swings += 1;
                let bolts = game.take_bolts();
                // **The subject's own streak, not the first in the queue.**
                // A walk that leaves a hostile's reach provokes, and a
                // reaction's streak is pushed from the *reactor* before the
                // swing this is asking about — so the cue to read is the
                // first one leaving the cell the subject ended on.
                assert_eq!(
                    bolts.iter().find(|b| b.from == end).map(|b| b.to),
                    forecast.target,
                    "board {i}: the swing landed somewhere the forecast did not name"
                );
            }
            ForecastAction::Routine(id) => {
                routines += 1;
                assert_eq!(id, "deadlock", "board {i}");
                let battle = game.world.resource::<TacticalBattle>();
                let hit: Vec<(i32, i32)> = bodies
                    .iter()
                    .filter(|&&e| stunned(&game, e))
                    .filter_map(|&e| battle.cell_of(e))
                    .collect();
                assert_eq!(
                    hit,
                    forecast.target.into_iter().collect::<Vec<_>>(),
                    "board {i}: the routine stalled someone the forecast did not name"
                );
            }
        }
        if forecast.target.is_none() {
            unreachable += 1;
        }
        if !forecast.walk.is_empty() {
            walked += 1;
        }
        compared += 1;
    }
    println!(
        "forecast sweep: {compared} compared ({unreachable} with nothing in reach, \
         {walked} walked), {skipped} skipped; {swings} swings, {routines} routines"
    );
    assert!(
        compared >= 18,
        "only {compared} of {BOARDS} boards were compared, so the sweep proves nothing"
    );
    assert!(
        walked > 0 && compared - unreachable > 0,
        "the sweep must compare a walk and a target: {walked} walked, \
         {unreachable} of {compared} with nothing in reach"
    );
    assert!(
        swings > 0 && routines > 0,
        "the sweep must compare both actions: {swings} swings, {routines} routines"
    );
}

/// Spec test 4: above temperature zero the walk is a draw, so the forecast
/// names the action and nothing else. The Cold twin's non-empty walk is what
/// makes the empty one here the temperature gate rather than a hostile that
/// had nowhere to go.
#[test]
fn above_zero_the_forecast_names_the_action_and_nothing_else() {
    let (warm, wild) = marooned_with(&[TamperKind::Profiled]);
    let forecast = warm
        .tactical_forecast(wild)
        .expect("a profiled hostile has a forecast");
    assert_eq!(forecast.action, ForecastAction::Swing);
    assert!(forecast.walk.is_empty(), "{:?}", forecast.walk);
    assert_eq!(forecast.target, None);

    let (cold, wild) = marooned_with(&[TamperKind::Temperature(0.0), TamperKind::Profiled]);
    let forecast = cold
        .tactical_forecast(wild)
        .expect("a profiled hostile has a forecast");
    assert!(
        !forecast.walk.is_empty(),
        "fixture: the marooned hostile must walk at temperature zero"
    );
}

/// A forecast is what `Profiled` buys: without it, nothing — Cold alone
/// included, and a profiled companion too, which the AI does not drive.
#[test]
fn an_unprofiled_hostile_has_no_forecast() {
    let (cold, wild) = marooned_with(&[TamperKind::Temperature(0.0)]);
    assert_eq!(cold.tactical_forecast(wild), None);
    let (virgin, wild) = marooned();
    assert_eq!(virgin.tactical_forecast(wild), None);

    let mut game = game(9960);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    tactical_fight(&mut game, 1, 40);
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Profiled, 3, false);
    game.world.entity_mut(companion).insert(tampered);
    assert_eq!(game.tactical_forecast(companion), None);
}

/// Decision 4: the forecast is exact at the moment a turn begins, so it is
/// offered before the first beat and withdrawn once the walk is planned.
#[test]
fn a_hostile_mid_turn_has_no_forecast() {
    let (mut game, wild) = marooned_with(&[TamperKind::Temperature(0.0), TamperKind::Profiled]);
    assert!(
        game.tactical_forecast(wild).is_some(),
        "fixture: before its first beat the hostile has a forecast"
    );
    assert_eq!(game.tactical_ai_beat(), AiBeat::Stepped);
    assert_eq!(
        game.world.resource::<TacticalBattle>().actor(),
        Some(wild),
        "fixture: the hostile is still mid-turn"
    );
    assert_eq!(game.tactical_forecast(wild), None);
}

/// A hostile carrying Cold, Profiled and Hallucinating, with `decoys`
/// placed on an open board and the turn handed to it.
fn cold_profiled_hallucinating(seed: u32, at: (i32, i32), decoys: &[(i32, i32)]) -> (Game, Entity) {
    let mut game = game(seed);
    let pack = tactical_fight(&mut game, 1, 1_000);
    let hostile = pack[0];
    open_ground(&mut game, &pack, &[at]);
    hallucinate(&mut game, hostile, decoys);
    let mut tampered = game
        .world
        .get_mut::<Tampered>(hostile)
        .expect("hallucinating");
    tampered.apply(TamperKind::Temperature(0.0), 3, false);
    tampered.apply(TamperKind::Profiled, 3, false);
    assert!(wait_for_turn(&mut game, hostile));
    game.take_bolts();
    (game, hostile)
}

/// **(M)** A hallucinating hostile's target is the decoy nearest where its
/// walk *ends*, and the forecast has to ask from there too. From (0, 0) the
/// nearer decoy is (3, 0), so that is what the walk closes on; the walk ends
/// at (5, 2), where (6, 3) is nearer, and that is what the turn strikes.
///
/// Mutation check: handing `chosen_target` the sides read from the start
/// cell rather than `tactical_sides_from(destination)` makes the forecast
/// name (3, 0). Verified and restored — see the commit body.
#[test]
fn a_hallucinating_forecast_names_the_decoy_nearest_where_the_walk_ends() {
    let near_start = (3, 0);
    let near_end = (6, 3);
    let (mut game, hostile) = cold_profiled_hallucinating(9970, (0, 0), &[near_start, near_end]);

    let forecast = game
        .tactical_forecast(hostile)
        .expect("a profiled hostile has a forecast");
    let destination = *forecast
        .walk
        .last()
        .expect("fixture: the hostile must walk");
    assert!(
        reach::distance((0, 0), near_start) < reach::distance((0, 0), near_end)
            && reach::distance(destination, near_end) < reach::distance(destination, near_start),
        "fixture: the nearest decoy must change along the walk to {destination:?}"
    );
    assert_eq!(forecast.target, Some(near_end));

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");
    assert_eq!(
        game.take_bolts().first().map(|b| b.to),
        forecast.target,
        "the turn must strike the decoy the forecast named"
    );
    assert_eq!(decoy_cells(&game), vec![near_start]);
}

/// A decoy the hostile cannot reach this turn is nothing to act on, so the
/// forecast names no target — rather than one the strike door then refuses.
///
/// Mutation check: dropping the `swing_reaches` gate from `best_swing`'s
/// decoy arm makes the forecast name (8, 8). Verified and restored.
#[test]
fn a_decoy_out_of_reach_is_no_forecast_target() {
    let far = (8, 8);
    let (mut game, hostile) = cold_profiled_hallucinating(9971, (0, 0), &[far]);
    {
        // Walled in, so the walk cannot close on anything.
        let board = &mut game.world.resource_mut::<TacticalBattle>().board;
        for (x, y) in [(1, 0), (0, 1), (1, 1)] {
            board.put(x, y, BattleCell::Blocked);
        }
    }

    let forecast = game
        .tactical_forecast(hostile)
        .expect("a profiled hostile has a forecast");
    assert!(forecast.walk.is_empty(), "{:?}", forecast.walk);
    assert_eq!(forecast.target, None);

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");
    assert!(
        game.take_bolts().is_empty(),
        "nothing was in reach to strike"
    );
    assert_eq!(decoy_cells(&game), vec![far]);
}

// ─────────────────────────────────────────────────────────────────────────
// Task 8: the view carries tags, forecasts, hijacks and decoys.
// ─────────────────────────────────────────────────────────────────────────

/// Every `Tampered` slot on a row is a `TamperTag`, in `TamperSlot` order —
/// Temperature, Profiled, Injected, Hallucinating — and Temperature itself
/// splits into `Hot`/`Cold` at the tuning constant, `<=` reading Cold.
#[test]
fn the_turn_row_carries_each_tamper_as_a_tag() {
    let (mut cold, wild) = marooned_with(&[
        TamperKind::Temperature(0.0),
        TamperKind::Profiled,
        TamperKind::Injected,
        TamperKind::Hallucinating { decoys: 3 },
    ]);
    let view = cold.tactical_view().expect("a fight is open");
    let row = view
        .order
        .iter()
        .find(|r| r.entity == wild)
        .expect("the tampered hostile has a rung in the strip");
    assert_eq!(
        row.tags,
        vec![
            TamperTag::Cold,
            TamperTag::Profiled,
            TamperTag::Injected,
            TamperTag::Hallucinating,
        ]
    );

    let (mut hot, wild) = marooned_with(&[TamperKind::Temperature(TACTICAL_AI_TEMPERATURE + 0.1)]);
    let view = hot.tactical_view().expect("a fight is open");
    let row = view
        .order
        .iter()
        .find(|r| r.entity == wild)
        .expect("the tampered hostile has a rung in the strip");
    assert_eq!(
        row.tags,
        vec![TamperTag::Hot],
        "above the constant reads Hot"
    );

    let (mut at_constant, wild) =
        marooned_with(&[TamperKind::Temperature(TACTICAL_AI_TEMPERATURE)]);
    let view = at_constant.tactical_view().expect("a fight is open");
    let row = view
        .order
        .iter()
        .find(|r| r.entity == wild)
        .expect("the tampered hostile has a rung in the strip");
    assert_eq!(
        row.tags,
        vec![TamperTag::Cold],
        "the constant itself must read Cold, the <= boundary"
    );
}

/// A profiled hostile's row carries the same forecast `Game::tactical_forecast`
/// answers directly — the view is a call into the planner, never a second
/// derivation of it.
#[test]
fn a_profiled_hostile_s_row_carries_its_forecast() {
    let (mut game, wild) = marooned_with(&[TamperKind::Temperature(0.0), TamperKind::Profiled]);
    let forecast = game
        .tactical_forecast(wild)
        .expect("fixture: a cold, profiled hostile has a forecast before its turn");
    let expected_action = match &forecast.action {
        ForecastAction::Swing => "swing".to_string(),
        ForecastAction::Routine(id) => game.ability_display_name(id),
    };

    let view = game.tactical_view().expect("a fight is open");
    let row = view
        .order
        .iter()
        .find(|r| r.entity == wild)
        .expect("the profiled hostile has a rung in the strip");
    let carried = row
        .forecast
        .as_ref()
        .expect("a profiled hostile's row must carry a forecast");
    assert_eq!(carried.action, expected_action);
    assert_eq!(carried.walk, forecast.walk);
    assert_eq!(carried.target, forecast.target);
}

/// A companion taken over by a Temperature or Injected entry says so on its
/// own row, and an ordinary companion's row does not.
#[test]
fn a_hijacked_companion_s_row_says_so() {
    let mut game = game(9803);
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    tactical_fight(&mut game, 1, 400);

    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Temperature(0.0), 1, false);
    game.world.entity_mut(companion).insert(tampered);

    let player = game.player_entity();
    let view = game.tactical_view().expect("a fight is open");
    let companion_row = view
        .order
        .iter()
        .find(|r| r.entity == companion)
        .expect("the companion has a rung in the strip");
    assert!(
        companion_row.taken_over,
        "a companion under a live Temperature entry must read as taken over"
    );
    let player_row = view
        .order
        .iter()
        .find(|r| r.entity == player)
        .expect("the player has a rung in the strip");
    assert!(
        !player_row.taken_over,
        "the player is never taken over, tampered or not"
    );
}

/// Every decoy on the board reaches the view, both sides' alike.
#[test]
fn the_view_carries_the_decoys() {
    let (mut game, wild) = marooned();
    hallucinate(&mut game, wild, &[(1, 1), (2, 2)]);

    let raw: Vec<DecoyView> = game
        .world
        .resource::<TacticalBattle>()
        .decoys()
        .iter()
        .map(|d| DecoyView {
            cell: d.cell,
            glyph: d.glyph,
            color: d.color,
            of_player: d.of_player,
        })
        .collect();
    assert_eq!(raw.len(), 2, "fixture: two decoys were placed");

    let view = game.tactical_view().expect("a fight is open");
    assert_eq!(view.decoys, raw);
}

/// Content: the routine tree is where the feature actually reaches the
/// player now, not `model_inspection` directly — todo #101 moved every one
/// of the eleven abilities that node used to grant into its own synthesised
/// node, gated at `research_zone: 3` exactly as the node was. Spec 15's own
/// assertion still holds, just over the whole tree rather than one node:
/// every `TamperSlot` has a route in, and `model_inspection` keeps both gear
/// recipes.
#[test]
fn the_routine_tree_teaches_every_tamper_kind_and_model_inspection_keeps_both_gear_recipes() {
    let game = game(9900);
    let research = game.world.resource::<crate::research::ResearchDb>();
    let abilities = game.world.resource::<AbilityDb>();

    let taught_slots: std::collections::BTreeSet<TamperSlot> = research
        .all()
        .filter_map(|node| node.teaches.as_deref())
        .filter_map(|id| abilities.get(id))
        .filter_map(|def| match &def.effect {
            AbilityEffect::Tamper { kind, .. } => Some(kind.slot()),
            _ => None,
        })
        .collect();
    let every_slot: std::collections::BTreeSet<TamperSlot> = [
        TamperSlot::Temperature,
        TamperSlot::Profiled,
        TamperSlot::Injected,
        TamperSlot::Hallucinating,
    ]
    .into_iter()
    .collect();
    assert_eq!(
        taught_slots, every_slot,
        "the routine tree must teach a routine into every Tampered slot"
    );

    let node = research
        .get("model_inspection")
        .expect("model_inspection ships");
    let recipe_results: Vec<&ItemId> = node.unlocks_recipes.iter().map(|r| &r.result).collect();
    for item in ["adversarial_patch", "attention_head"] {
        assert!(
            recipe_results.iter().any(|id| id.as_str() == item),
            "model_inspection's recipes must produce {item}"
        );
    }
}
