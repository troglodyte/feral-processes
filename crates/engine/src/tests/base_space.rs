//! The base on the field: its guard, its floor, mining, dig marks and the
//! crew that works them, finishes and the plan's own orders.
//!
//! The base has no space of its own (ADR 0032): every cell here is a field
//! coordinate, laid floor is the `BaseGrid` overlay, and a mined blocker is a
//! `Biome::Excavated` override in the `WorldMap`. Field terrain near the
//! origin is seed-generated, so a test that needs rock at a cell sets it
//! (`set_rock`) rather than trusting the seed.

use super::support::*;
use crate::floors::FloorId;
use crate::world::{Biome, Tile, WorldMap};
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Overrides field `cell` to an ordinary blocker — the `entropy` rock kind,
/// the one `RockDb::kind_at` falls back to.
fn set_rock(game: &mut Game, cell: (i32, i32)) {
    set_blocker(game, cell, Biome::Entropy);
}

/// Overrides field `cell` to a blocker of `biome`, whose rock kind
/// `RockDb::kind_at` derives.
fn set_blocker(game: &mut Game, (x, y): (i32, i32), biome: Biome) {
    game.world.resource_mut::<WorldMap>().set_override(
        x,
        y,
        Tile {
            biome,
            walkable: false,
            rock_shade: None,
        },
    );
}

/// Overrides field `cell` to mined ground: what `Game::strike_field_rock`
/// leaves behind, written directly.
fn excavate(game: &mut Game, (x, y): (i32, i32)) {
    game.world.resource_mut::<WorldMap>().set_override(
        x,
        y,
        Tile {
            biome: Biome::Excavated,
            walkable: true,
            rock_shade: None,
        },
    );
}

/// Whether nothing can stand on field `cell`: not floor, and a blocker.
fn is_solid(game: &Game, (x, y): (i32, i32)) -> bool {
    !game.ground().walkable(x, y)
}

/// Where an action is attempted from.
#[derive(Clone, Copy, Debug)]
enum At {
    /// On the base's floor, at the Home's tile.
    Base,
    /// On the field, well off the base's floor.
    OffFloor,
    Stack,
}

/// A cell of open field far from `TEST_HOME_CELL` and its pocket.
const FAR_FIELD: (i32, i32) = (40, 40);

/// The fragment every `Game::require_field` refusal carries.
const THE_FIELD: &str = "up on the field";

/// Builds a fresh fixture, moves the party to `at`, and runs `action`.
///
/// The fixture always runs first and the party is moved afterwards, which is
/// what keeps the runs comparable: the base, the machines and everything
/// else are identical, and where the party stands is the only difference.
fn attempt<T>(
    seed: u32,
    at: At,
    fixture: &impl Fn(&mut Game) -> T,
    action: &impl Fn(&mut Game, T) -> Result<(), String>,
) -> Result<(), String> {
    let mut game = game(seed);
    let subject = fixture(&mut game);
    match at {
        At::Base => stand_in_base(&mut game),
        At::OffFloor => stand_in_base_at(&mut game, FAR_FIELD.0, FAR_FIELD.1),
        At::Stack => descend(&mut game),
    }
    action(&mut game, subject)
}

/// Asserts `what` is a field action: permitted standing on the base, and
/// refused in the Stack *by the field guard* rather than by some later check
/// the fixture happened to trip. Reading the refusal is the whole assertion
/// — an `is_err()` here would pass just as well against a refusal for
/// reach, which says nothing about the guard.
fn is_a_field_action<T>(
    seed: u32,
    what: &str,
    fixture: impl Fn(&mut Game) -> T,
    action: impl Fn(&mut Game, T) -> Result<(), String>,
) {
    if let Err(refused) = attempt(seed, At::Base, &fixture, &action) {
        panic!("{what} must be permitted on the base, got: {refused}");
    }
    let refused = attempt(seed, At::Stack, &fixture, &action)
        .err()
        .unwrap_or_else(|| panic!("{what} must be refused in the Stack"));
    assert!(
        refused.contains(THE_FIELD),
        "{what} must be refused in the Stack by the field guard, got: {refused}"
    );
}

// ---------------------------------------------------------------------
// The re-read guard sites
// ---------------------------------------------------------------------

/// A machine, not the Home: founding the base has its own tests further
/// down.
#[test]
fn deploying_a_structure_is_a_field_action() {
    is_a_field_action(
        3100,
        "deploying",
        |game| {
            place_home(game);
            give(game, &ItemId::from(ids::CORE_FRAGMENT), 20);
        },
        |game, ()| place_machine(game, "extractor", "core_fragment", 1, 0).map(drop),
    );
}

#[test]
fn upgrading_a_structure_is_a_field_action() {
    is_a_field_action(
        3101,
        "upgrading",
        |game| {
            let node = deploy_upgradeable_node(game);
            // The ceiling is the zone level, so a zone-1 base has nothing to
            // upgrade *to* and every locale would refuse for that instead.
            set_zone(game, 2);
            stock_upgrade_materials(game, 50);
            give(game, &ItemId::from(ids::CORE_FRAGMENT), 50);
            node
        },
        file_upgrade,
    );
}

#[test]
fn demolishing_a_structure_is_a_field_action() {
    is_a_field_action(
        3102,
        "demolishing",
        deploy_upgradeable_node,
        |game, node| game.remove_structure(node),
    );
}

#[test]
fn working_a_machine_by_hand_is_a_field_action() {
    is_a_field_action(
        3103,
        "working a machine by hand",
        |game| spawn_extractor(game, TEST_HOME_CELL.0 + 1, TEST_HOME_CELL.1),
        |game, node| game.work_structure(node),
    );
}

#[test]
fn filing_a_work_order_is_a_field_action() {
    is_a_field_action(
        3104,
        "filing a work order",
        |game| {
            // The shipped three-deep line for a Routine Disk, each machine
            // orthogonally adjacent to its feeder — anything less and the
            // order is refused by the chain rather than by the locale.
            place_home(game);
            spawn_machine_with_recipe_at(game, "extractor", "core_fragment", 2, 0);
            spawn_machine_with_recipe_at(game, "factory", "blank_substrate", 3, 0);
            spawn_machine_with_recipe_at(game, "assembler", "routine_disk", 4, 0);
        },
        |game, ()| game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3)),
    );
}

/// A transfer reports what it took rather than a `Result`, so its refusal is
/// an empty haul: the base half of this is the load-bearing assertion.
#[test]
fn collecting_from_a_machine_is_a_field_action() {
    let stock_a_neighbour = |game: &mut Game| {
        let mut stock = Stock::new(crate::tuning::DEFAULT_OUTPUT_CAPACITY);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 4);
        game.world.spawn((
            Structure {
                kind: "extractor".to_string(),
            },
            Position {
                x: TEST_HOME_CELL.0 + 1,
                y: TEST_HOME_CELL.1,
            },
            stock,
        ));
    };
    let collect = |game: &mut Game, ()| {
        if take_everything_adjacent(game).is_empty() {
            Err("nothing was collected".to_string())
        } else {
            Ok(())
        }
    };

    assert!(
        attempt(3105, At::Base, &stock_a_neighbour, &collect).is_ok(),
        "collecting must work standing beside the machine"
    );
    assert!(
        attempt(3105, At::Stack, &stock_a_neighbour, &collect).is_err(),
        "collecting must take nothing in the Stack"
    );
}

#[test]
fn selling_an_item_is_a_field_action() {
    is_a_field_action(
        3106,
        "selling to a trader",
        |game| {
            let market = spawn_market(game);
            give(game, &ItemId::from(ids::FIREWALL_PLATING), 3);
            market
        },
        |game, market| game.sell_item(market, gear(&ItemId::from(ids::FIREWALL_PLATING), 0), 2),
    );
}

#[test]
fn buying_an_item_back_is_a_field_action() {
    is_a_field_action(
        3107,
        "buying back from a trader",
        |game| {
            let market = spawn_market(game);
            let plating = ItemId::from(ids::FIREWALL_PLATING);
            give(game, &plating, 2);
            // Sold from the base first, since selling is a field action too
            // — the shelf this buys back off has to be stocked somehow.
            from_inside_the_base(game, |g| g.sell_item(market, gear(&plating, 0), 2)).unwrap();
            let paid = game.sell_price(market, &plating).unwrap();
            give(game, &ItemId::from(ids::CREDITS), paid * 8);
            market
        },
        |game, market| game.buy_back(market, gear(&ItemId::from(ids::FIREWALL_PLATING), 0), 2),
    );
}

#[test]
fn selling_a_program_is_a_field_action() {
    is_a_field_action(
        3108,
        "selling a program",
        |game| {
            let market = spawn_market(game);
            let pet = spawn_tamed(game, 30, 5);
            (market, pet)
        },
        |game, (market, pet)| game.sell_companion(market, pet),
    );
}

#[test]
fn buying_an_item_is_a_field_action() {
    is_a_field_action(
        3109,
        "buying from a trader",
        |game| {
            let market = spawn_market(game);
            let (item, unit_cost) = game
                .trade_options(market)
                .expect("the fixture trader deals in something")
                .buy[0]
                .clone();
            give(game, &ItemId::from(ids::CREDITS), unit_cost * 4);
            (market, item)
        },
        |game, (market, item)| game.buy_item(market, item, 2),
    );
}

/// A rest is allowed everywhere and where the party stands decides only
/// what it *costs*: free on the base's floor, one rest charge anywhere else.
///
/// That makes the base the cheap place to power down rather than the only
/// one, so this is the axis worth pinning here: three runs differing by
/// where the party stands alone, each asserted on the outlet count rather
/// than on the clock, since no rest advances the clock any more.
#[test]
fn resting_is_free_on_the_base_floor_and_priced_off_it() {
    let outlets_spent = |at: At| {
        let mut game = game(3110);
        stand_in_base_beside_home(&mut game);
        game.lay_test_pocket();
        match at {
            At::Base => {}
            At::OffFloor => stand_in_base_at(&mut game, FAR_FIELD.0, FAR_FIELD.1),
            At::Stack => descend(&mut game),
        }
        let player = game.player_entity();
        *game.world.get_mut::<PowerReserve>(player).unwrap() =
            PowerReserve::new(10.0, crate::components::POWER_MAX);
        let held = |g: &Game| {
            g.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::OUTLET))
        };
        let before = held(&game);
        game.rest().unwrap();
        assert_eq!(
            game.world.get::<PowerReserve>(player).unwrap().get(),
            100.0,
            "the rest has to run everywhere, or the count below is vacuous: {:?}",
            game.message_log(5)
        );
        before - held(&game)
    };

    assert_eq!(
        outlets_spent(At::Base),
        0,
        "on the base's floor the walk home is the whole price"
    );
    assert_eq!(
        outlets_spent(At::OffFloor),
        1,
        "out on the grid a rest is bought with a carried charge"
    );
    assert_eq!(
        outlets_spent(At::Stack),
        1,
        "and underground is field too, at the same price"
    );
}

// ---------------------------------------------------------------------
// The locale predicates
// ---------------------------------------------------------------------

/// `is_underground` stays strictly "down inside the Stack". Standing on the
/// base is the field, and answering yes here would apply every Stack rule
/// in the game — no Power supply, Trace, the frame view — to the base.
#[test]
fn standing_on_the_base_is_not_underground() {
    let mut game = game(3112);
    place_home(&mut game);
    stand_in_base(&mut game);

    assert!(!game.is_underground(), "the base is not the Stack");
    assert!(game.on_base_floor());
    assert_eq!(game.field_pos(), Some(TEST_HOME_CELL));
    assert!(
        game.stack_view().is_none(),
        "there is no frame loaded on the field"
    );
}

/// `on_base_floor` is what the old "in base space" rules read now, so it
/// must answer no off the floor and in the Stack rather than echo a tile
/// that happens to be floored.
#[test]
fn on_base_floor_answers_only_for_laid_floor_on_the_field() {
    let mut game = game(3113);
    place_home(&mut game);
    stand_in_base(&mut game);
    assert!(game.on_base_floor(), "the Home's tile is floor");

    stand_in_base_at(&mut game, FAR_FIELD.0, FAR_FIELD.1);
    assert!(!game.on_base_floor(), "open field is not the base");
    assert_eq!(game.field_pos(), Some(FAR_FIELD));

    stand_in_base(&mut game);
    descend(&mut game);
    assert!(
        !game.on_base_floor(),
        "the Stack is not the base, whatever tile the entrance pins"
    );
    assert_eq!(game.field_pos(), None, "the Stack is not the field");
}

/// A Recharger's radius is measured from the party's field tile, and
/// `recharger_node.ron`'s own description reads "while you stand anywhere
/// on your base" — so a Recharger beside the party standing on the base
/// regens it. Walked through `place_structure` rather than a spawn at an
/// offset, so the Recharger stands where a real deploy puts it.
#[test]
fn a_recharger_regens_the_party_standing_on_the_base() {
    let mut game = game(3114);
    place_home(&mut game);
    stand_in_base(&mut game);
    place_now(&mut game, "recharger_node", 1, 0)
        .expect("floor beside the Home is inside the pocket");

    let player = game.player_entity();
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(5.0);
    let before = game.world.get::<PowerReserve>(player).unwrap().get();

    game.tick();

    let after = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        after > before,
        "a Recharger deployed on the base must regen a party standing there: \
         {before} -> {after}"
    );
}
// ---------------------------------------------------------------------
// The anchor: where the Home was founded
// ---------------------------------------------------------------------

/// `Game::new` spawns exactly one `BaseAnchor`, standing on the same tile
/// the player does — both are placed from the same `start` coordinate in
/// `Game::new`.
#[test]
fn a_new_game_has_exactly_one_anchor_where_the_player_starts() {
    let mut game = game(3116);
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let anchors: Vec<Position> = {
        let mut query = game.world.query_filtered::<&Position, With<BaseAnchor>>();
        query.iter(&game.world).copied().collect()
    };

    assert_eq!(
        anchors.len(),
        1,
        "a fresh game must have exactly one anchor"
    );
    assert_eq!(
        anchors[0], player_pos,
        "the anchor must stand where the player starts"
    );
    assert_eq!(
        game.anchor_position(),
        Some((player_pos.x, player_pos.y)),
        "Game::anchor_position must agree with the entity's own Position"
    );
}

/// The anchor's position round-trips through a real `Game::save`/
/// `Game::load`, not only the RON round trip — a round trip through
/// `to_ron`/`from_ron` alone cannot catch `#[serde(skip)]` on
/// `SaveData::anchor`, nor a load path that quietly falls back to the zone
/// spawn point instead of trusting the persisted value.
///
/// Moved off the zone spawn point first so the two cannot coincidentally
/// agree: a load path that derived the anchor from `spawn_point` instead of
/// `data.anchor` would pass a test that left the two equal, which is
/// exactly the trap this repo has been bitten by before — the zone spawn
/// point is usually `(0, 0)`, so a derivation bug is invisible against it.
#[test]
fn the_anchors_position_survives_a_real_save_and_load() {
    let mut game = game(3117);
    let anchor = {
        let mut query = game.world.query_filtered::<Entity, With<BaseAnchor>>();
        query.iter(&game.world).next().unwrap()
    };
    {
        let mut pos = game.world.get_mut::<Position>(anchor).unwrap();
        pos.x += 40;
        pos.y += 40;
    }
    let moved = *game.world.get::<Position>(anchor).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    assert_ne!(
        (moved.x, moved.y),
        (spawn.x, spawn.y),
        "the fixture must move the anchor away from the spawn point to be a real test"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_anchor_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.anchor_position(), Some((moved.x, moved.y)));
}

/// `run_raid` selects its target from `With<Durability>`, and the anchor
/// carries none — so a forced sweep against a base with nothing else
/// standing must find no target at all, rather than hitting the one entity
/// present. If the anchor ever gained a `Durability` component (the mistake
/// `components::BaseAnchor`'s doc warns against), this is what would go
/// non-empty.
#[test]
fn a_forced_raid_never_targets_the_anchor() {
    let mut game = game(3118);

    game.dev_force_raid();

    assert!(
        game.take_effects().is_empty(),
        "the anchor has no Durability, so a raid against an otherwise-empty base must find no target"
    );
}

/// `structure_report` is the roster every deployed-structure count in the
/// game reads from — `Game::structure_manifest` filters it by entity rather
/// than building a second one — and it requires a `Structure` component,
/// which the anchor deliberately does not carry. A fresh game reports
/// nothing deployed, and placing a real structure beside the anchor reports
/// exactly the one structure, not two.
#[test]
fn the_anchor_is_not_counted_as_a_deployed_structure() {
    let mut game = game(3119);
    assert!(
        game.structure_report().is_empty(),
        "the anchor exists from the start but must not appear on the structure roster"
    );

    place_home(&mut game);

    assert_eq!(
        game.structure_report().len(),
        1,
        "only the placed Home should be on the roster, not the anchor standing beside it"
    );
}

// ---------------------------------------------------------------------
// Moving on the field next to rock
// ---------------------------------------------------------------------

/// A run with a base: the run's first Home founded on `TEST_HOME_CELL` and
/// the fixture pocket laid around it.
fn game_with_a_base(seed: u32) -> Game {
    let mut game = game(seed);
    place_home(&mut game);
    game
}

/// A field cell one step north of the pocket's north edge, made solid rock:
/// the same cell whichever test asks, and minable, since the edge is floor.
fn solid_rock_north(game: &mut Game) -> (i32, i32) {
    let cell = (0, -TEST_POCKET_HALF_HEIGHT - 1);
    set_rock(game, cell);
    cell
}

/// Solid rock does not give, and the party stays out of it.
///
/// A step into a minable blocker is a *swing* (`Game::bump_rock`), and a
/// swing costs the turn a step would have, the same way shoving at a nest
/// does. What it never does is land the party inside the wall.
#[test]
fn swinging_at_solid_rock_does_not_move_the_party() {
    let mut game = game_with_a_base(3126);
    // Tools out: a disarmed bump is refused before it can swing, which is a
    // different code path from the one this test is about.
    game.toggle_mining();
    // The pocket's north edge, so the step north leaves it.
    let edge = (0, -TEST_POCKET_HALF_HEIGHT);
    let rock = solid_rock_north(&mut game);
    stand_in_base_at(&mut game, edge.0, edge.1);

    assert!(
        is_solid(&game, rock),
        "the fixture must be shoving at real rock"
    );
    let tick = game.current_tick();
    game.move_player(0, -1);

    assert_eq!(game.field_pos(), Some(edge), "solid rock does not give");
    assert_eq!(
        game.current_tick(),
        tick + 1,
        "a swing at rock costs the turn the step would have"
    );
}

/// Movement reads the ground, not the floor: mined-out rock is somewhere
/// you can stand long before it is somewhere you can build.
#[test]
fn mined_rock_is_walkable_before_it_is_floored() {
    let mut game = game_with_a_base(3128);
    let edge = (0, TEST_POCKET_HALF_HEIGHT);
    let mined = (0, TEST_POCKET_HALF_HEIGHT + 1);
    excavate(&mut game, mined);
    stand_in_base_at(&mut game, edge.0, edge.1);

    game.move_player(0, 1);

    assert_eq!(game.field_pos(), Some(mined));
    assert!(
        !game
            .world
            .resource::<base_grid::BaseGrid>()
            .is_floor(mined.0, mined.1),
        "the fixture cell must be mined and not floor, or this tests nothing"
    );
}

/// A swing at rock still breaks off a posted job, exactly as shoving at a
/// wall does elsewhere — `move_player` drops the job before it looks at
/// what is in the way, "since either way you stopped working to do it", and
/// `Game::work_structure` promises the player as much when it posts.
///
/// The drop happens *before* the wall is even looked at, which is why it
/// survived the shove turning into a swing: had it lived in the refusal
/// branch instead, digging would have quietly stopped ending jobs.
#[test]
fn swinging_at_solid_rock_still_breaks_off_a_job() {
    let mut game = game_with_a_base(3129);
    // The pocket's north edge, with a machine beside it: a job to break off,
    // and solid rock one step further north to break it off against.
    let edge = (0, -TEST_POCKET_HALF_HEIGHT);
    let node = spawn_extractor(&mut game, edge.0 + 1, edge.1);
    solid_rock_north(&mut game);
    // Tools out: a disarmed bump is refused before it can swing, which is a
    // different code path from the one this test is about.
    game.toggle_mining();
    stand_in_base_at(&mut game, edge.0, edge.1);
    game.work_structure(node).expect("standing beside the node");
    let player = game.player_entity();
    assert!(
        game.world.get::<Task>(player).is_some(),
        "the fixture must actually post a job"
    );
    let tick = game.current_tick();

    game.move_player(0, -1);

    assert_eq!(
        game.field_pos(),
        Some(edge),
        "the fixture's step must really meet rock, or this proves nothing"
    );
    assert_eq!(
        game.current_tick(),
        tick + 1,
        "and the swing itself must still cost its turn"
    );
    assert!(
        game.world.get::<Task>(player).is_none(),
        "a shove at rock is still you stopping work to try it"
    );
}

