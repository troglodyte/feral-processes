//! The cross-run achievement profile: what a run records, what the tick
//! decides has been earned, and what the next run gets paid for it.

use super::support::*;
use crate::achievements::{AchievementId, Profile};
use crate::resources::{PendingProfileWrites, RunFeats, ZoneLevel};
use crate::*;

fn earned_ids(game: &Game) -> Vec<String> {
    game.world
        .resource::<Profile>()
        .earned
        .iter()
        .map(|e| e.id.to_string())
        .collect()
}

fn pending_ids(game: &Game) -> Vec<String> {
    game.world
        .resource::<PendingProfileWrites>()
        .0
        .iter()
        .map(|id| id.to_string())
        .collect()
}

fn has_earned(game: &Game, id: &str) -> bool {
    game.world
        .resource::<Profile>()
        .contains(&AchievementId::from(id))
}

/// A hostile of `species_id` standing on the player's tile with stats set by
/// hand — a shipped boss is 200+ HP with a `growth_multiplier` of 2.0, and
/// these tests care about whether the kill was *recorded*, not about winning
/// a real boss fight.
fn spawn_boss_on_player_tile(game: &mut Game, species_id: &str, hp: i32) -> Entity {
    let at = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert!(
        game.species_defs()
            .into_iter()
            .any(|s| s.id == species_id && s.is_boss),
        "{species_id} should be a shipped boss species"
    );
    game.world
        .spawn((
            Creature {
                species: species_id.to_string(),
            },
            Hostile,
            Position { x: at.x, y: at.y },
            Stats {
                hp,
                max_hp: hp,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id()
}

/// Attacks until the battle is over. Bounded rather than looping forever, so
/// a fixture that can't actually win fails loudly.
fn fight_to_the_end(game: &mut Game) {
    for _ in 0..200 {
        if !game.has_active_battle() {
            return;
        }
        player_attacks(game);
    }
    panic!("200 rounds and the fight is still going — the fixture cannot win it");
}

/// `award_loot` directly rather than through a fight, because `RunFeats` is
/// a per-tick queue and `achievement_system` drains it in the same tick the
/// kill lands in — a post-fight read would see an empty queue whether or not
/// the record was ever written. What the drain then *earns* is
/// `killing_a_boss_earns_both_the_generic_and_the_species_rung_in_one_tick`.
#[test]
fn killing_a_boss_records_its_species() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game, "overseer", 1);
    game.award_loot(boss);

    assert_eq!(
        game.world.resource::<RunFeats>().bosses_defeated,
        vec!["overseer".to_string()]
    );
}

#[test]
fn killing_an_ordinary_program_records_nothing() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.award_loot(wild);

    assert!(
        game.world.resource::<RunFeats>().bosses_defeated.is_empty(),
        "only a boss is a feat"
    );
}

/// Asserted against the profile rather than against `RunFeats`, which the
/// tick drains either way: what has to be true is that fleeing earns
/// *nothing*, and a killed boss would have earned `boss_first` here.
#[test]
fn fleeing_a_boss_records_nothing() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game, "overseer", 500);
    game.start_battle(vec![boss]);
    flee_until_clear(&mut game);
    game.tick();

    assert!(
        !has_earned(&game, "boss_first"),
        "the record sits where `mark_lair_cleared` does, at the one point that \
         knows the boss died rather than being fled from"
    );
}

#[test]
fn reaching_zone_two_earns_the_first_breach() {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.enter_next_zone();
    game.tick();

    assert!(has_earned(&game, "breach_zone_2"));
    assert_eq!(pending_ids(&game), vec!["breach_zone_2".to_string()]);
}

#[test]
fn staying_in_zone_one_earns_nothing() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.tick();

    assert!(
        earned_ids(&game).is_empty(),
        "zone 1 on cycle 1 has cleared no thresholds, got {:?}",
        earned_ids(&game)
    );
}

#[test]
fn descending_to_depth_three_earns_the_stack_rung() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    dive_to_depth(&mut game, 3);
    game.tick();

    assert!(has_earned(&game, "stack_depth_3"));
    assert!(
        !has_earned(&game, "stack_depth_5"),
        "depth 3 is not depth 5"
    );
}

#[test]
fn surviving_five_hundred_cycles_earns_uptime() {
    let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<resources::GameClock>().tick = 499;
    game.tick();
    assert!(
        !has_earned(&game, "uptime_500"),
        "499 cycles is short of the threshold"
    );

    game.tick();
    assert!(has_earned(&game, "uptime_500"));
}

#[test]
fn killing_a_boss_earns_both_the_generic_and_the_species_rung_in_one_tick() {
    let mut game = Game::new(25, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game, "overseer", 1);
    game.start_battle(vec![boss]);
    fight_to_the_end(&mut game);

    assert!(has_earned(&game, "boss_first"));
    assert!(has_earned(&game, "boss_overseer"));
    assert!(
        !has_earned(&game, "boss_wintermute"),
        "a different boss is a different rung"
    );
}

#[test]
fn a_second_tick_after_a_boss_kill_earns_nothing_more() {
    let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game, "overseer", 1);
    game.start_battle(vec![boss]);
    fight_to_the_end(&mut game);
    let after_the_kill = pending_ids(&game);

    assert!(
        game.world.resource::<RunFeats>().bosses_defeated.is_empty(),
        "the queue must be drained whether or not anything was earned, or one \
         kill re-earns forever"
    );

    game.tick();
    game.tick();
    assert_eq!(
        pending_ids(&game),
        after_the_kill,
        "later ticks must not re-earn from the same kill"
    );
}

