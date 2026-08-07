//! Casting a field routine: `Game::field_routines` and
//! `Game::cast_field_routine`.

use super::support::*;
use crate::components::{FieldBuff, FieldBuffKind, Needs, Perks, Routines};
use crate::resources::Party;
use crate::tuning::{AFFINITY_MAX, AFFINITY_NEUTRAL, CREATURE_MAX_LEVEL};
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
    assert_eq!(routines[0].cost, "5 PWR");
    assert_eq!(routines[0].second_pick, FieldCastPick::Ally);

    let before = player_hunger(&game);
    game.cast_field_routine(0, FieldCastTarget::Ally(player))
        .expect("casting a field routine you can afford should succeed");

    // A successful cast ticks the clock (see `cast_field_routine`'s doc), so
    // the ordinary per-tick Power decay lands on top of the routine's own
    // cost. `systems::tick_needs` is the one place that decay formula
    // lives — read through it rather than restating its constant here.
    let (expected_hunger, _) = crate::systems::tick_needs(before - 5.0, 0.0, 1.0);
    assert_eq!(player_hunger(&game), expected_hunger);
    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(active.len(), 1);
    let buff = &active[0];
    assert_eq!(buff.kind, FieldBuffKind::Regen);
    assert_eq!(buff.name, "Test Field Regen");
    assert_eq!(
        buff.power,
        abilities::scaled_stat_power(2, 1, AFFINITY_NEUTRAL)
    );
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
    let refused = game.cast_field_routine(0, FieldCastTarget::Ally(player));
    assert!(refused.is_err(), "4.0 Power can't cover a 5.0 cost");
    assert_eq!(
        game.current_tick(),
        start,
        "a refused cast spends nothing, so it must cost no time"
    );

    game.world.get_mut::<Needs>(player).unwrap().hunger = 100.0;
    game.cast_field_routine(0, FieldCastTarget::Ally(player))
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

    let result = game.cast_field_routine(0, FieldCastTarget::Ally(player));

    assert_eq!(
        result,
        Err("Can't run Test Field Regen — not enough PWR.".to_string())
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

    let result = game.cast_field_routine(0, FieldCastTarget::Ally(player));

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
    game.enter_stack(pos.x, pos.y);

    let result = game.cast_field_routine(0, FieldCastTarget::Ally(player));

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
    game.add_companion(low).unwrap();
    game.add_companion(high).unwrap();
    // `CREATURE_MAX_LEVEL` rather than an arbitrary 20: a companion cannot
    // level past it in play, so a fixture that did would be scaling a cast
    // nobody can ever make.
    set_level(&mut game, high, CREATURE_MAX_LEVEL);
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
    game.cast_field_routine(low_index, FieldCastTarget::Ally(low))
        .unwrap();

    game.world
        .get_mut::<Needs>(game.player_entity())
        .unwrap()
        .hunger = 100.0;
    let routines = game.field_routines();
    let high_index = routines
        .iter()
        .position(|r| r.holder == high)
        .expect("the top-level holder's routine is listed");
    game.cast_field_routine(high_index, FieldCastTarget::Ally(high))
        .unwrap();

    let low_power = game.world.get::<FieldBuff>(low).unwrap().active[0].power;
    let high_power = game.world.get::<FieldBuff>(high).unwrap().active[0].power;
    assert_eq!(
        low_power,
        abilities::scaled_stat_power(2, 1, AFFINITY_NEUTRAL)
    );
    assert_eq!(
        high_power,
        abilities::scaled_stat_power(2, CREATURE_MAX_LEVEL, AFFINITY_NEUTRAL)
    );
    assert!(
        high_power > low_power,
        "a top-level holder's cast should outscale a level-1 holder's: \
         {high_power} vs {low_power}"
    );
}

