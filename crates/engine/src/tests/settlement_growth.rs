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

/// The load path is where this feature is most likely to break silently.
/// `restore_settlements` rebuilds every town's map entity from the record,
/// and if it draws from the authored `def.kind` a grown city reads `M` all
/// run and comes back from a save reading `s`. Nothing that never saves
/// would catch it.
#[test]
fn a_grown_town_still_draws_its_mainframe_glyph_after_a_load() {
    let dir = crate::tests::support::scratch_assets_dir("settlement_growth_glyph");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = game(4242);
    // A town the catalogue authored as a Server, so the glyph under test is
    // the grown one and not one it always had.
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    game.save(&path).unwrap();

    let mut loaded = crate::Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    let mut query = loaded
        .world
        .query::<(&crate::components::Settlement, &crate::components::Glyph)>();
    let glyph = query
        .iter(&loaded.world)
        .find(|(s, _)| s.key == key)
        .map(|(_, g)| g.ch)
        .expect("the grown town has an entity to draw");
    assert_eq!(
        glyph, 'M',
        "a grown town came back from a save drawing its authored Server glyph"
    );
}

/// An authored Mainframe is a Mainframe with nobody having done anything,
/// and a fresh Server is a Server. The door's base case, which every other
/// assertion in this file rests on.
#[test]
fn the_door_answers_the_authored_kind_before_anything_grows() {
    let game = game(4242);
    for (key, known) in &game.world.resource::<crate::resources::Settlements>().0 {
        assert_eq!(
            game.settlement_kind(*key),
            Some(known.def.kind),
            "{key:?} does not read as the catalogue authored it"
        );
    }
}

/// A grown Server draws a Mainframe's shelf. Without this the latch is a
/// flag nothing consumes.
#[test]
fn a_grown_server_draws_more_shelf_rows_than_it_did() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    let before = game.settlement_shelf(key, 0).len();
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    let after = game.settlement_shelf(key, 0).len();
    assert!(
        after > before,
        "a grown town's shelf did not deepen: {before} rows before, {after} after"
    );
}

/// A Server has no vitality band and must ignore commerce entirely. If it
/// read one, a town nobody trades with would quietly thin below the six
/// rows the shipped constant promises — a dwindle on the one settlement
/// that has nowhere to fall to.
#[test]
fn a_servers_shelf_ignores_commerce_at_every_band() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    let rows = |game: &mut crate::Game| game.settlement_shelf(key, 0).len();
    let baseline = rows(&mut game);
    for commerce in [
        crate::tuning::SETTLEMENT_COMMERCE_MIN,
        crate::tuning::SETTLEMENT_COMMERCE_STARVED,
        0,
        crate::tuning::SETTLEMENT_COMMERCE_THRIVING,
        crate::tuning::SETTLEMENT_COMMERCE_MAX,
    ] {
        game.world
            .resource_mut::<crate::resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .commerce = commerce;
        assert_eq!(
            rows(&mut game),
            baseline,
            "a Server's shelf moved at commerce {commerce}"
        );
    }
    assert_eq!(
        baseline as u32,
        crate::tuning::SETTLEMENT_SERVER_ROWS,
        "a Server draws something other than its own row count"
    );
}
