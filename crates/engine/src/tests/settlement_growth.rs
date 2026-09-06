//! A Server growing into a Mainframe: the latch, the drift and the shelf.
//!
//! `docs/superpowers/specs/2026-09-06-settlement-growth-design.md`.

use crate::settlements::SettlementKey;

fn game(seed: u32) -> crate::Game {
    crate::Game::new(
        seed,
        crate::DifficultyMode::Forgiving,
        &crate::tests::support::test_assets_dir(),
    )
    .unwrap()
}

/// The first key `Game::new` materialized, which is a town that actually
/// exists on this seed's map rather than a coordinate we hoped held one.
fn a_known_key(game: &crate::Game) -> SettlementKey {
    *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .keys()
        .next()
        .expect("test premise: a new run materializes at least one settlement")
}

/// **Not a RON round-trip.** A `#[serde(default)]` field that no writer ever
/// sets round-trips perfectly while carrying nothing, so a round-trip test
/// would pass with the feature deleted. This drives a real save to disk and
/// loads it back.
#[test]
fn the_growth_fields_survive_a_save_and_load() {
    let dir = crate::tests::support::scratch_assets_dir("settlement_growth_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = game(4242);
    let key = a_known_key(&game);
    {
        let mut standings = game.world.resource_mut::<crate::resources::Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.grown = true;
        relation.commerce = 37;
        relation.commerce_epoch = 5;
    }
    game.save(&path).unwrap();

    let loaded = crate::Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    let relation = loaded
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .copied()
        .expect("the relation came back");
    assert!(relation.grown, "the latch did not survive the save");
    assert_eq!(relation.commerce, 37, "commerce did not survive the save");
    assert_eq!(
        relation.commerce_epoch, 5,
        "the epoch did not survive the save"
    );
}
