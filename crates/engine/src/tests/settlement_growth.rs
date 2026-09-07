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

use crate::settlements::growth;

/// The first authored `Server` this seed materialized. Every growth
/// assertion needs one, because a town the catalogue already wrote as a
/// Mainframe proves nothing about growing into one.
fn a_known_server(game: &crate::Game) -> SettlementKey {
    let mut keys: Vec<_> = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .filter(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| *key)
        .collect();
    // `Settlements` is a `HashMap`, so its iteration order is not stable
    // between runs. Sorting is what keeps a two-town test from picking a
    // different pair on a re-run and reading as a flake.
    keys.sort_by_key(|key| (key.rx, key.ry));
    *keys
        .first()
        .expect("test premise: this seed materializes at least one authored Server")
}

fn commerce_of(game: &crate::Game, key: SettlementKey) -> i32 {
    game.world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .map_or(0, |relation: &crate::settlements::Relation| {
            relation.commerce
        })
}

fn world_seed(game: &crate::Game) -> u32 {
    game.world.resource::<crate::world::WorldMap>().seed()
}

/// The whole feature in one assertion: a town past its date is a city.
#[test]
fn a_server_past_its_due_tick_grows() {
    let mut game = game(4242);
    let key = a_known_server(&game);
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Server),
        "test premise: it has not grown already"
    );
    // Enough commerce to pull the date to now, rather than ticking the
    // clock for three thousand turns.
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
    let due = growth::due_tick(world_seed(&game), key);
    game.set_tick_for_test(due);
    game.settlement_growth_tick();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe)
    );
}

/// The latch is one-way. Commerce decays; if the inequality were
/// re-evaluated on every read, a city would un-grow the moment its trade
/// dried up -- which the design explicitly refuses.
#[test]
fn a_grown_city_never_falls_back_to_a_server() {
    let mut game = game(4242);
    let key = a_known_server(&game);
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    {
        let mut standings = game.world.resource_mut::<crate::resources::Standings>();
        standings.0.get_mut(&key).unwrap().commerce = crate::tuning::SETTLEMENT_COMMERCE_MIN;
    }
    for _ in 0..40 {
        game.settlement_growth_tick();
    }
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe),
        "a starved city fell back to a town"
    );
}

/// Trade is what makes the difference. This is the test that must fail with
/// the pull removed -- run it that way before believing it.
#[test]
fn trade_pulls_a_growth_date_forward() {
    let mut game = game(4242);
    let key = a_known_server(&game);
    let due = growth::due_tick(world_seed(&game), key);
    // One tick short of the date, which is where the pull has to do the
    // work or nothing does.
    game.set_tick_for_test(due - 1);
    game.settlement_growth_tick();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Server),
        "it grew a tick early with no commerce at all"
    );
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = 1;
    game.settlement_growth_tick();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe),
        "a point of commerce bought nothing"
    );
}

/// Hostility triples the drift. Same elapsed epochs, two bands, two
/// answers.
#[test]
fn a_hostile_town_starves_faster_than_a_neglected_one() {
    let mut game = game(4242);
    let mut keys: Vec<_> = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .keys()
        .copied()
        .collect();
    keys.sort_by_key(|key| (key.rx, key.ry));
    keys.truncate(2);
    assert_eq!(keys.len(), 2, "test premise: two towns materialized");
    let (neglected, hated) = (keys[0], keys[1]);
    game.adjust_standing(hated, crate::tuning::SETTLEMENT_MIN_STANDING);
    assert_eq!(
        game.standing_band(hated),
        crate::settlements::Standing::Hostile,
        "test premise: it is actually Hostile"
    );
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 10);
    game.settlement_growth_tick();
    assert!(
        commerce_of(&game, hated) < commerce_of(&game, neglected),
        "Hostile drifted no faster: {} vs {}",
        commerce_of(&game, hated),
        commerce_of(&game, neglected)
    );
}

/// Repairing standing stops the acceleration the tick it lands -- the band
/// is read live, never as a history.
#[test]
fn repairing_standing_restores_the_slower_drift() {
    let mut game = game(4242);
    let key = a_known_key(&game);
    game.adjust_standing(key, crate::tuning::SETTLEMENT_MIN_STANDING);
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 5);
    game.settlement_growth_tick();
    let after_hostile = commerce_of(&game, key);

    game.adjust_standing(key, crate::tuning::SETTLEMENT_MAX_STANDING * 2);
    assert_ne!(
        game.standing_band(key),
        crate::settlements::Standing::Hostile
    );
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 10);
    game.settlement_growth_tick();
    let after_repair = commerce_of(&game, key);

    let hostile_rate = -after_hostile / 5;
    let repaired_rate = -(after_repair - after_hostile) / 5;
    assert!(
        repaired_rate < hostile_rate,
        "the repaired town kept the Hostile rate: {repaired_rate} vs {hostile_rate}"
    );
}

