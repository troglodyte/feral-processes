//! Weapon crafting's wiring into `Game` (ADR 0031, phase 2): the crafted
//! tier, part items, the craft / build / delete verbs, and the save.

use super::support::*;
use crate::affixes::AffixId;
use crate::gear_craft::weapon::{Category, Frame, WeaponPart, frame_bill};
use crate::gear_craft::{FitError, GearCraftRefusal, GearPart};
use crate::resources::{GearDesigns, KnownAffixes, Research};
use crate::routine_craft::{StatusKind, Version};
use crate::tuning::QUALITY_DEFAULT;
use crate::*;

fn edge() -> WeaponPart {
    WeaponPart::Frame(Frame::Edge, Version::V1)
}

fn bleed() -> WeaponPart {
    WeaponPart::Rider(StatusKind::Bleed)
}

fn bench_game(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    game
}

fn know(game: &mut Game, part: WeaponPart) {
    game.world
        .resource_mut::<Research>()
        .0
        .insert(part.node_id());
}

fn parts(list: &[WeaponPart]) -> Vec<GearPart> {
    list.iter().copied().map(GearPart::Weapon).collect()
}

/// A saved design on the edge frame with a bleed rider.
fn designed(game: &mut Game) -> u32 {
    for part in [edge(), bleed()] {
        know(game, part);
    }
    game.create_design("Leaker", parts(&[edge(), bleed()]))
        .unwrap()
}

/// Compiles stored design `id` through the production verb, which reuses
/// the stored design for its name and parts.
fn compile_id(game: &mut Game, id: u32) -> Result<GearCopy, GearCraftRefusal> {
    let design = game
        .gear_designs()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap();
    game.compile_weapon(&design.name, design.parts)
}

/// Builds a copy of `id`, supplying whatever it costs.
fn built(game: &mut Game, id: u32) -> GearCopy {
    let design = game
        .gear_designs()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap();
    for part in design.weapon_parts() {
        give(game, &ItemId::weapon_part(part), 1);
    }
    let (frame, version) = design.frame().unwrap();
    let (material, count) = frame_bill(frame, version);
    give(game, &material, count);
    compile_id(game, id).unwrap()
}

fn round_trip(game: &mut Game, tag: &str) -> Game {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();
    Game::load(&path, &test_assets_dir()).unwrap()
}

/// Empties the pack of `item`, so a bill is not already met by the starter
/// kit.
fn empty_of(game: &mut Game, item: &ItemId) {
    let held = game.held_in_reach(item);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(item.clone(), held);
}

fn def(game: &Game, id: &str) -> Option<crate::items_db::ItemDef> {
    game.world.resource::<ItemDb>().get(id).cloned()
}

// ---------------------------------------------------------------------------
// Task 1: the crafted tier and its door
// ---------------------------------------------------------------------------

#[test]
fn a_crafted_id_resolves_through_get_and_stays_out_of_the_catalogue_walks() {
    let mut game = bench_game(71_001);
    let id = designed(&mut game);
    let key = format!("crafted_gear/{id}");
    let found = def(&game, &key).expect("a crafted id resolves through get");
    assert_eq!(found.name, "Leaker");
    assert!(found.on_hit_rider.is_some());
    assert!(
        game.world
            .resource::<ItemDb>()
            .all()
            .all(|d| d.id.as_str() != key),
        "a built weapon is not content to be found"
    );
    assert!(game.item_defs().iter().all(|d| d.id.as_str() != key));
}

#[test]
fn a_crafted_weapon_equips_wears_and_sells() {
    let mut game = bench_game(71_002);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    let player = game.player_entity();
    game.equip(player, &copy).unwrap();
    let worn = game.world.get::<components::Equipment>(player).unwrap();
    assert_eq!(worn.weapon.as_ref().unwrap().copy, copy);
    game.unequip(player, EquipmentSlot::Weapon).unwrap();

    let market = spawn_market_at(&mut game, 5, 5);
    assert_eq!(game.count_copies(&copy), 1);
    from_inside_the_base(&mut game, |g| g.sell_item(market, copy.clone(), 1)).unwrap();
    assert_eq!(game.count_copies(&copy), 0);
    assert_eq!(
        game.buyback_options(market).len(),
        1,
        "it sold, and can be bought back"
    );
}

#[test]
fn a_crafted_copy_cannot_be_fused_and_fuse_all_skips_it() {
    let mut game = bench_game(71_003);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    let second = built(&mut game, id);
    assert_eq!(copy, second, "two copies of one design are one stack");
    assert_eq!(game.fusion_refusal(&copy), Some(FusionRefusal::CraftedGear));
    assert!(game.fuse_item(&copy).unwrap_err().contains("Workbench"));
    assert_eq!(
        game.fuse_all_items().unwrap_err(),
        "Nothing in cargo has a matching pair to fuse."
    );
    assert_eq!(game.count_copies(&copy), 2, "nothing was consumed");
    // An authored copy of the same slot is still fusable, so the refusal is
    // the crafted copy's own.
    let authored = gear(&ItemId::from("shiv_routine"), 0);
    assert_eq!(game.fusion_refusal(&authored), None);
}

