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
        relation.commerce_credits = 61;
        relation.traded = true;
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
    assert_eq!(
        relation.commerce_credits, 61,
        "the remainder did not survive the save"
    );
    assert!(
        relation.traded,
        "the trade latch did not survive the save -- every loaded town would \
         read as never introduced and hold the first-contact floor forever"
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

/// Trade is the accelerant, and `credit_trade_volume` is the one door every
/// counter sale and route delivery already goes through. Wiring the shelf
/// and leaving this unwired would ship a pull nothing can reach.
#[test]
fn trading_with_a_town_raises_its_commerce() {
    let mut game = game(4242);
    let key = a_known_key(&game);
    let before = commerce_of(&game, key);
    game.credit_trade_volume(
        key,
        crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT * 3,
    );
    let after = commerce_of(&game, key);
    assert_eq!(
        after - before,
        3,
        "three points' worth of trade bought {}",
        after - before
    );
}

/// A basket under the threshold buys nothing yet, and the remainder is not
/// lost -- `Relation::credit_trade`'s rule, restated on the commerce axis.
/// Without it, ten small baskets earn nothing while one large basket of the
/// same volume earns the lot.
#[test]
fn small_baskets_and_one_large_basket_buy_the_same_commerce() {
    let per = crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT;
    let mut split = game(4242);
    let mut whole = game(4242);
    let key = a_known_key(&split);
    for _ in 0..10 {
        split.credit_trade_volume(key, per / 10);
    }
    whole.credit_trade_volume(key, per);
    assert_eq!(commerce_of(&split, key), commerce_of(&whole, key));
    assert_eq!(commerce_of(&whole, key), 1);
}

/// The first authored `Mainframe` this seed materialized, sorted so a
/// re-run picks the same one -- `Settlements` is a `HashMap` and its
/// iteration order is not stable.
fn a_known_mainframe(game: &crate::Game) -> SettlementKey {
    let mut keys: Vec<_> = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .filter(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Mainframe)
        .map(|(key, _)| *key)
        .collect();
    keys.sort_by_key(|key| (key.rx, key.ry));
    *keys
        .first()
        .expect("test premise: this seed materializes at least one authored Mainframe")
}

/// **First contact is never worse than the authored baseline.** A fresh
/// run, the real tick loop, nobody trading and nobody setting a clock or a
/// flag -- and the authored Mainframe on this seed's map still draws the
/// ten rows its author wrote, all the way past the tick its drift used to
/// thin it at.
///
/// This is the test the whole amendment exists for. Before it, `Kernel
/// Reach` fell from 10 rows to 6 at tick 12100 in exactly this run, so a
/// player's first ever sight of a city could be a Server-sized shelf they
/// had no opportunity to prevent.
///
/// **Not vacuous.** The drift is not suppressed -- the run is carried until
/// the town's *unfloored* band is actually `Starved`, and that is asserted,
/// so the floor is observed holding against a band that would otherwise
/// have thinned the shelf rather than against a drift that never arrived.
#[test]
fn an_untraded_city_holds_its_authored_shelf_through_a_real_run() {
    let mut game = game(4242);
    let key = a_known_mainframe(&game);
    let baseline = game.settlement_shelf(key, 0).len();
    assert_eq!(
        baseline as u32,
        crate::tuning::SETTLEMENT_STEADY_ROWS,
        "test premise: an untouched authored Mainframe opens Steady"
    );

    // Past the tick the drift used to thin this town at, with room to spare.
    let ceiling = crate::tuning::SETTLEMENT_GROWTH_DUE_MAX + 2000;
    while game.current_tick() < ceiling {
        game.tick();
        assert!(
            game.is_game_over().is_none(),
            "the run ended before the drift did"
        );
        assert_eq!(
            game.settlement_shelf(key, 0).len(),
            baseline,
            "a city nobody has traded with thinned at tick {}, commerce {}",
            game.current_tick(),
            commerce_of(&game, key)
        );
    }

    let commerce = commerce_of(&game, key);
    assert_eq!(
        growth::vitality(commerce),
        growth::Vitality::Starved,
        "test premise: the drift never reached the band the floor is holding \
         against -- commerce {commerce}, so this proves nothing"
    );
}

/// The other half, and the one the floor could gut. A town the party
/// **has** traded with and then neglected still thins to the Server floor.
/// Without this the amendment would read as green while having deleted the
/// dwindle.
///
/// The basket is an exact multiple of `SETTLEMENT_COMMERCE_CREDITS_PER_POINT`,
/// which leaves `Relation::commerce_credits` at zero -- the reason that
/// remainder cannot be the "has ever traded" signal, asserted here rather
/// than argued in a comment.
#[test]
fn a_traded_then_neglected_city_still_thins_to_the_server_floor() {
    let mut game = game(4242);
    let key = a_known_mainframe(&game);
    let baseline = game.settlement_shelf(key, 0).len();
    game.credit_trade_volume(
        key,
        crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT * 2,
    );
    assert_eq!(
        game.world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(1, |relation| relation.commerce_credits),
        0,
        "test premise: the remainder reset, so it cannot be the signal"
    );

    // Far enough for the drift to claw back what the basket bought and
    // carry on down to Starved.
    let ceiling = crate::tuning::SETTLEMENT_GROWTH_DUE_MAX + 4000;
    let mut thinned_at = None;
    while game.current_tick() < ceiling {
        game.tick();
        if game.settlement_shelf(key, 0).len() < baseline {
            thinned_at = Some(game.current_tick());
            break;
        }
    }
    let tick = thinned_at.expect("a traded-then-neglected city never thinned at all");
    assert_eq!(
        game.settlement_shelf(key, 0).len() as u32,
        crate::tuning::SETTLEMENT_SERVER_ROWS,
        "it thinned at tick {tick} but not to the Server floor"
    );
}

/// The carve-out. A player who has made a town Hostile has had contact with
/// it, and the spec gives Hostile standing its own accelerated decay -- a
/// floor that survived hostility would render that acceleration inert for
/// every town the party never traded with.
#[test]
fn a_hostile_city_thins_even_with_nobody_ever_trading_there() {
    let mut game = game(4242);
    let key = a_known_mainframe(&game);
    let baseline = game.settlement_shelf(key, 0).len();
    game.adjust_standing(key, crate::tuning::SETTLEMENT_MIN_STANDING);
    assert_eq!(
        game.standing_band(key),
        crate::settlements::Standing::Hostile,
        "test premise: it is actually Hostile"
    );
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 20);
    game.settlement_growth_tick();
    assert_eq!(
        growth::vitality(commerce_of(&game, key)),
        growth::Vitality::Starved,
        "test premise: the accelerated drift reached Starved"
    );
    assert!(
        game.settlement_shelf(key, 0).len() < baseline,
        "a Hostile city held the untraded floor: still {baseline} rows"
    );
}

