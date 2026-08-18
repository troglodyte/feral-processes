//! The map log pane's base/field filter — see `LogFilter`, `pane_rows` and
//! `App::visible_log`.

use super::support::*;
use crate::*;
use feral_processes_engine::{LogEntry, LogLine, MessageKind, MessageSource};

fn field(text: &str) -> LogLine {
    LogLine {
        kind: MessageKind::Info,
        source: MessageSource::Field,
        text: text.to_string(),
    }
}

fn base(text: &str) -> LogLine {
    LogLine {
        kind: MessageKind::Raid,
        source: MessageSource::Base,
        text: text.to_string(),
    }
}

fn outcome(text: &str) -> LogLine {
    LogLine {
        kind: MessageKind::Outcome,
        source: MessageSource::Field,
        text: text.to_string(),
    }
}

fn texts(rows: Vec<LogEntry>) -> Vec<String> {
    rows.into_iter().map(|e| e.text).collect()
}

fn shape(rows: &[LogEntry]) -> Vec<(&str, usize)> {
    rows.iter().map(|e| (e.text.as_str(), e.repeats)).collect()
}

#[test]
fn the_pane_starts_unfiltered() {
    let app = test_app(140);
    assert_eq!(app.log_filter, LogFilter::All);
}

#[test]
fn f_cycles_all_field_base_and_back() {
    let mut app = test_app(141);
    app.handle_key(GameKey::Char('f'));
    assert_eq!(app.log_filter, LogFilter::Field);
    app.handle_key(GameKey::Char('f'));
    assert_eq!(app.log_filter, LogFilter::Base);
    app.handle_key(GameKey::Char('f'));
    assert_eq!(app.log_filter, LogFilter::All);
}

/// The pane's header lists `LogFilter::ALL` and the key walks `next`, so a
/// disagreement would draw the options in an order the key does not follow.
#[test]
fn the_header_order_is_the_cycle_order() {
    let mut walked = Vec::new();
    let mut filter = LogFilter::ALL[0];
    for _ in 0..LogFilter::ALL.len() {
        walked.push(filter);
        filter = filter.next();
    }
    assert_eq!(walked, LogFilter::ALL);
    assert_eq!(filter, LogFilter::ALL[0], "the cycle has to close");
}

/// Cycling the filter is a view change, not an action — it must not advance
/// the world, or reading the log would cost turns.
#[test]
fn cycling_the_filter_does_not_tick() {
    let mut app = test_app(142);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('f'));
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

#[test]
fn the_field_filter_hides_base_lines() {
    let lines = vec![field("a field line"), base("a base line")];
    let shown = texts(pane_rows(&lines, 0, LogFilter::Field, 40));
    assert_eq!(shown, ["a field line"]);
}

#[test]
fn the_base_filter_hides_field_lines() {
    let lines = vec![field("a field line"), base("a base line")];
    let shown = texts(pane_rows(&lines, 0, LogFilter::Base, 40));
    assert_eq!(shown, ["a base line"]);
}

#[test]
fn the_all_filter_hides_nothing() {
    let lines = vec![field("a field line"), base("a base line")];
    let shown = texts(pane_rows(&lines, 0, LogFilter::All, 40));
    assert_eq!(shown, ["a field line", "a base line"]);
}

/// The pane draws its tail, so a capacity smaller than the log keeps the
/// newest lines — and the filter is applied *before* that cut, or a screenful
/// of base chatter would leave the field pane blank.
#[test]
fn the_pane_fills_to_capacity_with_lines_that_pass_the_filter() {
    let mut lines = Vec::new();
    for i in 0..30 {
        lines.push(base(&format!("base {i}")));
        lines.push(field(&format!("field {i}")));
    }
    let shown = texts(pane_rows(&lines, 0, LogFilter::Field, 5));
    assert_eq!(shown.len(), 5, "the pane must still fill");
    assert!(
        shown.iter().all(|t| t.starts_with("field ")),
        "got {shown:?}"
    );
    assert_eq!(shown.last().unwrap(), "field 29", "newest line last");
}

/// `App::hidden_log_lines` counts *raw* tail lines a battle has yet to scroll
/// in, so the chop has to happen before the filter thins the list — cutting
/// the same count out of a filtered list would eat revealed lines instead.
#[test]
fn the_unrevealed_battle_tail_is_chopped_before_the_filter_applies() {
    let lines = vec![
        field("older field line"),
        base("base chatter"),
        field("not revealed yet"),
    ];
    let shown = texts(pane_rows(&lines, 1, LogFilter::Field, 40));
    assert_eq!(shown, ["older field line"]);
}