// ---------------------------------------------------------------------------
// Task 2: part items
// ---------------------------------------------------------------------------

#[test]
fn every_catalogue_part_has_an_item_and_a_key_that_finds_it_again() {
    let game = bench_game(71_004);
    let db = game.world.resource::<ItemDb>();
    for part in WeaponPart::all() {
        let id = ItemId::weapon_part(part);
        let item = db
            .get(id.as_str())
            .unwrap_or_else(|| panic!("{part:?} has no item"));
        assert!(item.name.starts_with("Weapon part · "), "{}", item.name);
        assert_eq!(id.weapon_part_of(), Some(part), "the key round-trips");
        assert!(item.craftable.is_none() && item.droppable.is_none());
    }
    let keys: std::collections::BTreeSet<_> =
        WeaponPart::all().into_iter().map(|p| p.key()).collect();
    assert_eq!(keys.len(), WeaponPart::all().len(), "keys are unique");
}

#[test]
fn a_weapon_part_is_never_stocked_by_a_shelf() {
    let game = bench_game(71_005);
    let db = game.world.resource::<ItemDb>();
    assert!(
        db.creation_shelf()
            .iter()
            .all(|row| ItemId::from(row.id.as_str()).weapon_part_of().is_none())
    );
}

#[test]
fn building_crafts_an_unheld_part_only_once_researched_and_spends_its_bill() {
    let mut game = bench_game(71_006);
    know(&mut game, edge());
    let id = game.create_design("Plain", parts(&[edge()])).unwrap();
    game.world
        .resource_mut::<Research>()
        .0
        .remove(&edge().node_id());
    assert_eq!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::PartNotResearched(edge()))
    );
    know(&mut game, edge());
    let (frame_item, frame_count) = frame_bill(Frame::Edge, Version::V1);
    let (part_item, part_count) = crate::gear_craft::weapon::part_bill(edge());
    assert_eq!(frame_item, part_item, "premise: one material pays both");
    empty_of(&mut game, &frame_item);
    give(&mut game, &frame_item, frame_count + part_count - 1);
    assert!(matches!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::ShortOf { .. })
    ));
    assert_eq!(
        game.weapon_parts_held(edge()),
        0,
        "a refusal spends nothing"
    );
    give(&mut game, &frame_item, 1);
    compile_id(&mut game, id).unwrap();
    assert_eq!(game.held_in_reach(&frame_item), 0, "the bill was spent");
    assert_eq!(
        game.weapon_parts_held(edge()),
        0,
        "the crafted part was used"
    );
}

// ---------------------------------------------------------------------------
// Task 3: create, build, cap
// ---------------------------------------------------------------------------

#[test]
fn a_design_needs_a_name_a_fit_and_parts_the_player_may_use() {
    let mut game = bench_game(71_007);
    assert_eq!(
        game.create_design("Ghost", parts(&[edge()])),
        Err(GearCraftRefusal::PartNotResearched(edge())),
        "neither researched nor held"
    );
    know(&mut game, edge());
    assert_eq!(
        game.create_design("  ", parts(&[edge()])),
        Err(GearCraftRefusal::NameEmpty)
    );
    assert_eq!(
        game.create_design(&"x".repeat(25), parts(&[edge()])),
        Err(GearCraftRefusal::NameTooLong { max: 24 })
    );
    assert_eq!(
        game.create_design("Bare", parts(&[])),
        Err(GearCraftRefusal::Fit(FitError::NoFrame))
    );
    // A held part is usable without its research.
    give(&mut game, &ItemId::weapon_part(WeaponPart::Extend), 1);
    let id = game
        .create_design(" Reacher ", parts(&[edge(), WeaponPart::Extend]))
        .unwrap();
    assert_eq!(game.gear_designs()[0].name, "Reacher", "trimmed");
    assert_eq!(game.gear_designs()[0].id, id);
}

#[test]
fn the_cap_is_enforced_and_raised_by_capacity_research() {
    let mut game = bench_game(71_008);
    assert_eq!(game.gear_part_cap(), crate::tuning::GEAR_PART_CAP_START);
    let picks = [
        WeaponPart::Extend,
        WeaponPart::Tilt(crate::gear_craft::weapon::Tilt::Precise),
        WeaponPart::Capture,
    ];
    for part in picks.into_iter().chain([edge()]) {
        know(&mut game, part);
    }
    assert_eq!(
        game.create_design("Crowded", parts(&[edge(), picks[0], picks[1], picks[2]])),
        Err(GearCraftRefusal::Fit(FitError::OverCap {
            cap: crate::tuning::GEAR_PART_CAP_START
        }))
    );
    game.world
        .resource_mut::<Research>()
        .0
        .insert(crate::gear_craft::capacity_node_id(3));
    assert_eq!(game.gear_part_cap(), 3);
    game.create_design("Crowded", parts(&[edge(), picks[0], picks[1], picks[2]]))
        .unwrap();
}

