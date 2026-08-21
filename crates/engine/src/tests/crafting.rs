//! Recipes, crafting costs, and the blurbs describing what an item does.

use super::support::*;
use crate::*;

#[test]
fn craft_consumes_cost_and_grants_the_result() {
    let mut game = Game::new(20, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST);
    }

    game.craft(&ItemId::from(ids::ICE_BREAKER), 1, false)
        .unwrap();

    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
        0,
        "cost should be fully consumed"
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::ICE_BREAKER)),
        1,
        "the recipe's result should be granted"
    );
}

#[test]
fn craft_multiple_scales_cost_and_result() {
    let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST * 3);
    }

    game.craft(&ItemId::from(ids::ICE_BREAKER), 3, false)
        .unwrap();

    let inv = game.world.get::<Inventory>(player).unwrap();
    assert_eq!(
        inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
        0,
        "cost should scale with quantity"
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::ICE_BREAKER)),
        3,
        "quantity units should be granted"
    );
}

#[test]
fn max_craftable_floors_to_the_cheapest_affordable_whole_unit() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        // ICE_BREAKER_CORE_COST per unit; 7 fragments afford 2 whole
        // units with 1 left over, not 3.
        inv.add(
            ItemId::from(ids::CORE_FRAGMENT),
            ICE_BREAKER_CORE_COST * 2 + 1,
        );
    }

    assert_eq!(
        game.max_craftable(&ItemId::from(ids::ICE_BREAKER), false),
        2
    );
}

#[test]
fn max_craftable_is_zero_with_no_recipe_or_no_resources() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();

    assert_eq!(
        game.max_craftable(&ItemId::from(ids::ICE_BREAKER), false),
        0,
        "no resources at all"
    );
    assert_eq!(
        game.max_craftable(&ItemId::from(ids::CORE_FRAGMENT), false),
        0,
        "no recipe exists for this item"
    );
}

#[test]
fn craft_fails_without_enough_of_the_cost() {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
    }

    assert!(
        game.craft(&ItemId::from(ids::ICE_BREAKER), 1, false)
            .is_err()
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER)),
        0
    );
}

#[test]
fn craft_rejects_a_result_with_no_recipe() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.craft(&ItemId::from(ids::CORE_FRAGMENT), 1, false)
            .is_err()
    );
}

#[test]
fn item_blurbs_gloss_what_a_shipped_item_actually_does() {
    let game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.item_blurb(&ItemId::from(ids::POWER_CELL)).as_deref(),
        Some("+25 power"),
        "a consumable should quote what it restores"
    );
    assert_eq!(
        game.item_blurb(&ItemId::from("arc_lance")).as_deref(),
        Some("+3 atk"),
        "equipment should quote the stats it grants"
    );
    assert_eq!(
        game.item_blurb(&ItemId::from("black_ice_pick")).as_deref(),
        Some("+3 atk +2 decomp"),
        "an item granting several stats should list each"
    );
    assert_eq!(
        game.item_blurb(&ItemId::from(ids::CORE_FRAGMENT)),
        None,
        "a plain currency has nothing to gloss and reads fine as itself"
    );
}

/// The outlet is what `Game::rest` will spend (Task 2) — this task only
/// needs it craftable, starter (no bench), at the price the spec fixes.
/// Asserted against a fresh game, which unlocks no perks, so
/// `LeanCompiler`'s discount can't shave the 5 down and make this brittle.
#[test]
fn the_outlet_is_craftable_from_five_core_fragments_with_no_perks() {
    let game = Game::new(98, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| r.result == ItemId::from(ids::OUTLET))
        .expect("outlet.ron should declare a craftable recipe");
    assert_eq!(recipe.cost, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(
        game.craft_cost(&ItemId::from(ids::OUTLET), false),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)],
        "with no perks unlocked, craft_cost should match the raw recipe"
    );
}

