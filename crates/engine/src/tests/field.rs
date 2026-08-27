//! Running a field routine: `Game::field_routines` and
//! `Game::run_field_routine`.

use super::support::*;
use crate::components::{
    ActiveFieldBuff, BuffSource, FieldBuff, FieldBuffKind, Perks, PowerReserve, Routines,
};
use crate::resources::Party;
use crate::tuning::{AFFINITY_MAX, AFFINITY_NEUTRAL, CREATURE_MAX_LEVEL};
use crate::*;

fn game_with_field_ability() -> Game {
    let dir = modded_assets_dir(
        "field_routine",
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
        .get::<PowerReserve>(game.player_entity())
        .unwrap()
        .get()
}

fn reserve_of(game: &Game, entity: Entity) -> f32 {
    game.world.get::<PowerReserve>(entity).unwrap().get()
}

#[test]
fn running_arms_the_buff_and_deducts_power() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));

    let routines = game.field_routines();
    assert_eq!(routines.len(), 1);
    assert_eq!(routines[0].cost, "5 PWR");
    assert_eq!(routines[0].second_pick, FieldRoutinePick::Ally);

    let before = player_hunger(&game);
    game.run_field_routine(0, FieldRoutineTarget::Ally(player))
        .expect("running a field routine you can afford should succeed");

    // A successful run ticks the clock (see `run_field_routine`'s doc), so
    // the ordinary per-tick Power decay lands on top of the routine's own
    // cost. `systems::power_drain_per_tick` is the one place that decay formula
    // lives — read through it rather than restating its constant here.
    let expected_hunger = before - 5.0 - crate::systems::power_drain_per_tick(1.0);
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
    // duration: 20, aged by the one tick the successful run itself spends.
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

    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(4.0);
    let refused = game.run_field_routine(0, FieldRoutineTarget::Ally(player));
    assert!(refused.is_err(), "4.0 Power can't cover a 5.0 cost");
    assert_eq!(
        game.current_tick(),
        start,
        "a refused run spends nothing, so it must cost no time"
    );

    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(100.0);
    game.run_field_routine(0, FieldRoutineTarget::Ally(player))
        .expect("100.0 Power covers a 5.0 cost");
    assert_eq!(
        game.current_tick(),
        start + 1,
        "a successful run is a turn, exactly like use_item spending one on the same buff"
    );
}

#[test]
fn insufficient_power_returns_err_and_leaves_state_untouched() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(4.0);

    let result = game.run_field_routine(0, FieldRoutineTarget::Ally(player));

    assert_eq!(
        result,
        Err("Can't run Test Field Regen — not enough PWR.".to_string())
    );
    assert_eq!(
        player_hunger(&game),
        4.0,
        "a refused run must not spend Power"
    );
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .is_none_or(|b| b.active.is_empty()),
        "a refused run must not arm a buff"
    );
}

#[test]
fn running_during_a_battle_is_refused() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    let before = player_hunger(&game);

    let result = game.run_field_routine(0, FieldRoutineTarget::Ally(player));

    assert!(result.is_err(), "running mid-battle must be refused");
    assert_eq!(player_hunger(&game), before);
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .is_none_or(|b| b.active.is_empty())
    );
}

#[test]
fn running_underground_succeeds() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let pos = *game.world.get::<Position>(player).unwrap();
    game.enter_stack(pos.x, pos.y);

    let result = game.run_field_routine(0, FieldRoutineTarget::Ally(player));

    assert!(
        result.is_ok(),
        "running is not gated on require_surface and must work underground: {result:?}"
    );
    assert!(game.world.get::<FieldBuff>(player).is_some());
}