// ---------------------------------------------------------------------
// Founding, and deploying onto floor
// ---------------------------------------------------------------------

/// Stands the party on `TEST_HOME_CELL` with the 3x3 around it forced to
/// open ground, so founding there lays the whole ring whatever the seed put
/// under it.
fn ready_to_found(game: &mut Game) {
    let mut map = game.world.resource_mut::<WorldMap>();
    for dy in -1..=1 {
        for dx in -1..=1 {
            map.set_override(
                TEST_HOME_CELL.0 + dx,
                TEST_HOME_CELL.1 + dy,
                Tile {
                    biome: Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
    }
    stand_in_base_at(game, TEST_HOME_CELL.0, TEST_HOME_CELL.1);
}

/// The first Home goes on the tile the party stands on, and the anchor with
/// it: the base opens on ground the player chose. A direction is ignored.
#[test]
fn the_first_home_stands_on_the_partys_tile_with_the_anchor() {
    let mut game = game(3130);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    ready_to_found(&mut game);

    game.place_structure("home", 1, 0, None)
        .expect("the first Home is what opens the base, so it cannot need the base to exist");

    let home = game.home_position().expect("a Home was just deployed");
    assert_eq!((home.x, home.y), TEST_HOME_CELL);
    assert_eq!(game.anchor_position(), Some(TEST_HOME_CELL));
}

/// Founding floors exactly the ring around the Home, into `BaseGrid` and
/// nowhere else. Floor is an overlay: a ring laid *and* a `Biome::Platform`
/// stamped into the `WorldMap` would pass a floor-count assertion on its
/// own, so the overrides are asserted too.
#[test]
fn founding_lays_the_ring_and_stamps_no_world_tile() {
    let mut game = game(3132);
    assert_eq!(
        game.world.resource::<base_grid::BaseGrid>().floor_count(),
        0,
        "a fresh run has no floor"
    );
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    ready_to_found(&mut game);
    let overrides_before = game.world.resource::<WorldMap>().overrides().len();

    game.place_structure("home", 0, 0, None).unwrap();

    let grid = game.world.resource::<base_grid::BaseGrid>();
    assert_eq!(
        grid.floor_count(),
        9,
        "the Home's tile and the ring around it"
    );
    assert!(grid.is_floor(TEST_HOME_CELL.0, TEST_HOME_CELL.1));
    assert!(
        !grid.is_floor(TEST_HOME_CELL.0 + 2, TEST_HOME_CELL.1),
        "and it stops at the ring"
    );
    assert_eq!(
        game.world.resource::<WorldMap>().overrides().len(),
        overrides_before,
        "founding must write no tile into the field at all"
    );
    assert!(
        !game
            .world
            .resource::<WorldMap>()
            .overrides()
            .values()
            .any(|t| t.biome == Biome::Platform),
        "and no Biome::Platform anywhere in the overlay"
    );
}

/// A machine goes up on floor, deployed relative to the cell the player is
/// standing in.
#[test]
fn a_machine_deploys_onto_pocket_floor() {
    let mut game = game(3133);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 40);
    place_home(&mut game);
    stand_in_base_at(&mut game, 1, 0);

    place_machine(&mut game, "extractor", "core_fragment", 1, 0)
        .expect("floor two cells east of the Home is inside the pocket");

    let node = find_structure_by_kind(&mut game, "extractor").expect("it went up");
    let at = *game.world.get::<Position>(node).unwrap();
    assert_eq!(
        (at.x, at.y),
        (2, 0),
        "a deploy is measured from the party's tile"
    );
}

/// And the footprint rule is `BaseGrid::is_floor` and nothing else: ground
/// one step past the pocket's edge refuses, in the floor rule's own words.
#[test]
fn deploying_off_the_pocket_floor_is_refused() {
    let mut game = game(3134);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 40);
    place_home(&mut game);
    let edge = TEST_POCKET_HALF_WIDTH;
    stand_in_base_at(&mut game, edge, 0);
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_floor(edge, 0),
        "the fixture must stand on the pocket's last floored cell"
    );

    let refused = game
        .place_structure("extractor", 1, 0, None)
        .expect_err("one step past the edge is not floor");

    assert!(
        !refused.contains(THE_FIELD),
        "this must be refused by the footprint rule, not by the locale guard: {refused}"
    );
    assert!(
        !game
            .world
            .resource::<base_grid::BaseGrid>()
            .is_floor(edge + 1, 0),
        "and the cell it refused really is off the floor, or the test proves nothing"
    );
}

// ---------------------------------------------------------------------
// `broker_reach` measures the base, not the Broker
// ---------------------------------------------------------------------

/// A run with a Home and a Broker standing, and the party on the Home.
fn game_with_a_broker(seed: u32) -> Game {
    let mut game = game(seed);
    place_home(&mut game);
    spawn_machine_at(&mut game, "contract_broker", 1, 0);
    stand_in_base(&mut game);
    game
}

#[test]
fn a_broker_is_in_reach_from_pocket_floor() {
    let game = game_with_a_broker(3135);
    assert_eq!(game.broker_reach(), BrokerReach::AtBroker);
}

#[test]
fn a_broker_is_out_of_reach_off_the_floor() {
    let mut game = game_with_a_broker(3136);
    let outside = TEST_POCKET_HALF_WIDTH + 3;
    stand_in_base_at(&mut game, outside, 0);
    assert!(
        !game
            .world
            .resource::<base_grid::BaseGrid>()
            .is_floor(outside, 0),
        "the fixture cell must be off the floor, or this tests nothing"
    );
    assert_eq!(
        game.broker_reach(),
        BrokerReach::OffBase,
        "on the field but off the floor is off the base"
    );

    stand_in_base(&mut game);
    descend(&mut game);
    assert_eq!(
        game.broker_reach(),
        BrokerReach::OffBase,
        "and so is the Stack"
    );
}

#[test]
fn no_broker_standing_is_not_the_same_as_being_off_the_base() {
    let mut game = game(3137);
    place_home(&mut game);
    stand_in_base(&mut game);
    assert_eq!(
        game.broker_reach(),
        BrokerReach::NoBroker,
        "a base with no trader in it has no desk to be near"
    );
}

// ---------------------------------------------------------------------
// The base's reach, saved
// ---------------------------------------------------------------------

/// The floor `BaseGrid` laid survives a real save and load.
#[test]
fn a_reload_keeps_the_floor() {
    let mut game = game(3142);
    place_home(&mut game);
    let laid = game.world.resource::<base_grid::BaseGrid>().floor_count();
    assert!(
        laid >= test_pocket_cells().len(),
        "the fixture must have laid the pocket"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_no_slab_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.world.resource::<base_grid::BaseGrid>().floor_count(),
        laid,
        "the floor the base laid must come back, not an empty base"
    );
}

/// `distance_from_danger_origin` widened the opening ring by the base's reach
/// once a Home was deployed, and it has to keep doing that by the same
/// number — before *and after* a reload.
///
/// It read `Platform::center`/`radius`, which nothing sets on a fresh run any
/// more, so the ring silently **shrank** by four tiles when a Home went down
/// and grew back on the next load. The two halves are asserted together
/// because either alone passes against a reader stuck on one answer.
#[test]
fn deploying_a_home_widens_the_opening_ring_and_a_reload_keeps_it() {
    let mut game = game(3143);
    let spawn = game.zone_spawn_point();
    // Just outside the bare ring, so only the base's reach can bring it in.
    let probe = (spawn.0 + crate::tuning::OPENING_RING_TILES + 2, spawn.1);
    assert!(
        !game.in_opening_ring(probe.0, probe.1),
        "the probe must start outside the ring, or this tests nothing"
    );

    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    place_now(&mut game, "home", 1, 0).unwrap();
    assert!(
        game.in_opening_ring(probe.0, probe.1),
        "a base's own reach is what widens the opening ring"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_opening_ring_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.in_opening_ring(probe.0, probe.1),
        "and the same run reloaded must answer the same — a ring that moves \
         across a save is a run that plays two different ways"
    );
}

/// The same fault on the other reader: `frames_at` subtracts the base's reach
/// before charging a link its distance, so a run with a base standing gets
/// shallower stacks near it than a run without one.
///
/// Asserted as a strict inequality at a distance where the subtraction has to
/// change the answer — `STACK_TILES_PER_FRAME * 2` out, where four tiles of
/// reach is a whole frame — rather than as "the same before and after a
/// reload". That weaker shape passes against a reader that has stopped
/// subtracting anything at all, because it then answers the same wrong
/// number on both sides of the save.
#[test]
fn a_link_near_the_base_runs_shallower_than_one_by_a_baseless_run() {
    let mut game = game(3144);
    let spawn = game.zone_spawn_point();
    let link = (spawn.0 + crate::tuning::STACK_TILES_PER_FRAME * 2, spawn.1);

    let unbased = game.frames_at(link);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    place_now(&mut game, "home", 1, 0).unwrap();
    let based = game.frames_at(link);

    assert!(
        based < unbased,
        "the base's own reach has to come off a link's distance: \
         no base={unbased}, base={based}"
    );

    // And the same run reloaded answers the same, which is what the fault
    // actually broke: `Platform::radius` was rebuilt on load and not before
    // it, so one run played two ways.
    let path = std::env::temp_dir().join(format!(
        "feral_processes_frames_at_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.frames_at(link),
        based,
        "how deep a link runs must not depend on whether the run has been reloaded"
    );
}

// ---------------------------------------------------------------------
// `Game::view_tiles` merges the floor over the field
// ---------------------------------------------------------------------

/// `view_tiles` reads the field with the base merged over it: laid floor
/// reads as `Biome::Platform`, a mined, unfloored cell as its
/// `Biome::Excavated` override, and an untouched blocker as its own biome.
#[test]
fn view_tiles_merges_the_floor_over_the_field() {
    let mut game = game(3200);
    place_home(&mut game);
    let mined = (TEST_POCKET_HALF_WIDTH + 1, 0);
    excavate(&mut game, mined);
    let rock = (TEST_POCKET_HALF_WIDTH + 2, 0);
    set_rock(&mut game, rock);
    stand_in_base(&mut game);

    let half = TEST_POCKET_HALF_WIDTH + 3;
    let tiles = game.view_tiles(half, half);
    let at = |(x, y): (i32, i32)| tiles[(half + y) as usize][(half + x) as usize];

    assert_eq!(
        at(TEST_HOME_CELL).biome,
        Biome::Platform,
        "the Home's tile is laid floor"
    );
    assert!(at(TEST_HOME_CELL).walkable);

    assert_eq!(
        at(mined).biome,
        Biome::Excavated,
        "a mined, unfloored cell reads as Excavated"
    );
    assert!(at(mined).walkable, "mined rock is walkable even unfloored");

    assert_eq!(at(rock).biome, Biome::Entropy, "a blocker reads as itself");
    assert!(!at(rock).walkable, "nothing has dug there yet");
}

/// With no floor laid, `view_tiles` is a straight read of `WorldMap`
/// centred on the player's own `Position`: the merge only ever changes a
/// cell the base has floored.
#[test]
fn view_tiles_with_no_floor_is_the_world_map() {
    let mut game = game(3201);
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let half = 5;
    let expected: Vec<Vec<Tile>> = (-half..=half)
        .map(|ty| {
            (-half..=half)
                .map(|tx| {
                    game.world
                        .resource_mut::<WorldMap>()
                        .tile(ppos.x + tx, ppos.y + ty)
                })
                .collect()
        })
        .collect();

    let got = game.view_tiles(half, half);

    for (gy, (grow, erow)) in got.iter().zip(expected.iter()).enumerate() {
        for (gx, (g, e)) in grow.iter().zip(erow.iter()).enumerate() {
            assert_eq!(
                (g.biome, g.walkable),
                (e.biome, e.walkable),
                "view_tiles diverged from a direct WorldMap read at grid ({gx}, {gy})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rock you can hit
// ---------------------------------------------------------------------------

/// The party at the eastern edge of the fixture pocket, one step from solid
/// rock: a band of ordinary rock `FRONTIER` cells deep and three tall is
/// forced east of the edge, so `WALL`, the cells behind it and the cells
/// beside it are rock whatever the seed generated there.
///
/// The cheapest fixture that has a *frontier* at all — a wall with floor
/// behind it, which is the only place the base may cut (`minable`).
fn game_at_the_frontier(seed: u32) -> Game {
    let mut game = game(seed);
    place_home(&mut game);
    for x in WALL.0..WALL.0 + FRONTIER {
        for y in WALL.1 - 1..=WALL.1 + 1 {
            set_rock(&mut game, (x, y));
        }
    }
    stand_in_base_at(&mut game, TEST_POCKET_HALF_WIDTH, 0);
    game
}

/// How deep `game_at_the_frontier`'s band of rock runs east of the edge.
const FRONTIER: i32 = 4;

/// How many swings `swinger` needs to cut the cell at `at` — **read off
/// that cell's own kind**, not off `BASE_ROCK_DURABILITY`.
///
/// Since `assets/rock/` gives cells different durabilities and different
/// swing floors, a fixture budgeting from the fallback constant under-swings
/// whenever its coordinate happens to fall in a denser seam. The failure
/// reads as the crew never cutting through rather than as the fixture being
/// short something, which is why this is one helper and not a line repeated
/// at each site.
fn swings_for(game: &Game, swinger: Entity, at: (i32, i32)) -> u32 {
    let wall = game.wall_at(at.0, at.1);
    wall.durability
        .div_ceil(game.swing_damage(swinger).min(wall.swing_cap))
}

/// `game_at_the_frontier` with the player's cutting tools out.
///
/// `resources::MiningMode` starts **off**: a bump into solid rock is refused
/// for free, so a developed player cannot demolish their own base by
/// clipping a corner of it. Any fixture that digs by *walking* therefore has
/// to arm it first. Omitted, the wall simply never comes down, and the
/// failure reads as the dig being broken rather than as the fixture being
/// short something — which is why this is a named fixture and not a line
/// repeated at nine call sites.
fn game_at_the_frontier_cutting(seed: u32) -> Game {
    let mut game = game_at_the_frontier(seed);
    game.toggle_mining();
    game
}

/// The solid cell east of `game_at_the_frontier`'s standing tile.
const WALL: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 1, 0);

/// What a field cell is, as far as the base is concerned.
#[derive(Debug, PartialEq)]
enum Cell {
    /// A blocker nobody has cut.
    Solid,
    /// A blocker the base mined: its `Biome::Excavated` override.
    Excavated,
    /// Walkable ground the base never touched.
    Open,
    /// Laid floor.
    Floor,
}

fn cell(game: &Game, (x, y): (i32, i32)) -> Cell {
    if game.world.resource::<base_grid::BaseGrid>().is_floor(x, y) {
        return Cell::Floor;
    }
    let tile = game.world.resource::<WorldMap>().peek(x, y);
    match (tile.walkable, tile.biome) {
        (false, _) => Cell::Solid,
        (true, Biome::Excavated) => Cell::Excavated,
        (true, _) => Cell::Open,
    }
}

/// The swing count is computed from the constants rather than written down:
/// retuning `BASE_ROCK_DURABILITY` retunes this test with it, which is the
/// point — what is pinned is that a wall takes *the swings its durability
/// implies*, not that it takes three.
#[test]
fn a_wall_opens_after_the_swings_its_durability_implies() {
    let mut game = game_at_the_frontier_cutting(3210);
    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);
    assert!(
        swings > 1,
        "a wall that opens on the first swing makes this test vacuous — \
         the wall's durability is below one swing of a level-1 player"
    );

    for swing in 1..swings {
        game.move_player(1, 0);
        assert!(
            is_solid(&game, (WALL.0, WALL.1)),
            "the wall gave way on swing {swing} of {swings}"
        );
    }

    game.move_player(1, 0);
    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "the wall was still standing after the {swings} swings its \
         durability implies"
    );
}

#[test]
fn a_swing_at_rock_does_not_move_the_party() {
    let mut game = game_at_the_frontier(3211);
    let standing = game.field_pos().unwrap();

    game.move_player(1, 0);

    assert!(
        is_solid(&game, (WALL.0, WALL.1)),
        "this test is about a swing that does not break through"
    );
    assert_eq!(
        game.field_pos().unwrap(),
        standing,
        "a swing at rock moved the party into it"
    );
}

/// `attack_nest`'s determinism rule, one space over: identical swings have
/// to land identical damage, or wearing a wall down becomes a slot machine.
/// This is why mining does not go through `battle::resolve_attack`.
#[test]
fn identical_swings_at_rock_do_identical_damage() {
    let mut game = game_at_the_frontier(3212);
    let east = WALL;
    // Rock kind varies cell to cell, so "identical" means a wall of the same
    // kind: the first one along the pocket's north face that matches.
    let kind = |game: &Game, (x, y): (i32, i32)| {
        let wall = game.wall_at(x, y);
        (wall.durability, wall.swing_cap)
    };
    let north = (0, TEST_POCKET_HALF_HEIGHT + 1);
    set_rock(&mut game, north);
    assert_eq!(
        kind(&game, north),
        kind(&game, east),
        "the two walls must be the same kind for their damage to compare"
    );

    let player = game.player_entity();
    game.strike_field_rock(player, east.0, east.1);
    game.strike_field_rock(player, north.0, north.1);

    let left = |game: &mut Game, (x, y): (i32, i32)| {
        let site = game
            .dig_site_at(x, y)
            .expect("a struck wall has a dig site");
        game.world.get::<Durability>(site).unwrap().hp
    };
    assert_eq!(
        left(&mut game, east),
        left(&mut game, north),
        "two fresh walls took different damage from the same player"
    );
}

#[test]
fn a_swing_costs_a_turn() {
    let mut game = game_at_the_frontier_cutting(3214);
    let before = game.world.resource::<GameClock>().tick;

    game.move_player(1, 0);

    assert_eq!(
        game.world.resource::<GameClock>().tick - before,
        1,
        "a swing at rock should cost exactly one turn, like a swing at a nest"
    );
}

/// Settled decision 5, held as an assertion against the real assets: a cut
/// cell may pay a trickle, but never enough to undercut the Mining Node.
///
/// **Stated as a rate, because the per-cell form cannot fail.** A cell pays
/// at most one fragment — `ROCK_FRAGMENT_CHANCE` is a probability, and
/// `roll_rock_fragment` clamps it — against the four a Blank Substrate costs, so
/// "a cut pays less than a floor" passes for every legal value of the knob
/// and holds nothing. What can actually breach the decision is the payout
/// *per tick*, which three constants decide between them: raise the chance,
/// soften the rock, or quicken the swing far enough and the wall becomes a
/// better fragment source than the machine built to be one.
#[test]
fn mining_a_wall_never_undercuts_a_mining_node() {
    let game = game(3215);
    // A fresh player's swing, so the comparison is made where the player
    // actually stands rather than at a level nobody has reached yet — the
    // rock is the same rock all run, and it is the digger that improves.
    let player = game.player_entity();
    // Over **every** kind, not just the fallback: the payout is per cell
    // while the cost is per tick, so the *fastest* kind to cut is the one
    // that can undercut the machine. A kind authored soft enough — or a
    // fallback softened — reopens this hole for itself alone, which one
    // reading of `BASE_ROCK_DURABILITY` could never see.
    let per_swing = game.swing_damage(player);
    let fewest_ticks = game
        .world
        .resource::<crate::rock::RockDb>()
        .defs()
        .map(|def| {
            let swings = def
                .durability
                .div_ceil(per_swing.min(def.swing_cap()))
                .max(def.min_swings);
            swings * crate::tuning::BASE_DIG_TICKS_PER_SWING
        })
        .min()
        .unwrap_or(crate::tuning::BASE_ROCK_DURABILITY.div_ceil(per_swing));
    let per_tick_dug = crate::tuning::ROCK_FRAGMENT_CHANCE / fewest_ticks as f32;

    let structures = game.world.resource::<StructureDb>();
    let work = structures
        .get("extractor")
        .expect("the shipped assets deploy an Extractor")
        .machine
        .as_ref()
        .and_then(|m| {
            m.recipes
                .iter()
                .find(|r| r.produces.as_str() == ids::CORE_FRAGMENT)
        })
        .expect("an Extractor can pull Core Fragments");
    assert_eq!(
        work.produces.as_str(),
        ids::CORE_FRAGMENT,
        "this bound compares fragment sources — a Mining Node that stopped \
         producing fragments makes it meaningless"
    );
    // The node's ceiling, not its average: `level` makes a cycle fizzle
    // sometimes, and a bound that assumed the fizzle would quietly widen
    // every time node reliability was retuned.
    let per_tick_node =
        crate::systems::node_payout(1, ZoneLevel(1)) as f32 / work.ticks_per_unit as f32;

    assert!(
        per_tick_dug < per_tick_node,
        "a cut cell pays {per_tick_dug} Core Fragments a tick against a \
         Mining Node's best case of {per_tick_node} — the wall has become a \
         fragment tap"
    );
}

// ---------------------------------------------------------------------------
// laying a VectorStasis Tile
// ---------------------------------------------------------------------------

/// The party standing on a carved, unfloored cell holding exactly enough
/// Core Fragments for `tiles` tiles — the one situation `Game::lay_tile` is
/// for.
fn game_on_open_rock(seed: u32, tiles: u32) -> Game {
    let mut game = game_at_the_frontier(seed);
    excavate(&mut game, (WALL.0, WALL.1));
    stand_in_base_at(&mut game, WALL.0, WALL.1);
    set_fragments(&mut game, tiles * crate::tuning::FLOOR_TILE_COST);
    game
}

/// Sets the player's Core Fragments to exactly `n`. `game_at_the_frontier`
/// hands over 20 to pay for the Home and leaves the change in the pack, so
/// a tile test that only `give`s would be counting from an unknown start.
fn set_fragments(game: &mut Game, n: u32) {
    let fragment = ItemId::from(ids::CORE_FRAGMENT);
    let player = game.player_entity();
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&fragment);
    inv.take(fragment.clone(), held);
    inv.add(fragment, n);
}

fn fragments_held(game: &Game) -> u32 {
    count_item(game, ids::CORE_FRAGMENT)
}

/// Words the two refusals below may share, because everything is built out
/// of them: the test is about the *errand* each one leaves the player, and
/// two refusals that differ only in an article leave the same one.
fn meaningful_words(msg: &str) -> Vec<String> {
    msg.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .map(|w| w.to_lowercase())
        .collect()
}

#[test]
fn laying_a_tile_spends_its_core_fragments_and_no_substrate() {
    let mut game = game_on_open_rock(3220, 3);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 2);

    game.lay_tile()
        .expect("standing on carved rock, holding stock");

    assert_eq!(
        fragments_held(&game),
        2 * crate::tuning::FLOOR_TILE_COST,
        "a tile costs FLOOR_TILE_COST Core Fragments and leaves the rest"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        2,
        "a plain tile is pressed from fragments, not from the Lathe's substrate"
    );
}