/// And the way back: repairing standing out of `Hostile` puts the floor
/// back for a town still never traded with. The carve-out reads the
/// *current* band, never a history -- `settle_commerce_drift`'s own rule,
/// restated on the floor.
#[test]
fn repairing_standing_restores_an_untraded_citys_floor() {
    let mut game = game(4242);
    let key = a_known_mainframe(&game);
    let baseline = game.settlement_shelf(key, 0).len();
    game.adjust_standing(key, crate::tuning::SETTLEMENT_MIN_STANDING);
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 20);
    game.settlement_growth_tick();
    assert!(
        game.settlement_shelf(key, 0).len() < baseline,
        "test premise: it thinned while Hostile"
    );
    game.adjust_standing(key, crate::tuning::SETTLEMENT_MAX_STANDING * 2);
    assert_ne!(
        game.standing_band(key),
        crate::settlements::Standing::Hostile
    );
    assert_eq!(
        game.settlement_shelf(key, 0).len(),
        baseline,
        "the floor did not come back when the town stopped being Hostile"
    );
}

/// Contact is contact. A basket too small to buy a single point of
/// commerce still counts as having dealt with the town, because the floor
/// is about whether the player was ever offered the chance to keep the
/// city — not about how much they spent. Written against the door rather
/// than the field, so it fails if the latch moves off `credit_trade_volume`.
#[test]
fn a_basket_too_small_to_buy_a_point_still_counts_as_contact() {
    let mut game = game(4242);
    let key = a_known_mainframe(&game);
    game.credit_trade_volume(key, 1);
    let relation = *game
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .expect("the door opened a record");
    assert_eq!(relation.commerce, 0, "test premise: it bought no commerce");
    assert!(
        relation.traded,
        "a Credit through the door did not count as contact"
    );
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .get_mut(&key)
        .unwrap()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MIN;
    assert_eq!(
        game.settlement_shelf(key, 0).len() as u32,
        crate::tuning::SETTLEMENT_SERVER_ROWS,
        "a town dealt with once still held the untraded floor"
    );
}

