//! A Teardown Rig holds its own tool: `Game::install_rig_tool`,
//! `Game::remove_rig_tool` and the strip's move off the player's slots.
//!
//! The rig is the base's extractor and the player's own slots are the
//! player's — the two stopped sharing a tool here.

use super::support::*;
use crate::components::{Hopper, HopperEntry, Inventory, Tools};
use crate::items::DownedProgram;
use crate::tools::ToolId;
use crate::*;

fn tool(id: &str) -> ToolId {
    ToolId(id.to_string())
}

fn program(level: u32) -> DownedProgram {
    DownedProgram {
        species: "scrapper".to_string(),
        level,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    }
}

fn carriers(game: &Game, id: &str) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::tool(&tool(id)))
}

fn fitted(game: &Game, rig: Entity) -> Option<ToolId> {
    game.world
        .get::<Hopper>(rig)
        .and_then(|h| h.standing_tool.clone())
}

/// The player beside a rig at (3,3), carrying one carrier of each named
/// tool. Nothing is in the player's own `Tools` slots — a rig must not need
/// them, and every test here would pass for the wrong reason if it did.
fn player_beside_a_rig_carrying(tools: &[&str]) -> (Game, Entity) {
    let mut game = Game::new(4310, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let rig = spawn_machine_at(&mut game, "teardown_rig", 3, 3);
    let player = game.player_entity();
    for id in tools {
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::tool(&tool(id)), 1);
    }
    (game, rig)
}

/// Asserts `call` changed nothing a fit or a pull could have changed — the
/// per-refusal bar `Game::extract_program`'s own ladder is held to, since
/// one test over one refusal proves nothing about the others.
fn spends_nothing(
    game: &mut Game,
    rig: Entity,
    call: impl FnOnce(&mut Game) -> Result<(), String>,
) {
    let player = game.player_entity();
    let before_pack = game.world.get::<Inventory>(player).unwrap().items.clone();
    let before_tool = fitted(game, rig);
    assert!(call(game).is_err(), "the call should have been refused");
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before_pack,
        "a refusal spent something out of the pack"
    );
    assert_eq!(fitted(game, rig), before_tool, "a refusal moved the tool");
}

#[test]
fn fitting_a_tool_spends_exactly_one_carrier() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);

    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();

    assert_eq!(fitted(&game, rig), Some(tool("salvage_clamp")));
    assert_eq!(carriers(&game, "salvage_clamp"), 0, "the carrier went in");
}

/// **The rig's tool is not the player's.** A rig runs on what was carried
/// into it, and the player's own slots are read nowhere in the strip — the
/// coupling this feature exists to cut. Before it, `run_teardown_rigs`
/// resolved a queued entry against `installed_tools()`, so pulling your own
/// tool starved every rig in the base and nothing said so.
#[test]
fn a_rig_strips_with_its_own_tool_while_the_player_holds_none() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    let player = game.player_entity();
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    // `Game::new` grants the starter tool into the player's own slot, so
    // emptying it by hand is what makes this test say anything: with the
    // grant left standing the rig would run on the player's Salvage Clamp
    // exactly as it used to, and the assertion below would pass against the
    // coupling it exists to catch.
    game.world.get_mut::<Tools>(player).unwrap().0.clear();

    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: rig,
        progress: 0,
        required: 1,
    });
    game.world
        .get_mut::<Hopper>(rig)
        .unwrap()
        .queue
        .push(HopperEntry {
            program: program(5),
            tool: tool("salvage_clamp"),
        });
    stand_ample_grid_supply(&mut game);

    for _ in 0..200 {
        game.tick();
        if game.world.get::<Hopper>(rig).unwrap().queue.is_empty() {
            break;
        }
    }
    assert!(
        game.world.get::<Hopper>(rig).unwrap().queue.is_empty(),
        "the rig should have worked the program through on its own tool"
    );
    let output: u32 = game.world.get::<Stock>(rig).unwrap().output.values().sum();
    assert!(output > 0, "and paid out into its own buffer");
}

#[test]
fn pulling_a_tool_hands_the_carrier_back() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();

    game.remove_rig_tool(rig).unwrap();

    assert_eq!(fitted(&game, rig), None);
    assert_eq!(
        carriers(&game, "salvage_clamp"),
        1,
        "the object that went in comes back out — unlike a player slot, \
         which holds knowledge and hands back nothing"
    );
}

#[test]
fn swapping_hands_back_the_old_and_spends_the_new() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp", "core_tap"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();

    game.install_rig_tool(rig, &tool("core_tap")).unwrap();

    assert_eq!(fitted(&game, rig), Some(tool("core_tap")));
    assert_eq!(carriers(&game, "core_tap"), 0);
    assert_eq!(carriers(&game, "salvage_clamp"), 1, "the old one came back");
}

/// A stamp left standing is a body stripped with a tool that is not in the
/// machine, which no line on any screen could explain.
#[test]
fn swapping_retools_what_is_already_in_the_hopper() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp", "core_tap"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    game.world
        .get_mut::<Hopper>(rig)
        .unwrap()
        .queue
        .push(HopperEntry {
            program: program(5),
            tool: tool("salvage_clamp"),
        });

    game.install_rig_tool(rig, &tool("core_tap")).unwrap();

    assert_eq!(
        game.world.get::<Hopper>(rig).unwrap().queue[0].tool,
        tool("core_tap"),
    );
}