/// One fragment short of a tile is no tile: the refusal spends nothing.
#[test]
fn laying_a_tile_one_fragment_short_refuses_and_spends_nothing() {
    let mut game = game_on_open_rock(3227, 0);
    set_fragments(&mut game, crate::tuning::FLOOR_TILE_COST - 1);

    game.lay_tile()
        .expect_err("short of a tile means nothing happens");

    assert_eq!(fragments_held(&game), crate::tuning::FLOOR_TILE_COST - 1);
    assert!(
        !game
            .world
            .resource::<base_grid::BaseGrid>()
            .is_floor(WALL.0, WALL.1),
        "a refused tiling laid floor anyway"
    );
}

#[test]
fn a_tile_turns_the_cell_you_stand_on_into_floor() {
    let mut game = game_on_open_rock(3221, 1);
    assert!(
        !game
            .world
            .resource::<base_grid::BaseGrid>()
            .is_floor(WALL.0, WALL.1),
        "the fixture cell must start carved and unfloored"
    );

    game.lay_tile().unwrap();

    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_floor(WALL.0, WALL.1),
        "the cell the party stands on is what a tile floors"
    );
}

#[test]
fn laying_a_tile_without_fragments_refuses_and_spends_nothing() {
    let mut game = game_on_open_rock(3222, 0);

    game.lay_tile()
        .expect_err("nothing to lay means nothing happens");

    assert!(
        !game
            .world
            .resource::<base_grid::BaseGrid>()
            .is_floor(WALL.0, WALL.1),
        "a refused tiling laid floor anyway"
    );
}

/// Two different errands for the player — go mine fragments, against you
/// are standing in the wrong place — so they may not read as the same
/// refusal. `NoPost::BoxedIn` against `NoPost::NoRoute`, one level down.
#[test]
fn laying_a_tile_on_floor_refuses_in_different_words_from_having_no_fragments() {
    let mut broke = game_on_open_rock(3223, 0);
    let no_stock = broke.lay_tile().unwrap_err();

    let mut floored = game_on_open_rock(3224, 1);
    floored.lay_tile().expect("the first tile lands");
    let already = floored
        .lay_tile()
        .expect_err("the cell is floor now, and a second tile has nothing to do");

    let shared: Vec<String> = meaningful_words(&no_stock)
        .into_iter()
        .filter(|w| meaningful_words(&already).contains(w))
        .collect();
    assert!(
        shared.is_empty(),
        "the two refusals share wording ({shared:?}) — \
         \"{no_stock}\" against \"{already}\""
    );
    assert_eq!(
        fragments_held(&floored),
        0,
        "the refused second tile must not have spent anything"
    );
}

/// `Game::require_field`: laying a tile claims something about the cell the
/// party is standing on, and underground the player's `Position` is pinned
/// to the entrance and names no cell they can reach.
#[test]
fn laying_a_tile_in_the_stack_refuses() {
    let mut game = game_on_open_rock(3225, 1);
    descend(&mut game);

    let refused = game
        .lay_tile()
        .expect_err("there is no field cell underfoot down here");

    assert!(
        refused.contains(THE_FIELD),
        "a tiling attempt in the Stack must meet the field guard, got: {refused}"
    );
    assert_eq!(
        fragments_held(&game),
        crate::tuning::FLOOR_TILE_COST,
        "and must spend nothing"
    );
}

/// Settled decision 8, and the only thing pinning the player's word for a
/// laid tile: the fragments are raw stock in the store, the tile is what they
/// becomes underfoot. `BaseGrid`'s floor stays the code's name for it, the
/// same way "GC Entropy Sweep" is the player's word for a raid.
#[test]
fn the_laid_tile_is_named_a_vectorstasis_tile() {
    let mut game = game_on_open_rock(3226, 1);

    game.lay_tile().unwrap();

    let said = game
        .message_history(20)
        .iter()
        .any(|line| line.text.contains("VectorStasis Tile"));
    assert!(said, "laying a tile never names what was laid");
    assert_eq!(
        game.item_name(&ItemId::from(ids::CORE_FRAGMENT)),
        "Core Fragment",
        "and the stock it was pressed from keeps its own name"
    );
}

// ---------------------------------------------------------------------------
// The frontier stays cut
// ---------------------------------------------------------------------------

/// A base with `cut` mined out, and the party parked back on the Home's
/// tile so they are not standing in it.
fn game_with_a_cut_cell(seed: u32, cut: (i32, i32)) -> Game {
    let mut game = game_at_the_frontier(seed);
    excavate(&mut game, (cut.0, cut.1));
    stand_in_base_at(&mut game, TEST_HOME_CELL.0, TEST_HOME_CELL.1);
    game
}

/// Winds the clock `ticks` forward and runs one turn.
fn wait_out(game: &mut Game, ticks: u64) {
    game.world.resource_mut::<GameClock>().tick += ticks;
    game.wait();
}

/// Mined ground stays mined forever: only laid floor may ever take a
/// structure (`Game::place_structure`'s own gate), so leaving a cut cell
/// bare costs nothing, however long the base is left alone.
#[test]
fn an_unfloored_cell_stays_mined_forever() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3230, cut);

    wait_out(&mut game, 1000);

    assert!(
        cell(&game, cut) == Cell::Excavated,
        "an unfloored cell must stay open forever"
    );
}

// ---------------------------------------------------------------------------
// what you dug and what you marked, saved
// ---------------------------------------------------------------------------

/// Through a real `save`/`load` and not only the RON round trip: a round
/// trip cannot catch a `#[serde(skip)]`, and a half-cut wall healing on
/// reload is a bug the first play session would hit.
#[test]
fn a_half_cut_wall_survives_a_save_round_trip() {
    let mut game = game_at_the_frontier_cutting(3240);
    game.move_player(1, 0);
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("one swing starts a dig site");
    let chipped = game.world.get::<Durability>(site).unwrap().hp;
    assert!(
        chipped > 0 && chipped < crate::tuning::BASE_ROCK_DURABILITY,
        "the fixture must leave the wall part-cut, not whole and not open"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_dig_site_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let reloaded = loaded
        .dig_site_at(WALL.0, WALL.1)
        .expect("the half-cut wall must come back as a dig site");
    assert_eq!(
        loaded.world.get::<Durability>(reloaded).unwrap().hp,
        chipped,
        "the wall healed over the reload"
    );
}

/// The mark is written by a verb that does not exist until phase B, so this
/// sets the field on the component directly — the field is what the save
/// carries, and it has to carry it before anything can draw a plan worth
/// losing.
#[test]
fn a_mark_survives_a_save_round_trip() {
    let mut game = game_at_the_frontier_cutting(3241);
    game.move_player(1, 0);
    let site = game.dig_site_at(WALL.0, WALL.1).unwrap();
    game.world.get_mut::<DigSite>(site).unwrap().marked = true;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_dig_mark_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let reloaded = loaded.dig_site_at(WALL.0, WALL.1).unwrap();
    assert!(
        loaded.world.get::<DigSite>(reloaded).unwrap().marked,
        "a plan the player drew must survive the reload that loses it"
    );
}

// ---------------------------------------------------------------------------
// the marks a plan is drawn out of
// ---------------------------------------------------------------------------

/// Cells well outside the fixture pocket, drawn with the
/// far corner *up-left* of the anchor so the box has to be normalised rather
/// than assumed ordered.
const PLAN_ANCHOR: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 2, TEST_POCKET_HALF_WIDTH + 2);
const PLAN_FAR: (i32, i32) = (TEST_POCKET_HALF_WIDTH + 1, TEST_POCKET_HALF_WIDTH + 1);
const PLAN_CELLS: [(i32, i32); 4] = [
    PLAN_FAR,
    (PLAN_FAR.0, PLAN_ANCHOR.1),
    (PLAN_ANCHOR.0, PLAN_FAR.1),
    PLAN_ANCHOR,
];

fn is_marked(game: &mut Game, (x, y): (i32, i32)) -> bool {
    game.dig_site_at(x, y)
        .and_then(|e| game.world.get::<DigSite>(e))
        .is_some_and(|d| d.marked)
}

fn dig_site_count(game: &mut Game) -> usize {
    let mut query = game.world.query_filtered::<Entity, With<DigSite>>();
    query.iter(&game.world).count()
}

#[test]
fn marking_a_box_marks_every_solid_cell_in_it() {
    let mut game = game_at_the_frontier(3250);
    game.dev_lay_floor([(PLAN_FAR.0 - 1, PLAN_FAR.1)]);
    for (x, y) in PLAN_CELLS {
        set_rock(&mut game, (x, y));
        assert!(
            is_solid(&game, (x, y)),
            "the fixture must draw its box over solid rock"
        );
    }

    game.toggle_mark_box(PLAN_ANCHOR, PLAN_FAR, None);

    for cell in PLAN_CELLS {
        assert!(
            is_marked(&mut game, cell),
            "{cell:?} is inside the box the player drew and took no mark"
        );
    }
}

/// Decision 4's erase rule: there is no second verb, so the anchor cell is
/// what says which of the two a box does.
#[test]
fn an_anchor_on_a_marked_cell_clears_the_box_instead_of_marking_it() {
    let mut game = game_at_the_frontier(3251);
    game.toggle_mark_box(PLAN_ANCHOR, PLAN_FAR, None);

    game.toggle_mark_box(PLAN_ANCHOR, PLAN_FAR, None);

    for cell in PLAN_CELLS {
        assert!(
            !is_marked(&mut game, cell),
            "{cell:?} kept its mark when the box was drawn again"
        );
    }
    assert_eq!(
        dig_site_count(&mut game),
        0,
        "clearing a mark on untouched rock must leave no entity behind"
    );
}

/// A `Floor` cell is finished — there is nothing left to do to it, so there
/// is nothing to mark and nothing for the renderer to tint.
#[test]
fn marking_a_floor_cell_does_nothing() {
    let mut game = game_at_the_frontier(3252);
    let floored = (1, 1);
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_floor(floored.0, floored.1),
        "the fixture must draw over a cell the opening pocket already floored"
    );

    game.toggle_mark_box(floored, floored, None);

    assert!(!is_marked(&mut game, floored));
    assert_eq!(
        dig_site_count(&mut game),
        0,
        "a marked floor cell spawned a dig site with nothing to record"
    );
}

/// Marked `Open` means *floor it*, which is the second half of the one verb:
/// a cell someone else already cut still takes a mark.
#[test]
fn marking_an_open_cell_marks_it_for_flooring() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3253, cut);

    game.toggle_mark_box(cut, cut, None);

    assert!(
        is_marked(&mut game, cut),
        "an open, unfloored cell must be markable — that is the flooring half"
    );
}

/// The whole of settled decision 4 in one test: one verb runs a wall all the
/// way from solid to finished floor, and the mark clears itself at the end
/// rather than needing a second erase.
#[test]
fn a_mark_survives_the_cut_and_clears_when_the_cell_is_floored() {
    let mut game = game_at_the_frontier_cutting(3254);
    game.toggle_mark_box(WALL, WALL, None);

    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);
    for _ in 0..swings {
        game.move_player(1, 0);
    }
    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "the fixture must have cut the wall through"
    );
    assert!(
        is_marked(&mut game, WALL),
        "the mark did not survive the cut — marked solid means cut it, \
         marked open means floor it, and the crew needs the second half"
    );

    game.move_player(1, 0);
    assert_eq!(
        game.field_pos().unwrap(),
        WALL,
        "the party stepped onto the cut cell"
    );
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 1);
    game.lay_tile().unwrap();

    assert!(
        !is_marked(&mut game, WALL),
        "a floored cell is still marked"
    );
    assert_eq!(
        dig_site_count(&mut game),
        0,
        "the finished cell left its dig site behind"
    );
}

/// The leak check on the despawn clause: only a *marked* cell keeps its site
/// past the cut, or every wall the player ever hit stays in the world.
#[test]
fn an_unmarked_wall_leaves_no_entity_behind_when_it_is_cut() {
    let mut game = game_at_the_frontier_cutting(3255);
    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);

    for _ in 0..swings {
        game.move_player(1, 0);
    }

    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "the fixture must have cut the wall through"
    );
    assert_eq!(
        dig_site_count(&mut game),
        0,
        "an unmarked wall kept its dig site after it opened"
    );
}

/// The renderer draws these in the order it gets them, so the order has to
/// be a property of the answer rather than of bevy's entity iteration —
/// the same reason `Stock` keys by `BTreeMap`.
#[test]
fn marked_cells_is_sorted() {
    let mut game = game_at_the_frontier(3256);
    let near = PLAN_FAR;
    game.dev_lay_floor([(near.0 - 1, near.1)]);
    let far = (near.0 + 2, near.1 + 2);
    game.toggle_mark_box(far, near, None);

    let cells: Vec<(i32, i32)> = game.marked_cells().into_iter().map(|m| m.pos).collect();

    let mut sorted = cells.clone();
    sorted.sort_unstable();
    assert_eq!(cells, sorted, "marked_cells came back in query order");
    assert_eq!(cells.len(), 9, "a 3x3 box of solid rock is nine marks");
}

/// A mark carries how far through the wall the crew is, so the map can draw
/// it. The meter is the wall's own `Durability` and **not the digger's
/// `Task`**: that one is the swing *cadence* and resets to zero every swing,
/// so a bar drawn off it would sweep full and empty again between chips
/// while the cut itself barely moved.
#[test]
fn a_marks_cut_is_the_wall_it_has_left_and_not_the_swing_cadence() {
    let mut game = game_at_the_frontier_cutting(3257);
    game.toggle_mark_box(WALL, WALL, None);

    let cut_at = |game: &mut Game| {
        game.marked_cells()
            .into_iter()
            .find(|m| m.pos == WALL)
            .expect("the wall was just marked")
            .cut
    };

    assert_eq!(
        cut_at(&mut game),
        Some(0.0),
        "an untouched wall is all wall"
    );

    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);
    assert!(swings > 2, "a wall this thin cannot show a part-cut state");
    game.move_player(1, 0);

    let cut = cut_at(&mut game).expect("a solid wall has a meter");
    assert!(
        cut > 0.0 && cut < 1.0,
        "one swing of {swings} has to read as part-cut, not {cut}"
    );
}

/// A mark on an already-open cell means *floor it*, and that half of the one
/// dig verb has no meter: the wall is already gone, so there is no cut left
/// to report and a bar reading either end of it would be a lie.
#[test]
fn a_floor_it_mark_has_no_cut_to_report() {
    let mut game = game_at_the_frontier_cutting(3258);
    game.toggle_mark_box(WALL, WALL, None);

    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);
    for _ in 0..swings {
        game.move_player(1, 0);
    }
    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "the fixture must have cut the wall through"
    );

    assert_eq!(
        game.marked_cells()
            .into_iter()
            .find(|m| m.pos == WALL)
            .expect("the mark survives the cut")
            .cut,
        None
    );
}

// ---------------------------------------------------------------------------
// Floor finishes: a finish or a strip is a third kind of dig mark
// ---------------------------------------------------------------------------

/// The five cell states the brush table is keyed on — `BaseGrid`'s own
/// state, never a pending `DigSite`, which is what makes replacing a finish
/// target what is actually laid rather than a job still queued behind it.
#[derive(Clone, Copy, Debug)]
enum CellState {
    Solid,
    Open,
    FloorPlain,
    FloorX,
    FloorY,
}

#[derive(Clone, Copy, Debug)]
enum Brush {
    Plain,
    FinishX,
    Strip,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    Skip,
    MarkPlain,
    MarkFinishX,
    MarkStrip,
}

/// One assertion per cell of the design's brush table — three brushes times
/// five cell states, fifteen cases, each a fresh fixture so no case can leak
/// state into the next.
#[test]
fn the_brush_table_matches_the_design() {
    let finish_x = FloorId::from("cobalt_carpet");
    let finish_y = FloorId::from("moss_weave");

    let cases: &[(CellState, Brush, Outcome)] = &[
        (CellState::Solid, Brush::Plain, Outcome::MarkPlain),
        (CellState::Open, Brush::Plain, Outcome::MarkPlain),
        (CellState::FloorPlain, Brush::Plain, Outcome::Skip),
        (CellState::FloorX, Brush::Plain, Outcome::Skip),
        (CellState::FloorY, Brush::Plain, Outcome::Skip),
        (CellState::Solid, Brush::FinishX, Outcome::Skip),
        (CellState::Open, Brush::FinishX, Outcome::Skip),
        (CellState::FloorPlain, Brush::FinishX, Outcome::MarkFinishX),
        (CellState::FloorX, Brush::FinishX, Outcome::Skip),
        (CellState::FloorY, Brush::FinishX, Outcome::MarkFinishX),
        (CellState::Solid, Brush::Strip, Outcome::Skip),
        (CellState::Open, Brush::Strip, Outcome::Skip),
        (CellState::FloorPlain, Brush::Strip, Outcome::Skip),
        (CellState::FloorX, Brush::Strip, Outcome::MarkStrip),
        (CellState::FloorY, Brush::Strip, Outcome::MarkStrip),
    ];

    for &(state, brush, expected) in cases {
        let mut game = game_at_the_frontier(3259);
        let cell = WALL;
        match state {
            CellState::Solid => {}
            CellState::Open => excavate(&mut game, cell),
            CellState::FloorPlain => {
                game.world
                    .resource_mut::<base_grid::BaseGrid>()
                    .lay_floor(cell.0, cell.1);
            }
            CellState::FloorX => {
                let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
                grid.lay_floor(cell.0, cell.1);
                grid.set_finish(cell.0, cell.1, finish_x.clone());
            }
            CellState::FloorY => {
                let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
                grid.lay_floor(cell.0, cell.1);
                grid.set_finish(cell.0, cell.1, finish_y.clone());
            }
        }

        let order = match brush {
            Brush::Plain => None,
            Brush::FinishX => Some(FinishOrder::Apply(finish_x.clone())),
            Brush::Strip => Some(FinishOrder::Strip),
        };
        game.toggle_mark_box(cell, cell, order.as_ref());

        let site = game.dig_site_at(cell.0, cell.1);
        let name = || format!("{state:?} cell, {brush:?} brush");
        match expected {
            Outcome::Skip => {
                assert!(
                    site.is_none(),
                    "{}: expected no mark, but one was spawned",
                    name()
                );
            }
            Outcome::MarkPlain => {
                let dig = game
                    .world
                    .get::<DigSite>(site.unwrap_or_else(|| panic!("{}: expected a mark", name())))
                    .unwrap();
                assert!(dig.marked, "{}", name());
                assert_eq!(
                    dig.finish,
                    None,
                    "{}: a plain mark carried a finish",
                    name()
                );
            }
            Outcome::MarkFinishX => {
                let dig = game
                    .world
                    .get::<DigSite>(site.unwrap_or_else(|| panic!("{}: expected a mark", name())))
                    .unwrap();
                assert!(dig.marked, "{}", name());
                assert_eq!(
                    dig.finish,
                    Some(FinishOrder::Apply(finish_x.clone())),
                    "{}",
                    name()
                );
            }
            Outcome::MarkStrip => {
                let dig = game
                    .world
                    .get::<DigSite>(site.unwrap_or_else(|| panic!("{}: expected a mark", name())))
                    .unwrap();
                assert!(dig.marked, "{}", name());
                assert_eq!(dig.finish, Some(FinishOrder::Strip), "{}", name());
            }
        }
    }
}

