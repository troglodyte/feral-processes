//! Build requests: filing one, the crew fetching and raising it, what
//! outranks what, and what a cancel gives back.

use super::support::*;
use crate::*;

/// A Home on the player's own tile, the party standing in the base, and a
/// pack full of Core Fragments — enough to file anything these fixtures
/// need. Deliberately **no** staff: every test here says explicitly whether
/// there is a body to build, because that is the axis half of them are
/// about.
fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    game
}

/// The `BuildSite` standing at the party's cell plus `(dx, dy)`.
fn site_at(game: &mut Game, dx: i32, dy: i32) -> Entity {
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    game.build_site_at(px + dx, py + dy)
        .expect("a request was filed on that cell")
}

fn structure_at(game: &mut Game, dx: i32, dy: i32) -> Option<Entity> {
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    let (x, y) = (px + dx, py + dy);
    let mut query = game.world.query::<(Entity, &Position, &Structure)>();
    query
        .iter(&game.world)
        .find(|(_, p, _)| p.x == x && p.y == y)
        .map(|(e, ..)| e)
}

/// A worker with enough Integrity to outlast the ambient GC Entropy Sweeps
/// these fixtures tick through — `tests::hauling::hauler`'s reason exactly.
fn builder(game: &mut Game) -> Entity {
    spawn_tamed(game, 500, 3)
}

/// The headline: a deploy no longer deploys.
///
/// It files a request, charges nothing, and leaves the cell holding a
/// `BuildSite` rather than a `Structure` — which is the whole shape of the
/// feature and the thing every test below builds on.
#[test]
fn deploying_files_a_request_rather_than_standing_a_structure_up() {
    let mut game = base(1101);
    let before = count_item(&game, ids::CORE_FRAGMENT);

    game.place_structure("mining_node", 1, 0).unwrap();

    assert!(
        structure_at(&mut game, 1, 0).is_none(),
        "nothing is standing there yet — a program has to build it"
    );
    let site = site_at(&mut game, 1, 0);
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        before,
        "filing charges nothing: the materials are fetched by hand later"
    );
    let build = game.world.get::<BuildSite>(site).expect("it is a site");
    assert_eq!(build.structure, "mining_node");
    assert!(
        build.delivered.is_empty(),
        "nothing has been carried there yet"
    );
}

/// The whole loop, end to end: a body walks to the party, takes the
/// materials out of the pack, carries them to the cell, and raises the
/// structure.
///
/// **Asserted on the finished structure's components**, not merely on its
/// existence: `Game::spawn_structure` is the one place a structure's
/// component list is written precisely so a crew-built machine cannot come
/// out missing the `ResourceNode` or the `MachineStatus` a player-built one
/// has. Checking only that *something* stands there passes against a
/// half-written spawn.
#[test]
fn the_crew_fetches_the_materials_and_raises_the_structure() {
    let mut game = base(1102);
    builder(&mut game);

    game.place_structure("mining_node", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);
    let cost: u32 = game.world.get::<BuildSite>(site).unwrap().total_materials();
    let held_before = count_item(&game, ids::CORE_FRAGMENT);

    for _ in 0..400 {
        if structure_at(&mut game, 1, 0).is_some() {
            break;
        }
        game.tick();
    }

    let built = structure_at(&mut game, 1, 0).expect("the crew raises it");
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        held_before - cost,
        "and every unit it cost came out of the pack the crew fetched from"
    );
    assert!(
        game.world.get::<ResourceNode>(built).is_some(),
        "a crew-built extractor is a real extractor — `spawn_structure` is the one component list"
    );
    assert!(
        game.world.get::<MachineStatus>(built).is_some(),
        "and it has a status like any other machine"
    );
    assert!(
        game.world.get::<Stock>(built).is_some(),
        "and a buffer to produce into"
    );
}