#[test]
fn every_compilable_item_either_has_a_blurb_or_declares_nothing_blurb_worthy() {
    let game = Game::new(97, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for recipe in game.craft_recipes() {
        // Skip only when the def sets none of the three fields `item_blurb`
        // (`game/catalog.rs`) actually reads — read directly off the def
        // rather than through `ItemDef::category`, which checks a different
        // set (`routine`, `equipment`, `consume`, `role`) and is why this
        // guard used to silently drop the ICE Breaker: it sets only
        // `taming_potency`, which `category` never looks at, so it fell
        // through to `Material` and got skipped even though `item_blurb`
        // has "taming catalyst" to say about it. The outlet is the one
        // shipped recipe that genuinely sets none of the three and reads
        // fine as itself, the same as a bare currency would.
        let def = game
            .item_def(&recipe.result)
            .expect("a craftable recipe's result should have a definition");
        if def.equipment.is_none() && def.consume.is_none() && def.taming_potency.is_none() {
            continue;
        }
        let blurb = game.item_blurb(&recipe.result);
        assert!(
            blurb.is_some(),
            "{} is compilable but the compile menu would say nothing about it",
            recipe.result
        );
    }
}

/// The Compile screen prints `ItemCategory::short_label` as a leading
/// column, and that column only reads as a heading for the run of rows
/// beneath it if the list arrives grouped — which is the whole reason the
/// ordering is decided here rather than in the renderer. `handle_craft_key`
/// dispatches `recipes[idx]` while `draw_craft_menu` draws `recipes[i]` from
/// a separate call, so a sort applied on the draw side alone would put the
/// highlight on a different row from the one that fires.
#[test]
fn the_compile_list_comes_back_in_category_order() {
    let game = Game::new(99, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let keys: Vec<(ItemCategory, String)> = game
        .craft_recipes()
        .iter()
        .map(|r| game.category_sort_key(&r.result))
        .collect();
    assert!(keys.len() > 1, "a fresh game should offer several recipes");
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "the compile list must arrive in the same category order the \
         inventory and a trader's shelf use"
    );
}

/// The researched half of `craft_recipes` is pushed after the `ItemDb` walk,
/// so sorting before that push would leave every unlocked recipe trailing
/// the list in a block of its own — a Weapon under the Materials, with the
/// category column contradicting itself at the bottom of the screen.
#[test]
fn a_researched_recipe_sorts_into_its_category_rather_than_trailing() {
    let mut game = Game::new(100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "fabricator", 3, 0);
    unlock_research_chain(&mut game, "overclock");
    let core = ItemId::from("overclock_core");

    let recipes = game.craft_recipes();
    let at = recipes
        .iter()
        .position(|r| r.result == core)
        .expect("researching overclock with a Fabricator up should offer its recipe");
    assert_eq!(
        game.item_category(&core),
        ItemCategory::Weapon,
        "the fixture is only meaningful while the unlocked recipe is a Weapon"
    );
    assert!(
        recipes[at + 1..]
            .iter()
            .any(|r| game.item_category(&r.result) > ItemCategory::Weapon),
        "the unlocked Weapon should sit inside the Weapon run, not after \
         every category the base recipes cover"
    );
}

/// With the category column naming the kind, a blurb that named the slot as
/// well would repeat it on the one screen that reads a blurb at all. So the
/// column carries the kind and the gloss carries the magnitude, and an
/// equippable with nothing to say about magnitude says nothing.
///
/// Unreachable for the shipped roster — every shipped equippable declares a
/// non-zero stat — so it takes a modded item to walk.
#[test]
fn an_equippable_with_no_stats_leaves_its_kind_to_the_category_column() {
    let dir = modded_assets_dir(
        "statless_gear",
        &[],
        &[("blank_edge.ron", BLANK_EDGE)],
        &[],
        &[],
        &[],
    );
    let game = Game::new(23, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let blank = ItemId::from("blank_edge");

    assert_eq!(
        game.item_category(&blank),
        ItemCategory::Weapon,
        "the column still has the kind to report"
    );
    assert_eq!(
        game.item_blurb(&blank),
        None,
        "and the gloss has no magnitude to add to it"
    );
}

/// An equippable granting nothing, which no shipped item is.
const BLANK_EDGE: &str = r#"(
    id: "blank_edge",
    name: "Blank Edge",
    description: "A test weapon that grants nothing.",
    value: Some(1),
    equipment: Some((Weapon, ())),
)"#;

/// A modded recipe priced in a fusable item, so the one thing keeping
/// fused gear out of the production chain can actually be walked.
const PLATING_RECIPE: &str = r#"(
    id: "plating_probe",
    name: "Plating Probe",
    description: "A test recipe priced in armour.",
    value: Some(1),
    craftable: Some((cost: [("ablative_plating", 2)])),
)"#;

