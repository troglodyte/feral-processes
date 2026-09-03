//! Writing battle records out: the flag, the wire format, and the one
//! documented invariant this feature is deliberately an exception to.

use feral_processes_engine::arena::{OpponentSpec, PlayerSource, Scenario};

use super::support::test_app;
use crate::*;

/// A scratch path per test — the suite runs its cases as concurrent
/// threads, and two of these sharing one file would interleave their
/// records and fail each other's line counts.
fn scratch_telemetry(name: &str) -> PathBuf {
    let path = std::env::temp_dir()
        .join(format!("feral_processes_telemetry_{name}"))
        .join("battles.jsonl");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    path
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// An app on the main menu with both dev gates open, writing to `path`.
///
/// Both are set on the field rather than in the environment, for the reason
/// `app_with_arena` records: `std::env` is process-global and the suite runs
/// in parallel, so setting `FERAL_DEV_LOG` would turn collection on for
/// every other test in flight. `App::new` stays the only reader.
fn app_logging_to(seed: u32, path: PathBuf) -> App {
    let mut app = test_app(seed);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.arena_enabled = true;
    app.telemetry_enabled = true;
    app.telemetry_path = path;
    app
}

fn fight(app: &mut App) {
    app.handle_key(GameKey::Char('r'));
    let session = app.arena.as_mut().unwrap();
    session.seed = 3;
    session.scenario = Scenario {
        player: PlayerSource::Fresh { level: 20, zone: 1 },
        opponents: vec![OpponentSpec {
            species: "sprite".into(),
            count: 1,
        }],
        seed: 3,
        ..Scenario::default()
    };
    app.handle_key(GameKey::Char('f'));
    for _ in 0..500 {
        if !app.mode.is_battle() {
            return;
        }
        app.finish_reveal();
        app.handle_key(match app.mode {
            Mode::Battle => GameKey::Char('A'),
            _ => GameKey::Enter,
        });
    }
    panic!("the fixture never resolved: mode {:?}", app.mode);
}

/// Asserts on the *file*, because the omission is invisible otherwise —
/// and this is what a player's build does, every fight, forever.
#[test]
fn no_telemetry_file_is_written_when_disabled() {
    let path = scratch_telemetry("disabled");
    let mut app = app_logging_to(41, path.clone());
    app.telemetry_enabled = false;

    fight(&mut app);

    assert!(
        !path.exists(),
        "a build without the flag must leave no file at all"
    );
}

/// The carve-out, and the whole point of the writer's placement.
///
/// It sits beside `an_arena_fight_writes_no_save`,
/// `an_arena_fight_writes_no_profile` and
/// `an_arena_loss_writes_no_run_history`, which assert the *opposite* about
/// everything else an arena session could touch. That rule exists so a
/// tester's fight cannot corrupt a save or pay a real profile reward; a
/// dev-only file under `dev-logs/` does neither, and the arena is where
/// this feature is most wanted. The regression to head off is someone
/// folding the flush back inside `after_tick`'s `in_arena()` guard for
/// tidiness.
#[test]
fn an_arena_fight_still_writes_telemetry() {
    let path = scratch_telemetry("arena");
    let mut app = app_logging_to(42, path.clone());

    fight(&mut app);

    assert!(
        path.exists(),
        "the arena is exactly where a hand-played fight needs recording"
    );
    assert!(!lines(&path).is_empty());
}

/// The wire-format assertions the engine could not make: `serde_json` is
/// banned there, so `every_record_kind_round_trips` can only prove the
/// derives are wired. These are against the real written file.
#[test]

fn each_record_is_one_json_line() {
    let path = scratch_telemetry("format");
    let mut app = app_logging_to(43, path.clone());

    fight(&mut app);

    let lines = lines(&path);
    assert!(lines.len() > 2, "a whole fight is more than two records");
    for line in &lines {
        let value: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("{line} is not JSON: {e}"));
        assert!(
            value.get("t").and_then(|t| t.as_str()).is_some(),
            "every line carries the tag a reader dispatches on: {line}"
        );
    }
    assert!(
        lines.iter().any(|l| l.contains("\"t\":\"fight_start\"")),
        "the fight that was played has to be in there"
    );
}

/// The base records reach the same file, and their JSON is what
/// `dev-logs/README.md` documents. **A schema nobody serialized is a
/// schema that is wrong**: every field name here is one an analysis greps
/// for, and the tag is `#[serde(rename_all)]`'s output rather than the
/// variant name.
#[test]
fn a_base_record_writes_the_shape_the_schema_documents() {
    let path = scratch_telemetry("base_format");
    // The writer appends, and the scratch path outlives the process — a
    // leftover from an earlier run would otherwise make the count wrong
    // rather than the shape.
    let _ = std::fs::remove_file(&path);
    crate::app::telemetry::append_records(
        &path,
        &[feral_processes_engine::telemetry::Record::BaseSnapshot {
            tick: 3000,
            zone: 2,
            staff: 4,
            posted: 3,
            machines: 5,
            depots: 1,
            supply: 8,
            draw: 6,
        }],
    )
    .expect("the write");

    let lines = lines(&path);
    assert_eq!(lines.len(), 1, "one record is one line");
    let value: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
    assert_eq!(value["t"], "base_snapshot", "{}", lines[0]);
    for field in [
        "tick", "zone", "staff", "posted", "machines", "depots", "supply", "draw",
    ] {
        assert!(
            value.get(field).is_some(),
            "the schema documents {field} and the record does not carry it: {}",
            lines[0]
        );
    }
}

/// A dev log must never take a run down with it — the same contract
/// `flush_profile_writes` keeps.
#[test]
fn a_failed_telemetry_write_does_not_end_the_run() {
    // A path whose parent is a *file*, so both the create-dir and the open
    // fail however the writer is spelled.
    let blocker = std::env::temp_dir().join("feral_processes_telemetry_blocker");
    std::fs::write(&blocker, b"not a directory").unwrap();
    let mut app = app_logging_to(44, blocker.join("battles.jsonl"));

    fight(&mut app);

    assert_eq!(
        app.mode,
        Mode::ArenaResult,
        "the fight still played out to its result screen"
    );
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("telemetry")),
        "the failure is reported, not swallowed: {:?}",
        app.status_line
    );
    let _ = std::fs::remove_file(&blocker);
}
