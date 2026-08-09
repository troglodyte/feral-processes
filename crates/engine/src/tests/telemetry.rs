//! The battle telemetry buffer: that it is off unless asked for, costs
//! nothing while off, and hands its records over exactly once.

use super::support::*;
use crate::resources::BattleTelemetry;
use crate::telemetry::{ActionKind, Record};
use crate::*;

fn fresh() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// The trainer's guard. `train` runs 1.9M fights in a session and `arena`
/// tens of thousands, none of which asked for telemetry — the cost of this
/// feature to them has to be zero records, and this is what says so.
#[test]
fn telemetry_is_off_by_default() {
    let mut game = fresh();
    assert!(!game.world.resource::<BattleTelemetry>().on);
    assert!(game.take_battle_telemetry().is_empty());
}

/// Proves the closure is not invoked while disabled, which a "drains empty"
/// assertion cannot: an eager `record(Record::EnemyChoice { .. })` would
/// pass that while still building three `String`s on every swing of every
/// fight the trainer runs.
#[test]
fn a_disabled_game_does_not_build_records() {
    let mut game = fresh();
    game.record(|_| panic!("a disabled game must not build a record"));
    assert!(game.take_battle_telemetry().is_empty());
}

/// `take_*` is a drain, matching `take_pending_profile_writes` — app-core
/// appends what it is handed, so a second read must not append it twice.
#[test]
fn taking_the_records_empties_the_buffer() {
    let mut game = fresh();
    game.enable_battle_telemetry();
    game.record(|_| Record::FightEnd {
        fight: 1,
        rounds: 3,
        won: true,
        player_hp_frac: 0.5,
        companions_downed: 0,
    });

    assert_eq!(game.take_battle_telemetry().len(), 1);
    assert!(game.take_battle_telemetry().is_empty());
}

#[test]
fn fight_ids_increase() {
    let mut game = fresh();
    let first = game.next_fight_id();
    let second = game.next_fight_id();
    assert!(second > first, "{second} should follow {first}");
}

// ─────────────────────────────────────────────────────────────────────────
// Emission at the seams

/// A scrapper — melee-only, and stripped of routines so `wild_retaliate`
/// always takes the `choose_wild_action` branch rather than spending its
/// round on an installed ability, which emits no choice at all.
fn wild_scrapper(game: &mut Game, atk: i32, hp: i32) -> Entity {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = spawn_wild_without_routine(game, "scrapper", pos.x, pos.y);
    let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
    stats.atk = atk;
    stats.hp = hp;
    stats.max_hp = hp;
    wild
}

fn enemy_choices(records: &[Record]) -> Vec<&Record> {
    records
        .iter()
        .filter(|r| matches!(r, Record::EnemyChoice { .. }))
        .collect()
}

/// The number the whole feature exists to collect, and the one an ordering
/// slip silently inverts: taken after the swing, `target_hp_before` is the
/// HP *after* the hit and every conclusion drawn from the dataset is wrong
/// while still looking plausible.
#[test]
fn an_enemy_choice_records_the_targets_hp_before_the_hit() {
    let mut game = fresh();
    let player = game.player_entity();
    let wild = wild_scrapper(&mut game, 20, 500);
    insert_battle(&mut game, player, vec![wild]);
    game.enable_battle_telemetry();

    let before = game.world.get::<Stats>(player).unwrap().hp;
    game.wild_retaliate(wild, 0, player);

    let records = game.take_battle_telemetry();
    let choices = enemy_choices(&records);
    assert_eq!(choices.len(), 1, "one swing, one record: {records:?}");
    let Record::EnemyChoice {
        target_hp_before,
        target_slot,
        ..
    } = choices[0]
    else {
        unreachable!()
    };
    assert_eq!(*target_hp_before, before);
    assert_eq!(
        *target_slot, 0,
        "no companions, so the player is the target"
    );

    let after = game.world.get::<Stats>(player).unwrap().hp;
    assert!(
        after < before,
        "the swing must actually have landed, or the record proves nothing: {before} -> {after}"
    );
}

