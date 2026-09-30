//! Fitting and stripping affixes at a Mod Bench: `Game::apply_affix`,
//! `remove_affix` and the two queries the mod screen reads. The affixes are
//! this file's own (`t_*`), so no test depends on shipped content.

use crate::components::{Equipment, Stats};
use crate::items::{GearCopy, ItemId};
use crate::resources::Research;
use crate::tests::support::{give, modded_assets_dir, spawn_structure_at};
use crate::{DifficultyMode, Game};

const ARMOR_AFFIX: &str = r#"(id: "t_plate", prefix: Some("Plated"), stats: (mitigation: 6),
    slots: Some([Armor]),
    research: Some((cost: 1, apply_cost: [("core_fragment", 2), ("logic_wafer", 1)])))"#;
const ARMOR_AFFIX_2: &str = r#"(id: "t_weave", suffix: Some("of Weave"), stats: (mitigation: 3),
    slots: Some([Armor]),
    research: Some((cost: 1, apply_cost: [("core_fragment", 1)])))"#;
const WEAPON_AFFIX: &str = r#"(id: "t_sight", suffix: Some("of Sight"), stats: (decompiler: 2),
    slots: Some([Weapon]),
    research: Some((cost: 1, apply_cost: [("core_fragment", 1)])))"#;
const UNRESEARCHED: &str = r#"(id: "t_late", prefix: Some("Late"), stats: (mitigation: 1),
    slots: Some([Armor]),
    research: Some((cost: 1, apply_cost: [("core_fragment", 1)])))"#;

const ANY_SLOT_AFFIX: &str = r#"(id: "t_any", prefix: Some("Anyslot"), stats: (evasion: 1),
    research: Some((cost: 1, apply_cost: [("core_fragment", 1)])))"#;

fn plating() -> ItemId {
    ItemId::from("firewall_plating")
}

fn tiered(tier: u32, affixes: &[&str]) -> GearCopy {
    GearCopy::with_affixes(
        plating(),
        crate::components::Rarity::Ordinary,
        tier,
        affixes.iter().map(|a| (*a).into()).collect(),
        crate::tuning::QUALITY_DEFAULT,
    )
}

/// A game with a Mod Bench standing, the three affixes researched and a
/// fourth not, and a pack of materials.
fn bench_game(tag: &str, seed: u32) -> Game {
    let dir = modded_assets_dir(tag, &[], &[], &[], &[], &[]);
    for (name, body) in [
        ("t_plate.ron", ARMOR_AFFIX),
        ("t_weave.ron", ARMOR_AFFIX_2),
        ("t_sight.ron", WEAPON_AFFIX),
        ("t_late.ron", UNRESEARCHED),
        ("t_any.ron", ANY_SLOT_AFFIX),
    ] {
        std::fs::write(dir.join("affixes").join(name), body).unwrap();
    }
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &dir).unwrap();
    spawn_structure_at(&mut game, "mod_bench", 3, 3);
    for id in ["t_plate", "t_weave", "t_sight", "t_any"] {
        game.world
            .resource_mut::<Research>()
            .0
            .insert(format!("affix:{id}"));
    }
    give(&mut game, &ItemId::from("core_fragment"), 20);
    give(&mut game, &ItemId::from("logic_wafer"), 20);
    game
}

fn pack(game: &Game, item: &str) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<crate::components::Inventory>(player)
        .unwrap()
        .count(&ItemId::from(item))
}

fn carry(game: &mut Game, copy: &GearCopy) {
    game.add_copies(copy, 1);
}

#[test]
fn slots_are_one_plus_the_fusion_tier() {
    let game = bench_game("mods_slots", 9401);
    assert_eq!(game.affix_slots(&tiered(0, &[])), 1);
    assert_eq!(game.affix_slots(&tiered(3, &[])), 4);
}

#[test]
fn appliable_lists_researched_affixes_that_fit_the_copys_slot() {
    let game = bench_game("mods_appliable", 9402);
    let armor: Vec<String> = game
        .appliable_affixes(&tiered(0, &[]))
        .iter()
        .map(|a| a.as_str().to_string())
        .collect();
    assert_eq!(
        armor,
        vec!["t_any", "t_plate", "t_weave"],
        "t_late is unresearched"
    );
    let weapon = GearCopy::plain(ItemId::from("arc_lance"));
    let ids: Vec<String> = game
        .appliable_affixes(&weapon)
        .iter()
        .map(|a| a.as_str().to_string())
        .collect();
    assert_eq!(ids, vec!["t_any", "t_sight"]);
}

