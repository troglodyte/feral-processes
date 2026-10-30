//! Fixtures shared by the app-core tests: base.

use feral_processes_engine::affixes::AffixId;
use feral_processes_engine::components::Rarity;
use feral_processes_engine::progression::DerivedBase;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save::{self};

use super::*;
use crate::*;

/// A game where the player wears `weapon` — an item id plus the gear level
/// it was equipped at (see `components::EquippedItem`) — carries
/// `inventory`, and stands `zone` sectors deep.
///
/// Written straight into the save rather than staged through `Game::equip`,
/// which always stamps gear with the *current* zone level. The gap between
/// the level your weapon remembers and the one the zone would grant it now
/// is exactly what the swap screen's delta column reports, so there is no
/// other way to set it up. `Stats` is deliberately left at its unequipped
/// value: `equip_swap_rows` reads the recorded gear level and the item
/// catalogue, never the player's current attack.
pub(crate) fn app_wearing_weapon(
    seed: u32,
    weapon: Option<(&str, u32)>,
    inventory: &[(&str, u32)],
    zone: u32,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("gear", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = inventory
        .iter()
        .map(|(item, qty)| (ItemId::from(*item), *qty))
        .collect();
    if let Some((item, level)) = weapon {
        data.player.weapon = Some(ItemId::from(item));
        data.player.weapon_level = level;
    }
    data.zone = zone;
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game `zone` sectors deep, wearing nothing, carrying one copy of `item`
/// with `affix` on it.
///
/// Written into the save rather than staged through play because nothing
/// public grants an affixed copy: `Game::grant_gear_drop` is the only way one
/// enters the game and it is the engine's own, rolled off `GameRng`. An empty
/// slot is the point — with nothing worn, what the swap picker prints for a
/// candidate is what equipping it must actually grant, with no outgoing item's
/// figure in between.
pub(crate) fn app_carrying_affixed_gear(seed: u32, item: &str, affix: &str, zone: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("affixed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.zone = zone;
    data.player.gear_copies = vec![(
        save::GearCopySave {
            item: ItemId::from(item),
            rarity: Rarity::Ordinary,
            tier: 0,
            affix: None,
            affixes: vec![AffixId::from(affix)],
            quality: feral_processes_engine::tuning::QUALITY_DEFAULT,
            wear: 0,
        },
        1,
    )];
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An ordinary, unfused copy of `item` wearing `affix` — the affixed twin of
/// `gear` above.
pub(crate) fn affixed_gear(item: &str, affix: &str) -> GearCopy {
    GearCopy {
        rarity: Rarity::Ordinary,
        tier: 0,
        affixes: vec![AffixId::from(affix)],
        ..GearCopy::plain(ItemId::from(item))
    }
}

/// The party's two command pages, in rail order.
pub(crate) const PARTY_COMMAND_PAGES: [Mode; 2] = [Mode::PartyPrograms, Mode::PartyRoutines];

/// A carried copy of `item` at fusion `tier`, ordinary rare tier — the
/// app-core twin of the engine's test helper of the same name, and here for
/// the same reason: the screens and handlers now name a whole
/// `items::GearCopy`, and almost every test means the plain one.
pub(crate) fn gear(item: &ItemId, tier: u32) -> GearCopy {
    GearCopy {
        rarity: Rarity::Ordinary,
        tier,
        affixes: Vec::new(),
        ..GearCopy::plain(item.clone())
    }
}

/// A game with a Contract Broker one tile east of the player, standing on a
/// base the player is also standing on — which is what `BrokerReach::AtBroker`
/// asks for, and what a run that owns a Broker looks like anyway, since
/// nothing but a Home may be deployed before a Home is. `underground` drops
/// the party into a Stack frame afterwards, which is what refuses the verbs
/// while leaving the board readable.
///
/// The Home is what puts the floor there: `found_the_base` lays the
/// starting pocket into `BaseGrid` when it deploys one, so a save with a
/// Broker and no Home loads as a base that does not exist.
///
/// Built by editing a save for the reason `app_at_trading_posts` is: the
/// engine exposes no way to hand-place a structure from outside the crate.
pub(crate) fn app_at_a_contract_broker(seed: u32, underground: bool) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("broker", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // The Home is already standing, laid down by `found_the_base`; the desk
    // goes up beside it, on the pocket floor it laid.
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "contract_broker".to_string(),
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
    let locale = if underground {
        Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: feral_processes_engine::stack::Dir::North,
            entrance: data.player.position,
        }
    } else {
        // Standing in the base with the desk, which is what `AtBroker` asks
        // for. One cell south of the Home so nothing shares a tile.
        data.player.position = (TEST_HOME_CELL.0, TEST_HOME_CELL.1 + 1);
        Locale::Surface
    };
    data.locale = locale;
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    // The Broker fixtures are all about the *board*, and a run still running
    // the onboarding chain has an empty one by design.
    skip_tutorial(&mut app);
    app
}

/// Files every onboarding mission as finished, so the ordinary contract
/// board is live and nothing is in hand the player did not sign for.
///
/// A save round trip rather than a call into the engine: the engine's own
/// `skip_tutorial` is a `#[cfg(test)]` helper of that crate and unreachable
/// from here, and `contracts_done` is what the chain's position is derived
/// from — so writing it is the same thing the engine's helper does, through
/// the one public door app-core has for editing a run.
///
/// Which ids belong to the chain is read off `ContractRow::tutorial`, which
/// is exactly what the screen colours on.
pub(crate) fn skip_tutorial(app: &mut App) {
    let path = scratch_path("skip_tutorial", 0);
    let game = app.game.as_mut().unwrap();
    let ids: Vec<_> = game
        .contract_catalogue()
        .into_iter()
        .filter(|row| row.tutorial)
        .map(|row| row.id)
        .collect();
    assert!(
        !ids.is_empty(),
        "the shipped chain is what this skips; an empty one means it is not \
         reaching the catalogue"
    );
    game.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.contracts.retain(|held| held.def.tutorial.is_none());
    for id in ids {
        if !data.contracts_done.contains(&id) {
            data.contracts_done.push(id);
        }
    }
    save::save_to_file(&path, &data).unwrap();
    // The install the app was built with, not `test_assets_dir()` — they are
    // the same today, and a modded fixture is what would tell them apart.
    let assets = app.assets_dir.clone();
    app.game = Game::load(&path, &assets).ok();
    let _ = std::fs::remove_file(&path);
}

/// Takes the party out of the base entirely, without walking there.
///
/// A save round trip rather than ten movement keys, which is what the
/// contracts fixtures already do and for the same reason — the engine hands
/// app-core no way to write a `Position` — plus one of its own: walking ten
/// tiles ticks the world ten times, and a wild program met on the way would
/// answer a question about the contracts screen with a battle.
pub(crate) fn walk_far_from_the_base(app: &mut App) {
    let path = scratch_path("off_base", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    // Past the pocket's floor: being off the floor is the whole of being off
    // the base.
    stand_save_on_field(&mut data, TEST_POCKET_HALF_WIDTH + 10, 0);
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &test_assets_dir()).ok();
    let _ = std::fs::remove_file(&path);
}

/// A party standing in base space with two stocked machines either side of
/// them — the fixture the collect picker's tests are all built on.
///
/// The machines go in through a save edit rather than through play: the
/// engine hands app-core no way to write a `Stock`, which is the seam
/// working rather than a limitation of the test. `extractor` on both sides
/// so the pair pools into one row, and the western one is the *lower* tile,
/// so a take that spans them has an order to be right about.
/// The party in base space beside `depots` Depots, each already holding
/// `filled` units so the room left is a figure the test chose, and carrying
/// exactly `pack`.
///
/// The pack is **set**, not added to: `Game::new` seeds a starting kit, so a
/// fixture that adds would be measuring the kit as well as its own rows.
pub(crate) fn app_beside_depots(seed: u32, depots: i32, filled: u32, pack: &[(&str, u32)]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("deposit", seed);
    let _cleanup = RemoveOnDrop(&path);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let ballast = if filled > 0 {
        vec![(
            feral_processes_engine::items::ItemId::from("core_fragment"),
            filled,
        )]
    } else {
        Vec::new()
    };
    for x in 0..depots {
        data.structures.push(save::StructureSave {
            recipe: None,
            kind: "depot".to_string(),
            // Orthogonal to the party's base cell, which is what a collect
            // and a deposit both reach by — see `collect::ORTHOGONAL`.
            position: (if x == 0 { 1 } else { -1 }, 0),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: ballast.clone(),
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
    }
    data.player.inventory = pack
        .iter()
        .map(|(id, n)| (feral_processes_engine::items::ItemId::from(*id), *n))
        .collect();
    stand_save_on_field(&mut data, 0, 0);
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    app.mode = Mode::Playing;
    app
}

/// A founded base — Home only, nothing else standing — with `programs`
/// tamed programs added to the roster, each caught at zone 1.
///
/// Not `app_inside_a_small_base_with_programs`: that fixture always adds a
/// mining node on cell `(1, 0)`, which is exactly the cell the build-program
/// picker's own tests place a fresh structure on. This one leaves the pocket
/// empty around the Home so a deploy test has somewhere to land.
pub(crate) fn app_in_base_with_programs(seed: u32, programs: usize) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    for _ in 0..programs {
        tame_program_at_zone(&mut app, 1);
    }
    app
}

/// A base with two tamed programs and the party at `(0, 0)` in base space,
/// with an Extractor rebuild site awaiting a program at `(1, 0)` and, when
/// `depot` is set, a Depot at `(-1, 0)`.
///
/// The site is written into the save as filed, rather than waited for: a
/// ruin only becomes a site on a tick, and nothing here wants a tick.
pub(crate) fn app_beside_a_rebuild_site(seed: u32, depot: bool) -> App {
    let mut app = app_in_base_with_programs(seed, 2);
    let path = scratch_path("rebuild_site", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.build_sites.push(save::BuildSiteSave {
        position: (1, 0),
        structure: "extractor".to_string(),
        cost: Vec::new(),
        delivered: Vec::new(),
        progress: 0,
        goal: feral_processes_engine::components::BuildGoal::New,
        program: None,
        awaiting_program: true,
        recipe: None,
    });
    if depot {
        data.structures.push(save::StructureSave {
            recipe: None,
            kind: "depot".to_string(),
            position: (-1, 0),
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
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    app.mode = Mode::Playing;
    app
}

/// `app_beside_a_rebuild_site` with a second awaiting site north at
/// `(0, -1)` and no Depot: two things for `[c]` to ask about and no cargo.
pub(crate) fn app_beside_two_rebuild_sites(seed: u32) -> App {
    let mut app = app_beside_a_rebuild_site(seed, false);
    let path = scratch_path("two_sites", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    let mut second = data.build_sites[0].clone();
    second.position = (0, -1);
    data.build_sites.push(second);
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    app.mode = Mode::Playing;
    app
}

/// A base standing inside base space with a Research Node and the Extractor
/// its bills draw on, so `Game::select_research` has something to accept.
///
/// Built by editing a save, `app_inside_a_small_base`' reason: deploying two
/// structures through the build flow needs materials the player does not start
/// with, and what this test wants is a base that can take a project on rather
/// than the build flow that got it there.
pub(crate) fn app_in_base_with_a_research_node(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("research_base", seed);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let _cleanup = RemoveOnDrop(&path);

    let mut data = save::load_from_file(&path).unwrap();
    // The Research Node makes the currency a project runs on; the Extractor
    // is what makes every shipped bill's material, which
    // `work_orders::chain_break` refuses a project without.
    for (index, (kind, recipe)) in [
        ("research_node", None),
        ("extractor", Some("core_fragment")),
    ]
    .into_iter()
    .enumerate()
    {
        data.structures.push(save::StructureSave {
            recipe: recipe.map(Into::into),
            kind: kind.to_string(),
            position: (2 + index as i32, 2),
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
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    stand_in_base(&mut app);
    app
}

/// A founded base whose `routine_fabrication` node reads as researched, so
/// the routine tree is open and its always-visible families (Symlink's
/// among them) are listed — `routine/symlink` is the fixed point every
/// `Mode::RoutineResearch` test picks against. Carries a Research Node so
/// a listed node is not flagged for having nowhere to run — no Extractor,
/// since a synthesised node's bill is empty (spec §2 "Cost").
///
/// Built by editing a save, `app_in_base_with_a_research_node`'s reason:
/// what these tests want is a tree already open, not the Research Node
/// project that would earn it — `Game::select_research` and a Research
/// Node's own progress are the engine's tests to make, not app-core's to
/// redo through a slow tick loop.
pub(crate) fn app_in_base_with_routine_tree_open(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("routine_tree_open", seed);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let _cleanup = RemoveOnDrop(&path);

    let mut data = save::load_from_file(&path).unwrap();
    data.researched.push("routine_fabrication".to_string());
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "research_node".to_string(),
        position: (2, 2),
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
    stand_in_base(&mut app);
    app
}

/// `app_in_base_with_programs` with no programs — a base a deploy or an
/// upgrade can be offered but never afford, for a test about the refusal
/// rather than the spend.
pub(crate) fn app_in_base(seed: u32) -> App {
    app_in_base_with_programs(seed, 0)
}

/// Marks `id` researched by editing the save directly and reloading —
/// `app_in_base_with_routine_tree_open`'s trick, generalised: what a test
/// wants is the research already done, not `Game::select_research` and a
/// project's own progress replayed through a slow tick loop. Fusion's own
/// gate, `Game::fusion_unlocked`, reads exactly this: no loaded save
/// leaves `program_refactoring` researched by default, so any app-core
/// fixture whose test fuses two programs needs this call first.
pub(crate) fn mark_researched(app: &mut App, id: &str) {
    let path = scratch_path("mark_researched", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let _cleanup = RemoveOnDrop(&path);
    let mut data = save::load_from_file(&path).unwrap();
    data.researched.push(id.to_string());
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
}

/// Adds one tamed program to `app`'s roster, caught at `zone` — the one
/// thing about a program `programs_for_build`'s `>=` floor cares about that
/// no public `Game` method can set. Through the save round trip for the same
/// reason `distant_programs` is: the engine's `World` is private, and
/// `components::ZonePortal` is written only by `spawn_creature_from_save`.
pub(crate) fn tame_program_at_zone(app: &mut App, zone: u32) {
    tame_program_at_zone_with_build_rolls(app, zone, 1.0, 1.0)
}

pub(crate) fn app_beside_stocked_machines(seed: u32, stock: &[(&str, u32)]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("collect", seed);
    let _cleanup = RemoveOnDrop(&path);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let output: Vec<(feral_processes_engine::items::ItemId, u32)> = stock
        .iter()
        .map(|(id, n)| (feral_processes_engine::items::ItemId::from(*id), *n))
        .collect();
    for x in [-1, 1] {
        data.structures.push(save::StructureSave {
            recipe: Some("core_fragment".into()),
            kind: "extractor".to_string(),
            position: (x, 0),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: output.clone(),
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
    }
    stand_save_on_field(&mut data, 0, 0);
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    app.mode = Mode::Playing;
    app
}

/// A founded base with a Mod Bench standing, `affix:deflecting` researched,
/// one ordinary `nullsteel_plate` in the pack and `fragments` of
/// `core_fragment` to pay for applying it. Built by editing a save, for the reason
/// `app_in_base_with_routine_tree_open` gives.
pub(crate) fn app_at_a_mod_bench(seed: u32, fragments: u32) -> App {
    app_at_a_mod_bench_carrying(seed, fragments, Vec::new())
}

/// `app_at_a_mod_bench` with `modded` copies in the pack as well.
pub(crate) fn app_at_a_mod_bench_carrying(
    seed: u32,
    fragments: u32,
    modded: Vec<(save::GearCopySave, u32)>,
) -> App {
    let inventory = vec![
        (ItemId::from("core_fragment"), fragments),
        (ItemId::from("nullsteel_plate"), 1),
    ];
    app_at_a_bench(seed, "mod_bench", Vec::new(), inventory, modded, None)
}

/// A founded base with a Refit Bench holding `stock` in its input and
/// `plate` (worn down by `wear` fights) carried in the pack.
pub(crate) fn app_at_a_refit_bench(seed: u32, wear: u8, stock: Vec<(ItemId, u32)>) -> App {
    let worn = save::GearCopySave {
        item: ItemId::from("nullsteel_plate"),
        rarity: Rarity::Ordinary,
        tier: 0,
        affix: None,
        affixes: Vec::new(),
        quality: feral_processes_engine::tuning::QUALITY_DEFAULT,
        wear,
    };
    app_at_a_bench(
        seed,
        "refit_bench",
        stock,
        Vec::new(),
        vec![(worn, 1)],
        None,
    )
}

/// A Refit Bench with the party standing orthogonally beside it and `cores`
/// Annealed Cores in the pack, for the hand-loading picker.
pub(crate) fn app_beside_a_refit_bench_carrying(seed: u32, cores: u32) -> App {
    let inventory = vec![(ItemId::from("annealed_core"), cores)];
    let mut app = app_at_a_bench(seed, "refit_bench", Vec::new(), inventory, Vec::new(), None);
    stand_in_base_at(&mut app, 2, 1);
    app
}

/// A founded base with one `kind` bench standing and holding `bench_stock` in
/// its input, the pack set to `inventory` and the modded `gear_copies` to
/// `modded`.
pub(super) fn app_at_a_bench(
    seed: u32,
    kind: &str,
    bench_stock: Vec<(ItemId, u32)>,
    inventory: Vec<(ItemId, u32)>,
    modded: Vec<(save::GearCopySave, u32)>,
    worn_weapon: Option<(&str, u32, u8)>,
) -> App {
    let mut app = test_app(seed);
    let path = scratch_path(kind, seed);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let _cleanup = RemoveOnDrop(&path);

    let mut data = save::load_from_file(&path).unwrap();
    data.researched.push("affix:deflecting".to_string());
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: kind.to_string(),
        position: (2, 2),
        durability: None,
        tier: None,
        stock_input: bench_stock,
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
    // A plain copy lives in `inventory`; `gear_copies` holds only modded ones.
    data.player.inventory = inventory;
    data.player.gear_copies = modded;
    if let Some((item, level, wear)) = worn_weapon {
        data.player.weapon = Some(ItemId::from(item));
        data.player.weapon_level = level;
        data.player.weapon_wear = wear;
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    stand_in_base(&mut app);
    app
}

/// A founded base with a Refit Bench standing and the player wearing
/// `item`, locked in at gear `level` and worn down by `wear` fights, and the
/// bench holding `stock` to pay for the refit.
pub(crate) fn app_at_a_refit_bench_wearing(
    seed: u32,
    item: &str,
    level: u32,
    wear: u8,
    stock: Vec<(ItemId, u32)>,
) -> App {
    app_at_a_bench(
        seed,
        "refit_bench",
        stock,
        Vec::new(),
        Vec::new(),
        Some((item, level, wear)),
    )
}

/// The small base with a Lathe east of its Extractor, so the two are one
/// production line: Home, Lathe, Extractor in roster order.
pub(crate) fn app_inside_a_base_with_a_production_line(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = app_inside_a_small_base_with_programs(seed, false, 1);
    let path = scratch_path("line_base", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    let substrate_factory = save::StructureSave {
        recipe: Some("blank_substrate".into()),
        kind: "factory".to_string(),
        position: (2, 0),
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
    };
    data.structures.push(substrate_factory);
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app
}

/// The `base` a hand-written tamed `CreatureSave` needs: `Game::load` refuses
/// an owned program without one, and these figures are what it derives from.
pub(crate) fn tamed_base(max_hp: i32, atk: i32, mitigation: i32) -> Option<DerivedBase> {
    Some(DerivedBase {
        max_hp,
        atk,
        mitigation,
        ..DerivedBase::player()
    })
}

/// `app_inside_a_small_base_with_programs`'s base with a built Breeding Bay
/// beside the Home, `seeds` breeding seeds in the pack, and the **first**
/// owned program still resting from an earlier breeding when `first_resting`.
/// Through the save for the reason every fixture here is: nothing public
/// builds a structure for free or writes a cooldown.
pub(crate) fn app_with_a_breeding_bay(seed: u32, seeds: u32, first_resting: bool) -> App {
    let mut app = app_inside_a_small_base_with_programs(seed, false, 2);
    let assets_dir = test_assets_dir();
    let path = scratch_path("breeding_bay", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "breeding_bay".to_string(),
        position: (-1, 0),
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
        incubating: vec![None],
        study_yield: None,
    });
    data.player
        .inventory
        .push((ItemId::from("breeding_seed"), seeds));
    if first_resting {
        let first = data.creatures.iter_mut().find(|c| c.tamed).unwrap();
        first.breed_ready_at = u64::MAX / 2;
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Opens the bay's sheet the way inspecting it does.
pub(crate) fn open_the_bay_sheet(app: &mut App) -> Entity {
    let bay = app
        .game
        .as_mut()
        .unwrap()
        .structure_report()
        .into_iter()
        .find(|s| s.label == "Breeding Bay")
        .expect("the fixture builds a bay")
        .entity;
    app.pending_structure_manifest = Some(bay);
    app.mode = Mode::StructureManifest;
    bay
}

/// The mirror of `app_between_a_depot_and_a_machine`: the Factory moved west
/// to `(-1, 0)` and a Depot east at `(1, 0)`, so the Depot sorts *after* the
/// machine by `(x, y)`.
pub(crate) fn app_beside_a_machine_west_and_a_depot_east(seed: u32) -> App {
    let mut app = app_beside_a_machine(seed, "factory", None);
    let path = scratch_path("machine_west_depot_east", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    let factory = data
        .structures
        .iter_mut()
        .find(|s| s.kind == "factory")
        .expect("the factory from app_beside_a_machine");
    factory.position = (-1, 0);
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "depot".to_string(),
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
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    app.mode = Mode::Playing;
    app
}

/// `app_beside_a_machine` with a Depot on the other side: a Factory east at
/// `(1, 0)` and a Depot west at `(-1, 0)`, so `[c]` has two sides to ask
/// about.
pub(crate) fn app_between_a_depot_and_a_machine(seed: u32) -> App {
    let mut app = app_beside_a_machine(seed, "factory", None);
    let path = scratch_path("depot_and_machine", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "depot".to_string(),
        position: (-1, 0),
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
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    app.mode = Mode::Playing;
    app
}

/// `app_between_a_depot_and_a_machine` with a second Depot south at
/// `(0, 1)`: three structures in the hub's cycle, machine, south Depot,
/// west Depot.
pub(crate) fn app_beside_a_machine_and_two_depots(seed: u32) -> App {
    let mut app = app_between_a_depot_and_a_machine(seed);
    let path = scratch_path("machine_and_two_depots", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "depot".to_string(),
        position: (0, 1),
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
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    app.mode = Mode::Playing;
    app
}

/// `app_beside_a_machine` with the Factory east at `(1, 0)` set to
/// `bytecode_block` and holding three of them on its output shelf, so the
/// transfer and the recipe menu are offered on the same side.
pub(crate) fn app_beside_a_set_machine_holding_output(seed: u32) -> App {
    let mut app = app_beside_a_machine(seed, "factory", Some("bytecode_block"));
    let path = scratch_path("machine_with_output", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    let factory = data
        .structures
        .iter_mut()
        .find(|s| s.kind == "factory")
        .expect("the factory from app_beside_a_machine");
    factory.stock_output = vec![(ItemId::from("bytecode_block"), 3)];
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    app.mode = Mode::Playing;
    app
}

/// The party in base space beside one machine of `kind` at `(1, 0)`, set to
/// `recipe` (`None` for an unset machine). Built through the save round
/// trip, `app_beside_a_teardown_rig_holding`'s reason. `recipe` must be one
/// that needs no research, since a save restores it verbatim.
pub(crate) fn app_beside_a_machine(seed: u32, kind: &str, recipe: Option<&str>) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("machine", seed);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    stand_save_on_field(&mut data, 0, 0);
    data.structures.push(save::StructureSave {
        recipe: recipe.map(ItemId::from),
        kind: kind.to_string(),
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

/// Types `text` one key at a time, the way a player fills a list filter.
pub(crate) fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(GameKey::Char(c));
    }
}