/// Discovery is not an event. A town materialized already past its date was
/// simply always a city -- the clock has been running whether or not anyone
/// was watching. Announcing here would name a place the party has never
/// seen.
///
/// Driven by walking the party into a region no run has resolved yet, not
/// by re-calling the resolver on ground it has already settled: that call
/// returns without materializing anything and the silence would prove
/// nothing.
#[test]
fn a_town_found_past_its_date_arrives_grown_and_silent() {
    let mut game = game(4242);
    let seed = world_seed(&game);
    // A region far outside the 3x3 block `Game::new` resolved, holding a
    // town the catalogue authored as a Server.
    let (key, def) = (4..40)
        .flat_map(|r| (-1..=1).map(move |dy| SettlementKey { rx: r, ry: dy }))
        .find_map(|key| {
            let placed = crate::settlements::settlement_at(
                seed,
                game.world.resource::<crate::settlements::SettlementDb>(),
                key,
            )?;
            let def = game
                .world
                .resource::<crate::settlements::SettlementDb>()
                .get(&placed.def_id)?
                .clone();
            (def.kind == crate::settlements::SettlementKind::Server).then_some((key, def))
        })
        .expect("test premise: some far region holds an authored Server");
    assert!(
        !game
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .contains_key(&key),
        "test premise: {key:?} has not been resolved yet"
    );

    // Stand the party in the middle of that region and put the clock past
    // its date, so the resolver meets a town that grew while nobody looked.
    let centre = crate::settlements::placement::REGION_TILES / 2;
    let player = game.player_entity();
    {
        let mut pos = game
            .world
            .get_mut::<crate::components::Position>(player)
            .unwrap();
        pos.x = key.rx * crate::settlements::placement::REGION_TILES + centre;
        pos.y = key.ry * crate::settlements::placement::REGION_TILES + centre;
    }
    game.set_tick_for_test(growth::due_tick(seed, key) + 1);

    let before = game.message_history(500).len();
    game.ensure_local_settlements();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe),
        "{} materialized as a {:?} despite being {} ticks past its date",
        def.name,
        game.settlement_kind(key),
        1
    );
    assert_eq!(
        game.message_history(500).len(),
        before,
        "materializing a settlement wrote a line"
    );
}

/// **Reachability.** Every other test in this file arranges the world by
/// hand. This one does not: a fresh run, the real tick loop, nobody trading
/// and nobody setting a clock or a flag -- and a town on the map is a city
/// by the end of it. Without this the whole feature could ship as a
/// consequence that is green in a unit test and unreachable in play.
#[test]
fn a_real_run_grows_a_town_with_nobody_touching_it() {
    let mut game = game(4242);
    let servers: Vec<SettlementKey> = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .filter(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| *key)
        .collect();
    assert!(
        !servers.is_empty(),
        "test premise: this seed materializes at least one authored Server"
    );

    // The bound is the authored ceiling plus the deepest a neglected town's
    // own decay can push it: nothing is trading here, so commerce runs
    // negative and the date slides later than the catalogue's span.
    let ceiling = crate::tuning::SETTLEMENT_GROWTH_DUE_MAX
        + (-crate::tuning::SETTLEMENT_COMMERCE_MIN) as u64
            * crate::tuning::SETTLEMENT_COMMERCE_PULL_TICKS;
    let mut grown_at = None;
    while game.current_tick() < ceiling {
        game.tick();
        assert!(
            game.is_game_over().is_none(),
            "the run ended before a town did"
        );
        if let Some(key) = servers.iter().find(|key| {
            game.settlement_kind(**key) == Some(crate::settlements::SettlementKind::Mainframe)
        }) {
            grown_at = Some((*key, game.current_tick()));
            break;
        }
    }
    let (key, tick) = grown_at.expect(
        "no authored Server ever grew in a real run -- the growth path is unreachable in play",
    );
    assert!(
        !game
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map(|relation| relation.commerce)
            .unwrap_or(0)
            .is_positive(),
        "test premise: nobody traded, so this town's commerce is not positive"
    );
    // It grew off the clock alone, so it cannot have grown before the
    // earliest date the catalogue allows.
    assert!(
        tick >= crate::tuning::SETTLEMENT_GROWTH_DUE_MIN,
        "a town grew at tick {tick}, before the authored floor"
    );
}

/// The one door commerce is written through clamps at both ends, which is
/// the only reason a single clamp is enough. A second writer that skipped
/// it would let a trade run commerce past the band thresholds and put a
/// city permanently at a row count the constants never authored.
#[test]
fn the_commerce_door_clamps_at_both_ends() {
    let mut game = game(4242);
    let key = a_known_key(&game);
    game.adjust_commerce(key, i32::MAX);
    assert_eq!(
        commerce_of(&game, key),
        crate::tuning::SETTLEMENT_COMMERCE_MAX
    );
    game.adjust_commerce(key, i32::MIN);
    assert_eq!(
        commerce_of(&game, key),
        crate::tuning::SETTLEMENT_COMMERCE_MIN
    );
}
