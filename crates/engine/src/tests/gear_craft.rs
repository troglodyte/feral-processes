//! Weapon crafting's wiring into `Game` (ADR 0031, phase 2): the crafted
//! tier, part items, the craft / build / delete verbs, and the save.

use super::support::*;
use crate::affixes::AffixId;
use crate::gear_craft::weapon::{Category, Frame, WeaponPart, frame_bill};
use crate::gear_craft::{DesignHolder, FitError, GearCraftRefusal, GearPart};
use crate::resources::{BuybackLedger, GearDesigns, KnownAffixes, Research};
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
    game.build_design(id).unwrap()
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
fn crafting_a_part_needs_its_research_and_spends_its_bill() {
    let mut game = bench_game(71_006);
    let part = WeaponPart::Extend;
    assert_eq!(
        game.craft_weapon_part(part),
        Err(GearCraftRefusal::PartNotResearched(part))
    );
    know(&mut game, part);
    let (material, count) = crate::gear_craft::weapon::part_bill(part);
    empty_of(&mut game, &material);
    let held = game.held_in_reach(&material);
    assert!(held < count, "premise: the starter pack cannot pay");
    assert!(matches!(
        game.craft_weapon_part(part),
        Err(GearCraftRefusal::ShortOf { .. })
    ));
    assert_eq!(game.weapon_parts_held(part), 0, "a refusal spends nothing");
    give(&mut game, &material, count);
    game.craft_weapon_part(part).unwrap();
    assert_eq!(game.weapon_parts_held(part), 1);
    assert_eq!(game.held_in_reach(&material), held, "the bill was spent");
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
    assert!(matches!(
        game.build_design(id),
        Err(GearCraftRefusal::MissingParts(_))
    ));
    for part in [edge(), bleed()] {
        give(&mut game, &ItemId::weapon_part(part), 1);
    }
    let (material, count) = frame_bill(Frame::Edge, Version::V1);
    empty_of(&mut game, &material);
    let have = game.held_in_reach(&material);
    assert!(have < count, "premise: the bill is not already in the pack");
    let refusal = game.build_design(id).unwrap_err();
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
    let copy = game.build_design(id).unwrap();
    assert_eq!(
        copy,
        GearCopy::plain(ItemId::from(format!("crafted_gear/{id}")))
    );
    assert_eq!(game.count_copies(&copy), 1);
    assert_eq!(game.weapon_parts_held(edge()), 0);
    assert_eq!(game.weapon_parts_held(bleed()), 0);
    assert_eq!(game.held_in_reach(&material), have);
    assert_eq!(game.gear_designs().len(), 1, "the design stays");
    assert_eq!(
        game.build_design(id),
        Err(GearCraftRefusal::MissingParts(vec![
            (edge(), 1),
            (bleed(), 1)
        ])),
        "building again needs a fresh set"
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
// Task 4: delete
// ---------------------------------------------------------------------------

#[test]
fn a_design_with_no_copy_can_be_deleted_and_leaves_the_tier() {
    let mut game = bench_game(71_020);
    let id = designed(&mut game);
    game.delete_design(id).unwrap();
    assert!(game.gear_designs().is_empty());
    assert!(def(&game, &format!("crafted_gear/{id}")).is_none());
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::NoSuchDesign(id))
    );
    let next = designed(&mut game);
    assert_ne!(next, id, "an id is never reused");
}

#[test]
fn delete_is_refused_while_a_plain_copy_is_in_the_pack() {
    let mut game = bench_game(71_021);
    let id = designed(&mut game);
    built(&mut game, id);
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Pack))
    );
}

#[test]
fn delete_is_refused_while_a_non_plain_copy_is_in_the_gear_ledger() {
    let mut game = bench_game(71_022);
    let id = designed(&mut game);
    let rare = GearCopy {
        rarity: Rarity::Gold,
        ..GearCopy::plain(ItemId::from(format!("crafted_gear/{id}")))
    };
    game.add_copies(&rare, 1);
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Pack))
    );
}

#[test]
fn delete_is_refused_while_the_player_wears_it() {
    let mut game = bench_game(71_023);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    let player = game.player_entity();
    game.equip(player, &copy).unwrap();
    assert_eq!(game.count_copies(&copy), 0, "premise: only worn now");
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Worn))
    );
}

#[test]
fn delete_is_refused_while_a_companion_wears_it() {
    let mut game = bench_game(71_024);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    let pet = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("a shipped species");
    game.equip(pet, &copy).unwrap();
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Companion))
    );
}

#[test]
fn delete_is_refused_while_a_trader_holds_it_for_buyback() {
    let mut game = bench_game(71_025);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    let market = spawn_market_at(&mut game, 5, 5);
    from_inside_the_base(&mut game, |g| g.sell_item(market, copy.clone(), 1)).unwrap();
    assert_eq!(game.count_copies(&copy), 0, "premise: sold");
    assert!(
        game.world
            .resource::<BuybackLedger>()
            .0
            .values()
            .flatten()
            .any(|(c, _)| *c == copy)
    );
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Buyback))
    );
}