/// A finish-marked box clears exactly as a plain one does — the anchor
/// decides, and clearing does not care what kind of mark it is clearing.
#[test]
fn a_finish_marked_box_is_cleared_by_recommitting_with_the_plain_brush() {
    let mut game = game_at_the_frontier(3260);
    let cell = WALL;
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(cell.0, cell.1);
    let apply = FinishOrder::Apply(FloorId::from("cobalt_carpet"));
    game.toggle_mark_box(cell, cell, Some(&apply));
    assert!(
        game.dig_site_at(cell.0, cell.1).is_some(),
        "the fixture must have actually marked the cell"
    );

    game.toggle_mark_box(cell, cell, None);

    assert!(
        game.dig_site_at(cell.0, cell.1).is_none(),
        "re-committing the same box left the finish mark standing"
    );
}

/// Save/load's own gate for `DigSite::finish` — a real round trip through
/// `Game::save`/`Game::load`, which a RON round trip cannot hold to: it
/// cannot catch a skipped field or a load path that forgets to restore one.
#[test]
fn a_finish_mark_and_a_strip_mark_survive_a_real_save_and_load() {
    let mut game = game_at_the_frontier(3261);
    let finished = WALL;
    let stripped = (WALL.0 + 1, WALL.1);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(finished.0, finished.1);
        grid.lay_floor(stripped.0, stripped.1);
        grid.set_finish(stripped.0, stripped.1, FloorId::from("cobalt_carpet"));
    }
    let apply = FinishOrder::Apply(FloorId::from("moss_weave"));
    game.toggle_mark_box(finished, finished, Some(&apply));
    game.toggle_mark_box(stripped, stripped, Some(&FinishOrder::Strip));

    let path = std::env::temp_dir().join(format!(
        "feral_processes_finish_mark_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let applied_site = loaded
        .dig_site_at(finished.0, finished.1)
        .expect("the Apply mark must survive the reload");
    assert_eq!(
        loaded.world.get::<DigSite>(applied_site).unwrap().finish,
        Some(FinishOrder::Apply(FloorId::from("moss_weave")))
    );
    let stripped_site = loaded
        .dig_site_at(stripped.0, stripped.1)
        .expect("the Strip mark must survive the reload");
    assert_eq!(
        loaded.world.get::<DigSite>(stripped_site).unwrap().finish,
        Some(FinishOrder::Strip)
    );
}

/// The Floors screen lists finish and strip marks; the Digging screen lists
/// plain marks; no cell is on both, and the map's `marked_cells` has all.
#[test]
fn floor_marks_exclude_plain_dig_marks_and_digging_excludes_floor_marks() {
    let mut game = game_at_the_frontier(3262);
    let dug = WALL;
    let finished = (WALL.0 + 1, WALL.1);
    let stripped = (WALL.0 + 2, WALL.1);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(finished.0, finished.1);
        grid.lay_floor(stripped.0, stripped.1);
        grid.set_finish(stripped.0, stripped.1, FloorId::from("cobalt_carpet"));
    }
    game.toggle_mark_box(dug, dug, None);
    let apply = FinishOrder::Apply(FloorId::from("moss_weave"));
    game.toggle_mark_box(finished, finished, Some(&apply));
    game.toggle_mark_box(stripped, stripped, Some(&FinishOrder::Strip));

    let floors = game.floor_marks();
    let digs = game.dig_marks();

    assert_eq!(
        floors.iter().map(|m| m.pos).collect::<Vec<_>>(),
        vec![finished, stripped]
    );
    assert_eq!(floors[1].finish, None, "a strip names no finish");
    assert!(
        floors[0].finish.as_deref().is_some_and(|n| !n.is_empty()),
        "an Apply names the finish it lays"
    );
    assert_eq!(digs.iter().map(|m| m.pos).collect::<Vec<_>>(), vec![dug]);
    assert_eq!(game.marked_cells().len(), 3);
}

// ---------------------------------------------------------------------------
// the crew
// ---------------------------------------------------------------------------

/// A base with `n` programs on its staff and the party parked back on the
/// Home's tile.
///
/// The party's cell matters: everything below is about work that happens
/// while nobody is standing over it, and a fixture that left the player at
/// the frontier could credit the crew with a wall the player's own bumps
/// brought down.
fn base_with_a_crew(seed: u32, n: usize) -> (Game, Vec<Entity>) {
    let mut game = game_at_the_frontier(seed);
    stand_in_base_at(&mut game, TEST_HOME_CELL.0, TEST_HOME_CELL.1);
    let mut staff = Vec::new();
    for _ in 0..n {
        // No assign call: an owned program outside the party is staff.
        let worker = spawn_tamed(&mut game, 10, 3);
        staff.push(worker);
    }
    staff.sort();
    (game, staff)
}

fn mark(game: &mut Game, cell: (i32, i32)) {
    game.toggle_mark_box(cell, cell, None);
}

/// Marks `cell` although nothing floored touches it: floor is laid beside it
/// just long enough for the mark to be accepted, then taken back. The state an
/// old save can hold, for the tests about a crew that must not trust the mark.
fn mark_unchecked(game: &mut Game, cell: (i32, i32)) {
    let beside = (cell.0 - 1, cell.1);
    game.dev_lay_floor([beside]);
    mark(game, cell);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .revert(beside.0, beside.1);
}

fn pass(game: &mut Game, ticks: usize) {
    for _ in 0..ticks {
        game.tick();
    }
}

fn posted_at(game: &Game, worker: Entity) -> Option<Entity> {
    game.world.get::<Task>(worker).map(|t| t.target)
}

/// Long enough for a crew to walk out to the frontier and cut `WALL` down,
/// derived from the constants rather than written out: retuning the swing
/// rate or the rock's durability retunes the wait with it.
fn ticks_to_cut(game: &Game, worker: Entity) -> usize {
    let swings = swings_for(game, worker, WALL);
    (swings * crate::tuning::BASE_DIG_TICKS_PER_SWING) as usize + WALK_ALLOWANCE
}

/// Slack for the walk from wherever an idle body was drifting out to the
/// wall, which is a handful of tiles at one tile a tick.
const WALK_ALLOWANCE: usize = 20;

/// Raw log lines, deliberately **not** `Game::message_history`: that folds
/// repeats through `resources::condense`, so a line pushed every tick would
/// count as one and "it says so exactly once" would pass against no fix at
/// all.
fn lines_saying(game: &Game, needle: &str) -> usize {
    game.message_log(500)
        .iter()
        .filter(|line| line.text.contains(needle))
        .count()
}

/// The fragment of the stall announcement every test below counts on. Not
/// `"is cut off"`, which `MachineStatus::Stranded` also says: two different
/// stalls sharing a needle would let either one satisfy the other's test.
const CUT_OFF: &str = "marked cell at";

/// The fragment of the *other* stall announcement — a crew that has cut its
/// cell open and has no Blank Substrate anywhere to floor it with.
const NO_FRAGMENTS: &str = "nothing to floor";

/// Exactly enough Core Fragments on a base shelf to floor `tiles` cells,
/// and none in the pack — `0` is the dry crew. The crew pays for tiles out
/// of base stores only, so the pack is emptied rather than left holding
/// `game_at_the_frontier`'s change, which would otherwise hide a crew that
/// reached into it.
///
/// A cut needs none — the crew cuts with nothing in store and floors later
/// — so this is for the tests that expect the cell floored, or that want
/// the crew's next job to be something other than a dry tile.
fn fragments_for(game: &mut Game, tiles: u32) {
    set_fragments(game, 0);
    let shelf = fragment_shelf(game);
    let mut stock = game.world.get_mut::<Stock>(shelf).unwrap();
    let fragment = ItemId::from(ids::CORE_FRAGMENT);
    match tiles * crate::tuning::FLOOR_TILE_COST {
        0 => {
            stock.output.remove(&fragment);
        }
        n => {
            stock.output.insert(fragment, n);
        }
    }
}

/// The one Depot `fragments_for` stocks, spawned on first use at the
/// starting pocket's west edge, clear of `WALL` and the Home's tile.
fn fragment_shelf(game: &mut Game) -> Entity {
    let at = Position {
        x: -TEST_POCKET_HALF_WIDTH,
        y: TEST_POCKET_HALF_HEIGHT,
    };
    let mut query = game.world.query::<(Entity, &Structure, &Position)>();
    if let Some((e, ..)) = query
        .iter(&game.world)
        .find(|(_, s, p)| s.kind == "depot" && **p == at)
    {
        return e;
    }
    spawn_machine_at(game, "depot", at.x, at.y)
}

/// Core Fragments left on `fragments_for`'s shelf.
fn shelf_fragments(game: &mut Game) -> u32 {
    let shelf = fragment_shelf(game);
    game.world
        .get::<Stock>(shelf)
        .unwrap()
        .output
        .get(&ItemId::from(ids::CORE_FRAGMENT))
        .copied()
        .unwrap_or(0)
}

/// The whole claim of the feature: the base grows while you are somewhere
/// else.
#[test]
fn a_crew_cuts_a_marked_wall_without_the_player() {
    let (mut game, staff) = base_with_a_crew(3260, 1);
    mark(&mut game, WALL);
    assert!(
        is_solid(&game, (WALL.0, WALL.1)),
        "precondition: the marked cell starts solid"
    );

    let wait = ticks_to_cut(&game, staff[0]);
    pass(&mut game, wait);

    // Any opened state, not `Open` alone: the test is about the cut, and
    // whether a tile follows is the substrate tests' question.
    assert!(
        cell(&game, WALL) != Cell::Solid,
        "the crew never cut the wall the player marked"
    );
}

/// The cell `WALL` cut open already, so the crew's next landing there is
/// the tile and nothing else — no swing, and no fragment shaking loose from
/// a cut to muddy a count.
fn open_wall(game: &mut Game) {
    excavate(game, WALL);
}

/// Settled decision 4's second half, worked by somebody else: the mark
/// survives the cut, so the same body floors what it just opened.
#[test]
fn a_crew_floors_a_marked_cell_after_cutting_it() {
    let (mut game, staff) = base_with_a_crew(3261, 1);
    fragments_for(&mut game, 3);
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait);

    assert_eq!(
        cell(&game, WALL),
        Cell::Floor,
        "the crew cut the cell and left it bare"
    );
    assert!(
        game.dig_site_at(WALL.0, WALL.1).is_none(),
        "a finished cell keeps no dig site"
    );
}

/// **Resolved question 1, worked end to end by the crew.** A field blocker
/// next to floor, marked with the plain brush, is a floor request the base
/// accepts while the cell is still rock: the crew digs it first — the cell
/// becomes an `Excavated` override — and then floors it, without the player.
///
/// A blocker of its own kind on the pocket's north edge rather than `WALL`,
/// so nothing about the frontier fixture's band is doing the work.
#[test]
fn a_marked_blocker_beside_floor_is_dug_and_then_floored_by_the_crew() {
    let (mut game, staff) = base_with_a_crew(3292, 1);
    fragments_for(&mut game, 1);
    let blocker = (0, -TEST_POCKET_HALF_HEIGHT - 1);
    set_blocker(&mut game, blocker, Biome::DataVoid);
    assert_eq!(cell(&game, blocker), Cell::Solid, "precondition: rock");

    game.toggle_mark_box(blocker, blocker, None);
    assert!(
        is_marked(&mut game, blocker),
        "a blocker beside floor takes a floor request"
    );

    let budget = (swings_for(&game, staff[0], blocker) + 1)
        * crate::tuning::BASE_DIG_TICKS_PER_SWING
        + WALK_ALLOWANCE as u32;
    for _ in 0..budget * 2 {
        if cell(&game, blocker) == Cell::Floor {
            break;
        }
        game.tick();
    }

    assert_eq!(
        cell(&game, blocker),
        Cell::Floor,
        "the crew left the marked blocker unfloored"
    );
    assert_eq!(
        game.world
            .resource::<WorldMap>()
            .peek(blocker.0, blocker.1)
            .biome,
        Biome::Excavated,
        "the floor went down over a cell the crew dug, not over rock"
    );
    assert!(
        game.dig_site_at(blocker.0, blocker.1).is_none(),
        "a finished cell keeps no dig site"
    );
}

/// The other half of the gate (`field_mining::minable`): the crew digs
/// outward from the floor, so a marked blocker that touches neither floor
/// nor pending floor is never cut, even with a body posted beside it.
///
/// The body is posted by hand on mined ground next to the cell — the
/// scheduler would never send one, since a station is a floor face — so the
/// gate in `run_dig_crew` is the only thing between it and a swing. Then a
/// floor request on the mined cell it stands on makes the blocker pending-
/// adjacent, and the same body cuts it: the refusal was the gate's and
/// nothing else's.
#[test]
fn a_marked_blocker_off_the_floor_is_not_cut_until_floor_is_pending_beside_it() {
    let mut game = game_at_the_frontier(3293);
    // `WALL` mined but not floored or marked; the blocker beyond it touches
    // only that and rock.
    excavate(&mut game, WALL);
    let blocker = (WALL.0 + 1, WALL.1);
    mark_unchecked(&mut game, blocker);
    let site = game
        .dig_site_at(blocker.0, blocker.1)
        .expect("a marked blocker has a dig site");
    let full = game.world.get::<Durability>(site).unwrap().hp;

    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: WALL.0,
            y: WALL.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    for _ in 0..crate::tuning::BASE_DIG_TICKS_PER_SWING * 3 {
        game.run_dig_crew();
    }

    assert_eq!(
        game.world.get::<Durability>(site).map(|d| d.hp),
        Some(full),
        "the crew swung at a blocker no floor or pending floor touches"
    );
    assert_eq!(cell(&game, blocker), Cell::Solid);

    // A floor request on the mined cell: pending floor beside the blocker.
    game.toggle_mark_box(WALL, WALL, None);
    for _ in 0..crate::tuning::BASE_DIG_TICKS_PER_SWING {
        game.run_dig_crew();
    }

    assert!(
        game.world
            .get::<Durability>(site)
            .is_none_or(|d| d.hp < full),
        "with pending floor beside it the same body must cut it — or the \
         refusal above was not the gate's"
    );
}

/// The crew pays the same `FLOOR_TILE_COST` Core Fragments the player's own
/// tile costs, and never a Blank Substrate it happens to have to hand.
#[test]
fn a_crew_tile_costs_the_same_fragments_as_a_hand_laid_one() {
    let (mut game, _) = base_with_a_crew(3290, 1);
    open_wall(&mut game);
    fragments_for(&mut game, 3);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 2);
    mark(&mut game, WALL);

    pass(
        &mut game,
        4 * crate::tuning::BASE_DIG_TICKS_PER_SWING as usize,
    );

    assert_eq!(cell(&game, WALL), Cell::Floor);
    assert_eq!(
        shelf_fragments(&mut game),
        2 * crate::tuning::FLOOR_TILE_COST,
        "a laid tile costs exactly FLOOR_TILE_COST Core Fragments, whoever lays it"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        2,
        "a plain tile spends no substrate"
    );
}

/// **The crew is base labour and pays out of the base's own stores** — the
/// same buffers the stock block counts, and the same ones `base_holding`
/// reads. Paying out of the player's pack alone left a crew standing on a
/// cut it had just finished while a Depot four tiles away held the
/// stock, and it said nothing about it: the store the base filled was
/// not the store the crew could spend.
#[test]
fn a_crew_pays_for_its_tile_out_of_the_base_stock() {
    let (mut game, _) = base_with_a_crew(3280, 1);
    open_wall(&mut game);
    fragments_for(&mut game, 0);
    let cost = crate::tuning::FLOOR_TILE_COST;
    let depot = spawn_machine_at(&mut game, "depot", 2, 0);
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 3 * cost);
    mark(&mut game, WALL);

    pass(
        &mut game,
        4 * crate::tuning::BASE_DIG_TICKS_PER_SWING as usize,
    );

    assert_eq!(
        cell(&game, WALL),
        Cell::Floor,
        "the crew left the cell bare with a full shelf beside it"
    );
    assert_eq!(
        game.world.get::<Stock>(depot).unwrap().output[&ItemId::from(ids::CORE_FRAGMENT)],
        2 * cost,
        "a laid tile costs exactly FLOOR_TILE_COST Core Fragments, whichever store pays"
    );
}

/// **The crew never spends the player's fragments.** Core Fragments are
/// what the player builds and trades with, and they cross a breach in the
/// pack: a crew that fell back on the pack would quietly spend a marked
/// corridor's worth while the party is away, and the dry line would never
/// fire because the pack covered it. Laying a tile out of the pack is the
/// player's own verb, `Game::lay_tile`.
#[test]
fn a_crew_never_spends_the_player_s_fragments() {
    let (mut game, _) = base_with_a_crew(3281, 1);
    open_wall(&mut game);
    fragments_for(&mut game, 0);
    set_fragments(&mut game, 3 * crate::tuning::FLOOR_TILE_COST);
    mark(&mut game, WALL);

    pass(
        &mut game,
        4 * crate::tuning::BASE_DIG_TICKS_PER_SWING as usize,
    );

    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "the crew floored the cell out of the player's pack"
    );
    assert_eq!(fragments_held(&game), 3 * crate::tuning::FLOOR_TILE_COST);
    assert_eq!(
        lines_saying(&game, NO_FRAGMENTS),
        1,
        "with the base dry the crew says so, whatever the pack holds"
    );
}

/// A crew with nothing to lay says so **once**, under
/// `systems::set_machine_status`' rule: entering a state is news, staying in
/// it is not. Silence is what this cost the player — a marked cell, a body
/// standing on it, and no way to find out that the only thing missing was
/// stock.
#[test]
fn a_crew_with_no_fragments_says_so_once() {
    let (mut game, staff) = base_with_a_crew(3282, 1);
    fragments_for(&mut game, 0);
    // Cut open by hand rather than by the crew, so the floor job this test
    // is about is in front of it from the first tick.
    excavate(&mut game, (WALL.0, WALL.1));
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait * 4);

    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "a cell floored with no fragments anywhere"
    );
    assert!(
        is_marked(&mut game, WALL),
        "the plan should outlive the shortage — the tile goes down when stock does"
    );
    assert_eq!(
        lines_saying(&game, NO_FRAGMENTS),
        1,
        "the crew's shortage is news once, not once a cycle for the rest of the run"
    );
}

