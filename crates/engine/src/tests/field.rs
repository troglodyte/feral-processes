//! Casting a field routine: `Game::field_routines` and
//! `Game::cast_field_routine`.

use super::support::*;
use crate::components::{FieldBuff, FieldBuffKind, Needs, Routines};
use crate::resources::Party;
use crate::tuning::AFFINITY_NEUTRAL;
use crate::*;

fn game_with_field_ability() -> Game {
    let dir = modded_assets_dir(
        "field_cast",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_regen.ron", FIELD_ONLY_ABILITY)],
    );
    Game::new(9101, DifficultyMode::Forgiving, &dir).unwrap()
}

fn player_hunger(game: &Game) -> f32 {
    game.world
        .get::<Needs>(game.player_entity())
        .unwrap()
        .hunger
}

#[test]
fn casting_arms_the_buff_and_deducts_power() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));

    let routines = game.field_routines();
    assert_eq!(routines.len(), 1);
    assert_eq!(routines[0].power_cost, 5.0);
    assert!(routines[0].needs_ally_target);

    let before = player_hunger(&game);
    game.cast_field_routine(0, Some(player))
        .expect("casting a field routine you can afford should succeed");

    // A successful cast ticks the clock (see `cast_field_routine`'s doc), so
    // the ordinary per-tick Power decay lands on top of the routine's own
    // cost. `systems::decay_needs` is the one place that decay formula
    // lives — read through it rather than restating its constant here.
    let (expected_hunger, _) = crate::systems::decay_needs(before - 5.0, 0.0, 1.0);
    assert_eq!(player_hunger(&game), expected_hunger);
    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(active.len(), 1);
    let buff = &active[0];
    assert_eq!(buff.kind, FieldBuffKind::Regen);
    assert_eq!(buff.name, "Test Field Regen");
    assert_eq!(buff.power, abilities::scaled_power(2, 1, AFFINITY_NEUTRAL));
    // duration: 20, aged by the one tick the successful cast itself spends.
    assert_eq!(buff.remaining, 19);
    assert_eq!(buff.source, BuffSource::Routine);
}

#[test]
fn a_successful_cast_ticks_the_clock_and_a_refused_one_does_not() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let start = game.current_tick();

    game.world.get_mut::<Needs>(player).unwrap().hunger = 4.0;
    let refused = game.cast_field_routine(0, Some(player));
    assert!(refused.is_err(), "4.0 Power can't cover a 5.0 cost");
    assert_eq!(
        game.current_tick(),
        start,
        "a refused cast spends nothing, so it must cost no time"
    );

    game.world.get_mut::<Needs>(player).unwrap().hunger = 100.0;
    game.cast_field_routine(0, Some(player))
        .expect("100.0 Power covers a 5.0 cost");
    assert_eq!(
        game.current_tick(),
        start + 1,
        "a successful cast is a turn, exactly like use_item spending one on the same buff"
    );
}

#[test]
fn insufficient_power_returns_err_and_leaves_state_untouched() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game.world.get_mut::<Needs>(player).unwrap().hunger = 4.0;

    let result = game.cast_field_routine(0, Some(player));

    assert_eq!(
        result,
        Err("Not enough Power to run Test Field Regen.".to_string())
    );
    assert_eq!(
        player_hunger(&game),
        4.0,
        "a refused cast must not spend Power"
    );
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .is_none_or(|b| b.active.is_empty()),
        "a refused cast must not arm a buff"
    );
}

#[test]
fn casting_during_a_battle_is_refused() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    let before = player_hunger(&game);

    let result = game.cast_field_routine(0, Some(player));

    assert!(result.is_err(), "casting mid-battle must be refused");
    assert_eq!(player_hunger(&game), before);
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .is_none_or(|b| b.active.is_empty())
    );
}

#[test]
fn casting_underground_succeeds() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let pos = *game.world.get::<Position>(player).unwrap();
    game.enter_dungeon(pos.x, pos.y);

    let result = game.cast_field_routine(0, Some(player));

    assert!(
        result.is_ok(),
        "casting is not gated on require_surface and must work underground: {result:?}"
    );
    assert!(game.world.get::<FieldBuff>(player).is_some());
}