#[test]
fn an_affix_must_be_researched_or_salvaged() {
    let mut game = bench_game(71_009);
    know(&mut game, edge());
    let honed = AffixId::from("honed");
    let design = vec![GearPart::Weapon(edge()), GearPart::Affix(honed.clone())];
    assert_eq!(
        game.create_design("Sharp", design.clone()),
        Err(GearCraftRefusal::AffixNotKnown(honed.clone()))
    );
    game.world
        .resource_mut::<KnownAffixes>()
        .0
        .insert(honed.clone());
    let id = game.create_design("Sharp", design).unwrap();
    let stats = def(&game, &format!("crafted_gear/{id}"))
        .unwrap()
        .equipment
        .unwrap()
        .1;
    let plain_id = { game.create_design("Plain", parts(&[edge()])).unwrap() };
    let plain = def(&game, &format!("crafted_gear/{plain_id}"))
        .unwrap()
        .equipment
        .unwrap()
        .1;
    assert_ne!(
        format!("{stats:?}"),
        format!("{plain:?}"),
        "the affix folded into the weapon's stats"
    );
}

#[test]
fn a_build_consumes_its_parts_and_the_frame_bill_and_mints_an_ordinary_copy() {
    let mut game = bench_game(71_010);
    let id = designed(&mut game);
    for part in [edge(), bleed()] {
        give(&mut game, &ItemId::weapon_part(part), 1);
    }
    let (material, count) = frame_bill(Frame::Edge, Version::V1);
    empty_of(&mut game, &material);
    let have = game.held_in_reach(&material);
    assert!(have < count, "premise: the bill is not already in the pack");
    let refusal = compile_id(&mut game, id).unwrap_err();
    assert!(
        matches!(refusal, GearCraftRefusal::ShortOf { ref item, .. } if *item == material),
        "{refusal:?}"
    );
    assert_eq!(
        game.weapon_parts_held(edge()),
        1,
        "a refusal spends nothing"
    );

    give(&mut game, &material, count);
    let copy = compile_id(&mut game, id).unwrap();
    assert_eq!(
        copy,
        GearCopy::plain(ItemId::from(format!("crafted_gear/{id}")))
    );
    assert_eq!(game.count_copies(&copy), 1);
    assert_eq!(game.weapon_parts_held(edge()), 0);
    assert_eq!(game.weapon_parts_held(bleed()), 0);
    assert_eq!(game.held_in_reach(&material), have);
    assert_eq!(game.gear_designs().len(), 1, "the design stays");
    assert!(
        matches!(
            compile_id(&mut game, id),
            Err(GearCraftRefusal::ShortOf { ref item, .. }) if *item == material
        ),
        "building again needs the bill again"
    );
}

#[test]
fn a_built_weapon_is_worth_less_than_everything_the_build_consumed() {
    let mut game = bench_game(71_011);
    let id = designed(&mut game);
    let worth = def(&game, &format!("crafted_gear/{id}"))
        .unwrap()
        .value
        .unwrap();
    let db = game.world.resource::<ItemDb>();
    let (material, count) = frame_bill(Frame::Edge, Version::V1);
    let bill = ItemDb::value_of(db.get(material.as_str()).unwrap()) * count;
    let parts: u32 = [edge(), bleed()]
        .into_iter()
        .map(|p| ItemDb::value_of(db.get(ItemId::weapon_part(p).as_str()).unwrap()))
        .sum();
    assert!(worth < bill + parts, "no Credit loop through the bench");
}

// ---------------------------------------------------------------------------
// Task 5: the save
// ---------------------------------------------------------------------------

#[test]
fn a_design_a_worn_copy_and_a_known_affix_survive_a_save_and_load() {
    let mut game = bench_game(71_030);
    let honed = AffixId::from("honed");
    game.world
        .resource_mut::<KnownAffixes>()
        .0
        .insert(honed.clone());
    for part in [edge(), bleed()] {
        know(&mut game, part);
    }
    let id = game
        .create_design(
            "Everything",
            vec![
                GearPart::Weapon(edge()),
                GearPart::Weapon(bleed()),
                GearPart::Affix(honed.clone()),
            ],
        )
        .unwrap();
    let copy = built(&mut game, id);
    let player = game.player_entity();
    game.equip(player, &copy).unwrap();
    let before = def(&game, &copy.item.0).unwrap();
    let designs = game.world.resource::<GearDesigns>().clone();

    let loaded = round_trip(&mut game, "gear_craft_roundtrip");
    assert_eq!(*loaded.world.resource::<GearDesigns>(), designs);
    assert!(loaded.world.resource::<KnownAffixes>().0.contains(&honed));
    let after = def(&loaded, &copy.item.0).expect("the worn copy resolves after the load");
    assert_eq!(
        format!("{after:?}"),
        format!("{before:?}"),
        "the def is re-derived, whole"
    );
    assert!(
        after.on_hit_rider.is_some(),
        "runtime-only rider is re-derived"
    );
    let worn = loaded
        .world
        .get::<components::Equipment>(loaded.player_entity())
        .unwrap();
    assert_eq!(worn.weapon.as_ref().unwrap().copy, copy);
}

