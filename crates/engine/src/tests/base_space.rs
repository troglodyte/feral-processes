//! Base space: the third locale, and which guard each action carries.
//!
//! `Game::require_surface` used to mean "not in the Stack", because with two
//! locales "not underground" and "on the surface proper" were one condition.
//! They are two now, and every guarded action has had to declare which it
//! meant. **A wrong declaration is silent** — nothing about it fails to
//! compile, and both answers refuse in the Stack, so the Stack tests cannot
//! tell them apart either. These are what makes it loud: each one drives the
//! real entry point from all three locales and asserts which refusal it met.

use super::support::*;
use crate::game::base_space::BASE_EXIT_CELL;
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Where an action is attempted from.
#[derive(Clone, Copy, Debug)]
enum At {
    Surface,
    Base,
    Stack,
}

/// The fragment every `Game::require_base` refusal carries, in both of the
/// locales it refuses. Matched on rather than the whole string so the two
/// wordings ("not out here" / "not down here") stay free to differ, which is
/// the point of having two.
const THE_BASE: &str = "back at the base";

/// The same for `Game::require_surface`. Deliberately *not* a substring of
/// `THE_BASE` or the other way round: a test that cannot tell the two
/// refusals apart proves nothing about which guard a site is on.
const OPEN_GRID: &str = "open grid";

/// Builds a fresh fixture, moves the party to `at`, and runs `action`.
///
/// The fixture always runs on the surface and the locale is set afterwards,
/// which is what keeps the three runs comparable: the base, the machines and
/// the player's `Position` are identical in all three, and the locale is the
/// only difference between them.
fn attempt<T>(
    seed: u32,
    at: At,
    fixture: &impl Fn(&mut Game) -> T,
    action: &impl Fn(&mut Game, T) -> Result<(), String>,
) -> Result<(), String> {
    let mut game = game(seed);
    let subject = fixture(&mut game);
    match at {
        At::Surface => {}
        At::Base => stand_in_base(&mut game),
        At::Stack => descend(&mut game),
    }
    action(&mut game, subject)
}

/// Asserts `what` belongs to base space: permitted there, and refused in the
/// other two *by the base guard* rather than by some later check the fixture
/// happened to trip. Reading the refusal is the whole assertion — an
/// `is_err()` here would pass just as well against a site left on
/// `require_surface`, since that refuses in the Stack too.
fn is_a_base_action<T>(
    seed: u32,
    what: &str,
    fixture: impl Fn(&mut Game) -> T,
    action: impl Fn(&mut Game, T) -> Result<(), String>,
) {
    if let Err(refused) = attempt(seed, At::Base, &fixture, &action) {
        panic!("{what} must be permitted in base space, got: {refused}");
    }
    for at in [At::Surface, At::Stack] {
        let refused = attempt(seed, at, &fixture, &action)
            .err()
            .unwrap_or_else(|| panic!("{what} must be refused on the {at:?}"));
        assert!(
            refused.contains(THE_BASE),
            "{what} must be refused on the {at:?} by the base guard, got: {refused}"
        );
    }
}

// ---------------------------------------------------------------------
// The eleven re-read guard sites
// ---------------------------------------------------------------------

/// A machine, not the Home: founding the base is the one deploy made from
/// the open grid (`Game::place_structure`), and it has its own tests further
/// down. Everything after it is a base action.
#[test]
fn deploying_a_structure_is_a_base_action() {
    is_a_base_action(
        3100,
        "deploying",
        |game| {
            place_home(game);
            give(game, &ItemId::from(ids::CORE_FRAGMENT), 20);
        },
        |game, ()| game.place_structure("mining_node", 1, 0),
    );
}

#[test]
fn upgrading_a_structure_is_a_base_action() {
    is_a_base_action(
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
        |game, node| game.upgrade_structure(node),
    );
}

#[test]
fn demolishing_a_structure_is_a_base_action() {
    is_a_base_action(
        3102,
        "demolishing",
        deploy_upgradeable_node,
        |game, node| game.remove_structure(node),
    );
}

#[test]
fn working_a_machine_by_hand_is_a_base_action() {
    is_a_base_action(
        3103,
        "working a machine by hand",
        |game| {
            let at = *game.world.get::<Position>(game.player_entity()).unwrap();
            spawn_mining_node(game, at.x + 1, at.y)
        },
        |game, node| game.work_structure(node),
    );
}

#[test]
fn filing_a_work_order_is_a_base_action() {
    is_a_base_action(
        3104,
        "filing a work order",
        |game| {
            // The shipped three-deep line for a Routine Disk, each machine
            // orthogonally adjacent to its feeder — anything less and the
            // order is refused by the chain rather than by the locale.
            place_home(game);
            spawn_machine_at(game, "mining_node", 2, 0);
            spawn_machine_at(game, "lathe", 3, 0);
            spawn_machine_at(game, "disk_press", 4, 0);
        },
        |game, ()| game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 3)),
    );
}

/// `collect_adjacent` reports what it took rather than a `Result`, so its
/// refusal is an empty haul. That makes the base half of this the load-bearing
/// assertion: an empty haul on the surface is what a site left on
/// `require_surface` would produce in base space instead.
#[test]
fn collecting_from_a_machine_is_a_base_action() {
    let stock_a_neighbour = |game: &mut Game| {
        let at = *game.world.get::<Position>(game.player_entity()).unwrap();
        let mut stock = Stock::new(crate::tuning::DEFAULT_OUTPUT_CAPACITY);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 4);
        game.world.spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position {
                x: at.x + 1,
                y: at.y,
            },
            stock,
        ));
    };
    let collect = |game: &mut Game, ()| {
        if game.collect_adjacent().is_empty() {
            Err("nothing was collected".to_string())
        } else {
            Ok(())
        }
    };

    assert!(
        attempt(3105, At::Base, &stock_a_neighbour, &collect).is_ok(),
        "collecting must work in base space, where the machines are"
    );
    for at in [At::Surface, At::Stack] {
        assert!(
            attempt(3105, at, &stock_a_neighbour, &collect).is_err(),
            "collecting must take nothing on the {at:?}"
        );
    }
}

