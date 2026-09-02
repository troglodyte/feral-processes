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

/// Every copy of `item` the player is carrying, from **both** stores.
///
/// A copy that rolls exactly `QUALITY_DEFAULT` is plain by definition —
/// nothing tells it from an authored one — so it stacks in `Inventory`
/// rather than taking a ledger row, and a batch that read the ledger alone
/// would come back short by however many rolled perfectly.
fn compiled_copies(game: &Game, item: &str) -> Vec<GearCopy> {
    let player = game.player_entity();
    let id = ItemId::from(item);
    let plain = game
        .world
        .get::<Inventory>(player)
        .map(|inv| inv.count(&id))
        .unwrap_or(0);
    let mut copies: Vec<GearCopy> =
        std::iter::repeat_n(GearCopy::plain(id.clone()), plain as usize).collect();
    if let Some(ledger) = game.world.get::<GearCopies>(player) {
        for (copy, qty) in ledger.copies.iter().filter(|(c, _)| c.item == id) {
            copies.extend(std::iter::repeat_n(copy.clone(), *qty as usize));
        }
    }
    copies
}

/// A compiled piece of gear is a *copy* now, not a stack: it carries the
/// quality it rolled and so lands in the ledger rather than in `Inventory`.
#[test]
fn a_compiled_piece_of_gear_carries_the_quality_it_rolled() {
    use crate::tuning::{QUALITY_BASE, QUALITY_SPREAD};
    let mut game = Game::new(49, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let edge = ItemId::from("kinetic_edge");
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 7)]);

    game.craft(&edge, 1, false).unwrap();

    let copies = compiled_copies(&game, "kinetic_edge");
    assert_eq!(copies.len(), 1, "one compile, one copy");
    assert_eq!(
        copies[0].rarity,
        Rarity::Ordinary,
        "crafting is not a chase"
    );
    assert!(copies[0].affixes.is_empty(), "and it rolls no affix either");
    assert!(
        (QUALITY_BASE..=QUALITY_BASE + QUALITY_SPREAD).contains(&copies[0].quality),
        "a bare bench compiles inside its band, got {}",
        copies[0].quality
    );
    assert_eq!(
        held(&game, &edge),
        u32::from(copies[0].quality == crate::tuning::QUALITY_DEFAULT),
        "a copy that rolled the authored spec is plain and stacks; anything \
         else takes a ledger row of its own"
    );
}

/// The loop the axis exists for: compile a batch, keep the best. The roll is
/// per unit, so a batch is a spread rather than N of one thing.
///
/// Five units rather than the twelve this used to ask for: a gear recipe no
/// machine assembles costs `HAND_CRAFT_DEFAULT_CYCLE` times the multiplier
/// per unit, which is fifteen points of Power, and
/// `tuning::HAND_CRAFT_POWER_FLOOR` refuses a batch past the reserve. Five
/// is as many rolls as the spread needs and the batch size was never the
/// subject.
#[test]
fn a_batch_compiles_copies_that_differ_from_each_other() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let edge = ItemId::from("kinetic_edge");
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 7 * 5)]);

    game.craft(&edge, 5, false).unwrap();

    let mut seen: Vec<u8> = compiled_copies(&game, "kinetic_edge")
        .iter()
        .map(|c| c.quality)
        .collect();
    assert_eq!(seen.len(), 5, "five units, five copies");
    seen.sort_unstable();
    seen.dedup();
    assert!(
        seen.len() > 1,
        "a per-unit roll should spread a batch, got {seen:?}"
    );
}