/// Pulling the tool stops the rig dead — the programs already fetched stay
/// where they are rather than draining on a tool that has left the base.
#[test]
fn pulling_the_tool_stalls_a_loaded_rig_without_losing_the_queue() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: rig,
        progress: 0,
        required: 1,
    });
    game.world
        .get_mut::<Hopper>(rig)
        .unwrap()
        .queue
        .push(HopperEntry {
            program: program(5),
            tool: tool("salvage_clamp"),
        });
    stand_ample_grid_supply(&mut game);

    game.remove_rig_tool(rig).unwrap();
    for _ in 0..50 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<Hopper>(rig).unwrap().queue.len(),
        1,
        "the program stays in the hopper"
    );
    assert_eq!(
        game.world.get::<crate::components::MachineStatus>(rig),
        Some(&crate::components::MachineStatus::Starved),
        "and the rig says it has nothing to work with"
    );
}

// ---------------------------------------------------------------------------
// The refusals, one test each: a single test over one of them passes against
// every path that never spends anyway.
// ---------------------------------------------------------------------------

#[test]
fn fitting_with_no_rig_adjacent_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    // Walk off the rig's cell without leaving base space.
    stand_in_base_at(&mut game, 9, 9);
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("salvage_clamp"))
    });
}

#[test]
fn fitting_an_unknown_tool_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("no_such_tool"))
    });
}

/// A `Routines` tool writes into a slot and a `Gear` tool rolls a drop
/// table; neither has anywhere to put its answer in a machine's output, so
/// both are refused at the fit rather than at the strip.
#[test]
fn fitting_a_routines_tool_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["routine_reader"]);
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("routine_reader"))
    });
}

#[test]
fn fitting_a_gear_tool_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["harness_puller"]);
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("harness_puller"))
    });
}

#[test]
fn fitting_a_tool_with_no_carrier_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&[]);
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("salvage_clamp"))
    });
}

#[test]
fn fitting_the_tool_already_in_the_rig_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp", "salvage_clamp"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("salvage_clamp"))
    });
}

#[test]
fn fitting_during_a_battle_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    spends_nothing(&mut game, rig, |g| {
        g.install_rig_tool(rig, &tool("salvage_clamp"))
    });
}

#[test]
fn pulling_from_an_untooled_rig_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    spends_nothing(&mut game, rig, |g| g.remove_rig_tool(rig));
}

#[test]
fn pulling_with_no_rig_adjacent_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    stand_in_base_at(&mut game, 9, 9);
    spends_nothing(&mut game, rig, |g| g.remove_rig_tool(rig));
}

// ---------------------------------------------------------------------------
// Destruction: both paths, and neither fails to compile if it is missed.
// ---------------------------------------------------------------------------

#[test]
fn demolishing_a_rig_hands_its_tool_back() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    assert_eq!(carriers(&game, "salvage_clamp"), 0);

    game.remove_structure(rig).unwrap();

    assert_eq!(
        carriers(&game, "salvage_clamp"),
        1,
        "taking the building down must not take the tool with it"
    );
}

#[test]
fn a_rig_destroyed_by_a_raid_hands_its_tool_back() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();
    assert_eq!(carriers(&game, "salvage_clamp"), 0);
    // `damage_structure` returns early without one, so a rig hand-spawned
    // by the fixture is immune to the path this test is about.
    game.world
        .entity_mut(rig)
        .insert(crate::components::Durability { hp: 1, max_hp: 1 });

    game.damage_structure(rig, 9_999, "The Teardown Rig");

    assert_eq!(
        carriers(&game, "salvage_clamp"),
        1,
        "the second destruction path owes the same return"
    );
}

// ---------------------------------------------------------------------------
// The screen's data
// ---------------------------------------------------------------------------

#[test]
fn the_view_names_the_fitted_tool_and_offers_what_the_pack_can_fit() {
    let (mut game, rig) = player_beside_a_rig_carrying(&[
        "salvage_clamp",
        "core_tap",
        "routine_reader",
        "harness_puller",
    ]);
    game.install_rig_tool(rig, &tool("salvage_clamp")).unwrap();

    let view = game.rig_tool_view(rig).expect("the player is beside it");
    assert_eq!(view.tile, (3, 3));
    assert_eq!(
        view.installed.as_ref().map(|r| r.id.clone()),
        Some(tool("salvage_clamp"))
    );
    let offered: Vec<String> = view.candidates.iter().map(|r| r.id.0.clone()).collect();
    assert_eq!(
        offered,
        vec!["core_tap".to_string()],
        "the fitted one is not offered again, and a rig runs neither a \
         Routines nor a Gear tool"
    );
}

/// `DepotFilterView`'s self-closing rule: the screen acts on a machine, so
/// the view has to stop answering the moment that machine stops being one
/// the player is standing at.
#[test]
fn the_view_closes_when_the_rig_stops_standing() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    assert!(game.rig_tool_view(rig).is_some());

    game.remove_structure(rig).unwrap();

    assert!(game.rig_tool_view(rig).is_none());
}

#[test]
fn the_view_closes_when_the_player_walks_away() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["salvage_clamp"]);
    assert!(game.rig_tool_view(rig).is_some());

    stand_in_base_at(&mut game, 9, 9);

    assert!(game.rig_tool_view(rig).is_none());
}

/// The tool is already a save field (`StructureSave::standing_tool`), so
/// this costs no `SAVE_FORMAT_VERSION` bump — but nothing else asserts that
/// the *fitted* one survives, and a rig that forgets its tool on reload
/// reads as the base being broken.
#[test]
fn a_fitted_tool_survives_a_save_and_load() {
    let (mut game, rig) = player_beside_a_rig_carrying(&["core_tap"]);
    let _ = rig;
    game.install_rig_tool(rig, &tool("core_tap")).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_rig_tool_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let reloaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let standing: Vec<ToolId> = reloaded
        .world
        .iter_entities()
        .filter_map(|e| e.get::<Hopper>()?.standing_tool.clone())
        .collect();
    assert_eq!(standing, vec![tool("core_tap")]);
}
