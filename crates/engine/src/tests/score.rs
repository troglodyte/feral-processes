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

/// Underground a wild body's level is still the surface `ZoneLevel`: a
/// frame's tier is read live from it (`Game::frame_spec`), so there is no
/// second formula for a kill's worth to drift from. Sortie hostiles are
/// surface spawns and `run_sorties` keeps running while the player is down.
#[test]
fn an_underground_kill_is_worth_the_zone_level() {
    let mut game = new_game(9107);
    game.world.resource_mut::<crate::resources::ZoneLevel>().0 = 7;
    descend(&mut game);
    assert!(game.stack_pos().is_some(), "the player is underground");
    let tier = game
        .stack_pos()
        .map(|p| game.frame_spec(p.depth, p.frames, p.entrance).tier);
    assert_eq!(tier, Some(7));
    let wild = spawn_wild_on_player_tile(&mut game);
    game.award_loot(wild, 0.0);

    assert_eq!(tally(&game).foe_levels, 7);
}

#[test]
fn a_sortie_kill_with_the_player_underground_counts_the_zone_level() {
    let found = (5000..5020).any(|seed| {
        let (mut game, _) = super::sorties::a_dispatched_sortie(seed, DifficultyMode::Forgiving);
        let zone = game.world.resource::<crate::resources::ZoneLevel>().0;
        descend(&mut game);
        let total = game.world.resource::<crate::resources::Sorties>().0[0].ticks_total;
        for _ in 0..(total - 1) {
            game.wait();
        }
        let killed = !game.world.resource::<crate::resources::Sorties>().0[0]
            .programs
            .is_empty();
        if killed {
            let levels = tally(&game).foe_levels;
            assert!(
                levels > 0 && levels.is_multiple_of(u64::from(zone)),
                "{levels} vs zone {zone}"
            );
        }
        killed
    });
    assert!(found, "no seed's sortie banked a kill underground");
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

fn set_foe_levels(game: &mut Game, levels: u64) {
    game.world.resource_mut::<RunTally>().foe_levels = levels;
}

fn lifetime(game: &Game) -> u64 {
    game.profile().lifetime_score
}

#[test]
fn a_death_banks_the_card_total() {
    let mut game = new_game(9201);
    set_foe_levels(&mut game, 50);
    let total = game.score_card().total;
    assert!(total > 0);

    assert_eq!(game.bank_run_score(), total);
    assert_eq!(lifetime(&game), total);
    assert_eq!(tally(&game).banked, total);
}

#[test]
fn an_escape_then_a_death_bank_exactly_the_total_overall() {
    let mut game = new_game(9202);
    set_foe_levels(&mut game, 50);
    let at_escape = game.bank_run_score();
    set_foe_levels(&mut game, 80);
    let at_death = game.bank_run_score();

    assert!(at_escape > 0 && at_death > 0);
    assert_eq!(lifetime(&game), game.score_card().total);
}

#[test]
fn banking_twice_with_nothing_new_banks_nothing() {
    let mut game = new_game(9203);
    set_foe_levels(&mut game, 50);
    game.bank_run_score();
    let before = lifetime(&game);

    assert_eq!(game.bank_run_score(), 0);
    assert_eq!(lifetime(&game), before);
}

#[test]
fn a_total_that_fell_after_banking_is_not_banked_again_on_regrowth() {
    let mut game = new_game(9204);
    set_foe_levels(&mut game, 50);
    let first = game.bank_run_score();
    set_foe_levels(&mut game, 10);
    assert_eq!(game.bank_run_score(), 0);
    set_foe_levels(&mut game, 50);
    assert_eq!(game.bank_run_score(), 0, "the first {first} is already in");
}

#[test]
fn the_history_line_ends_with_the_score() {
    let mut game = new_game(9205);
    set_foe_levels(&mut game, 50);
    let total = game.score_card().total;
    game.world
        .resource_mut::<crate::resources::GameOver>()
        .reason = Some("Test over".into());

    let line = game.history_summary().unwrap();
    assert!(line.ends_with(&format!("Score: {total}.")), "{line}");
}

#[test]
fn the_card_reads_live_keys_structures_and_the_escape() {
    let mut game = new_game(9206);
    let player = game.player_entity();
    {
        let mut keys = game
            .world
            .get_mut::<crate::components::PhaseKeys>(player)
            .unwrap();
        keys.story_complete = true;
        keys.held = 0b101;
    }
    let before = game.score_card();
    for x in 0..2 {
        game.world.spawn((
            crate::components::Structure {
                kind: "relay".to_string(),
            },
            crate::components::Position { x, y: 0 },
        ));
    }
    let card = game.score_card();
    let count_of = |card: &crate::score::ScoreCard, label: &str| {
        card.lines.iter().find(|l| l.label == label).unwrap().count
    };
    assert_eq!(count_of(&card, "Phase keys held"), 2);
    assert_eq!(
        count_of(&card, "Structures standing"),
        count_of(&before, "Structures standing") + 2
    );
    let escape = card
        .lines
        .iter()
        .find(|l| l.label == "Escaped the Basin")
        .unwrap();
    assert_eq!(escape.count, 1);
}
