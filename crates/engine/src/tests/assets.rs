//! Startup validation of the shipped and modded asset directories.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::tuning::{AFFINITY_MAX, AFFINITY_MIN, AFFINITY_NEUTRAL};
use crate::*;

#[test]
fn game_new_aborts_startup_when_the_item_set_is_missing_the_currency_role() {
    // The economy can't run without a Currency-role item — see
    // `ItemDb::missing_roles` — so `Game::new` must abort before the
    // world is built rather than let play reach `Game::currency()`'s
    // `.expect("validated at startup")` deep in gameplay.
    let dir = assets_dir_missing_currency_item();
    let result = Game::new(900, DifficultyMode::Forgiving, &dir);
    let _ = std::fs::remove_dir_all(&dir);

    // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
    // use `Result::expect_err` / `unwrap_err`.
    let Err(err) = result else {
        panic!("startup should abort rather than run with no item holding the Currency role");
    };
    assert!(
        err.to_string().contains("Currency"),
        "error should name the missing role: {err}"
    );
}

#[test]
fn game_load_aborts_when_the_item_set_is_missing_the_currency_role() {
    // Resuming a save is the other door into the same world, and it
    // reaches the same `Game::currency()` `.expect("validated at
    // startup")` — so an item set that lost its Currency-role holder
    // between saving and loading has to be refused here too, not only
    // in `Game::new`.
    let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_missing_currency_load_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let dir = assets_dir_missing_currency_item();
    let result = Game::load(&path, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&path);

    // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
    // use `Result::expect_err` / `unwrap_err`.
    let Err(err) = result else {
        panic!("loading should abort rather than resume with no item holding the Currency role");
    };
    assert!(
        err.to_string().contains("Currency"),
        "error should name the missing role: {err}"
    );
}

#[test]
fn every_shipped_asset_file_loads_without_a_warning() {
    // A malformed shipped asset is warn-and-skipped like a mod's would
    // be, so it costs the player content silently instead of failing the
    // build. This is the only thing that catches it — a serde attribute
    // missing from `ItemId` once made every asset load fail this way.
    let game = Game::new(901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let skipped: Vec<String> = game
        .message_log(usize::MAX)
        .into_iter()
        .map(|e| e.text)
        .filter(|text| text.contains("skipped invalid"))
        .collect();

    assert!(
        skipped.is_empty(),
        "shipped assets must all parse: {skipped:#?}"
    );
}

#[test]
fn the_shipped_species_kits_reference_only_real_abilities() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();
    let mut declared = 0;
    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                db.get(&ability.id).is_some(),
                "species {:?} names unknown ability {:?}",
                species.id,
                ability.id
            );
            assert!(
                ability.level >= 1 && ability.level <= crate::tuning::CREATURE_MAX_LEVEL,
                "species {:?}: ability {:?} unlocks at level {}, outside 1..={}",
                species.id,
                ability.id,
                ability.level,
                crate::tuning::CREATURE_MAX_LEVEL
            );
            declared += 1;
        }
    }
    assert!(
        declared >= 10,
        "the shipped roster should actually use the ability system, found {declared}"
    );
}

/// Authored text replaced a derivation that could not go blank. The only
/// thing left to guard mechanically is that nothing shipped is *missing*
/// text — a wrong number in an authored line is a review problem, not a
/// test one.
#[test]
fn every_shipped_item_and_structure_has_description_text() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.item_defs() {
        assert!(
            !def.description.trim().is_empty(),
            "item {} ships with no description",
            def.id.as_str()
        );
    }
    for def in game.structure_defs() {
        assert!(
            !def.description.trim().is_empty(),
            "structure {} ships with no description",
            def.id
        );
    }
}

/// Crafting must never mint Credits. Base salvage is deliberately sellable
/// (see `Game::sell_item`), and a Mining Node with a program on it produces
/// that salvage forever — so an item worth more than the sum of its
/// ingredients is an unbounded Credit press, in the one currency that
/// survives a breach. The ceiling is what makes the price ladder safe to
/// raise; it is asserted over the real assets because it is a property of
/// the data, not of the code that reads it.
#[test]
fn no_craftable_item_is_worth_more_than_its_ingredients() {
    let game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.item_defs() {
        let Some(craftable) = &def.craftable else {
            continue;
        };
        let ingredients: u32 = craftable
            .cost
            .iter()
            .map(|(id, qty)| game.item_value(id) * qty)
            .sum();
        assert!(
            game.item_value(&def.id) <= ingredients,
            "{} is worth {} but its recipe costs {} — crafting it to sell prints Credits",
            def.id.as_str(),
            game.item_value(&def.id),
            ingredients
        );
        checked += 1;
    }
    assert!(checked > 20, "only {checked} craftable items were checked");
}

