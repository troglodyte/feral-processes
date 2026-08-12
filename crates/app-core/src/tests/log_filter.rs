//! The map log pane's base/field filter — see `LogFilter`, `pane_rows` and
//! `App::visible_log`.

use super::support::*;
use crate::*;
use feral_processes_engine::{LogLine, MessageKind, MessageSource};

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

fn texts(rows: Vec<LogLine>) -> Vec<String> {
    rows.into_iter().map(|e| e.text).collect()
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
