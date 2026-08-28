//! The history screen's fold of repeated lines — see `resources::condense`.

use super::support::test_assets_dir;
use crate::resources::{
    CONDENSE_LOOKBACK, DifficultyMode, LogEntry, LogLine, MessageKind, MessageSource, SwingOutcome,
    condense,
};

fn line(kind: MessageKind, source: MessageSource, text: &str) -> LogLine {
    LogLine {
        kind,
        source,
        text: text.to_string(),
        outcome: None,
    }
}

fn info(lines: &[&str]) -> Vec<LogLine> {
    lines
        .iter()
        .map(|l| line(MessageKind::Info, MessageSource::Field, l))
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
    lines.extend(
        filler
            .iter()
            .map(|f| line(MessageKind::Info, MessageSource::Field, f)),
    );
    lines.extend(info(&["repeated"]));
    assert_eq!(
        condense(&lines).first().map(|e| e.repeats),
        Some(2),
        "{CONDENSE_LOOKBACK} entries back is still in reach"
    );

    let filler: Vec<String> = (0..CONDENSE_LOOKBACK).map(|i| i.to_string()).collect();
    let mut lines = info(&["repeated"]);
    lines.extend(
        filler
            .iter()
            .map(|f| line(MessageKind::Info, MessageSource::Field, f)),
    );
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
        line(MessageKind::Info, MessageSource::Field, "the same words"),
        line(MessageKind::Outcome, MessageSource::Field, "the same words"),
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

/// Source is part of the key for the same reason kind is. The two channels
/// are read separately, so a line the base produced and an identical line the
/// field produced are two events even though they read alike.
#[test]
fn the_same_text_from_two_sources_stays_two_entries() {
    let lines = vec![
        line(MessageKind::Info, MessageSource::Field, "it comes apart"),
        line(MessageKind::Info, MessageSource::Base, "it comes apart"),
    ];
    let entries = condense(&lines);
    assert_eq!(
        shape(&entries),
        [("it comes apart", 1), ("it comes apart", 1)]
    );
    assert_eq!(entries[0].source, MessageSource::Field);
    assert_eq!(entries[1].source, MessageSource::Base);
}

/// Field is the default, so the ~130 log calls that predate the split keep
/// their meaning without being touched.
#[test]
fn an_ordinary_log_line_is_field_sourced() {
    let mut game = crate::Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.log("you strike the sentinel");
    let last = game.message_log(1);
    assert_eq!(last[0].source, MessageSource::Field);
}

/// The two axes are orthogonal: tagging a line base must not disturb the kind
/// that `retain_outcomes_since_battle` and the colour table read.
#[test]
fn a_base_line_keeps_its_kind() {
    let mut game = crate::Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.log_base_kind(MessageKind::Raid, "a raid strikes the Fabricator");
    let last = game.message_log(1);
    assert_eq!(last[0].kind, MessageKind::Raid);
    assert_eq!(last[0].source, MessageSource::Base);
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

/// A refusal reaches the log under its own kind, so the colour table can
/// paint it as a refusal and `retain_outcomes_since_battle` can drop it.
#[test]
fn a_refusal_lands_in_the_log_under_its_own_kind() {
    let mut game = crate::Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.note_refusal("Requires Zone 3 first.");
    let last = game.message_log(1);
    assert_eq!(last[0].kind, MessageKind::Refusal);
    assert_eq!(last[0].source, MessageSource::Field);
    assert_eq!(last[0].text, "Requires Zone 3 first.");
}

/// The one rule that is not obvious: a refusal is silent while a fight is
/// open. `MessageLog::since_round` slices the battle pane by *position* and
/// the reveal counts *raw* lines, so a line pushed from a battle submenu
/// would both appear mid-narration and swallow a keypress' worth of reveal.
/// The player still sees the refusal — it is on the popup they typed into.
#[test]
fn a_refusal_stays_out_of_the_log_during_a_battle() {
    let mut game = crate::Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    super::support::start_battle_with_a_wild_program(&mut game);
    let before = game.message_log(crate::MESSAGE_LOG_CAP).len();

    game.note_refusal("Not enough Power.");

    assert_eq!(
        game.message_log(crate::MESSAGE_LOG_CAP).len(),
        before,
        "a refusal must not enter the log while a battle is open"
    );
}

/// `SwingOutcome` collapses `battle::AttackOutcome` to just the band — no
/// damage figure, no `FumbleRung` detail — since the only reader is a
/// per-swing sound cue picking one of a handful of clips, and every one of
/// those clips is what a fumble sounds like regardless of which rung it
/// landed on.
#[test]
fn swing_outcome_reduces_every_attack_outcome_band() {
    use crate::battle::{AttackOutcome, FumbleRung};
    assert_eq!(
        SwingOutcome::from(AttackOutcome::Crit { dmg: 9 }),
        SwingOutcome::Crit
    );
    assert_eq!(
        SwingOutcome::from(AttackOutcome::Hit { dmg: 4 }),
        SwingOutcome::Hit
    );
    assert_eq!(SwingOutcome::from(AttackOutcome::Miss), SwingOutcome::Miss);
    for rung in [
        FumbleRung::Exposed,
        FumbleRung::Recoil { dmg: 1 },
        FumbleRung::Opening { dmg: 0 },
        FumbleRung::Crash,
    ] {
        assert_eq!(
            SwingOutcome::from(AttackOutcome::Fumble(rung)),
            SwingOutcome::Fumble,
            "every fumble rung reduces to the same cue: {rung:?}"
        );
    }
}

/// `Game::log_swing` is the only `log_*` variant that stamps a line with a
/// `SwingOutcome` — plain narration stays `None`, which
/// `an_ordinary_log_line_is_field_sourced` above already covers for `log`.
#[test]
fn log_swing_stamps_the_line_with_its_outcome() {
    let mut game = crate::Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.log_swing(
        MessageKind::PartyDamage,
        crate::battle::AttackOutcome::Crit { dmg: 12 },
        "you tear it clean through",
    );
    let last = game.message_log(1);
    assert_eq!(last[0].outcome, Some(SwingOutcome::Crit));
    assert_eq!(last[0].kind, MessageKind::PartyDamage);
    assert_eq!(last[0].source, MessageSource::Field);
}