#[test]
fn a_higher_level_holder_casts_a_larger_magnitude() {
    let mut game = game_with_field_ability();
    let low = spawn_tamed(&mut game, 10, 3);
    let high = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, low);
    enlist(&mut game, high);
    // `CREATURE_MAX_LEVEL` rather than an arbitrary 20: a companion cannot
    // level past it in play, so a fixture that did would be scaling an invocation
    // nobody can ever make.
    set_level(&mut game, high, CREATURE_MAX_LEVEL);
    game.world
        .entity_mut(low)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game.world
        .entity_mut(high)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    *game
        .world
        .get_mut::<PowerReserve>(game.player_entity())
        .unwrap() = PowerReserve::new(100.0);

    let routines = game.field_routines();
    let low_index = routines
        .iter()
        .position(|r| r.holder == low)
        .expect("the level-1 holder's routine is listed");
    game.run_field_routine(low_index, FieldRoutineTarget::Ally(low))
        .unwrap();

    *game
        .world
        .get_mut::<PowerReserve>(game.player_entity())
        .unwrap() = PowerReserve::new(100.0);
    let routines = game.field_routines();
    let high_index = routines
        .iter()
        .position(|r| r.holder == high)
        .expect("the top-level holder's routine is listed");
    game.run_field_routine(high_index, FieldRoutineTarget::Ally(high))
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
        "a top-level holder's invocation should outscale a level-1 holder's: \
         {high_power} vs {low_power}"
    );
}

/// The two `FieldBuffKind::scales_with_invoker` tests below share this
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
    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(100.0);
    player
}

#[test]
fn a_percentage_kind_is_delivered_at_its_authored_value_regardless_of_level_or_affinity() {
    let dir = modded_assets_dir(
        "field_routine_pct_unscaled",
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

    game.run_field_routine(0, FieldRoutineTarget::None)
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
        "field_routine_flat_still_scales",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_atk.ron", FIELD_ONLY_PARTY_ABILITY)],
    );
    let mut game = Game::new(9106, DifficultyMode::Forgiving, &dir).unwrap();
    let player = buff_affinity_maxed_player(&mut game);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_atk".to_string()]));

    game.run_field_routine(0, FieldRoutineTarget::None)
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

/// `field_routine_targets` — what the ally picker offers — is narrower than
/// `routine_holders`: only the player and the active `Party`, since those
/// are the only entities `tick_field_buffs` ever walks. An owned program
/// that isn't in the party must not appear, even though it's a perfectly
/// valid `routine_holders` row (installing a routine on it is legitimate).
#[test]
fn field_routine_targets_excludes_an_owned_program_outside_the_party() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    let party_member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, party_member);
    let benched = spawn_tamed(&mut game, 10, 3);

    let targets = game.field_routine_targets(0);

    assert!(targets.iter().any(|t| t.entity == player));
    assert!(targets.iter().any(|t| t.entity == party_member));
    assert!(
        targets.iter().all(|t| t.entity != benched),
        "an owned program outside the active party must not be offered"
    );

    let holders = game.routine_holders();
    assert!(
        holders.iter().any(|h| h.entity == benched),
        "routine_holders (install/uninstall) still lists it — only running narrows"
    );
}

/// Arms the player with the fixture's `OneAlly` field routine, so
/// `field_routines()[0]` is it and `field_routine_targets(0)` is the picker
/// that would follow.
fn game_with_the_field_routine_installed() -> Game {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    game
}

/// The picker's whole job is choosing a body for the buff, and every shipped
/// `OneAlly` routine is about a stat: HP for Regen, ATK for Overclock, DEF
/// and Mitigation for the other two. Without the numbers on the row, the
/// only way to pick is to remember them.
#[test]
fn an_ally_picker_row_carries_the_targets_stats() {
    let mut game = game_with_the_field_routine_installed();
    let member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, member);
    let stats = *game.world.get::<Stats>(member).unwrap();

    let targets = game.field_routine_targets(0);
    let row = targets
        .iter()
        .find(|t| t.entity == member)
        .expect("the party member is offered");

    assert_eq!((row.hp, row.max_hp), (stats.hp, stats.max_hp));
    assert_eq!((row.atk, row.mitigation), (stats.atk, stats.mitigation));
    assert_eq!(row.power, stats.power());
}