#[test]
fn apply_pays_exactly_the_apply_cost_and_rekeys_the_copy() {
    let mut game = bench_game("mods_apply", 9403);
    let copy = tiered(1, &[]);
    carry(&mut game, &copy);
    let (core, wafer) = (pack(&game, "core_fragment"), pack(&game, "logic_wafer"));
    let (modded, msg) = game.apply_affix(&copy, &"t_plate".into()).unwrap();
    assert_eq!(modded.affixes, vec!["t_plate".into()]);
    assert_eq!((modded.tier, modded.item.clone()), (1, plating()));
    assert_eq!(pack(&game, "core_fragment"), core - 2);
    assert_eq!(pack(&game, "logic_wafer"), wafer - 1);
    assert_eq!(game.count_copies(&copy), 0, "the old key is gone");
    assert_eq!(game.count_copies(&modded), 1, "the new key is findable");
    assert!(msg.contains("Plated"), "{msg}");
}

fn refused(game: &mut Game, copy: &GearCopy, affix: &str) -> String {
    let before = (pack(game, "core_fragment"), pack(game, "logic_wafer"));
    let err = game.apply_affix(copy, &affix.into()).unwrap_err();
    assert_eq!(
        (pack(game, "core_fragment"), pack(game, "logic_wafer")),
        before,
        "a refusal spends nothing"
    );
    err
}

#[test]
fn apply_refuses_without_a_bench() {
    let mut game = bench_game("mods_nobench", 9404);
    let bench = crate::tests::support::first_structure(&mut game, "mod_bench");
    game.world.despawn(bench);
    let copy = tiered(0, &[]);
    carry(&mut game, &copy);
    assert_eq!(
        refused(&mut game, &copy, "t_plate"),
        "Build a Mod Bench first."
    );
    assert_eq!(
        game.remove_affix(&copy, &"t_plate".into()).unwrap_err(),
        "Build a Mod Bench first."
    );
}

#[test]
fn apply_refuses_an_unresearched_affix() {
    let mut game = bench_game("mods_unresearched", 9405);
    let copy = tiered(0, &[]);
    carry(&mut game, &copy);
    assert_eq!(
        refused(&mut game, &copy, "t_late"),
        "Late hasn't been researched."
    );
}

#[test]
fn apply_refuses_an_affix_that_does_not_fit_the_slot() {
    let mut game = bench_game("mods_wrong_slot", 9406);
    let copy = tiered(0, &[]);
    carry(&mut game, &copy);
    assert_eq!(
        refused(&mut game, &copy, "t_sight"),
        "of Sight can't be fitted to Armor."
    );
}

#[test]
fn apply_refuses_a_full_copy_and_a_tier_buys_a_second_slot() {
    let mut game = bench_game("mods_cap", 9407);
    let one = tiered(0, &["t_weave"]);
    carry(&mut game, &one);
    let err = refused(&mut game, &one, "t_plate");
    assert!(err.contains("no free affix slot (1 of 1 used)"), "{err}");

    let two = tiered(1, &["t_weave"]);
    carry(&mut game, &two);
    let (filled, _) = game.apply_affix(&two, &"t_plate".into()).unwrap();
    assert_eq!(filled.affixes.len(), 2);
    let err = refused(&mut game, &filled, "t_plate");
    assert!(err.contains("(2 of 2 used)"), "{err}");
}

#[test]
fn an_over_cap_copy_keeps_everything_and_takes_nothing_new() {
    let mut game = bench_game("mods_overcap", 9408);
    let over = tiered(0, &["t_plate", "t_weave", "t_weave"]);
    carry(&mut game, &over);
    assert!(refused(&mut game, &over, "t_plate").contains("no free affix slot"));
    // Stripping one still leaves it over the cap, so it still refuses...
    let (less, _) = game.remove_affix(&over, &"t_weave".into()).unwrap();
    assert_eq!(less.affixes.len(), 2);
    assert!(refused(&mut game, &less, "t_plate").contains("no free affix slot"));
    // ...until it is back under.
    let (one, _) = game.remove_affix(&less, &"t_weave".into()).unwrap();
    assert_eq!(one.affixes.len(), 1);
    assert!(refused(&mut game, &one, "t_plate").contains("no free affix slot"));
    let (none, _) = game.remove_affix(&one, &"t_plate".into()).unwrap();
    assert!(none.affixes.is_empty());
}