// ---------------------------------------------------------------------------
// Task 6: the views
// ---------------------------------------------------------------------------

#[test]
fn the_preview_quotes_the_built_weapons_numbers() {
    let mut game = bench_game(71_040);
    let id = designed(&mut game);
    empty_of(&mut game, &ItemId::from(ids::CORE_FRAGMENT));
    let def = def(&game, &format!("crafted_gear/{id}")).unwrap();

    let preview = game.design_preview("Leaker", &parts(&[edge(), bleed()]));
    assert!(preview.power.is_some());
    assert_eq!(preview.description, def.description);
    assert_eq!(preview.range, 1);
    assert_eq!((preview.part_count, preview.part_cap), (1, 2));
    assert!(matches!(
        preview.blocked,
        Some(GearCraftRefusal::ShortOf { .. })
    ));
    assert_eq!(
        preview.parts_missing,
        vec![("Edge v1".to_string(), 1), ("Rider: Bleed".to_string(), 1)]
    );

    let bad = game.design_preview("", &parts(&[edge()]));
    assert_eq!(bad.blocked, Some(GearCraftRefusal::NameEmpty));
}

#[test]
fn a_preview_holding_more_of_a_part_than_it_needs_lists_nothing_missing() {
    let mut game = bench_game(71_042);
    give(&mut game, &ItemId::weapon_part(edge()), 3);
    let preview = game.design_preview("Spare", &parts(&[edge()]));
    assert!(
        preview.parts_missing.is_empty(),
        "{:?}",
        preview.parts_missing
    );
}

#[test]
fn a_part_that_does_not_fit_is_greyed_with_the_reason() {
    let mut game = bench_game(71_041);
    know(&mut game, edge());
    know(&mut game, WeaponPart::Extend);
    let launcher = WeaponPart::Frame(Frame::Launcher, Version::V1);
    know(&mut game, launcher);

    let options = game.design_part_options(Category::Frame, &[], 0);
    let edge_option = options.iter().find(|o| o.part == edge()).unwrap();
    assert_eq!(edge_option.unavailable, None);
    assert!(
        options
            .iter()
            .all(|o| o.part != WeaponPart::Frame(Frame::Lance, Version::V1)),
        "an unresearched, unheld part is not offered"
    );

    let on_launcher = game.design_part_options(Category::Range, &[launcher], 0);
    assert_eq!(
        on_launcher[0].unavailable,
        Some(GearCraftRefusal::Fit(FitError::RangeAtMax(Frame::Launcher)))
    );
    let on_edge = game.design_part_options(Category::Range, &[edge()], 0);
    assert_eq!(on_edge[0].unavailable, None);
}

#[test]
fn the_slot_rows_name_what_fills_each_category() {
    let game = bench_game(71_042);
    let rows = game.design_slot_rows(&[edge(), bleed()]);
    assert_eq!(rows[0].category, Category::Frame);
    assert_eq!(rows[0].chosen, Some(edge()));
    let rider = rows.iter().find(|r| r.category == Category::Rider).unwrap();
    assert_eq!(rider.chosen, Some(bleed()));
    assert!(
        rows.iter()
            .filter(|r| !matches!(r.category, Category::Frame | Category::Rider))
            .all(|r| r.chosen.is_none())
    );
}

// ---------------------------------------------------------------------------
// Phase 3: salvage
// ---------------------------------------------------------------------------

fn open_weapon_tree(game: &mut Game) {
    unlock_research_chain(game, "weapon_bench");
}

fn ragged_copy(game: &mut Game, affixes: Vec<AffixId>) -> GearCopy {
    let copy = GearCopy::with_affixes(
        ItemId::from("ragged_edge"),
        Rarity::Ordinary,
        0,
        affixes,
        QUALITY_DEFAULT,
    );
    game.add_copies(&copy, 1);
    copy
}

#[test]
fn salvage_consumes_the_copy_hands_out_parts_discovers_nodes_and_learns_affixes() {
    let mut game = bench_game(72_101);
    open_weapon_tree(&mut game);
    let honed = AffixId::from("honed");
    let copy = ragged_copy(&mut game, vec![honed.clone()]);
    let parts = game.salvage_weapon(&copy).unwrap();
    assert!(
        parts.contains(&WeaponPart::Proc(
            crate::gear_craft::weapon::Proc::ClockSkew
        )),
        "ragged_edge grants clock_skew: {parts:?}"
    );
    assert_eq!(game.count_copies(&copy), 0, "the copy is consumed");
    let discovered = &game
        .world
        .resource::<crate::resources::DiscoveredResearch>()
        .0;
    for part in &parts {
        assert!(game.weapon_parts_held(*part) >= 1, "{part:?} handed out");
        assert!(discovered.contains(&part.node_id()), "{part:?} discovered");
    }
    assert!(game.world.resource::<KnownAffixes>().0.contains(&honed));
    assert!(game.affix_known(&honed));
}

