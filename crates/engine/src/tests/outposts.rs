//! Outposts: the record's save form (Phase 1) and growth/production
//! (Phase 2, Tasks 3–4) of `docs/superpowers/plans/2026-09-23-outposts.md`.
//! The refusal ladders in `Game::found_outpost`/`post_to_outpost` and the
//! def loader in `outposts::OutpostDb` have their own inline unit tests;
//! this file is the cross-cutting save/load round trip and the
//! `Game::run_outposts` integration tests, `tests::routes`'s shape.

use std::collections::BTreeMap;

use rand::RngExt;

use super::support::*;
use crate::Game;
use crate::components::PostedAt;
use crate::items::ItemId;
use crate::outposts::{Outpost, OutpostDb};
use crate::resources::{DifficultyMode, GameRng, Outposts};
use crate::tuning::{
    DEFAULT_BASE_INT, OUTPOST_CYCLE_TICKS, OUTPOST_MAX_INTEGRITY, OUTPOST_STOCK_CAP,
};
use crate::world::Biome;

/// A founded outpost with every field away from its zero value, so a round
/// trip that silently dropped one would still show a plausible-looking
/// record on the other side.
fn a_founded_outpost() -> Outpost {
    let mut stock = BTreeMap::new();
    stock.insert(ItemId("cache_grain".to_string()), 7);
    stock.insert(ItemId("static_mesh".to_string()), 3);
    Outpost {
        biome: Biome::Deadlock,
        growth: 42,
        integrity: 80,
        stock,
        stale_ticks: 5,
        cycle_progress: 11,
        announced: None,
    }
}

