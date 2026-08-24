//! Periodic caravan traders: the catalogue, the derived schedule and shelf,
//! the journey in and out, and what the player may take off it.

use super::support::*;
use crate::caravans::{CaravanDb, CaravanDef};

fn caravans_dir() -> std::path::PathBuf {
    test_assets_dir().join("caravans")
}

fn shipped_db() -> CaravanDb {
    let (db, warnings) = CaravanDb::load_dir(&caravans_dir()).unwrap();
    assert!(warnings.is_empty(), "shipped caravans warned: {warnings:?}");
    db
}

#[test]
fn the_shipped_directory_loads_clean() {
    let db = shipped_db();
    assert!(
        db.all().count() >= 2,
        "two traders is the minimum that makes 'which trader visits' mean anything"
    );
}

/// A broken file costs the game that one trader and nothing else. Written to
/// a scratch dir — mutating `assets/` is how a timed-out run once left a
/// shipped item edited.
#[test]
fn a_malformed_file_is_skipped_with_one_warning() {
    let dir = scratch_assets_dir("caravans_malformed");
    std::fs::create_dir_all(&*dir).unwrap();
    for entry in std::fs::read_dir(caravans_dir()).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|e| e.to_str()) == Some("ron") {
            std::fs::copy(entry.path(), &*dir.join(entry.file_name())).unwrap();
        }
    }
    let shipped = std::fs::read_dir(&*dir).unwrap().count();
    std::fs::write(&*dir.join("broken.ron"), "( id: \"nope\"").unwrap();

    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();

    assert_eq!(warnings.len(), 1, "one bad file, one warning: {warnings:?}");
    assert_eq!(
        db.all().count(),
        shipped,
        "every other file in the directory still loaded"
    );
}

/// A def the schema refuses is skipped the same way a syntactically broken
/// one is — `complaint` runs at load so an unusable trader is a startup
/// warning rather than an empty shelf nobody can explain.
#[test]
fn a_def_with_no_rows_or_no_weights_is_refused() {
    let dir = scratch_assets_dir("caravans_invalid");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(
        &*dir.join("rowless.ron"),
        "(id: \"a\", name: \"A\", description: \"d\", glyph: 'Ω', color: DarkGreen, rows: 0, \
         weights: (gear: 1), min_zone: 1, max_zone: 9)",
    )
    .unwrap();
    std::fs::write(
        &*dir.join("weightless.ron"),
        "(id: \"b\", name: \"B\", description: \"d\", glyph: 'Ω', color: DarkGreen, rows: 3, \
         weights: (), min_zone: 1, max_zone: 9)",
    )
    .unwrap();
    std::fs::write(
        &*dir.join("inverted.ron"),
        "(id: \"c\", name: \"C\", description: \"d\", glyph: 'Ω', color: DarkGreen, rows: 3, \
         weights: (gear: 1), min_zone: 9, max_zone: 1)",
    )
    .unwrap();

    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();

    assert_eq!(db.all().count(), 0, "none of the three is usable");
    assert_eq!(warnings.len(), 3, "each says why: {warnings:?}");
}

#[test]
fn for_zone_keeps_only_the_window_and_sorts_by_id() {
    let dir = scratch_assets_dir("caravans_window");
    std::fs::create_dir_all(&*dir).unwrap();
    // Filenames deliberately in the opposite order to the ids, so a walk
    // that returned directory order rather than id order comes out wrong.
    for (file, id, lo, hi) in [
        ("z.ron", "aardvark", 1u32, 3u32),
        ("m.ron", "middle", 3, 5),
        ("a.ron", "zulu", 6, 9),
    ] {
        std::fs::write(
            &*dir.join(file),
            format!(
                "(id: \"{id}\", name: \"N\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
                 rows: 3, weights: (gear: 1), min_zone: {lo}, max_zone: {hi})"
            ),
        )
        .unwrap();
    }
    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");

    let ids = |zone| -> Vec<String> {
        db.for_zone(zone)
            .into_iter()
            .map(|d| d.id.clone())
            .collect()
    };

    assert_eq!(ids(1), vec!["aardvark"], "below the second window");
    assert_eq!(
        ids(3),
        vec!["aardvark", "middle"],
        "both windows contain 3, and the answer is in id order"
    );
    assert_eq!(ids(4), vec!["middle"]);
    assert!(ids(10).is_empty(), "past every window");
}