/// The other half of that press, and the one the recipe ceiling above can't
/// see: a `work.produces` structure makes its item out of nothing on a
/// timer, so the item's *value* is a Credit-per-tick rate. The Compiler
/// (`flat_payout`, 8 ticks) would out-earn a Mining Node nearly fourfold if
/// ICE Breakers were priced at their 3-Fragment recipe. Anything a base can
/// print sits at the floor price; worth comes from what you can't print.
#[test]
fn every_base_produced_item_sits_at_the_floor_price() {
    let game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.structure_defs() {
        let Some(work) = &def.work else {
            continue;
        };
        assert_eq!(
            game.item_value(&work.produces),
            tuning::DEFAULT_ITEM_VALUE,
            "{} prints {} every {} ticks, so pricing it above the floor makes the structure a Credit press",
            def.id,
            work.produces.as_str(),
            work.ticks_per_unit
        );
        checked += 1;
    }
    assert_eq!(
        checked, 4,
        "expected the four producing structures; the press ceiling has to cover every one"
    );
}

/// A modded item that predates the `value` field still trades — at the flat
/// rate every item in the game used before the ladder existed.
#[test]
fn an_item_with_no_authored_value_falls_back_to_the_floor_price() {
    let game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let unpriced = ItemId::from("no_such_modded_item");
    assert!(game.item_defs().iter().all(|d| d.id != unpriced));
    assert_eq!(game.item_value(&unpriced), tuning::DEFAULT_ITEM_VALUE);
}

/// The twenty hunt-only routines are reachable exactly one way: off a wild
/// carrier. A species or research file naming one would quietly restore the
/// "just target the species" loop this set exists to break.
#[test]
fn no_species_or_research_file_grants_a_wild_only_ability() {
    let game = Game::new(3301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild_only: Vec<String> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.clone())
        .collect();
    assert_eq!(wild_only.len(), 20, "twenty routines are hunt-only");

    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                !wild_only.contains(&ability.id),
                "species {:?} grants {:?}, which is meant to be findable only in the field",
                species.id,
                ability.id
            );
        }
    }
    for node in game.world.resource::<crate::research::ResearchDb>().all() {
        for id in &node.unlocks_abilities {
            assert!(
                !wild_only.contains(id),
                "research node {:?} unlocks {:?}, which is meant to be findable only in the field",
                node.id,
                id
            );
        }
    }
}

/// A cooldown is measured in battle rounds (`Game::wild_retaliate` ticks it
/// down once per round a hostile carrier could have fired again), so it only
/// means something for an ability that can enter a battle. A cooldown of 0
/// there means a hostile carrier fires the routine every single round.
///
/// Two kinds of ability never enter a battle at all, so a cooldown on either
/// would just be inert: `decompile`, the player's capture mechanism, which
/// hostiles never use and which must stay spammable so a failed capture roll
/// doesn't change the core loop; and any `FieldBuff` ability, which is cast
/// outside battle and limited by `power_cost` instead — the doc on
/// `AbilityEffect::FieldBuff` explains why `cooldown` is dead weight on that
/// variant.
#[test]
fn every_shipped_ability_but_decompile_and_field_routines_has_a_cooldown() {
    let game = Game::new(3302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if def.id == crate::abilities::DECOMPILE_ABILITY_ID {
            assert_eq!(def.cooldown, 0, "decompile stays spammable, deliberately");
            continue;
        }
        if matches!(
            def.effect,
            crate::abilities::AbilityEffect::FieldBuff { .. }
        ) {
            assert_eq!(
                def.cooldown, 0,
                "ability {:?} is field-only, so cooldown should be left at its default",
                def.id
            );
            continue;
        }
        assert!(
            def.cooldown >= 1,
            "ability {:?} has no cooldown, so a wild carrier would fire it every round",
            def.id
        );
    }
}

const AFFINITY_SPECIES: &str = r#"(
    id: "test_healer",
    name: "Test Healer",
    glyph: 'h',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (heal: 1.5, damage: 0.8),
)"#;

