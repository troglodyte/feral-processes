//! `Game`'s four alert-board doors, `post_alert`, and the save round trip —
//! see `alerts` for the pure collapse/cap logic exercised there instead.

use super::support::*;
use crate::alerts::{Alert, AlertKind};
use crate::*;

/// A temp path unique to the calling test — `traps.rs`'s reason: a fixed
/// path is shared with the `dev_template` loop, which deletes files mid-run
/// and produces a panic naming the load line rather than the collision.
fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_alerts_{tag}_{}.bin",
        std::process::id()
    ))
}

#[test]
fn a_fresh_game_has_no_alerts() {
    let game = Game::new(19001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.alerts().is_empty());
    assert_eq!(game.unread_alerts(), 0);
}

#[test]
fn post_alert_shows_up_unread_in_the_view() {
    let mut game = Game::new(19002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down.");
    game.post_alert(
        AlertKind::SweepIncoming,
        "sweep",
        "Sensors catch movement at the perimeter.",
    );

    let alerts = game.alerts();
    assert_eq!(alerts.len(), 2);
    assert_eq!(game.unread_alerts(), 2);
    assert!(alerts.iter().all(|a| a.unread));
    // Newest first.
    assert_eq!(alerts[0].kind, AlertKind::SweepIncoming);
    assert_eq!(alerts[1].kind, AlertKind::ProgramDowned);
}

#[test]
fn mark_alerts_read_zeroes_unread_alerts() {
    let mut game = Game::new(19003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down.");
    game.post_alert(AlertKind::DepotsFull, "depots", "Every depot is full.");
    assert_eq!(game.unread_alerts(), 2);

    game.mark_alerts_read();

    assert_eq!(game.unread_alerts(), 0);
    assert!(game.alerts().iter().all(|a| !a.unread));
    // Marking read does not remove anything.
    assert_eq!(game.alerts().len(), 2);
}

#[test]
fn dismiss_alert_removes_exactly_the_indexed_row() {
    let mut game = Game::new(19004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "one");
    game.post_alert(AlertKind::ProgramDowned, "prog-2", "two");
    game.post_alert(AlertKind::ProgramDowned, "prog-3", "three");

    game.dismiss_alert(1);

    let alerts = game.alerts();
    assert_eq!(alerts.len(), 2);
    assert_eq!(alerts[0].text, "three");
    assert_eq!(alerts[1].text, "one");
}

#[test]
fn dismiss_alert_out_of_range_is_a_no_op() {
    let mut game = Game::new(19005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "one");

    game.dismiss_alert(5);

    assert_eq!(game.alerts().len(), 1);
}

/// A real save to a real file and a real load back. **A RON round trip
/// cannot catch a skipped field**; only a save-then-load can, and this repo
/// has shipped a `#[serde(skip)]` that left the round-trip test green.
#[test]
fn alerts_survive_a_save_and_load_with_order_count_and_unread_intact() {
    let mut game = Game::new(19006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down.");
    game.post_alert(
        AlertKind::SweepIncoming,
        "sweep",
        "Sensors catch movement at the perimeter.",
    );
    // Re-posting collapses this one, bumping its count to 2.
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down again.");
    game.mark_alerts_read();
    // Posted after the mark, so this one alone should come back unread.
    game.post_alert(AlertKind::DepotsFull, "depots", "Every depot is full.");

    let before = game.alerts();
    assert_eq!(before.len(), 3);

    let path = save_path("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.alerts(),
        before,
        "order, count and unread all survive the round trip"
    );
    assert_eq!(loaded.unread_alerts(), 1);
}

/// Load re-applies the cap, so a hand-edited or `savetool`-packed save
/// cannot exceed it — see `alerts::cap`'s doc.
#[test]
fn a_save_carrying_more_than_the_cap_loads_capped() {
    let mut game = Game::new(19007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = save_path("cap");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.alerts = (0..(tuning::ALERT_BOARD_CAP + 10))
        .map(|i| Alert {
            kind: AlertKind::ProgramDowned,
            subject: format!("prog-{i}"),
            text: format!("Program {i} is down."),
            count: 1,
            unread: true,
        })
        .collect();
    save::save_to_file(&path, &data).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.alerts().len(), tuning::ALERT_BOARD_CAP);
}