/// The two `FieldBuffKind::scales_with_caster` tests below share this
/// holder shape — level 20 (`ABILITY_SCALE_LEVEL_CAP` is 40, so this
/// is short of the cap but still well past level 1) and `AFFINITY_MAX`
/// worth of the `BuffAffinity` perk, bought directly onto `Perks` rather
/// than through the purchase flow since only the resulting level matters
/// here. Both `Def` and `Mitigation` fall under `AffinityKind::Buff`, so one
/// holder proves both halves of the split against the same multiplier.
fn buff_affinity_maxed_player(game: &mut Game) -> Entity {
    let player = game.player_entity();
    set_level(game, player, 20);
    let levels_to_max = ((AFFINITY_MAX - AFFINITY_NEUTRAL)
        / crate::tuning::AFFINITY_PERK_BONUS_PER_LEVEL)
        .ceil() as usize;
    game.world.entity_mut(player).insert(Perks {
        points: 0,
        unlocked: vec![Perk::BuffAffinity; levels_to_max],
    });
    game.world.get_mut::<Needs>(player).unwrap().hunger = 100.0;
    player
}

#[test]
fn a_percentage_kind_is_delivered_at_its_authored_value_regardless_of_level_or_affinity() {
    let dir = modded_assets_dir(
        "field_cast_pct_unscaled",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_mitigation.ron", FIELD_ONLY_MITIGATION_ABILITY)],
    );
    let mut game = Game::new(9105, DifficultyMode::Forgiving, &dir).unwrap();
    let player = buff_affinity_maxed_player(&mut game);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_mitigation".to_string()]));

    game.cast_field_routine(0, FieldCastTarget::None)
        .expect("a WholeParty target needs no picked ally");

    let power = game.world.get::<FieldBuff>(player).unwrap().active[0].power;
    // scaled_stat_power(10, 20, AFFINITY_MAX) would be 40 — if this test passed
    // against that number instead of 10, the split below would be a no-op.
    assert_ne!(abilities::scaled_stat_power(10, 20, AFFINITY_MAX), 10);
    assert_eq!(
        power, 10,
        "a percentage-point kind must land at exactly its authored value"
    );
}

#[test]
fn a_flat_kind_still_scales_for_the_same_high_level_high_affinity_holder() {
    let dir = modded_assets_dir(
        "field_cast_flat_still_scales",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_def.ron", FIELD_ONLY_PARTY_ABILITY)],
    );
    let mut game = Game::new(9106, DifficultyMode::Forgiving, &dir).unwrap();
    let player = buff_affinity_maxed_player(&mut game);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_def".to_string()]));

    game.cast_field_routine(0, FieldCastTarget::None)
        .expect("a WholeParty target needs no picked ally");

    let power = game.world.get::<FieldBuff>(player).unwrap().active[0].power;
    assert_eq!(
        power,
        abilities::scaled_stat_power(4, 20, AFFINITY_MAX),
        "a point-amount kind must still scale for the same holder the percentage test above did not"
    );
    assert_ne!(
        power, 4,
        "if this equals the authored value, the holder in this test isn't actually scaling anything"
    );
}

/// `field_cast_targets` — what the ally picker offers — is narrower than
/// `routine_holders`: only the player and the active `Party`, since those
/// are the only entities `tick_field_buffs` ever walks. An owned program
/// that isn't in the party must not appear, even though it's a perfectly
/// valid `routine_holders` row (installing a routine on it is legitimate).
#[test]
fn field_cast_targets_excludes_an_owned_program_outside_the_party() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    let party_member = spawn_tamed(&mut game, 10, 3);
    game.add_companion(party_member).unwrap();
    let benched = spawn_tamed(&mut game, 10, 3);

    let targets = game.field_cast_targets();

    assert!(targets.iter().any(|t| t.entity == player));
    assert!(targets.iter().any(|t| t.entity == party_member));
    assert!(
        targets.iter().all(|t| t.entity != benched),
        "an owned program outside the active party must not be offered"
    );

    let holders = game.routine_holders();
    assert!(
        holders.iter().any(|h| h.entity == benched),
        "routine_holders (install/uninstall) still lists it — only casting narrows"
    );
}