/// A census over the real directory. What it holds is what `complaint`
/// refuses, asserted here as well because `complaint` skipping a file is
/// silent to anyone reading the shipped set.
#[test]
fn every_shipped_caravan_is_stockable() {
    for def in shipped_db().all() {
        let CaravanDef {
            id,
            rows,
            weights,
            min_zone,
            max_zone,
            ..
        } = def;
        assert!(*rows >= 1, "{id} would stand there with nothing to sell");
        assert!(weights.gear + weights.routines + weights.programs + weights.materials > 0);
        assert!(min_zone <= max_zone, "{id}'s window is inverted");
    }
}

// ---------------------------------------------------------------------------
// The derived schedule
// ---------------------------------------------------------------------------

use crate::game::caravan::CaravanVisit;
use crate::resources::{GameClock, GameRng, Locale, ZoneLevel};
use crate::tuning::{
    CARAVAN_ARRIVAL_JITTER_TICKS, CARAVAN_STAY_TICKS, CARAVAN_VISIT_INTERVAL_TICKS,
};
use crate::{DifficultyMode, Game, Glyph, GlyphColor, Position, Structure};

fn fresh() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Stands a structure of `kind` at an absolute base-space cell, bypassing the
/// build rules — what is under test is what a standing Market *enables*.
fn deploy(game: &mut Game, kind: &str, x: i32, y: i32) -> crate::Entity {
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            Glyph {
                ch: '!',
                color: GlyphColor::Yellow,
            },
        ))
        .id()
}

/// A base with a Home, an iso Market and the party standing in it — the
/// state every caravan question is asked from.
fn based(game: &mut Game) {
    game.lay_starting_pocket();
    deploy(game, "home", 0, 0);
    deploy(game, "market", 2, 0);
    stand_in_base_at(game, 1, 1);
}

fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

/// One visit per interval, exactly: every interval's arrival lands inside its
/// own interval and its whole stay finishes before the next one opens.
#[test]
fn exactly_one_visit_falls_in_each_interval() {
    let mut game = fresh();
    based(&mut game);
    let interval = CARAVAN_VISIT_INTERVAL_TICKS;

    for visit in 0..40u64 {
        let v = game.visit_at(visit).expect("a trader for zone 1");
        assert_eq!(v.visit, visit);
        assert!(
            v.arrival_tick >= visit * interval,
            "visit {visit} arrived before its own interval opened"
        );
        assert!(
            v.depart_tick <= (visit + 1) * interval,
            "visit {visit} was still standing when visit {} opened, so two \
             could be due at once",
            visit + 1
        );
    }
}

/// A constant jitter passes a bounds-only check, so the spread is asserted
/// too — that is the half that says the arrival is actually unpredictable.
#[test]
fn arrival_jitter_is_bounded_and_actually_varies() {
    let mut game = fresh();
    based(&mut game);
    let interval = CARAVAN_VISIT_INTERVAL_TICKS;

    let offsets: std::collections::BTreeSet<u64> = (0..60u64)
        .map(|visit| {
            let v = game.visit_at(visit).unwrap();
            v.arrival_tick - visit * interval
        })
        .collect();

    assert!(
        offsets.iter().all(|&o| o < CARAVAN_ARRIVAL_JITTER_TICKS),
        "an offset escaped the jitter window: {offsets:?}"
    );
    assert!(
        offsets.len() > 1,
        "every visit arrived at the same offset, so the jitter is a constant"
    );
}

/// Consecutive visits must not read as the same visit twice. All three draws
/// are folded apart, so a `%` reduction — which reads little but the low bits
/// the final multiply never disturbs — shows up here as neighbours agreeing.
#[test]
fn consecutive_visits_differ() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 4);

    let mut same = 0;
    for visit in 0..80u64 {
        let a = game.visit_at(visit).unwrap();
        let b = game.visit_at(visit + 1).unwrap();
        let a_offset = a.arrival_tick - visit * CARAVAN_VISIT_INTERVAL_TICKS;
        let b_offset = b.arrival_tick - (visit + 1) * CARAVAN_VISIT_INTERVAL_TICKS;
        if a.def_id == b.def_id && a_offset == b_offset && a.bearing == b.bearing {
            same += 1;
        }
    }
    assert_eq!(same, 0, "{same} consecutive pairs were indistinguishable");
}

