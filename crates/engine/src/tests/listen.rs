//! `Z` in the Stack — listening for what the frame still has in it.
//!
//! The key is deliberately named by nothing on screen; see
//! `crates/engine/EASTER_EGGS.md`.

use super::support::*;
use crate::crash_logs::CrashLogDb;
use crate::resources::Trace;
use crate::stack::{CellKind, Dir};
use crate::tuning::TRACE_PER_LISTEN;
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn trace_of(game: &Game) -> u32 {
    game.world.resource::<Trace>().0
}

/// Every cell of the current frame that `Game::listen` should count as
/// unspent. Spelled out here rather than reached for in the engine on
/// purpose: a fixture that shared the production predicate could not catch
/// the production predicate drifting.
fn unspent(game: &Game) -> Vec<(i32, i32)> {
    let pos = game.stack_pos().expect("not underground");
    let level = frame(game);
    every_cell(&level)
        .filter(|&cell| match level.cell(cell.0, cell.1) {
            CellKind::Cache => game.cache_unopened(pos, cell),
            CellKind::SealedDoor => !game.seal_open(pos, cell),
            CellKind::Orphan => game.orphan_present(pos, cell),
            CellKind::Lair => !game.lair_cleared(pos),
            _ => false,
        })
        .collect()
}

/// Marks everything in the frame spent, so listening has nothing left to
/// find. A frame holds three caches and often an orphan, so emptying the
/// one cache a test walked to is not enough to buy silence.
fn spend_everything(game: &mut Game) {
    let pos = game.stack_pos().expect("not underground");
    let level = frame(game);
    for cell in unspent(game) {
        let memory = game.frame_memory_mut(pos);
        match level.cell(cell.0, cell.1) {
            CellKind::Cache => {
                memory.looted.insert(cell);
            }
            CellKind::SealedDoor => {
                memory.opened.insert(cell);
            }
            CellKind::Orphan => {
                memory.adopted.insert(cell);
            }
            CellKind::Lair => memory.cleared = true,
            _ => {}
        }
    }
}

