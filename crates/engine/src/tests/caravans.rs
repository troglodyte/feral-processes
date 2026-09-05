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
        assert!(
            def.bonus_share <= 100,
            "{id}'s bonus_share {} is a percentage",
            def.bonus_share
        );
    }
}

/// The fourth thing `complaint` refuses, asserted the way the other three
/// are: a percentage above 100 is a content mistake, and left in it would
/// simply saturate at "every gear row" with nothing said.
#[test]
fn a_def_with_a_bonus_share_over_a_hundred_is_refused() {
    let dir = scratch_assets_dir("caravans_bonus_share");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(
        &*dir.join("greedy.ron"),
        "(id: \"greedy\", name: \"G\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 3, weights: (gear: 1), bonus_share: 101, min_zone: 1, max_zone: 9)",
    )
    .unwrap();
    std::fs::write(
        &*dir.join("fine.ron"),
        "(id: \"fine\", name: \"F\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 3, weights: (gear: 1), bonus_share: 100, min_zone: 1, max_zone: 9)",
    )
    .unwrap();

    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();

    assert_eq!(warnings.len(), 1, "one bad file, one warning: {warnings:?}");
    assert!(db.get("greedy").is_none(), "101% was stocked anyway");
    assert!(db.get("fine").is_some(), "100% is a legal shelf");
}

/// A shelf with no `bonus_share` authored at all is exactly the shelf the
/// feature replaced — the `#[serde(default)]` half of the additive-change
/// rule, and what a modder's existing file gets.
#[test]
fn a_def_with_no_bonus_share_parses_at_zero() {
    let def: CaravanDef = ron::from_str(
        "(id: \"plain\", name: \"P\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 3, weights: (gear: 1), min_zone: 1, max_zone: 9)",
    )
    .expect("a file written before bonus_share existed still parses");
    assert_eq!(def.bonus_share, 0);
}

// ---------------------------------------------------------------------------
// The derived schedule
// ---------------------------------------------------------------------------

use crate::game::caravan::CaravanVisit;
use crate::resources::{GameClock, GameRng, Locale};
use crate::tuning::{
    CARAVAN_ARRIVAL_JITTER_TICKS, CARAVAN_STAY_TICKS, CARAVAN_VISIT_INTERVAL_TICKS,
};
use crate::{DifficultyMode, Game, Glyph, GlyphColor, Position, Structure};

/// Onboarding skipped: `based` raises a Home, which finishes the chain's
/// first mission and pays Credits into the middle of a ledger assertion.
fn fresh() -> Game {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    skip_tutorial(&mut game);
    game
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

// ---------------------------------------------------------------------------
// The derived shelf
// ---------------------------------------------------------------------------

use crate::items::ids;
use crate::views::{CaravanOffer, CaravanOfferKind};

fn shelf(game: &mut Game, visit: u64) -> Vec<CaravanOffer> {
    let v = game.visit_at(visit).unwrap();
    game.caravan_shelf(&v)
}

/// Every visit index the schedule can reach in a long run, at one sector.
/// Used by the censuses, which have to sweep rather than sample: a shelf is
/// a fold, so a rule that holds for the first ten rows says nothing.
fn every_shelf(game: &mut Game, visits: u64) -> Vec<CaravanOffer> {
    (0..visits).flat_map(|v| shelf(game, v)).collect()
}

#[test]
fn a_shelf_holds_the_visiting_defs_row_count() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);

    for visit in 0..20 {
        let v = game.visit_at(visit).unwrap();
        let rows = game
            .world
            .resource::<CaravanDb>()
            .get(&v.def_id)
            .unwrap()
            .rows;
        assert_eq!(
            game.caravan_shelf(&v).len() as u32,
            rows,
            "visit {visit} ({}) stocked the wrong number of rows",
            v.def_id
        );
    }
}

/// Dedupe is per kind, and each key is what the player can tell apart: a
/// routine is its ability, a program its species, a stack of cargo its item.
/// Two material rows differing only in stack size are the same row twice.
/// Gear alone keys on the whole `GearCopy`, because rarity, affix and
/// quality are what make two copies of one item different things to buy.
///
/// Swept **within** each shelf and never across the run: a wagon may stock
/// what the last one did, and should — the pools are redrawn per visit.
#[test]
fn a_shelf_never_lists_the_same_thing_twice() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);

    for visit in 0..30 {
        let mut seen: Vec<String> = Vec::new();
        for offer in shelf(&mut game, visit) {
            let key = match &offer.kind {
                CaravanOfferKind::Gear(copy) => format!("gear {copy:?}"),
                CaravanOfferKind::Routine(ability) => format!("routine {ability}"),
                CaravanOfferKind::Program(species) => format!("program {species}"),
                CaravanOfferKind::Material(item) => format!("material {item:?}"),
            };
            assert!(
                !seen.contains(&key),
                "visit {visit} stocked {key} twice on one shelf"
            );
            seen.push(key);
        }
    }
}

/// A def whose best-weighted category runs dry fills the rest of its shelf
/// out of the others rather than coming up short. This is the shipped Kennel
/// Run's own case — there are fewer non-boss species than its shelf has rows
/// — but it is pinned against a def that reaches it on purpose, because what
/// holds it is the draw and not the content.
#[test]
fn a_drained_pool_hands_the_rest_of_the_shelf_to_the_others() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    game.world.insert_resource(one_def(
        "drains",
        50,
        "(gear: 1, routines: 0, programs: 100, materials: 0)",
    ));

    let rows = shelf(&mut game, 0);
    assert_eq!(rows.len(), 50, "the shelf came up short of its row count");
    let programs = rows
        .iter()
        .filter(|o| matches!(o.kind, CaravanOfferKind::Program(_)))
        .count();
    let gear = rows
        .iter()
        .filter(|o| matches!(o.kind, CaravanOfferKind::Gear(_)))
        .count();
    assert!(
        programs < rows.len(),
        "the program pool never drained, so this proves nothing"
    );
    assert_eq!(
        programs + gear,
        rows.len(),
        "the shelf dealt a kind its weights gave no share of"
    );
    assert!(gear > 0, "the drained pool took the rest of the shelf down");
}

/// `rows` is a ceiling, not a count: with nothing else weighted there is
/// nothing to fall back to, so the shelf stops when its one pool empties.
/// A def that asks for more than is installed gets what is installed.
#[test]
fn a_shelf_deeper_than_its_pools_stops_when_they_empty() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    game.world.insert_resource(one_def(
        "only_programs",
        500,
        "(gear: 0, routines: 0, programs: 1, materials: 0)",
    ));

    let rows = shelf(&mut game, 0);
    let species = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .all()
        .filter(|d| !d.is_boss)
        .count();
    assert_eq!(
        rows.len(),
        species,
        "a shelf of nothing but programs is exactly the roster, once each"
    );
}

