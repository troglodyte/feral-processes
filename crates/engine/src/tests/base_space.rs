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
        |game, ()| game.queue_work_order(ItemId::from("routine_disk"), 3),
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

/// A Recharger's radius is measured from the player's `Position`, and that
/// `Position` is pinned to the anchor tile for the whole of a visit to base
/// space — so a Recharger standing near the anchor would otherwise top the
/// party up while they are out of phase, every tick, for free.
///
/// Asserted against the surface half in the same test, since the base half
/// alone passes against a system that regenerates nothing anywhere.
#[test]
fn power_regen_does_not_reach_the_party_in_base_space() {
    let restored = |at: At| {
        let mut game = game(3114);
        spawn_recharger_node(&mut game, 0, 0);
        let player = game.player_entity();
        *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(20.0);
        if let At::Base = at {
            stand_in_base(&mut game);
        }
        let before = game.world.get::<PowerReserve>(player).unwrap().get();
        game.tick();
        game.world.get::<PowerReserve>(player).unwrap().get() - before
    };

    assert!(
        restored(At::Surface) > 0.0,
        "the same fixture on the surface must still regenerate"
    );
    assert!(
        restored(At::Base) <= 0.0,
        "a Recharger beside the anchor must not reach a party out of phase"
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

/// Solid rock is not walkable, and shoving at it is not a turn. Both halves
/// of the pin are asserted: base-space coordinates unchanged, *and* the
/// surface `Position` unchanged — without the second, a `move_player` that
/// never dispatched on locale at all would walk the party across the zone
/// map and still pass.
#[test]
fn walking_into_solid_rock_is_refused_and_costs_no_turn() {
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
    assert_eq!(game.current_tick(), tick, "a refused step costs no turn");
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

/// A refused step still breaks off a posted job, exactly as shoving at a
/// wall on the zone surface does — `move_player` drops the job before it
/// looks at what is in the way, "since either way you stopped working to do
/// it", and `Game::work_structure` promises the player as much when it
/// posts.
///
/// The turn and the job point opposite ways at this one site — the step
/// costs nothing, the job ends anyway — so this is the only thing that says
/// the ordering inside `move_in_base` was chosen rather than fallen out of
/// an early return.
#[test]
fn shoving_at_solid_rock_still_breaks_off_a_job() {
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
        "the fixture's step must really be refused, or this proves nothing"
    );
    assert_eq!(game.current_tick(), tick, "and must still cost no turn");
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