#[test]
fn selling_an_item_is_a_base_action() {
    is_a_base_action(
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
fn buying_an_item_back_is_a_base_action() {
    is_a_base_action(
        3107,
        "buying back from a trader",
        |game| {
            let market = spawn_market(game);
            let plating = ItemId::from(ids::FIREWALL_PLATING);
            give(game, &plating, 2);
            // Sold from inside the base, since selling is a base action too —
            // the shelf this buys back off has to be stocked somehow.
            from_inside_the_base(game, |g| g.sell_item(market, gear(&plating, 0), 2)).unwrap();
            let paid = game.sell_price(market, &plating).unwrap();
            give(game, &ItemId::from(ids::CREDITS), paid * 8);
            market
        },
        |game, market| game.buy_back(market, gear(&ItemId::from(ids::FIREWALL_PLATING), 0), 2),
    );
}

#[test]
fn selling_a_program_is_a_base_action() {
    is_a_base_action(
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
fn buying_an_item_is_a_base_action() {
    is_a_base_action(
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

/// **Controller ruling 7, slice-1 Task 6.** The one site of the eleven that
/// kept `require_surface` — until the base moved. `rest` demands a structure
/// whose def sets `enables_rest` within reach and Home is the only shipped
/// one, so with Home standing in base space the surface guard would leave
/// resting working only through the anchor-and-origin coordinate collision
/// at `(0, 0)`, which this repo already records as a vacuous-assertion trap.
///
/// `rest` logs its refusal rather than returning it, so the probe is whether
/// the world moved: a rest that runs advances `REST_TICKS` at once, and a
/// refused one spends nothing at all.
#[test]
fn resting_is_a_base_action() {
    let rested = |at: At| {
        let mut game = game(3110);
        // Stands the Home *and* puts the party in base space, so the locale
        // is the only thing the three runs differ by.
        stand_in_base_beside_home(&mut game);
        match at {
            At::Base => {}
            At::Surface => game.world.insert_resource(Locale::Surface),
            At::Stack => descend(&mut game),
        }
        let before = game.current_tick();
        game.rest();
        game.current_tick() > before
    };

    assert!(
        rested(At::Base),
        "resting must work where Home actually stands"
    );
    assert!(
        !rested(At::Surface),
        "resting must be refused on the open grid"
    );
    assert!(!rested(At::Stack), "resting must be refused underground");
}

/// And the refusals say which guard caught them, so a site left on
/// `require_surface` could not pass the test above by permitting everywhere.
#[test]
fn a_refused_rest_names_the_base_it_wanted() {
    let mut game = game(3111);
    stand_in_base_beside_home(&mut game);
    game.world.insert_resource(Locale::Surface);

    game.rest();

    let said = game
        .message_log(200)
        .into_iter()
        .any(|line| line.text.contains(THE_BASE));
    assert!(
        said,
        "a rest refused on the open grid must name the base guard, log: {:?}",
        game.message_log(5)
    );
}

/// The other half of the flip, and the half a locale guard alone cannot
/// prove: the reach check has to measure the player's **base** cell against
/// the structure's base-space `Position`.
///
/// The player's surface `Position` is dragged far past Home's `enables_rest`
/// radius first. A reach still measured on the surface tile fails outright;
/// one measured in base space is unaffected, because neither the party nor
/// the Home moved in the space they are both actually in.
#[test]
fn rest_measures_its_reach_in_base_space() {
    let mut game = game(3112);
    stand_in_base_beside_home(&mut game);
    stand_player_at(&mut game, 500, 500);
    let before = game.current_tick();

    game.rest();

    assert!(
        game.current_tick() > before,
        "reach is between the party's base cell and Home's, and the surface tile has no say: {:?}",
        game.message_log(5)
    );
}

// ---------------------------------------------------------------------
// The two systems that guard on `is_underground` rather than on a
// `require_*`, and the locale predicates themselves
// ---------------------------------------------------------------------

/// `is_underground` stays strictly "down inside the Stack". Base space is off
/// the surface too, and answering yes here would apply every Stack rule in
/// the game — no Power supply, Trace, the frame view — to the base.
#[test]
fn base_space_is_not_underground() {
    let mut game = game(3112);
    stand_in_base(&mut game);

    assert!(!game.is_underground(), "base space is not the Stack");
    assert!(game.in_base());
    assert_eq!(game.base_pos(), Some((0, 0)));
    assert!(
        game.stack_view().is_none(),
        "there is no frame loaded in base space"
    );
}

/// The mirror: `base_pos` is the refusal mechanism for anything base-only, so
/// it must answer `None` in both of the other two locales rather than a
/// default pair.
#[test]
fn base_pos_answers_nowhere_but_base_space() {
    let mut game = game(3113);
    assert_eq!(game.base_pos(), None, "the surface is not base space");
    assert!(!game.in_base());

    descend(&mut game);
    assert_eq!(game.base_pos(), None, "the Stack is not base space either");
    assert!(!game.in_base());
}

/// A Recharger's radius is measured from `Game::base_pos`, not the player's
/// `Position` — that stays pinned to the anchor tile for the whole of a
/// visit to base space, and every `Structure`, Recharger included, can only
/// ever be deployed *in* base space now (`place_structure` requires
/// `require_base` for everything but the founding Home).
///
/// **Reaching the party in base space is the deliberate, corrected
/// behaviour, not a bug.** `recharger_node.ron`'s own description reads
/// "while you stand anywhere on your base" — a Recharger that could never
/// regen anyone in base space would be a structure with no reachable
/// purpose at all. See `power_regen_system`'s doc comment for the fuller
/// argument; this test held the opposite claim before that fix and is the
/// reason the defect survived review.
///
/// Walked through the real entry points — `enter_base` and `place_structure`
/// — rather than `spawn_recharger_node`'s old trick of spawning a structure
/// at an offset from the player's `Position`. That fixture places a
/// structure in whatever coordinate space `Position` happens to hold,
/// which is base-space numbers on the surface and surface-space numbers
/// once `stand_in_base` flips the locale without moving anything — the
/// exact manufactured coincidence that let the old, inverted assertion
/// here pass.
#[test]
fn a_recharger_regens_the_party_through_the_real_path_into_base_space() {
    let mut game = game(3114);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    game.place_structure("recharger_node", 1, 0)
        .expect("floor beside the origin is inside the starting pocket");
    assert_eq!(
        game.base_pos(),
        Some((0, 0)),
        "the party never moved off the exit cell — the Recharger sits one step away"
    );

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
        "a Recharger deployed in base space must regen a party genuinely \
         standing in there, reached through enter_base: {before} -> {after}"
    );
}

/// The surface arm of the same guard, and not a no-op the way comparing the
/// raw `Position` there might look. `BASE_EXIT_CELL` puts a base's Home at
/// base-space `(0, 0)` on *every* run, and `scan_center`'s own doc names why
/// that matters: a surface tile reads as the same numbers as a base-space
/// one exactly when the anchor sits near base space's origin — which the
/// zone spawn point usually does, but "usually" is not "always", so
/// `stand_player_at` forces the coincidence here rather than trusting the
/// fixture's seed to land on it.
#[test]
fn a_recharger_does_not_reach_the_party_genuinely_on_the_surface() {
    let mut game = game(3117);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    game.place_structure("recharger_node", 1, 0)
        .expect("floor beside the origin is inside the starting pocket");
    game.leave_base().unwrap();
    assert!(!game.in_base(), "back on the open grid");

    // Forced onto numbers a base-space Recharger's radius-10 reach would
    // have matched under the old, buggy compare: the Recharger sits at
    // base-space (1, 0).
    stand_player_at(&mut game, 0, 0);

    let player = game.player_entity();
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(5.0);
    let before = game.world.get::<PowerReserve>(player).unwrap().get();

    game.tick();

    let after = game.world.get::<PowerReserve>(player).unwrap().get();
    let expected_from_decay_alone = before - crate::tuning::HUNGER_DECAY_PER_TICK;
    assert!(
        (after - expected_from_decay_alone).abs() < 1e-4,
        "a Recharger standing in base space must not reach a party genuinely \
         on the open surface, however close the raw numbers land: expected \
         decay only ({before} -> {expected_from_decay_alone}), got {after}"
    );
}

/// The same trap on the other reader of the pinned `Position`: a guardian
/// standing beside the anchor tile would otherwise open a battle on a party
/// that is not there.
#[test]
fn a_nest_guardian_does_not_open_a_battle_in_base_space() {
    let provoked = |at: At| {
        let mut game = game(3115);
        let at_tile = *game.world.get::<Position>(game.player_entity()).unwrap();
        // No `Nest` component needed — the leash check only asks for a
        // `Position` on the entity `NestGuardian::nest` names.
        let nest = game.world.spawn(at_tile).id();
        let guardian = game
            .spawn_wild_creature("construct", at_tile.x + 1, at_tile.y)
            .expect("construct ships with the game");
        game.world
            .entity_mut(guardian)
            .insert((NestGuardian { nest }, Pursuing));
        if let At::Base = at {
            stand_in_base(&mut game);
        }
        game.tick();
        game.has_active_battle()
    };

    assert!(
        provoked(At::Surface),
        "an adjacent pursuer must still engage on the surface"
    );
    assert!(
        !provoked(At::Base),
        "a pursuer beside the anchor must not reach a party out of phase"
    );
}

// ---------------------------------------------------------------------
// The anchor: the permanent door into base space
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
// Walking in and out through the anchor, and moving around inside
// ---------------------------------------------------------------------

/// The fragment `Game::enter_base` uses when the anchor has nothing behind
/// it yet. Deliberately shares no wording with the "you are not on the
/// anchor" refusal: a player standing in the right place with no base built
/// and a player standing in the wrong place are told different things, and a
/// test that could not tell the two apart would pass against an entry that
/// refused for the wrong reason.
const NO_BASE_YET: &str = "nothing on the other side";

/// A run that has a base to walk into: the run's first Home deployed the way
/// a player deploys it, which stands it on `BASE_EXIT_CELL` and lays the
/// starting pocket around it.
///
/// Nothing is laid by hand any more. The Home is the thing that opens base
/// space, so a fixture that floored cells itself would be describing a state
/// the game cannot reach.
fn game_with_a_base(seed: u32) -> Game {
    let mut game = game(seed);
    place_home(&mut game);
    game
}

/// A base-space cell outside the starting pocket: solid rock, and the same
/// cell whichever test asks. One step north of the pocket's north edge.
fn solid_rock_north() -> (i32, i32) {
    (0, -crate::tuning::STARTING_POCKET_RADIUS - 1)
}

/// The player's tile on the zone surface.
fn player_tile(game: &Game) -> Position {
    *game.world.get::<Position>(game.player_entity()).unwrap()
}

/// The round trip the spec names: stepping through the anchor puts the party
/// out of phase at the exit cell, coming back out restores the surface, and
/// the player's surface `Position` is the anchor tile throughout — pinned
/// there rather than merely starting there, which is what every guard on the
/// eleven re-read sites is protecting against.
#[test]
fn entering_and_leaving_through_the_anchor_round_trips_the_players_position() {
    let mut game = game_with_a_base(3120);
    let anchor = game.anchor_position().expect("a fresh game has an anchor");
    let before = player_tile(&game);
    assert_eq!(
        (before.x, before.y),
        anchor,
        "the fixture must start the player on the anchor, or this tests nothing"
    );

    game.enter_base()
        .expect("standing on an anchor with a Home behind it");

    assert!(game.in_base());
    assert!(!game.is_underground(), "base space is not the Stack");
    assert_eq!(game.base_pos(), Some((0, 0)), "you arrive at the exit cell");
    assert_eq!(
        player_tile(&game),
        before,
        "the surface Position stays pinned to the anchor tile"
    );

    game.leave_base().expect("standing on the exit cell");

    assert!(!game.in_base());
    assert_eq!(game.locale(), Locale::Surface);
    assert_eq!(
        player_tile(&game),
        before,
        "you come back out where you went in"
    );
}

/// The anchor is the only door. Asserted against the same fixture from the
/// anchor tile too, so this cannot pass against an `enter_base` that refuses
/// everywhere.
#[test]
fn the_anchor_is_the_only_way_in() {
    let mut game = game_with_a_base(3121);
    {
        let player = game.player_entity();
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x += 3;
    }

    let refused = game
        .enter_base()
        .expect_err("three tiles off the anchor is not the door");
    assert!(
        !refused.contains(NO_BASE_YET),
        "the fixture has a Home, so this must refuse for the position, got: {refused}"
    );

    {
        let player = game.player_entity();
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x -= 3;
    }
    game.enter_base()
        .expect("back on the anchor, the same call must be permitted");
}

/// A new run has no base at all — `place_structure` refuses everything until
/// a Home is deployed — so the anchor leads nowhere, and says so in its own
/// words rather than by claiming the player is standing somewhere else.
#[test]
fn an_anchor_with_no_home_behind_it_refuses_entry() {
    let mut game = game(3122);
    let standing = player_tile(&game);
    assert_eq!(
        (standing.x, standing.y),
        game.anchor_position().unwrap(),
        "the player starts on the anchor, so position cannot be what refuses"
    );

    let refused = game
        .enter_base()
        .expect_err("a run with no Home has no base to enter");

    assert!(
        refused.contains(NO_BASE_YET),
        "entry with no Home must name the missing base, got: {refused}"
    );
    assert!(!game.in_base());
}

/// **The silent-guard test.** In the Stack the player's `Position` is pinned
/// to the entrance tile, and a run that dived from its starting tile dived
/// from the anchor — so the "are you standing on the anchor?" check passes
/// four frames down. `enter_base` therefore has to ask `require_surface`
/// first, and this is the only thing that says so.
#[test]
fn entering_is_refused_from_the_stack_even_standing_on_the_anchor_tile() {
    let mut game = game_with_a_base(3123);
    descend(&mut game);

    let pinned = player_tile(&game);
    assert_eq!(
        (pinned.x, pinned.y),
        game.anchor_position().unwrap(),
        "the fixture must pin Position to the anchor tile, or this tests nothing"
    );

    let refused = game
        .enter_base()
        .expect_err("there is no anchor four frames down");

    assert!(
        refused.contains(OPEN_GRID),
        "entry from the Stack must be refused by the surface guard, got: {refused}"
    );
    assert!(!game.in_base());
}

/// Leaving belongs to base space, and is refused in the other two locales by
/// the base guard rather than by a check that happens to fail there.
#[test]
fn leaving_is_a_base_action() {
    is_a_base_action(
        3124,
        "leaving base space",
        |_| (),
        |game, ()| game.leave_base(),
    );
}

/// The way out is the one cell the Home stands on, not wherever you happen
/// to be — otherwise base space would have a door in every wall.
#[test]
fn the_way_out_is_only_at_the_exit_cell() {
    let mut game = game_with_a_base(3125);
    game.enter_base().unwrap();
    game.move_player(1, 0);
    assert_eq!(
        game.base_pos(),
        Some((1, 0)),
        "the fixture must walk a tile"
    );

    let refused = game
        .leave_base()
        .expect_err("one cell east of the Home is not the door");
    assert!(
        !refused.contains(THE_BASE),
        "this must refuse for the cell, not by the locale guard, got: {refused}"
    );
    assert!(
        game.in_base(),
        "a refused exit leaves the party where it was"
    );

    game.move_player(-1, 0);
    game.leave_base().expect("back on the exit cell");
    assert!(!game.in_base());
}

/// Solid rock does not give, and the party stays out of it. Both halves of
/// the pin are asserted: base-space coordinates unchanged, *and* the surface
/// `Position` unchanged — without the second, a `move_player` that never
/// dispatched on locale at all would walk the party across the zone map and
/// still pass.
///
/// The turn is slice 2's, and it moved: a step into rock is a *swing* now
/// (`Game::strike_rock`), and a swing costs the turn a step would have, the
/// same way shoving at a nest on the surface does. What it never does is
/// land the party inside the wall, which is what this asserts.
#[test]
fn swinging_at_solid_rock_moves_nothing_in_either_space() {
    let mut game = game_with_a_base(3126);
    game.enter_base().unwrap();
    // The pocket's north edge, so the step north leaves it.
    let edge = (0, -crate::tuning::STARTING_POCKET_RADIUS);
    stand_in_base_at(&mut game, edge.0, edge.1);
    let standing = player_tile(&game);
    let tick = game.current_tick();

    // Past the pocket, and never laid: absent from `BaseGrid`, so solid.
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_solid(solid_rock_north().0, solid_rock_north().1),
        "the fixture must be shoving at real rock"
    );
    game.move_player(0, -1);

    assert_eq!(game.base_pos(), Some(edge), "solid rock does not give");
    assert_eq!(
        game.current_tick(),
        tick + 1,
        "a swing at rock costs the turn the step would have"
    );
    assert_eq!(player_tile(&game), standing);
}

/// A step onto laid Floor moves the base-space coordinates, spends a turn,
/// and leaves the surface `Position` exactly where it was.
#[test]
fn walking_onto_floor_moves_only_the_base_space_coordinates() {
    let mut game = game_with_a_base(3127);
    game.enter_base().unwrap();
    let standing = player_tile(&game);
    let tick = game.current_tick();

    game.move_player(1, 0);

    assert_eq!(game.base_pos(), Some((1, 0)));
    assert!(game.current_tick() > tick, "a step in base space is a turn");
    assert_eq!(
        player_tile(&game),
        standing,
        "the surface Position must not move a tile"
    );
}

/// Movement reads `BaseGrid::walkable`, not `is_floor`: mined-out rock is
/// somewhere you can stand long before it is somewhere you can build.
#[test]
fn mined_rock_is_walkable_before_it_is_floored() {
    let mut game = game_with_a_base(3128);
    let tick = game.current_tick();
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .open(0, 1, tick);
    game.enter_base().unwrap();

    game.move_player(0, 1);

    assert_eq!(game.base_pos(), Some((0, 1)));
    assert!(
        !game.world.resource::<base_grid::BaseGrid>().is_floor(0, 1),
        "the fixture cell must be Open and not Floor, or this tests nothing"
    );
}

/// A swing at rock still breaks off a posted job, exactly as shoving at a
/// wall on the zone surface does — `move_player` drops the job before it
/// looks at what is in the way, "since either way you stopped working to do
/// it", and `Game::work_structure` promises the player as much when it
/// posts.
///
/// The drop happens *before* the wall is even looked at, which is why it
/// survived slice 2 turning that shove into a swing: had it lived in the
/// refusal branch instead, digging would have quietly stopped ending jobs.
#[test]
fn swinging_at_solid_rock_still_breaks_off_a_job() {
    let mut game = game_with_a_base(3129);
    // The pocket's north edge, with a machine beside it: a job to break off,
    // and solid rock one step further north to break it off against.
    let edge = (0, -crate::tuning::STARTING_POCKET_RADIUS);
    let node = spawn_mining_node(&mut game, edge.0 + 1, edge.1);
    game.enter_base().unwrap();
    stand_in_base_at(&mut game, edge.0, edge.1);
    game.work_structure(node).expect("standing beside the node");
    let player = game.player_entity();
    assert!(
        game.world.get::<Task>(player).is_some(),
        "the fixture must actually post a job"
    );
    let tick = game.current_tick();

    // Past the pocket's edge, and never laid: absent from `BaseGrid`, so solid.
    game.move_player(0, -1);

    assert_eq!(
        game.base_pos(),
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
// The pocket, and the base moving into it
// ---------------------------------------------------------------------

/// How many cells `Game::lay_starting_pocket` is supposed to floor, counted
/// the long way round rather than read off the implementation: the chamfered
/// box the base slab has always been, at `STARTING_POCKET_RADIUS`.
///
/// Spelled out here so a change to either constant is a change this test
/// *sees* rather than one it silently follows.
fn pocket_cells() -> usize {
    let r = crate::tuning::STARTING_POCKET_RADIUS;
    let cut = crate::tuning::PLATFORM_CORNER_CUT;
    let mut n = 0;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx.abs() + dy.abs() <= 2 * r - cut {
                n += 1;
            }
        }
    }
    n
}

/// **Controller ruling 10.** Deploying is a base action, and the anchor
/// refuses entry until a Home stands — so a fresh run could do neither, and
/// the first Home has to be deployable from the open grid or a run has no
/// way to start a base at all.
///
/// Every other base fixture in the suite teleports: `place_home` routes
/// through `from_inside_the_base` and app-core's `stand_in_base` overwrites
/// a loaded save's locale. That is exactly why none of them caught the
/// deadlock, so this one starts from a plain `Game::new` and walks.
#[test]
fn a_fresh_run_deploys_its_first_home_from_the_open_grid_and_walks_in() {
    let mut game = game(3130);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    assert!(
        !game.in_base(),
        "the fixture must start on the open grid, or it teleports like the others"
    );

    game.place_structure("home", 1, 0)
        .expect("the first Home is what opens the base, so it cannot need the base to exist");

    let home = game.home_position().expect("a Home was just deployed");
    assert_eq!(
        (home.x, home.y),
        BASE_EXIT_CELL,
        "the first Home stands on base space's own origin, whatever direction was pointed"
    );

    game.enter_base()
        .expect("the anchor opens onto the pocket the Home just laid");
    assert_eq!(game.base_pos(), Some(BASE_EXIT_CELL));
}

/// And only the first one. Every later deployment is a base action, refused
/// on the open grid by the base guard rather than by some later check.
#[test]
fn every_deployment_after_the_first_home_is_a_base_action() {
    let mut game = game(3131);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 40);
    game.place_structure("home", 1, 0)
        .expect("the founding deploy is permitted from outside");

    let refused = game
        .place_structure("mining_node", 1, 0)
        .expect_err("a machine is a base action");

    assert!(
        refused.contains(THE_BASE),
        "a machine deployed from the open grid must be refused by the base guard, got: {refused}"
    );
}

/// The pocket is laid into `BaseGrid` and nowhere else. The whole point of
/// the move is that `Biome::Platform` stops being written into `WorldMap`,
/// so this asserts the override overlay as well as the floor count — a
/// pocket laid *and* a slab stamped would pass a floor-count assertion on
/// its own.
#[test]
fn deploying_a_home_lays_the_pocket_and_stamps_no_world_tile() {
    let mut game = game(3132);
    assert_eq!(
        game.world.resource::<base_grid::BaseGrid>().floor_count(),
        0,
        "base space starts solid"
    );
    let overrides_before = game.world.resource::<WorldMap>().overrides().len();
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);

    game.place_structure("home", 1, 0).unwrap();

    let grid = game.world.resource::<base_grid::BaseGrid>();
    assert_eq!(
        grid.floor_count(),
        pocket_cells(),
        "the pocket is the whole starting slab, laid as Floor"
    );
    assert!(grid.is_floor(0, 0), "including the cell the Home stands on");
    assert!(
        grid.is_solid(crate::tuning::STARTING_POCKET_RADIUS + 1, 0),
        "and it stops: past the radius is unmined rock"
    );
    assert_eq!(
        game.world.resource::<WorldMap>().overrides().len(),
        overrides_before,
        "laying the pocket must write no tile into the zone surface at all"
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

/// A machine goes up on pocket Floor exactly as it always went up on the
/// slab, deployed relative to the cell the player is standing in.
#[test]
fn a_machine_deploys_onto_pocket_floor() {
    let mut game = game(3133);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 40);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    game.move_player(1, 0);
    assert_eq!(game.base_pos(), Some((1, 0)), "stepped off the Home tile");

    game.place_structure("mining_node", 1, 0)
        .expect("floor two cells east of the origin is inside the pocket");

    let node = find_structure_by_kind(&mut game, "mining_node").expect("it went up");
    let at = *game.world.get::<Position>(node).unwrap();
    assert_eq!(
        (at.x, at.y),
        (2, 0),
        "a deploy is measured from the player's base cell, not their surface tile"
    );
}

/// And the footprint rule is `BaseGrid::is_floor` and nothing else: solid
/// rock one step past the pocket's edge refuses, in the pocket's own words.
#[test]
fn deploying_off_the_pocket_floor_is_refused() {
    let mut game = game(3134);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 40);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    let edge = crate::tuning::STARTING_POCKET_RADIUS;
    game.world.insert_resource(Locale::Base { x: edge, y: 0 });
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_floor(edge, 0),
        "the fixture must stand on the pocket's last floored cell"
    );

    let refused = game
        .place_structure("mining_node", 1, 0)
        .expect_err("one step past the edge is unmined rock");

    assert!(
        !refused.contains(THE_BASE),
        "this must be refused by the footprint rule, not by the locale guard: {refused}"
    );
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_solid(edge + 1, 0),
        "and the cell it refused really is solid, or the test proves nothing"
    );
}