/// The reducer, pinned by the fault it exists to stop.
///
/// `derive::index` reads the *high* bits; `%` reads the low ones the final
/// multiply provably never disturbs, and on a pool the size of `BEARINGS`
/// that makes the sequence **literally periodic** — measured at period 8,
/// so a player who watched four visits could name the fifth's direction.
/// The distribution is uniform either way, which is why this asserts on
/// period rather than on balance.
#[test]
fn the_bearing_sequence_does_not_cycle() {
    let mut game = fresh();
    based(&mut game);
    let bearings: Vec<u8> = (0..64u64)
        .map(|v| game.visit_at(v).unwrap().bearing)
        .collect();

    for period in 1..=16usize {
        let cycles = bearings.windows(period + 1).all(|w| w[0] == w[period]);
        assert!(
            !cycles,
            "the bearing repeats every {period} visits: {bearings:?}"
        );
    }
}

/// The gate is a counter to stand beside, and it is asked of
/// `StructureDef::trade` rather than of a hardcoded id.
#[test]
fn no_market_means_no_visit() {
    let mut game = fresh();
    game.lay_starting_pocket();
    deploy(&mut game, "home", 0, 0);
    stand_in_base_at(&mut game, 1, 1);
    let arrival = game.visit_at(0).unwrap().arrival_tick;
    set_tick(&mut game, arrival);

    assert_eq!(
        game.scheduled_visit(),
        None,
        "a base with nowhere to trade has nothing to visit"
    );

    deploy(&mut game, "market", 2, 0);
    assert!(
        game.scheduled_visit().is_some(),
        "a Market standing is the whole gate"
    );
}

/// Outside the arrival-to-departure window there is no open visit, however
/// many are scheduled.
#[test]
fn a_visit_is_open_only_inside_its_own_window() {
    let mut game = fresh();
    based(&mut game);
    let v = game.visit_at(0).unwrap();

    set_tick(&mut game, v.arrival_tick.saturating_sub(1));
    if v.arrival_tick > 0 {
        assert_eq!(game.scheduled_visit(), None, "one tick early");
    }
    set_tick(&mut game, v.arrival_tick);
    assert_eq!(game.scheduled_visit(), Some(v.clone()), "on the tick");
    set_tick(&mut game, v.depart_tick - 1);
    assert!(
        game.scheduled_visit().is_some(),
        "the last tick of the stay"
    );
    set_tick(&mut game, v.depart_tick);
    assert_eq!(game.scheduled_visit(), None, "packed up");
    assert_eq!(
        v.depart_tick - v.arrival_tick,
        CARAVAN_STAY_TICKS,
        "the stay is the tuned one"
    );
}

/// The whole point of deriving it: no save field, and the same answer on the
/// other side of a reload.
#[test]
fn the_schedule_survives_a_save_and_load() {
    let dir = scratch_assets_dir("caravan_schedule_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = fresh();
    based(&mut game);
    set_tick(&mut game, 5_000);
    let before: Vec<CaravanVisit> = (0..10).map(|v| game.visit_at(v).unwrap()).collect();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let after: Vec<CaravanVisit> = (0..10).map(|v| loaded.visit_at(v).unwrap()).collect();

    assert_eq!(before, after, "the rhythm is a property of the base's seed");
}

/// The property that keeps the feature from shifting every other roll in the
/// run: reading the schedule never touches `GameRng`.
#[test]
fn reading_the_schedule_draws_no_game_rng() {
    let mut game = fresh();
    based(&mut game);
    set_tick(&mut game, 3_210);

    reseed_rng(&mut game, 99);
    let control = draws(&mut game, 4);

    reseed_rng(&mut game, 99);
    for visit in 0..25 {
        let _ = game.visit_at(visit);
    }
    let _ = game.scheduled_visit();
    let after = draws(&mut game, 4);

    assert_eq!(
        control, after,
        "the schedule moved the shared RNG stream, so it shifts every later roll"
    );
}

/// The next `n` values off the shared stream. Compared against a control run
/// from the same reseed: an equal sequence means nothing in between drew.
fn draws(game: &mut Game, n: usize) -> Vec<u64> {
    use rand::RngExt;
    (0..n)
        .map(|_| game.world.resource_mut::<GameRng>().0.random())
        .collect()
}