#[test]
fn a_founded_outpost_survives_a_real_save_round_trip() {
    let scratch = scratch_assets_dir("outpost_roundtrip");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(7000, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let before = a_founded_outpost();
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert((30, -12), before.clone());

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let outposts = &loaded.world.resource::<Outposts>().0;
    assert_eq!(
        outposts.len(),
        1,
        "the founded outpost must survive the load"
    );
    let after = &outposts[&(30, -12)];
    assert_eq!(after.biome, before.biome);
    assert_eq!(after.growth, before.growth);
    assert_eq!(after.integrity, before.integrity);
    assert_eq!(after.stock, before.stock);
    assert_eq!(after.stale_ticks, before.stale_ticks);
    assert_eq!(after.cycle_progress, before.cycle_progress);
    // Not part of `save::OutpostSave`, but `Game::reseed_outpost_announcements`
    // fixes it back to the real trend before `load` returns — this outpost
    // has no crew, which `outposts::trend` reads as `Declining` regardless
    // of every other field.
    assert_eq!(
        after.announced,
        Some(crate::outposts::Trend::Declining),
        "announced must be re-seeded to the current trend, not left None"
    );
}

#[test]
fn two_outposts_save_and_load_in_key_order() {
    let scratch = scratch_assets_dir("outpost_key_order");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(7001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    {
        let mut outposts = game.world.resource_mut::<Outposts>();
        // Inserted out of key order, so a save that merely preserved
        // insertion order would still pass a naive check.
        outposts.0.insert((50, 0), a_founded_outpost());
        outposts.0.insert((-5, 0), a_founded_outpost());
    }
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();

    let data = crate::save::load_from_file(&path).unwrap();
    let tiles: Vec<(i32, i32)> = data.outposts.iter().map(|o| o.tile).collect();
    assert_eq!(
        tiles,
        vec![(-5, 0), (50, 0)],
        "outposts save in BTreeMap (x, y) order, never insertion order"
    );

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(loaded.world.resource::<Outposts>().0.len(), 2);
}

/// A save written before outposts existed carries no `outposts` key at all,
/// and must load with none standing rather than refusing or panicking —
/// `tests::routes::a_pre_routes_save_loads_with_no_routes`'s shape.
#[test]
fn a_pre_outposts_save_loads_with_none_standing() {
    let scratch = scratch_assets_dir("outpost_pre_save");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(7002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert((1, 1), a_founded_outpost());
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();

    // Stripped to what a save written before this field existed looked
    // like — the real save's own `outposts` key, removed, rather than a
    // hand-built RON fixture that only proves the parser accepts an absent
    // field.
    let mut data = crate::save::load_from_file(&path).unwrap();
    data.outposts.clear();
    let text = crate::save::to_ron(&data).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("outposts:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() < text.lines().count(),
        "the key must have been there to strip, or this proves nothing"
    );
    let old_path = scratch.join("old.bin");
    let stripped_data = crate::save::from_ron(&stripped).expect("a pre-outposts save still parses");
    crate::save::save_to_file(&old_path, &stripped_data).unwrap();

    let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
    assert!(
        loaded.world.resource::<Outposts>().0.is_empty(),
        "a pre-outposts save has none standing"
    );
}

/// The whole feature is additive behind `#[serde(default)]` — no
/// `SAVE_FORMAT_VERSION` bump. `tests::routes::save_format_version_is_unchanged_by_routes`'s
/// shape.
#[test]
fn save_format_version_is_unchanged_by_outposts() {
    assert_eq!(
        crate::save::SAVE_FORMAT_VERSION,
        32,
        "adding an outpost field is additive under field-named RON and must \
         not cost a version bump — see the doc comment on SAVE_FORMAT_VERSION"
    );
}

// ---------------------------------------------------------------------
// Crew round trip: `CreatureSave::outpost`
// ---------------------------------------------------------------------

/// Two crew members posted at a real, saved outpost record keep both their
/// `PostedAt` marker and their role across a real save/load round trip.
#[test]
fn posted_crew_survives_a_save_and_load() {
    let scratch = scratch_assets_dir("outpost_crew_roundtrip");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(7010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (60, -20);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, a_founded_outpost());
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(first).insert(PostedAt(tile));
    game.world.entity_mut(second).insert(PostedAt(tile));

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    assert_eq!(
        loaded.outpost_crew(tile).len(),
        2,
        "both crew members must come back posted"
    );
    for member in loaded.outpost_crew(tile) {
        assert_eq!(
            loaded.program_role(member),
            Some(crate::ProgramRole::Outpost)
        );
    }
}

/// A crew member whose outpost tile no longer names a record drops the
/// membership on load and rejoins the base staff — `nest_position`'s
/// leniency, `Game::attach_outpost_crew`'s own doc.
#[test]
fn an_orphaned_crew_membership_is_dropped_on_load() {
    let scratch = scratch_assets_dir("outpost_crew_orphan");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(7011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (61, -21);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, a_founded_outpost());
    let program = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(program).insert(PostedAt(tile));
    // The record is gone by the time this saves — simulating an outpost
    // destroyed or edited away between sessions, without needing to
    // hand-edit the RON on disk.
    game.world.resource_mut::<Outposts>().0.remove(&tile);

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let restored = loaded
        .world
        .iter_entities()
        .find(|e| e.contains::<crate::components::Tamed>())
        .expect("the program itself survives the load")
        .id();
    assert!(
        loaded.world.get::<PostedAt>(restored).is_none(),
        "an orphaned membership must not be reattached"
    );
}

// ---------------------------------------------------------------------
// Production: `Game::run_outposts` — Phase 2, Task 4
// ---------------------------------------------------------------------

/// One crew member, freshly founded (tier 0), one tick short of its first
/// production cycle — every shipped tier-1 row names exactly one item, so
/// this is also the fixture the single-entry-union test wants.
fn an_outpost_ready_to_cycle(seed: u32) -> (Game, (i32, i32)) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    let mut outpost = Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY);
    outpost.cycle_progress = OUTPOST_CYCLE_TICKS - 1;
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, outpost);
    let crew = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(crew).insert(PostedAt(tile));
    (game, tile)
}

fn peek(g: &mut Game) -> u64 {
    use rand::RngExt;
    g.world.resource_mut::<GameRng>().0.random()
}

/// The whole feature's own no-draw guarantee, `routes`'s and `sorties`'
/// shape: comparing the stream is what tells a system that draws and
/// discards from one that never touched `GameRng` at all.
#[test]
fn run_outposts_draws_no_rng_with_no_outposts_standing() {
    let mut game = Game::new(7020, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.world.resource::<Outposts>().0.is_empty());

    reseed_rng(&mut game, 55);
    let without = peek(&mut game);

    reseed_rng(&mut game, 55);
    game.run_outposts();
    let with = peek(&mut game);

    assert_eq!(without, with, "no outpost standing must draw nothing");
}