/// The dry latch is not a one-way door: a *later* shortage at the same
/// site has to be news too, or the crew reads as broken the moment a run
/// digs more than one cell dry.
///
/// A tile's fragments appearing and vanishing again before this site's own cycle
/// reaches it is not contrived — it is what happens every time a restocked
/// unit gets claimed by a different site (or a build request) first, and
/// the base is dry from this site's point of view exactly as before. The
/// give-then-take here stands in for that race without needing a second
/// consumer in the fixture.
///
/// Mutation-proof the same way the build side's twin is: delete the
/// clearing arm and the second assertion fails; drop the latch altogether
/// and the first one does.
#[test]
fn a_crew_reports_a_later_drought_at_the_same_site() {
    let (mut game, staff) = base_with_a_crew(3283, 1);
    fragments_for(&mut game, 0);
    // A floor job, opened by hand for `a_crew_with_no_fragments_says_so_once`'s
    // reason.
    excavate(&mut game, (WALL.0, WALL.1));
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait);

    assert_eq!(
        lines_saying(&game, NO_FRAGMENTS),
        1,
        "the first drought is news"
    );

    // A source exists for one tick — long enough for the scheduler to see
    // it and clear the latch — and is gone again before this site's own
    // cycle comes back around to spend it.
    fragments_for(&mut game, 1);
    game.tick();
    fragments_for(&mut game, 0);

    pass(
        &mut game,
        crate::tuning::BASE_DIG_TICKS_PER_SWING as usize - 1,
    );

    assert_eq!(
        lines_saying(&game, NO_FRAGMENTS),
        2,
        "a later drought at the same site must be news too, not permanent silence"
    );
}

/// **The deadlock this closes**: a body pinned to a job it cannot pay for is
/// a body that cannot go make the thing it is short of. A dry base with a
/// tile waiting to be laid and a Mining Node on order has to send its one
/// body to the node — which is the only way the fragments a tile costs ever
/// arrive — rather than stand it at a tile it may not lay.
/// `a_request_the_base_cannot_supply_does_not_deadlock_production`'s shape,
/// one subsystem over.
#[test]
fn a_dry_dig_job_frees_the_body_for_production() {
    let (mut game, staff) = base_with_a_crew(3284, 1);
    fragments_for(&mut game, 0);
    excavate(&mut game, (WALL.0, WALL.1));
    let mine = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    game.set_standing_job(mine, true, false).unwrap();
    mark(&mut game, WALL);

    pass(&mut game, 2);

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "a body held at a tile the base cannot floor is a base that never \
         mines the fragments that would let it floor"
    );
    assert!(
        is_marked(&mut game, WALL),
        "the plan outlives the shortage — dropping the want must not touch the mark"
    );
}

/// **The rule**: cutting rock is never blocked by a missing tile. The crew
/// mines the cell out now and floors it once the fragments exist. The open,
/// still-marked cell is then an ordinary tile job, dry until they arrive —
/// told once, in the tile job's own words.
#[test]
fn a_crew_cuts_with_no_fragments_and_floors_once_they_arrive() {
    let (mut game, staff) = base_with_a_crew(3285, 1);
    fragments_for(&mut game, 0);
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]);
    pass(&mut game, wait * 2);

    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "a marked cell must be cut even with nothing to floor it with, got {:?}",
        cell(&game, WALL)
    );
    assert!(
        is_marked(&mut game, WALL),
        "the plan outlives the shortage — the tile goes down when stock does"
    );
    assert_eq!(
        lines_saying(&game, NO_FRAGMENTS),
        1,
        "the cut cell's floor job is dry, and that is news once"
    );

    fragments_for(&mut game, 1);
    pass(
        &mut game,
        crate::tuning::BASE_DIG_TICKS_PER_SWING as usize + WALK_ALLOWANCE,
    );

    assert_eq!(
        cell(&game, WALL),
        Cell::Floor,
        "the cut cell is floored once a tile's fragments exist"
    );
}

/// A base one fragment short of a tile is dry, not half-paid: the budget
/// claims the whole `FLOOR_TILE_COST`, so the job is dropped and said once
/// rather than posted to a body that cannot lay it.
#[test]
fn a_crew_one_fragment_short_of_a_tile_is_dry() {
    let (mut game, _) = base_with_a_crew(3291, 1);
    open_wall(&mut game);
    fragments_for(&mut game, 0);
    let shelf = fragment_shelf(&mut game);
    game.world.get_mut::<Stock>(shelf).unwrap().output.insert(
        ItemId::from(ids::CORE_FRAGMENT),
        crate::tuning::FLOOR_TILE_COST - 1,
    );
    mark(&mut game, WALL);

    pass(
        &mut game,
        4 * crate::tuning::BASE_DIG_TICKS_PER_SWING as usize,
    );

    assert!(cell(&game, WALL) == Cell::Excavated);
    assert_eq!(lines_saying(&game, NO_FRAGMENTS), 1);
    assert_eq!(
        shelf_fragments(&mut game),
        crate::tuning::FLOOR_TILE_COST - 1
    );
}

/// **The budget is shared, and one tile's fragments floor one cell.** Both
/// cells are opened by hand: a crew cut can shake a fragment loose, and two
/// of those would pay for the second tile and make the count luck.
#[test]
fn one_tile_of_fragments_floors_one_of_two_cells() {
    let (mut game, _) = base_with_a_crew(3288, 2);
    let second = (WALL.0, WALL.1 + 1);
    for (x, y) in [WALL, second] {
        excavate(&mut game, (x, y));
    }
    fragments_for(&mut game, 1);
    mark(&mut game, WALL);
    mark(&mut game, second);

    pass(
        &mut game,
        4 * (crate::tuning::BASE_DIG_TICKS_PER_SWING as usize + WALK_ALLOWANCE),
    );

    let floored = [WALL, second]
        .into_iter()
        .filter(|c| cell(&game, *c) == Cell::Floor)
        .count();
    assert_eq!(
        floored, 1,
        "one tile's fragments must floor one cell, not both or neither"
    );
}

/// **A cell already open outranks one still solid**: laying its tile spends
/// the shared fragments and cutting the other cell does not, so with one
/// body the tile job runs first — and finishes fast enough that the cut
/// behind it has not even started.
#[test]
fn a_cell_waiting_on_its_floor_outranks_a_cell_still_whole() {
    let (mut game, _staff) = base_with_a_crew(3289, 1);
    let solid = (WALL.0, WALL.1 + 1);
    excavate(&mut game, (WALL.0, WALL.1));
    fragments_for(&mut game, 1);
    mark(&mut game, WALL);
    mark(&mut game, solid);

    // Exactly one floor cycle plus the walk out: enough to land WALL's tile
    // job and nothing more, so a crew that touched `solid` at all in this
    // window would have to be cutting it rather than finishing WALL first.
    let wait = crate::tuning::BASE_DIG_TICKS_PER_SWING as usize + WALK_ALLOWANCE;
    pass(&mut game, wait);

    assert_eq!(
        cell(&game, WALL),
        Cell::Floor,
        "the open cell must get the one tile — it is the one already exposed"
    );
    assert!(
        is_solid(&game, (solid.0, solid.1)),
        "and the cut waits its turn behind the open cell's own floor job"
    );
}

/// The dropped want has to reach the body, not merely stop being listed —
/// and the case that actually exercises the difference is a site that was
/// *already posted* going dry, not one that was dry from the moment it was
/// marked. With nothing else in the base, `wanted` goes from one entry to
/// none the tick the drought hits: `schedule_base_labour`'s "quiet base"
/// guard exists to skip re-diffing a want list that looks the same tick to
/// tick, and `wanted.iter().all(|p| posted.contains(p))` is vacuously true
/// against an *empty* `wanted` whatever `posted` holds — so an emptied want
/// list is exactly the state that guard must not mistake for "nothing
/// changed": the body's stale `Task` still names the site.
///
/// Posted by hand rather than walked into place: `post_digger` and
/// `dig_site_at` are `pub(crate)`, and a walk of unknown length racing a
/// twelve-tick swing cycle cannot pin the moment progress starts without
/// either finishing the cut early (and despawning the very site this test
/// needs to still exist) or never quite catching it posted at all — the
/// first draft of this test did exactly the former.
#[test]
fn a_floor_job_that_dries_up_mid_cycle_still_frees_its_body() {
    let (mut game, staff) = base_with_a_crew(3287, 1);
    excavate(&mut game, (WALL.0, WALL.1));
    mark(&mut game, WALL);
    fragments_for(&mut game, 1);
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("marking an open cell spawns its dig site");

    // On the floor face west of the cell: the crew stands on floor alone.
    let worker = staff[0];
    if let Some(mut pos) = game.world.get_mut::<Position>(worker) {
        *pos = Position {
            x: WALL.0 - 1,
            y: WALL.1,
        };
    }
    game.post_digger(worker, site);
    // A few swings in, well short of `BASE_DIG_TICKS_PER_SWING` — the site
    // must still be mid-cycle, not yet due to attempt the lay.
    pass(&mut game, 3);
    let progress = game.world.get::<Task>(worker).map(|t| t.progress);
    assert!(
        matches!(progress, Some(p) if p > 0 && p < crate::tuning::BASE_DIG_TICKS_PER_SWING),
        "sanity: the body must still be mid-swing for the transition below \
         to mean anything, got {progress:?}"
    );

    // The one tile's fragments vanish before this site's own cycle comes
    // back around to spend them — the same race
    // `a_crew_reports_a_later_drought_at_the_same_site` stands in for, but
    // with nothing else in the base to fall back on.
    fragments_for(&mut game, 0);

    pass(
        &mut game,
        crate::tuning::BASE_DIG_TICKS_PER_SWING as usize + WALK_ALLOWANCE,
    );

    assert_eq!(
        posted_at(&game, worker),
        None,
        "a body posted to a want that dried up and vanished is not freed, \
         it is merely unlisted"
    );
}

/// Settled decision 7, and the reason digging cannot starve production: dig
/// wants are appended last, so `truncate(staff.len())` cuts them first.
#[test]
fn a_dig_job_never_takes_a_body_off_a_work_order() {
    let (mut game, staff) = base_with_a_crew(3262, 1);
    let mine = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 60))
        .unwrap();
    mark(&mut game, WALL);

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "the base's one body must work the order, not the excavation"
    );
}

#[test]
fn a_dig_job_is_taken_when_there_is_a_spare_body() {
    let (mut game, staff) = base_with_a_crew(3263, 2);
    fragments_for(&mut game, 1);
    let mine = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 60))
        .unwrap();
    mark(&mut game, WALL);

    game.tick();

    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("marking a wall spawns its dig site");
    let posts: Vec<Entity> = staff.iter().filter_map(|&s| posted_at(&game, s)).collect();
    assert!(
        posts.contains(&mine),
        "the order still comes first, got {posts:?}"
    );
    assert!(
        posts.contains(&site),
        "the spare body has nothing else to do and must dig, got {posts:?}"
    );
}

/// The tile a stranded station stands on, and the cell marked beside it.
/// A lone floor tile far enough out that no floor joins it to the base —
/// the crew walks on floor alone — so a station exists and no route to it
/// does: `NoPost::NoRoute`, the half of the split that is the player's
/// errand.
const STRANDED_STATION: (i32, i32) = (20, 0);
const STRANDED_CELL: (i32, i32) = (21, 0);

fn game_with_an_unroutable_mark(seed: u32) -> (Game, Vec<Entity>) {
    let (mut game, staff) = base_with_a_crew(seed, 1);
    // A cut the base could pay to floor: these tests are about the route,
    // and a dry base drops the want before the route is ever in question.
    fragments_for(&mut game, 1);
    game.dev_lay_floor([STRANDED_STATION]);
    set_rock(&mut game, STRANDED_CELL);
    mark(&mut game, STRANDED_CELL);
    (game, staff)
}

/// `set_machine_status`' rule, one subsystem over: entering a state is news
/// and staying in it is not.
#[test]
fn an_unreachable_dig_site_complains_exactly_once() {
    let (mut game, _) = game_with_an_unroutable_mark(3264);

    pass(&mut game, 20);

    assert_eq!(
        lines_saying(&game, CUT_OFF),
        1,
        "a stall the player has to fix is announced once, not every tick"
    );
}

/// "Can anybody reach this" is asked of the staff, so with nobody on shift
/// it answers no for want of an *anybody* — and every marked cell in the
/// base would be announced cut off by a crew that is simply not at work.
/// Nothing is posted on such a tick either way, so the guard costs nothing
/// and the sentence is the whole of what it buys.
#[test]
fn a_base_with_nobody_on_shift_does_not_call_its_marks_cut_off() {
    let (mut game, staff) = base_with_a_crew(3267, 1);
    mark(&mut game, WALL);
    for worker in staff {
        game.world
            .entity_mut(worker)
            .insert(crate::components::Downed);
    }

    pass(&mut game, 20);

    assert_eq!(
        lines_saying(&game, CUT_OFF),
        0,
        "a reachable wall was called cut off because the crew was off its feet"
    );
}

/// Settled decision 6's silent half. The interior of any block you mark has
/// no face to stand at and resolves itself as the shell comes down, so
/// saying so would fire for every buried cell of every plan ever drawn.
#[test]
fn a_dig_site_with_no_exposed_face_never_complains() {
    let (mut game, _) = base_with_a_crew(3265, 1);
    let buried = (20, 20);
    mark(&mut game, buried);
    // A face is a floor cell beside the mark — the crew stands on floor
    // alone — so a mark with no floor around it has none.
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert!(
            !game
                .world
                .resource::<base_grid::BaseGrid>()
                .is_floor(buried.0 + dx, buried.1 + dy),
            "precondition: the marked cell must have no face to stand at"
        );
    }

    pass(&mut game, 20);

    assert_eq!(
        lines_saying(&game, CUT_OFF),
        0,
        "a buried cell is the normal interior of a plan and must stay silent"
    );
}

/// The flag has to clear, or the second stall is silent forever.
#[test]
fn a_site_that_becomes_reachable_again_can_complain_again() {
    let (mut game, staff) = game_with_an_unroutable_mark(3266);
    pass(&mut game, 5);
    assert_eq!(lines_saying(&game, CUT_OFF), 1, "precondition");

    // A corridor from the pocket out to the stranded station. The station
    // itself is not part of it — reverting the corridor below has to leave
    // a tile to stand on, or the site would come back as `BoxedIn` and be
    // silent for a different reason than the one under test.
    let corridor = (TEST_POCKET_HALF_WIDTH + 1)..STRANDED_STATION.0;
    game.dev_lay_floor(corridor.clone().map(|x| (x, 0)));
    game.tick();
    let site = game
        .dig_site_at(STRANDED_CELL.0, STRANDED_CELL.1)
        .expect("the mark is still standing");
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(site),
        "precondition: a route means a posting"
    );

    for x in corridor {
        game.world
            .resource_mut::<base_grid::BaseGrid>()
            .revert(x, 0);
    }
    pass(&mut game, 5);

    assert_eq!(
        lines_saying(&game, CUT_OFF),
        2,
        "a route lost after the posting is news again"
    );
}

/// The middle rung of settled decision 7. Dig jobs sit below work orders
/// *and* below standing jobs, and the order test alone cannot see the
/// difference: an order's wants are built before standing jobs either way,
/// so a plan that only outranked orders would still pass it.
#[test]
fn a_dig_job_never_takes_a_body_off_a_standing_job() {
    let (mut game, staff) = base_with_a_crew(3268, 1);
    let mine = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    game.set_standing_job(mine, true, false).unwrap();
    mark(&mut game, WALL);

    game.tick();

    assert_eq!(
        posted_at(&game, staff[0]),
        Some(mine),
        "a standing job is the player saying keep this running, and outranks a plan"
    );
}

/// `entity_label`'s fall-through is `"You"`, so a dig site with no arm of
/// its own makes every screen that names a post report a digger as the
/// player standing at their own.
#[test]
fn a_posted_digger_is_named_by_the_cell_it_is_cutting() {
    let (mut game, staff) = base_with_a_crew(3269, 1);
    fragments_for(&mut game, 1);
    mark(&mut game, WALL);
    game.tick();
    assert!(
        posted_at(&game, staff[0]).is_some(),
        "precondition: the body must have taken the dig job"
    );

    let activity = game.program_activity(staff[0]);

    assert!(
        activity.contains("Marked Wall (") && activity.contains(&format!("{}, {}", WALL.0, WALL.1)),
        "a digger's post must name the cell it is cutting, got: {activity}"
    );
    assert!(
        !activity.contains("You"),
        "and must not fall through to the player, got: {activity}"
    );
}

/// Cancelling a plan has to stop the crew, and "the site is gone" is not
/// what cancelling looks like: an unmarked site is deliberately *kept*
/// while it still holds chip progress. `dig_wants` drops it, but
/// `schedule_base_labour` never takes a body off a post it has nowhere
/// better to send — so without a check here the digger cuts a wall the
/// player already told it to leave.
#[test]
fn a_digger_drops_a_post_whose_mark_was_cleared() {
    let target = WALL;
    let mut game = game_at_the_frontier(3240);
    let player = game.player_entity();
    game.toggle_mark_box(target, target, None);
    // One swing, so the site holds progress and survives the unmarking.
    game.strike_field_rock(player, target.0, target.1);
    let site = game
        .dig_site_at(target.0, target.1)
        .expect("a marked, struck wall has a dig site");
    let chipped = game.world.get::<Durability>(site).unwrap().hp;

    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: TEST_POCKET_HALF_WIDTH,
            y: 0,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    game.toggle_mark_box(target, target, None);

    for _ in 0..50 {
        game.run_dig_crew();
    }

    assert!(
        game.world.get::<Task>(digger).is_none(),
        "the digger kept its post on a cell the player unmarked"
    );
    assert_eq!(
        game.world.get::<Durability>(site).map(|d| d.hp),
        Some(chipped),
        "the crew went on cutting a cancelled job"
    );
}

/// An unmarked site earns its keep by holding chip progress, and a spent
/// meter on a solid cell holds none. Nothing in play reverts a cell to solid
/// any more, but a stray site in that shape — left over from an older save,
/// or reconstructed here by hand — must not linger forever once unmarked.
/// Keeping one leaves an invisible entity that is drawn nowhere, wanted by
/// nobody, and written to every save from then on.
#[test]
fn clearing_a_mark_leaves_no_site_behind_on_a_reverted_cell() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3241, cut);
    // A mined cell's site starts at zero: there is nothing left to cut.
    game.toggle_mark_box(cut, cut, None);
    assert!(
        game.dig_site_at(cut.0, cut.1).is_some(),
        "marking a mined cell spawns a site to floor"
    );

    // The blocker put back by hand: nothing in play does it, but the shape
    // it leaves behind — a spent meter on a solid cell — still has to be
    // handled gracefully if it is ever seen again.
    set_rock(&mut game, cut);
    assert!(
        cell(&game, cut) == Cell::Solid,
        "precondition: the cell must be solid rock again"
    );
    game.toggle_mark_box(cut, cut, None);

    assert!(
        game.dig_site_at(cut.0, cut.1).is_none(),
        "an unmarked site with a spent meter on solid rock outlived its mark"
    );
}

/// What a program brings to a wall is its own species' band, not the
/// player's fists. `natural_range_of` is the one derivation — a second
/// reading of a species' first move here is the drift `swing_damage` exists
/// to prevent.
#[test]
fn a_crew_program_swings_its_own_species_band_at_rock() {
    let mut game = game_at_the_frontier(3242);
    let worker = spawn_tamed(&mut game, 30, 3);
    // Scrapper's first move is power 8, spread 2 — a mean of 8 against
    // `PLAYER_UNARMED_DAMAGE`'s 5, so reading the player's band is visible.
    game.world
        .get_mut::<components::Creature>(worker)
        .expect("a tamed program is a creature")
        .species = "scrapper".to_string();

    let atk = game.effective_atk(worker);
    assert_eq!(
        game.swing_damage(worker),
        (8 + atk).max(1) as u32,
        "a crew program swung the player's unarmed band instead of its own"
    );
}

/// Every mining cue queued since the last drain, by cell.
///
/// Filtered rather than compared whole: a swing spends a tick, and a tick
/// is free to queue a raid's own effects alongside. The assertion is about
/// this cue, not about the queue.
fn mining_cues(game: &mut Game) -> Vec<(i32, i32)> {
    game.take_effects()
        .into_iter()
        .filter(|e| e.kind == EffectKind::Mine)
        .map(|e| e.pos)
        .collect()
}

/// Mining is the one base verb with nothing of its own to animate —
/// the cell wears a progress bar and no glyph moves — so the cue is the
/// whole of the feedback that a swing landed.
///
/// Queued from `Game::swing_at_rock` and not from either of its callers,
/// which is what makes the player's bump and a posted digger's cycle sound
/// alike: the two share the one place rock takes damage.
#[test]
fn a_swing_at_rock_queues_a_mining_cue() {
    let mut game = game_at_the_frontier_cutting(3244);
    game.take_effects();

    game.move_player(1, 0);

    assert_eq!(
        mining_cues(&mut game),
        vec![WALL],
        "the player's bump into rock queued no mining cue"
    );
}