/// The player is a target too, and carries no `Creature` — the stats have to
/// come off `Stats`, which both sides have, rather than off anything only a
/// program owns.
#[test]
fn the_ally_pickers_player_row_carries_stats_too() {
    let mut game = game_with_the_field_routine_installed();
    let player = game.player_entity();
    let stats = *game.world.get::<Stats>(player).unwrap();

    let targets = game.field_routine_targets(0);
    let row = targets
        .iter()
        .find(|t| t.entity == player)
        .expect("the player is always offered");

    assert_eq!((row.hp, row.max_hp), (stats.hp, stats.max_hp));
    assert_eq!(row.atk, stats.atk);
}

/// `arm_field_buff` displaces a running `Routine` buff of the same kind, so
/// running on a target that already has one replaces it rather than stacking
/// — and nothing else on this screen says so. The tag names the buff it
/// would overwrite, because two different routines can arm one kind
/// (Ablative Layer and Long Winter both arm Mitigation) and "already
/// running" alone would not say which is about to go.
#[test]
fn an_ally_picker_row_names_the_buff_this_cast_would_replace() {
    let mut game = game_with_the_field_routine_installed();
    let member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, member);
    game.arm_field_buff(
        member,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Repair Loop Single".to_string(),
            power: 7,
            remaining: 62,
            interval: 4,
            source: BuffSource::Routine,
        },
    );

    let targets = game.field_routine_targets(0);
    let running = targets
        .iter()
        .find(|t| t.entity == member)
        .expect("the party member is offered")
        .running
        .as_ref()
        .expect("a routine-armed Regen is exactly what this invocation would replace");

    assert_eq!(running.name, "Repair Loop Single");
    assert_eq!(running.remaining, "62t");
    assert_eq!(
        running.magnitude,
        FieldBuffKind::Regen.magnitude_label(7, 4),
        "the magnitude is the engine's to word, the same as the buff list's"
    );

    let player = game.player_entity();
    assert!(
        targets
            .iter()
            .find(|t| t.entity == player)
            .expect("the player is always offered")
            .running
            .is_none(),
        "a target carrying nothing of this kind gets no tag"
    );
}

/// A buff of the same kind armed by a *consumable* is deliberately not
/// named: `arm_field_buff` displaces `Consumable` and `Routine` entries
/// separately, so this invocation would leave it running. The tag says what is
/// about to be overwritten, not what is running in general — which is the
/// distinction that makes it worth reading.
#[test]
fn a_consumable_buff_of_the_same_kind_is_not_named_as_replaced() {
    let mut game = game_with_the_field_routine_installed();
    let member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, member);
    game.arm_field_buff(
        member,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Repair Patch".to_string(),
            power: 3,
            remaining: 30,
            interval: 1,
            source: BuffSource::Consumable,
        },
    );

    let targets = game.field_routine_targets(0);
    assert!(
        targets
            .iter()
            .find(|t| t.entity == member)
            .expect("the party member is offered")
            .running
            .is_none()
    );
}

/// The two Stack movement routines open no ally picker at all, and a routine
/// index naming one carries no `FieldBuffKind` to compare against — the rows
/// still list, untagged, rather than the call having to be unreachable.
#[test]
fn an_index_naming_no_field_buff_tags_nothing() {
    let mut game = game_with_the_field_routine_installed();
    let member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, member);
    game.arm_field_buff(
        member,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Repair Loop Single".to_string(),
            power: 7,
            remaining: 62,
            interval: 4,
            source: BuffSource::Routine,
        },
    );

    let targets = game.field_routine_targets(99);

    assert_eq!(targets.len(), 2, "the roster is unaffected by the index");
    assert!(targets.iter().all(|t| t.running.is_none()));
}

/// The picker narrowing above is a UX convenience, not the only guard: the
/// engine has to refuse a `OneAlly` run aimed at an owned-but-benched
/// program too, or a caller that bypasses the picker (a bug, a future UI)
/// could still arm a buff on an entity `tick_field_buffs` never walks — paid
/// for, frozen at full duration, and doing nothing forever.
#[test]
fn running_on_an_owned_program_outside_the_party_is_refused() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    let benched = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    let before = player_hunger(&game);

    let result = game.run_field_routine(0, FieldRoutineTarget::Ally(benched));

    assert!(
        result.is_err(),
        "running on an owned, non-party program must be refused"
    );
    assert_eq!(
        player_hunger(&game),
        before,
        "a refused run must not spend Power"
    );
    assert!(
        game.world
            .get::<FieldBuff>(benched)
            .is_none_or(|b| b.active.is_empty()),
        "a refused run must not arm a buff on the rejected target"
    );
}