/// A builder fetches from the base's own shelves, not only from the pack.
///
/// The party is sent out of base space entirely, so the pack is not a
/// source at all — a builder walks over to *you*, and there has to be a
/// pair of hands there to take from. What is left is a Depot holding the
/// materials, which is what the stock strip counts.
#[test]
fn a_builder_draws_from_a_base_shelf_with_the_party_away() {
    let mut game = base(1103);
    builder(&mut game);
    place_now(&mut game, "depot", 0, 1).unwrap();
    game.place_structure("mining_node", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);
    let cost: u32 = game.world.get::<BuildSite>(site).unwrap().total_materials();

    // Fill the Depot, then empty the pack and leave: the shelf is now the
    // only store in the game holding a Core Fragment.
    let depot = structure_at(&mut game, 0, 1).expect("a Depot stands there");
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cost);
    let held = count_item(&game, ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::CORE_FRAGMENT), held);

    for _ in 0..400 {
        if structure_at(&mut game, 1, 0).is_some() {
            break;
        }
        game.tick();
    }

    assert!(
        structure_at(&mut game, 1, 0).is_some(),
        "the crew fetched off the shelf and raised it"
    );
    assert_eq!(
        game.world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0),
        0,
        "and the shelf paid for it"
    );
}

/// A load is `HAUL_CARRY_CAPACITY` at a time, so a bill bigger than one
/// carry takes several trips.
///
/// **Asserted by watching the site fill in steps**, not by counting ticks: a
/// body that could teleport the whole bill would land it in one write, and
/// the intermediate partial delivery is the only observable difference. The
/// structure chosen costs more than one carry — checked here rather than
/// assumed, so a retune of its recipe fails loudly instead of making this
/// test vacuous.
#[test]
fn a_bill_bigger_than_one_carry_takes_several_trips() {
    let mut game = base(1104);
    builder(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    game.place_structure("armory", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);
    let total = game.world.get::<BuildSite>(site).unwrap().total_materials();
    assert!(
        total > tuning::HAUL_CARRY_CAPACITY,
        "the fixture needs a bill bigger than one carry to say anything: {total}"
    );

    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..600 {
        if let Some(build) = game.world.get::<BuildSite>(site) {
            seen.insert(build.delivered.iter().map(|(_, q)| q).sum::<u32>());
        } else {
            break;
        }
        game.tick();
    }

    let partial: Vec<u32> = seen
        .iter()
        .copied()
        .filter(|&n| n > 0 && n < total)
        .collect();
    assert!(
        !partial.is_empty(),
        "the site should be seen part-supplied between trips, not jump from 0 to {total}: {seen:?}"
    );
    assert!(
        partial
            .iter()
            .all(|n| n % tuning::HAUL_CARRY_CAPACITY == 0 || *n == total),
        "every intermediate figure should be a whole number of carries: {partial:?}"
    );
}

/// A build outranks a work order: the base takes its only body off a
/// machine to raise the request.
///
/// **The priority *is* the position in `schedule_base_labour`'s want list**,
/// since `truncate(staff.len())` cuts from the end — so with one body and
/// two wants, which job it holds is the whole assertion. Filed *after* the
/// order and after the body is already posted, so this cannot pass by the
/// build simply having been there first.
#[test]
fn a_build_request_takes_the_body_off_a_work_order() {
    let mut game = base(1105);
    let body = builder(&mut game);
    place_now(&mut game, "mining_node", 0, 1).unwrap();
    let node = structure_at(&mut game, 0, 1).expect("a node stands there");
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50));
    game.tick();
    assert_eq!(
        game.world.get::<Task>(body).map(|t| t.target),
        Some(node),
        "the one body works the order first"
    );

    game.place_structure("depot", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);
    game.tick();

    let task = game.world.get::<Task>(body).expect("it is still posted");
    assert_eq!(
        (task.kind, task.target),
        (TaskKind::Construct, site),
        "and the build takes it off the machine, because a build outranks an order"
    );
}