/// The negative half, and what keeps the cue keyed to a swing rather than to
/// a keypress: a disarmed bump is refused for exactly as slice 1 refused it
/// — no tick, nothing damaged — so it has to be silent too.
#[test]
fn a_bump_that_does_not_cut_is_silent() {
    let mut game = game_at_the_frontier(3245);
    game.take_effects();

    game.move_player(1, 0);

    assert!(
        mining_cues(&mut game).is_empty(),
        "a bump that cut nothing sounded like a swing"
    );
}

/// Swinger-blind, which is the whole reason the cue sits in `swing_at_rock`
/// rather than in `bump_rock`: a crew cutting is the base's own sound,
/// and nothing about it is keyed to the player standing there to hear it.
#[test]
fn a_crew_swing_queues_the_same_cue() {
    let mut game = game_at_the_frontier(3246);
    let worker = spawn_tamed(&mut game, 30, 3);
    game.take_effects();

    game.strike_field_rock(worker, WALL.0, WALL.1);

    assert_eq!(
        mining_cues(&mut game),
        vec![WALL],
        "a crew program's swing queued no mining cue"
    );
}

/// A game with one program posted on a dig job at `at`, and the program.
///
/// Shaped after `a_digger_drops_a_post_whose_mark_was_cleared`: the site is
/// spawned by a first swing, because `swing_at_rock` creates it lazily rather
/// than marking doing it.
fn a_posted_digger(seed: u32, at: (i32, i32)) -> (Game, Entity) {
    let mut game = game_at_the_frontier(seed);
    let player = game.player_entity();
    game.toggle_mark_box(at, at, None);
    game.strike_field_rock(player, at.0, at.1);
    let site = game
        .dig_site_at(at.0, at.1)
        .expect("a marked, struck wall has a dig site");
    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: at.0 - 1,
            y: at.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    (game, digger)
}

/// A crew cycle is `BASE_DIG_TICKS_PER_SWING` ticks long, so a cue on the
/// swing alone is one chip every six real seconds while the cell's own
/// progress bar moves every tick.
///
/// Both sides cue on **a tick of cutting** instead. The player's bump spends
/// one tick and lands a swing; a crew spends twelve ticks for the same
/// swing. That difference is the whole of why a player hammering sounds
/// faster than a crew working, and it is the simulation's difference rather
/// than a presentation one.
#[test]
fn a_crew_cues_every_tick_it_cuts_and_not_just_when_the_swing_lands() {
    let ticks = crate::tuning::BASE_DIG_TICKS_PER_SWING;
    let (mut game, _digger) = a_posted_digger(3248, WALL);
    game.take_effects();

    for _ in 0..ticks {
        game.run_dig_crew();
    }

    let cues = mining_cues(&mut game);
    assert_eq!(
        cues.len(),
        ticks as usize,
        "a {ticks}-tick cycle sounded {} time(s)",
        cues.len()
    );
    assert!(
        cues.iter().all(|&pos| pos == WALL),
        "a cue landed off the cell being cut: {cues:?}"
    );
}

/// Exactly one cue on the tick the swing lands, and not two. That tick is
/// cued by `swing_at_rock` rather than by the progress counter — every other
/// tick of the cycle is the other way round — so the seam between the two
/// push sites is the one place a tick could double up.
#[test]
fn the_tick_a_crew_swing_lands_cues_once() {
    let ticks = crate::tuning::BASE_DIG_TICKS_PER_SWING;
    let (mut game, _digger) = a_posted_digger(3249, WALL);
    for _ in 0..ticks - 1 {
        game.run_dig_crew();
    }
    game.take_effects();

    game.run_dig_crew();

    assert_eq!(
        mining_cues(&mut game),
        vec![WALL],
        "the landing tick cued twice — once for the progress tick and once \
         for the swing"
    );
}

/// Flooring is the other half of the one dig verb, and it is not mining. A
/// crew laying substrate over a cut cell has to be silent, or the cue means
/// "the crew is busy" rather than "rock is being cut".
#[test]
fn a_crew_laying_floor_is_silent() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3250, cut);
    game.toggle_mark_box(cut, cut, None);
    let site = game
        .dig_site_at(cut.0, cut.1)
        .expect("marking an open cell spawns a site to floor");
    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: cut.0 - 1,
            y: cut.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    game.take_effects();

    for _ in 0..crate::tuning::BASE_DIG_TICKS_PER_SWING * 2 {
        game.run_dig_crew();
    }

    assert!(
        mining_cues(&mut game).is_empty(),
        "laying floor sounded like cutting rock"
    );
}

/// A swing that breaks through is still one swing and still one cue. The
/// break has its own base-news line, and a second clip on top of the last
/// swing of a wall would be the only double cue in the game.
#[test]
fn breaking_through_queues_one_cue_and_not_two() {
    let mut game = game_at_the_frontier_cutting(3247);
    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);
    for _ in 0..swings - 1 {
        game.strike_field_rock(player, WALL.0, WALL.1);
    }
    assert!(
        cell(&game, WALL) == Cell::Solid,
        "precondition: the wall must still be standing before the last swing"
    );
    game.take_effects();

    game.strike_field_rock(player, WALL.0, WALL.1);

    assert_eq!(
        mining_cues(&mut game),
        vec![WALL],
        "the swing that opened the cell queued a second cue"
    );
}

/// A refusal is news to the player standing there, not to the base's own
/// record of what it did. `lay_tile`'s other two refusals are reported by
/// the `Err` alone, and this one wrote the base log as well — so every
/// press of `v` with an empty pack left a permanent line behind.
#[test]
fn a_refused_tile_is_reported_once() {
    let mut game = game_on_open_rock(3243, 0);

    let refusal = game.lay_tile().expect_err("no substrate means no tile");

    assert!(
        !game
            .message_log(40)
            .into_iter()
            .any(|entry| entry.text == refusal),
        "the refusal was written to the base log as well as returned: {refusal}"
    );
}

/// Most dig sites are not marked: one is left behind by every wall the
/// player bumps into and walks away from. Calling those "Marked" is a claim
/// about a plan they appear in nowhere.
#[test]
fn an_unmarked_half_cut_wall_is_not_called_marked() {
    let mut game = game_at_the_frontier(3244);
    let player = game.player_entity();
    game.strike_field_rock(player, WALL.0, WALL.1);
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("a struck wall has a dig site");

    let label = game.entity_label(site);

    assert!(
        label.starts_with("Chipped Wall"),
        "an unmarked, half-cut wall is named as though it were in a plan: {label}"
    );
}

/// A post is reachable if *any* of its four faces is, not if the nearest one
/// is. Dig sites are the first target whose faces routinely sit in different
/// parts of the base — a marked cell on a rock spur has a corridor on one
/// side and an unreached pocket on the other — and picking one face up front
/// meant a tie broken the wrong way latched `announced_stuck` and skipped the
/// site for the rest of the run.
#[test]
fn a_post_is_reachable_through_a_face_that_is_not_the_nearest() {
    use crate::game::base::hauling::post_reach;

    let mut grid = base_grid::BaseGrid::default();
    // The target sits at the origin, solid. Its west face is an isolated
    // pocket; its east face joins the corridor the worker is standing in.
    // Both faces are the same Chebyshev distance from the worker, so
    // `station_tile`'s `(distance, x, y)` order takes the western — the
    // unreachable one — first.
    for (x, y) in [(-1, 0), (1, 0), (1, 1), (0, 2)] {
        grid.lay_floor(x, y);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: 0, y: 2 };
    let blocked = crate::game::base::hauling::Occupancy::default();

    assert_eq!(
        post_reach(&grid, from, target, 1, &blocked, grid.radius()),
        Ok(()),
        "a post with one reachable face was reported unreachable"
    );
}

/// The other half: a target with no standable face at all is still
/// `BoxedIn`, and one whose every face is walled off from the worker is
/// still `NoRoute`. Widening the search must not collapse the two answers,
/// which leave the player different errands.
#[test]
fn a_post_with_no_reachable_face_is_still_refused() {
    use crate::game::base::hauling::{NoPost, post_reach};

    let mut grid = base_grid::BaseGrid::default();
    for (x, y) in [(-1, 0), (1, 0), (0, 2)] {
        grid.lay_floor(x, y);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: 0, y: 2 };
    let blocked = crate::game::base::hauling::Occupancy::default();

    assert_eq!(
        post_reach(&grid, from, target, 1, &blocked, grid.radius()),
        Err(NoPost::NoRoute),
        "both faces are cut off from the worker, so this is a route problem"
    );

    let mut boxed = base_grid::BaseGrid::default();
    boxed.lay_floor(from.x, from.y);
    assert_eq!(
        post_reach(&boxed, from, target, 1, &blocked, boxed.radius()),
        Err(NoPost::BoxedIn),
        "nothing can stand beside it at all, which is a digging problem"
    );
}

/// **A body is squeezed past but never stopped on.** The target's west face
/// is the only way to its north face; with a program standing on the west
/// face the walk may not pass through it, because arriving there would put
/// two bodies on one cell — while the same program one cell short of the
/// face is a throughway, and the step onto it says it is a squeeze.
#[test]
fn a_body_on_a_face_is_not_a_throughway_and_a_body_short_of_it_is() {
    use crate::game::base::hauling::{NoPost, Step, blocked_tiles, step_to_post};

    let mut grid = base_grid::BaseGrid::default();
    for (x, y) in [(-1, 2), (-1, 1), (-1, 0), (0, -1)] {
        grid.lay_floor(x, y);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: -1, y: 2 };
    let on_face = blocked_tiles(std::iter::empty(), [Position { x: -1, y: 0 }].into_iter());
    let short_of_it = blocked_tiles(std::iter::empty(), [Position { x: -1, y: 1 }].into_iter());

    assert_eq!(
        step_to_post(&grid, from, target, 1, &on_face, grid.radius()),
        Err(NoPost::NoRoute),
        "the only way on runs through a face somebody is standing on"
    );
    assert_eq!(
        step_to_post(&grid, from, target, 1, &short_of_it, grid.radius()),
        Ok(Some(Step {
            to: Position { x: -1, y: 1 },
            squeeze: true,
        })),
        "a body in the corridor is a throughway, priced as a squeeze"
    );
}

/// **The crew's one-field answer agrees with the posted walk's.** `crew_reach`
/// builds its field with no target in mind, so a body on one of a target's
/// faces reads as a throughway there; `reaches` is what puts the arrival
/// rule back, and the two geometries of
/// `a_body_on_a_face_is_not_a_throughway_and_a_body_short_of_it_is` must
/// come out as `post_reach` says.
#[test]
fn the_crew_field_and_the_posted_walk_agree_about_a_body_on_a_face() {
    use crate::game::base::hauling::{blocked_tiles, crew_reach, post_reach, reaches};

    let mut grid = base_grid::BaseGrid::default();
    for (x, y) in [(-1, 2), (-1, 1), (-1, 0), (0, -1)] {
        grid.lay_floor(x, y);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: -1, y: 2 };
    for (body, reachable) in [((-1, 0), false), ((-1, 1), true)] {
        let blocked = blocked_tiles(
            std::iter::empty(),
            [Position {
                x: body.0,
                y: body.1,
            }]
            .into_iter(),
        );
        let field = crew_reach(&grid, from, &blocked, grid.radius());
        assert_eq!(
            post_reach(&grid, from, target, 1, &blocked, grid.radius()).is_ok(),
            reachable,
            "precondition: the posted walk's answer for a body at {body:?}"
        );
        assert_eq!(
            reaches(&grid, &field, from, target, 1, &blocked, grid.radius()),
            reachable,
            "the crew field disagrees with the posted walk for a body at {body:?}"
        );
    }
}

/// The interior of a marked block must not spend the crew's budget.
///
/// `dig_wants` lists every marked cell in tile order and
/// `schedule_base_labour` truncates that list to the number of bodies —
/// **before** anything asks whether a body could be sent. A block marked
/// out in open rock is boxed in everywhere but its face, so the sites that
/// sort first are the ones nothing can stand beside, and they are refused
/// *silently*. Ahead of the fix the whole dig budget went to them and the
/// one cell a body could actually have cut was cut off the end of the list.
#[test]
fn a_boxed_in_mark_does_not_starve_a_reachable_one() {
    let (mut game, staff) = base_with_a_crew(3264, 1);
    // Exactly one tile's worth, so the budget is starved too if a cell with
    // no face to stand at is allowed to claim one.
    fragments_for(&mut game, 1);
    // Three cells in open rock, far enough out that nothing walkable
    // touches any of them, and every one of them sorts before `WALL`.
    let buried = [
        (-TEST_POCKET_HALF_WIDTH - 3, -1),
        (-TEST_POCKET_HALF_WIDTH - 3, 0),
        (-TEST_POCKET_HALF_WIDTH - 3, 1),
    ];
    for cell in buried {
        mark_unchecked(&mut game, cell);
    }
    mark(&mut game, WALL);
    assert!(
        game.marked_cells().first().map(|m| m.pos) != Some(WALL),
        "precondition: the buried cells must sort ahead of the reachable one"
    );

    game.tick();

    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("marking a wall spawns its dig site");
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(site),
        "the one body must dig the one cell it can reach"
    );
}

/// The other half of the same starvation, and the half `has_station` cannot
/// answer: a marked cell with a perfectly good face that **no body can walk
/// to**. A sealed-off pocket, or a plan drawn beyond
/// `haul_walk_radius`, leaves cells that pass the face test and fail the
/// walk — and the walk used to be asked *below* the truncation, so those
/// cells took the whole budget and the one reachable cell was cut off the
/// end of the list. The crew stood idle with a plan on the wall.
#[test]
fn an_unroutable_mark_does_not_starve_a_reachable_one() {
    let (mut game, staff) = base_with_a_crew(3265, 4);
    // Exactly one tile's worth: the sealed cells must not claim the
    // substrate on their way to being dropped either, which is why the
    // budget is settled *after* this drop and not inside `dig_wants`.
    fragments_for(&mut game, 1);
    // A sealed cell of open rock, walled in on every side: it gives its four
    // neighbours a face nothing can route to.
    let sealed = (-TEST_POCKET_HALF_WIDTH - 4, 0);
    excavate(&mut game, (sealed.0, sealed.1));
    for cell in [
        (sealed.0 - 1, sealed.1),
        (sealed.0 + 1, sealed.1),
        (sealed.0, sealed.1 - 1),
        (sealed.0, sealed.1 + 1),
    ] {
        mark_unchecked(&mut game, cell);
    }
    mark(&mut game, WALL);
    assert!(
        game.marked_cells().first().map(|m| m.pos) != Some(WALL),
        "precondition: the sealed cells must sort ahead of the reachable one"
    );

    game.tick();

    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("marking a wall spawns its dig site");
    let digging: Vec<Entity> = staff
        .iter()
        .copied()
        .filter(|&w| posted_at(&game, w) == Some(site))
        .collect();
    assert_eq!(
        digging.len(),
        1,
        "the one cell the crew can reach must have exactly one body on it, \
         and the four it cannot must have none"
    );
}

/// A helper for the hardness tests: how many swings `swinger` needs to open
/// the cell at `(x, y)`, driven through the real `strike_field_rock`.
fn swings_to_open(game: &mut Game, swinger: Entity, x: i32, y: i32) -> u32 {
    for n in 1..500 {
        game.strike_field_rock(swinger, x, y);
        if !is_solid(game, (x, y)) {
            return n;
        }
    }
    panic!("the cell at ({x}, {y}) never opened");
}

/// The blocker biomes, each of which `RockDb::kind_at` maps to a rock kind.
const BLOCKER_BIOMES: [Biome; 3] = [Biome::Entropy, Biome::DataVoid, Biome::BlackIce];

/// Forces a blocker of rock kind `kind` onto a field cell away from any
/// base, and returns it, so a test can name a kind rather than a coordinate.
/// A kind is the blocker's biome's (`RockDb::kind_at`), so the cell is
/// overridden to the biome that maps to it — one cell per biome, so two
/// kinds asked of one game never share a cell.
fn rock_cell_of(game: &mut Game, kind: &str) -> (i32, i32) {
    let (i, biome) = BLOCKER_BIOMES
        .iter()
        .enumerate()
        .find(|(_, b)| game.world.resource::<crate::rock::RockDb>().kind_at(**b).id == kind)
        .unwrap_or_else(|| panic!("no blocker biome maps to rock kind {kind}"));
    let cell = (FAR_FIELD.0, FAR_FIELD.1 + 2 * i as i32);
    set_blocker(game, cell, *biome);
    cell
}

/// **The reported bug.** A developed player walked into a wall and the wall
/// was gone — `swing_damage` grows all run while `BASE_ROCK_DURABILITY` was
/// one flat number, so past a certain level every cell fell to one bump.
///
/// Asserted over *every* shipped kind, because the fix is a per-kind floor
/// and a kind authored with `min_swings: 1` would reopen the hole for
/// itself alone.
#[test]
fn no_shipped_kind_falls_to_one_swing_however_hard_you_hit() {
    let mut game = game(7731);
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.atk = 5_000;
    }
    let kinds: Vec<(String, u32)> = game
        .world
        .resource::<crate::rock::RockDb>()
        .defs()
        .map(|d| (d.id.clone(), d.min_swings))
        .collect();
    for (kind, floor) in kinds {
        let (x, y) = rock_cell_of(&mut game, &kind);
        let swings = swings_to_open(&mut game, player, x, y);
        assert_eq!(
            swings, floor,
            "{kind} fell in {swings} swings to a 5000-atk player, floor is {floor}"
        );
    }
}

/// The other half, and the one that says the floor bit *where the bug was*
/// and nowhere else: a fresh player's dig rate on ordinary rock has not
/// moved. At 24 durability and a floor of 2 the cap is 12, and a level-1
/// swing of ~11 is under it, so nothing about the opening game changes.
///
/// Both halves in one file on purpose — the first alone passes against a
/// game that simply made all rock take four swings, which would be a
/// different and worse change.
#[test]
fn a_fresh_players_dig_rate_on_ordinary_rock_is_unchanged() {
    let mut game = game(7732);
    let player = game.player_entity();
    let per_swing = game.swing_damage(player);
    let expected = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(per_swing);
    assert!(
        expected > crate::tuning::BASE_ROCK_MIN_SWINGS,
        "the fixture must swing under the cap for this to say anything: \
         {per_swing} a swing opens ordinary rock in {expected}"
    );
    let (x, y) = rock_cell_of(&mut game, "entropy");
    assert_eq!(swings_to_open(&mut game, player, x, y), expected);
}

/// A denser kind is actually denser through the real swing path, not just in
/// the file.
#[test]
fn a_denser_kind_takes_more_swings_than_ordinary_rock() {
    let mut game = game(7733);
    let player = game.player_entity();
    let ordinary = rock_cell_of(&mut game, "entropy");
    let fused = rock_cell_of(&mut game, "fused");
    let a = swings_to_open(&mut game, player, ordinary.0, ordinary.1);
    let b = swings_to_open(&mut game, player, fused.0, fused.1);
    assert!(b > a, "fused took {b} swings, ordinary took {a}");
}

/// A half-cut wall of a denser kind comes back with that kind's ceiling, not
/// the fallback's. The load path re-derives from the blocker's saved field
/// override, so nothing new is written to the save — and clamping against
/// the flat constant instead would silently cap a Fused wall at 24.
#[test]
fn a_half_cut_dense_wall_reloads_with_its_own_ceiling() {
    let mut game = game(7734);
    let player = game.player_entity();
    let (x, y) = rock_cell_of(&mut game, "fused");
    game.strike_field_rock(player, x, y);
    let before = {
        let site = game.dig_site_at(x, y).expect("a struck wall has a site");
        let d = game.world.get::<Durability>(site).unwrap();
        (d.hp, d.max_hp)
    };
    assert!(
        before.1 > crate::tuning::BASE_ROCK_DURABILITY,
        "the fixture must pick a kind harder than the fallback"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_dense_wall_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let site = loaded
        .dig_site_at(x, y)
        .expect("the site survives the save");
    let d = loaded.world.get::<Durability>(site).unwrap();
    assert_eq!(
        (d.hp, d.max_hp),
        before,
        "a dense wall reloaded at the wrong ceiling"
    );
}

/// Mining is off when a run starts — the bump has to be armed before it
/// destroys terrain.
#[test]
fn mining_is_off_when_a_run_starts() {
    assert!(!game(8801).mining());
}

/// **The reported bug's other half.** With mining disarmed a step into rock
/// is refused for free: no damage, no dig site, and — the part that makes it
/// a refusal rather than a wasted turn — no tick.
///
/// All three asserted together on purpose. Damage alone passes against a
/// gate that swallowed the swing but still spent the turn, which would be a
/// silent tax on walking into your own walls.
#[test]
fn with_mining_off_a_step_into_rock_costs_nothing_at_all() {
    let mut game = game_at_the_frontier(8802);
    assert!(!game.mining());
    let before_tick = game.world.resource::<GameClock>().tick;
    let before_sites = dig_site_count(&mut game);

    game.move_player(1, 0);

    assert!(
        cell(&game, WALL) == Cell::Solid,
        "a disarmed bump cut the wall"
    );
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        before_tick,
        "a refusal spent a turn"
    );
    assert_eq!(
        dig_site_count(&mut game),
        before_sites,
        "a refusal left a dig site behind"
    );
}