/// The frame's first cache, and the single open cell beside it. A cache
/// sits in a dead end, so that neighbour is the only place it can be heard
/// from one step away — and `dir` points from the mouth at the cache.
fn a_cache_and_its_mouth(game: &Game) -> ((i32, i32), (i32, i32), Dir) {
    let level = frame(game);
    let cache = every_cell(&level)
        .find(|&(x, y)| level.cell(x, y) == CellKind::Cache)
        .expect("every frame hides at least one cache");
    let (mouth, dir) = [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .find_map(|dir| {
            let (dx, dy) = dir.delta();
            let neighbour = (cache.0 + dx, cache.1 + dy);
            // The neighbour lies `dir` from the cache, so looking back at
            // the cache from there means facing the opposite way.
            level
                .walkable(neighbour.0, neighbour.1)
                .then_some((neighbour, dir.turn_left().turn_left()))
        })
        .expect("a dead end has exactly one way in");
    (cache, mouth, dir)
}

/// Listens and returns the reading. Found by taking the lines pushed by
/// this call rather than the last line in the log, because `raise_trace`
/// can log a band crossing after the reading.
fn listen(game: &mut Game) -> String {
    let before = game.message_log(usize::MAX).len();
    game.listen()
        .expect("listening underground should be allowed");
    let after = game.message_log(usize::MAX);
    after
        .get(before)
        .expect("listening should have logged a reading")
        .text
        .clone()
}

#[test]
fn listening_names_the_direction_and_distance_of_an_unopened_cache() {
    let mut game = game();
    descend(&mut game);
    let (cache, mouth, dir) = a_cache_and_its_mouth(&game);
    assert_eq!(
        unspent(&game)
            .into_iter()
            .filter(|&c| manhattan(c, mouth) <= 1)
            .collect::<Vec<_>>(),
        vec![cache],
        "the fixture wants the cache to be the only thing one step from the mouth"
    );
    stand_at(&mut game, mouth, dir);
    let before = game.current_tick();

    let reading = listen(&mut game);

    assert!(
        reading.contains("ahead"),
        "listening at a cache mouth pointed somewhere else: {reading}"
    );
    assert!(
        reading.contains("1 step"),
        "the reading should carry the Manhattan distance: {reading}"
    );
    assert!(
        game.current_tick() > before,
        "listening should have cost a turn"
    );
}

fn manhattan(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

#[test]
fn the_same_cache_reads_as_a_different_bearing_from_each_facing() {
    let mut game = game();
    descend(&mut game);
    let (_, mouth, dir) = a_cache_and_its_mouth(&game);

    // A bearing off the party's own facing, not off compass north: turning
    // in place moves the cache around you. Each of the four is asserted by
    // name, because two facings merely *differing* would still pass with
    // the rotation's sign inverted.
    for (facing, expected) in [
        (dir, "ahead"),
        (dir.turn_left(), "to your right"),
        (dir.turn_right(), "to your left"),
        (dir.turn_left().turn_left(), "behind"),
    ] {
        stand_at(&mut game, mouth, facing);
        let reading = listen(&mut game);
        assert!(
            reading.contains(expected),
            "facing {facing:?} the cache should read {expected}: {reading}"
        );
    }
}

#[test]
fn a_swept_frame_reports_silence() {
    let mut game = game();
    descend(&mut game);
    let (_, mouth, dir) = a_cache_and_its_mouth(&game);
    stand_at(&mut game, mouth, dir);
    spend_everything(&mut game);

    let reading = listen(&mut game);

    assert!(
        !reading.contains("ahead") && !reading.contains("behind") && !reading.contains("to your"),
        "a swept frame should have no bearing left to give: {reading}"
    );
    assert!(
        unspent(&game).is_empty(),
        "the fixture failed to spend the frame"
    );
}

#[test]
fn the_trace_charge_lands_whether_or_not_anything_is_heard() {
    let mut game = game();
    descend(&mut game);
    let (_, mouth, dir) = a_cache_and_its_mouth(&game);
    stand_at(&mut game, mouth, dir);

    let before = trace_of(&game);
    listen(&mut game);
    assert_eq!(
        trace_of(&game) - before,
        TRACE_PER_LISTEN,
        "listening with something to hear should charge Trace"
    );

    spend_everything(&mut game);
    let before = trace_of(&game);
    listen(&mut game);
    assert_eq!(
        trace_of(&game) - before,
        TRACE_PER_LISTEN,
        "a frame with nothing left in it still charges — the silence is what the turn bought"
    );
}

#[test]
fn listening_on_the_surface_is_refused_and_costs_nothing() {
    let mut game = game();
    let tick = game.current_tick();

    assert!(
        game.listen().is_err(),
        "there is nothing to listen to on open grid"
    );

    assert_eq!(trace_of(&game), 0, "a refusal raised Trace");
    assert_eq!(game.current_tick(), tick, "a refusal spent a turn");
}

// ---- the crash log ---------------------------------------------------

/// Every rotten cell of the current frame — the two `CellKind`s that read a
/// crash log rather than a bearing.
fn rotten_cells(game: &Game) -> Vec<(i32, i32)> {
    let level = frame(game);
    every_cell(&level)
        .filter(|&(x, y)| matches!(level.cell(x, y), CellKind::Fault | CellKind::Corruption))
        .collect()
}

/// A scratch crash-log directory holding `files` as `(filename, id, lines)`.
/// The caller removes it.
fn crash_log_dir(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "feral_crash_logs_{tag}_{}_{}",
        std::process::id(),
        files.len()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

#[test]
fn standing_on_rot_reads_the_crash_log_instead_of_a_bearing() {
    let mut game = game();
    descend(&mut game);
    let cell = *rotten_cells(&game)
        .first()
        .expect("every frame grows corruption");
    stand_at(&mut game, cell, Dir::North);
    let (trace, tick) = (trace_of(&game), game.current_tick());

    let reading = listen(&mut game);

    assert!(
        !reading.starts_with("You go still"),
        "rotten ground should read its own log, not point at something: {reading}"
    );
    assert!(
        game.world
            .resource::<CrashLogDb>()
            .all()
            .iter()
            .any(|line| *line == reading),
        "the reading came from somewhere other than the shipped crash logs: {reading}"
    );
    assert_eq!(trace_of(&game) - trace, TRACE_PER_LISTEN);
    assert!(game.current_tick() > tick, "reading rot should cost a turn");
}

/// The line is a property of the place, not of how many rolls happened
/// first. This is the test that fails the day someone reaches for
/// `GameRng` to pick it — a draw does not survive a reload, so the same
/// corrupted tile would say something else afterwards.
#[test]
fn the_same_rotten_cell_reads_the_same_line_after_a_reload() {
    let mut game = game();
    descend(&mut game);
    let cell = *rotten_cells(&game)
        .first()
        .expect("every frame grows corruption");
    stand_at(&mut game, cell, Dir::North);
    let before = listen(&mut game);

    let path = std::env::temp_dir().join(format!(
        "feral_crash_log_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        listen(&mut loaded),
        before,
        "the rot said something different after a reload"
    );
}

#[test]
fn the_line_varies_with_the_cell_it_is_read_from() {
    let mut game = game();
    descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let zone = game.world.resource::<ZoneLevel>().0;
    let db = game.world.resource::<CrashLogDb>();

    let readings: std::collections::HashSet<&str> = rotten_cells(&game)
        .into_iter()
        .filter_map(|cell| db.line_for(zone, pos.depth, cell))
        .collect();

    assert!(
        readings.len() > 1,
        "every rotten cell in the frame read the same line — the cell is not in the index"
    );
}

/// `std::fs::read_dir` returns entries in no defined order, so the pooled
/// lines are sorted by id. Without that the same cell reads a different
/// line between runs and across a reload — which is the bug the round-trip
/// test above would only catch by luck. The filenames here are deliberately
/// the reverse of the ids.
#[test]
fn the_pool_is_ordered_by_id_rather_than_by_directory_order() {
    let dir = crash_log_dir(
        "sorted",
        &[
            ("zzz.ron", r#"(id: "aaa", lines: ["first"])"#),
            ("aaa.ron", r#"(id: "zzz", lines: ["second"])"#),
        ],
    );
    let (db, warnings) = CrashLogDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(db.all(), ["first", "second"]);
    assert_eq!(db.line_for(0, 0, (0, 0)), Some("first"));
}

#[test]
fn a_malformed_crash_log_is_skipped_and_the_rest_still_load() {
    let dir = crash_log_dir(
        "malformed",
        &[
            ("good.ron", r#"(id: "good", lines: ["a line"])"#),
            ("broken.ron", "(id: \"broken\", lines: [ oops"),
        ],
    );
    let (db, warnings) = CrashLogDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(db.all(), ["a line"]);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("broken.ron"), "{warnings:?}");
}

/// A crash-log directory with nothing in it is not an error and not a
/// modulo by zero: the rotten cell falls back to the bearing reading.
#[test]
fn an_empty_crash_log_directory_leaves_the_key_working() {
    let dir = crash_log_dir("empty", &[]);
    let (db, warnings) = CrashLogDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(db.all().is_empty());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(db.line_for(3, 2, (5, 7)), None);

    let mut game = game();
    descend(&mut game);
    let cell = *rotten_cells(&game)
        .first()
        .expect("every frame grows corruption");
    stand_at(&mut game, cell, Dir::North);
    game.world.insert_resource(db);

    let reading = listen(&mut game);

    assert!(
        reading.starts_with("You go still"),
        "with no crash logs loaded, rot should fall back to the bearing: {reading}"
    );
}