/// A `CaravanDb` holding one made-up trader, so a test can pin the draw
/// against weights the shipped content has no reason to carry.
fn one_def(id: &str, rows: u32, weights: &str) -> CaravanDb {
    let dir = scratch_assets_dir(&format!("caravans_{id}"));
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(
        &*dir.join("only.ron"),
        format!(
            "(
    id: \"{id}\",
    name: \"Test Wagon\",
    description: \"A test wagon.\",
    glyph: 'W',
    color: DarkGreen,
    rows: {rows},
    weights: {weights},
    bonus_share: 0,
    min_zone: 0,
    max_zone: 99,
)"
        ),
    )
    .unwrap();
    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();
    assert!(
        warnings.is_empty(),
        "the test def did not load: {warnings:?}"
    );
    db
}

/// The bias, not an exact composition — pinning the latter would pin the RNG
/// stream rather than the feature.
///
/// **Shares rather than a majority**, because a shelf is now deeper than
/// some of the pools it draws from and the outcome bias is bounded by pool
/// depth rather than by the weights alone: there are sixteen non-boss
/// species against fifty rows, so no def can make programs the largest part
/// of a wagon however it is weighted. What the weights still decide — and
/// all they can decide — is which trader deals in a category at all and
/// which one deals in more of it.
#[test]
fn a_defs_weights_decide_what_it_deals_in() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);

    let mut gear_trader = (0u32, 0u32);
    let mut program_trader = (0u32, 0u32);
    for visit in 0..120u64 {
        let v = game.visit_at(visit).unwrap();
        let is_program_weighted = game
            .world
            .resource::<CaravanDb>()
            .get(&v.def_id)
            .map(|d| d.weights.programs > d.weights.gear)
            .unwrap();
        for offer in game.caravan_shelf(&v) {
            let bucket = if is_program_weighted {
                &mut program_trader
            } else {
                &mut gear_trader
            };
            match offer.kind {
                CaravanOfferKind::Gear(_) => bucket.0 += 1,
                CaravanOfferKind::Program(_) => bucket.1 += 1,
                _ => {}
            }
        }
    }

    assert!(
        gear_trader.0 > gear_trader.1,
        "the gear-weighted trader sold more programs than gear: {gear_trader:?}"
    );
    assert_eq!(
        gear_trader.1, 0,
        "the gear-weighted trader is weighted out of programs entirely"
    );
    assert!(
        program_trader.1 > gear_trader.1,
        "the program-weighted trader sold no more programs than the one \
         weighted out of them: {program_trader:?} against {gear_trader:?}"
    );
    assert!(
        gear_trader.0 > program_trader.0,
        "the gear-weighted trader sold no more gear than the program-weighted \
         one: {gear_trader:?} against {program_trader:?}"
    );
}

#[test]
fn the_same_visit_stocks_the_same_shelf_twice_and_across_a_reload() {
    let dir = scratch_assets_dir("caravan_shelf_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    let once = shelf(&mut game, 6);
    let twice = shelf(&mut game, 6);
    assert_eq!(once, twice, "reading a shelf changed it");
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(
        once,
        shelf(&mut loaded, 6),
        "the shelf is a property of the base's seed, not of the session"
    );
}

#[test]
fn a_deeper_sector_charges_more_for_the_same_row() {
    let mut game = fresh();
    based(&mut game);

    set_zone(&mut game, 1);
    let near: Vec<u32> = shelf(&mut game, 3).iter().map(|o| o.unit_cost).collect();
    set_zone(&mut game, 5);
    let far: Vec<u32> = shelf(&mut game, 3).iter().map(|o| o.unit_cost).collect();

    assert_eq!(near.len(), far.len(), "the fixture changed the shelf shape");
    assert!(
        near.iter().zip(&far).all(|(a, b)| b > a),
        "a sector-5 shelf is not dearer than a sector-1 one: {near:?} vs {far:?}"
    );
}

/// A craftable sold under what its ingredients are worth is an infinite
/// Credit loop through the nearest counter.
#[test]
fn no_craftable_is_sold_under_its_parts() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 4);

    for offer in every_shelf(&mut game, 60) {
        let item = match &offer.kind {
            CaravanOfferKind::Gear(copy) => copy.item.clone(),
            CaravanOfferKind::Material(item) => item.clone(),
            CaravanOfferKind::Routine(ability) => crate::ItemId::etched(ability),
            CaravanOfferKind::Program(_) => continue,
        };
        let Some(parts) = game
            .world
            .resource::<crate::items_db::ItemDb>()
            .get(item.as_str())
            .and_then(|d| d.craftable.clone())
        else {
            continue;
        };
        let worth: u32 = parts.cost.iter().map(|(i, q)| game.item_value(i) * q).sum();
        assert!(
            offer.unit_cost > worth,
            "{} sells at {} but its parts are worth {worth}",
            offer.name,
            offer.unit_cost
        );
    }
}

/// A tool carrier must never be a caravan's own offer — buying one would let
/// Credits skip the research→forge chain the feature exists to make the
/// player earn. `stock_pool`'s exclusion (`game::caravan`) is what this
/// walks: an etched ability already gets the same treatment for the same
/// reason.
#[test]
fn no_caravan_shelf_ever_stocks_a_tool_carrier() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 4);

    for offer in every_shelf(&mut game, 60) {
        if let CaravanOfferKind::Material(item) = &offer.kind {
            assert!(
                item.tool_id().is_none(),
                "{} is a tool carrier and must never be stocked for sale",
                offer.name
            );
        }
    }
}

/// A carrier already forged is still ordinary cargo on the *sell* side — the
/// same asymmetry an etched disk already has (buyable nowhere, sellable
/// anywhere held cargo is).
#[test]
fn a_held_tool_carrier_is_still_sellable_to_a_caravan() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    let starter = crate::tools::ToolId(crate::tuning::STARTER_TOOL_ID.to_string());
    let carrier = crate::ItemId::tool(&starter);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(carrier.clone(), 1);

    let sells = game.caravan_view().unwrap().sells;
    assert!(
        sells.iter().any(|row| row.copy.item == carrier),
        "a held carrier must be sellable back to the caravan"
    );
}