#[test]
fn apply_refuses_what_the_pack_cannot_pay_and_names_the_shortfall() {
    let mut game = bench_game("mods_short", 9409);
    let copy = tiered(0, &[]);
    carry(&mut game, &copy);
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::Inventory>(player)
        .unwrap()
        .take(ItemId::from("logic_wafer"), 20);
    let err = refused(&mut game, &copy, "t_plate");
    assert_eq!(err, "Need 1 Logic Wafer (have 0).");
    assert_eq!(game.count_copies(&copy), 1, "the copy is untouched");
}

#[test]
fn apply_refuses_a_copy_the_player_does_not_hold() {
    let mut game = bench_game("mods_absent", 9410);
    let err = refused(&mut game, &tiered(0, &[]), "t_plate");
    assert!(err.starts_with("You don't have"), "{err}");
}

#[test]
fn removing_one_of_two_duplicates_removes_one() {
    let mut game = bench_game("mods_dupe", 9411);
    let copy = tiered(1, &["t_weave", "t_weave"]);
    carry(&mut game, &copy);
    let (left, msg) = game.remove_affix(&copy, &"t_weave".into()).unwrap();
    assert_eq!(left.affixes, vec!["t_weave".into()]);
    assert!(msg.contains("of Weave"), "{msg}");
    assert_eq!(game.count_copies(&left), 1);
}

#[test]
fn remove_refuses_an_affix_the_copy_lacks() {
    let mut game = bench_game("mods_lacks", 9412);
    let copy = tiered(0, &["t_weave"]);
    carry(&mut game, &copy);
    let err = game.remove_affix(&copy, &"t_plate".into()).unwrap_err();
    assert!(err.contains("doesn't carry that affix"), "{err}");
    assert_eq!(game.count_copies(&copy), 1);
}

#[test]
fn removing_the_last_affix_makes_the_copy_plain_and_findable() {
    let mut game = bench_game("mods_plain", 9413);
    let copy = tiered(0, &["t_weave"]);
    carry(&mut game, &copy);
    let (plain, _) = game.remove_affix(&copy, &"t_weave".into()).unwrap();
    assert!(plain.is_plain());
    assert_eq!(game.count_copies(&plain), 1, "it moved to the plain store");
    assert_eq!(game.count_copies(&copy), 0);
}

#[test]
fn a_worn_copy_keeps_its_stats_right_across_apply_and_remove() {
    let mut game = bench_game("mods_worn", 9414);
    let player = game.player_entity();
    let copy = tiered(1, &[]);
    carry(&mut game, &copy);
    game.equip(player, &copy).unwrap();
    let base = game.world.get::<Stats>(player).unwrap().mitigation;
    let level = game
        .world
        .get::<Equipment>(player)
        .unwrap()
        .get(crate::items::EquipmentSlot::Armor)
        .unwrap()
        .level;

    let (modded, _) = game.apply_affix(&copy, &"t_plate".into()).unwrap();
    let gain = game.copy_bonus(&modded, level).unwrap().mitigation
        - game.copy_bonus(&copy, level).unwrap().mitigation;
    assert!(gain > 0, "the fixture affix must add mitigation");
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().mitigation,
        base + gain
    );
    let worn = game
        .world
        .get::<Equipment>(player)
        .unwrap()
        .get(crate::items::EquipmentSlot::Armor)
        .unwrap();
    assert_eq!(worn.copy, modded, "the slot follows the re-keyed copy");
    assert_eq!(worn.level, level);
    assert_eq!(game.count_copies(&modded), 0, "worn, not also carried");

    let (back, _) = game.remove_affix(&modded, &"t_plate".into()).unwrap();
    assert_eq!(back, copy);
    assert_eq!(game.world.get::<Stats>(player).unwrap().mitigation, base);
}

#[test]
fn removing_names_the_affix_stripped_not_the_last_one() {
    let mut game = bench_game("mods_named", 9415);
    let copy = tiered(1, &["t_plate", "t_weave"]);
    carry(&mut game, &copy);
    let (left, _) = game.remove_affix(&copy, &"t_plate".into()).unwrap();
    assert_eq!(left.affixes, vec!["t_weave".into()]);
}

