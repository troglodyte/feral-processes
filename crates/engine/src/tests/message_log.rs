//! The history screen's fold of repeated lines — see `resources::condense`.

use super::support::test_assets_dir;
use crate::resources::{CONDENSE_LOOKBACK, DifficultyMode, LogEntry, MessageKind, condense};

fn info(lines: &[&str]) -> Vec<(MessageKind, String)> {
    lines
        .iter()
        .map(|l| (MessageKind::Info, (*l).to_string()))
        .collect()
}

fn shape(entries: &[LogEntry]) -> Vec<(&str, usize)> {
    entries
        .iter()
        .map(|e| (e.text.as_str(), e.repeats))
        .collect()
}

#[test]
fn an_unbroken_run_collapses_however_long() {
    let lines = info(&["extracted 2 Data Shard."; 40]);
    assert_eq!(shape(&condense(&lines)), [("extracted 2 Data Shard.", 40)]);
}

#[test]
fn a_line_that_stands_alone_repeats_once() {
    let lines = info(&["A raid strikes the Fabricator."]);
    assert_eq!(
        shape(&condense(&lines)),
        [("A raid strikes the Fabricator.", 1)]
    );
}

#[test]
fn an_empty_log_folds_to_nothing() {
    assert!(condense(&[]).is_empty());
}

/// Two cronjobs each pushing a yield line per cycle — the case adjacency
/// alone would never collapse.
#[test]
fn interleaved_producers_inside_the_window_collapse() {
    let lines = info(&["shard", "scrap", "shard", "scrap", "shard", "scrap"]);
    assert_eq!(shape(&condense(&lines)), [("shard", 3), ("scrap", 3)]);
}

/// The same warning twice with a run of unrelated lines between reads as two
/// events, not one doubled event.
#[test]
fn duplicates_beyond_the_window_stay_separate() {
    let mut lines = info(&["power reserves critical"]);
    lines.extend(info(&["a", "b", "c", "d", "e"]));
    lines.extend(info(&["power reserves critical"]));
    let entries = condense(&lines);
    assert_eq!(entries.first().map(|e| e.repeats), Some(1));
    assert_eq!(
        entries.last().map(|e| e.text.as_str()),
        Some("power reserves critical")
    );
    assert_eq!(entries.last().map(|e| e.repeats), Some(1));
}

/// The window is measured in emitted entries, so exactly `CONDENSE_LOOKBACK`
/// distinct entries between two copies still folds them together — one more
/// does not.
#[test]
fn the_window_is_counted_in_entries() {
    let filler: Vec<String> = (0..CONDENSE_LOOKBACK - 1).map(|i| i.to_string()).collect();
    let mut lines = info(&["repeated"]);
    lines.extend(filler.iter().map(|f| (MessageKind::Info, f.clone())));
    lines.extend(info(&["repeated"]));
    assert_eq!(
        condense(&lines).first().map(|e| e.repeats),
        Some(2),
        "{CONDENSE_LOOKBACK} entries back is still in reach"
    );

    let filler: Vec<String> = (0..CONDENSE_LOOKBACK).map(|i| i.to_string()).collect();
    let mut lines = info(&["repeated"]);
    lines.extend(filler.iter().map(|f| (MessageKind::Info, f.clone())));
    lines.extend(info(&["repeated"]));
    assert_eq!(
        condense(&lines).len(),
        CONDENSE_LOOKBACK + 2,
        "one entry further back is out of reach"
    );
}

/// Kind is part of the key: the same sentence as narration and as a result is
/// styled differently and means something different.
#[test]
fn the_same_text_under_two_kinds_stays_two_entries() {
    let lines = vec![
        (MessageKind::Info, "the same words".to_string()),
        (MessageKind::Outcome, "the same words".to_string()),
    ];
    let entries = condense(&lines);
    assert_eq!(
        shape(&entries),
        [("the same words", 1), ("the same words", 1)]
    );
    assert_eq!(entries[0].kind, MessageKind::Info);
    assert_eq!(entries[1].kind, MessageKind::Outcome);
}

/// A fold anchors at the first occurrence, so the screen's order is the order
/// things first happened.
#[test]
fn a_fold_anchors_at_the_first_occurrence() {
    let lines = info(&["first", "second", "first"]);
    assert_eq!(shape(&condense(&lines)), [("first", 2), ("second", 1)]);
}

#[test]
fn message_history_folds_what_was_pushed() {
    let mut game = crate::Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for _ in 0..5 {
        game.log("a cronjob yields nothing");
    }
    game.log_kind(MessageKind::Raid, "a raid strikes");
    let history = game.message_history(crate::MESSAGE_LOG_CAP);
    let tail = &history[history.len() - 2..];
    assert_eq!(
        shape(tail),
        [("a cronjob yields nothing", 5), ("a raid strikes", 1)]
    );
}
