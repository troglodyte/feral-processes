//! The battle telemetry buffer: that it is off unless asked for, costs
//! nothing while off, and hands its records over exactly once.

use super::support::*;
use crate::resources::BattleTelemetry;
use crate::telemetry::Record;
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