#[test]
fn salvage_refuses_before_anything_is_spent() {
    let mut game = bench_game(72_102);
    let copy = ragged_copy(&mut game, vec![]);
    assert_eq!(
        game.salvage_weapon(&copy),
        Err(GearCraftRefusal::WeaponTreeClosed)
    );
    assert_eq!(game.count_copies(&copy), 1);

    open_weapon_tree(&mut game);
    let absent = GearCopy::plain(ItemId::from("arc_lance"));
    assert_eq!(
        game.salvage_weapon(&absent),
        Err(GearCraftRefusal::NoCopyHeld)
    );
    let module = GearCopy::plain(ItemId::from("trace_sniffer"));
    give(&mut game, &module.item, 1);
    assert_eq!(
        game.salvage_weapon(&module),
        Err(GearCraftRefusal::NotSalvageable)
    );
    assert_eq!(game.count_copies(&module), 1);

    know(&mut game, edge());
    let id = game.create_design("Mine", parts(&[edge()])).unwrap();
    let built = built(&mut game, id);
    assert_eq!(
        game.salvage_weapon(&built),
        Err(GearCraftRefusal::NotSalvageable)
    );
    assert_eq!(game.count_copies(&built), 1);
}

#[test]
fn a_proc_part_can_never_be_crafted_only_salvaged() {
    let mut game = bench_game(72_103);
    let proc = WeaponPart::Proc(crate::gear_craft::weapon::Proc::CoreDump);
    know(&mut game, edge());
    know(&mut game, proc);
    let id = game
        .create_design("Dumper", parts(&[edge(), proc]))
        .unwrap();
    give(&mut game, &ItemId::from("core_fragment"), 50);
    assert_eq!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::SalvageOnly(proc))
    );
    assert_eq!(game.weapon_parts_held(proc), 0);
}

#[test]
fn salvage_rows_list_authored_weapons_in_the_pack_with_their_parts() {
    let mut game = bench_game(72_150);
    assert!(game.salvage_rows().is_empty());
    let copy = ragged_copy(&mut game, vec![]);
    let id = designed(&mut game);
    built(&mut game, id);
    let rows = game.salvage_rows();
    assert_eq!(rows.len(), 1, "the built weapon is not listed: {rows:?}");
    assert_eq!(rows[0].copy, copy);
    assert!(!rows[0].parts.is_empty());
}

/// A bill naming one item on two lines is checked as the sum, so a short one
/// is refused with the item and not as a busy forge.
#[test]
fn a_bill_short_only_in_sum_names_the_item() {
    let mut game = bench_game(71_040);
    let item = ItemId::from(ids::CORE_FRAGMENT);
    empty_of(&mut game, &item);
    give(&mut game, &item, 4);
    assert_eq!(
        game.pay_gear_bill(&[(item.clone(), 3), (item.clone(), 3)]),
        Err(GearCraftRefusal::ShortOf {
            item,
            need: 6,
            held: 4
        })
    );
}

// ---------------------------------------------------------------------------
// A compile crafts the missing parts
// ---------------------------------------------------------------------------

fn fragment() -> ItemId {
    ItemId::from(ids::CORE_FRAGMENT)
}

/// The Edge-and-Bleed design with the pack emptied of fragments and no part
/// held; frame and both parts bill fragments.
fn bare_forge(seed: u32) -> (Game, u32) {
    let mut game = bench_game(seed);
    let id = designed(&mut game);
    empty_of(&mut game, &fragment());
    (game, id)
}

fn full_bill() -> u32 {
    let (_, frame) = frame_bill(Frame::Edge, Version::V1);
    let (_, edge_part) = crate::gear_craft::weapon::part_bill(edge());
    let (_, bleed_part) = crate::gear_craft::weapon::part_bill(bleed());
    frame + edge_part + bleed_part
}

#[test]
fn a_forge_with_no_parts_builds_from_materials_and_spends_exactly_the_bill() {
    let (mut game, id) = bare_forge(73_001);
    give(&mut game, &fragment(), full_bill() + 5);
    let copy = compile_id(&mut game, id).unwrap();
    assert_eq!(game.count_copies(&copy), 1);
    assert_eq!(game.held_in_reach(&fragment()), 5);
    assert_eq!(game.weapon_parts_held(edge()), 0);
    assert_eq!(game.weapon_parts_held(bleed()), 0);
}

#[test]
fn held_parts_are_spent_first_and_only_the_shortfall_is_billed() {
    let (mut game, id) = bare_forge(73_002);
    give(&mut game, &ItemId::weapon_part(edge()), 1);
    let (_, edge_part) = crate::gear_craft::weapon::part_bill(edge());
    let need = full_bill() - edge_part;
    give(&mut game, &fragment(), need);
    compile_id(&mut game, id).unwrap();
    assert_eq!(game.held_in_reach(&fragment()), 0);
    assert_eq!(game.weapon_parts_held(edge()), 0);
}

