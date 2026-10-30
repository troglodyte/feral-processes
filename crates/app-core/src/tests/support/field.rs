//! Fixtures shared by the app-core tests: the field, outposts and routines.

use feral_processes_engine::resources::Locale;
use feral_processes_engine::save::{self};
use feral_processes_engine::stack::{Dir, FrameSpec, generate};

use super::*;
use crate::*;

pub(crate) fn test_app(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let saves_dir = std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_saves"));
    let history_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}.log"));
    let profile_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_profile.ron"));
    let _ = std::fs::remove_file(&profile_path);
    let mut app = App::new(
        assets_dir.clone(),
        saves_dir,
        history_path,
        profile_path,
        arenas_dir(),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_test_{seed}_telemetry.jsonl"
        )),
    );
    // A fresh profile fights on the battle map; most tests here drive the
    // group model, and a tactical test turns it back on itself.
    app.profile.tactical_battles = false;
    app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
    // A new run queues the onboarding chain's first briefing, which would
    // otherwise take the screen the moment any test reached `Playing` — the
    // engine's own notification tests drain for the same reason. A test
    // about the briefing itself queues its own.
    if let Some(game) = &mut app.game {
        while game.take_notification().is_some() {}
    }
    app.mode = Mode::Playing;
    app
}

/// A game where the player owns `count` programs parked well outside
/// `MENU_SCAN_RADIUS` — a cronjob worker left at a far-flung node, say.
/// Built by editing a save and reloading it, since the engine deliberately
/// exposes no way to hand-place a tamed program from outside the crate.
pub(crate) fn app_owning_distant_programs(seed: u32, count: i32) -> App {
    distant_programs(seed, |game| {
        let species = game
            .species_defs()
            .into_iter()
            .find(|d| !d.is_hybrid())
            .unwrap()
            .id
            .clone();
        (0..count).map(|_| species.clone()).collect()
    })
}