#[test]
fn a_species_declares_affinities_and_omitted_ones_stay_neutral() {
    let dir = super::support::modded_assets_dir(
        "affinity_species",
        &[],
        &[],
        &[("test_healer.ron", AFFINITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), 1.5);
    assert_eq!(aff.get(AffinityKind::Damage), 0.8);
    // The three the file never named must default individually, not leave
    // the whole struct at its all-neutral fallback.
    assert_eq!(aff.get(AffinityKind::Buff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Debuff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Drain), AFFINITY_NEUTRAL);
}

#[test]
fn a_species_file_with_no_affinities_field_still_loads_neutral() {
    // The #[serde(default)] contract that keeps every shipped file and
    // every third-party mod parsing untouched.
    let dir = super::support::modded_assets_dir(
        "affinity_absent",
        &[],
        &[],
        &[("test_plain.ron", super::support::TWO_ABILITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_medic").unwrap();
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(aff.get(kind), AFFINITY_NEUTRAL);
    }
}

#[test]
fn an_out_of_range_affinity_is_clamped_at_load() {
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: 99.0");
    let dir = super::support::modded_assets_dir(
        "affinity_clamped",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), AFFINITY_MAX);
}

#[test]
fn an_affinity_below_the_floor_is_clamped_at_load() {
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: 0.0");
    let dir = super::support::modded_assets_dir(
        "affinity_clamped_low",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), AFFINITY_MIN);
}

/// The ten field routines this file ships, one per `FieldBuffKind` variant.
/// Mirrors the table in `assets/abilities/README.md`'s `FieldBuff` section.
const FIELD_ROUTINE_IDS: &[&str] = &[
    "repair_loop",
    "coolant_flush",
    "trickle_charge",
    "hardened_shell",
    "overclock",
    "ablative_layer",
    "deep_scan",
    "trace_analysis",
    "ghost_protocol",
    "salvage_routine",
];

/// Each of the ten field routines loads, carries a `FieldBuff` effect, ships
/// a real description (the picker's only detail line for it — see
/// `AbilityDef::description`), and stays out of the wild-carrier pool: a
/// field routine is installed, never found on a hostile.
#[test]
fn the_ten_field_routines_load_with_real_descriptions_and_no_wild_weight() {
    let game = Game::new(3303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();
    for id in FIELD_ROUTINE_IDS {
        let def = db
            .get(id)
            .unwrap_or_else(|| panic!("missing field routine {id:?}"));
        assert!(
            matches!(
                def.effect,
                crate::abilities::AbilityEffect::FieldBuff { .. }
            ),
            "{id:?} should carry a FieldBuff effect, got {:?}",
            def.effect
        );
        assert!(
            !def.description.trim().is_empty(),
            "{id:?} ships with no description"
        );
        assert_eq!(
            def.wild_weight, 0,
            "{id:?} should never spawn on a wild carrier"
        );
    }
}

/// Every `FieldBuffKind` variant must be exercised by at least one shipped
/// ability. The match below is exhaustive on purpose — no wildcard arm — so
/// an eleventh `FieldBuffKind` added later without shipped content fails to
/// *compile* here rather than silently shipping a buff kind nothing grants.
#[test]
fn every_field_buff_kind_is_exercised_by_a_shipped_ability() {
    use crate::abilities::AbilityEffect;
    use crate::components::FieldBuffKind;

    let game = Game::new(3304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();

    let mut regen = false;
    let mut coolant = false;
    let mut trickle = false;
    let mut def_kind = false;
    let mut atk = false;
    let mut mitigation = false;
    let mut capture_boost = false;
    let mut xp_boost = false;
    let mut encounter_damp = false;
    let mut drop_boost = false;

    for def in db.all() {
        if let AbilityEffect::FieldBuff { kind, .. } = &def.effect {
            match kind {
                FieldBuffKind::Regen => regen = true,
                FieldBuffKind::Coolant => coolant = true,
                FieldBuffKind::Trickle => trickle = true,
                FieldBuffKind::Def => def_kind = true,
                FieldBuffKind::Atk => atk = true,
                FieldBuffKind::Mitigation => mitigation = true,
                FieldBuffKind::CaptureBoost => capture_boost = true,
                FieldBuffKind::XpBoost => xp_boost = true,
                FieldBuffKind::EncounterDamp => encounter_damp = true,
                FieldBuffKind::DropBoost => drop_boost = true,
            }
        }
    }

    assert!(regen, "no shipped ability grants a Regen field buff");
    assert!(coolant, "no shipped ability grants a Coolant field buff");
    assert!(trickle, "no shipped ability grants a Trickle field buff");
    assert!(def_kind, "no shipped ability grants a Def field buff");
    assert!(atk, "no shipped ability grants an Atk field buff");
    assert!(
        mitigation,
        "no shipped ability grants a Mitigation field buff"
    );
    assert!(
        capture_boost,
        "no shipped ability grants a CaptureBoost field buff"
    );
    assert!(xp_boost, "no shipped ability grants an XpBoost field buff");
    assert!(
        encounter_damp,
        "no shipped ability grants an EncounterDamp field buff"
    );
    assert!(
        drop_boost,
        "no shipped ability grants a DropBoost field buff"
    );
}

#[test]
fn a_nan_affinity_disqualifies_the_file_and_the_rest_still_load() {
    // NaN specifically, not just inf: f32::clamp returns NaN for a NaN
    // input, so the clamp alone would pass this straight through into
    // every magnitude the species ever casts.
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: NaN");
    let dir = super::support::modded_assets_dir(
        "affinity_nan",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(
        game.species_affinities("test_healer").is_none(),
        "a species with a non-finite affinity should not have loaded"
    );
    // A single bad mod file must not take the shipped roster down with it.
    assert!(game.species_affinities("drone").is_some());
}

/// A routine nothing grants is content that cannot be reached: it loads, it
/// passes every schema check, and no player will ever see it. The ten field
/// routines shipped in exactly that state once, so this pins the *other* half
/// of the contract — an ability file existing is not the same as it being
/// obtainable.
///
/// Research hands routine items to the player; a species file is what a
/// companion's kit comes from. Either counts as reachable.
#[test]
fn every_shipped_field_routine_can_actually_be_obtained() {
    let game = Game::new(4711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let granted: std::collections::HashSet<&str> = game
        .world
        .resource::<crate::research::ResearchDb>()
        .all()
        .flat_map(|node| node.unlocks_abilities.iter())
        .map(|id| id.as_str())
        .chain(
            game.world
                .resource::<SpeciesDb>()
                .all()
                .flat_map(|s| s.abilities.iter())
                .map(|a| a.id.as_str()),
        )
        .collect();

    let unreachable: Vec<&str> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .filter(|def| {
            matches!(
                def.effect,
                crate::abilities::AbilityEffect::FieldBuff { .. }
            )
        })
        .map(|def| def.id.as_str())
        .filter(|id| !granted.contains(id))
        .collect();

    assert!(
        unreachable.is_empty(),
        "these field routines are in no research node and no species kit, \
         so nothing in a real game can ever hand them over: {unreachable:?}"
    );
}

/// The "no second recipe format" property, pinned by a test: a machine's
/// recipe *is* the named item's own `craftable.cost`, resolved through the
/// item db. Nothing can drift from it, because there is nothing else to
/// drift — and a modder who adds a craftable item gets an automatable one
/// for free.
#[test]
fn an_assembler_runs_the_named_items_own_craftable_recipe() {
    let game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<ItemDb>();
    let authored = items
        .get(ids::POWER_CELL)
        .and_then(|d| d.craftable.as_ref())
        .expect("power_cell ships with a recipe");

    let def: crate::structures::StructureDef = ron::from_str(
        r#"(
            id: "test_assembler", name: "Test Assembler", glyph: 'A', color: Cyan,
            build_cost: [],
            work: None,
            assembles: Some((item: "power_cell", ticks_per_unit: 8)),
        )"#,
    )
    .expect("an assembles block must parse");

    assert_eq!(
        crate::systems::assembly_recipe(&def, items),
        Some(authored.cost.as_slice()),
        "the machine's recipe is the item's own, not a second copy of it"
    );
}

/// A structure with no `assembles` has no recipe to run — the resolver must
/// say so rather than falling back to something.
#[test]
fn a_structure_that_assembles_nothing_resolves_no_recipe() {
    let game = Game::new(904, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<ItemDb>();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let mining_node = db.get("mining_node").expect("mining_node ships");

    assert!(crate::systems::assembly_recipe(mining_node, items).is_none());
}

/// Without this, a typo'd `assembles` ships a machine that can never run and
/// says nothing at all. The same trap a mod falls into.
#[test]
fn every_shipped_assembles_names_an_item_that_declares_a_recipe() {
    let game = Game::new(903, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<ItemDb>();
    let mut checked = 0;
    for def in game
        .world
        .resource::<crate::structures::StructureDb>()
        .all()
    {
        if let Some(assembles) = &def.assembles {
            assert!(
                items
                    .get(assembles.item.as_str())
                    .is_some_and(|d| d.craftable.is_some()),
                "{} assembles {:?}, which is not a craftable item",
                def.id,
                assembles.item
            );
            assert!(
                assembles.ticks_per_unit > 0,
                "{} would produce a unit every tick",
                def.id
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 3,
        "the shipped chain is Refinery, Winding Node and Assembly Bay — if that \
         changes, change this count deliberately rather than letting the check go vacuous"
    );
}