/// The craft floor is **slack on the shipped item set** — every shipped
/// recipe already costs less than its result is worth
/// (`every_craftable_is_worth_more_than_its_parts`), so the census above
/// passes with the floor deleted. A mod's item is what makes it bite, and
/// what it exists for: a craftable a caravan sells under its parts is an
/// infinite Credit loop through the nearest counter.
#[test]
fn an_underpriced_craftable_is_still_sold_above_its_parts() {
    const CHEAP: &str = r#"(
        id: "cheap_plate",
        name: "Cheap Plate",
        description: "Worth less than what goes into it.",
        value: Some(1),
        craftable: Some((cost: [("core_fragment", 40)])),
    )"#;
    let dir = modded_assets_dir(
        "caravan_underpriced",
        &[],
        &[("cheap_plate.ron", CHEAP)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(19, DifficultyMode::Forgiving, &dir).unwrap();
    based(&mut game);
    let item = crate::ItemId::from("cheap_plate");

    let parts = game.item_value(&crate::ItemId::from(ids::CORE_FRAGMENT)) * 40;
    let asked = game.caravan_unit_cost(&item);

    assert!(
        asked > parts,
        "the caravan asks {asked} for something whose parts are worth {parts},          so a player could buy it and sell the parts forever"
    );
}

/// The one hard floor under the whole feature. Breaching is earned by
/// fighting and descending; a caravan is convenience, and convenience must
/// never be the way past that. Held by `stock_pool`'s `EconomyRole`
/// exclusion, so no weighting of any def at any sector can reach one.
#[test]
fn no_shelf_anywhere_ever_stocks_a_currency() {
    let mut game = fresh();
    based(&mut game);
    let fragment = crate::ItemId::from(ids::PORTAL_FRAGMENT);

    for zone in 1..=8u32 {
        set_zone(&mut game, zone);
        for offer in every_shelf(&mut game, 40) {
            let item = match &offer.kind {
                CaravanOfferKind::Gear(copy) => copy.item.clone(),
                CaravanOfferKind::Material(item) => item.clone(),
                CaravanOfferKind::Routine(ability) => crate::ItemId::etched(ability),
                CaravanOfferKind::Program(_) => continue,
            };
            assert_ne!(item, fragment, "a Portal Fragment reached a shelf");
            assert!(
                game.world
                    .resource::<crate::items_db::ItemDb>()
                    .get(item.as_str())
                    .is_none_or(|d| d.role.is_none() && !d.banked),
                "{} is currency or banked and reached a shelf",
                offer.name
            );
        }
    }
}

#[test]
fn reading_a_shelf_draws_no_game_rng() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);

    reseed_rng(&mut game, 41);
    let control = draws(&mut game, 4);

    reseed_rng(&mut game, 41);
    let _ = every_shelf(&mut game, 12);
    let after = draws(&mut game, 4);

    assert_eq!(
        control, after,
        "stocking a shelf moved the shared RNG stream"
    );
}

// ---------------------------------------------------------------------------
// The component, the space predicate and the save
// ---------------------------------------------------------------------------

use crate::components::{Caravan, CaravanStage};

/// Stands a caravan on `tile` at `stage`, without walking it there.
fn stand_caravan(game: &mut Game, stage: CaravanStage, tile: (i32, i32)) -> crate::Entity {
    let visit = game.visit_index();
    game.world
        .spawn((
            Caravan {
                stage,
                visit,
                arrival_tile: (9, 9),
                stage_ticks: 0,
                announced_stuck: false,
            },
            Position {
                x: tile.0,
                y: tile.1,
            },
            Glyph {
                ch: 'Ω',
                color: GlyphColor::DarkGreen,
            },
        ))
        .id()
}

fn drawn(game: &mut Game, entity: crate::Entity) -> bool {
    game.view_entities(40, 40)
        .iter()
        .any(|v| v.entity == entity)
}

/// Asserted **through `view_entities` from both locales**, never by calling
/// the predicate directly: the map and the space rule have to be tested
/// against each other or the two can agree on a wrong answer.
#[test]
fn a_caravan_is_drawn_in_exactly_one_space_per_stage() {
    let mut game = fresh();
    based(&mut game);
    let caravan = stand_caravan(&mut game, CaravanStage::Approaching, (3, 0));

    game.world.insert_resource(Locale::Surface);
    assert!(
        drawn(&mut game, caravan),
        "an approaching caravan is out in the sector"
    );
    stand_in_base_at(&mut game, 1, 1);
    assert!(
        !drawn(&mut game, caravan),
        "and must not also be inside the base — base space's origin and the \
         zone spawn point commonly alias, so a wrong-space draw looks right"
    );

    game.world.get_mut::<Caravan>(caravan).unwrap().stage = CaravanStage::Docked;
    assert!(
        drawn(&mut game, caravan),
        "a docked caravan is standing in the base"
    );
    game.world.insert_resource(Locale::Surface);
    assert!(
        !drawn(&mut game, caravan),
        "and is no longer out in the sector"
    );
}

/// The two transition ticks are decided, not defaulted: both are spent on
/// the anchor tile, which is a tile of the zone surface.
#[test]
fn the_transition_stages_stand_on_the_surface() {
    for stage in [CaravanStage::Docking, CaravanStage::Leaving] {
        assert!(
            !stage.in_base_space(),
            "{stage:?} is spent standing on the anchor, out in the sector"
        );
    }
    for stage in [CaravanStage::Crossing, CaravanStage::Docked] {
        assert!(stage.in_base_space(), "{stage:?} is inside");
    }
}

/// A real save→load, not only the RON round trip: a `#[serde(skip)]` would
/// leave a round-trip test green while losing the field on disk.
#[test]
fn a_caravan_mid_journey_survives_a_save_and_load() {
    let dir = scratch_assets_dir("caravan_component_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = fresh();
    based(&mut game);
    let before = Caravan {
        stage: CaravanStage::Crossing,
        visit: game.visit_index(),
        arrival_tile: (-7, 4),
        stage_ticks: 13,
        announced_stuck: false,
    };
    game.world.spawn((
        before.clone(),
        Position { x: 2, y: -1 },
        Glyph {
            ch: 'Ω',
            color: GlyphColor::DarkGreen,
        },
    ));
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&Caravan, &Position)>();
    let found: Vec<(Caravan, Position)> = query
        .iter(&loaded.world)
        .map(|(c, p)| (c.clone(), *p))
        .collect();

    assert_eq!(found.len(), 1, "one caravan in, one caravan out");
    assert_eq!(found[0].0, before, "the whole journey came back");
    assert_eq!((found[0].1.x, found[0].1.y), (2, -1), "and where it stood");
}

/// The field is additive behind `#[serde(default)]`, so it costs nothing.
#[test]
fn caravans_cost_no_save_format_bump() {
    assert_eq!(
        crate::save::SAVE_FORMAT_VERSION,
        32,
        "a caravan is an additive named-struct field and must not bump this"
    );
}

