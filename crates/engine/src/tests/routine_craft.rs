//! Routine crafting's wiring into `Game` (ADR 0029, phase 2): the crafted
//! tier, part items, the craft / build / install verbs, where a built
//! routine is destroyed, research, and the save.

use super::support::*;
use crate::abilities::AbilityDb;
use crate::components::{Routines, Tamed};
use crate::resources::{CraftedRoutines, Research};
use crate::routine_craft::{AreaKind, Core, CraftedRoutine, RangeKind, Subroutine, Version};
use crate::routine_tree::{capacity_node_id, part_node_id};
use crate::*;

fn heal() -> Subroutine {
    Subroutine::Core(Core::Heal, Version::V1)
}

fn damage() -> Subroutine {
    Subroutine::Core(Core::Damage, Version::V1)
}

/// A game with a Workbench standing, the way extraction tests get one.
fn bench_game(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    game
}

fn know(game: &mut Game, part: &Subroutine) {
    game.world
        .resource_mut::<Research>()
        .0
        .insert(part_node_id(part));
}

fn stock(game: &mut Game, part: &Subroutine, qty: u32) {
    give(game, &ItemId::part(part), qty);
}

fn crafted(game: &Game) -> CraftedRoutines {
    game.world.resource::<CraftedRoutines>().clone()
}

/// Builds `parts` straight from stock, no matter what they cost.
fn build(game: &mut Game, name: &str, parts: &[Subroutine]) -> String {
    for part in parts {
        stock(game, part, 1);
    }
    game.build_routine(name, parts)
        .unwrap_or_else(|e| panic!("building {name}: {e}"))
}

fn held_ids(game: &Game, entity: Entity) -> Vec<String> {
    game.world.get::<Routines>(entity).unwrap().0.clone()
}

/// The companion: a shipped species so it survives a save, with room for a
/// routine.
fn companion(game: &mut Game) -> Entity {
    let pet = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("a shipped species");
    set_level(game, pet, 10);
    assert!(game.routine_slots(pet) > held_ids(game, pet).len());
    pet
}

// ---------------------------------------------------------------------------
// Task 1: the crafted tier
// ---------------------------------------------------------------------------

#[test]
fn a_crafted_id_resolves_through_get_and_stays_out_of_the_catalogue_walks() {
    let mut game = bench_game(70_001);
    let id = build(&mut game, "Mender", &[heal()]);

    let db = game.world.resource::<AbilityDb>();
    let def = db.get(&id).expect("a crafted id resolves through get");
    assert_eq!(def.name, "Mender");
    assert!(
        db.all().all(|d| d.id != id),
        "a built routine is not content to be found, so `all` skips it"
    );
    assert!(db.wild_pool().iter().all(|(d, _)| d.id != id));
    assert!(db.exclusive_pool().iter().all(|d| d.id != id));
}

#[test]
fn an_installed_built_routine_is_in_the_players_actor_abilities() {
    let mut game = bench_game(70_002);
    let player = game.player_entity();
    make_room_for_routine(&mut game, player);
    let id = build(&mut game, "Mender", &[heal()]);

    game.install_built(player, &id).unwrap();

    assert!(
        game.actor_abilities(player).iter().any(|a| a.id == id),
        "the kit must resolve a crafted id"
    );
}

// ---------------------------------------------------------------------------
// Task 2: part items
// ---------------------------------------------------------------------------