/// Armed, the bump is the swing it has always been — the toggle adds a gate
/// and changes nothing behind it.
#[test]
fn with_mining_on_a_step_into_rock_swings_exactly_as_before() {
    let mut game = game_at_the_frontier(8803);
    game.toggle_mining();
    assert!(game.mining());
    let player = game.player_entity();
    let swings = swings_for(&game, player, WALL);

    for _ in 0..swings {
        game.move_player(1, 0);
    }
    assert!(
        cell(&game, WALL) == Cell::Excavated,
        "an armed bump did not cut the wall"
    );
}

/// The toggle survives a save, and a save written before it existed loads
/// with mining off.
#[test]
fn mining_mode_survives_a_real_save_and_load() {
    let mut game = game(8804);
    game.toggle_mining();
    assert!(game.mining(), "the fixture must arm it to be a real test");

    let path = std::env::temp_dir().join(format!(
        "feral_processes_mining_mode_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(loaded.mining(), "the toggle did not survive the save");
}

/// **The separation the toggle depends on.** A mark is an instruction the
/// base was already given; putting your own tools away says nothing about
/// it. `run_dig_crew` must never read `MiningMode`, or disarming the bump
/// silently stalls every dig job in the base and reads as the crew being
/// broken.
#[test]
fn a_posted_crew_cuts_a_marked_cell_with_the_players_mining_off() {
    let (mut game, staff) = base_with_a_crew(8805, 1);
    assert!(!game.mining(), "the fixture must leave mining disarmed");
    fragments_for(&mut game, 1);
    mark(&mut game, WALL);
    let ticks = ticks_to_cut(&game, staff[0]);
    pass(&mut game, ticks);
    assert!(
        cell(&game, WALL) != Cell::Solid,
        "the crew stopped digging because the player put their own tools away"
    );
}

/// Arming the bump is not an action. `handle_playing_key` `return`s on `n`
/// rather than falling through to the tick, and this is the engine half of
/// that promise.
#[test]
fn arming_the_bump_spends_no_turn() {
    let mut game = game(8806);
    let before = game.world.resource::<GameClock>().tick;
    game.toggle_mining();
    game.toggle_mining();
    assert_eq!(game.world.resource::<GameClock>().tick, before);
}

/// The rock shade the map draws at field `cell`, or `None` for a cell drawn
/// anonymous — read through `view_tiles_at` centred on the cell itself.
fn shade_at(game: &mut Game, cell: (i32, i32)) -> Option<f32> {
    game.view_tiles_at(cell, 0, 0)[0][0].rock_shade
}

/// A face is a blocker with laid floor orthogonally against it. Diagonal
/// does not count — a diagonal neighbour does not expose a face — and
/// neither does walkable ground itself.
#[test]
fn a_face_is_a_blocker_with_floor_orthogonally_beside_it() {
    let mut game = game_at_the_frontier(9201);
    // The pocket's corners are cut, so this cell touches floor at a corner
    // and nowhere else: `(9, 2)` is floor, `(9, 3)` and `(10, 2)` are not.
    let diagonal = (TEST_POCKET_HALF_WIDTH + 1, 3);
    set_rock(&mut game, diagonal);
    assert!(
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .all(|(dx, dy)| !game
                .world
                .resource::<base_grid::BaseGrid>()
                .is_floor(diagonal.0 + dx, diagonal.1 + dy)),
        "precondition: the diagonal cell must touch floor at a corner only"
    );

    assert!(
        shade_at(&mut game, WALL).is_some(),
        "the frontier wall is a face"
    );
    assert!(
        shade_at(&mut game, (WALL.0 + 1, WALL.1)).is_none(),
        "rock behind a face is not itself a face"
    );
    assert!(
        shade_at(&mut game, TEST_HOME_CELL).is_none(),
        "floor is not a face — the predicate must require a blocker"
    );
    assert!(
        shade_at(&mut game, diagonal).is_none(),
        "exposure must be decided orthogonally, never diagonally"
    );
}

/// Flooring a cell lights up its neighbours, and reverting it (via
/// `BaseGrid::revert`, a fixture-only operation now) puts them out again.
/// The reason exposure is derived per lookup rather than cached: more than
/// one verb moves it. Mining alone does not — a cut cell is ground, not
/// floor, and the base digs outward from its floor (`minable`).
#[test]
fn flooring_a_cell_exposes_its_neighbours_and_reverting_it_un_exposes_them() {
    let mut game = game_at_the_frontier_cutting(9202);
    let beyond = (WALL.0 + 1, WALL.1);
    assert!(
        shade_at(&mut game, beyond).is_none(),
        "the cell behind the wall must start buried"
    );

    let player = game.player_entity();
    for _ in 0..swings_for(&game, player, WALL) {
        game.move_player(1, 0);
    }
    assert_eq!(cell(&game, WALL), Cell::Excavated, "precondition: cut");
    assert!(
        shade_at(&mut game, beyond).is_none(),
        "a cut but unfloored cell exposed what was behind it"
    );

    game.dev_lay_floor([WALL]);
    assert!(
        shade_at(&mut game, beyond).is_some(),
        "flooring a cell did not expose what was behind it"
    );

    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .revert(WALL.0, WALL.1);
    assert!(
        shade_at(&mut game, beyond).is_none(),
        "reverting a cell left its neighbour still lit"
    );
}

/// **The trap the visibility rule exists to avoid.** If the map hides a
/// kind while `x` names it, the hiding is decorative and prospecting is dead
/// on arrival. Asserted as an agreement between the two readers rather than
/// against a hardcoded string, so they cannot drift apart.
#[test]
fn the_map_and_the_examine_ray_agree_about_a_wall() {
    let mut game = game_at_the_frontier(9203);
    let named = game
        .describe_base_rock(1, 0, crate::tuning::EXAMINE_RANGE_TILES)
        .expect("the ray must find the frontier wall");

    let seen = shade_at(&mut game, WALL);
    assert!(
        seen.is_some(),
        "the ray named a wall the map drew as anonymous rock"
    );

    let biome = game.world.resource::<WorldMap>().peek(WALL.0, WALL.1).biome;
    let kind = game
        .world
        .resource::<crate::rock::RockDb>()
        .kind_at(biome)
        .clone();
    assert!(
        named.starts_with(&kind.name),
        "{named:?} is not {}",
        kind.name
    );
    assert_eq!(seen, Some(kind.shade));
}

/// Deep rock is anonymous on the map, and that is the whole mechanic: a
/// coloured wall three cells into solid ground would be an arrow rather than
/// something you found.
#[test]
fn rock_behind_a_face_is_drawn_anonymous() {
    let mut game = game_at_the_frontier(9204);
    let deep = (WALL.0 + 2, WALL.1);
    assert!(is_solid(&game, deep), "the fixture must name buried rock");
    assert_eq!(
        shade_at(&mut game, deep),
        None,
        "buried rock advertised its kind"
    );
}

/// **Visibility is a display filter and never a gameplay one.** A swing at
/// rock the player cannot see meets that kind's real durability and real
/// floor — finding that out the hard way is the point. The regression to
/// head off is a later "fix" that resolves unseen rock to the default kind
/// to make the two halves agree.
#[test]
fn a_swing_at_unseen_rock_meets_its_real_kind() {
    let mut game = game(9205);
    let hard = game
        .world
        .resource::<crate::rock::RockDb>()
        .defs()
        .max_by_key(|d| d.durability)
        .expect("the shipped roster is not empty")
        .clone();
    // A buried cell of the hardest kind: unseen on the map, and still that
    // kind to anything that hits it.
    let buried = rock_cell_of(&mut game, &hard.id);
    assert_eq!(shade_at(&mut game, buried), None, "precondition: unseen");
    assert_eq!(game.wall_at(buried.0, buried.1).durability, hard.durability);
}

/// A posted digger is on the map too, and for the same reason a builder is:
/// a `DigSite` carries no `Glyph`, so the body is the only end of the
/// posting there is to draw.
///
/// `position_is_honest` read `TaskKind::GatherResource` alone, so a digger
/// was drawn nowhere for the whole cut — the player marked a wall and the
/// program that went to cut it disappeared.
#[test]
fn a_posted_digger_is_drawn_on_the_map() {
    let target = WALL;
    let mut game = game_at_the_frontier(3260);
    game.toggle_mark_box(target, target, None);
    let site = game
        .dig_site_at(target.0, target.1)
        .expect("a marked wall has a dig site");

    let digger = spawn_tamed_on_map(&mut game, TEST_POCKET_HALF_WIDTH, 0);
    game.world.entity_mut(digger).insert(Task {
        kind: TaskKind::Excavate,
        target: site,
        progress: 0,
        required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
    });

    let view = game
        .view_entities(40, 40)
        .into_iter()
        .find(|v| v.entity == digger)
        .expect("the digger is standing beside the wall");
    assert!(
        views::drawn_on_surface_map(view.is_tamed, view.position_is_honest),
        "a posted digger is drawn wherever it is standing"
    );
    assert!(
        view.wears_job_mark,
        "and wears the mark itself — a dig site has no glyph to wear it"
    );
}

// ---------------------------------------------------------------------------
// Floor finishes: the crew lays and strips finishes
// ---------------------------------------------------------------------------

/// A game with one program posted on a finish job at `at`: `at` is laid
/// floor, marked with `order`, and the worker stands one tile west of it —
/// `a_posted_digger`'s own shape, but over floor rather than rock, since a
/// finish or strip mark never has rock for a swing to spawn lazily.
fn a_posted_finisher(seed: u32, at: (i32, i32), order: FinishOrder) -> (Game, Entity) {
    let mut game = game_at_the_frontier(seed);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(at.0, at.1);
    game.toggle_mark_box(at, at, Some(&order));
    let site = game
        .dig_site_at(at.0, at.1)
        .expect("a marked floor cell has a dig site");
    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: at.0 - 1,
            y: at.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    (game, digger)
}

/// Runs the crew for exactly one work cycle — long enough for an
/// already-posted, already-adjacent worker to land its job once.
fn run_to_landing(game: &mut Game) {
    for _ in 0..crate::tuning::BASE_DIG_TICKS_PER_SWING {
        game.run_dig_crew();
    }
}

/// A base shelf holding `qty` Blank Substrate, without a real Depot —
/// `tests::base_ledger::what_leaves_the_shelves_is_counted_as_consumed`'s
/// own pattern, reused here to test the base half of `Game::spend_substrate`.
fn give_base(game: &mut Game, qty: u32) -> Entity {
    let shelf = game
        .world
        .spawn((
            Structure {
                kind: "depot".to_string(),
            },
            Position { x: 1, y: 1 },
            crate::components::Stock::default(),
        ))
        .id();
    game.world
        .get_mut::<crate::components::Stock>(shelf)
        .unwrap()
        .output
        .insert(ItemId::from(ids::BLANK_SUBSTRATE), qty);
    shelf
}

fn base_substrate(game: &Game, shelf: Entity) -> u32 {
    game.world
        .get::<crate::components::Stock>(shelf)
        .unwrap()
        .output
        .get(&ItemId::from(ids::BLANK_SUBSTRATE))
        .copied()
        .unwrap_or(0)
}

#[test]
fn crew_applies_a_finish_spending_exactly_the_cost() {
    let finish = FloorId::from("cobalt_carpet");
    let (mut game, _digger) = a_posted_finisher(3270, WALL, FinishOrder::Apply(finish.clone()));
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    let before = count_item(&game, ids::BLANK_SUBSTRATE);

    run_to_landing(&mut game);

    assert_eq!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .finish_at(WALL.0, WALL.1),
        Some(&finish),
        "the cell must wear the finish it was marked for"
    );
    assert_eq!(
        before - count_item(&game, ids::BLANK_SUBSTRATE),
        crate::tuning::FLOOR_FINISH_COST,
        "the crew must spend exactly the finish's cost, no more and no less"
    );
    assert!(
        game.dig_site_at(WALL.0, WALL.1).is_none(),
        "a finished site is despawned"
    );
}

/// With fewer than the cost anywhere in reach, the full pipeline (not a
/// hand-posted worker) must never assign anybody to the job, and nothing may
/// move against it.
#[test]
fn an_apply_site_below_cost_spends_nothing_and_is_not_a_want() {
    let (mut game, staff) = base_with_a_crew(3271, 1);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(WALL.0, WALL.1);
    let apply = FinishOrder::Apply(FloorId::from("cobalt_carpet"));
    game.toggle_mark_box(WALL, WALL, Some(&apply));
    give(
        &mut game,
        &ItemId::from(ids::BLANK_SUBSTRATE),
        crate::tuning::FLOOR_FINISH_COST - 1,
    );
    let before = count_item(&game, ids::BLANK_SUBSTRATE);

    pass(&mut game, 80);

    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        before,
        "an unaffordable finish job must never be handed a body to spend against"
    );
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .finish_at(WALL.0, WALL.1)
            .is_none(),
        "the finish must never be applied"
    );
    assert_eq!(
        posted_at(&game, staff[0]),
        None,
        "the dry site is not a want, so the one body has nothing to do"
    );
}

/// The dry-finish announcement must name what the job actually needs, not
/// claim the store is empty: `give` below leaves `FLOOR_FINISH_COST - 1`
/// (more than zero) Blank Substrate in reach, so "no Blank Substrate in
/// store" is simply false the moment any amount short of the cost is held.
#[test]
fn a_dry_finish_names_the_cost_rather_than_claiming_the_store_is_empty() {
    let (mut game, _staff) = base_with_a_crew(3273, 1);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(WALL.0, WALL.1);
    let apply = FinishOrder::Apply(FloorId::from("cobalt_carpet"));
    game.toggle_mark_box(WALL, WALL, Some(&apply));
    give(
        &mut game,
        &ItemId::from(ids::BLANK_SUBSTRATE),
        crate::tuning::FLOOR_FINISH_COST - 1,
    );

    pass(&mut game, 1);

    let said = game.message_history(20).iter().any(|line| {
        line.text.contains(&format!(
            "needs {} Blank Substrate",
            crate::tuning::FLOOR_FINISH_COST
        ))
    });
    assert!(
        said,
        "the announcement must name the cost ({})",
        crate::tuning::FLOOR_FINISH_COST
    );
    let claimed_empty = game
        .message_history(20)
        .iter()
        .any(|line| line.text.contains("no Blank Substrate in store"));
    assert!(
        !claimed_empty,
        "the store is not empty — the crew is short of the cost, not out of the item"
    );
}

/// Replacing spends the full cost again; nothing is refunded for the finish
/// that was already there.
#[test]
fn replacing_a_finish_spends_the_cost_and_refunds_nothing() {
    let mut game = game_at_the_frontier(3272);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(WALL.0, WALL.1);
        grid.set_finish(WALL.0, WALL.1, FloorId::from("cobalt_carpet"));
    }
    let new_finish = FloorId::from("moss_weave");
    game.toggle_mark_box(WALL, WALL, Some(&FinishOrder::Apply(new_finish.clone())));
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("a replacement mark spawns a site");
    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: WALL.0 - 1,
            y: WALL.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    let before = count_item(&game, ids::BLANK_SUBSTRATE);

    run_to_landing(&mut game);

    assert_eq!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .finish_at(WALL.0, WALL.1),
        Some(&new_finish)
    );
    assert_eq!(
        before - count_item(&game, ids::BLANK_SUBSTRATE),
        crate::tuning::FLOOR_FINISH_COST,
        "replacing a finish must spend the full cost again"
    );
}

#[test]
fn strip_removes_the_finish_leaves_floor_and_spends_nothing() {
    let mut game = game_at_the_frontier(3273);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(WALL.0, WALL.1);
        grid.set_finish(WALL.0, WALL.1, FloorId::from("cobalt_carpet"));
    }
    game.toggle_mark_box(WALL, WALL, Some(&FinishOrder::Strip));
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("a strip mark spawns a site");
    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: WALL.0 - 1,
            y: WALL.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    let before = count_item(&game, ids::BLANK_SUBSTRATE);

    run_to_landing(&mut game);

    let grid = game.world.resource::<base_grid::BaseGrid>();
    assert!(
        grid.finish_at(WALL.0, WALL.1).is_none(),
        "the finish must be gone"
    );
    assert!(
        grid.is_floor(WALL.0, WALL.1),
        "the cell must still be floor"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        before,
        "stripping must move no stock"
    );
    assert!(
        game.dig_site_at(WALL.0, WALL.1).is_none(),
        "a stripped site is despawned"
    );
}

/// The same mutation-proof rule `the_dry_report_is_said_again_after_the_base_restocks_and_runs_out`
/// uses: an entry count would read the line said twice as said once.
#[test]
fn a_dry_apply_site_announces_once_across_several_ticks() {
    let (mut game, _staff) = base_with_a_crew(3275, 1);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(WALL.0, WALL.1);
    let apply = FinishOrder::Apply(FloorId::from("cobalt_carpet"));
    game.toggle_mark_box(WALL, WALL, Some(&apply));
    // No Blank Substrate anywhere, base or pack.

    pass(&mut game, 80);

    let dry_lines = |g: &Game| {
        g.message_history(400)
            .into_iter()
            .filter(|m| m.text.contains("can't finish the floor"))
            .map(|m| m.repeats)
            .sum::<usize>()
    };
    assert_eq!(
        dry_lines(&game),
        1,
        "said once on entering the state, not once a tick"
    );
}

/// A `Strip` site needs no substrate at all, so it must be worked through
/// the full pipeline even when the base holds none anywhere.
#[test]
fn a_strip_site_is_a_want_with_zero_substrate() {
    let (mut game, _staff) = base_with_a_crew(3276, 1);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(WALL.0, WALL.1);
        grid.set_finish(WALL.0, WALL.1, FloorId::from("cobalt_carpet"));
    }
    game.toggle_mark_box(WALL, WALL, Some(&FinishOrder::Strip));

    pass(
        &mut game,
        crate::tuning::BASE_DIG_TICKS_PER_SWING as usize + WALK_ALLOWANCE,
    );

    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .finish_at(WALL.0, WALL.1)
            .is_none(),
        "a Strip site must be worked even with zero substrate anywhere"
    );
}

/// `BaseGrid::clear_finish` reports whether it actually removed anything,
/// and the crew's own log must agree — a `Strip` site landing on a cell
/// something else already stripped must say nothing rather than claim a
/// strip that did not happen.
#[test]
fn stripping_an_already_bare_cell_logs_nothing() {
    let mut game = game_at_the_frontier(9103);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(WALL.0, WALL.1);
        grid.set_finish(WALL.0, WALL.1, FloorId::from("cobalt_carpet"));
    }
    game.toggle_mark_box(WALL, WALL, Some(&FinishOrder::Strip));
    let site = game
        .dig_site_at(WALL.0, WALL.1)
        .expect("a marked floor cell has a dig site");
    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: WALL.0 - 1,
            y: WALL.1,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    // Gone before the crew's cycle lands, so `clear_finish` below finds
    // nothing left to remove.
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .clear_finish(WALL.0, WALL.1);

    run_to_landing(&mut game);

    let said = game
        .message_history(20)
        .iter()
        .any(|line| line.text.contains("strips the finish"));
    assert!(!said, "nothing was stripped, so nothing should be logged");
}

/// `BaseGrid::revert` is the only way to make this cell — nothing in play
/// takes a `Floor` cell back — but the store must still hold "a finish
/// implies floor" if it ever happens.
#[test]
fn a_cell_that_stopped_being_floor_despawns_an_apply_site_uncharged() {
    let finish = FloorId::from("cobalt_carpet");
    let (mut game, _digger) = a_posted_finisher(3277, WALL, FinishOrder::Apply(finish));
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    let before = count_item(&game, ids::BLANK_SUBSTRATE);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .revert(WALL.0, WALL.1);

    run_to_landing(&mut game);

    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        before,
        "an uncharged despawn must spend nothing"
    );
    assert!(
        game.dig_site_at(WALL.0, WALL.1).is_none(),
        "the stale site must be despawned"
    );
}