/// What the base buys: a developed bench and a careful compile put the
/// *whole* batch above what a bare one can reach.
///
/// The reserve is refilled between the two batches for the reason the pack
/// is restocked between them: a hand-compile burns Power a tick and
/// `tuning::HAND_CRAFT_POWER_FLOOR` refuses the second batch off the back
/// of the first otherwise.
#[test]
fn a_better_bench_lifts_every_copy_in_the_batch() {
    use crate::tuning::{
        QUALITY_BASE, QUALITY_BENCH_PER_TIER, QUALITY_CAREFUL_BONUS, QUALITY_SPREAD,
    };
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let lance = ItemId::from("arc_lance");
    spawn_structure_at(&mut game, "fabricator", 4, 4);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 12 * 5)]);

    game.craft(&lance, 5, false).unwrap();
    let bare = compiled_copies(&game, "arc_lance");
    assert!(
        bare.iter()
            .all(|c| c.quality <= QUALITY_BASE + QUALITY_SPREAD),
        "a tier-one bench cannot reach past its band: {:?}",
        bare.iter().map(|c| c.quality).collect::<Vec<_>>()
    );

    let bench = find_structure_by_kind(&mut game, "fabricator").unwrap();
    game.world.entity_mut(bench).insert(StructureTier(5));
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 18 * 5)]);
    fill_power(&mut game);
    game.craft(&lance, 5, true).unwrap();

    let floor = QUALITY_BASE + 4 * QUALITY_BENCH_PER_TIER + QUALITY_CAREFUL_BONUS;
    let developed: Vec<u8> = compiled_copies(&game, "arc_lance")
        .iter()
        .map(|c| c.quality)
        .filter(|q| *q >= floor)
        .collect();
    assert_eq!(
        developed.len(),
        5,
        "every copy off the developed bench should clear {floor}"
    );
}

/// Only gear costs a quality roll, and the way to see it is against the
/// ticks the compile spends anyway.
///
/// A hand-compile's clock cost scales with the batch now, so comparing one
/// unit against five no longer isolates anything: the two runs would differ
/// because they ticked different numbers of times. The honest comparison is
/// a compile against the *same span of bare ticks* — a material lands the
/// shared stream in exactly the place doing nothing would, and a piece of
/// gear does not.
#[test]
fn only_gear_spends_a_quality_roll() {
    fn fixture() -> Game {
        let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 500)]);
        game
    }
    fn stream_after_compiling(item: &str, quantity: u32) -> u64 {
        let mut game = fixture();
        game.craft(&ItemId::from(item), quantity, false).unwrap();
        game.world.resource_mut::<GameRng>().0.random()
    }
    fn stream_after_ticks(ticks: u32) -> u64 {
        let mut game = fixture();
        for _ in 0..ticks {
            game.tick();
        }
        game.world.resource_mut::<GameRng>().0.random()
    }
    fn unit_ticks(item: &str) -> u32 {
        fixture().hand_craft_ticks(&ItemId::from(item))
    }

    assert_eq!(
        stream_after_compiling(ids::ICE_BREAKER, 5),
        stream_after_ticks(5 * unit_ticks(ids::ICE_BREAKER)),
        "a material compiles off the ticks alone and spends no draw of its own"
    );
    assert_ne!(
        stream_after_compiling("kinetic_edge", 1),
        stream_after_ticks(unit_ticks("kinetic_edge")),
        "gear rolls its quality, so one unit is one draw past the ticks"
    );
}

/// The perk term, the player-agency half of the bench term: every level of
/// `Perk::TightenTolerances` raises the floor a compiled copy rolls off, and
/// the spread is the same width above it.
///
/// Player *level* is deliberately not a term here — `scaled_for_level`
/// already scales gear to its wearer, so a level term inside quality would
/// double-dip on the same input. The perk is the same idea spent as a
/// choice.
#[test]
fn the_craft_floor_rises_with_the_tighten_tolerances_perk() {
    use crate::tuning::{QUALITY_BASE, QUALITY_PERK_PER_LEVEL, QUALITY_SPREAD};
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let edge = ItemId::from("kinetic_edge");

    let recipe = recipe_for(&mut game, "kinetic_edge");
    let unperked = game.player_craft_order(&recipe, false);
    assert_eq!(
        game.craft_quality_floor(&unperked),
        QUALITY_BASE,
        "no levels, no term"
    );

    for _ in 0..2 {
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::TightenTolerances).unwrap();
    }
    let perked = game.player_craft_order(&recipe, false);
    assert_eq!(
        game.craft_quality_floor(&perked),
        QUALITY_BASE + 2 * QUALITY_PERK_PER_LEVEL,
        "two levels, two steps"
    );

    // And it reaches the compile itself, not just the quoted floor: a bench
    // the recipe never names cannot be what lifted these.
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 7 * 6)]);
    game.craft(&edge, 6, false).unwrap();
    let floor = QUALITY_BASE + 2 * QUALITY_PERK_PER_LEVEL;
    let rolled: Vec<u8> = compiled_copies(&game, "kinetic_edge")
        .iter()
        .map(|c| c.quality)
        .collect();
    assert_eq!(rolled.len(), 6, "six units, six copies");
    assert!(
        rolled
            .iter()
            .all(|q| (floor..=floor + QUALITY_SPREAD).contains(q)),
        "every copy should sit in the perked band {floor}..={}, got {rolled:?}",
        floor + QUALITY_SPREAD
    );
}