#[test]
fn a_higher_level_holder_casts_a_larger_magnitude() {
    let mut game = game_with_field_ability();
    let low = spawn_tamed(&mut game, 10, 3);
    let high = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, high, 20);
    game.world
        .entity_mut(low)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game.world
        .entity_mut(high)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game.world
        .get_mut::<Needs>(game.player_entity())
        .unwrap()
        .hunger = 100.0;

    let routines = game.field_routines();
    let low_index = routines
        .iter()
        .position(|r| r.holder == low)
        .expect("the level-1 holder's routine is listed");
    game.cast_field_routine(low_index, Some(low)).unwrap();

    game.world
        .get_mut::<Needs>(game.player_entity())
        .unwrap()
        .hunger = 100.0;
    let routines = game.field_routines();
    let high_index = routines
        .iter()
        .position(|r| r.holder == high)
        .expect("the level-20 holder's routine is listed");
    game.cast_field_routine(high_index, Some(high)).unwrap();

    let low_power = game.world.get::<FieldBuff>(low).unwrap().active[0].power;
    let high_power = game.world.get::<FieldBuff>(high).unwrap().active[0].power;
    assert_eq!(low_power, abilities::scaled_power(2, 1, AFFINITY_NEUTRAL));
    assert_eq!(high_power, abilities::scaled_power(2, 20, AFFINITY_NEUTRAL));
    assert!(
        high_power > low_power,
        "a level-20 holder's cast should outscale a level-1 holder's: \
         {high_power} vs {low_power}"
    );
}

#[test]
fn a_run_scoped_kind_lands_on_the_player_even_when_cast_off_a_companion() {
    let dir = modded_assets_dir(
        "field_cast_run",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_trickle.ron", FIELD_ONLY_RUN_ABILITY)],
    );
    let mut game = Game::new(9102, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["test_field_trickle".to_string()]));

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == companion)
        .expect("the companion's routine is listed");
    assert!(
        !routines[index].needs_ally_target,
        "a Run-scoped routine needs no target picker"
    );

    // `target: None` — a Run-scoped kind must ignore it entirely rather
    // than requiring one.
    game.cast_field_routine(index, None)
        .expect("a Run-scoped cast needs no target");

    assert!(
        game.world.get::<FieldBuff>(companion).is_none(),
        "a Run-scoped buff must not land on the holder that cast it"
    );
    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].kind, FieldBuffKind::Trickle);
}

#[test]
fn whole_party_arms_every_living_member_and_skips_the_dead() {
    let dir = modded_assets_dir(
        "field_cast_party",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_def.ron", FIELD_ONLY_PARTY_ABILITY)],
    );
    let mut game = Game::new(9103, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let alive = spawn_tamed(&mut game, 10, 3);
    let dead = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Stats>(dead).unwrap().hp = 0;
    game.world.resource_mut::<Party>().0.extend([alive, dead]);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_def".to_string()]));

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == player)
        .expect("the player's routine is listed");
    assert!(!routines[index].needs_ally_target);

    game.cast_field_routine(index, None)
        .expect("a WholeParty cast needs no target");

    assert_eq!(game.world.get::<FieldBuff>(player).unwrap().active.len(), 1);
    assert_eq!(game.world.get::<FieldBuff>(alive).unwrap().active.len(), 1);
    assert!(
        game.world.get::<FieldBuff>(dead).is_none(),
        "a dead party member must not be armed"
    );
}

#[test]
fn field_routines_lists_across_holders_and_excludes_non_field() {
    let dir = modded_assets_dir(
        "field_cast_listing",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_regen.ron", FIELD_ONLY_ABILITY)],
    );
    let mut game = Game::new(9104, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(player).insert(Routines(vec![
        "test_field_regen".to_string(),
        crate::abilities::DECOMPILE_ABILITY_ID.to_string(),
    ]));
    game.world.entity_mut(companion).insert(Routines(vec![
        "test_field_regen".to_string(),
        crate::abilities::FALLBACK_ABILITY_ID.to_string(),
    ]));

    let routines = game.field_routines();

    assert_eq!(
        routines.len(),
        2,
        "decompile and the fallback are not FieldBuff abilities and must be excluded: \
         found {} rows",
        routines.len(),
    );
    assert!(
        routines
            .iter()
            .any(|r| r.holder == player && r.holder_label == "You")
    );
    assert!(
        routines
            .iter()
            .any(|r| r.holder == companion && r.holder_label != "You")
    );
    assert!(routines.iter().all(|r| r.ability == "test_field_regen"));
}
