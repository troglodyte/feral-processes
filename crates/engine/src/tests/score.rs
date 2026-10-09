//! The run score's tally sites, banking and history line
//! (`docs/superpowers/specs/2026-10-09-run-score-design.md`).

use super::support::*;
use crate::resources::{DifficultyMode, RunTally};
use crate::*;

/// A real save and load, never RON alone —
/// `ron-round-trip-cannot-catch-a-skipped-field`.
#[test]
fn a_nonzero_run_tally_survives_a_save_and_load() {
    let dir = scratch_assets_dir("run_tally_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20261009, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tally = RunTally {
        foe_levels: 41,
        bosses: 2,
        compiled: 3,
        achievements: 4,
        deepest_depth: 5,
        banked: 600,
    };
    *game.world.resource_mut::<RunTally>() = tally.clone();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(*loaded.world.resource::<RunTally>(), tally);
}

fn tally(game: &Game) -> RunTally {
    game.world.resource::<RunTally>().clone()
}

fn new_game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

#[test]
fn a_present_kill_is_worth_the_foes_level() {
    let mut game = new_game(9101);
    game.world.resource_mut::<crate::resources::ZoneLevel>().0 = 7;
    let wild = spawn_wild_on_player_tile(&mut game);
    game.award_loot(wild, 0.0);

    assert_eq!(tally(&game).foe_levels, 7);
    assert_eq!(tally(&game).bosses, 0, "an ordinary program is not a boss");
}

#[test]
fn a_boss_kill_moves_the_boss_count() {
    let mut game = new_game(9102);
    let boss = spawn_boss_on_player_tile(&mut game);
    game.award_loot(boss, 0.0);

    assert_eq!(tally(&game).bosses, 1);
}

#[test]
fn a_sortie_kill_adds_the_foes_level() {
    let found = (5000..5020).any(|seed| {
        let (mut game, _) = super::sorties::a_dispatched_sortie(seed, DifficultyMode::Forgiving);
        let total = game.world.resource::<crate::resources::Sorties>().0[0].ticks_total;
        for _ in 0..(total - 1) {
            game.wait();
        }
        let killed = !game.world.resource::<crate::resources::Sorties>().0[0]
            .programs
            .is_empty();
        killed && tally(&game).foe_levels > 0
    });
    assert!(
        found,
        "no seed's sortie both banked a kill and moved the tally"
    );
}

#[test]
fn a_decompile_moves_the_compiled_count() {
    let mut game = new_game(9103);
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 1000;
    let taken = (0..50).any(|_| game.decompile_body(wild, player));
    assert!(taken, "a skill of 1000 decompiles within 50 rolls");

    assert_eq!(tally(&game).compiled, 1);
}

#[test]
fn an_earned_achievement_moves_the_tally() {
    let mut game = new_game(9104);
    let boss = spawn_boss_on_player_tile(&mut game);
    game.award_loot(boss, 0.0);
    game.tick();

    let earned = game.profile().earned.len() as u32;
    assert!(earned > 0, "a boss kill earns at least boss_first");
    assert_eq!(tally(&game).achievements, earned);
}

#[test]
fn depth_is_a_high_water_mark() {
    let mut game = new_game(9105);
    game.descend_to(3, 5, (10, 10));
    assert_eq!(tally(&game).deepest_depth, 3);
    game.descend_to(2, 5, (10, 10));
    assert_eq!(tally(&game).deepest_depth, 3, "climbing does not lower it");
}