/// A seam that quietly misses swings yields a biased dataset, which is
/// worse than no dataset at all. Counted against the log lines the same
/// branch writes, so the two cannot drift apart unnoticed.
#[test]
fn every_enemy_swing_produces_one_record() {
    let mut game = fresh();
    let player = game.player_entity();
    // Fat and feeble on purpose: nothing dies on either side, so the fight
    // runs the full five rounds instead of ending early.
    let pack: Vec<Entity> = (0..3).map(|_| wild_scrapper(&mut game, 1, 5_000)).collect();
    insert_battle(&mut game, player, pack);
    game.enable_battle_telemetry();

    let mut records = Vec::new();
    for _ in 0..5 {
        assert!(
            game.has_active_battle(),
            "the fixture must outlast the test"
        );
        resolve_round_with(&mut game, BattleAction::Attack { group: 0 });
        records.extend(game.take_battle_telemetry());
    }

    let swings = game
        .world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|l| l.text.starts_with("The rogue program executes"))
        .count();
    assert!(swings > 0, "the fixture produced no enemy swings at all");
    assert_eq!(
        enemy_choices(&records).len(),
        swings,
        "every swing the log narrates must have a record behind it"
    );
}

/// The routines half — the reason the feature exists. `arena::run_rep`
/// plays All-Attack and so can never answer what a party using its kit
/// does; this is the record that can.
#[test]
fn a_party_special_records_its_ability_and_target() {
    let mut game = fresh();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 30, 5);
    game.add_companion(companion).unwrap();
    let wild = wild_scrapper(&mut game, 1, 5_000);
    insert_battle(&mut game, player, vec![wild]);
    let expected = game.actor_abilities(companion)[0].id.clone();
    game.enable_battle_telemetry();

    companion_uses_special(
        &mut game,
        companion,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );

    let records = game.take_battle_telemetry();
    let specials: Vec<&Record> = records
        .iter()
        .filter(|r| {
            matches!(
                r,
                Record::PartyAction {
                    kind: ActionKind::Special,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(specials.len(), 1, "one Special was planned: {records:?}");
    let Record::PartyAction {
        slot,
        name,
        target_slot,
        ..
    } = specials[0]
    else {
        unreachable!()
    };
    assert_eq!(*slot, 1);
    assert_eq!(name.as_deref(), Some(expected.as_str()));
    assert_eq!(*target_slot, Some(0), "the rally landed on the player");
}

#[test]
fn a_fight_emits_a_start_and_an_end_sharing_one_id() {
    let mut game = fresh();
    game.enable_battle_telemetry();

    let ids = |game: &mut Game| {
        let wild = wild_scrapper(game, 0, 5_000);
        game.start_battle(vec![wild]);
        flee_until_clear(game);
        let records = game.take_battle_telemetry();
        let start = records.iter().find_map(|r| match r {
            Record::FightStart { fight, .. } => Some(*fight),
            _ => None,
        });
        let end = records.iter().find_map(|r| match r {
            Record::FightEnd { fight, .. } => Some(*fight),
            _ => None,
        });
        (start.expect("a fight starts"), end.expect("and ends"))
    };

    let (first_start, first_end) = ids(&mut game);
    assert_eq!(first_start, first_end, "one fight, one id");

    let (second_start, _) = ids(&mut game);
    assert_ne!(
        first_start, second_start,
        "a second fight must be distinguishable in the file"
    );
}

/// The `None` path — "nothing it has reaches from where it stands" — is not
/// a swing and must not read as one. Guards against an "every call emits"
/// refactor inventing attacks that never happened.
#[test]
fn a_back_group_that_cannot_reach_emits_no_choice() {
    let mut game = fresh();
    let player = game.player_entity();
    let wild = wild_scrapper(&mut game, 5, 100);
    insert_battle(&mut game, player, vec![wild]);
    game.enable_battle_telemetry();

    assert!(
        game.choose_wild_action(wild, tuning::ENGAGED_GROUPS, player)
            .is_none(),
        "a melee-only species in a back group reaches nothing"
    );
    assert!(
        enemy_choices(&game.take_battle_telemetry()).is_empty(),
        "no swing happened, so nothing may be recorded"
    );
}