/// The same program the cargo fixture gives, developed: `level` and `ring`
/// written straight onto its save record and reloaded.
///
/// Through the save because nothing in the engine's public API can level a
/// companion or open a ring for free — `award_party_xp` is `pub(crate)` and
/// `open_kernel_ring` charges for it, which is the point of both. Three
/// Privilege Rings ride along in cargo so the ring half of the screen has
/// something to spend.
pub(crate) fn app_owning_a_developed_program(seed: u32, level: u32, ring: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app =
        app_owning_a_program_and_an_extractor_with_cargo(seed, &[], &[("privilege_ring", 3)]);
    let path = scratch_path("developed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    for c in data.creatures.iter_mut().filter(|c| c.tamed) {
        c.level = level;
        c.ring = ring;
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// An app owning one seated program that has `banked` points, held or not,
/// written onto its save record and reloaded - `app_owning_a_developed_program`'s
/// route, for the same reason. Returns the program.
pub(crate) fn app_owning_a_program_with_points(
    seed: u32,
    banked: u32,
    hold: bool,
) -> (App, Entity) {
    let assets_dir = test_assets_dir();
    let mut app = app_owning_distant_programs(seed, 1);
    let path = scratch_path("program_points", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    for c in data.creatures.iter_mut().filter(|c| c.tamed) {
        // A record with a base is what load restores as seated. Its attack
        // is not the player's, so a preview that assumed the player's base
        // would read a different figure.
        let mut base = feral_processes_engine::progression::DerivedBase::player();
        base.atk += 7;
        c.base = Some(base);
        c.stat_points = banked;
        c.hold_points = hold;
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    // The player is the first subject, so the program is the second.
    let program = app.game.as_mut().unwrap().manifest_subjects()[1];
    (app, program)
}

/// An app whose player already holds `programs` in
/// `components::DownedPrograms`, without playing to a kill for one — the
/// engine exposes no way to hand-place one from outside the crate,
/// `distant_programs`' reason for the same save round trip.
pub(crate) fn app_holding_downed_programs(
    seed: u32,
    programs: Vec<feral_processes_engine::items::DownedProgram>,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("downed_programs", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.downed_programs = programs;
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// The party in base space beside nothing, an empty pack, and one downed
/// program in it: the one state where the transfer has something to offer
/// and no neighbouring structure to aim at.
pub(crate) fn app_in_base_holding_a_downed_program(seed: u32) -> App {
    let mut app = app_beside_depots(seed, 0, 0, &[]);
    let path = scratch_path("base_downed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.downed_programs = vec![feral_processes_engine::items::DownedProgram {
        species: "scrapper".to_string(),
        level: 4,
        rarity: feral_processes_engine::components::Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    }];
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// A founded base with a Holding Cell at base-space `(2, 0)` (pen `(3, 1)`),
/// the party standing beside it, `programs` in the downed store and
/// `protocols` Reinitialization Protocols in the pack — `Mode::PinPrisoner`'s
/// own fixture. Zero protocols is a legitimate call: the refusal test wants
/// a record and an empty pack.
pub(crate) fn app_beside_a_holding_cell(
    seed: u32,
    programs: Vec<feral_processes_engine::items::DownedProgram>,
    protocols: u32,
) -> App {
    let mut app = app_owning_a_program_and_a_station_of("holding_cell", "holding_cell", seed);
    let path = scratch_path("holding_cell_records", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.downed_programs = programs;
    if protocols > 0 {
        data.player.inventory.push((
            ItemId::from(feral_processes_engine::items::ids::REINITIALIZATION_PROTOCOL),
            protocols,
        ));
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &test_assets_dir()).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    stand_beside_the_extractor(&mut app);
    app
}

/// `app_holding_downed_programs` plus a Teardown Rig standing beside the
/// party, in base space — the two preconditions the rig verbs need.
///
/// The rig sits at base cell `(1, 0)`, orthogonally east of the `(0, 0)`
/// the locale stands the party in, because `Game::adjacent_teardown_rig` is
/// an adjacency question and not an ownership one. Built through the same
/// save round trip for that fixture's reason: the engine exposes no way to
/// hand-place a downed program from outside the crate, and staging the rig
/// through the build flow would need the Teardown research first.
pub(crate) fn app_beside_a_teardown_rig_holding(
    seed: u32,
    programs: Vec<feral_processes_engine::items::DownedProgram>,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("teardown_rig", seed);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.downed_programs = programs;
    stand_save_on_field(&mut data, 0, 0);
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "teardown_rig".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// The party in base space beside a Splice Rig at `(1, 0)`, carrying
/// `pack`. Built through the save round trip for
/// `app_beside_a_teardown_rig_holding`'s reason: the rig's research and
/// build flow are not what these tests are about.
pub(crate) fn app_beside_a_splice_rig_carrying(seed: u32, pack: &[(&str, u32)]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("splice_rig", seed);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = pack.iter().map(|(id, n)| (ItemId::from(*id), *n)).collect();
    stand_save_on_field(&mut data, 0, 0);
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "splice_rig".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An app on `Mode::Inventory` with at least ten distinct cargo rows, so
/// a stray lowercase `d` has a real row under the highlight to act on
/// rather than falling out on an empty list regardless of what key was
/// pressed. A test asserting only that `d` leaves the mode unchanged on a
/// short inventory would pass against several wrong implementations.
pub(crate) fn app_on_inventory_with_many_items(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("many_items", seed);
    let game = app.game.as_mut().unwrap();
    let items: Vec<ItemId> = game
        .item_defs()
        .iter()
        .take(10)
        .map(|def| def.id.clone())
        .collect();
    assert!(
        items.len() >= 10,
        "test premise: the shipped catalogue defines at least ten items"
    );
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Merged, `app_owning_a_program_and_an_extractor_deep`'s reason: pushing a
    // second row for something the starting kit already carries is
    // invisible, since `Inventory::count` reads the first match and
    // `Game::load` restores the `Vec` verbatim.
    for item in items {
        if !data.player.inventory.iter().any(|(i, _)| *i == item) {
            data.player.inventory.push((item, 1));
        }
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Surrounds the player with a hostile on all eight neighbours — `Game::
/// travel_step`'s own `no_route_when_boxed_in_by_hostiles` fixture, built
/// the app-core way since the engine's `World` is private to it: a route's
/// cost function refuses every cell a hostile stands on, so no route out
/// exists at all, whatever the goal.
pub(crate) fn box_in_player_with_hostiles(app: &mut App) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("box_in", 0);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let ring: Vec<(i32, i32)> = [-1, 0, 1]
        .into_iter()
        .flat_map(|dx| [-1, 0, 1].into_iter().map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| (dx, dy) != (0, 0))
        .map(|(dx, dy)| (px + dx, py + dy))
        .collect();
    data.creatures.retain(|c| !ring.contains(&c.position));
    for pos in ring {
        data.creatures
            .push(wild_creature_save(species.clone(), pos));
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// A settlement one tile east of the player — `test_assets_dir`'s catalogue
/// is never consulted, since a hand-built def is all a fixture that never
/// opens the market or job board needs.
///
/// The engine exposes `resources::Settlements`/`resources::KnownSettlement`
/// publicly and restores an entity from it on load
/// (`Game::restore_settlements`), which is the one door app-core has onto a
/// settlement: unlike a creature or a structure, there is no
/// `Game::place_structure`-shaped public setter, and the engine's own
/// `tests::support::place_settlement` is `pub(super)` to that crate alone.
/// So this goes through the same save-edit-reload trick every other fixture
/// here uses, writing `SaveData::settlements` directly.
///
/// Clears the footprint and the ring round it of a creature, a nest or a
/// Stack entrance, any of which would answer the bump ladder *before*
/// `find_settlement_at` is ever reached — the settlement arm is the fourth, after the wild-creature, nest
/// and surface-link arms, and any of those three intercepting the press
/// would make this fixture's tests exercise the wrong feature.
pub(crate) fn place_settlement_east_of_player(
    app: &mut App,
) -> (
    feral_processes_engine::settlements::SettlementKey,
    (i32, i32),
) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("settlement", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // Two east, not one: a town's footprint is a square around its tile, so
    // at one east its west column would cover the player and the load's
    // displacement would move them. Its nearest cell is then the one a
    // step east bumps.
    let target = (px + 2, py);
    let near = |(x, y): (i32, i32)| (x - target.0).abs().max((y - target.1).abs()) <= 2;
    data.creatures.retain(|c| !near(c.position));
    data.nests.retain(|n| !near(n.position));
    data.link_sites.retain(|&site| !near(site));
    let key = feral_processes_engine::settlements::SettlementKey { rx: 0, ry: 0 };
    data.settlements.0.insert(
        key,
        feral_processes_engine::resources::KnownSettlement {
            tile: target,
            def: generic_settlement_def(),
            visited: false,
        },
    );
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    (key, target)
}

/// `place_settlement_east_of_player`'s shape one landmark over — an outpost
/// is a record with no entity (`resources::Outposts`, not a spawned one), so
/// it is written straight into the save rather than founded through
/// `Game::found_outpost`, which would refuse a tile this close to the
/// player's own spawn (`OUTPOST_MIN_ANCHOR_DISTANCE`). Returns the tile.
pub(crate) fn place_outpost_east_of_player(app: &mut App) -> (i32, i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("outpost", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let target = (px + 1, py);
    data.creatures.retain(|c| c.position != target);
    data.nests.retain(|n| n.position != target);
    data.link_sites.retain(|&site| site != target);
    data.settlements.0.retain(|_, s| s.tile != target);
    data.outposts.push(save::OutpostSave {
        tile: target,
        biome: feral_processes_engine::world::Biome::Deadlock,
        growth: 0,
        integrity: feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
        stock: Vec::new(),
        stale_ticks: 0,
        cycle_progress: 0,
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    target
}

/// `place_outpost_east_of_player`'s shape with `integrity` pre-damaged —
/// `Game`'s `world` field is private with no accessor (the architectural
/// rule this repo enforces at the compiler), so a test that needs a damaged
/// outpost writes it through the save file exactly as the healthy fixture
/// does, rather than reaching into the ECS from app-core.
pub(crate) fn place_damaged_outpost_east_of_player(app: &mut App, integrity: u32) -> (i32, i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("outpost_damaged", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let target = (px + 1, py);
    data.creatures.retain(|c| c.position != target);
    data.nests.retain(|n| n.position != target);
    data.link_sites.retain(|&site| site != target);
    data.settlements.0.retain(|_, s| s.tile != target);
    data.outposts.push(save::OutpostSave {
        tile: target,
        biome: feral_processes_engine::world::Biome::Deadlock,
        growth: 0,
        integrity,
        stock: Vec::new(),
        stale_ticks: 0,
        cycle_progress: 0,
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    target
}

/// Merges `items` into the player's pack via a save round trip — extended
/// row by row rather than assigned over, `app_owning_a_program_and_a_
/// extractor_with_cargo`'s reason one screen over: replacing the whole
/// inventory would silently delete whatever `Game::new`'s starting kit
/// already granted.
pub(crate) fn give_player_items(app: &mut App, items: &[(&str, u32)]) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("give_player_items", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    for (item, qty) in items {
        let id = feral_processes_engine::items::ItemId::from(*item);
        match data.player.inventory.iter_mut().find(|(i, _)| *i == id) {
            Some((_, have)) => *have += qty,
            None => data.player.inventory.push((id, *qty)),
        }
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// `place_outpost_east_of_player` plus one ordinary base-staff program, for
/// a test driving `Mode::OutpostPost`'s picker by keypress alone — nothing
/// here needs the program's `Entity`, since the picker is walked with
/// letters the way a player would.
pub(crate) fn place_outpost_with_a_staff_program_east_of_player(app: &mut App) -> (i32, i32) {
    place_outpost_with_staff_programs_east_of_player(app, 1)
}

/// `place_settlement_east_of_player`, out of reach — twelve tiles east, so
/// `Game::settlement_reach` (Chebyshev 1) answers no while `x` still would.
/// The tile needs no clearing: nothing walks into it.
pub(crate) fn place_settlement_far_from_player(
    app: &mut App,
) -> feral_processes_engine::settlements::SettlementKey {
    let assets_dir = test_assets_dir();
    let path = scratch_path("settlement_far", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let key = feral_processes_engine::settlements::SettlementKey { rx: 0, ry: 0 };
    data.settlements.0.insert(
        key,
        feral_processes_engine::resources::KnownSettlement {
            tile: (px + 12, py),
            def: generic_settlement_def(),
            visited: false,
        },
    );
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    key
}

/// A settlement def with every required field filled in — there is no
/// `#[serde(default)]` on `SettlementDef`, so a fixture that skipped one
/// would not compile, and this is the one place a test settlement's prose
/// is authored rather than borrowed from the shipped catalogue.
pub(super) fn generic_settlement_def() -> feral_processes_engine::settlements::SettlementDef {
    feral_processes_engine::settlements::SettlementDef {
        id: "test_settlement".to_string(),
        name: "Test Settlement".to_string(),
        blurb: "A settlement placed for a test.".to_string(),
        kind: feral_processes_engine::settlements::SettlementKind::Server,
        specialty: feral_processes_engine::settlements::Specialty::Materials,
        temperament: feral_processes_engine::settlements::Temperament::Open,
    }
}

/// A game where the player owns one tamed program carrying `routines` and
/// has an Extractor standing beside the party and a Workbench far off, so
/// the extraction flow has both of its preconditions. Built by editing a save and reloading it, for the same
/// reason `app_owning_distant_programs` is.
pub(crate) fn app_owning_a_program_and_an_extractor(seed: u32, routines: &[&str]) -> App {
    app_owning_a_program_and_an_extractor_with_cargo(seed, routines, &[])
}

/// The same, plus exactly `cargo` in the player's inventory — the refactor
/// flow needs a program *and* something to spend on it, and the second half
/// is the one that changes during a run.
pub(crate) fn app_owning_a_program_and_an_extractor_with_cargo(
    seed: u32,
    routines: &[&str],
    cargo: &[(&str, u32)],
) -> App {
    app_owning_a_program_and_an_extractor_deep(seed, routines, cargo, false)
}

/// A base with a founded Home, one program on the base staff, and a
/// standing Research Station — `Mode::PinSubject`'s own fixture,
/// `app_owning_a_program_and_an_extractor`'s shape with the structure swapped
/// for one that declares `studies`. The Station is written straight into
/// the save rather than raised through `Game::place_structure`, which would
/// file a build request and spend the one program this fixture exists to
/// keep free to pin — `app_owning_a_program_and_an_extractor_deep`'s own
/// reason for doing the same with its Extractor.
///
/// Leaves `data.locale` untouched, `app_owning_a_program_and_an_extractor`'s
/// own shape: a caller that needs the party inside calls `stand_in_base`
/// itself, since the row this fixture exists for is `Locality::Base`.
pub(crate) fn app_owning_a_program_and_a_research_station(seed: u32) -> App {
    app_owning_a_program_and_a_station_of("research_station", "research_node", seed)
}

/// A game where the player stands next to a Black Market — the shipped
/// trader that buys programs as well as items — holding exactly
/// `inventory`, and owning one tamed program so the trader's program rows
/// are populated too. Built by editing a save and reloading it, for the
/// same reason `app_owning_a_program_and_an_extractor` is: staging a trading
/// post through the build flow needs a Home, build clearance and 16 Core
/// Fragments, and the player starts with 5.
pub(crate) fn app_at_a_trading_post(seed: u32, inventory: &[(&str, u32)]) -> App {
    app_at_trading_posts(seed, inventory, 1)
}

/// Where every fixture founds its base: the field's origin, where a new run's
/// party starts, so a structure written into a save at `(1, 0)` stands beside
/// the Home.
pub(crate) const TEST_HOME_CELL: (i32, i32) = (0, 0);

/// The fixture pocket `found_the_base` floors around the Home: a 19x9 box
/// with its corners cut, test ground so a fixture has room to stand machines
/// without growing the floor tile by tile.
pub(crate) const TEST_POCKET_HALF_WIDTH: i32 = 9;
pub(crate) const TEST_POCKET_HALF_HEIGHT: i32 = 4;
pub(super) const TEST_POCKET_CORNER_CUT: i32 = 2;

/// Every cell of the fixture pocket.
pub(crate) fn test_pocket_cells() -> Vec<(i32, i32)> {
    let (w, h) = (TEST_POCKET_HALF_WIDTH, TEST_POCKET_HALF_HEIGHT);
    (-h..=h)
        .flat_map(|dy| (-w..=w).map(move |dx| (dx, dy)))
        .filter(|(dx, dy): &(i32, i32)| dx.abs() + dy.abs() <= w + h - TEST_POCKET_CORNER_CUT)
        .map(|(dx, dy)| (TEST_HOME_CELL.0 + dx, TEST_HOME_CELL.1 + dy))
        .collect()
}

/// Writes the party's place into a save: on the field at `(x, y)`.
pub(crate) fn stand_save_on_field(data: &mut save::SaveData, x: i32, y: i32) {
    data.locale = Locale::Surface;
    data.player.position = (x, y);
}

/// Founds the run's base: deploys the first Home through the public API on
/// the party's tile, then floors the fixture pocket around it.
///
/// Every fixture that hand-places a structure into the base needs this
/// first. `place_structure` refuses a cell with no floor, so a base written
/// straight into a save with no floor under it is not a base.
pub(crate) fn found_the_base(app: &mut App) {
    let game = app.game.as_mut().expect("a fixture with a game");
    assert_eq!(
        game.field_pos(),
        Some(TEST_HOME_CELL),
        "a fresh run starts on the origin, where the fixtures found the Home"
    );
    game.place_structure("home", 0, 0, None)
        .expect("a fresh run can afford its first Home");
    game.dev_lay_floor(test_pocket_cells());
    // Founding fires the base tutorial, which would take the screen on this
    // fixture's next keypress and leave every downstream assertion reading
    // `Mode::Notification` for no reason it could name. A test that is about
    // the notification uses `dismiss_notifications` instead.
    drain_notifications(app);
}

/// Empties the engine's notification queue without ever showing it.
pub(crate) fn drain_notifications(app: &mut App) {
    let Some(game) = app.game.as_mut() else {
        return;
    };
    while game.take_notification().is_some() {}
}

/// Dismisses the notification on screen and everything queued behind it,
/// through the real key path. For a fixture that reached one by playing.
pub(crate) fn dismiss_notifications(app: &mut App) {
    while app.mode == Mode::Notification {
        app.handle_key(GameKey::Esc);
    }
}

/// Stands `app`'s party on the Home's tile, by the same save-edit-reload
/// trick `app_underground` uses.
pub(crate) fn stand_in_base(app: &mut App) {
    stand_in_base_at(app, 0, 0);
}

/// Drops `app`'s party into depth 1 of a Stack through the tile they stand
/// on, by the save-edit-reload `app_underground` uses.
pub(crate) fn go_underground(app: &mut App) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("go_underground", 0);
    let _cleanup = RemoveOnDrop(&path);
    app.game
        .as_mut()
        .expect("a fixture with a game")
        .save(&path)
        .unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Stack {
        depth: 1,
        frames: 2,
        x: 1,
        y: 1,
        facing: feral_processes_engine::stack::Dir::North,
        entrance: data.player.position,
    };
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
}

/// Stands the party on the one cell orthogonally adjacent to the Extractor
/// `app_owning_a_program_and_an_extractor` plants at base-space `(2, 0)`.
///
/// `Mode::UpgradeDirection` aims at a *neighbour*, so an upgrade test has to
/// say where the party is standing before it can press a direction — the
/// exit cell `stand_in_base` leaves it on is two tiles west of that machine
/// and every arrow key from there finds nothing.
pub(crate) fn stand_beside_the_extractor(app: &mut App) {
    stand_in_base_at(app, 1, 0);
}

/// A base with a Workbench beside the party and Automation researched.
pub(crate) fn app_at_a_workbench(seed: u32) -> App {
    let mut app = app_owning_a_program_and_a_station_of("routine_craft", "workbench", seed);
    stand_beside_the_extractor(&mut app);
    let game = app.game.as_mut().unwrap();
    let _ = game.dev_unlock_research(&"automation".to_string());
    app
}

/// Deletes the `.sav` it was built from when it drops, unwind included —
/// `save`, `load_from_file`, `save_to_file` and `Game::load` all sit
/// between this guard's construction and the plain `remove_file` a
/// straight-line version would use at the end, and every one of them can
/// panic. This repo has a recorded history of `/tmp` inode exhaustion from
/// exactly that shape of leak, so the cleanup has to survive a panic partway
/// through, not just the happy path.
pub(super) struct RemoveOnDrop<'a>(pub(super) &'a std::path::Path);

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
    }
}

/// The same, on a chosen field cell — for a test that needs the party
/// somewhere other than the Home, the pocket's edge most often.
pub(crate) fn stand_in_base_at(app: &mut App, x: i32, y: i32) {
    // Counted rather than keyed on a seed, the same as `app_underground`:
    // tests run in parallel and several share a seed.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_base_{unique}.sav"));
    let _cleanup = RemoveOnDrop(&path);
    let game = app.game.as_mut().expect("a fixture with a game");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    stand_save_on_field(&mut data, x, y);
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
}

/// The routine ids a fixture's save should carry for `tokens`. A plain token
/// is an authored ability id and passes through; `built:<core>[+<part>...]`
/// (`built:field_regen+group`) adds a routine built from those parts to
/// the save's crafted record and names its `crafted/<n>` id, the way a
/// player's Workbench would.
pub(super) fn expand_routines(data: &mut save::SaveData, tokens: &[&str]) -> Vec<String> {
    use feral_processes_engine::routine_craft::{
        AreaKind, Core, CraftedRoutine, RangeKind, Subroutine, Version,
    };
    tokens
        .iter()
        .map(|token| {
            let Some(spec) = token.strip_prefix("built:") else {
                return token.to_string();
            };
            let parts: Vec<Subroutine> = spec
                .split('+')
                .map(|word| match word {
                    "group" => Subroutine::Area(AreaKind::Group),
                    "everyone" => Subroutine::Area(AreaKind::Everyone),
                    "near" => Subroutine::Range(RangeKind::Near),
                    "far" => Subroutine::Range(RangeKind::Far),
                    core => Subroutine::Core(
                        *Core::ALL
                            .iter()
                            .find(|c| c.key() == core)
                            .unwrap_or_else(|| panic!("no Core called {core}")),
                        Version::V1,
                    ),
                })
                .collect();
            data.next_crafted_id += 1;
            let id = format!("crafted/{}", data.next_crafted_id);
            data.crafted_routines.push(CraftedRoutine {
                id: id.clone(),
                name: spec.to_string(),
                parts,
            });
            id
        })
        .collect()
}

/// Frees the player's second routine slot on an existing fixture, through
/// the save the way `stand_in_base_at` does: a new game's built Patch fills
/// it, and the install screens are about installing into it.
pub(crate) fn free_a_routine_slot(app: &mut App) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_known_{unique}.sav"));
    let _cleanup = RemoveOnDrop(&path);
    let game = app.game.as_mut().expect("a fixture with a game");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines.retain(|r| !r.starts_with("crafted/"));
    data.crafted_routines.clear();
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
}

/// Installs `routines` into the player's own slots on an existing fixture —
/// `player.routines`, the *equipped* set a fight can actually run, rather
/// than a built routine an Install screen offers. For a test that needs a companion already in the party
/// (`app_with_companions_in_the_party`) standing beside a player who can run
/// a tactical routine, since `app_with_player_routines` builds its own fresh
/// fixture rather than editing one that already carries other state.
pub(crate) fn install_player_routines(app: &mut App, routines: &[&str]) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let path = std::env::temp_dir().join(format!("feral_processes_appcore_installed_{unique}.sav"));
    let _cleanup = RemoveOnDrop(&path);
    let game = app.game.as_mut().expect("a fixture with a game");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = expand_routines(&mut data, routines);
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
}

/// Teaches the player `species` as a learned image on an existing fixture —
/// `resources::EmulationImages`, todo #100 Task 6's own store. No public
/// writer short of a real extraction exists outside a save round trip:
/// `install_player_routines`'s shape, one field over.
pub(crate) fn learn_image(app: &mut App, species: &str) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_image_{unique}.sav"));
    let _cleanup = RemoveOnDrop(&path);
    let game = app.game.as_mut().expect("a fixture with a game");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.emulation_images.push(species.to_string());
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
}

/// Empties the player's routine slots on an existing fixture, through the
/// save the way `stand_in_base_at` does.
///
/// A round trip rather than `Game::uninstall_routine`, because `decompile`
/// is welded into slot 0 and that verb refuses it — the empty kit is
/// reachable only as save state now (an older save whose owner popped it
/// out, which is also what `stack_market`'s fixture builds).
pub(crate) fn clear_player_routines(app: &mut App) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_kit_{unique}.sav"));
    let _cleanup = RemoveOnDrop(&path);
    let game = app.game.as_mut().expect("a fixture with a game");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines.clear();
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
}

/// A game where the player has `routines` installed (in place of the
/// default `decompile`) and `hunger` set to a chosen level, so a field-routine
/// test can pin affordability on either side of a routine's `power_cost`
/// exactly. Built by editing a save and reloading it, for the same reason
/// `app_owning_a_program_and_an_extractor` is.
pub(crate) fn app_with_player_routines(seed: u32, routines: &[&str], hunger: f32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field", seed);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = expand_routines(&mut data, routines);
    data.player.power = hunger;
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An app whose player carries `routines` and is standing on depth 1 of a
/// Stack frame, at the frame's own entry cell facing north.
///
/// Written into the save rather than walked to: `Game::enter_stack` is
/// `pub(crate)` in the engine and app-core is outside that boundary, and a
/// fixture that hunted the zone for a surface link would be a test about
/// world generation.
pub(crate) fn app_underground_with_routines(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field_stack", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = expand_routines(&mut data, routines);
    data.locale = feral_processes_engine::resources::Locale::Stack {
        depth: 1,
        frames: 2,
        // Overwritten below by the frame's own entry — the save only has to
        // put the party *somewhere* legal for the frame to regenerate.
        x: 1,
        y: 1,
        facing: feral_processes_engine::stack::Dir::North,
        entrance: data.player.position,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game where the player has one program standing in the active `Party`,
/// so a battle opens with two planning slots rather than the single one
/// every other battle fixture here produces. Built by editing a save and
/// reloading it, for the reason `app_owning_a_program_and_an_extractor` is:
/// nothing outside the engine can hand-place a tamed program, and
/// `party_slot` is the save field that puts one on the roster.
pub(crate) fn app_with_companions_in_the_party(seed: u32, count: u32) -> App {
    app_with_companions_and_cargo(seed, count, &[])
}

/// `app_with_companions_in_the_party` whose companions each remember three
/// events, so a memories page has rows to scroll through. Edited into a save
/// and reloaded, since the engine offers no way to write a memory from
/// outside the crate.
pub(crate) fn app_with_remembering_companions(seed: u32, count: u32) -> App {
    let mut app = app_with_companions_in_the_party(seed, count);
    let path = scratch_path("remembering", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    for creature in data.creatures.iter_mut().filter(|c| c.tamed) {
        creature.memories = ["hard_won", "vented", "ran_down"]
            .into_iter()
            .map(|def| save::MemorySave {
                def: def.into(),
                subject: feral_processes_engine::components::MemorySubject::Nothing,
                subject_name: None,
                reinforced: 0,
                strikes: 1,
            })
            .collect();
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// An `App` standing on the entry cell of Stack frame 1.
///
/// Built by editing a save and reloading it, the same trick
/// `app_owning_distant_programs` uses: the engine deliberately exposes no
/// way to drop the player into the Stack from outside the crate, since on a
/// real run that only ever happens by walking onto an entrance.
pub(crate) fn app_underground(seed: u32) -> App {
    // Counted rather than keyed on the seed alone: tests run in parallel and
    // several share a seed, so a seed-named scratch file has two of them
    // reading and deleting the same path.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path =
        std::env::temp_dir().join(format!("feral_processes_appcore_stack_{seed}_{unique}.sav"));
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let spec = FrameSpec {
        world_seed: data.seed,
        entrance: data.player.position,
        depth: 1,
        frames: 2,
        tier: 1,
    };
    let entry = generate(spec).entry;
    data.locale = Locale::Stack {
        depth: spec.depth,
        frames: spec.frames,
        x: entry.0,
        y: entry.1,
        facing: Dir::North,
        entrance: spec.entrance,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// `app_underground`, minus every Power Outlet the fresh game starts with —
/// the fixture a "rest is refused for want of a charge" test needs, since
/// `app_underground` inherits a new game's two.
pub(crate) fn app_underground_with_no_rest_charge(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("stack_no_outlet", seed);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player
        .inventory
        .retain(|(id, _)| id.as_str() != feral_processes_engine::items::ids::OUTLET);
    let spec = FrameSpec {
        world_seed: data.seed,
        entrance: data.player.position,
        depth: 1,
        frames: 2,
        tier: 1,
    };
    let entry = generate(spec).entry;
    data.locale = Locale::Stack {
        depth: spec.depth,
        frames: spec.frames,
        x: entry.0,
        y: entry.1,
        facing: Dir::North,
        entrance: spec.entrance,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}