// ---------------------------------------------------------------------
// `broker_reach` measures the base, not the Broker
// ---------------------------------------------------------------------

/// A run with a Home and a Broker standing, and the party still outside.
///
/// The Home is what lays the pocket, so a Broker without one would stand in
/// a base space that is still solid everywhere — and `broker_reach` would
/// answer `OffBase` for want of floor rather than for want of distance.
fn game_with_a_broker(seed: u32) -> Game {
    let mut game = game(seed);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    spawn_machine_at(&mut game, "contract_broker", 1, 0);
    game
}

#[test]
fn a_broker_is_in_reach_from_pocket_floor() {
    let mut game = game_with_a_broker(3135);
    game.enter_base().unwrap();
    assert_eq!(game.broker_reach(), BrokerReach::AtBroker);
}

#[test]
fn a_broker_is_out_of_reach_off_the_floor() {
    let mut game = game_with_a_broker(3136);
    assert_eq!(
        game.broker_reach(),
        BrokerReach::OffBase,
        "on the zone surface the party is not in the base at all"
    );

    game.enter_base().unwrap();
    let outside = crate::tuning::STARTING_POCKET_RADIUS + 3;
    game.world
        .insert_resource(Locale::Base { x: outside, y: 0 });
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_solid(outside, 0),
        "the fixture cell must be off the floor, or this tests nothing"
    );
    assert_eq!(
        game.broker_reach(),
        BrokerReach::OffBase,
        "in base space but off the floor is still off the base"
    );
}