#[test]
fn a_bill_sums_across_the_frame_and_the_crafted_parts_and_a_short_sum_spends_nothing() {
    let (mut game, id) = bare_forge(73_003);
    let (item, _) = frame_bill(Frame::Edge, Version::V1);
    assert_eq!(
        item,
        fragment(),
        "premise: frame and parts share a material"
    );
    give(&mut game, &fragment(), full_bill() - 1);
    // Each line alone is affordable; only the sum is short.
    let refusal = compile_id(&mut game, id).unwrap_err();
    assert_eq!(
        refusal,
        GearCraftRefusal::ShortOf {
            item: fragment(),
            need: full_bill(),
            held: full_bill() - 1
        }
    );
    assert_eq!(game.held_in_reach(&fragment()), full_bill() - 1);
    assert_eq!(
        game.count_copies(&GearCopy::plain(ItemId::from(format!("crafted_gear/{id}")))),
        0
    );
}

#[test]
fn a_held_unresearched_part_is_used_once_but_not_crafted() {
    let mut game = bench_game(73_004);
    let launcher = WeaponPart::Frame(Frame::Edge, Version::V1);
    // Edge researched for the frame; Extend is held but never researched.
    know(&mut game, launcher);
    know(&mut game, WeaponPart::Extend);
    let id = game
        .create_design("Reacher", parts(&[launcher, WeaponPart::Extend]))
        .unwrap();
    game.world
        .resource_mut::<Research>()
        .0
        .remove(&WeaponPart::Extend.node_id());
    give(&mut game, &fragment(), 100);
    give(&mut game, &ItemId::weapon_part(WeaponPart::Extend), 1);
    compile_id(&mut game, id).unwrap();
    assert_eq!(game.weapon_parts_held(WeaponPart::Extend), 0);
    // Needed twice is impossible (a modifier category holds one), so the
    // second build is short of the one it cannot craft.
    assert_eq!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::PartNotResearched(WeaponPart::Extend))
    );
    assert!(
        game.held_in_reach(&fragment()) > 0,
        "a refusal spends nothing"
    );
}

#[test]
fn the_refusal_order_is_frame_then_unresearched_then_proc_then_short() {
    let mut game = bench_game(73_005);
    let proc = WeaponPart::Proc(crate::gear_craft::weapon::Proc::CoreDump);
    know(&mut game, edge());
    know(&mut game, proc);
    know(&mut game, bleed());
    let id = game
        .create_design("Dumper", parts(&[edge(), bleed(), proc]))
        .unwrap();
    empty_of(&mut game, &fragment());
    game.world
        .resource_mut::<Research>()
        .0
        .remove(&bleed().node_id());
    // The design check comes first: an unresearched, unheld Bleed beats a
    // missing Proc, which beats a short bill.
    assert_eq!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::PartNotResearched(bleed()))
    );
    know(&mut game, bleed());
    assert_eq!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::SalvageOnly(proc))
    );
    give(&mut game, &ItemId::weapon_part(proc), 1);
    assert!(matches!(
        compile_id(&mut game, id),
        Err(GearCraftRefusal::ShortOf { .. })
    ));
    assert_eq!(game.weapon_parts_held(proc), 1, "nothing was spent");
    // No Frame beats the rest, checked on the pure step.
    let frameless = crate::gear_craft::GearDesign {
        id: 0,
        name: "x".into(),
        slot: EquipmentSlot::Weapon,
        parts: parts(&[proc]),
    };
    assert_eq!(
        crate::gear_craft::plan_build(&crate::gear_craft::BuildInput {
            design: &frameless,
            held: &|_| 0,
            known: &|_| false,
            in_reach: &|_| 0,
        }),
        Err(GearCraftRefusal::Fit(FitError::NoFrame))
    );
}

#[test]
fn the_design_bill_and_preview_equal_the_inventory_delta_of_a_forge() {
    let (mut game, id) = bare_forge(73_006);
    give(&mut game, &ItemId::weapon_part(bleed()), 1);
    give(&mut game, &fragment(), 100);
    let bill = game.design_preview("Leaker", &parts(&[edge(), bleed()]));
    assert!(bill.uncraftable.is_empty());
    let before = game.held_in_reach(&fragment());
    compile_id(&mut game, id).unwrap();
    let spent = before - game.held_in_reach(&fragment());
    let line = &bill.materials[0];
    assert_eq!(line.label, game.item_name(&fragment()).to_string());
    assert_eq!((line.have, line.need), (before, spent));
    assert_eq!(bill.materials.len(), 1);
}

#[test]
fn a_design_with_every_part_held_bills_only_the_frame() {
    let (mut game, _) = bare_forge(73_007);
    for part in [edge(), bleed()] {
        give(&mut game, &ItemId::weapon_part(part), 1);
    }
    let bill = game.design_preview("Leaker", &parts(&[edge(), bleed()]));
    let (_, frame) = frame_bill(Frame::Edge, Version::V1);
    assert_eq!(bill.materials.len(), 1);
    assert_eq!(bill.materials[0].need, frame);
}