/// Cancelling gives the delivered materials back.
///
/// The units left their shelf when a builder picked them up and have been
/// standing on the cell ever since — `run_build_crew` does not spend them
/// until the structure is raised — so a cancel is a refund of goods that
/// still exist. They go back to a Depot, never into a machine's output
/// buffer, where they would read as something that machine produced.
#[test]
fn cancelling_returns_what_was_already_carried_to_the_site() {
    let mut game = base(1106);
    builder(&mut game);
    place_now(&mut game, "depot", 0, 1).unwrap();
    let depot = structure_at(&mut game, 0, 1).expect("a Depot stands there");
    game.place_structure("mining_node", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);

    // Let the crew carry at least one load to the cell.
    let mut delivered = 0;
    for _ in 0..400 {
        delivered = game
            .world
            .get::<BuildSite>(site)
            .map(|b| b.delivered.iter().map(|(_, q)| q).sum::<u32>())
            .unwrap_or(0);
        if delivered > 0 {
            break;
        }
        game.tick();
    }
    assert!(delivered > 0, "the fixture needs a part-supplied site");
    let banked = game
        .world
        .get::<Stock>(depot)
        .unwrap()
        .output
        .get(&ItemId::from(ids::CORE_FRAGMENT))
        .copied()
        .unwrap_or(0);

    game.cancel_build_request(site).unwrap();

    assert!(
        game.world.get::<BuildSite>(site).is_none(),
        "the request is gone"
    );
    assert_eq!(
        game.world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0),
        banked + delivered,
        "and every unit standing on the cell went back on the shelf"
    );
}

/// A request survives a reload with the materials already carried to it.
///
/// **`delivered` is the load-bearing save field.** Those units left their
/// shelves when a builder picked them up; dropped from the save they would
/// be destroyed by a reload and the crew would fetch them a second time out
/// of a base that no longer had them. Asserted through a real round trip
/// rather than on the `SaveData` struct, since a `#[serde(skip)]` leaves the
/// struct-level test green.
#[test]
fn a_part_supplied_request_survives_a_reload() {
    let mut game = base(1107);
    builder(&mut game);
    game.place_structure("mining_node", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);
    let mut delivered = 0;
    for _ in 0..400 {
        delivered = game
            .world
            .get::<BuildSite>(site)
            .map(|b| b.delivered.iter().map(|(_, q)| q).sum::<u32>())
            .unwrap_or(0);
        if delivered > 0 {
            break;
        }
        game.tick();
    }
    assert!(delivered > 0, "the fixture needs a part-supplied site");
    let (x, y) = {
        let p = game.world.get::<Position>(site).unwrap();
        (p.x, p.y)
    };
    let cost = game.world.get::<BuildSite>(site).unwrap().cost.clone();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_build_site_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut reloaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    stand_in_base(&mut reloaded);
    let site = reloaded
        .build_site_at(x, y)
        .expect("the request came back on its own cell");
    let build = reloaded.world.get::<BuildSite>(site).unwrap();
    assert_eq!(
        build.delivered.iter().map(|(_, q)| q).sum::<u32>(),
        delivered,
        "with what had already been carried there still standing on it"
    );
    assert_eq!(build.cost, cost, "and the bill it was filed against");
}