#[test]
fn no_broker_standing_is_not_the_same_as_being_off_the_base() {
    let mut game = game(3137);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    assert_eq!(
        game.broker_reach(),
        BrokerReach::NoBroker,
        "a base with no trader in it has no desk to be near"
    );
}

// ---------------------------------------------------------------------
// The base is out of phase, and the zone surface must not feel it
// ---------------------------------------------------------------------

/// Paints open ground over a box around the player and clears everything
/// alive off it, so a walk below is testing the rule under test rather than
/// the seed's terrain or whatever wandered in. A wild program on the route
/// would open a battle, and `move_player` refuses every step after that —
/// which reads exactly like the wall this is looking for.
fn clear_ground_around_the_player(game: &mut Game, half: i32) {
    let at = *game.world.get::<Position>(game.player_entity()).unwrap();
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        for dy in -half..=half {
            for dx in -half..=half {
                map.set_override(
                    at.x + dx,
                    at.y + dy,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                    },
                );
            }
        }
    }
    clear_the_route(game);
}

/// Everything on the zone surface that answers a step with something other
/// than a step. Called before *each* one below rather than once: every step
/// ticks the world, and `maybe_spawn_wild_creature` can put a program on the
/// route between two of them.
fn clear_the_route(game: &mut Game) {
    let alive: Vec<Entity> = {
        let mut query = game
            .world
            .query_filtered::<Entity, Or<(With<Hostile>, With<Nest>, With<SurfaceLink>)>>();
        query.iter(&game.world).collect()
    };
    for entity in alive {
        game.world.despawn(entity);
    }
    // And any fight one of them opened. A step draws an ambush *after* it
    // lands (`Game::maybe_ambush`), so the fight refuses the step after this
    // one — which is the same shape as the wall under test and would read as
    // it on an unlucky seed.
    game.world.remove_resource::<BattleState>();
}

/// **A base standing must not wall the open grid.**
///
/// Every `Structure` is in base space, so a surface reader asking a
/// base-space query about a zone tile answers by numeric coincidence — and
/// the coincidence is the common case, because `find_walkable_start` returns
/// `(0, 0)` whenever it can and the pocket is laid around base space's own
/// origin. Left in, the founding Home made the **anchor tile itself**
/// unwalkable: step off it and the step back was refused silently, with no
/// message and no turn, and the only way home was a symlink.
///
/// A full circuit rather than one step, and every step asserted, because the
/// tiles a base occupies are a patch and not a line — a fix that only
/// stopped the Home blocking would leave every machine an invisible wall.
#[test]
fn a_base_standing_does_not_wall_the_zone_surface() {
    let mut game = game(3140);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 60);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    // Machines all around the exit cell, so the surface tiles they would
    // shadow are the ones the circuit walks over.
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        game.place_structure("mining_node", dx, dy)
            .unwrap_or_else(|e| panic!("({dx}, {dy}) is pocket floor: {e}"));
    }
    game.leave_base().unwrap();
    clear_ground_around_the_player(&mut game, 3);

    let anchor = game.anchor_position().expect("a fresh game has an anchor");
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!(
        (start.x, start.y),
        anchor,
        "the party leaves the base standing on the anchor"
    );

    // Once round the anchor and back onto it. The last step is the one the
    // Home used to refuse.
    let circuit = [
        (1, 0),
        (0, 1),
        (-1, 0),
        (-1, 0),
        (0, -1),
        (0, -1),
        (1, 0),
        (1, 0),
        (0, 1),
        (-1, 0),
    ];
    let mut expected = start;
    for (dx, dy) in circuit {
        clear_the_route(&mut game);
        expected = Position {
            x: expected.x + dx,
            y: expected.y + dy,
        };
        game.move_player(dx, dy);
        assert_eq!(
            *game.world.get::<Position>(game.player_entity()).unwrap(),
            expected,
            "a step onto ({}, {}) was refused — a base-space structure is \
             shadowing the zone surface",
            expected.x,
            expected.y
        );
    }
    assert_eq!(
        (expected.x, expected.y),
        anchor,
        "the circuit has to end back on the anchor, or it never tested the \
         tile the Home stands on"
    );
}