/// The picker narrowing above is a UX convenience, not the only guard: the
/// engine has to refuse a `OneAlly` cast aimed at an owned-but-benched
/// program too, or a caller that bypasses the picker (a bug, a future UI)
/// could still arm a buff on an entity `tick_field_buffs` never walks — paid
/// for, frozen at full duration, and doing nothing forever.
#[test]
fn casting_on_an_owned_program_outside_the_party_is_refused() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    let benched = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let before = player_hunger(&game);

    let result = game.cast_field_routine(0, FieldCastTarget::Ally(benched));

    assert!(
        result.is_err(),
        "casting on an owned, non-party program must be refused"
    );
    assert_eq!(
        player_hunger(&game),
        before,
        "a refused cast must not spend Power"
    );
    assert!(
        game.world
            .get::<FieldBuff>(benched)
            .is_none_or(|b| b.active.is_empty()),
        "a refused cast must not arm a buff on the rejected target"
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
        routines[index].second_pick == FieldCastPick::None,
        "a Run-scoped routine needs no target picker"
    );

    // `target: None` — a Run-scoped kind must ignore it entirely rather
    // than requiring one.
    game.cast_field_routine(index, FieldCastTarget::None)
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
    assert_eq!(routines[index].second_pick, FieldCastPick::None);

    game.cast_field_routine(index, FieldCastTarget::None)
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

#[test]
fn active_buffs_is_empty_when_nothing_is_running() {
    let mut game = game_with_field_ability();

    assert!(
        game.active_buffs().is_empty(),
        "a fresh game has no field buff and no combat buff armed"
    );
}

#[test]
fn active_buffs_reports_a_player_buff_with_no_holder_label() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));

    game.cast_field_routine(0, FieldCastTarget::Ally(player))
        .unwrap();
    let stored = game.world.get::<FieldBuff>(player).unwrap().active[0].clone();

    let buffs = game.active_buffs();

    assert_eq!(buffs.len(), 1);
    assert_eq!(buffs[0].name, "Test Field Regen");
    assert_eq!(
        buffs[0].holder_label, None,
        "the player carries no holder label"
    );
    assert_eq!(buffs[0].remaining, stored.remaining);
    assert_eq!(
        buffs[0].magnitude,
        FieldBuffKind::Regen.magnitude_label(stored.power, stored.interval)
    );
}

#[test]
fn active_buffs_reports_a_companion_buff_with_its_name() {
    let mut game = game_with_field_ability();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game.world
        .get_mut::<Needs>(game.player_entity())
        .unwrap()
        .hunger = 100.0;

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == companion)
        .expect("the companion's routine is listed");
    game.cast_field_routine(index, FieldCastTarget::Ally(companion))
        .unwrap();
    let expected_label = game.creature_label(companion);

    let buffs = game.active_buffs();

    assert_eq!(buffs.len(), 1);
    assert_eq!(buffs[0].holder_label, Some(expected_label));
}

#[test]
fn active_buffs_magnitude_reflects_the_scaled_power_not_the_authored_one() {
    let mut game = game_with_field_ability();
    let holder = spawn_tamed(&mut game, 10, 3);
    game.add_companion(holder).unwrap();
    set_level(&mut game, holder, CREATURE_MAX_LEVEL);
    game.world
        .entity_mut(holder)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game.world
        .get_mut::<Needs>(game.player_entity())
        .unwrap()
        .hunger = 100.0;

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == holder)
        .expect("the top-level holder's routine is listed");
    game.cast_field_routine(index, FieldCastTarget::Ally(holder))
        .unwrap();

    let scaled = abilities::scaled_stat_power(2, CREATURE_MAX_LEVEL, AFFINITY_NEUTRAL);
    // The authored magnitude in `FIELD_ONLY_ABILITY` is 2 — a top-level
    // holder's cast must scale well past that, so asserting against the
    // scaled value (rather than "2") actually exercises the distinction.
    assert_ne!(scaled, 2);

    let buffs = game.active_buffs();

    assert_eq!(buffs.len(), 1);
    assert_eq!(
        buffs[0].magnitude,
        FieldBuffKind::Regen.magnitude_label(scaled, 1)
    );
}

#[test]
fn active_buffs_includes_a_combat_buff_and_the_map_shows_none_without_one() {
    let mut game = Game::new(9105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    assert!(
        game.active_buffs().is_empty(),
        "outside battle, with no field buff armed, the list must be empty"
    );

    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.begin_defend(player);

    let buffs = game.active_buffs();

    assert_eq!(buffs.len(), 1);
    assert_eq!(buffs[0].name, "Defense");
    assert_eq!(buffs[0].remaining, 1);
    assert_eq!(buffs[0].holder_label, None);
}