// ---- Hand-compiling costs real time ----

/// The cycle a hand-compile is priced off is the machine's own, so the
/// number moves with the content rather than with a literal in the test.
fn assembler_cycle(game: &Game, kind: &str) -> u32 {
    game.world
        .resource::<StructureDb>()
        .get(kind)
        .unwrap()
        .assembles
        .as_ref()
        .unwrap()
        .ticks_per_unit
}

fn work_cycle(game: &Game, kind: &str) -> u32 {
    game.world
        .resource::<StructureDb>()
        .get(kind)
        .unwrap()
        .work
        .as_ref()
        .unwrap()
        .ticks_per_unit
}

/// The machine exists to do this, so hand-compiling is priced off the
/// machine's own cycle — the Lathe's, for a Blank Substrate.
#[test]
fn a_hand_compile_is_priced_off_the_assembler_that_makes_it() {
    let game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert_eq!(
        game.hand_craft_ticks(&ItemId::from(ids::BLANK_SUBSTRATE)),
        crate::tuning::HAND_CRAFT_TICK_MULT * assembler_cycle(&game, "lathe"),
    );
}

/// An extractor's `work` block is the second lookup, so an item no
/// assembler builds but a node produces is still priced off a machine.
#[test]
fn a_hand_compile_falls_back_to_the_extractor_that_produces_it() {
    let game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert_eq!(
        game.hand_craft_ticks(&ItemId::from(ids::POWER_CELL)),
        crate::tuning::HAND_CRAFT_TICK_MULT * work_cycle(&game, "power_conduit"),
    );
}

/// Most craftables have no machine at all, and they still have to cost
/// something.
#[test]
fn a_hand_compile_with_no_machine_takes_the_default_cycle() {
    let game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert_eq!(
        game.hand_craft_ticks(&ItemId::from("kinetic_edge")),
        crate::tuning::HAND_CRAFT_TICK_MULT * crate::tuning::HAND_CRAFT_DEFAULT_CYCLE,
    );
}

/// The screen's figure and the loop's cost are the same number, so the
/// clock has to move by exactly what `hand_craft_ticks` quotes.
#[test]
fn compiling_one_unit_spends_its_whole_cycle() {
    let mut game = Game::new(303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST)]);
    let ice = ItemId::from(ids::ICE_BREAKER);
    let cost = game.hand_craft_ticks(&ice);
    let before = game.current_tick();

    game.craft(&ice, 1, false).unwrap();

    assert_eq!(game.current_tick() - before, u64::from(cost));
}

#[test]
fn a_batch_spends_a_whole_cycle_per_unit() {
    let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST * 3)],
    );
    let ice = ItemId::from(ids::ICE_BREAKER);
    let cost = game.hand_craft_ticks(&ice);
    let before = game.current_tick();

    game.craft(&ice, 3, false).unwrap();

    assert_eq!(game.current_tick() - before, u64::from(3 * cost));
    assert_eq!(count_item(&game, ids::ICE_BREAKER), 3);
}

/// Every refusal lands before anything is spent — asserted per refusal,
/// because a single test over one of them passes against the four paths
/// that never spend anyway.
fn refused_compile_spends_nothing(
    game: &mut Game,
    result: &ItemId,
    quantity: u32,
    arrange: impl FnOnce(&mut Game),
) {
    set_inventory(game, &[(ids::CORE_FRAGMENT, 50)]);
    arrange(game);
    let before = game.current_tick();

    assert!(game.craft(result, quantity, false).is_err());

    assert_eq!(game.current_tick(), before, "a refusal spends no time");
    assert_eq!(
        count_item(game, ids::CORE_FRAGMENT),
        50,
        "a refusal spends no material"
    );
}

