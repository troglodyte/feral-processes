//! Outposts: the record's save form — Phase 1, Task 2 of
//! `docs/superpowers/plans/2026-09-23-outposts.md`. The refusal ladder in
//! `Game::found_outpost` and the def loader in `outposts::OutpostDb` have
//! their own inline unit tests; this file is the cross-cutting save/load
//! round trip, `tests::routes`'s shape.

use std::collections::BTreeMap;

use super::support::{scratch_assets_dir, test_assets_dir};
use crate::Game;
use crate::items::ItemId;
use crate::outposts::Outpost;
use crate::resources::{DifficultyMode, Outposts};
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
    // Not part of `save::OutpostSave` — inert until Phase 5 re-seeds it
    // from the freshly-derived trend right after load.
    assert_eq!(after.announced, None);
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