#[test]
fn a_bill_lists_what_can_never_be_crafted() {
    let mut game = bench_game(73_008);
    let proc = WeaponPart::Proc(crate::gear_craft::weapon::Proc::CoreDump);
    know(&mut game, edge());
    know(&mut game, proc);
    let dumper = parts(&[edge(), proc]);
    let bill = game.design_preview("Dumper", &dumper);
    assert_eq!(
        bill.uncraftable,
        vec![format!("{} — salvage a weapon first", proc.label())]
    );
    game.world
        .resource_mut::<Research>()
        .0
        .remove(&edge().node_id());
    give(&mut game, &ItemId::weapon_part(proc), 1);
    assert_eq!(
        game.design_preview("Dumper", &dumper).uncraftable,
        vec![format!("{} — research first", edge().label())]
    );
}

#[test]
fn craft_cost_is_offered_for_researched_non_proc_parts_only() {
    let mut game = bench_game(73_009);
    let proc = WeaponPart::Proc(crate::gear_craft::weapon::Proc::CoreDump);
    know(&mut game, edge());
    know(&mut game, WeaponPart::Extend);
    give(&mut game, &ItemId::weapon_part(proc), 1);
    give(&mut game, &ItemId::weapon_part(bleed()), 1);
    let cost = |game: &Game, category, part: WeaponPart| {
        game.design_part_options(category, &[edge()], 0)
            .into_iter()
            .find(|o| o.part == part)
            .unwrap()
            .craft_cost
    };
    let name = game.item_name(&fragment()).to_string();
    assert_eq!(
        cost(&game, Category::Range, WeaponPart::Extend),
        Some((name.clone(), crate::tuning::GEAR_MODIFIER_PART_COST))
    );
    know(&mut game, bleed());
    give(&mut game, &ItemId::weapon_part(WeaponPart::Extend), 1);
    assert!(
        cost(&game, Category::Range, WeaponPart::Extend).is_some(),
        "held or not"
    );
    assert!(cost(&game, Category::Rider, bleed()).is_some());
    assert_eq!(
        cost(&game, Category::Proc, proc),
        None,
        "a Proc is salvage only"
    );
    game.world
        .resource_mut::<Research>()
        .0
        .remove(&bleed().node_id());
    assert_eq!(
        cost(&game, Category::Rider, bleed()),
        None,
        "held but unresearched"
    );
}

#[test]
fn each_crafted_part_is_receipted_as_a_craft() {
    let (mut game, id) = bare_forge(73_010);
    give(&mut game, &fragment(), full_bill());
    game.world
        .resource_mut::<crate::resources::BattleTelemetry>()
        .on = true;
    compile_id(&mut game, id).unwrap();
    let crafted = |part: WeaponPart| -> u32 {
        game.world
            .resource::<crate::resources::BattleTelemetry>()
            .records
            .iter()
            .filter_map(|r| match r {
                crate::telemetry::Record::Acquire {
                    item, qty, source, ..
                } if *item == ItemId::weapon_part(part).0 && source == "craft" => Some(*qty),
                _ => None,
            })
            .sum()
    };
    assert_eq!(crafted(edge()), 1);
    assert_eq!(crafted(bleed()), 1);
}

#[test]
fn a_forge_pays_from_a_depot_before_the_pack_and_not_outside_base_space() {
    let (mut game, id) = bare_forge(73_011);
    touch_a_workbench(&mut game);
    give(&mut game, &fragment(), full_bill());
    let mut stock = Stock::new(500);
    stock.output.insert(fragment(), 3);
    let depot = game
        .world
        .spawn((
            Structure {
                kind: "depot".to_string(),
            },
            Position { x: 9, y: 9 },
            stock,
        ))
        .id();
    assert_eq!(
        game.design_preview("Leaker", &parts(&[edge(), bleed()]))
            .materials[0]
            .have,
        full_bill() + 3
    );
    compile_id(&mut game, id).unwrap();
    let shelf = game
        .world
        .get::<Stock>(depot)
        .unwrap()
        .output
        .get(&fragment())
        .copied()
        .unwrap_or(0);
    assert_eq!(shelf, 0, "the Depot's stock goes first");
    let carried = game
        .world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&fragment());
    assert_eq!(carried, 3, "the pack pays only the rest");

    game.world.insert_resource(Locale::Surface);
    assert_eq!(
        game.design_preview("Leaker", &parts(&[edge(), bleed()]))
            .materials[0]
            .have,
        3
    );
}

// ---------------------------------------------------------------------------
// compile_weapon: the one verb the Build weapon page calls
// ---------------------------------------------------------------------------

fn leaker_parts() -> Vec<GearPart> {
    parts(&[edge(), bleed()])
}

/// A bench with both parts researched and the pack holding exactly the
/// preview's material bill.
fn funded_for_leaker(seed: u32) -> Game {
    let mut game = bench_game(seed);
    for part in [edge(), bleed()] {
        know(&mut game, part);
    }
    empty_of(&mut game, &fragment());
    let preview = game.design_preview("Leaker", &leaker_parts());
    for line in &preview.materials {
        assert_eq!(line.need, full_bill(), "premise: one material pays all");
    }
    give(&mut game, &fragment(), full_bill());
    game
}