#[test]
fn a_compile_refused_for_a_battle_spends_nothing() {
    let mut game = Game::new(305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    refused_compile_spends_nothing(&mut game, &ItemId::from(ids::ICE_BREAKER), 1, |g| {
        start_battle_with_a_wild_program(g);
    });
}

#[test]
fn a_compile_refused_for_game_over_spends_nothing() {
    let mut game = Game::new(306, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    refused_compile_spends_nothing(&mut game, &ItemId::from(ids::ICE_BREAKER), 1, |g| {
        g.world.resource_mut::<GameOver>().reason = Some("done".into());
    });
}

#[test]
fn a_compile_of_zero_units_spends_nothing() {
    let mut game = Game::new(307, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    refused_compile_spends_nothing(&mut game, &ItemId::from(ids::ICE_BREAKER), 0, |_| {});
}

#[test]
fn a_compile_with_no_recipe_spends_nothing() {
    let mut game = Game::new(308, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    refused_compile_spends_nothing(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 1, |_| {});
}

#[test]
fn a_compile_short_of_material_spends_nothing() {
    let mut game = Game::new(309, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = game.current_tick();
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST - 1)],
    );

    assert!(
        game.craft(&ItemId::from(ids::ICE_BREAKER), 1, false)
            .is_err()
    );

    assert_eq!(game.current_tick(), before, "a refusal spends no time");
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        ICE_BREAKER_CORE_COST - 1,
        "a refusal spends no material"
    );
}

/// An abort keeps every completed unit and refunds the one in flight, so
/// the only thing walking away costs is the time already spent.
#[test]
fn aborting_keeps_the_finished_units_and_refunds_the_one_in_flight() {
    let mut game = Game::new(310, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ice = ItemId::from(ids::ICE_BREAKER);
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST * 3)],
    );
    let per_unit = game.hand_craft_ticks(&ice);

    game.begin_hand_craft(&ice, 3, false).unwrap();
    // Through the first unit, then one tick into the second.
    for _ in 0..=per_unit {
        game.advance_hand_craft();
    }
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        ICE_BREAKER_CORE_COST,
        "the first two units' material is spent, the third's untouched"
    );

    game.abort_hand_craft();

    assert!(!game.hand_craft_in_progress());
    assert_eq!(count_item(&game, ids::ICE_BREAKER), 1, "one unit finished");
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        ICE_BREAKER_CORE_COST * 2,
        "the in-flight unit is refunded and the third was never taken"
    );
}

/// The finished report has no unit in flight to size a bar against, so a
/// naive implementation reports `ticks_total: 0` on exactly the frame a
/// progress bar most needs a denominator — the batch's last one.
#[test]
fn the_finished_report_still_carries_the_full_tick_total() {
    let mut game = Game::new(313, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ice = ItemId::from(ids::ICE_BREAKER);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST)]);
    let ticks_total = game.hand_craft_ticks(&ice);

    game.begin_hand_craft(&ice, 1, false).unwrap();
    let mut last = None;
    for _ in 0..ticks_total {
        last = game.advance_hand_craft();
    }

    let progress = last.expect("the batch's last tick reports a progress");
    assert!(progress.finished, "the batch should be done by now");
    assert_eq!(
        progress.ticks_total, ticks_total,
        "the bar needs a real denominator on the frame that ends the batch"
    );
}

/// A tick can start a fight — `nest_aggro_tick` is the precedent, and a
/// compile loop inherits the obligation drag terrain already carries: the
/// remaining ticks must not resolve behind a fight the player has not seen.
#[test]
fn a_battle_opening_mid_compile_ends_the_loop_with_the_unit_refunded() {
    let mut game = Game::new(311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ice = ItemId::from(ids::ICE_BREAKER);
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST * 2)],
    );
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let nest = game.spawn_nest("scrapper", pos.x + 1, pos.y);
    game.provoke_nest(nest);
    let before = game.current_tick();

    game.craft(&ice, 2, false).unwrap();

    assert!(
        game.has_active_battle(),
        "the fixture never started a fight"
    );
    assert_eq!(
        game.current_tick() - before,
        1,
        "the rest of the compile must not run behind a fight"
    );
    assert_eq!(count_item(&game, ids::ICE_BREAKER), 0, "no unit finished");
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        ICE_BREAKER_CORE_COST * 2,
        "the interrupted unit's material comes back"
    );
    assert!(!game.hand_craft_in_progress());
}