/// A stale `Apply` order — an id `FloorDb` no longer resolves — must never
/// reach `BaseGrid::set_finish`: the crew despawns the site uncharged
/// rather than painting a finish nothing can name. `set_mark` never checks
/// `FloorDb` at all, so this fixture can hand the crew the bad order
/// directly, the same way a save from before this check existed could.
#[test]
fn crew_finishes_an_unresolvable_apply_uncharged_and_despawns_it() {
    let bogus = FloorId::from("no_such_finish");
    let (mut game, _digger) = a_posted_finisher(9101, WALL, FinishOrder::Apply(bogus));
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    let before = count_item(&game, ids::BLANK_SUBSTRATE);

    run_to_landing(&mut game);

    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        before,
        "an unresolvable finish must spend nothing"
    );
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .finish_at(WALL.0, WALL.1)
            .is_none(),
        "the grid must never wear a finish FloorDb cannot resolve"
    );
    assert!(
        game.dig_site_at(WALL.0, WALL.1).is_none(),
        "the stale site must be despawned"
    );
}

/// A save made while `cobalt_carpet` was installed, loaded after a mod
/// dropped it: `restore_dig_sites` must prune the stale `Apply` order
/// rather than spawn a `DigSite` the crew can never finish —
/// `BaseGrid::prune_finishes`'s rule, reached a second way for the order
/// rather than the grid's own `finishes` map.
#[test]
fn a_restored_apply_naming_a_dropped_finish_is_pruned_at_load() {
    let finish = FloorId::from("cobalt_carpet");
    let (mut game, _digger) = a_posted_finisher(9102, WALL, FinishOrder::Apply(finish));
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_apply_finish_dropped_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let assets = assets_dir_missing_floor("apply_finish_dropped", "cobalt_carpet.ron");
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.dig_site_at(WALL.0, WALL.1).is_none(),
        "an Apply order naming a dropped finish must not survive the load"
    );
    let warned = loaded
        .message_history(20)
        .iter()
        .any(|line| line.text.contains("no such finish is loaded"));
    assert!(
        warned,
        "the drop must be logged, the way prune_finishes's own drops are"
    );
}

/// With one tile site and one finish site and a single worker, the tile
/// site is worked first — `dig_wants`' own ordering rule.
#[test]
fn with_one_worker_the_tile_site_is_worked_before_the_finish_site() {
    let (mut game, staff) = base_with_a_crew(3278, 1);
    let tile_cell = WALL;
    excavate(&mut game, (tile_cell.0, tile_cell.1));
    game.toggle_mark_box(tile_cell, tile_cell, None);
    // Touches the now-Open `tile_cell`, so it has a station of its own —
    // laid directly rather than dug, since only the ordering is under test.
    // `y - 1` rather than `y + 1`: a plain `(x, y)` sort would then rank
    // this cell *first*, so the assertion below only passes because
    // `dig_wants`' sort key puts a finish want (`f.is_some()`) after a tile
    // want regardless of position.
    let finish_cell = (tile_cell.0, tile_cell.1 - 1);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(finish_cell.0, finish_cell.1);
    game.toggle_mark_box(
        finish_cell,
        finish_cell,
        Some(&FinishOrder::Apply(FloorId::from("cobalt_carpet"))),
    );
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    fragments_for(&mut game, 1);

    // One tick is enough for `schedule_base_labour` to post the one body —
    // it writes the assignment outright, and the walk it takes to reach the
    // post is a separate, later concern this test is not about.
    pass(&mut game, 1);

    let tile_site = game
        .dig_site_at(tile_cell.0, tile_cell.1)
        .expect("one tick cannot have finished the tile job");
    assert_eq!(
        posted_at(&game, staff[0]),
        Some(tile_site),
        "with one body and both wants available, the tile job must be worked first"
    );
}

#[test]
fn spend_substrate_spends_the_base_before_the_pack() {
    let finish = FloorId::from("cobalt_carpet");
    let (mut game, _digger) = a_posted_finisher(3279, WALL, FinishOrder::Apply(finish));
    let shelf = give_base(&mut game, 2);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 5);
    let pack_before = count_item(&game, ids::BLANK_SUBSTRATE);

    run_to_landing(&mut game);

    assert_eq!(
        base_substrate(&game, shelf),
        0,
        "the base's own shelf must be drained first"
    );
    assert_eq!(
        pack_before - count_item(&game, ids::BLANK_SUBSTRATE),
        crate::tuning::FLOOR_FINISH_COST - 2,
        "only the shortfall past the base's 2 units comes from the pack"
    );
}

#[test]
fn spend_substrate_below_the_count_moves_nothing_from_either_store() {
    let finish = FloorId::from("cobalt_carpet");
    let (mut game, _digger) = a_posted_finisher(3280, WALL, FinishOrder::Apply(finish));
    let shelf = give_base(&mut game, 1);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 1);
    let pack_before = count_item(&game, ids::BLANK_SUBSTRATE);

    run_to_landing(&mut game);

    assert_eq!(
        base_substrate(&game, shelf),
        1,
        "below the count, the base shelf must be untouched"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        pack_before,
        "below the count, the pack must be untouched"
    );
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .finish_at(WALL.0, WALL.1)
            .is_none(),
        "nothing should have been applied"
    );
}

// ---------------------------------------------------------------------------
// Floor finishes: the engine's view of a finish
// ---------------------------------------------------------------------------

/// `view_finishes_at` must walk the same grid `view_tiles_at` does, or gui's
/// two lookups on one coordinate would silently disagree about its shape.
#[test]
fn view_finishes_at_has_view_tiles_ats_own_shape() {
    let mut game = game_at_the_frontier(4400);
    let (cx, cy) = game.field_pos().unwrap();
    let tiles = game.view_tiles_at((cx, cy), 3, 2);
    let finishes = game.view_finishes_at((cx, cy), 3, 2);

    assert_eq!(finishes.len(), tiles.len());
    for (t_row, f_row) in tiles.iter().zip(finishes.iter()) {
        assert_eq!(t_row.len(), f_row.len());
    }
}

/// A finish resolves at exactly the index `view_tiles_at` draws that cell's
/// `Biome::Platform` at — the property that lets gui zip the two grids with
/// no coordinate translation of its own.
#[test]
fn a_finished_cell_lands_at_view_tiles_ats_own_index() {
    let mut game = game_at_the_frontier(4401);
    let (cx, cy) = game.field_pos().unwrap();
    // The player's own standing tile is already laid floor — the pocket.
    assert!(game.world.resource_mut::<base_grid::BaseGrid>().set_finish(
        cx,
        cy,
        FloorId::from("cobalt_carpet")
    ));

    let half = 3;
    let tiles = game.view_tiles_at((cx, cy), half, half);
    let finishes = game.view_finishes_at((cx, cy), half, half);

    let (row, col) = (half as usize, half as usize);
    assert_eq!(
        tiles[row][col].biome,
        Biome::Platform,
        "the fixture's own cell"
    );
    let finish = finishes[row][col]
        .as_ref()
        .expect("the centre cell was just finished");
    assert_eq!(finish.name, "Cobalt Carpet");
}

/// A finish lives on laid floor alone, so a run with no floor resolves none
/// anywhere.
#[test]
fn view_finishes_at_is_all_none_with_no_floor_laid() {
    let game = game(4402);
    let finishes = game.view_finishes_at((0, 0), 2, 2);
    assert!(
        finishes.iter().flatten().all(|f| f.is_none()),
        "nothing is laid; nothing here should resolve"
    );
}

/// A finish underfoot is named ahead of the wall beyond it — the shape
/// correction 8 asks for: the ray's own first stop is checked for a finish
/// before the ray runs.
#[test]
fn describe_base_rock_names_the_finish_before_the_wall_beyond_it() {
    let mut game = game(4403);
    stand_in_base_at(&mut game, 0, 0);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(1, 0);
        assert!(grid.set_finish(1, 0, FloorId::from("cobalt_carpet")));
        // (2, 0) is left solid by construction — the wall beyond the finish.
    }

    let line = game
        .describe_base_rock(1, 0, 5)
        .expect("a finish plus a wall beyond it must still answer");
    assert!(
        line.starts_with("Cobalt Carpet underfoot; "),
        "the finish must be named first: {line}"
    );
    assert!(
        line.contains("swings to cut through"),
        "the rock line must still follow: {line}"
    );
}

/// A finish with no rock in range still answers `Some` — an examined room
/// that is fully floored and fully finished is not silence.
#[test]
fn describe_base_rock_names_a_finish_with_no_rock_in_range() {
    let mut game = game(4404);
    stand_in_base_at(&mut game, 0, 0);
    {
        let mut grid = game.world.resource_mut::<base_grid::BaseGrid>();
        for step in 1..=5 {
            grid.lay_floor(step, 0);
        }
        assert!(grid.set_finish(1, 0, FloorId::from("cobalt_carpet")));
    }

    let line = game
        .describe_base_rock(1, 0, 5)
        .expect("a finish alone must still answer Some");
    assert_eq!(line, "Cobalt Carpet underfoot.");
}

// ---------------------------------------------------------------------------
// The dig plan asks for its own finishes
// ---------------------------------------------------------------------------

/// `cell` floored and marked for a Cobalt Carpet: the one kind of dig job
/// that spends Blank Substrate, and so the one the plan's order is for.
fn mark_finish(game: &mut Game, cell: (i32, i32)) {
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(cell.0, cell.1);
    let apply = FinishOrder::Apply(FloorId::from("cobalt_carpet"));
    game.toggle_mark_box(cell, cell, Some(&apply));
}

/// A Mining Node and the Lathe beside it: the shortest line that makes Blank
/// Substrate, so `chain_break` lets an order for it stand.
fn lay_substrate_line(game: &mut Game) {
    spawn_machine_with_recipe_at(game, "extractor", "core_fragment", 2, 0);
    spawn_machine_with_recipe_at(game, "factory", "blank_substrate", 2, 1);
}

fn dig_orders(game: &Game) -> Vec<WorkOrder> {
    game.work_orders()
        .iter()
        .filter(|o| o.for_dig)
        .cloned()
        .collect()
}

/// Nothing asked the Lathe for what a plan needs, so no body ever went to
/// run it. A plan with finishes marked files a standing Blank Substrate
/// order sized to what they will spend, and files it **below** the player's
/// own orders.
#[test]
fn a_finish_plan_files_a_standing_substrate_order_below_the_players() {
    let (mut game, _staff) = base_with_a_crew(3400, 1);
    lay_substrate_line(&mut game);
    game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 20))
        .unwrap();
    mark_finish(&mut game, WALL);
    mark_finish(&mut game, (WALL.0, WALL.1 + 1));

    pass(&mut game, 2);

    let orders = game.work_orders();
    assert_eq!(
        orders.len(),
        2,
        "one player order and one plan order: {orders:?}"
    );
    assert_eq!(orders[0].item, ItemId::from(ids::CORE_FRAGMENT));
    assert!(
        !orders[0].for_dig,
        "the player's order is still the player's"
    );
    let plan = &orders[1];
    assert!(plan.for_dig, "the plan's order says whose it is");
    assert_eq!(plan.item, ItemId::from(ids::BLANK_SUBSTRATE));
    assert!(
        plan.standing,
        "a level held, so flooring draws it down and wakes it"
    );
    assert_eq!(
        plan.qty,
        2 * crate::tuning::FLOOR_FINISH_COST,
        "one finish's substrate per marked cell"
    );
}

/// **The fix for a base that cut 112 cells and floored none**: nothing asked
/// for the tiles a plan needs, so no body ever went to make them. A tile is
/// paid in Core Fragments, so a plan of cuts and tiles files a standing
/// Core Fragment order — a cut cell's future tile included — and asks the
/// Lathe for nothing.
#[test]
fn a_tile_plan_files_a_fragment_order_and_no_substrate_order() {
    let (mut game, _staff) = base_with_a_crew(3405, 1);
    lay_substrate_line(&mut game);
    fragments_for(&mut game, 0);
    excavate(&mut game, (WALL.0, WALL.1));
    mark(&mut game, WALL);
    mark(&mut game, (WALL.0, WALL.1 + 1));

    pass(&mut game, 2);

    let orders = dig_orders(&game);
    assert_eq!(orders.len(), 1, "one plan order, for fragments: {orders:?}");
    assert_eq!(orders[0].item, ItemId::from(ids::CORE_FRAGMENT));
    assert_eq!(
        orders[0].qty,
        2 * crate::tuning::FLOOR_TILE_COST,
        "one tile's fragments per marked cell, open or still solid"
    );
}

/// The order tracks the plan: clearing the marks withdraws it, silently —
/// the player cancelled the marks, not the order.
#[test]
fn clearing_the_plan_withdraws_its_order() {
    let (mut game, _staff) = base_with_a_crew(3401, 1);
    lay_substrate_line(&mut game);
    mark_finish(&mut game, WALL);
    pass(&mut game, 2);
    assert_eq!(
        dig_orders(&game).len(),
        1,
        "precondition: the plan filed its order"
    );

    mark(&mut game, WALL);
    pass(&mut game, 2);

    assert!(
        dig_orders(&game).is_empty(),
        "a plan with nothing marked asks for nothing: {:?}",
        game.work_orders()
    );
}

/// Cancelled by hand it would only be filed again next tick, so the queue
/// says where the lever is instead.
#[test]
fn the_plans_order_cannot_be_cancelled_by_hand() {
    let (mut game, _staff) = base_with_a_crew(3402, 1);
    lay_substrate_line(&mut game);
    mark_finish(&mut game, WALL);
    pass(&mut game, 2);

    let err = game
        .cancel_work_order(0)
        .expect_err("the plan's order is the plan's");

    assert!(
        err.contains("mark"),
        "the refusal names the lever, got: {err}"
    );
    assert_eq!(dig_orders(&game).len(), 1);
}

/// With nothing standing that makes Blank Substrate, the plan files nothing —
/// an order `queue_work_order` would refuse is not one the plan may file
/// behind its back.
#[test]
fn a_plan_with_no_lathe_files_nothing() {
    let (mut game, _staff) = base_with_a_crew(3403, 1);
    mark_finish(&mut game, WALL);

    pass(&mut game, 2);

    assert!(game.work_orders().is_empty(), "{:?}", game.work_orders());
}

/// A tile and a finish at once file two plan orders, each tracked by its
/// own item: withdrawing the finish's substrate must leave the tile's
/// fragment order standing.
#[test]
fn a_tile_and_a_finish_keep_two_plan_orders_apart() {
    let (mut game, _staff) = base_with_a_crew(3406, 1);
    lay_substrate_line(&mut game);
    fragments_for(&mut game, 0);
    open_wall(&mut game);
    mark(&mut game, WALL);
    let finished = (WALL.0, WALL.1 + 1);
    mark_finish(&mut game, finished);
    pass(&mut game, 2);

    let items: Vec<ItemId> = dig_orders(&game).into_iter().map(|o| o.item).collect();
    assert_eq!(
        items.len(),
        2,
        "one fragment order and one substrate order: {items:?}"
    );

    game.toggle_mark_box(finished, finished, None);
    pass(&mut game, 2);

    let orders = dig_orders(&game);
    assert_eq!(orders.len(), 1, "{orders:?}");
    assert_eq!(orders[0].item, ItemId::from(ids::CORE_FRAGMENT));
    assert_eq!(orders[0].qty, crate::tuning::FLOOR_TILE_COST);
}

/// No Mining Node, no fragment order — the tile half of
/// `a_plan_with_no_lathe_files_nothing`.
#[test]
fn a_tile_plan_with_no_mining_node_files_nothing() {
    let (mut game, _staff) = base_with_a_crew(3407, 1);
    mark(&mut game, WALL);

    pass(&mut game, 2);

    assert!(game.work_orders().is_empty(), "{:?}", game.work_orders());
}

/// The provenance survives a reload, or a reloaded plan's order turns into a
/// player order the plan can no longer resize or withdraw — and the plan
/// files a second one beside it. A save→load test, since a skipped field is
/// invisible to the RON round trip.
#[test]
fn a_plan_order_keeps_its_provenance_across_a_save() {
    let (mut game, _staff) = base_with_a_crew(3404, 1);
    lay_substrate_line(&mut game);
    mark_finish(&mut game, WALL);
    pass(&mut game, 2);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_space_plan_order_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    pass(&mut loaded, 2);

    assert_eq!(
        dig_orders(&loaded).len(),
        1,
        "exactly the one plan order after a reload: {:?}",
        loaded.work_orders()
    );
    assert_eq!(loaded.work_orders().len(), 1);
}

/// Rock is cut straight ahead only. A diagonal swing could open a cell that
/// touches the base at a corner alone, and the base grows four ways, so a
/// room dug that way would stand out of every siege's reach. Refused like a
/// disarmed bump: free, no tick. The target is minable (floor sits under it
/// orthogonally), so the diagonal is the only reason to refuse.
#[test]
fn a_diagonal_bump_into_rock_is_refused_without_a_swing() {
    let mut game = game_with_a_base(3129);
    game.toggle_mining();
    let edge = (0, -TEST_POCKET_HALF_HEIGHT);
    stand_in_base_at(&mut game, edge.0, edge.1);
    let target = (edge.0 + 1, edge.1 - 1);
    set_rock(&mut game, target);
    assert!(
        is_solid(&game, target),
        "the fixture must be shoving at real rock"
    );
    let tick = game.current_tick();

    game.move_player(1, -1);

    assert_eq!(game.field_pos(), Some(edge));
    assert_eq!(cell(&game, target), Cell::Solid, "and nothing was cut");
    assert_eq!(
        game.current_tick(),
        tick,
        "a refused diagonal costs no turn"
    );
}

/// Floor joins the base: a press on open ground with no floor beside it is
/// refused, and one beside laid floor is accepted.
#[test]
fn a_tile_press_must_touch_floor() {
    let mut game = game(3140);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 10);
    let far = (OFF_THE_FLOOR.0, OFF_THE_FLOOR.1);
    step_off_the_floor(&mut game);
    stand_player_at(&mut game, far.0, far.1);
    let err = game.lay_tile().expect_err("no floor beside it");
    assert_eq!(
        err,
        crate::game::base::building::PlacementRefusal::NotAdjacent.message()
    );

    let (ex, ey) = (far.0 - 1, far.1);
    game.dev_lay_floor([(ex, ey)]);
    game.lay_tile().expect("floor beside it");
}

/// A marked blocker beside floor is accepted; an unmarked one is
/// `NeedsMining`, and one touching nothing is `NotAdjacent`.
#[test]
fn floor_request_refusals_on_blockers() {
    use crate::game::base::building::{PlacementRefusal as R, floor_request_refusal};
    let mut game = game(3141);
    place_home(&mut game);
    let rock = (TEST_POCKET_HALF_WIDTH + 6, 0);
    set_rock(&mut game, rock);
    game.dev_lay_floor([(rock.0 - 1, rock.1)]);
    let pending = std::collections::BTreeSet::new();
    assert_eq!(
        floor_request_refusal(rock, game.ground(), &pending, false),
        Some(R::NeedsMining)
    );
    assert_eq!(
        floor_request_refusal(rock, game.ground(), &pending, true),
        None
    );
    let lone = (rock.0 + 5, rock.1);
    set_rock(&mut game, lone);
    assert_eq!(
        floor_request_refusal(lone, game.ground(), &pending, true),
        Some(R::NotAdjacent)
    );
}

/// A program taken on open ground carries that tile, and has to find its way
/// onto a base founded at the real 3x3 — not the wider test pocket, whose
/// floor reached the old arrival ring and hid that a founded base's never did.
#[test]
fn a_program_captured_off_the_floor_arrives_on_a_freshly_founded_base() {
    let mut game = game(3150);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    ready_to_found(&mut game);
    game.place_structure("home", 0, 0, None).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    *game.world.get_mut::<Position>(pet).unwrap() = Position {
        x: TEST_HOME_CELL.0 + 7,
        y: TEST_HOME_CELL.1 + 4,
    };

    for _ in 0..200 {
        game.tick();
    }

    let at = *game.world.get::<Position>(pet).unwrap();
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_floor(at.x, at.y),
        "a captured program should be on the base's floor by now: {at:?}"
    );
}