// ---------------------------------------------------------------------------
// The journey
// ---------------------------------------------------------------------------

use crate::MessageKind;
use crate::resources::MessageLog;

/// A base with an anchor standing, ticked forward to the moment the next
/// visit opens — the state every journey test starts from.
///
/// The clock is wound rather than the caravan hand-placed, so what is under
/// test is the transition and not a fixture's idea of one.
fn at_the_arrival(game: &mut Game) -> CaravanVisit {
    let visit = game.visit_at(game.visit_index() + 1).unwrap();
    set_tick(game, visit.arrival_tick);
    visit
}

fn caravan_of(game: &mut Game) -> Option<Caravan> {
    let mut query = game.world.query::<&Caravan>();
    query.iter(&game.world).next().cloned()
}

fn stage_of(game: &mut Game) -> Option<CaravanStage> {
    caravan_of(game).map(|c| c.stage)
}

/// Ticks until `want` is reached or `budget` runs out, and says which.
fn tick_until(game: &mut Game, budget: u32, want: impl Fn(&mut Game) -> bool) -> bool {
    for _ in 0..budget {
        if want(game) {
            return true;
        }
        game.caravan_tick();
        game.world.resource_mut::<GameClock>().tick += 1;
    }
    want(game)
}

/// All four transitions, driven by ticking rather than by hand-writing a
/// stage — a fixture that writes the stage tests nothing but the enum.
#[test]
fn a_caravan_walks_in_docks_and_walks_back_out() {
    let mut game = fresh();
    based(&mut game);
    at_the_arrival(&mut game);

    game.caravan_tick();
    assert_eq!(
        stage_of(&mut game),
        Some(CaravanStage::Approaching),
        "the visit opened and nobody walked in"
    );

    assert!(
        tick_until(&mut game, 200, |g| stage_of(g)
            == Some(CaravanStage::Docking)),
        "it never reached the anchor"
    );
    assert!(
        tick_until(&mut game, 20, |g| stage_of(g)
            == Some(CaravanStage::Crossing)),
        "it stood on the anchor and never phased out"
    );
    assert!(
        tick_until(&mut game, 200, |g| stage_of(g)
            == Some(CaravanStage::Docked)),
        "it never reached the counter"
    );

    // The counter it docked at, checked here rather than in its own test:
    // "beside the Market" is the whole of what `Docked` claims.
    let counter = game.trading_structures().next().unwrap().1;
    let standing = {
        let mut query = game.world.query::<(&Caravan, &Position)>();
        *query.iter(&game.world).next().unwrap().1
    };
    assert!(
        crate::game::base::hauling::at_station(standing, counter),
        "docked at {standing:?}, which is not beside the counter at {counter:?}"
    );

    assert!(
        tick_until(&mut game, CARAVAN_STAY_TICKS as u32 + 200, |g| {
            caravan_of(g).is_none()
        }),
        "it never left"
    );
}

/// Arrival and departure are each one line, and neither is `Raid` — a trader
/// is not a sweep and must not read as one.
#[test]
fn arrival_and_departure_each_log_one_ordinary_line() {
    let mut game = fresh();
    based(&mut game);
    at_the_arrival(&mut game);

    game.caravan_tick();
    let arrival = caravan_lines(&mut game);
    assert_eq!(arrival.len(), 1, "arrival said: {arrival:?}");

    tick_until(&mut game, CARAVAN_STAY_TICKS as u32 + 600, |g| {
        caravan_of(g).is_none()
    });
    let all = caravan_lines(&mut game);
    assert!(
        all.len() >= 2,
        "a whole visit should have said arrival and departure: {all:?}"
    );
    assert!(
        all.iter().all(|(_, kind)| *kind != MessageKind::Raid),
        "a caravan logged as a raid: {all:?}"
    );
}

/// Every line the caravan wrote, with its kind — matched on the shipped
/// traders' own names rather than on a phrase, so rewording a log line does
/// not silently empty this.
fn caravan_lines(game: &mut Game) -> Vec<(String, MessageKind)> {
    let names: Vec<String> = game
        .world
        .resource::<CaravanDb>()
        .all()
        .map(|d| d.name.clone())
        .collect();
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|e| names.iter().any(|n| e.text.contains(n.as_str())))
        .map(|e| (e.text.clone(), e.kind))
        .collect()
}

/// Ticks run regardless of where the party is: the base keeps working while
/// they are underground, and a caravan is a property of the base.
#[test]
fn a_caravan_advances_while_the_party_is_underground() {
    let mut game = fresh();
    based(&mut game);
    at_the_arrival(&mut game);
    game.caravan_tick();
    let started = {
        let mut query = game.world.query::<(&Caravan, &Position)>();
        *query.iter(&game.world).next().unwrap().1
    };

    game.world.insert_resource(Locale::Surface);
    descend(&mut game);
    assert!(game.is_underground(), "the fixture never got underground");

    for _ in 0..20 {
        game.caravan_tick();
    }
    let now = {
        let mut query = game.world.query::<(&Caravan, &Position)>();
        *query.iter(&game.world).next().unwrap().1
    };
    assert_ne!(
        (started.x, started.y),
        (now.x, now.y),
        "the caravan stood still because the party was elsewhere"
    );
}

/// Two Markets and one answer. Bevy's query iteration order is not stable,
/// so a caravan that docked at a different counter between two loads of one
/// save would be reporting iteration order rather than the base.
///
/// **The two are stood up in the opposite order to their positions**, which
/// is what makes this bite — `assembler_system`'s own test does the same,
/// for the same reason: with spawn order and `(x, y)` order agreeing, an
/// unsorted walk gives the right answer by luck.
#[test]
fn a_base_with_two_markets_always_docks_at_the_same_one() {
    let dir = scratch_assets_dir("caravan_two_markets");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = fresh();
    game.lay_starting_pocket();
    deploy(&mut game, "home", 0, 0);
    deploy(&mut game, "market", 2, 0);
    deploy(&mut game, "market", -2, 1);
    stand_in_base_at(&mut game, 1, 1);
    game.save(&path).unwrap();

    let chosen = |_run: u32| -> (i32, i32) {
        let game = Game::load(&path, &test_assets_dir()).unwrap();
        let counter = game.trading_structures().next().unwrap().1;
        (counter.x, counter.y)
    };

    assert_eq!(
        chosen(0),
        (-2, 1),
        "the counter picked is not the (x, y)-first one, so the walk is \
         reporting whatever order the query happened to hand back"
    );
    for run in 1..6 {
        assert_eq!(
            chosen(run),
            (-2, 1),
            "the counter moved between two loads of one save"
        );
    }
}