#[test]
fn a_compile_mints_one_copy_and_spends_exactly_the_previews_bill() {
    let mut game = funded_for_leaker(74_001);
    let need = game.design_preview("Leaker", &leaker_parts()).materials[0].need;
    let copy = game.compile_weapon("Leaker", leaker_parts()).unwrap();
    assert_eq!(game.count_copies(&copy), 1);
    assert_eq!(game.held_in_reach(&fragment()), full_bill() - need);
    assert_eq!(game.gear_designs().len(), 1);
}

#[test]
fn a_refused_compile_stores_no_design_and_spends_nothing() {
    let mut game = funded_for_leaker(74_002);
    empty_of(&mut game, &fragment());
    give(&mut game, &fragment(), full_bill() - 1);
    let refusal = game.compile_weapon("Leaker", leaker_parts()).unwrap_err();
    assert!(matches!(refusal, GearCraftRefusal::ShortOf { .. }));
    assert!(game.gear_designs().is_empty());
    assert_eq!(game.world.resource::<GearDesigns>().next_id, 0);
    assert_eq!(game.held_in_reach(&fragment()), full_bill() - 1);

    let mut game = bench_game(74_003);
    empty_of(&mut game, &fragment());
    give(&mut game, &fragment(), full_bill() * 2);
    assert_eq!(
        game.compile_weapon("Leaker", leaker_parts()),
        Err(GearCraftRefusal::PartNotResearched(edge()))
    );
    assert!(game.gear_designs().is_empty());
    assert_eq!(game.held_in_reach(&fragment()), full_bill() * 2);
}

#[test]
fn compiling_one_weapon_twice_shares_an_item_id_and_one_design() {
    let mut game = funded_for_leaker(74_004);
    give(&mut game, &fragment(), full_bill());
    let first = game.compile_weapon("Leaker", leaker_parts()).unwrap();
    let second = game.compile_weapon(" Leaker ", leaker_parts()).unwrap();
    assert_eq!(first, second);
    assert_eq!(game.count_copies(&first), 2);
    assert_eq!(game.gear_designs().len(), 1);
}

#[test]
fn the_same_name_with_different_parts_is_a_second_design() {
    let mut game = funded_for_leaker(74_005);
    give(&mut game, &fragment(), full_bill());
    let a = game.compile_weapon("Leaker", leaker_parts()).unwrap();
    let b = game.compile_weapon("Leaker", parts(&[edge()])).unwrap();
    assert_ne!(a.item, b.item);
    assert_eq!(game.gear_designs().len(), 2);
}

#[test]
fn an_unresearched_part_is_hidden_unless_held_and_a_misfit_is_greyed() {
    let mut game = bench_game(74_006);
    know(&mut game, edge());
    let listed = |game: &Game, category, part: WeaponPart| {
        game.design_part_options(category, &[edge()], 0)
            .into_iter()
            .find(|o| o.part == part)
    };
    assert!(listed(&game, Category::Rider, bleed()).is_none());
    give(&mut game, &ItemId::weapon_part(bleed()), 1);
    assert!(listed(&game, Category::Rider, bleed()).is_some());
    // A researched part that cannot fit stays, with its reason.
    let frames = game.design_part_options(Category::Frame, &[edge()], 0);
    assert!(frames.iter().all(|o| o.part == edge()
        || o.unavailable.is_none()
        || matches!(o.unavailable, Some(GearCraftRefusal::Fit(_)))));
    assert!(
        frames
            .iter()
            .all(|o| !matches!(o.unavailable, Some(GearCraftRefusal::PartNotResearched(_))))
    );
}

#[test]
fn the_previews_blocked_is_what_compile_would_refuse() {
    let proc = WeaponPart::Proc(crate::gear_craft::weapon::Proc::CoreDump);
    let check = |game: &mut Game, name: &str, parts: Vec<GearPart>| {
        let blocked = game.design_preview(name, &parts).blocked;
        assert_eq!(blocked, game.compile_weapon(name, parts).err());
        blocked
    };
    // Short one material: the preview names it.
    let mut game = funded_for_leaker(74_010);
    empty_of(&mut game, &fragment());
    give(&mut game, &fragment(), full_bill() - 1);
    assert!(matches!(
        check(&mut game, "Leaker", leaker_parts()),
        Some(GearCraftRefusal::ShortOf { .. })
    ));
    // A Proc that is neither held nor ever salvaged.
    let mut game = funded_for_leaker(74_011);
    assert!(check(&mut game, "Dumper", parts(&[edge(), proc])).is_some());
    // A valid draft is not blocked.
    let mut game = funded_for_leaker(74_012);
    assert_eq!(check(&mut game, "Leaker", leaker_parts()), None);
    // Busy: a battle is running.
    let mut game = funded_for_leaker(74_013);
    start_battle_with_a_wild_program(&mut game);
    assert_eq!(
        check(&mut game, "Leaker", leaker_parts()),
        Some(GearCraftRefusal::Busy)
    );
}