#[test]
fn every_catalogue_part_has_an_item_that_names_it_back() {
    let game = Game::new(70_003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    for part in Subroutine::all() {
        let id = ItemId::part(&part);
        let def = items
            .get(id.as_str())
            .unwrap_or_else(|| panic!("{} has no item", part.label()));
        assert_eq!(def.id.part_of(), Some(part), "{id} does not round-trip");
        assert!(def.craftable.is_none(), "a part is crafted by its verb");
        assert_eq!(def.value, Some(crate::tuning::CRAFT_PART_VALUE));
    }
}

/// The research-bill seam rule, applied to a part's cost: a part may only
/// ask for what its own research prerequisites let the player make. Every
/// part sits behind `routine_fabrication` (it opens the routine tree) and
/// the Routine Disk needs the same node, so a material is fine if it is raw
/// or craftable from raw with no research of its own.
#[test]
fn no_parts_material_needs_research_beyond_the_routine_trees_own() {
    let game = Game::new(70_004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let raw = ItemId::from(crate::tuning::CRAFT_MATERIAL_FRAGMENT);
    let mut seen = std::collections::BTreeSet::new();
    for part in Subroutine::all() {
        let (material, count) = crate::routine_craft::material(&part);
        assert!(count >= 1);
        if !seen.insert(material.clone()) {
            continue;
        }
        let def = items.get(material.as_str()).expect("the material ships");
        if material == raw {
            continue;
        }
        let recipe = def
            .craftable
            .as_ref()
            .unwrap_or_else(|| panic!("{material} cannot be made at all"));
        assert!(
            recipe.research.is_none(),
            "{material} needs research of its own, which a part's research does not imply"
        );
        assert!(
            recipe.cost.iter().all(|(item, _)| *item == raw),
            "{material} is made from something other than raw salvage"
        );
    }
    assert_eq!(seen.len(), 3, "the three materials the spec names");
}

#[test]
fn a_part_cannot_be_bought_off_a_caravan_shelf_but_sells_back() {
    let game = Game::new(70_005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let item = ItemId::part(&heal());
    assert!(
        game.item_value(&item) > crate::tuning::CONTRACT_MAX_DELIVER_VALUE,
        "a part is above a contract delivery's ceiling, as an etched disk was"
    );
}

// ---------------------------------------------------------------------------
// Task 3: craft, build, install, uninstall
// ---------------------------------------------------------------------------

#[test]
fn crafting_a_part_spends_a_disk_and_its_material_and_needs_research_and_a_bench() {
    let mut game = Game::new(70_006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let part = damage();
    let (material, count) = crate::routine_craft::material(&part);

    let err = game.craft_subroutine(part).unwrap_err();
    assert!(err.contains("Workbench"), "{err}");

    stand_an_extraction_bench(&mut game, 30, 30);
    let err = game.craft_subroutine(part).unwrap_err();
    assert!(err.contains("Research"), "{err}");

    know(&mut game, &part);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(material.clone(), 9_999);
    let err = game.craft_subroutine(part).unwrap_err();
    assert!(err.contains("blank"), "{err}");

    give(&mut game, &ItemId::from(ids::ROUTINE_DISK), 2);
    let err = game.craft_subroutine(part).unwrap_err();
    assert!(err.contains(&format!("{count}")), "{err}");
    assert_eq!(game.blank_disks_held(), 2, "a refusal spent a disk");

    give(&mut game, &material, count + 1);
    game.craft_subroutine(part).unwrap();

    assert_eq!(game.parts_held(&part), 1);
    assert_eq!(game.blank_disks_held(), 1);
    assert_eq!(
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&material),
        1
    );
}

#[test]
fn a_build_spends_its_parts_waits_unslotted_and_never_reuses_an_id() {
    let mut game = bench_game(70_007);
    let first = build(&mut game, "  Mender ", &[heal()]);

    assert_eq!(game.parts_held(&heal()), 0, "the part was spent");
    let state = crafted(&game);
    assert!(state.unslotted.contains(&first));
    let record = state.built.iter().find(|r| r.id == first).unwrap();
    assert_eq!(record.name, "Mender", "the name is trimmed");

    game.destroy_built(std::slice::from_ref(&first));
    let second = build(&mut game, "Again", &[heal()]);
    assert_ne!(first, second, "a destroyed routine's id is never reused");
}

#[test]
fn a_build_is_refused_for_a_name_a_missing_part_a_misfit_and_the_cap() {
    let mut game = bench_game(70_008);
    stock(&mut game, &heal(), 1);

    assert!(game.build_routine("", &[heal()]).is_err(), "no name");
    assert!(
        game.build_routine(&"x".repeat(25), &[heal()]).is_err(),
        "a 25 character name"
    );
    let err = game.build_routine("Hurt", &[damage()]).unwrap_err();
    assert!(err.contains("missing"), "{err}");
    let err = game
        .build_routine("Nope", &[heal(), Subroutine::Range(RangeKind::Near)])
        .unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(game.parts_held(&heal()), 1, "no refusal spends anything");

    // The cap: 2 parts to start, one more per capacity node researched.
    let three = [
        heal(),
        Subroutine::Range(RangeKind::Near),
        Subroutine::Area(AreaKind::Group),
    ];
    for part in &three {
        stock(&mut game, part, 1);
    }
    assert_eq!(game.part_cap(), crate::tuning::CRAFT_PART_CAP_START);
    let err = game.build_routine("Big", &three).unwrap_err();
    assert!(err.contains("at most 2"), "{err}");

    game.world
        .resource_mut::<Research>()
        .0
        .insert(capacity_node_id(3));
    assert_eq!(game.part_cap(), 3);
    game.build_routine("Big", &three).unwrap();
}

#[test]
fn extracted_parts_build_without_being_researched() {
    let mut game = bench_game(70_009);
    assert!(!game.subroutine_known(&damage()));
    build(&mut game, "Bite", &[damage()]);
}

#[test]
fn installing_writes_a_free_slot_for_the_player_and_a_companion() {
    let mut game = bench_game(70_010);
    let player = game.player_entity();
    let pet = companion(&mut game);
    let a = build(&mut game, "Mender", &[heal()]);
    let b = build(&mut game, "Bite", &[damage()]);

    // The player's two slots are Decompile and Patch.
    let err = game.install_built(player, &a).unwrap_err();
    assert!(err.contains("slot"), "{err}");
    assert!(crafted(&game).unslotted.contains(&a), "a refusal keeps it");

    game.install_built(pet, &a).unwrap();
    assert!(held_ids(&game, pet).contains(&a));
    assert!(!crafted(&game).unslotted.contains(&a));
    assert!(
        game.install_built(pet, &a).is_err(),
        "it exists in one place only"
    );

    make_room_for_routine(&mut game, player);
    game.install_built(player, &b).unwrap();
    assert!(held_ids(&game, player).contains(&b));
}

#[test]
fn a_companion_cannot_be_given_emulate_and_a_stranger_cannot_be_given_anything() {
    let mut game = bench_game(70_011);
    let emulate = build(
        &mut game,
        "Mimic",
        &[Subroutine::Core(Core::Emulate, Version::V1)],
    );
    let pet = companion(&mut game);
    assert_eq!(
        game.install_built(pet, &emulate).unwrap_err(),
        "Only you can emulate."
    );

    let wild = spawn_wild_without_routine(&mut game, "scrapper", 6, 6);
    assert!(game.install_built(wild, &emulate).is_err());
    assert!(
        game.world.get::<Tamed>(wild).is_none(),
        "the fixture is not owned"
    );

    let player = game.player_entity();
    make_room_for_routine(&mut game, player);
    game.install_built(player, &emulate)
        .expect("the player may carry Emulate");
}

#[test]
fn uninstalling_a_built_routine_destroys_it_and_its_def() {
    let mut game = bench_game(70_012);
    let player = game.player_entity();
    make_room_for_routine(&mut game, player);
    let id = build(&mut game, "Mender", &[heal()]);
    game.install_built(player, &id).unwrap();
    let slot = held_ids(&game, player)
        .iter()
        .position(|r| *r == id)
        .unwrap();

    game.uninstall_routine(player, slot).unwrap();

    assert!(crafted(&game).built.iter().all(|r| r.id != id));
    assert!(crafted(&game).unslotted.iter().all(|u| *u != id));
    assert!(game.world.resource::<AbilityDb>().get(&id).is_none());
    assert!(!held_ids(&game, player).contains(&id));
}

// ---------------------------------------------------------------------------
// Task 4: where a built routine is destroyed, and where it is not
// ---------------------------------------------------------------------------

/// A companion carrying one built routine, and that routine's id.
fn companion_with_a_routine(game: &mut Game) -> (Entity, String) {
    let pet = companion(game);
    let id = build(game, "Mender", &[heal()]);
    game.install_built(pet, &id).unwrap();
    assert!(game.world.resource::<AbilityDb>().get(&id).is_some());
    (pet, id)
}

fn is_gone(game: &Game, id: &str) -> bool {
    crafted(game).built.iter().all(|r| r.id != id)
        && game.world.resource::<AbilityDb>().get(id).is_none()
}

#[test]
fn selling_a_companion_destroys_its_built_routines() {
    let mut game = bench_game(70_013);
    stand_in_base(&mut game);
    let market = spawn_market(&mut game);
    let (pet, id) = companion_with_a_routine(&mut game);

    game.sell_companion(market, pet).unwrap();

    assert!(is_gone(&game, &id));
}

#[test]
fn fusing_companions_destroys_both_parents_built_routines() {
    let mut game = bench_game(70_014);
    unlock_research_chain(&mut game, "program_refactoring");
    let (a, first) = companion_with_a_routine(&mut game);
    let b = companion(&mut game);
    let second = build(&mut game, "Bite", &[damage()]);
    game.install_built(b, &second).unwrap();

    game.fuse_companions(a, b, None).unwrap();

    assert!(is_gone(&game, &first));
    assert!(is_gone(&game, &second));
}

#[test]
fn tearing_a_companion_down_at_the_workbench_destroys_the_rest() {
    let mut game = bench_game(70_015);
    let (pet, id) = companion_with_a_routine(&mut game);
    let index = game
        .extractable_routines(pet)
        .iter()
        .position(|r| r.ability == id)
        .expect("a built routine is offered like any other");
    let parts = game.parts_of_routine(&id);

    game.extract_routine(pet, index).unwrap();

    assert!(is_gone(&game, &id), "the routine itself is taken apart");
    for (part, count) in crate::routine_craft::parts_used(&parts) {
        assert!(
            game.parts_held(&part) >= count,
            "a built routine's own parts come back out"
        );
    }
}

/// A committed build consumes the program for good (`consume_site`), and
/// the built routines it held go with it.
#[test]
fn a_finished_build_destroys_the_built_routines_of_the_program_it_spent() {
    let mut game = bench_game(70_030);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    let (pet, id) = companion_with_a_routine(&mut game);
    game.world.entity_mut(pet).insert(components::ZonePortal(1));
    tame_at_zone(&mut game, 1); // the crew that is left to build it
    game.place_structure("extractor", 1, 0, Some(pet))
        .expect("the program pays for the order");
    assert!(
        crafted(&game).built.iter().any(|r| r.id == id),
        "a cancellable order has not yet consumed it"
    );

    let (px, py) = game.base_pos().unwrap();
    let site = game.build_site_at(px + 1, py).unwrap();
    game.consume_site(site);

    assert!(is_gone(&game, &id));
}

#[test]
fn losing_a_companion_for_good_under_permadeath_destroys_its_built_routines() {
    let mut game = bench_game(70_016);
    *game.world.resource_mut::<DifficultyMode>() = DifficultyMode::Permadeath;
    let (pet, id) = companion_with_a_routine(&mut game);

    game.bench_or_dissolve(pet);

    assert!(is_gone(&game, &id));
}

#[test]
fn a_downed_companion_keeps_its_built_routines() {
    let mut game = bench_game(70_017);
    let (pet, id) = companion_with_a_routine(&mut game);

    game.bench_or_dissolve(pet);

    assert!(game.world.get_entity(pet).is_ok(), "benched, not gone");
    assert!(held_ids(&game, pet).contains(&id));
    assert!(game.world.resource::<AbilityDb>().get(&id).is_some());
}

#[test]
fn a_posted_companion_keeps_its_built_routines() {
    let mut game = bench_game(70_018);
    let (pet, id) = companion_with_a_routine(&mut game);
    game.world
        .entity_mut(pet)
        .insert(components::PostedAt((7, 7)));

    assert!(game.extract_routine(pet, 0).is_err(), "posted is refused");
    assert!(held_ids(&game, pet).contains(&id));
    assert!(game.world.resource::<AbilityDb>().get(&id).is_some());
}

// ---------------------------------------------------------------------------
// Task 5: research
// ---------------------------------------------------------------------------

#[test]
fn the_tree_has_a_node_for_every_part_and_a_capacity_ladder() {
    let game = Game::new(70_019, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::research::ResearchDb>();
    for part in Subroutine::all() {
        let node = db
            .get(&part_node_id(&part))
            .unwrap_or_else(|| panic!("{} has no research node", part.label()));
        assert_eq!(node.tree, ResearchTree::Routines);
        assert!(node.teaches.is_none() && node.materials.is_empty());
        if let Subroutine::Core(core, v) = part
            && v != Version::V1
        {
            let below = Subroutine::Core(core, Version::from_number(v.number() - 1).unwrap());
            assert_eq!(node.requires, vec![part_node_id(&below)]);
        }
    }
    let mut zone = 0;
    for cap in 3..=6 {
        let node = db.get(&capacity_node_id(cap)).expect("a capacity node");
        assert!(node.min_zone > zone, "one capacity step per zone");
        zone = node.min_zone;
    }
    assert_eq!(db.get(&capacity_node_id(3)).unwrap().min_zone, 1);
}

#[test]
fn researching_capacity_nodes_raises_the_cap_one_at_a_time() {
    let mut game = Game::new(70_020, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.part_cap(), 2);
    unlock_research_chain(&mut game, &capacity_node_id(3));
    assert_eq!(game.part_cap(), 3);
    unlock_research_chain(&mut game, &capacity_node_id(5));
    assert_eq!(game.part_cap(), 5, "the chain pulls in the one before");
}

#[test]
fn a_new_game_can_rebuild_patch_but_not_yet_craft_a_damage_core() {
    let game = Game::new(70_021, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.subroutine_known(&heal()));
    assert!(!game.subroutine_known(&damage()));
}

#[test]
fn a_part_node_is_listed_once_its_zone_is_reached_and_not_before() {
    let mut game = Game::new(70_022, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "routine_fabrication");
    let listed = |game: &mut Game, id: &str| {
        game.research_nodes(ResearchTree::Routines)
            .iter()
            .any(|row| row.id == id)
    };
    let v2 = part_node_id(&Subroutine::Core(Core::Damage, Version::V2));
    let v1 = part_node_id(&damage());
    assert!(listed(&mut game, &v1), "a zone-1 part is visible");
    assert!(!listed(&mut game, &v2), "a zone-2 part is not, yet");
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    assert!(listed(&mut game, &v2));
}

// ---------------------------------------------------------------------------
// Task 7: a new game
// ---------------------------------------------------------------------------

#[test]
fn a_new_game_holds_decompile_and_a_built_patch_and_nothing_else() {
    let game = Game::new(70_023, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert_eq!(
        held_ids(&game, player),
        vec![
            crate::abilities::DECOMPILE_ABILITY_ID.to_string(),
            "crafted/1".to_string()
        ]
    );
    let state = crafted(&game);
    assert_eq!(
        state.built,
        vec![CraftedRoutine {
            id: "crafted/1".into(),
            name: "Patch".into(),
            parts: vec![heal()],
        }]
    );
    assert!(state.unslotted.is_empty());
    assert!(!game.knows_routine("hot_patch"), "no authored grant");
}

// ---------------------------------------------------------------------------
// Task 8: the save
// ---------------------------------------------------------------------------

fn round_trip(game: &mut Game, tag: &str) -> Game {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();
    Game::load(&path, &test_assets_dir()).unwrap()
}

#[test]
fn a_built_routine_survives_a_save_and_load_on_the_player_and_a_companion() {
    let mut game = bench_game(70_024);
    let player = game.player_entity();
    make_room_for_routine(&mut game, player);
    let pet = companion(&mut game);
    let mine = build(
        &mut game,
        "Wide Mender",
        &[heal(), Subroutine::Area(AreaKind::Group)],
    );
    let theirs = build(&mut game, "Bite", &[damage()]);
    let spare = build(&mut game, "Spare", &[heal()]);
    game.install_built(player, &mine).unwrap();
    game.install_built(pet, &theirs).unwrap();
    let pet_program = *game.world.get::<components::ProgramId>(pet).unwrap();
    let before = crafted(&game);

    // A battle in between, so the routine has been through a fight.
    start_battle_with_a_wild_program(&mut game);
    resolve_a_planned_round(&mut game);
    let mut loaded = round_trip(&mut game, "craft_roundtrip");

    assert_eq!(crafted(&loaded), before, "the record travels");
    let player = loaded.player_entity();
    assert!(held_ids(&loaded, player).contains(&mine));
    assert!(
        loaded
            .actor_abilities(player)
            .iter()
            .any(|a| a.id == mine && a.name == "Wide Mender"),
        "the player's slot resolves after the load"
    );
    let pet = loaded
        .world
        .query::<(Entity, &components::ProgramId)>()
        .iter(&loaded.world)
        .find(|(_, id)| **id == pet_program)
        .map(|(e, _)| e)
        .expect("the companion came back");
    assert!(held_ids(&loaded, pet).contains(&theirs));
    assert!(
        loaded
            .actor_abilities(pet)
            .iter()
            .any(|a| a.id == theirs && a.name == "Bite"),
        "the companion's slot resolves after the load"
    );
    assert!(crafted(&loaded).unslotted.contains(&spare));
    assert!(loaded.world.resource::<AbilityDb>().get(&spare).is_some());
}

#[test]
fn an_uninstalled_routine_is_gone_from_the_save_and_the_crafted_tier() {
    let mut game = bench_game(70_025);
    let player = game.player_entity();
    make_room_for_routine(&mut game, player);
    let id = build(&mut game, "Mender", &[heal()]);
    game.install_built(player, &id).unwrap();
    let slot = held_ids(&game, player)
        .iter()
        .position(|r| *r == id)
        .unwrap();
    game.uninstall_routine(player, slot).unwrap();

    let loaded = round_trip(&mut game, "craft_uninstalled");

    assert!(crafted(&loaded).built.iter().all(|r| r.id != id));
    assert!(loaded.world.resource::<AbilityDb>().get(&id).is_none());
    assert_eq!(
        crafted(&loaded).next_id,
        crafted(&game).next_id,
        "the counter still only grows across a load"
    );
}

// ---------------------------------------------------------------------------
// Task 9: the Workbench pages
// ---------------------------------------------------------------------------

#[test]
fn the_build_rows_name_why_a_core_takes_no_part_in_a_slot() {
    let game = bench_game(70_040);

    let rows = game.build_rows(&[]);
    assert_eq!(
        rows.iter().map(|r| r.slot).collect::<Vec<_>>(),
        ["Core", "Range", "Area", "Timing", "Damage type", "Status"]
    );
    assert!(rows[0].unavailable.is_none());
    assert!(rows[1..].iter().all(|r| r.unavailable.is_some()));

    let heal_rows = game.build_rows(&[heal(), Subroutine::Area(AreaKind::Group)]);
    assert_eq!(heal_rows[0].chosen, vec![heal()]);
    assert_eq!(heal_rows[2].chosen, vec![Subroutine::Area(AreaKind::Group)]);
    assert!(heal_rows[1].unavailable.is_none(), "Heal takes a Range");
    assert!(heal_rows[3].unavailable.is_none(), "and a Trickle");
    assert_eq!(
        heal_rows[4].unavailable,
        Some("only Damage and Drain deal a type"),
        "the reason replaces the part"
    );
    assert!(heal_rows[5].unavailable.is_some());

    let tp = game.build_rows(&[Subroutine::Core(Core::Teleport, Version::V1)]);
    assert!(tp[1..].iter().all(|r| r.unavailable.is_some()));
}

#[test]
fn a_slots_list_offers_only_what_fits_with_the_count_held() {
    let mut game = bench_game(70_041);
    let group = Subroutine::Area(AreaKind::Group);
    stock(&mut game, &group, 3);

    let cores = game.build_part_options("Core", &[]);
    assert_eq!(
        cores.len(),
        Subroutine::all()
            .iter()
            .filter(|p| matches!(p, Subroutine::Core(..)))
            .count()
    );
    assert!(
        game.build_part_options("Area", &[]).is_empty(),
        "no Core, no list"
    );

    let areas = game.build_part_options("Area", &[heal()]);
    assert_eq!(
        areas.iter().map(|o| o.part).collect::<Vec<_>>(),
        vec![group],
        "an ally Core has a Group and never an Everyone"
    );
    assert_eq!(areas[0].held, 3);

    let types = game.build_part_options("Damage type", &[damage()]);
    assert_eq!(types.len(), 5);
    assert!(types.iter().all(|o| o.held == 0));
    assert!(game.build_part_options("Damage type", &[heal()]).is_empty());
}

#[test]
fn the_preview_quotes_what_the_build_will_run_as() {
    let mut game = bench_game(70_042);
    let parts = [heal(), Subroutine::Range(RangeKind::Near)];
    let empty = game.build_preview("Mender", &[]);
    assert_eq!(empty.description, "");
    assert!(empty.blocked.is_some());

    let missing = game.build_preview("Mender", &parts);
    assert_eq!(missing.parts_missing.len(), 2, "both parts are short");
    assert!(missing.blocked.as_ref().unwrap().contains("missing"));

    for part in &parts {
        stock(&mut game, part, 1);
    }
    let preview = game.build_preview("Mender", &parts);
    assert!(preview.parts_missing.is_empty());
    assert_eq!(preview.blocked, None);
    assert_eq!((preview.part_count, preview.part_cap), (2, 2));
    assert_eq!(preview.parts_used.len(), 2);

    let id = game.build_routine("Mender", &parts).unwrap();
    let def = game.world.resource::<AbilityDb>().get(&id).unwrap().clone();
    assert_eq!(preview.description, def.description);
    assert_eq!(preview.cooldown, def.cooldown);
    assert_eq!(
        preview.power_cost,
        crate::abilities::routine_power_cost(&def)
    );
    assert!(!preview.description.is_empty());
}

#[test]
fn the_preview_says_when_the_cap_or_the_bench_blocks_the_build() {
    let mut game = Game::new(70_043, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let three = [
        heal(),
        Subroutine::Range(RangeKind::Near),
        Subroutine::Area(AreaKind::Group),
    ];
    for part in &three {
        stock(&mut game, part, 1);
    }
    let blocked = game.build_preview("Big", &three).blocked.unwrap();
    assert!(blocked.contains("at most 2"), "{blocked}");

    let two = &three[..2];
    let blocked = game.build_preview("Fine", two).blocked.unwrap();
    assert!(blocked.contains("Workbench"), "{blocked}");
}

#[test]
fn the_subroutines_list_marks_the_unresearched_and_counts_what_is_held() {
    let mut game = bench_game(70_044);
    stock(&mut game, &damage(), 2);
    let rows = game.subroutine_rows();
    assert_eq!(rows.len(), Subroutine::all().len());

    let row = |part: Subroutine| rows.iter().find(|r| r.part == part).unwrap().clone();
    let healing = row(heal());
    assert!(healing.known && healing.locked.is_none());
    assert_eq!(healing.slot, "Core");

    let dmg = row(damage());
    assert!(!dmg.known);
    assert_eq!(dmg.locked, Some("locked · research to unlock"));
    assert_eq!(dmg.held, 2);
    let (material, count) = crate::routine_craft::material(&damage());
    assert_eq!(dmg.material, game.item_name(&material));
    assert_eq!(dmg.material_count, count);

    know(&mut game, &damage());
    let dmg = game
        .subroutine_rows()
        .into_iter()
        .find(|r| r.part == damage())
        .unwrap();
    assert!(dmg.known && dmg.locked.is_none());
}

#[test]
fn the_built_page_lists_only_routines_not_in_a_slot() {
    let mut game = bench_game(70_045);
    let player = game.player_entity();
    make_room_for_routine(&mut game, player);
    let a = build(&mut game, "Mender", &[heal()]);
    let b = build(&mut game, "Bite", &[damage()]);

    let ids = |game: &Game| -> Vec<String> {
        game.built_routine_rows()
            .into_iter()
            .map(|r| r.id)
            .collect()
    };
    assert_eq!(ids(&game), vec![a.clone(), b.clone()]);
    let first = &game.built_routine_rows()[0];
    assert_eq!(first.name, "Mender");
    assert_eq!(first.parts, vec![heal().label()]);
    assert!(!first.description.is_empty());

    game.install_built(player, &a).unwrap();
    assert_eq!(ids(&game), vec![b]);
}