/// The other half of the same rule: a Portal stands in base space, so it is
/// walked onto **there**. Firing a breach from the surface tile that happens
/// to carry its base-space numbers is the same misread pointed the other way,
/// and this one costs a zone.
#[test]
fn a_portal_in_the_base_does_not_breach_from_the_zone_surface() {
    let mut game = game(3141);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    give(&mut game, &ItemId::from(ids::PORTAL_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    game.enter_base().unwrap();
    game.place_structure("portal", 1, 0)
        .expect("a Portal is a structure like any other and stands on floor");
    game.leave_base().unwrap();
    clear_ground_around_the_player(&mut game, 3);
    let zone_before = game.player_status().zone;

    // The surface tile one east of the anchor, which is where the Portal's
    // base-space coordinates land when the two spaces share an origin.
    game.move_player(1, 0);

    assert_eq!(
        game.player_status().zone,
        zone_before,
        "walking the open grid must not fire a Portal standing out of phase"
    );
    assert!(
        find_structure_by_kind(&mut game, "portal").is_some(),
        "and the Portal must still be standing, not consumed by a step \
         taken in another coordinate space"
    );

    // And it does breach, from where it actually stands. Back onto the
    // anchor first — the door is a tile, not a key.
    game.move_player(-1, 0);
    game.enter_base().unwrap();
    game.move_player(1, 0);
    assert_eq!(
        game.player_status().zone,
        zone_before + 1,
        "walking onto it inside the base is what breaches"
    );
}

// ---------------------------------------------------------------------
// `resources::Platform` no longer exists, and the two readers that used to
// branch on it take their radius off the grid instead
// ---------------------------------------------------------------------

/// The load path used to rebuild `Platform::center` from
/// `Game::home_position`, which is a **base-space** coordinate now — that
/// made a loaded run behave differently from the same run before the
/// reload, and it put `Game::clear_platform`'s 401x401 override sweep back
/// in reach of a Home demolition. Both retired with `resources::Platform`
/// itself, so there is nothing left to resurrect; what is still worth
/// asserting is the other half this test always carried — that the pocket
/// `BaseGrid` actually laid survives the round trip.
#[test]
fn a_reload_keeps_the_pocket() {
    let mut game = game(3142);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    assert!(
        game.home_position().is_some(),
        "the fixture must have a Home, or the load path has nothing to rebuild from"
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
        pocket_cells(),
        "the pocket the Home laid must come back, not an empty base"
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
    game.place_structure("home", 1, 0).unwrap();
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
    game.place_structure("home", 1, 0).unwrap();
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
// `Game::view_tiles` dispatches on locale — Task 7
// ---------------------------------------------------------------------

/// In base space, `view_tiles` synthesises tiles from `BaseGrid` rather
/// than reading `WorldMap` at the player's (surface-pinned) `Position`:
/// laid floor reads as `Biome::Platform`, a merely-carved cell as
/// `Biome::Excavated`, and solid, untouched rock as `Biome::Entropy` — the
/// three-way mapping `Game::view_tiles` documents.
#[test]
fn view_tiles_synthesises_the_three_base_biomes() {
    let mut game = game(3200);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    // A cell mined but not floored, just past the starting pocket's edge —
    // `BaseGrid::open` is `pub(crate)` and unused by any gameplay path this
    // slice, so this is the only way to put one on the board at all.
    let open_at = (crate::tuning::STARTING_POCKET_RADIUS + 1, 0);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .open(open_at.0, open_at.1, 0);
    stand_in_base(&mut game);

    // A window wide enough to reach the Open cell just past the pocket and
    // solid rock well beyond it, centred on base space's own origin —
    // `stand_in_base` always lands the party there.
    let half = crate::tuning::STARTING_POCKET_RADIUS + 3;
    let tiles = game.view_tiles(half, half);
    let at = |x: i32, y: i32| tiles[(half + y) as usize][(half + x) as usize];

    assert_eq!(
        at(0, 0).biome,
        Biome::Platform,
        "base space's own origin is laid floor — the Home stands on it"
    );
    assert!(at(0, 0).walkable);

    assert_eq!(
        at(open_at.0, open_at.1).biome,
        Biome::Excavated,
        "a carved, unfloored cell reads as Excavated"
    );
    assert!(
        at(open_at.0, open_at.1).walkable,
        "carved rock is walkable even unfloored"
    );

    let solid = (half, half);
    assert_eq!(
        at(solid.0, solid.1).biome,
        Biome::Entropy,
        "untouched base space, absent from BaseGrid, reads as Entropy"
    );
    assert!(
        !at(solid.0, solid.1).walkable,
        "Entropy is solid — nothing has dug there yet"
    );
}

/// `view_tiles` on the surface is unchanged: still a straight read of
/// `WorldMap` centred on the player's own `Position`, `Biome::Platform`
/// included wherever a test hand-writes one — the point being that base
/// space's synthesis in `view_tiles` only ever engages through
/// `Game::base_pos`, never by inspecting the biome it would produce.
#[test]
fn view_tiles_is_unchanged_on_the_surface() {
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
                "surface view_tiles diverged from a direct WorldMap read at grid ({gx}, {gy})"
            );
        }
    }
}

// ---------------------------------------------------------------------
// What the map draws is what is standing in the space the party is in
// ---------------------------------------------------------------------

/// The player's glyph follows the party around base space.
///
/// `Position` stays pinned to the anchor tile on the zone surface while the
/// party is out of phase (see `resources::Locale`), and `view_entities` is
/// where the map gets every glyph it draws — the player's included. Reading
/// the pinned tile there draws `@` whereever the anchor's surface
/// coordinates happen to alias into base space and leaves it there, however
/// far the party walks.
#[test]
fn the_player_is_drawn_where_the_party_stands_in_base_space() {
    let mut game = game(3210);
    let pinned = *game.world.get::<Position>(game.player_entity()).unwrap();
    let standing = (pinned.x + 2, pinned.y + 1);
    stand_in_base_at(&mut game, standing.0, standing.1);

    let views = game.view_entities(10, 10);
    let player = views
        .iter()
        .find(|v| v.is_player)
        .expect("the player is drawn on the base map");

    assert_eq!(
        player.pos, standing,
        "the player's glyph is drawn at the base-space cell the party is on, \
         not at the surface tile Position is pinned to"
    );

    // Out on the surface the two are the same tile, so the substitution has
    // to be a no-op there rather than a second rule.
    game.world.insert_resource(Locale::Surface);
    let out_here = game.view_entities(10, 10);
    let player = out_here
        .iter()
        .find(|v| v.is_player)
        .expect("the player is drawn on the zone map");
    assert_eq!(
        player.pos,
        (pinned.x, pinned.y),
        "on the surface the player is drawn on its own Position"
    );
}

/// Nothing standing on the zone surface is drawn inside the base.
///
/// A `SurfaceLink` — the `>` of a Stack entrance — and the anchor itself
/// both carry a `Position` and a `Glyph` and neither is a `Structure` or a
/// `Creature`, so the two existing space gates in `view_entities` look
/// straight past them and they draw at whatever base-space cell their
/// surface coordinates alias onto.
#[test]
fn surface_fixtures_are_not_drawn_inside_the_base() {
    let mut game = game(3211);
    let center = *game.world.get::<Position>(game.player_entity()).unwrap();
    let link = game
        .world
        .spawn((
            SurfaceLink,
            Position {
                x: center.x + 2,
                y: center.y,
            },
            Glyph {
                ch: '>',
                color: GlyphColor::Yellow,
            },
        ))
        .id();
    let anchor = game.world.resource::<AnchorEntity>().0;
    stand_in_base(&mut game);

    let views = game.view_entities(20, 20);

    assert!(
        !views.iter().any(|v| v.entity == link),
        "a Stack entrance stands on the zone surface and must not be drawn in base space"
    );
    assert!(
        !views.iter().any(|v| v.entity == anchor),
        "the anchor stands on the zone surface too — from the inside it is the way out, \
         not a tile of the base"
    );

    // And the other way, so the gate cannot be satisfied by drawing nothing:
    // both are fixtures of the zone map and both belong on it.
    game.world.insert_resource(Locale::Surface);
    let out_here = game.view_entities(20, 20);
    assert!(
        out_here.iter().any(|v| v.entity == link),
        "a Stack entrance is drawn on the zone surface"
    );
    assert!(
        out_here.iter().any(|v| v.entity == anchor),
        "so is the anchor — it is the door, and you have to be able to find it"
    );
}

/// A program standing in the base is not drawn out on the zone surface.
///
/// Base staff are parked around the Home in base-space coordinates every
/// tick by `schedule_base_labour`, which is exactly what makes their
/// `Position` honest — and `drawn_on_surface_map` reads that honesty as
/// "draw it". On the surface those coordinates mean a different tile
/// entirely, so the base's roster shows up scattered across the open grid.
#[test]
fn a_program_standing_in_the_base_is_not_drawn_on_the_zone_surface() {
    let mut game = game(3212);
    let center = *game.world.get::<Position>(game.player_entity()).unwrap();
    let staff = spawn_tamed_on_map(&mut game, center.x + 2, center.y + 1);
    assert_eq!(
        game.program_role(staff),
        Some(ProgramRole::Staff),
        "a program that is neither wielded nor in the party is base staff"
    );
    assert!(
        game.position_is_honest(staff),
        "idle staff carry a live base-space tile — that is the case this test is about"
    );

    let views = game.view_entities(20, 20);

    assert!(
        !views.iter().any(|v| v.entity == staff),
        "an owned program stands in base space and must not be drawn on the zone surface"
    );
}

/// The examine ray agrees with the map about that last one.
///
/// `views::drawn_on_surface_map` is the one rule for "Examine names only
/// what the map draws", and a program the map no longer draws must stop
/// being nameable from the open grid too.
#[test]
fn the_examine_ray_does_not_name_a_program_standing_in_the_base() {
    let mut game = game(3213);
    let center = *game.world.get::<Position>(game.player_entity()).unwrap();
    let staff = spawn_tamed_on_map(&mut game, center.x + 2, center.y);

    let found = game.find_target_in_direction(1, 0, crate::tuning::EXAMINE_RANGE_TILES);

    assert!(
        !matches!(found, Some(InspectTarget::Creature(e)) if e == staff),
        "examine may not name a program that is standing in the base"
    );
}

// ---------------------------------------------------------------------------
// Slice 2: rock you can hit
// ---------------------------------------------------------------------------

/// The party in base space at the eastern edge of the starting pocket, one
/// step from solid rock.
///
/// Deploying the Home is what lays the pocket, so this is the cheapest
/// fixture that has a *frontier* at all — a wall with floor behind it, which
/// is the only place a player ever swings.
fn game_at_the_frontier(seed: u32) -> Game {
    let mut game = game(seed);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("home", 1, 0).unwrap();
    stand_in_base_at(&mut game, crate::tuning::STARTING_POCKET_RADIUS, 0);
    game
}

/// The solid cell east of `game_at_the_frontier`'s standing tile.
const WALL: (i32, i32) = (crate::tuning::STARTING_POCKET_RADIUS + 1, 0);

fn cell(game: &Game, (x, y): (i32, i32)) -> Option<base_grid::BaseCell> {
    game.world.resource::<base_grid::BaseGrid>().cell(x, y)
}

/// The swing count is computed from the constants rather than written down:
/// retuning `BASE_ROCK_DURABILITY` retunes this test with it, which is the
/// point — what is pinned is that a wall takes *the swings its durability
/// implies*, not that it takes three.
#[test]
fn a_wall_opens_after_the_swings_its_durability_implies() {
    let mut game = game_at_the_frontier(3210);
    let player = game.player_entity();
    let per_swing = game.swing_damage(player);
    let swings = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(per_swing);
    assert!(
        swings > 1,
        "a wall that opens on the first swing makes this test vacuous — \
         BASE_ROCK_DURABILITY is below one swing of a level-1 player"
    );

    for swing in 1..swings {
        game.move_player(1, 0);
        assert!(
            game.world
                .resource::<base_grid::BaseGrid>()
                .is_solid(WALL.0, WALL.1),
            "the wall gave way on swing {swing} of {swings}"
        );
    }

    game.move_player(1, 0);
    assert!(
        matches!(cell(&game, WALL), Some(base_grid::BaseCell::Open { .. })),
        "the wall was still standing after the {swings} swings its \
         durability implies"
    );
}

#[test]
fn a_swing_at_rock_does_not_move_the_party() {
    let mut game = game_at_the_frontier(3211);
    let standing = game.base_pos().unwrap();

    game.move_player(1, 0);

    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_solid(WALL.0, WALL.1),
        "this test is about a swing that does not break through"
    );
    assert_eq!(
        game.base_pos().unwrap(),
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
    let north = (0, crate::tuning::STARTING_POCKET_RADIUS + 1);

    let player = game.player_entity();
    game.strike_rock(player, east.0, east.1);
    game.strike_rock(player, north.0, north.1);

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

/// A3's entropy window is measured against this and nothing else, so the
/// tick a cell was opened on has to be the tick the swing landed on.
#[test]
fn an_opened_cell_records_the_tick_it_was_opened() {
    let mut game = game_at_the_frontier(3213);
    let player = game.player_entity();
    let swings = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(game.swing_damage(player));

    for _ in 1..swings {
        game.move_player(1, 0);
    }
    // Read before the breaking swing: the clock advances after it lands.
    let opened_on = game.world.resource::<GameClock>().tick;
    game.move_player(1, 0);

    assert_eq!(
        cell(&game, WALL),
        Some(base_grid::BaseCell::Open {
            mined_at: opened_on
        }),
        "the opened cell records a different tick from the swing that opened it"
    );
}

#[test]
fn a_swing_costs_a_turn() {
    let mut game = game_at_the_frontier(3214);
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
/// at most one fragment — `BASE_MINE_FRAGMENT_CHANCE` is a probability, and
/// `strike_rock` clamps it — against the four a Blank Substrate costs, so
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
    let swings = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(game.swing_damage(player));
    let per_tick_dug = crate::tuning::BASE_MINE_FRAGMENT_CHANCE
        / (swings * crate::tuning::BASE_DIG_TICKS_PER_SWING) as f32;

    let structures = game.world.resource::<StructureDb>();
    let work = structures
        .get("mining_node")
        .expect("the shipped assets deploy a Mining Node")
        .work
        .as_ref()
        .expect("a Mining Node is worked");
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
// Slice 2: laying a VectorStasis Tile
// ---------------------------------------------------------------------------

/// The party standing on a carved, unfloored cell with substrate to hand —
/// the one situation `Game::lay_tile` is for.
fn game_on_open_rock(seed: u32, substrate: u32) -> Game {
    let mut game = game_at_the_frontier(seed);
    let tick = game.current_tick();
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .open(WALL.0, WALL.1, tick);
    stand_in_base_at(&mut game, WALL.0, WALL.1);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), substrate);
    game
}

fn substrate_held(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(ids::BLANK_SUBSTRATE))
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
fn laying_a_tile_spends_exactly_one_substrate() {
    let mut game = game_on_open_rock(3220, 3);

    game.lay_tile()
        .expect("standing on carved rock, holding stock");

    assert_eq!(
        substrate_held(&game),
        2,
        "a tile costs one Blank Substrate and leaves the rest"
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
fn laying_a_tile_without_substrate_refuses_and_spends_nothing() {
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

/// Two different errands for the player — go make a substrate, against you
/// are standing in the wrong place — so they may not read as the same
/// refusal. `NoPost::BoxedIn` against `NoPost::NoRoute`, one level down.
#[test]
fn laying_a_tile_on_floor_refuses_in_different_words_from_having_no_substrate() {
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
        substrate_held(&floored),
        0,
        "the refused second tile must not have spent anything"
    );
}

/// `Game::require_base`, not `require_surface`: laying a tile claims
/// something about where the party is standing, and out on the open grid
/// there is no cell of base space under them at all.
#[test]
fn laying_a_tile_on_the_surface_refuses() {
    let mut game = game_on_open_rock(3225, 1);
    game.world.insert_resource(Locale::Surface);

    let refused = game
        .lay_tile()
        .expect_err("there is no base space out here");

    assert!(
        refused.contains(THE_BASE),
        "a tiling attempt on the surface must meet the base guard, got: {refused}"
    );
    assert_eq!(substrate_held(&game), 1, "and must spend nothing");
}

/// Settled decision 8, and the only thing pinning the player's word for a
/// laid tile: the substrate is raw stock in the store, the tile is what it
/// becomes underfoot. `BaseCell::Floor` stays the code's name for it, the
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
        game.item_name(&ItemId::from(ids::BLANK_SUBSTRATE)),
        "Blank Substrate",
        "and the stock it was pressed from keeps its own name"
    );
}

// ---------------------------------------------------------------------------
// Slice 2: entropy on the frontier
// ---------------------------------------------------------------------------

/// A base with `cut` carved out on the tick the fixture returns, and the
/// party parked back on the exit cell so they are not standing in it.
fn game_with_a_cut_cell(seed: u32, cut: (i32, i32)) -> Game {
    let mut game = game_at_the_frontier(seed);
    let tick = game.current_tick();
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .open(cut.0, cut.1, tick);
    stand_in_base_at(&mut game, BASE_EXIT_CELL.0, BASE_EXIT_CELL.1);
    game
}

/// Winds the clock to `ticks` past the moment `cut` was opened and runs one
/// turn, which is what puts the schedule over it.
fn wait_out(game: &mut Game, ticks: u64) {
    game.world.resource_mut::<GameClock>().tick += ticks;
    game.wait();
}

/// The wall re-knits whole: the cell leaves `BaseGrid` entirely rather than
/// coming back as chipped rock, which is what makes an abandoned frontier
/// cost the swings it cost the first time.
#[test]
fn an_unfloored_cell_reverts_after_the_entropy_window() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3230, cut);

    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS + 1);

    assert_eq!(
        cell(&game, cut),
        None,
        "an abandoned cut cell must be absent from BaseGrid, not chipped rock"
    );
}

