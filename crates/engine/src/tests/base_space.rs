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

#[test]
fn deploying_a_structure_is_a_base_action() {
    is_a_base_action(
        3100,
        "deploying",
        |game| give(game, &ItemId::from(ids::CORE_FRAGMENT), 20),
        |game, ()| game.place_structure("home", 1, 0),
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
            place_home(game, 0, 1);
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

/// The one site of the eleven that kept `require_surface`. `rest` logs its
/// refusal rather than returning it, so the probe is whether the world moved:
/// a rest that runs advances `REST_TICKS` at once, and a refused one spends
/// nothing at all.
///
/// **Contested.** `rest` demands a structure whose def sets `enables_rest`
/// within reach, and Home is the only shipped one — so once the base's
/// structures move into base space (slice-1 Task 6), the only place a rest
/// can legally happen is the one locale this guard refuses. See the seam
/// entry in `docs/seams.md` and the task report; the table this test
/// implements is the spec's, and flipping it is a one-line change here.
#[test]
fn resting_is_a_surface_action() {
    let rested = |at: At| {
        let mut game = game(3110);
        spawn_rest_structure_at_player(&mut game);
        match at {
            At::Surface => {}
            At::Base => stand_in_base(&mut game),
            At::Stack => descend(&mut game),
        }
        let before = game.current_tick();
        game.rest();
        game.current_tick() > before
    };

    assert!(
        rested(At::Surface),
        "resting must still work where the guard permits it"
    );
    assert!(!rested(At::Base), "resting must be refused in base space");
    assert!(!rested(At::Stack), "resting must be refused underground");
}

/// And the refusals say which guard caught them, so a site quietly moved to
/// `require_base` could not pass the test above by refusing everywhere.
#[test]
fn a_refused_rest_names_the_open_grid_it_wanted() {
    let mut game = game(3111);
    spawn_rest_structure_at_player(&mut game);
    stand_in_base(&mut game);

    game.rest();

    let said = game
        .message_log(200)
        .into_iter()
        .any(|line| line.text.contains(OPEN_GRID));
    assert!(
        said,
        "a rest refused in base space must name the surface guard, log: {:?}",
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

    from_inside_the_base(&mut game, |g| {
        give(g, &ItemId::from(ids::CORE_FRAGMENT), 20);
        g.place_structure("home", 1, 0)
    })
    .unwrap();

    assert_eq!(
        game.structure_report().len(),
        1,
        "only the placed Home should be on the roster, not the anchor standing beside it"
    );
}