/// The Market comes down mid-visit and the trader has no reason to stay.
#[test]
fn a_caravan_leaves_early_when_the_counter_comes_down() {
    let mut game = fresh();
    based(&mut game);
    at_the_arrival(&mut game);
    game.caravan_tick();
    assert!(
        tick_until(&mut game, 400, |g| stage_of(g)
            == Some(CaravanStage::Docked)),
        "it never docked"
    );

    let counter = game.trading_structures().next().unwrap().0;
    game.world.despawn(counter);

    game.caravan_tick();
    assert_eq!(
        stage_of(&mut game),
        Some(CaravanStage::Leaving),
        "the counter is gone and the trader is still standing there"
    );
}

/// The base-space stall says so **once**, not once a tick, per
/// `set_machine_status`' rule — a per-tick line is what makes a latch
/// necessary in the first place.
#[test]
fn a_caravan_with_no_way_to_the_counter_complains_once() {
    let mut game = fresh();
    based(&mut game);
    // A counter walled off from the door: base space is solid everywhere the
    // pocket was not laid, so a Market outside it has no route in.
    let counter = game.trading_structures().next().unwrap().0;
    game.world.despawn(counter);
    deploy(&mut game, "market", 60, 60);
    at_the_arrival(&mut game);

    game.caravan_tick();
    assert!(
        tick_until(&mut game, 400, |g| stage_of(g)
            == Some(CaravanStage::Crossing)),
        "it never got inside"
    );
    for _ in 0..60 {
        game.caravan_tick();
    }

    let complaints = caravan_lines(&mut game)
        .into_iter()
        .filter(|(text, _)| text.contains("way through"))
        .count();
    assert_eq!(complaints, 1, "said it {complaints} times");
    assert!(
        caravan_of(&mut game).is_some(),
        "it waits out the visit rather than giving up — the player can clear \
         a way through, which is why it is worth saying at all"
    );
}

// ---------------------------------------------------------------------------
// Examine, and the glyph on the map
// ---------------------------------------------------------------------------

use crate::views::InspectTarget;

/// The map and the ray asserted **against each other**, never against a
/// string: they are one rule read from two places, and a test that pinned a
/// name would pass with either half broken.
///
/// The ray is aimed **west**, away from the counter. Aimed east it finds the
/// Market first and the "not from inside the base" half passes without the
/// space gate ever being consulted — which is what the first draft of this
/// test did.
#[test]
fn the_examine_ray_names_what_the_map_draws_of_a_caravan() {
    let mut game = fresh();
    based(&mut game);
    game.world.insert_resource(Locale::Surface);
    let anchor = game.anchor_position().unwrap();
    let player = game.player_entity();
    *game.world.get_mut::<Position>(player).unwrap() = Position {
        x: anchor.0,
        y: anchor.1,
    };
    let caravan = stand_caravan(
        &mut game,
        CaravanStage::Approaching,
        (anchor.0 - 3, anchor.1),
    );
    let look = |g: &mut Game| g.find_target_in_direction(-1, 0, crate::tuning::EXAMINE_RANGE_TILES);

    assert!(
        drawn(&mut game, caravan),
        "the map has to draw it for the ray to have a subject"
    );
    assert_eq!(
        look(&mut game),
        Some(InspectTarget::Caravan(caravan)),
        "the map drew it and the ray looked straight through"
    );

    // And the other way: out of phase it is on neither.
    stand_in_base_at(&mut game, anchor.0, anchor.1);
    assert!(!drawn(&mut game, caravan), "not drawn from inside the base");
    assert_eq!(
        look(&mut game),
        None,
        "and must not be nameable from inside the base either — base space's \
         origin and the zone spawn point commonly alias, so an ungated ray \
         names a trader out in the sector as though it were standing here"
    );
}

/// It is a subject to look at, never a subject to fight: no `Creature`, no
/// `Stats`, and so no manifest.
#[test]
fn a_caravan_is_not_a_combat_participant() {
    let mut game = fresh();
    based(&mut game);
    let caravan = stand_caravan(&mut game, CaravanStage::Approaching, (3, 0));

    assert!(game.world.get::<crate::Stats>(caravan).is_none());
    assert!(game.world.get::<crate::Creature>(caravan).is_none());
    assert!(game.world.get::<crate::Hostile>(caravan).is_none());
    assert!(
        game.manifest(caravan).is_none(),
        "a trader has no sheet to open"
    );
    assert!(
        game.caravan_blurb(caravan).is_some(),
        "what it has instead is its own line"
    );
}

// ---------------------------------------------------------------------------
// Reach, transactions and within-visit depletion
// ---------------------------------------------------------------------------

use crate::game::caravan::CaravanReach;
use crate::resources::{BuybackLedger, CaravanMemory};
use crate::{GearCopy, Inventory};

/// A base with a docked trader and the party standing beside it — the state
/// every transaction question is asked from. Returns the open visit.
fn docked(game: &mut Game) -> CaravanVisit {
    let visit = at_the_arrival(game);
    game.caravan_tick();
    assert!(
        tick_until(game, 600, |g| stage_of(g) == Some(CaravanStage::Docked)),
        "the fixture never got a trader to the counter"
    );
    visit
}

fn credits(game: &mut Game) -> u32 {
    let currency = game.trade_currency();
    game.world
        .get::<Inventory>(game.player_entity())
        .map(|inv| inv.count(&currency))
        .unwrap_or(0)
}

fn give_credits(game: &mut Game, n: u32) {
    let currency = game.trade_currency();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(currency, n);
}

#[test]
fn caravan_reach_reports_the_three_states() {
    let mut game = fresh();
    based(&mut game);
    assert_eq!(
        game.caravan_reach(),
        CaravanReach::NoCaravan,
        "nothing is visiting"
    );

    at_the_arrival(&mut game);
    game.caravan_tick();
    assert_eq!(
        game.caravan_reach(),
        CaravanReach::NotDocked,
        "it is still walking in"
    );

    assert!(tick_until(&mut game, 600, |g| stage_of(g)
        == Some(CaravanStage::Docked)));
    assert_eq!(game.caravan_reach(), CaravanReach::AtCaravan);

    // The far edge of the pocket is still the base — the walk to the counter
    // is visibility, not a gate.
    stand_in_base_at(&mut game, crate::tuning::STARTING_POCKET_RADIUS, 0);
    assert_eq!(game.caravan_reach(), CaravanReach::AtCaravan);

    game.world.insert_resource(Locale::Surface);
    assert_eq!(
        game.caravan_reach(),
        CaravanReach::NotDocked,
        "out on the grid there is nothing to take"
    );
}