/// The Home is the one structure the player still stands up by hand.
///
/// Founding is the one build with nobody to ask — base space does not exist
/// yet, so there is no roster standing in it and no shelf to fetch from —
/// which is why it keeps the pack-charging, refuse-on-shortfall shape the
/// rest of the verb has shed.
#[test]
fn a_home_is_still_placed_by_the_player_and_paid_for_on_the_spot() {
    let mut game = Game::new(1108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = count_item(&game, ids::CORE_FRAGMENT);

    game.place_structure("home", 0, 0)
        .expect("a Home costs nothing and founds the base");

    stand_in_base(&mut game);
    assert!(
        find_structure_by_kind(&mut game, "home").is_some(),
        "the Home is standing the moment the call returns — no crew involved"
    );
    assert!(
        count_item(&game, ids::CORE_FRAGMENT) <= before,
        "and it was paid for out of the pack rather than fetched"
    );
}

/// A second request on a cell already spoken for is refused, and refused
/// *distinctly* from a cell with a structure on it — the two leave the
/// player different errands.
#[test]
fn a_cell_already_on_order_refuses_a_second_request() {
    let mut game = base(1109);
    game.place_structure("mining_node", 1, 0).unwrap();

    let err = game
        .place_structure("depot", 1, 0)
        .expect_err("the cell is already spoken for");
    assert!(
        err.contains("already set to build"),
        "the refusal should say the crew is behind, not that something is deployed: {err}"
    );
}

/// A request counts against `max_deployed` alongside what is already
/// standing.
///
/// `spawn_structure` performs no checks at all — by the time a crew finishes
/// a request they were answered when it was filed — so if the ceiling only
/// counted built structures, a player could queue a whole base's worth of a
/// capped machine and every one of them would be raised.
///
/// Driven off the shipped `max_deployed` rather than a literal, so retuning
/// the cap moves the test with it; the cap is asserted to exist at all,
/// since a roster with none would make this pass against no fix.
#[test]
fn requests_count_against_the_deployment_ceiling() {
    let mut game = base(1110);
    unlock_research_chain(&mut game, "cache_coherence");
    let capped = game
        .buildable_structure_defs()
        .into_iter()
        .find(|d| d.max_deployed > 0)
        .expect("some shipped structure carries a deployment ceiling");
    let cap = capped.max_deployed;

    // File exactly the ceiling, each on its own cell.
    for i in 0..cap as i32 {
        game.place_structure(&capped.id, 1, i - 1)
            .unwrap_or_else(|e| panic!("request {i} should be filed: {e}"));
    }
    let err = game
        .place_structure(&capped.id, -1, 0)
        .expect_err("one past the ceiling, even though nothing has been raised");
    assert!(
        err.contains("as many as this grid will hold"),
        "unexpected refusal: {err}"
    );
}

/// A request nobody can reach must not eat a body for the rest of the run.
///
/// **The starvation shape, and it is silent.** `run_build_crew` gives the
/// post up when the walk fails — that is what keeps the stall announcement
/// honest, since the scheduler only ever looks at sites nobody is posted to
/// — so an unreachable site posts a body, loses it in the same tick, and is
/// handed the same body again on the next. The body never reaches the site
/// and never does anything else either, and the production want that was
/// truncated to make room for the build stays unfilled forever. Nothing in
/// the log says why.
///
/// Walled in by *rock* rather than by structures: the pocket the Home lays
/// is finite, so a cell outside it is solid on every side and genuinely has
/// no station to stand at.
#[test]
fn a_request_no_program_can_reach_does_not_starve_the_base() {
    let mut game = base(1112);
    let body = builder(&mut game);
    place_now(&mut game, "mining_node", 0, 1).unwrap();
    let node = structure_at(&mut game, 0, 1).expect("a node stands there");
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50));
    game.tick();
    assert_eq!(
        game.world.get::<Task>(body).map(|t| t.target),
        Some(node),
        "precondition: the body is working the order"
    );

    // An island of laid floor well outside the starting pocket, solid rock
    // on every side of it. **Reachable in play, not a contrived state**:
    // laid tile is permanent while bare cut ground is not, so a player who
    // digs out to a spot, floors it, and lets the corridor behind them
    // revert to solid has exactly this.
    let far = tuning::STARTING_POCKET_RADIUS + 6;
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .lay_floor(px + far, py + far);
    game.place_structure("depot", far, far)
        .expect("filing is not gated on reachability");

    for _ in 0..20 {
        game.tick();
    }

    let task = game
        .world
        .get::<Task>(body)
        .expect("the body must still be posted to something");
    assert_eq!(
        (task.kind, task.target),
        (TaskKind::GatherResource, node),
        "an unreachable request must not hold the base's only body — it should be skipped, \
         leaving the body on the order it was already working"
    );
}