/// `Inventory` is by definition the tier-0 store, and every recipe reads it
/// — so a fused copy is not an ingredient, however many base copies went
/// into it. No shipped recipe is priced in equipment, so the path is walked
/// with a modded item rather than left unasserted; the machine half needs no
/// test of its own, since `Stock` is the only thing an assembler pulls from
/// and nothing in the game puts a player's copy into one.
#[test]
fn a_fused_copy_is_not_a_recipe_ingredient() {
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let probe = ItemId::from("plating_probe");
    let dir = modded_assets_dir(
        "fused_recipe",
        &[],
        &[("plating_probe.ron", PLATING_RECIPE)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(21, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 2);

    assert_eq!(
        game.max_craftable(&probe, false),
        1,
        "two spares buy one probe"
    );

    game.fuse_item(&gear(&armor, 0)).unwrap();

    assert_eq!(
        game.max_craftable(&probe, false),
        0,
        "the copies went into a fused one, which no recipe can reach"
    );
    assert!(game.craft(&probe, 1, false).is_err());
    assert_eq!(
        held_at(&game, &armor, 1),
        1,
        "and the refusal left the fused copy alone"
    );
}

/// The bench half of a compiled copy's quality floor: which deployed
/// structure of a kind the player gets to compile against.
///
/// Three cases in one test because the answer is a single `Option<u32>` and
/// the interesting part is how the three disagree — a bench that was never
/// built is not a bench at tier 1, and a bench that predates upgrades is.
#[test]
fn best_structure_tier_reads_the_best_deployed_one() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert_eq!(
        game.best_structure_tier("fabricator"),
        None,
        "a bench nobody built is not a bench at tier 1"
    );

    spawn_structure_at(&mut game, "fabricator", 4, 4);
    assert_eq!(
        game.best_structure_tier("fabricator"),
        Some(1),
        "a structure carrying no StructureTier is standing at tier 1"
    );

    for (x, tier) in [(5, 3), (6, 2)] {
        game.world.spawn((
            Structure {
                kind: "fabricator".to_string(),
            },
            Position { x, y: 4 },
            StructureTier(tier),
        ));
    }
    assert_eq!(
        game.best_structure_tier("fabricator"),
        Some(3),
        "the player compiles at their best bench, not their newest or their worst"
    );
    assert_eq!(
        game.best_structure_tier("armory"),
        None,
        "and a different kind is a different bench"
    );
}

/// The one recipe of `result` the player can compile right now.
fn recipe_for(game: &mut Game, result: &str) -> CraftRecipe {
    game.craft_recipes()
        .into_iter()
        .find(|r| r.result == ItemId::from(result))
        .unwrap_or_else(|| panic!("{result} should be compilable in this fixture"))
}

/// Every term of a compiled copy's floor, each moving it on its own.
///
/// Asserted against the constants rather than against literals: the numbers
/// are a balance decision and are expected to move, while "a better bench
/// compiles better gear" is the feature.
#[test]
fn the_craft_floor_rises_with_the_bench_and_the_careful_toggle() {
    use crate::tuning::{QUALITY_BASE, QUALITY_BENCH_PER_TIER, QUALITY_CAREFUL_BONUS};
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let benchless = recipe_for(&mut game, "kinetic_edge");
    let bare = game.player_craft_order(&benchless, false);
    assert_eq!(
        game.craft_quality_floor(&bare),
        QUALITY_BASE,
        "a recipe naming no bench compiles at the base floor"
    );

    let careful = game.player_craft_order(&benchless, true);
    assert_eq!(
        game.craft_quality_floor(&careful),
        QUALITY_BASE + QUALITY_CAREFUL_BONUS,
        "being careful about it raises the floor on its own"
    );

    spawn_structure_at(&mut game, "fabricator", 4, 4);
    let benched = recipe_for(&mut game, "arc_lance");
    let tier_one = game.player_craft_order(&benched, false);
    assert_eq!(
        game.craft_quality_floor(&tier_one),
        QUALITY_BASE,
        "a bench at its first tier is worth no more than no bench at all"
    );

    let bench = find_structure_by_kind(&mut game, "fabricator").unwrap();
    game.world.entity_mut(bench).insert(StructureTier(3));
    let tier_three = game.player_craft_order(&benched, false);
    assert_eq!(
        game.craft_quality_floor(&tier_three),
        QUALITY_BASE + 2 * QUALITY_BENCH_PER_TIER,
        "two tiers above the first, so two steps"
    );
}

/// A modded bench with an absurd `max_tier` must not overflow the floor on
/// its way to being clamped — `roll_quality` holds the one clamp, so the
/// floor is allowed to exceed the band but never to wrap.
#[test]
fn an_absurd_bench_tier_clamps_rather_than_wrapping() {
    use crate::tuning::QUALITY_MAX;
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "fabricator", 4, 4);
    let bench = find_structure_by_kind(&mut game, "fabricator").unwrap();
    game.world.entity_mut(bench).insert(StructureTier(9_000));

    let recipe = recipe_for(&mut game, "arc_lance");
    let order = game.player_craft_order(&recipe, true);
    let floor = game.craft_quality_floor(&order);

    assert_eq!(
        game.roll_quality(floor),
        QUALITY_MAX,
        "however good the bench, the band is the band"
    );
}