/// The battle pane narrates one fight. A background system logging into the
/// same range while the party is out fighting — a sweep on the base, a
/// machine clogging — is not part of that narration, and `since_round` slices
/// the log by position, so nothing upstream of here can tell the two apart.
#[test]
fn the_battle_pane_leaves_base_news_out() {
    let lines = vec![
        field("── round 1 ──"),
        base("The Fabricator loses 8 Durability to a GC Entropy Sweep!"),
        field("You unleash a data strike for 2 damage."),
    ];
    let shown = texts(battle_rows(&lines, 3));
    assert_eq!(
        shown,
        ["── round 1 ──", "You unleash a data strike for 2 damage."]
    );
}

/// The reveal counts *raw* lines — `App::hidden_log_lines` chops the same
/// figure off the map pane's tail — so the truncation has to happen before
/// the source filter. Filtering first would let the narration outrun the
/// pacing by however much base chatter had landed in the round.
#[test]
fn the_battle_pane_truncates_before_it_filters() {
    let lines = vec![
        field("── round 1 ──"),
        base("The Mining Node is clogged."),
        field("not revealed yet"),
    ];
    let shown = texts(battle_rows(&lines, 2));
    assert_eq!(shown, ["── round 1 ──"]);
}

/// The header's "there is more you aren't seeing" figure. Zero when nothing
/// is filtered out, so the pane says nothing when there is nothing to say.
#[test]
fn nothing_is_reported_hidden_while_unfiltered() {
    let lines = vec![field("a field line"), base("a base line")];
    assert_eq!(filtered_out_count(&lines, LogFilter::All), 0);
}

#[test]
fn the_hidden_count_reports_the_channel_being_suppressed() {
    let lines = vec![base("base 0"), base("base 1"), base("base 2"), field("f")];
    assert_eq!(filtered_out_count(&lines, LogFilter::Field), 3);
    assert_eq!(filtered_out_count(&lines, LogFilter::Base), 1);
}

/// A round that kills seven programs pushes the same `Outcome` sentence
/// seven times, and reading it seven times says nothing the count does not.
///
/// Folded here rather than in storage, and after the truncation rather than
/// before it: the reveal paces on *raw* lines, so the count ticks up as the
/// kills scroll in and `App::hidden_log_lines` still agrees with the map.
#[test]
fn the_battle_pane_folds_a_repeated_line() {
    let mut lines = vec![field("── round 1 ──")];
    lines.extend(
        std::iter::repeat_with(|| outcome("The rogue program crashes and deletes itself!")).take(7),
    );
    assert_eq!(
        shape(&battle_rows(&lines, lines.len())),
        [
            ("── round 1 ──", 1),
            ("The rogue program crashes and deletes itself!", 7),
        ]
    );
}

/// Kills arrive interleaved with the line announcing who steps up behind
/// them, so an adjacent-runs-only fold would collapse nothing at all. This
/// is the whole reason the pane borrows `condense`'s lookback window rather
/// than folding neighbours.
#[test]
fn the_battle_pane_folds_through_an_interleaved_line() {
    let mut lines = Vec::new();
    for _ in 0..3 {
        lines.push(outcome("The rogue program crashes and deletes itself!"));
        lines.push(field("Another rogue program from the pack engages!"));
    }
    assert_eq!(
        shape(&battle_rows(&lines, lines.len())),
        [
            ("The rogue program crashes and deletes itself!", 3),
            ("Another rogue program from the pack engages!", 3),
        ]
    );
}

/// The map pane folds too — a finished fight keeps its `Outcome` lines, so
/// the seven kills land there as well once the map is back.
#[test]
fn the_map_pane_folds_a_repeated_line() {
    let mut lines = vec![field("You step east.")];
    lines.extend(
        std::iter::repeat_with(|| outcome("The rogue program crashes and deletes itself!")).take(7),
    );
    assert_eq!(
        shape(&pane_rows(&lines, 0, LogFilter::All, 40)),
        [
            ("You step east.", 1),
            ("The rogue program crashes and deletes itself!", 7),
        ]
    );
}

/// The fold comes before the capacity cut, so a burst of repeats costs the
/// pane one row rather than a screenful — the older lines it would otherwise
/// have pushed out are still in reach.
#[test]
fn the_map_pane_folds_before_the_capacity_cut() {
    let mut lines = vec![field("older field line")];
    lines.extend(
        std::iter::repeat_with(|| outcome("The rogue program crashes and deletes itself!")).take(7),
    );
    let shown = texts(pane_rows(&lines, 0, LogFilter::All, 2));
    assert_eq!(
        shown,
        [
            "older field line",
            "The rogue program crashes and deletes itself!"
        ]
    );
}