#[test]
fn a_bought_row_is_gone_for_the_visit_and_back_the_next() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    give_credits(&mut game, 1_000_000);

    let before = game.caravan_view().unwrap().offers;
    let row = before[0].index;
    game.buy_caravan_offer(row).unwrap();

    let after = game.caravan_view().unwrap().offers;
    assert_eq!(
        after.len(),
        before.len() - 1,
        "the row is still on the shelf"
    );
    assert!(
        !after.iter().any(|o| o.index == row),
        "and it is the one that was bought that went"
    );
    assert_eq!(
        game.buy_caravan_offer(row),
        Err("That's not on the wagon.".into()),
        "buying it twice"
    );

    // The memory is keyed by visit index, so the next visit is untouched
    // without anything having to reset it.
    let next = game.visit_index() + 1;
    assert!(
        game.caravan_spent(next).is_empty(),
        "next month's trader arrived already sold out"
    );
}

/// Asserted on the ledger, not on a screen: a caravan keeps no shelf, and a
/// screen that merely fails to draw one would pass either way.
#[test]
fn selling_to_a_caravan_stocks_no_buyback_shelf() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    let item = crate::ItemId::from(ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 10);

    let before = credits(&mut game);
    game.sell_to_caravan(GearCopy::plain(item.clone()), 4)
        .unwrap();

    assert!(credits(&mut game) > before, "it paid nothing");
    assert!(
        game.world.resource::<BuybackLedger>().0.is_empty(),
        "a caravan rolls away — there is nothing to buy it back from"
    );
}

/// The counter's own rate, read off its `TradeDef` rather than restated, so
/// retuning the Market moves this with it.
#[test]
fn a_caravan_pays_the_counters_own_rate() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    let item = crate::ItemId::from(ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 10);

    let counter = game.trading_structures().next().unwrap().0;
    let rate = game.trade_options(counter).unwrap().sell_rate;
    let expected = game.item_value(&item) * rate;

    let row = game
        .caravan_view()
        .unwrap()
        .sells
        .into_iter()
        .find(|r| r.copy.item == item)
        .expect("what the player is holding has to be sellable");
    assert_eq!(row.unit_price, expected);
}

/// Every refusal lands before anything is spent — a purchase that took the
/// Credits and then failed is the one bug the player cannot undo, and a
/// caravan has no buyback to put it right with.
#[test]
fn every_refusal_leaves_credits_and_cargo_exactly_as_they_were() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    give_credits(&mut game, 50);
    let item = crate::ItemId::from(ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 3);

    let dearest = game
        .caravan_view()
        .unwrap()
        .offers
        .into_iter()
        .max_by_key(|o| o.unit_cost * o.qty)
        .unwrap();
    let holdings = |g: &mut Game| -> (u32, u32) {
        let held = g
            .world
            .get::<Inventory>(g.player_entity())
            .map(|inv| inv.count(&crate::ItemId::from(ids::CORE_FRAGMENT)))
            .unwrap_or(0);
        (credits(g), held)
    };

    let before = holdings(&mut game);
    assert!(
        game.buy_caravan_offer(dearest.index).is_err(),
        "50 Credits should not cover the dearest row on the wagon"
    );
    assert!(game.buy_caravan_offer(9_999).is_err(), "no such row");
    assert!(
        game.sell_to_caravan(GearCopy::plain(item), 0).is_err(),
        "selling nothing"
    );
    assert!(
        game.sell_to_caravan(GearCopy::plain(crate::ItemId::from("nothing_at_all")), 1)
            .is_err(),
        "selling what you do not hold"
    );
    assert_eq!(holdings(&mut game), before, "a refusal spent something");

    // And off the base, where there is no counter to refuse at.
    game.world.insert_resource(Locale::Surface);
    assert!(game.buy_caravan_offer(dearest.index).is_err());
    assert_eq!(holdings(&mut game), before);
}

/// The wagon used to be despawned at a breach because its journey was
/// defined against the anchor tile of the sector it walked into, and that
/// sector was about to stop existing. It does not stop existing now, so the
/// trader keeps walking and remembers what it sold you.
#[test]
fn a_breach_leaves_the_caravan_and_its_memory_alone() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    give_credits(&mut game, 1_000_000);
    let row = game.caravan_view().unwrap().offers[0].index;
    game.buy_caravan_offer(row).unwrap();
    let bought = game.world.resource::<CaravanMemory>().clone();
    assert!(
        !bought.bought.is_empty(),
        "test premise: something was bought"
    );

    game.enter_next_zone();

    assert_eq!(
        *game.world.resource::<CaravanMemory>(),
        bought,
        "a breach forgot a sale that really happened"
    );
    assert!(
        caravan_of(&mut game).is_some(),
        "a breach despawned a trader standing on ground that still exists"
    );
}

#[test]
fn caravan_memory_survives_a_save_and_load() {
    let dir = scratch_assets_dir("caravan_memory_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    give_credits(&mut game, 1_000_000);
    let row = game.caravan_view().unwrap().offers[0].index;
    game.buy_caravan_offer(row).unwrap();
    let before = game.world.resource::<CaravanMemory>().clone();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(
        *loaded.world.resource::<CaravanMemory>(),
        before,
        "a reload put a bought row back on the wagon"
    );
    assert_eq!(
        crate::save::SAVE_FORMAT_VERSION,
        32,
        "both caravan fields are additive named-struct ones"
    );
}

// ---------------------------------------------------------------------------
// What grade of gear a shelf carries, and how it spreads across the slots

/// Stands one synthetic trader up in place of the shipped pair, so a shelf's
/// composition can be asserted against an authored `rows`/`weights`/
/// `bonus_share` instead of against whichever wagon the schedule happened to
/// send. Written to a scratch dir and swapped in as a resource — mutating
/// `assets/` is how a timed-out run once left a shipped item edited.
fn only_trader(game: &mut Game, name: &str, body: &str) {
    let dir = scratch_assets_dir(name);
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(&*dir.join("solo.ron"), body).unwrap();
    let (db, warnings) = CaravanDb::load_dir(&*dir).unwrap();
    assert!(
        warnings.is_empty(),
        "the fixture's own def warned: {warnings:?}"
    );
    assert_eq!(
        db.all().count(),
        1,
        "the fixture stands up exactly one trader"
    );
    game.world.insert_resource(db);
}

/// Every gear row's slot, in the order the shelf deals them.
fn gear_slots(game: &mut Game, visit: u64) -> Vec<crate::items::EquipmentSlot> {
    shelf(game, visit)
        .into_iter()
        .filter_map(|offer| match offer.kind {
            CaravanOfferKind::Gear(copy) => game.equipment_of(&copy.item).map(|(slot, _)| slot),
            _ => None,
        })
        .collect()
}

/// Every copy of gear on a shelf, in row order.
fn gear_copies(game: &mut Game, visit: u64) -> Vec<crate::items::GearCopy> {
    shelf(game, visit)
        .into_iter()
        .filter_map(|offer| match offer.kind {
            CaravanOfferKind::Gear(copy) => Some(copy),
            _ => None,
        })
        .collect()
}

/// **The coverage is a guarantee, not an average.** Drawn from one pool the
/// split followed the file count rather than the slot, so a wagon could
/// stand there with six weapons and no armour — a shop that stocks one
/// thing. Swept over every visit rather than sampled: a shelf is a fold, so
/// a rule that holds for the first one says nothing.
#[test]
fn a_shelf_deals_gear_across_every_equipment_slot() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    only_trader(
        &mut game,
        "caravan_slot_spread",
        "(id: \"solo\", name: \"S\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 9, weights: (gear: 1), bonus_share: 0, min_zone: 1, max_zone: 99)",
    );

    for visit in 0..30u64 {
        let slots = gear_slots(&mut game, visit);
        assert_eq!(slots.len(), 9, "visit {visit} did not fill its shelf");
        for slot in crate::items::EquipmentSlot::ALL {
            assert!(
                slots.contains(&slot),
                "visit {visit} stocked no {slot:?}: {slots:?}"
            );
        }
        // Nine rows across three slots is three each, and the round-robin is
        // positional — an equal-chance draw would pass the clause above and
        // still leave a wagon seven-eighths weapons.
        for slot in crate::items::EquipmentSlot::ALL {
            assert_eq!(
                slots.iter().filter(|s| **s == slot).count(),
                3,
                "visit {visit} dealt {slot:?} unevenly: {slots:?}"
            );
        }
    }
}