/// A tier-0 union names exactly one item in every shipped biome, so a
/// cycle's item pick must cost no draw beyond the success roll —
/// `outposts::yields`'s own rule. Proven by replaying the identical success
/// roll by hand and comparing the stream afterward: any extra draw the real
/// call made shows up as a mismatch here, whatever seed is used, since a
/// failed roll and a succeeding one both consume exactly the manual
/// reference's one draw unless an item pick follows it.
#[test]
fn run_outposts_draws_no_extra_rng_when_the_union_has_one_item() {
    let build_seed = 7021;
    let (mut real, _) = an_outpost_ready_to_cycle(build_seed);
    let (mut reference, _) = an_outpost_ready_to_cycle(build_seed);

    let rng_seed = 5;
    reseed_rng(&mut real, rng_seed);
    real.run_outposts();
    let after_real = peek(&mut real);

    reseed_rng(&mut reference, rng_seed);
    let chance = crate::systems::mining_success_chance(1, 0, DEFAULT_BASE_INT, 0.0, 0.0);
    let _ = reference
        .world
        .resource_mut::<GameRng>()
        .0
        .random_bool(chance);
    let after_reference = peek(&mut reference);

    assert_eq!(
        after_real, after_reference,
        "a single-entry union must draw exactly the success roll and nothing more"
    );
}

/// Loads a two-item tier-0 union from a scratch `assets/outposts/` dir,
/// rather than reaching into `OutpostDb`'s private field — `OutpostDb::
/// load_dir`'s own public door, `game::outposts::tests`'s pattern for a
/// custom def. `OutpostDef` derives no `Serialize` (it is load-only
/// content), so this is hand-written RON rather than a serialized struct.
fn install_two_item_outpost_def(game: &mut Game, tag: &str) {
    let text = r#"
OutpostDef(
    name: "Test Outpost",
    glyph: '?',
    kit: "outpost_kit",
    tiers: [
        ( yields: { Deadlock: ["raw_trace", "static_mesh"] } ),
    ],
    features: [],
)
"#;
    let dir = scratch_assets_dir(tag).join("outposts");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("outpost.ron"), text).unwrap();
    let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
    assert!(
        warnings.is_empty(),
        "the written def must parse: {warnings:?}"
    );
    assert!(db.def().is_some());
    game.world.insert_resource(db);
}

/// The counterpart: a two-item union costs the item-pick draw on top of the
/// success roll, found via a seed search — `first_rng_seed_where`'s reason,
/// since only a succeeding roll ever reaches the pick.
#[test]
fn run_outposts_draws_an_extra_rng_value_when_the_union_has_two_items_and_the_roll_succeeds() {
    let build = |rng_seed: u64| {
        let (mut game, tile) = an_outpost_ready_to_cycle(9000);
        install_two_item_outpost_def(&mut game, &format!("outpost_two_item_{rng_seed}"));
        reseed_rng(&mut game, rng_seed);
        (game, tile)
    };
    let chance = crate::systems::mining_success_chance(1, 0, DEFAULT_BASE_INT, 0.0, 0.0);
    let mut found_seed = None;
    for rng_seed in 0..64u64 {
        let (mut probe, tile) = build(rng_seed);
        probe.run_outposts();
        let stock: u32 = probe.world.resource::<Outposts>().0[&tile]
            .stock
            .values()
            .sum();
        if stock > 0 {
            found_seed = Some(rng_seed);
            break;
        }
    }
    let rng_seed = found_seed.expect("a two-item union should succeed within 64 seeds");

    let (mut real, _) = build(rng_seed);
    real.run_outposts();
    let after_real = peek(&mut real);

    let (mut reference, _) = build(rng_seed);
    let _ = reference
        .world
        .resource_mut::<GameRng>()
        .0
        .random_bool(chance);
    let after_reference = peek(&mut reference);

    assert_ne!(
        after_real, after_reference,
        "a succeeding roll against a two-item union must draw a second value"
    );
}

/// A dark outpost (zero integrity, Phase 5's raid-to-zero outcome) skips
/// growth and production entirely — nothing in the record moves.
#[test]
fn a_dark_outpost_neither_grows_nor_produces() {
    let (mut game, tile) = an_outpost_ready_to_cycle(7029);
    {
        let mut outposts = game.world.resource_mut::<Outposts>();
        outposts.0.get_mut(&tile).unwrap().integrity = 0;
    }
    let before = game.world.resource::<Outposts>().0[&tile].clone();

    for _ in 0..(OUTPOST_CYCLE_TICKS * 5) {
        game.run_outposts();
    }

    let after = game.world.resource::<Outposts>().0[&tile].clone();
    assert_eq!(
        before, after,
        "a dark outpost's record must not change at all"
    );
    // Deleted-fix check: without the integrity guard `cycle_progress` and
    // `stale_ticks` both advance here, and a crew that happened to succeed
    // would add to `stock` on top.
}