/// Compiling by hand burns Power at the standing per-tick rate, so a long
/// enough batch flatlines the player without ever asking them.
///
/// The drain is the feature and stays — what is refused is the batch that
/// can be seen in advance to end the run, whole rather than shortened, per
/// the no-silent-caps rule `MAX_ACTIVE_CONTRACTS` states.
#[test]
fn a_batch_that_would_run_the_reserve_out_is_refused_whole() {
    let mut game = Game::new(320, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ice = ItemId::from(ids::ICE_BREAKER);
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST * 8)],
    );

    // Eight units is 640 ticks at the Compiler's own cycle times the
    // shipped multiplier, and 96 Power against a reserve that tops out at
    // 100 — a batch nothing but the clock could stop.
    let refusal = game
        .craft(&ice, 8, false)
        .expect_err("a batch past the reserve must be refused");
    assert!(
        refusal.contains("Power"),
        "the refusal has to say what stopped it, not just refuse: {refusal}"
    );
    assert_eq!(
        count_item(&game, ids::ICE_BREAKER),
        0,
        "a refused batch compiles nothing at all rather than as many as fit"
    );
}

/// The other half of the same rule: a batch the reserve can carry is not
/// refused, or the feature is a ban on hand-compiling rather than a bound
/// on it.
#[test]
fn a_batch_the_reserve_can_carry_still_compiles() {
    let mut game = Game::new(321, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ice = ItemId::from(ids::ICE_BREAKER);
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST * 5)],
    );

    game.craft(&ice, 5, false)
        .expect("five units is 60 Power out of a full hundred");

    assert_eq!(count_item(&game, ids::ICE_BREAKER), 5);
}

/// The floor is a margin above `POWER_MIN`, not `POWER_MIN` itself: a batch
/// projected to land at a few points left starves on the next background
/// tick, which is the state the refusal exists to prevent reached one tick
/// later.
#[test]
fn the_reserve_floor_is_a_margin_and_not_zero() {
    let make = |reserve: f32| {
        let mut game = Game::new(322, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, ICE_BREAKER_CORE_COST)]);
        let player = game.player_entity();
        *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(reserve);
        game
    };
    let ice = ItemId::from(ids::ICE_BREAKER);

    // One unit is 12 Power. From 20 it ends above zero and below the floor.
    assert!(
        make(20.0).craft(&ice, 1, false).is_err(),
        "landing under the floor is refused even though it never reaches zero"
    );
    assert!(
        make(30.0).craft(&ice, 1, false).is_ok(),
        "landing clear of the floor is not this refusal's business"
    );
}

/// The sixth refusal, asserted on its own like the other five — a batch
/// stopped for Power must not have spent a tick or a fragment on the way
/// to being stopped.
#[test]
fn a_compile_refused_for_power_spends_nothing() {
    let mut game = Game::new(323, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    refused_compile_spends_nothing(&mut game, &ItemId::from(ids::ICE_BREAKER), 8, |g| {
        // 50 fragments is already more than eight units cost, so the
        // material check cannot be what refuses this.
        let player = g.player_entity();
        assert!(
            g.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::CORE_FRAGMENT))
                >= ICE_BREAKER_CORE_COST * 8
        );
    });
}

/// The quoted maximum is a batch the compile takes.
///
/// `[M]` used to answer the pack alone, and the pack stopped being the only
/// ceiling the moment a batch could be refused for the Power its ticks
/// burn. This is the careful surcharge's own rule in a second place: a
/// maximum the compile refuses reads as the key doing nothing.
#[test]
fn the_quoted_maximum_is_a_batch_the_compile_accepts() {
    for item in [ids::ICE_BREAKER, "kinetic_edge"] {
        let mut game = Game::new(324, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 500)]);
        let id = ItemId::from(item);

        let most = game.max_craftable(&id, false);
        assert!(
            most > 0,
            "{item} should be compilable at all in this fixture"
        );
        assert!(
            most < 500,
            "{item}'s quote is still bounded by the pack somewhere below the \
             fragments on hand"
        );

        game.craft(&id, most, false)
            .unwrap_or_else(|e| panic!("the quote for {item} was refused: {e}"));
    }
}