/// What keeps "the party is standing inside rock" unreachable *by
/// construction* rather than merely unlikely — the same argument
/// `die_in_the_rock` makes for the Stack, one locale over.
#[test]
fn a_cell_the_party_is_standing_on_never_reverts() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3231, cut);
    stand_in_base_at(&mut game, cut.0, cut.1);

    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS * 3);

    assert!(
        matches!(cell(&game, cut), Some(base_grid::BaseCell::Open { .. })),
        "the cell under the party's feet closed over them"
    );
}

#[test]
fn a_cell_a_posted_program_is_standing_on_never_reverts() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3232, cut);
    // Hand-spawned rather than posted through `work_structure`: what the
    // system reads is a `Task` and a base-space `Position`, and a fixture
    // that walked a real program out to the frontier would be asserting on
    // the scheduler instead.
    let node = spawn_mining_node(&mut game, 0, 1);
    game.world.spawn((
        Task {
            kind: TaskKind::GatherResource,
            target: node,
            progress: 0,
            required: 10,
        },
        Position { x: cut.0, y: cut.1 },
    ));

    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS * 3);

    assert!(
        matches!(cell(&game, cut), Some(base_grid::BaseCell::Open { .. })),
        "the cell under a posted program closed over it"
    );
}

/// Entropy takes the frontier you dug and never floored, and nothing else:
/// a laid tile is permanent, or a base could not be left alone.
#[test]
fn a_floored_cell_never_reverts() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3233, cut);
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(cut.0, cut.1);

    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS * 5);

    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_floor(cut.0, cut.1),
        "laid floor is permanent — entropy takes the frontier, not the base"
    );
}