/// The careful toggle's price: half again of every ingredient line, rounded
/// up, charged on what the player actually pays rather than on the authored
/// recipe.
///
/// The perk half of this is the ordering assertion. `Perk::LeanCompiler`
/// floors a line at 1, and a careful compile of a floored line costs 2 — so
/// the surcharge rides the discounted number. Reversed, a fully perked
/// recipe would be careful for free.
#[test]
fn careful_compiling_costs_half_again_of_the_discounted_price() {
    let mut game = Game::new(47, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let edge = ItemId::from("kinetic_edge");
    let fragment = ItemId::from(ids::CORE_FRAGMENT);

    assert_eq!(
        game.craft_cost(&edge, false),
        vec![(fragment.clone(), 7)],
        "the shipped recipe, unchanged"
    );
    assert_eq!(
        game.craft_cost(&edge, true),
        vec![(fragment.clone(), 11)],
        "seven and a half, rounded up: being careful is never free"
    );

    for _ in 0..10 {
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let _ = game.unlock_perk(Perk::LeanCompiler);
    }
    assert_eq!(
        game.craft_cost(&edge, false),
        vec![(fragment.clone(), 1)],
        "the perk floors a line at one"
    );
    assert_eq!(
        game.craft_cost(&edge, true),
        vec![(fragment, 2)],
        "and careful is half again of *that*, not of the authored seven"
    );
}

/// What the toggle costs the player in units they can actually make, at both
/// of the two places a price is asked: the screen's "max affordable" and the
/// compile's own affordability check.
#[test]
fn a_careful_batch_is_smaller_and_can_be_refused_outright() {
    let mut game = Game::new(48, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let edge = ItemId::from("kinetic_edge");
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 21)]);

    assert_eq!(game.max_craftable(&edge, false), 3, "three at seven each");
    assert_eq!(
        game.max_craftable(&edge, true),
        1,
        "and one at eleven, with change nowhere near a second"
    );

    assert!(
        game.craft(&edge, 2, false).is_ok(),
        "two plain copies are affordable"
    );
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 7)]);
    assert!(
        game.craft(&edge, 1, true).is_err(),
        "exactly one plain copy's worth of material does not buy a careful one"
    );
    assert!(
        game.craft(&edge, 1, false).is_ok(),
        "and the same material still buys the ordinary compile"
    );
}