/// Stock never exceeds `OUTPOST_STOCK_CAP`, even run well past the point a
/// perfectly reliable crew would have filled it.
#[test]
fn stock_stops_at_the_cap() {
    let (mut game, tile) = an_outpost_ready_to_cycle(7030);
    for _ in 0..(OUTPOST_CYCLE_TICKS * (OUTPOST_STOCK_CAP + 20)) {
        game.run_outposts();
    }
    let total: u32 = game.world.resource::<Outposts>().0[&tile]
        .stock
        .values()
        .sum();
    assert!(total <= OUTPOST_STOCK_CAP, "stock {total} exceeded the cap");
}

/// A production success reports through `base_ledger::emit` as the base
/// output page already reads it — `Event::Extract`'s `produced` fold.
#[test]
fn a_successful_cycle_raises_the_ledger_produced_count() {
    let (mut game, _) = an_outpost_ready_to_cycle(7031);
    let before: u32 = game
        .world
        .resource::<crate::base_ledger::BaseLedger>()
        .lifetime
        .values()
        .map(|t| t.mined)
        .sum();
    // A handful of cycles, since any one crew roll may fizzle — the ledger
    // total across several attempts is what a flaky single roll cannot make
    // vacuous.
    for _ in 0..(OUTPOST_CYCLE_TICKS * 30) {
        game.run_outposts();
    }
    let after: u32 = game
        .world
        .resource::<crate::base_ledger::BaseLedger>()
        .lifetime
        .values()
        .map(|t| t.mined)
        .sum();
    assert!(after > before, "at least one of thirty cycles should land");
}

/// A seeded run of many cycles is deterministic — the same seed and the
/// same fixture must land on the same stock, or a build's own retune would
/// be indistinguishable from a flaky test.
#[test]
fn a_seeded_run_of_many_cycles_is_deterministic() {
    let seed = 7032;
    let run = || {
        let (mut game, tile) = an_outpost_ready_to_cycle(seed);
        reseed_rng(&mut game, 20260923);
        for _ in 0..(OUTPOST_CYCLE_TICKS * 25) {
            game.run_outposts();
        }
        game.world.resource::<Outposts>().0[&tile].stock.clone()
    };
    assert_eq!(run(), run());
}

// ---------------------------------------------------------------------
// Alerts: `Game::announce_outpost_trend` and the reload latch — Phase 5,
// design correction 10.
// ---------------------------------------------------------------------

/// A crewless outpost is `Trend::Declining` from its very first tick
/// (`outposts::trend`'s crew-floor check), which posts an `OutpostDeclining`
/// alert on the transition out of `announced: None`.
#[test]
fn an_uncrewed_outpost_posts_a_declining_alert_on_its_first_tick() {
    let mut game = Game::new(7040, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY));

    game.run_outposts();

    let alerts = game.alerts();
    let alert = alerts
        .iter()
        .find(|a| a.kind == crate::alerts::AlertKind::OutpostDeclining)
        .expect("an uncrewed outpost must post OutpostDeclining on its first tick");
    assert_eq!(alert.count, 1);
    // Deleted-fix check: without `announce_outpost_trend`'s call from
    // `tick_one_outpost`, `alerts` above is empty and this `find` panics.
}

/// Holding the same trend across many ticks posts nothing further — the
/// board's own `count` would climb past 1 if `announced` didn't latch it,
/// since `alerts::post` collapses same-kind-same-subject posts by bumping
/// rather than by refusing a repeat.
#[test]
fn holding_the_same_declining_state_posts_nothing_more() {
    let mut game = Game::new(7041, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY));

    for _ in 0..(OUTPOST_CYCLE_TICKS * 5) {
        game.run_outposts();
    }

    let alerts = game.alerts();
    let alert = alerts
        .iter()
        .find(|a| a.kind == crate::alerts::AlertKind::OutpostDeclining)
        .unwrap();
    assert_eq!(
        alert.count, 1,
        "a trend that never changes must post exactly once, however many ticks it holds"
    );
}