#[test]
fn delete_is_refused_while_a_copy_lies_on_a_floor_tile() {
    let mut game = bench_game(71_026);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    game.take_copies(&copy, 1);
    game.world
        .spawn(components::FloorItem(items::FloorThing::Item(
            copy.item.clone(),
        )));
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Floor))
    );
}

#[test]
fn delete_is_refused_while_a_copy_sits_in_a_depot() {
    let mut game = bench_game(71_027);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    game.take_copies(&copy, 1);
    let depot = spawn_machine_at(&mut game, "depot", 4, 0);
    let mut stock = game.world.get_mut::<components::Stock>(depot).unwrap();
    *stock.output.entry(copy.item.clone()).or_default() += 1;
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Stored))
    );
}

#[test]
fn delete_is_refused_while_a_body_carries_a_copy() {
    let mut game = bench_game(71_028);
    let id = designed(&mut game);
    let copy = built(&mut game, id);
    game.take_copies(&copy, 1);
    let hauler = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("a shipped species");
    game.world.entity_mut(hauler).insert(components::Carrying {
        item: copy.item.clone(),
        qty: 1,
    });
    assert_eq!(
        game.delete_design(id),
        Err(GearCraftRefusal::InUse(DesignHolder::Carried))
    );
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
    assert_eq!(
        loaded.design_holder(id),
        Some(DesignHolder::Worn),
        "the worn copy still blocks a delete"
    );
}

#[test]
fn a_deleted_design_is_gone_from_the_save_and_the_tier() {
    let mut game = bench_game(71_031);
    let id = designed(&mut game);
    let keep = designed(&mut game);
    game.delete_design(id).unwrap();
    let loaded = round_trip(&mut game, "gear_craft_delete");
    assert!(def(&loaded, &format!("crafted_gear/{id}")).is_none());
    assert!(def(&loaded, &format!("crafted_gear/{keep}")).is_some());
    assert_eq!(loaded.world.resource::<GearDesigns>().designs.len(), 1);
    assert_eq!(
        loaded.world.resource::<GearDesigns>().next_id,
        keep,
        "the counter survives, so a deleted id is not reused"
    );
}

// ---------------------------------------------------------------------------
// Task 6: the views
// ---------------------------------------------------------------------------

#[test]
fn the_designs_list_and_the_preview_quote_the_same_numbers() {
    let mut game = bench_game(71_040);
    let id = designed(&mut game);
    let row = game.design_rows().remove(0);
    assert_eq!(row.id, id);
    assert_eq!(
        row.parts,
        vec!["Edge v1".to_string(), "Rider: Bleed".into()]
    );
    assert!(row.power.is_some());
    assert!(matches!(
        row.blocked,
        Some(GearCraftRefusal::MissingParts(_))
    ));

    let preview = game.design_preview("Leaker", &parts(&[edge(), bleed()]));
    assert_eq!(preview.power, row.power, "one rating, two pages");
    assert_eq!(preview.description, row.description);
    assert_eq!(preview.range, 1);
    assert_eq!((preview.part_count, preview.part_cap), (1, 2));
    assert_eq!(preview.blocked, None);
    assert_eq!(
        preview.parts_missing,
        vec![("Edge v1".to_string(), 1), ("Rider: Bleed".to_string(), 1)]
    );

    let bad = game.design_preview("", &parts(&[edge()]));
    assert_eq!(bad.blocked, Some(GearCraftRefusal::NameEmpty));
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
    let locked = options
        .iter()
        .find(|o| o.part == WeaponPart::Frame(Frame::Lance, Version::V1))
        .unwrap();
    assert_eq!(
        locked.unavailable,
        Some(GearCraftRefusal::PartNotResearched(locked.part))
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

#[test]
fn the_part_rows_list_every_part_locked_until_researched() {
    let mut game = bench_game(71_043);
    let rows = game.weapon_part_rows();
    assert_eq!(rows.len(), WeaponPart::all().len());
    assert!(rows.iter().all(|r| !r.known));
    know(&mut game, WeaponPart::Extend);
    give(&mut game, &ItemId::weapon_part(WeaponPart::Extend), 2);
    let row = game
        .weapon_part_rows()
        .into_iter()
        .find(|r| r.part == WeaponPart::Extend)
        .unwrap();
    assert!(row.known);
    assert_eq!(row.held, 2);
    assert_eq!(row.material_count, crate::tuning::GEAR_MODIFIER_PART_COST);
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
    know(&mut game, proc);
    give(&mut game, &ItemId::from("core_fragment"), 50);
    assert_eq!(
        game.craft_weapon_part(proc),
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