// ---------------------------------------------------------------------------
// The moment itself: the repaint, the line and the notification
// ---------------------------------------------------------------------------

/// Puts `key` one tick past a date it can reach now, so a single
/// `settlement_growth_tick` throws its latch. Commerce at the ceiling pulls
/// the date forward rather than the test ticking three thousand turns.
fn stand_a_server_on_its_date(game: &mut crate::Game, key: SettlementKey) {
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
    let due = growth::due_tick(world_seed(game), key);
    game.set_tick_for_test(due);
}

/// The glyph is baked into the entity at materialization, so a flip that
/// does not repaint it leaves the map saying `s` about a city until the
/// next load. Nothing else in this feature would notice.
#[test]
fn growing_repaints_the_map_glyph_in_place() {
    let mut game = game(4242);
    let key = a_known_server(&game);
    let glyph_of = |game: &mut crate::Game| {
        let mut query = game
            .world
            .query::<(&crate::components::Settlement, &crate::components::Glyph)>();
        query
            .iter(&game.world)
            .find(|(settlement, _)| settlement.key == key)
            .map(|(_, drawn)| drawn.ch)
            .expect("the town has an entity on the map")
    };
    assert_eq!(
        glyph_of(&mut game),
        crate::settlements::SettlementKind::Server.glyph(),
        "test premise: it is still drawn as a town"
    );

    stand_a_server_on_its_date(&mut game, key);
    game.settlement_growth_tick();

    assert_eq!(
        glyph_of(&mut game),
        crate::settlements::SettlementKind::Mainframe.glyph(),
        "the map still draws the town it used to be"
    );
}

/// A change the player can read, naming the town it happened to.
#[test]
fn growing_writes_a_line_naming_the_town() {
    let mut game = game(4242);
    let key = a_known_server(&game);
    let name = game.settlement_name(key);
    stand_a_server_on_its_date(&mut game, key);
    game.settlement_growth_tick();
    assert!(
        game.message_history(500)
            .iter()
            .any(|entry| entry.text.contains(&name)),
        "no line named {name}"
    );
}

/// It fires once. `settlement_growth_tick` runs every tick and the latch is
/// already set on the second one -- a missing "did this call flip it" check
/// would write the line every tick for the rest of the run.
///
/// **Counted by summing `repeats`, not by counting entries.**
/// `message_history` condenses a line repeated inside its lookback window
/// into one entry, so twenty copies of this line land as a single row and
/// an assertion on `len()` would pass with the bug in place.
#[test]
fn growing_announces_once_and_not_every_tick_after() {
    let written = |game: &crate::Game, name: &str| -> usize {
        game.message_history(500)
            .iter()
            .filter(|entry| entry.text.contains(name))
            .map(|entry| entry.repeats)
            .sum()
    };
    let mut game = game(4242);
    let key = a_known_server(&game);
    let name = game.settlement_name(key);
    stand_a_server_on_its_date(&mut game, key);
    game.settlement_growth_tick();
    assert_eq!(
        written(&game, &name),
        1,
        "test premise: the flip wrote the line exactly once"
    );
    for _ in 0..20 {
        game.settlement_growth_tick();
    }
    assert_eq!(
        written(&game, &name),
        1,
        "the growth line is still being written"
    );
}

