//! `AbilityEffect::Tamper`: the schema, the load checks, and the effect
//! hidden everywhere it cannot run yet. Nothing seats one — that is a later
//! task — so what is tested here is the shape of the five shipped routines
//! and every place that must never offer or choose one outside a tactical
//! battle.

use super::support::{
    battle_with_a_pack_of, insert_battle, spawn_wild_without_routine, test_assets_dir,
};
use super::tactical::{tactical_fight, wait_for_turn};
use crate::Game;
use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget, TamperKind};
use crate::battle::BattleAction;
use crate::components::{Position, Routines, Stats};
use crate::resources::{DifficultyMode, Sorties};

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