/// A reload does not re-post an alert for a state the player already saw —
/// `Game::reseed_outpost_announcements`' whole purpose. Saved while
/// `Declining`, so `announced` on disk (dropped: it isn't part of
/// `OutpostSave`) would come back `None` without the re-seed, and the very
/// next tick would post a second time.
#[test]
fn a_reload_does_not_post_the_same_alert_again() {
    let scratch = scratch_assets_dir("outpost_alert_reload");
    std::fs::create_dir_all(&*scratch).unwrap();
    let mut game = Game::new(7042, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY));
    game.run_outposts();
    assert_eq!(
        game.alerts()
            .iter()
            .find(|a| a.kind == crate::alerts::AlertKind::OutpostDeclining)
            .unwrap()
            .count,
        1
    );

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    loaded.run_outposts();

    let alert = loaded
        .alerts()
        .into_iter()
        .find(|a| a.kind == crate::alerts::AlertKind::OutpostDeclining)
        .unwrap();
    assert_eq!(
        alert.count, 1,
        "a reload must not re-post an alert for a state already announced before the save"
    );
}

/// `Trend::Stale` posts its own kind, distinct from `Declining` — enough
/// crew to clear the floor, growth already at the top tier (so a full crew
/// reads `Stable`/`Stale` rather than `Growing`), and stock sitting at the
/// cap.
#[test]
fn stale_posts_its_own_alert_kind() {
    let mut game = Game::new(7043, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    let mut outpost = Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY);
    outpost.growth =
        crate::tuning::OUTPOST_TIER_GROWTH[crate::tuning::OUTPOST_TIER_GROWTH.len() - 1];
    outpost
        .stock
        .insert(ItemId("raw_trace".to_string()), OUTPOST_STOCK_CAP);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, outpost);
    for _ in 0..*crate::tuning::OUTPOST_TIER_CREW.last().unwrap() {
        let crew = spawn_tamed(&mut game, 10, 3);
        game.world.entity_mut(crew).insert(PostedAt(tile));
    }

    game.run_outposts();

    assert!(
        game.alerts()
            .iter()
            .any(|a| a.kind == crate::alerts::AlertKind::OutpostStale),
        "a fully staffed, fully grown, full-stock outpost must post OutpostStale"
    );
}

// ---------------------------------------------------------------------
// Attention: `Game::attention` — Phase 5, design correction 10.
// ---------------------------------------------------------------------

/// A declining outpost earns an `AttentionKind::OutpostTrend` row, worded
/// with its tile — `Game::attention`'s own row-per-outpost rule.
#[test]
fn a_declining_outpost_earns_an_attention_row() {
    let mut game = Game::new(7044, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY));

    let rows = game.attention();

    assert!(
        rows.iter()
            .any(|r| r.kind == crate::views::AttentionKind::OutpostTrend && r.text.contains("400")),
        "a crewless, declining outpost must earn an attention row naming its tile: {rows:?}"
    );
}

/// A dark outpost earns `AttentionKind::OutpostDark` instead of the trend
/// row — `Game::attention`'s dark branch returns before reading `Trend` at
/// all.
#[test]
fn a_dark_outpost_earns_its_own_attention_row() {
    let mut game = Game::new(7045, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tile = (400, 400);
    let mut outpost = Outpost::new(Biome::Deadlock, OUTPOST_MAX_INTEGRITY);
    outpost.integrity = 0;
    game.world
        .resource_mut::<Outposts>()
        .0
        .insert(tile, outpost);

    let rows = game.attention();

    assert!(
        rows.iter()
            .any(|r| r.kind == crate::views::AttentionKind::OutpostDark),
        "a dark outpost must earn its own attention row: {rows:?}"
    );
    assert!(
        !rows
            .iter()
            .any(|r| r.kind == crate::views::AttentionKind::OutpostTrend),
        "a dark outpost must not also earn a trend row"
    );
}

/// A healthy, growing outpost earns no attention row at all.
#[test]
fn a_healthy_outpost_earns_no_attention_row() {
    let (mut game, _tile) = an_outpost_ready_to_cycle(7046);

    let rows = game.attention();

    assert!(
        !rows.iter().any(|r| matches!(
            r.kind,
            crate::views::AttentionKind::OutpostTrend | crate::views::AttentionKind::OutpostDark
        )),
        "a single-crew, freshly founded outpost is Growing and must earn no row: {rows:?}"
    );
}