/// The notification is gated on the party having actually stood there. A
/// notification takes the screen, and a city on the far side of the map
/// that the player has never reached interrupting them is the failure this
/// gate exists to prevent -- and it is invisible without a test, because the
/// log line fires either way.
///
/// The opening briefing is drained first: `Game::new` hands out the
/// onboarding chain's first mission and notifies for it, so an
/// undrained queue answers `Some` whatever growth does.
#[test]
fn only_a_visited_towns_growth_takes_the_screen() {
    let grow = |visited: bool| {
        let mut game = game(4242);
        let key = a_known_server(&game);
        game.world
            .resource_mut::<crate::resources::Settlements>()
            .0
            .get_mut(&key)
            .expect("the key came from this resource")
            .visited = visited;
        while game.take_notification().is_some() {}
        stand_a_server_on_its_date(&mut game, key);
        game.settlement_growth_tick();
        assert_eq!(
            game.settlement_kind(key),
            Some(crate::settlements::SettlementKind::Mainframe),
            "test premise: it grew"
        );
        game.take_notification().is_some()
    };
    assert!(
        grow(true),
        "a visited town's growth never reached the screen"
    );
    assert!(
        !grow(false),
        "a town the party has never stood in interrupted them"
    );
}

/// **Reachability, for the announcement.** `a_real_run_grows_a_town...`
/// proves a town grows in play; this proves the player is told. Nothing
/// here sets a clock, a latch or the `visited` flag by hand: the party
/// walks onto the town through `move_player` -- the one arm that records
/// having been there -- and then the real tick loop runs until the town
/// grows underneath them.
///
/// Without this the gate could ship dead. A notification that only ever
/// fires when a test writes `visited = true` is a screen no player reaches,
/// and the log line firing either way would hide it.
#[test]
fn a_real_run_shows_the_growth_screen_for_a_town_the_party_walked_to() {
    let mut game = game(4242);
    let key = a_known_server(&game);
    let tile = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .get(&key)
        .expect("the key came from this resource")
        .tile;

    // Stand the party one step off the town and walk the last tile, so the
    // visit itself goes through the shipped verb rather than a field write.
    // Which neighbour is walkable is the map's business, so try all four.
    let player = game.player_entity();
    let mut walked = false;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        {
            let mut pos = game
                .world
                .get_mut::<crate::components::Position>(player)
                .expect("the player stands somewhere");
            pos.x = tile.0 - dx;
            pos.y = tile.1 - dy;
        }
        game.move_player(dx, dy);
        if game.world.resource::<crate::resources::Settlements>().0[&key].visited {
            walked = true;
            break;
        }
    }
    assert!(walked, "the party could not reach {:?} from any side", tile);
    while game.take_notification().is_some() {}

    let ceiling = crate::tuning::SETTLEMENT_GROWTH_DUE_MAX
        + (-crate::tuning::SETTLEMENT_COMMERCE_MIN) as u64
            * crate::tuning::SETTLEMENT_COMMERCE_PULL_TICKS;
    while game.current_tick() < ceiling
        && game.settlement_kind(key) != Some(crate::settlements::SettlementKind::Mainframe)
    {
        game.tick();
        assert!(
            game.is_game_over().is_none(),
            "the run ended before the town did"
        );
    }
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe),
        "the town never grew inside the authored span"
    );

    let name = game.settlement_name(key);
    assert!(
        game.message_history(500)
            .iter()
            .any(|entry| entry.text.contains(&name)),
        "a real run grew {name} and wrote no line about it"
    );
    let titles: Vec<String> = std::iter::from_fn(|| game.take_notification())
        .map(|shown| shown.title)
        .collect();
    assert!(
        titles.iter().any(|title| title
            == crate::notifications::NotificationKind::SettlementGrown
                .def()
                .title),
        "a real run grew a town the party had walked to and never took the screen: {titles:?}"
    );
}