#[test]
fn an_achievement_is_earned_only_once() {
    let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.enter_next_zone();
    game.tick();
    game.tick();
    game.tick();

    assert_eq!(pending_ids(&game), vec!["breach_zone_2".to_string()]);
    assert_eq!(earned_ids(&game).len(), 1);
}

#[test]
fn an_earned_line_survives_the_end_of_a_battle() {
    let mut game = Game::new(28, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    game.start_battle(vec![wild]);
    // Crossed mid-fight, which is the case `MessageKind::Info` would lose:
    // `retain_outcomes_since_battle` prunes everything but four kinds when
    // the battle ends.
    game.world.insert_resource(ZoneLevel(2));
    fight_to_the_end(&mut game);

    assert!(!game.has_active_battle(), "the fight should be over");
    let log = game.message_log(100);
    assert!(
        log.iter().any(|l| l.text.contains("First Breach")),
        "the earned line should have survived the end of the battle, got {:?}",
        log.iter().map(|l| &l.text).collect::<Vec<_>>()
    );
}

/// A profile holding exactly `id`, with `rolled_stat` as recorded — the shape
/// app-core hands `install_profile` after reading `profile.ron`.
fn profile_of(id: &str, rolled_stat: Option<achievements::MainStat>) -> Profile {
    let mut profile = Profile::default();
    profile.record(achievements::Earned {
        id: AchievementId::from(id),
        first_tick: 1,
        permadeath: false,
        rolled_stat,
    });
    profile
}

#[test]
fn a_profile_stat_reward_applies_at_new_game() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.install_profile(profile_of(
        "breach_zone_2",
        Some(achievements::MainStat::Atk),
    ));
    game.grant_profile_rewards();

    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().atk,
        tuning::PLAYER_BASE_STATS.atk + 1
    );
}

#[test]
fn an_integrity_reward_starts_the_run_at_full_hp() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.install_profile(profile_of(
        "breach_zone_2",
        Some(achievements::MainStat::Integrity),
    ));
    game.grant_profile_rewards();

    let player = game.player_entity();
    let stats = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(stats.max_hp, tuning::PLAYER_BASE_STATS.max_hp + 1);
    assert_eq!(stats.hp, stats.max_hp, "the run must not start damaged");
}

#[test]
fn a_perk_point_reward_applies_at_new_game() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.install_profile(profile_of("breach_zone_4", None));
    game.grant_profile_rewards();

    let player = game.player_entity();
    assert_eq!(game.world.get::<Perks>(player).unwrap().points, 1);
}

#[test]
fn a_starting_program_is_owned_but_not_deployed() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.install_profile(profile_of("stack_depth_8", None));
    game.grant_profile_rewards();

    assert_eq!(game.pet_count(), 1, "the program should be owned");
    assert!(
        game.world.resource::<resources::Party>().0.is_empty(),
        "Party is the deployed battle line; the player deploys it themselves"
    );
}

/// The load path in miniature. A save already has its bonuses baked into
/// `Stats` and `Perks::points`; paying again on load would double them on
/// every single reload.
#[test]
fn installing_a_profile_pays_nothing_on_its_own() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.install_profile(profile_of(
        "breach_zone_2",
        Some(achievements::MainStat::Atk),
    ));

    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().atk,
        tuning::PLAYER_BASE_STATS.atk,
        "installing says what has been earned; it does not pay for it"
    );
}

#[test]
fn granting_twice_pays_once() {
    let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.install_profile(profile_of(
        "breach_zone_2",
        Some(achievements::MainStat::Atk),
    ));
    game.grant_profile_rewards();
    game.grant_profile_rewards();

    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().atk,
        tuning::PLAYER_BASE_STATS.atk + 1,
        "nothing should call this twice, but the doubling would be invisible if it did"
    );
}

#[test]
fn a_starting_program_naming_an_unknown_species_warns_and_pays_nothing() {
    let dir = scratch_assets_with_achievement(
        "ghost_program",
        r#"(
            id: "ghost_program",
            name: "ZeroDay Program",
            description: "d",
            trigger: ZoneReached(2),
            reward: StartingProgram("no_such_species"),
        )"#,
    );
    let mut game = Game::new(37, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    game.install_profile(profile_of("ghost_program", None));
    game.grant_profile_rewards();

    assert_eq!(game.pet_count(), 0);
    let log = game.message_log(100);
    assert!(
        log.iter().any(|l| l.text.contains("no_such_species")),
        "an unknown species should say so, got {:?}",
        log.iter().map(|l| &l.text).collect::<Vec<_>>()
    );
}

/// `grant_starting_program` is the fourth door into the roster, and the only
/// one a player never chooses to walk through — the program arrives with the
/// run. It has to be able to cast like any other companion. See
/// `Game::roster_parts` for why each door is tested on its own.
#[test]
fn a_starting_program_arrives_holding_a_full_reserve() {
    let dir = scratch_assets_with_achievement(
        "charged_program",
        r#"(
            id: "charged_program",
            name: "Charged Program",
            description: "d",
            trigger: ZoneReached(2),
            reward: StartingProgram("scrapper"),
        )"#,
    );
    let mut game = Game::new(38, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    game.install_profile(profile_of("charged_program", None));
    game.grant_profile_rewards();

    let player = game.player_entity();
    let reserves: Vec<f32> = game
        .world
        .iter_entities()
        .filter(|e| e.get::<Tamed>().is_some_and(|t| t.owner == player))
        .filter_map(|e| e.get::<PowerReserve>().map(|r| r.get()))
        .collect();
    assert_eq!(
        reserves,
        vec![POWER_MAX],
        "the program a profile hands you must arrive able to cast"
    );
}