/// Pins the comparison's direction. The window is how long a cut cell
/// survives, so the tick it reaches it is the last one it is still open on.
#[test]
fn a_cell_reverts_only_after_the_window_not_on_the_tick_it_hits_it() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3234, cut);

    // The clock advances at the *end* of a tick, so the turn this spends
    // runs the schedule with the cell exactly `BASE_ENTROPY_REFILL_TICKS`
    // old — the last tick it is still open on.
    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS);
    assert!(
        matches!(cell(&game, cut), Some(base_grid::BaseCell::Open { .. })),
        "the cell must survive the tick the window is reached on"
    );

    game.wait();
    assert_eq!(cell(&game, cut), None, "and go on the next one");
}

// ---------------------------------------------------------------------------
// Slice 2: what you dug and what you marked, saved
// ---------------------------------------------------------------------------

/// Through a real `save`/`load` and not only the RON round trip: a round
/// trip cannot catch a `#[serde(skip)]`, and a half-cut wall healing on
/// reload is a bug the first play session would hit.
#[test]
fn a_half_cut_wall_survives_a_save_round_trip() {
    let mut game = game_at_the_frontier(3240);
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
    let mut game = game_at_the_frontier(3241);
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
// Slice 2: the marks a plan is drawn out of
// ---------------------------------------------------------------------------

/// Two cells of solid rock well outside the opening pocket, drawn with the
/// far corner *up-left* of the anchor so the box has to be normalised rather
/// than assumed ordered.
const PLAN_ANCHOR: (i32, i32) = (6, 6);
const PLAN_FAR: (i32, i32) = (5, 5);

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
    for (x, y) in [(5, 5), (5, 6), (6, 5), (6, 6)] {
        assert!(
            game.world.resource::<base_grid::BaseGrid>().is_solid(x, y),
            "the fixture must draw its box over solid rock"
        );
    }

    game.toggle_mark_box(PLAN_ANCHOR, PLAN_FAR);

    for cell in [(5, 5), (5, 6), (6, 5), (6, 6)] {
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
    game.toggle_mark_box(PLAN_ANCHOR, PLAN_FAR);

    game.toggle_mark_box(PLAN_ANCHOR, PLAN_FAR);

    for cell in [(5, 5), (5, 6), (6, 5), (6, 6)] {
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

    game.toggle_mark_box(floored, floored);

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

    game.toggle_mark_box(cut, cut);

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
    let mut game = game_at_the_frontier(3254);
    game.toggle_mark_box(WALL, WALL);

    let player = game.player_entity();
    let swings = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(game.swing_damage(player));
    for _ in 0..swings {
        game.move_player(1, 0);
    }
    assert!(
        matches!(cell(&game, WALL), Some(base_grid::BaseCell::Open { .. })),
        "the fixture must have cut the wall through"
    );
    assert!(
        is_marked(&mut game, WALL),
        "the mark did not survive the cut — marked solid means cut it, \
         marked open means floor it, and the crew needs the second half"
    );

    game.move_player(1, 0);
    assert_eq!(
        game.base_pos().unwrap(),
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
    let mut game = game_at_the_frontier(3255);
    let player = game.player_entity();
    let swings = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(game.swing_damage(player));

    for _ in 0..swings {
        game.move_player(1, 0);
    }

    assert!(
        matches!(cell(&game, WALL), Some(base_grid::BaseCell::Open { .. })),
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
    game.toggle_mark_box((7, 7), (5, 5));

    let cells = game.marked_cells();

    let mut sorted = cells.clone();
    sorted.sort_unstable();
    assert_eq!(cells, sorted, "marked_cells came back in query order");
    assert_eq!(cells.len(), 9, "a 3x3 box of solid rock is nine marks");
}

// ---------------------------------------------------------------------------
// Slice 2: the crew
// ---------------------------------------------------------------------------

/// A base with `n` programs on its staff and the party parked back on the
/// exit cell.
///
/// The party's cell matters: everything below is about work that happens
/// while nobody is standing over it, and a fixture that left the player at
/// the frontier could credit the crew with a wall the player's own bumps
/// brought down.
fn base_with_a_crew(seed: u32, n: usize) -> (Game, Vec<Entity>) {
    let mut game = game_at_the_frontier(seed);
    stand_in_base_at(&mut game, BASE_EXIT_CELL.0, BASE_EXIT_CELL.1);
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
    game.toggle_mark_box(cell, cell);
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
    let swings = crate::tuning::BASE_ROCK_DURABILITY.div_ceil(game.swing_damage(worker));
    (swings * crate::tuning::BASE_DIG_TICKS_PER_SWING) as usize + WALK_ALLOWANCE
}

/// Slack for the walk from the parking ring out to the wall, which is a
/// handful of tiles at one tile a tick.
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
const NO_SUBSTRATE: &str = "nothing to floor";

/// The whole claim of the feature: the base grows while you are somewhere
/// else.
#[test]
fn a_crew_cuts_a_marked_wall_without_the_player() {
    let (mut game, staff) = base_with_a_crew(3260, 1);
    mark(&mut game, WALL);
    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_solid(WALL.0, WALL.1),
        "precondition: the marked cell starts solid"
    );

    let wait = ticks_to_cut(&game, staff[0]);
    pass(&mut game, wait);

    assert!(
        matches!(cell(&game, WALL), Some(base_grid::BaseCell::Open { .. })),
        "the crew never cut the wall the player marked"
    );
}

/// Settled decision 4's second half, worked by somebody else: the mark
/// survives the cut, so the same body floors what it just opened — and pays
/// the same one Blank Substrate the player's own tile costs.
#[test]
fn a_crew_floors_a_marked_cell_after_cutting_it() {
    let (mut game, staff) = base_with_a_crew(3261, 1);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 3);
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait);

    assert_eq!(
        cell(&game, WALL),
        Some(base_grid::BaseCell::Floor),
        "the crew cut the cell and left it bare"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        2,
        "a laid tile costs exactly one Blank Substrate, whoever lays it"
    );
    assert!(
        game.dig_site_at(WALL.0, WALL.1).is_none(),
        "a finished cell keeps no dig site"
    );
}

/// **The crew is base labour and pays out of the base's own stores** — the
/// same buffers the stock strip counts, and the same ones `base_holding`
/// reads. Paying out of the player's pack alone left a crew standing on a
/// cut it had just finished while a Depot four tiles away held the
/// substrate, and it said nothing about it: the store the base filled was
/// not the store the crew could spend.
#[test]
fn a_crew_pays_for_its_tile_out_of_the_base_stock() {
    let (mut game, staff) = base_with_a_crew(3280, 1);
    let depot = spawn_machine_at(&mut game, "depot", 2, 0);
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::BLANK_SUBSTRATE), 3);
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait);

    assert_eq!(
        cell(&game, WALL),
        Some(base_grid::BaseCell::Floor),
        "the crew cut the cell and left it bare with a full shelf beside it"
    );
    assert_eq!(
        game.world.get::<Stock>(depot).unwrap().output[&ItemId::from(ids::BLANK_SUBSTRATE)],
        2,
        "a laid tile costs exactly one Blank Substrate, whichever store pays"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        0,
        "the crew reached into the player's pack for a tile the base could pay for"
    );
}

/// The stores are spent before the pack, so what the player is carrying
/// stays theirs to lay by hand — and the fallback is what keeps a base with
/// no machine holding any substrate paving at all.
#[test]
fn a_crew_falls_back_to_the_player_s_pack() {
    let (mut game, staff) = base_with_a_crew(3281, 1);
    let depot = spawn_machine_at(&mut game, "depot", 2, 0);
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::BLANK_SUBSTRATE), 1);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 2);
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait);

    assert_eq!(
        cell(&game, WALL),
        Some(base_grid::BaseCell::Floor),
        "the crew never floored the cell"
    );
    assert!(
        game.world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&ItemId::from(ids::BLANK_SUBSTRATE))
            .is_none(),
        "the shelf is spent first — the pack is the fallback, not the other way round"
    );
    assert_eq!(
        count_item(&game, ids::BLANK_SUBSTRATE),
        2,
        "the player's own substrate was spent while the base still held some"
    );
}