fn saved_and_loaded(game: &mut Game, tag: &str) -> Game {
    let path =
        std::env::temp_dir().join(format!("feral_processes_{tag}_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}

/// A RON round trip cannot catch a skipped field, so this goes through the
/// real `Game::save`/`Game::load`: a modded copy worn and a modded copy
/// carried both come back carrying what they carried.
#[test]
fn a_modded_copy_survives_save_and_load_worn_and_carried() {
    let assets = crate::tests::support::test_assets_dir();
    let mut game = Game::new(9420, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let carried = tiered(1, &["deflecting"]);
    let worn = GearCopy::with_affixes(
        ItemId::from("arc_lance"),
        crate::components::Rarity::Ordinary,
        0,
        vec!["of_introspection".into()],
        crate::tuning::QUALITY_DEFAULT,
    );
    game.add_copies(&carried, 1);
    game.add_copies(&worn, 1);
    game.equip(player, &worn).unwrap();

    let loaded = saved_and_loaded(&mut game, "modded_copy");
    assert_eq!(loaded.count_copies(&carried), 1);
    let back = loaded
        .world
        .get::<Equipment>(loaded.player_entity())
        .and_then(|e| e.weapon.clone())
        .expect("the weapon is still worn");
    assert_eq!(back.copy, worn);
}

#[test]
fn a_researched_and_discovered_affix_node_survives_save_and_load() {
    let assets = crate::tests::support::test_assets_dir();
    let mut game = Game::new(9421, DifficultyMode::Forgiving, &assets).unwrap();
    assert!(!game.affix_tree_open());
    for id in ["mod_bench", "affix:deflecting"] {
        game.world
            .resource_mut::<Research>()
            .0
            .insert(id.to_string());
    }
    game.world
        .resource_mut::<crate::resources::DiscoveredResearch>()
        .0
        .insert("affix:of_introspection".to_string());

    let loaded = saved_and_loaded(&mut game, "affix_research");
    assert!(loaded.affix_tree_open());
    assert!(loaded.affix_researched(&"deflecting".into()));
    assert!(!loaded.affix_researched(&"of_introspection".into()));
    let listed: Vec<String> = loaded
        .research_nodes(crate::research::ResearchTree::Affixes)
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert!(
        listed.contains(&"affix:deflecting".to_string()),
        "{listed:?}"
    );
    assert!(
        listed.contains(&"affix:of_introspection".to_string()),
        "a discovery is permanent: {listed:?}"
    );
    assert!(
        !listed.contains(&"affix:of_deflection".to_string()),
        "an undiscovered node stays hidden"
    );
}

/// The refusal that has to sit above every other: a mid-battle bench is
/// not a bench, and a dead run has nothing to mod.
#[test]
fn apply_and_remove_refuse_during_a_battle_and_after_game_over() {
    let mut game = bench_game("mods_guards", 9420);
    let copy = tiered(0, &["t_weave"]);
    carry(&mut game, &copy);

    game.world
        .resource_mut::<crate::resources::GameOver>()
        .reason = Some("done".into());
    assert_eq!(
        refused(&mut game, &tiered(0, &[]), "t_plate"),
        "Can't do that right now."
    );
    assert_eq!(
        game.remove_affix(&copy, &"t_weave".into()).unwrap_err(),
        "Can't do that right now."
    );
    game.world
        .resource_mut::<crate::resources::GameOver>()
        .reason = None;

    crate::tests::support::start_battle_with_a_wild_program(&mut game);
    assert!(game.has_active_battle());
    assert_eq!(
        refused(&mut game, &tiered(0, &[]), "t_plate"),
        "Can't do that right now."
    );
    assert_eq!(
        game.remove_affix(&copy, &"t_weave".into()).unwrap_err(),
        "Can't do that right now."
    );
}

/// The spec fits affixes to Weapon and Armor only; a Module copy is
/// refused even by an affix that names no slot restriction.
#[test]
fn a_module_copy_cannot_be_modded() {
    let mut game = bench_game("mods_module", 9421);
    let module = GearCopy::plain(ItemId::from("adversarial_patch"));
    carry(&mut game, &module);
    let err = refused(&mut game, &module, "t_any");
    assert_eq!(err, "Adversarial Patch can't be modified.");
    assert_eq!(game.affix_slots(&module), 0);
    assert!(game.appliable_affixes(&module).is_empty());
}