#[test]
fn a_run_scoped_kind_lands_on_the_player_even_when_cast_off_a_companion() {
    let dir = modded_assets_dir(
        "field_routine_run",
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
        routines[index].second_pick == FieldRoutinePick::None,
        "a Run-scoped routine needs no target picker"
    );

    // `target: None` — a Run-scoped kind must ignore it entirely rather
    // than requiring one.
    game.run_field_routine(index, FieldRoutineTarget::None)
        .expect("a Run-scoped invocation needs no target");

    assert!(
        game.world.get::<FieldBuff>(companion).is_none(),
        "a Run-scoped buff must not land on the holder that ran it"
    );
    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].kind, FieldBuffKind::Trickle);
}

#[test]
fn whole_party_arms_every_living_member_and_skips_the_dead() {
    let dir = modded_assets_dir(
        "field_routine_party",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_atk.ron", FIELD_ONLY_PARTY_ABILITY)],
    );
    let mut game = Game::new(9103, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let alive = spawn_tamed(&mut game, 10, 3);
    let dead = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Stats>(dead).unwrap().hp = 0;
    game.world.resource_mut::<Party>().0.extend([alive, dead]);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["test_field_atk".to_string()]));

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == player)
        .expect("the player's routine is listed");
    assert_eq!(routines[index].second_pick, FieldRoutinePick::None);

    game.run_field_routine(index, FieldRoutineTarget::None)
        .expect("a WholeParty invocation needs no target");

    assert_eq!(game.world.get::<FieldBuff>(player).unwrap().active.len(), 1);
    assert_eq!(game.world.get::<FieldBuff>(alive).unwrap().active.len(), 1);
    assert!(
        game.world.get::<FieldBuff>(dead).is_none(),
        "a dead party member must not be armed"
    );
}

/// The same walk against the **shipped** routine rather than a fixture one:
/// `hardened_shell_party` hardens the player and every companion off one invocation,
/// which is the whole of what it buys over `hardened_shell` at the same +4.
///
/// Worth an asset-level test of its own because everything about the wide invocation
/// is authored: `target: WholeParty` is what reaches the party, and an
/// `assets/` edit narrowing it to `OneAlly` would leave a routine that still
/// loads, still lists, still costs the wide price and quietly hardens one body.
#[test]
fn the_shipped_party_def_routine_hardens_the_whole_party() {
    let mut game = Game::new(9105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 10, 3);
    game.world.resource_mut::<Party>().0.extend([first, second]);
    game.world
        .entity_mut(player)
        .insert(Routines(vec!["hardened_shell_party".to_string()]));

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.ability == "hardened_shell_party")
        .expect("the party routine is listed once installed");
    assert_eq!(
        routines[index].second_pick,
        FieldRoutinePick::None,
        "a WholeParty routine opens no ally picker"
    );

    game.run_field_routine(index, FieldRoutineTarget::None)
        .expect("a WholeParty invocation needs no target");

    // Authored, not scaled: `hardened_shell_party` is a `Mitigation` buff
    // now, and a percentage kind is delivered at exactly what the file says
    // — see `FieldBuffKind::scales_with_invoker`.
    for holder in [player, first, second] {
        let active = &game.world.get::<FieldBuff>(holder).unwrap().active;
        assert_eq!(active.len(), 1, "one buff per body");
        assert_eq!(active[0].kind, FieldBuffKind::Mitigation);
        assert_eq!(active[0].power, 12);
        assert!(
            active[0].runs_until_rest(),
            "an authored duration would give the wide invocation a turn count"
        );
    }
}