/// A crew with nothing to lay says so **once**, under
/// `systems::set_machine_status`' rule: entering a state is news, staying in
/// it is not. Silence is what this cost the player — a marked cell, a body
/// standing on it, and no way to find out that the only thing missing was
/// stock.
#[test]
fn a_crew_with_no_substrate_says_so_once() {
    let (mut game, staff) = base_with_a_crew(3282, 1);
    mark(&mut game, WALL);

    let wait = ticks_to_cut(&game, staff[0]) + crate::tuning::BASE_DIG_TICKS_PER_SWING as usize;
    pass(&mut game, wait * 4);

    assert!(
        matches!(cell(&game, WALL), Some(base_grid::BaseCell::Open { .. })),
        "a cell floored with no substrate anywhere"
    );
    assert!(
        is_marked(&mut game, WALL),
        "the plan should outlive the shortage — the tile goes down when stock does"
    );
    assert_eq!(
        lines_saying(&game, NO_SUBSTRATE),
        1,
        "the crew's shortage is news once, not once a cycle for the rest of the run"
    );
}

/// Settled decision 7, and the reason digging cannot starve production: dig
/// wants are appended last, so `truncate(staff.len())` cuts them first.
#[test]
fn a_dig_job_never_takes_a_body_off_a_work_order() {
    let (mut game, staff) = base_with_a_crew(3262, 1);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
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
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
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
/// Open ground far enough out that nothing walkable touches it, so a station
/// exists and no route to it does — `NoPost::NoRoute`, the half of the split
/// that is the player's errand.
const STRANDED_STATION: (i32, i32) = (20, 0);
const STRANDED_CELL: (i32, i32) = (21, 0);

fn game_with_an_unroutable_mark(seed: u32) -> (Game, Vec<Entity>) {
    let (mut game, staff) = base_with_a_crew(seed, 1);
    let tick = game.current_tick();
    game.world.resource_mut::<base_grid::BaseGrid>().open(
        STRANDED_STATION.0,
        STRANDED_STATION.1,
        tick,
    );
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

/// Settled decision 6's silent half. The interior of any block you mark has
/// no face to stand at and resolves itself as the shell comes down, so
/// saying so would fire for every buried cell of every plan ever drawn.
#[test]
fn a_dig_site_with_no_exposed_face_never_complains() {
    let (mut game, _) = base_with_a_crew(3265, 1);
    let buried = (20, 20);
    mark(&mut game, buried);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert!(
            game.world
                .resource::<base_grid::BaseGrid>()
                .is_solid(buried.0 + dx, buried.1 + dy),
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
    let corridor = (crate::tuning::STARTING_POCKET_RADIUS + 1)..STRANDED_STATION.0;
    let tick = game.current_tick();
    for x in corridor.clone() {
        game.world
            .resource_mut::<base_grid::BaseGrid>()
            .open(x, 0, tick);
    }
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

/// A cell the crew cut and never floored is taken back by entropy — and
/// taking it back has to mean the wall is whole again.
///
/// A mark outlives the cut, so the `DigSite` outlives it too, holding a
/// `Durability` that is already spent. Without the re-knit clause the next
/// swing lands on nothing and opens the cell for free, which reads as
/// `BASE_ENTROPY_REFILL_TICKS` doing nothing at all.
#[test]
fn a_marked_cell_entropy_took_back_costs_the_whole_wall_again() {
    let mut game = game_at_the_frontier(3267);
    let player = game.player_entity();
    assert!(
        crate::tuning::BASE_ROCK_DURABILITY > game.swing_damage(player),
        "a wall that opens on one swing makes this test vacuous"
    );
    mark(&mut game, WALL);
    while game
        .world
        .resource::<base_grid::BaseGrid>()
        .is_solid(WALL.0, WALL.1)
    {
        game.strike_rock(player, WALL.0, WALL.1);
    }
    stand_in_base_at(&mut game, BASE_EXIT_CELL.0, BASE_EXIT_CELL.1);
    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS + 1);
    assert_eq!(
        cell(&game, WALL),
        None,
        "precondition: the abandoned cut cell must have re-knit"
    );

    game.strike_rock(player, WALL.0, WALL.1);

    assert!(
        game.world
            .resource::<base_grid::BaseGrid>()
            .is_solid(WALL.0, WALL.1),
        "one swing at a re-knit wall opened it for free"
    );
}

/// The middle rung of settled decision 7. Dig jobs sit below work orders
/// *and* below standing jobs, and the order test alone cannot see the
/// difference: an order's wants are built before standing jobs either way,
/// so a plan that only outranked orders would still pass it.
#[test]
fn a_dig_job_never_takes_a_body_off_a_standing_job() {
    let (mut game, staff) = base_with_a_crew(3268, 1);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
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

/// The occupancy rule is "a body is standing here", not "a body holding a
/// `Task` is standing here". Base staff between postings hold none — and a
/// cell reverted under one seals it inside solid rock, where `post_field`
/// gates its own start tile on `BaseGrid::walkable` and can never route it
/// out again.
#[test]
fn a_cell_an_idle_base_staffer_is_standing_on_never_reverts() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3234, cut);
    let staffer = spawn_tamed(&mut game, 10, 3);
    *game.world.get_mut::<Position>(staffer).unwrap() = Position { x: cut.0, y: cut.1 };
    assert_eq!(
        game.base_staff(),
        vec![staffer],
        "precondition: an owned program outside the party is base staff"
    );

    // The bevy schedule rather than a whole turn, because a whole turn runs
    // `schedule_base_labour` first and `park_idle_staff` would walk the body
    // off the cell before entropy could look at it. That gap is the case
    // this test is about: the labour scheduler early-returns on a game over
    // or an active battle, and declines a park tile that is occupied or
    // unwalkable, while the schedule holding `base_entropy_system` keeps
    // running regardless.
    game.world.resource_mut::<GameClock>().tick += crate::tuning::BASE_ENTROPY_REFILL_TICKS * 3;
    game.schedule.run(&mut game.world);

    assert!(
        matches!(cell(&game, cut), Some(base_grid::BaseCell::Open { .. })),
        "the cell under an idle base staffer closed over it"
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
    game.toggle_mark_box(target, target);
    // One swing, so the site holds progress and survives the unmarking.
    game.strike_rock(player, target.0, target.1);
    let site = game
        .dig_site_at(target.0, target.1)
        .expect("a marked, struck wall has a dig site");
    let chipped = game.world.get::<Durability>(site).unwrap().hp;

    let digger = spawn_tamed(&mut game, 30, 3);
    game.world.entity_mut(digger).insert((
        Position {
            x: crate::tuning::STARTING_POCKET_RADIUS,
            y: 0,
        },
        Task {
            kind: TaskKind::Excavate,
            target: site,
            progress: 0,
            required: crate::tuning::BASE_DIG_TICKS_PER_SWING,
        },
    ));
    game.toggle_mark_box(target, target);

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
/// meter on a solid cell holds none — `strike_rock` refills it on the next
/// swing. Keeping one leaves an invisible entity that is drawn nowhere,
/// wanted by nobody, and written to every save from then on.
#[test]
fn clearing_a_mark_leaves_no_site_behind_on_a_reverted_cell() {
    let cut = WALL;
    let mut game = game_with_a_cut_cell(3241, cut);
    // An `Open` cell's site starts at zero: there is nothing left to cut.
    game.toggle_mark_box(cut, cut);
    assert!(
        game.dig_site_at(cut.0, cut.1).is_some(),
        "marking an open cell spawns a site to floor"
    );

    wait_out(&mut game, crate::tuning::BASE_ENTROPY_REFILL_TICKS * 3);
    assert!(
        cell(&game, cut).is_none(),
        "the frontier cell should have reverted to solid"
    );
    game.toggle_mark_box(cut, cut);

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
    game.strike_rock(player, WALL.0, WALL.1);
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
        grid.open(x, y, 0);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: 0, y: 2 };
    let blocked = std::collections::HashSet::new();

    assert_eq!(
        post_reach(&grid, from, target, &blocked, grid.radius()),
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
        grid.open(x, y, 0);
    }
    let target = Position { x: 0, y: 0 };
    let from = Position { x: 0, y: 2 };
    let blocked = std::collections::HashSet::new();

    assert_eq!(
        post_reach(&grid, from, target, &blocked, grid.radius()),
        Err(NoPost::NoRoute),
        "both faces are cut off from the worker, so this is a route problem"
    );

    let mut boxed = base_grid::BaseGrid::default();
    boxed.open(from.x, from.y, 0);
    assert_eq!(
        post_reach(&boxed, from, target, &blocked, boxed.radius()),
        Err(NoPost::BoxedIn),
        "nothing can stand beside it at all, which is a digging problem"
    );
}
