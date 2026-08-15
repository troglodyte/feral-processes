//! What a level-up announces. The three sites that can grant one — the
//! player, a party member and a posted worker — each write a header line
//! and a block of stat lines under it, and they have to agree on the shape
//! since `progression::stat_block` is the one thing that renders it.

use super::support::*;
use crate::components::{Decompiler, Experience, Stats};
use crate::progression::{StatRow, stat_block};
use crate::resources::{CONDENSE_LOOKBACK, LogLine, MessageSource, condense};
use crate::tuning::{DECOMPILER_SKILL_PER_LEVEL, PERK_POINTS_PER_LEVEL};
use crate::*;

/// The indented stat lines out of a log, in order. Reaching for the two
/// leading spaces rather than for a stat name keeps the assertion about
/// "this is a block under a header" rather than about any one stat.
fn stat_lines(game: &Game) -> Vec<String> {
    game.message_log(MESSAGE_LOG_CAP)
        .into_iter()
        .filter(|e| e.text.starts_with("  "))
        .map(|e| e.text)
        .collect()
}

#[test]
fn a_player_level_up_lists_what_each_stat_grew_to() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().xp_to_next = 5;
    let before = *game.world.get::<Stats>(player).unwrap();

    game.award_player_xp(player, 5);

    let after = *game.world.get::<Stats>(player).unwrap();
    assert_eq!(
        stat_lines(&game),
        vec![
            format!("  Max HP {} → {}", before.max_hp, after.max_hp),
            format!("  ATK {} → {}", before.atk, after.atk),
            format!("  DEF {} → {}", before.def, after.def),
            format!("  Perk Points 0 → {PERK_POINTS_PER_LEVEL}"),
            format!(
                "  Decompiler {} → {}",
                game.world.get::<Decompiler>(player).unwrap().skill - DECOMPILER_SKILL_PER_LEVEL,
                game.world.get::<Decompiler>(player).unwrap().skill
            ),
        ],
        "the player's block also reports the Perk Point and Decompiler skill a level pays"
    );
}

#[test]
fn every_line_of_a_level_up_block_is_tagged_level_up() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().xp_to_next = 5;

    game.award_player_xp(player, 5);

    // `retain_outcomes_since_battle` keeps only Outcome/Loot/LevelUp/Raid, so
    // a stat line tagged anything else would be dropped on the way out of a
    // fight and the block would arrive on the map with its header alone.
    let block: Vec<_> = game
        .message_log(MESSAGE_LOG_CAP)
        .into_iter()
        .filter(|e| e.text.starts_with("  "))
        .collect();
    // Without this the filter below is satisfied by a log with no block in it
    // at all, and the test would keep passing with the feature torn out.
    assert!(!block.is_empty(), "there should be a block to check");
    let untagged: Vec<_> = block
        .iter()
        .filter(|e| e.kind != MessageKind::LevelUp)
        .collect();
    assert!(
        untagged.is_empty(),
        "every stat line has to survive the battle prune: {untagged:?}"
    );
}

#[test]
fn a_companion_level_up_lists_its_stat_growth() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    game.world
        .get_mut::<Experience>(companion)
        .unwrap()
        .xp_to_next = 1;
    let before = *game.world.get::<Stats>(companion).unwrap();

    // The party earns half, so this has to clear 1 XP after halving.
    game.award_player_xp(player, 4);

    let after = *game.world.get::<Stats>(companion).unwrap();
    let name = game.creature_label(companion);
    let log = game.message_log(MESSAGE_LOG_CAP);
    let header = log
        .iter()
        .position(|e| e.text.starts_with(&format!("{name} gains")))
        .expect("the companion should have announced a level-up");
    assert_eq!(
        log[header + 1].text,
        format!("  Max HP {} → {}", before.max_hp, after.max_hp),
        "the block has to follow its own header: {log:?}"
    );
    assert!(
        !log.iter().any(|e| e.text.contains("Perk Points")),
        "Perk Points are the player's alone: {log:?}"
    );
}

#[test]
fn a_posted_worker_levels_up_in_the_base_log_beside_its_machine() {
    let mut game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = deploy_upgradeable_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    game.world.get_mut::<Experience>(worker).unwrap().xp_to_next = 1;

    for _ in 0..60 {
        game.wait();
    }

    let log = game.message_log(MESSAGE_LOG_CAP);
    let header = log
        .iter()
        .find(|e| e.text.contains("levels up to"))
        .unwrap_or_else(|| panic!("the worker should have levelled at the node: {log:?}"));
    assert_eq!(header.source, MessageSource::Base, "base news, not field");
    assert_eq!(header.kind, MessageKind::LevelUp);
    assert!(
        header.text.starts_with("Your subroutine at the "),
        "the base log names a worker by the machine it is posted to: {:?}",
        header.text
    );
    assert!(
        !log.iter()
            .any(|e| e.text.starts_with("Your subroutine extracted") && e.text.contains("levels")),
        "the level-up is its own line now, not a tail on the payout: {log:?}"
    );
    assert!(
        stat_lines(&game).iter().any(|l| l.starts_with("  Max HP ")),
        "the base block reports growth like the two field ones: {log:?}"
    );
}

/// The reason the stat lines carry `before → after` rather than a bare
/// `+1 DEF`. `condense` folds an identical `(kind, source, text)` into the
/// nearest match within `CONDENSE_LOOKBACK`, so two entities gaining the same
/// amount of the same stat in one fight would collapse into one history row
/// under whichever announced first — deleting the other's line outright.
#[test]
fn two_entities_gaining_the_same_stat_stay_two_history_rows() {
    let level_up = |text: &str| LogLine {
        kind: MessageKind::LevelUp,
        source: MessageSource::Field,
        text: text.to_string(),
    };
    // Built through the shipping formatter, not from hand-written strings: a
    // format that went back to a bare "+1 DEF" has to fail here, and it only
    // can if these lines are the ones the game would really push. Both
    // entities gain exactly one point of DEF — the collision case.
    let player = stat_block(&[
        StatRow::new("Max HP", 108, 120),
        StatRow::new("DEF", 11, 12),
    ]);
    let companion = stat_block(&[StatRow::new("Max HP", 46, 57), StatRow::new("DEF", 8, 9)]);
    let mut lines = vec![level_up("You gain 40 XP and reach level 5!")];
    lines.extend(player.iter().map(|l| level_up(l)));
    lines.push(level_up("Scrapper gains 20 XP and levels up to 3!"));
    lines.extend(companion.iter().map(|l| level_up(l)));
    // Both DEF lines sit inside the lookback window of each other, which is
    // what a bare "+1 DEF" pair would fold across.
    assert!(lines.len() <= CONDENSE_LOOKBACK * 2);

    let entries = condense(&lines);
    assert_eq!(
        entries.len(),
        6,
        "each entity's block must stay its own: {entries:?}"
    );
    assert!(
        entries.iter().all(|e| e.repeats == 1),
        "nothing here is a repeat: {entries:?}"
    );
}