/// Which slot a wagon leads with rotates, or two traders in one sector both
/// open with a weapon and the round-robin reads as a fixed list.
#[test]
fn which_slot_a_shelf_leads_with_varies_between_visits() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    only_trader(
        &mut game,
        "caravan_slot_lead",
        "(id: \"solo\", name: \"S\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 6, weights: (gear: 1), bonus_share: 0, min_zone: 1, max_zone: 99)",
    );

    let mut leads: Vec<_> = (0..30u64)
        .filter_map(|v| gear_slots(&mut game, v).first().copied())
        .map(|s| s.short_label())
        .collect();
    leads.sort_unstable();
    leads.dedup();
    assert!(
        leads.len() > 1,
        "every shelf led with the same slot: {leads:?}"
    );
}

/// The share is what the shelf *shows*, not what it rolls for. A per-row
/// chance leaves a twelve-row wagon able to come up with none, which is the
/// case the field exists to rule out.
#[test]
fn a_defs_bonus_share_decides_how_many_rows_are_standout() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    only_trader(
        &mut game,
        "caravan_bonus_count",
        "(id: \"solo\", name: \"S\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 8, weights: (gear: 1), bonus_share: 50, min_zone: 1, max_zone: 99)",
    );

    let want = crate::game::caravan::bonus_row_count(8, 50);
    assert_eq!(want, 4, "the fixture is chosen so the count is unambiguous");

    for visit in 0..30u64 {
        let copies = gear_copies(&mut game, visit);
        // A standout row is separable from outside by quality alone: the
        // plain floor is `QUALITY_DROP_BASE` and the whole plain spread sits
        // below `CARAVAN_BONUS_QUALITY_FLOOR`, which is the item's authored
        // figure. That gap is the feature, not an artefact of the test.
        let standout: Vec<_> = copies
            .iter()
            .filter(|c| c.quality >= crate::tuning::CARAVAN_BONUS_QUALITY_FLOOR)
            .collect();
        assert_eq!(
            standout.len(),
            want,
            "visit {visit} showed {} standout rows of 8",
            standout.len()
        );
        for copy in &standout {
            assert!(
                !copy.affixes.is_empty(),
                "visit {visit} sold a standout row with no affix: {copy:?}"
            );
        }
        assert!(
            copies.len() - standout.len() == 8 - want,
            "visit {visit} left no plain rows to compare against"
        );
    }
}

/// The other end of the same axis, and the property that keeps an existing
/// modded file behaving exactly as it did: no share, no standout rows.
#[test]
fn a_trader_with_no_bonus_share_stocks_a_shelf_of_plain_copies() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    only_trader(
        &mut game,
        "caravan_bonus_zero",
        "(id: \"solo\", name: \"S\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 9, weights: (gear: 1), min_zone: 1, max_zone: 99)",
    );

    for visit in 0..30u64 {
        for copy in gear_copies(&mut game, visit) {
            assert!(
                copy.quality < crate::tuning::CARAVAN_BONUS_QUALITY_FLOOR,
                "visit {visit} stocked a standout row on a wagon that authored none: {copy:?}"
            );
        }
    }
}

/// A standout row is a *good* find, not a guaranteed rare one — and the
/// rarity still moves, which is what says the narrowed window is being drawn
/// against rather than the tier being assigned.
#[test]
fn a_standout_row_is_likelier_to_be_rare_without_always_being_rare() {
    let mut game = fresh();
    based(&mut game);
    set_zone(&mut game, 3);
    only_trader(
        &mut game,
        "caravan_bonus_rarity",
        "(id: \"solo\", name: \"S\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 12, weights: (gear: 1), bonus_share: 100, min_zone: 1, max_zone: 99)",
    );
    let bonus_rare = (0..60u64)
        .flat_map(|v| gear_copies(&mut game, v))
        .filter(|c| c.rarity != crate::components::Rarity::Ordinary)
        .count();

    only_trader(
        &mut game,
        "caravan_plain_rarity",
        "(id: \"solo\", name: \"S\", description: \"d\", glyph: 'Ω', color: DarkGreen, \
         rows: 12, weights: (gear: 1), bonus_share: 0, min_zone: 1, max_zone: 99)",
    );
    let plain_rare = (0..60u64)
        .flat_map(|v| gear_copies(&mut game, v))
        .filter(|c| c.rarity != crate::components::Rarity::Ordinary)
        .count();

    assert!(
        bonus_rare > plain_rare,
        "standout rows came up rare no more often than plain ones \
         ({bonus_rare} vs {plain_rare} of 720)"
    );
    assert!(
        bonus_rare < 720,
        "every standout row was rare, so the tier is being assigned rather than rolled"
    );
}