#[test]
fn field_routines_lists_across_holders_and_excludes_non_field() {
    let dir = modded_assets_dir(
        "field_routine_listing",
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

    game.run_field_routine(0, FieldRoutineTarget::Ally(player))
        .unwrap();
    let stored = game.world.get::<FieldBuff>(player).unwrap().active[0].clone();

    let buffs = game.active_buffs();

    assert_eq!(buffs.len(), 1);
    assert_eq!(buffs[0].name, "Test Field Regen");
    assert_eq!(
        buffs[0].holder_label, None,
        "the player carries no holder label"
    );
    assert_eq!(buffs[0].remaining, stored.duration_label());
    assert_eq!(
        buffs[0].magnitude,
        FieldBuffKind::Regen.magnitude_label(stored.power, stored.interval)
    );
}

#[test]
fn active_buffs_reports_a_companion_buff_with_its_name() {
    let mut game = game_with_field_ability();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    *game
        .world
        .get_mut::<PowerReserve>(game.player_entity())
        .unwrap() = PowerReserve::new(100.0);

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == companion)
        .expect("the companion's routine is listed");
    game.run_field_routine(index, FieldRoutineTarget::Ally(companion))
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
    enlist(&mut game, holder);
    set_level(&mut game, holder, CREATURE_MAX_LEVEL);
    game.world
        .entity_mut(holder)
        .insert(Routines(vec!["test_field_regen".to_string()]));
    *game
        .world
        .get_mut::<PowerReserve>(game.player_entity())
        .unwrap() = PowerReserve::new(100.0);

    let routines = game.field_routines();
    let index = routines
        .iter()
        .position(|r| r.holder == holder)
        .expect("the top-level holder's routine is listed");
    game.run_field_routine(index, FieldRoutineTarget::Ally(holder))
        .unwrap();

    let scaled = abilities::scaled_stat_power(2, CREATURE_MAX_LEVEL, AFFINITY_NEUTRAL);
    // The authored magnitude in `FIELD_ONLY_ABILITY` is 2 — a top-level
    // holder's invocation must scale well past that, so asserting against the
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
    assert_eq!(buffs[0].name, "Mitigation");
    assert_eq!(buffs[0].remaining, "1t");
    assert_eq!(buffs[0].holder_label, None);
}

/// A routine is paid for by whoever runs it. The row already names its
/// holder, and a companion's reserve is refilled by rest exactly as its
/// battle Specials' is — so a companion's field routine draws on the
/// companion, not on the player's bar.
#[test]
fn a_companions_field_routine_spends_the_companions_reserve() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["test_field_regen".to_string()]));

    let routines = game.field_routines();
    assert_eq!(routines.len(), 1, "only the companion holds one");
    assert_eq!(routines[0].holder, companion);

    let player_before = player_hunger(&game);
    let companion_before = reserve_of(&game, companion);
    game.run_field_routine(0, FieldRoutineTarget::Ally(player))
        .expect("the companion can afford its own routine");

    assert_eq!(
        reserve_of(&game, companion),
        companion_before - 5.0,
        "the invoker pays"
    );
    assert_eq!(
        player_hunger(&game),
        player_before - crate::systems::power_drain_per_tick(1.0),
        "the player pays only the turn's ordinary decay, never the routine's cost"
    );
}

/// The gate reads the same reserve the charge takes from — a full player
/// bar cannot run a drained companion's routine, and a drained player bar
/// does not block a companion's.
#[test]
fn a_holders_own_reserve_decides_whether_its_routine_is_offered() {
    let mut game = game_with_field_ability();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["test_field_regen".to_string()]));

    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(100.0);
    *game.world.get_mut::<PowerReserve>(companion).unwrap() = PowerReserve::new(4.0);
    assert_eq!(
        game.field_routines()[0].unavailable.as_deref(),
        Some("not enough PWR"),
        "a full player bar must not make a drained companion's routine look runnable"
    );
    assert!(
        game.run_field_routine(0, FieldRoutineTarget::Ally(player))
            .is_err(),
        "and the invocation must refuse it too"
    );

    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(1.0);
    *game.world.get_mut::<PowerReserve>(companion).unwrap() = PowerReserve::new(100.0);
    assert_eq!(
        game.field_routines()[0].unavailable,
        None,
        "a drained player must not block a companion's own routine"
    );
    game.run_field_routine(0, FieldRoutineTarget::Ally(player))
        .expect("the companion has the Power for it");
}