/// The second unreachable shape, and it needs its own guard: a site with
/// somewhere to *stand* but no way to *get* there.
///
/// A one-cell island is caught by `hauling::has_station` — no walkable
/// neighbour at all. A **two**-cell island is not: there is a perfectly good
/// tile beside the site, and it is just as cut off as the site is. Only the
/// route check catches this, which is why both guards exist; a test with one
/// island proves only half the fix.
///
/// This one **announces**, where the boxed-in case is dropped silently in
/// `build_wants`. The player floored these cells deliberately and asked for
/// a machine on one, so being unable to reach it is news either way.
#[test]
fn a_request_walled_off_behind_its_own_standing_room_is_skipped_and_announced() {
    let mut game = base(1113);
    let body = builder(&mut game);
    place_now(&mut game, "mining_node", 0, 1).unwrap();
    let node = structure_at(&mut game, 0, 1).expect("a node stands there");
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50));
    game.tick();

    let far = tuning::STARTING_POCKET_RADIUS + 6;
    let (px, py) = game.base_pos().expect("the fixture stands in the base");
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        grid.lay_floor(px + far, py + far);
        // The standing room: orthogonally beside the site, and just as cut
        // off. Without it `has_station` catches this and the route check
        // never runs.
        grid.lay_floor(px + far + 1, py + far);
    }
    game.place_structure("depot", far, far).unwrap();

    for _ in 0..20 {
        game.tick();
    }

    let task = game
        .world
        .get::<Task>(body)
        .expect("the body must still be posted to something");
    assert_eq!(
        (task.kind, task.target),
        (TaskKind::GatherResource, node),
        "a site with standing room but no route must not hold the base's only body"
    );
    assert!(
        game.message_history(200)
            .into_iter()
            .any(|m| m.text.contains("cut off")),
        "and the base says so, once — the player floored that cell on purpose"
    );
}

/// The dry report is said once per drought, not once per request.
///
/// **A build waits on a bill of several items over many trips**, which is
/// what makes this different from a dig site's latch. Said once and never
/// again, a base that ran out of Core Fragments early would go silent about
/// running out later — and "the crew stopped and told me nothing" is the
/// exact failure the announcement exists to prevent.
///
/// Mutation-proof by construction: the second half fails the moment the
/// clearing arm is deleted, and the first half fails if the latch is
/// dropped altogether, so neither direction passes on its own.
#[test]
fn the_dry_report_is_said_again_after_the_base_restocks_and_runs_out() {
    let mut game = base(1114);
    builder(&mut game);
    unlock_research_chain(&mut game, "armor_bench");
    // Empty the pack: the base has nothing to fetch at all.
    let held = count_item(&game, ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::CORE_FRAGMENT), held);
    game.place_structure("armory", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);

    // **Counted through `repeats`, not by counting entries.**
    // `Game::message_history` runs `resources::condense`, which folds a
    // repeated line into one row carrying a count — so an entry count reads
    // a line said twice as a line said once, and this test passed against
    // the very bug it exists to catch.
    let dry_lines = |g: &Game| {
        g.message_history(400)
            .into_iter()
            .filter(|m| m.text.contains("nothing to raise"))
            .map(|m| m.repeats)
            .sum::<usize>()
    };
    for _ in 0..80 {
        game.tick();
    }
    let first = dry_lines(&game);
    assert_eq!(first, 1, "said once on entering the state, not once a tick");

    // Restock less than the bill: the crew fetches what there is, delivers
    // it, and runs out again.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 1);
    for _ in 0..120 {
        if game.world.get::<BuildSite>(site).is_none() {
            break;
        }
        game.tick();
    }

    assert!(
        dry_lines(&game) > first,
        "and again once the crew has spent the restock and run out a second time — \
         a latch that never clears leaves the base silent for the rest of the run"
    );
}