/// The wagon's offers come off the roll shuffled by construction — the draw
/// re-reads its weights per row, so a weapon, a program and a second weapon
/// is the normal shape. Grouped, each kind is one contiguous run.
#[test]
fn the_wagons_offers_come_back_grouped_by_kind() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);

    let offers = game.caravan_view().unwrap().offers;
    assert!(offers.len() > 3, "a wagon with no shelf proves nothing");
    let ranks: Vec<u8> = offers
        .iter()
        .map(|o| game.caravan_group(&o.kind).0)
        .collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(
        ranks, sorted,
        "each kind must be one contiguous run: {ranks:?}"
    );

    // ...and the run the roll actually produced is *not* that order, or the
    // grouping is passing against no fix at all.
    let visit = caravan_of(&mut game).unwrap().visit;
    let dealt: Vec<u8> = shelf(&mut game, visit)
        .iter()
        .map(|o| game.caravan_group(&o.kind).0)
        .collect();
    assert_ne!(
        dealt, sorted,
        "the deal was already sorted, so this fixture cannot see the grouping"
    );
}

/// Sorting moves rows on screen and must move no shelf identity. `index` is
/// handed out before the sort, `CaravanMemory` keys on it and
/// `buy_caravan_offer` resolves by it — so buying the row drawn *last* buys
/// the slot it names, not the slot that happens to sit at that position.
#[test]
fn buying_the_last_drawn_row_buys_the_slot_it_names() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    give_credits(&mut game, 1_000_000);

    let offers = game.caravan_view().unwrap().offers;
    let last = offers.last().unwrap().clone();
    assert_ne!(
        last.index,
        offers.len() - 1,
        "the sorted order must differ from the dealt order here, or the test \
         passes against no fix at all"
    );

    game.buy_caravan_offer(last.index).unwrap();

    assert!(
        game.world
            .resource::<CaravanMemory>()
            .bought
            .contains(&last.index),
        "the shelf slot the row named is what was spent"
    );
    assert!(
        !game
            .caravan_view()
            .unwrap()
            .offers
            .iter()
            .any(|o| o.index == last.index),
        "and it is gone from the wagon"
    );
}

/// Stocks the player with enough Core Fragments that selling them covers
/// `cost`, and returns the sell line that does it.
fn cargo_worth(game: &mut Game, cost: u32) -> (GearCopy, u32) {
    let item = crate::ItemId::from(ids::CORE_FRAGMENT);
    let unit = game
        .caravan_view()
        .unwrap()
        .sells
        .iter()
        .find(|row| row.copy.item == item)
        .map(|row| row.unit_price)
        .unwrap_or_else(|| {
            // Nothing of it in the pack yet, so the wagon lists no sell row —
            // seed one unit and ask again.
            let player = game.player_entity();
            game.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(item.clone(), 1);
            let price = game
                .caravan_view()
                .unwrap()
                .sells
                .iter()
                .find(|row| row.copy.item == item)
                .map(|row| row.unit_price)
                .expect("the wagon prices cargo it will take");
            game.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), 1);
            price
        });
    assert!(unit > 0, "the fixture needs cargo the wagon will pay for");
    let qty = cost.div_ceil(unit);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), qty);
    (GearCopy::plain(item), qty)
}

/// **The reason the function exists.** The player starts unable to afford
/// the row and can afford it only out of what the same basket is selling, so
/// this is red the moment the buys are applied before the sells.
#[test]
fn a_basket_is_funded_by_its_own_sales() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);

    let offer = game.caravan_view().unwrap().offers[0].clone();
    let price = offer.unit_cost * offer.qty;
    let sell = cargo_worth(&mut game, price);
    assert_eq!(
        credits(&mut game),
        0,
        "the player must not be able to pay for this out of pocket"
    );
    let proceeds = game
        .caravan_view()
        .unwrap()
        .sells
        .iter()
        .find(|row| row.copy == sell.0)
        .map(|row| row.unit_price * sell.1)
        .expect("the wagon will take the cargo");

    game.commit_caravan_basket(vec![sell], vec![offer.index])
        .expect("the sale covers the purchase");

    // **The ledger, not just the outcome.** `Inventory::take` clamps, so a
    // basket that bought before it sold still delivers the goods — it simply
    // takes whatever was in the purse (nothing) and the price vanishes. Only
    // the arithmetic catches that.
    assert_eq!(
        credits(&mut game),
        proceeds - price,
        "the purchase must have been paid for out of the sale"
    );

    assert!(
        !game
            .caravan_view()
            .unwrap()
            .offers
            .iter()
            .any(|o| o.index == offer.index),
        "the row was bought"
    );
    assert!(
        game.world
            .resource::<CaravanMemory>()
            .bought
            .contains(&offer.index),
        "and its shelf slot is spent"
    );
}

/// A basket that cannot be paid for refuses whole: no cargo gone, no Credits
/// gone, no shelf slot spent. A caravan has no buyback, so a half-committed
/// basket is the one bug the player cannot undo.
#[test]
fn an_unaffordable_basket_spends_nothing() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);

    let offer = game.caravan_view().unwrap().offers[0].clone();
    let price = offer.unit_cost * offer.qty;
    // Cargo worth a fraction of the row, so the sale is real and still short.
    let (copy, _) = cargo_worth(&mut game, price);
    let held_before = game.count_copies(&copy);
    let credits_before = credits(&mut game);

    assert!(
        game.commit_caravan_basket(vec![(copy.clone(), 1)], vec![offer.index])
            .is_err(),
        "one unit of cargo does not cover the row"
    );

    assert_eq!(game.count_copies(&copy), held_before, "cargo left the pack");
    assert_eq!(credits(&mut game), credits_before, "Credits moved");
    assert!(
        game.world.resource::<CaravanMemory>().bought.is_empty(),
        "a shelf slot was spent by a refused basket"
    );
    assert!(
        game.caravan_view()
            .unwrap()
            .offers
            .iter()
            .any(|o| o.index == offer.index),
        "and the row is still on the wagon"
    );
}

/// **The basket is the visit.** One tick whatever the line count, or a wagon
/// with a long basket charges the player several turns of raid pressure and
/// need decay for one stop.
#[test]
fn a_basket_costs_one_tick_whatever_its_size() {
    let mut game = fresh();
    based(&mut game);
    docked(&mut game);
    give_credits(&mut game, 1_000_000);

    let offers = game.caravan_view().unwrap().offers;
    let many: Vec<usize> = offers.iter().take(3).map(|o| o.index).collect();
    assert!(many.len() > 1, "the fixture needs more than one row");
    let (copy, qty) = cargo_worth(&mut game, 1);

    let before = game.current_tick();
    game.commit_caravan_basket(vec![(copy, qty)], many)
        .expect("an affordable basket commits");
    assert_eq!(
        game.current_tick() - before,
        1,
        "a basket spends one turn, not one per line"
    );
}