/// The list a build-order screen will page, built before the screen exists.
///
/// Shipped now rather than with the screen because it is already the one
/// derivation three readers share — the map, the examine line, and this —
/// and a fourth opinion about how far along a build is, written later
/// against a component instead of against `BuildOrderRow`, is exactly how
/// two screens come to disagree.
///
/// Tile order, `assembler_system`'s reason: bevy's iteration order is not
/// stable, and a list that reshuffled between openings is one the player
/// cannot learn. The fixture files its two requests in the *opposite* order
/// to their tiles on purpose — filed in tile order the sort proves nothing.
#[test]
fn the_build_order_report_lists_every_request_in_a_stable_tile_order() {
    let mut game = base(1115);
    game.place_structure("depot", 1, 0).unwrap();
    game.place_structure("mining_node", -1, 0).unwrap();

    let report = game.build_order_report();
    assert_eq!(report.len(), 2, "both requests are listed");
    assert!(
        report[0].pos.0 < report[1].pos.0,
        "sorted by tile, not by the order they were filed: {:?}",
        report.iter().map(|r| r.pos).collect::<Vec<_>>()
    );
    assert_eq!(report.iter().map(|r| r.pos).collect::<Vec<_>>(), {
        let again = game.build_order_report();
        again.iter().map(|r| r.pos).collect::<Vec<_>>()
    });

    let node = report
        .iter()
        .find(|r| r.structure.contains("Mining Node"))
        .expect("the node is in the list under its display name, not its id");
    assert!(
        node.materials > 0,
        "it carries the bill it was filed against"
    );
    assert_eq!(
        node.delivered, 0,
        "nothing has been carried to it — there is no builder in this fixture"
    );
    assert!(node.builder.is_none(), "and nobody is on it");
    assert_eq!(
        node.percent(),
        0,
        "so it reads as not started, rather than dividing by something"
    );
    assert_eq!(
        node.required_ticks,
        node.materials * tuning::BUILD_TICKS_PER_MATERIAL,
        "the meter is derived from the bill, never stored beside it"
    );
}

/// **The deadlock**: a request the base cannot supply must not hold the body
/// that would produce what it needs.
///
/// One program, one Mining Node, an order for Core Fragments, and a request
/// filed for a machine that costs Core Fragments the base does not have.
/// Build wants outrank production, so the body is posted to the site; the
/// site is dry, so the body stands there; and the node that would make the
/// fragments is never worked again. The crew says "nothing to raise it with"
/// exactly once and the base is finished for the rest of the run.
///
/// This is not an exotic state — it is a new player filing a build they
/// cannot afford, which is the *supported* thing to do and the whole reason
/// filing charges nothing.
#[test]
fn a_request_the_base_cannot_supply_does_not_deadlock_production() {
    let mut game = base(1116);
    let body = builder(&mut game);
    place_now(&mut game, "mining_node", 0, 1).unwrap();
    let node = structure_at(&mut game, 0, 1).expect("a node stands there");
    // Nothing anywhere: the only way to get a Core Fragment is to mine one.
    let held = count_item(&game, ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::CORE_FRAGMENT), held);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50));
    game.place_structure("depot", 1, 0).unwrap();

    for _ in 0..200 {
        game.tick();
    }

    let worked_the_node = game
        .world
        .get::<Task>(body)
        .is_some_and(|t| t.kind == TaskKind::GatherResource && t.target == node);
    assert!(
        worked_the_node,
        "the body must go back to the node that makes what the site is waiting for — \
         held at the dry site, the base can never produce the materials to finish it"
    );
}

/// Examine names the request and what is still to be carried to it.
///
/// The materials standing on a site are deliberately not drawn on the map,
/// so this line is the only place they are visible — which is why it has to
/// name both halves rather than report a bare percentage.
#[test]
fn examining_a_site_says_what_is_going_up_and_what_it_is_short_of() {
    let mut game = base(1111);
    game.place_structure("mining_node", 1, 0).unwrap();
    let site = site_at(&mut game, 1, 0);

    let blurb = game
        .build_site_blurb(site)
        .expect("a site has something to say");
    assert!(
        blurb.contains("Mining Node"),
        "it names what is going up: {blurb}"
    );
    assert!(
        blurb.contains("Core Fragment"),
        "and what is still to be fetched: {blurb}"
    );
    assert!(
        blurb.contains("Nobody is free"),
        "and that there is no body on it, which is a state and not a fault: {blurb}"
    );
}
