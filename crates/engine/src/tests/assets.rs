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

/// A production line is a straight line: raw producer → refiner → bench, each
/// machine pulling from exactly one upstream. That is a property of the
/// *recipes*, not of the machines — an assembler runs its product's own
/// `craftable.cost`, so a second ingredient added to any of those four items
/// silently turns its bench back into a corner puzzle needing two lines stood
/// up before anything comes out. That shape is what the flatten removed.
///
/// The engine still supports multi-input assemblers and mods may ship them —
/// `chains::a_machine_short_one_of_its_two_ingredients_stays_starved` walks
/// that path. This is a statement about shipped content only.
#[test]
fn every_shipped_assembler_recipe_is_a_single_ingredient() {
    let game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.item_defs();
    let mut checked = 0;
    for def in game.structure_defs() {
        let Some(assembles) = &def.assembles else {
            continue;
        };
        let recipe = items
            .iter()
            .find(|i| i.id == assembles.item)
            .and_then(|i| i.craftable.as_ref())
            .unwrap_or_else(|| panic!("{} assembles {} with no recipe", def.id, assembles.item));
        assert_eq!(
            recipe.cost.len(),
            1,
            "{} assembles {} out of {} ingredients — a bench on two feeders needs two lines",
            def.id,
            assembles.item.as_str(),
            recipe.cost.len()
        );
        checked += 1;
    }
    assert_eq!(
        checked, 9,
        "expected the nine shipped assemblers; one that lost its recipe would drop out of this scan unnoticed"
    );
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

/// Standard and premium armour and modules are made *out of* the factory, so
/// the base is the way gear happens rather than an option beside the fragment
/// economy. Fourteen `.ron` files agreeing with each other is not a policy —
/// any one of them reverted to raw Core Fragments would opt that item back out
/// in silence, and nothing else in the suite would notice.
///
/// "Factory-made" is derived from what structures actually `assembles`, never
/// a list here, so a mod that adds a refiner extends this for free. Weapons
/// are deliberately outside it: they stay on the fragment economy, and moving
/// them is a decision to make, not a gap to fill.
#[test]
fn standard_and_premium_gear_is_made_from_intermediates() {
    let game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.structure_defs();
    let factory_made: std::collections::HashSet<&str> = structures
        .iter()
        .filter_map(|def| def.assembles.as_ref())
        .map(|assembles| assembles.item.as_str())
        .collect();

    let mut checked = 0;
    for def in game.item_defs() {
        let Some((slot, _)) = &def.equipment else {
            continue;
        };
        if !matches!(
            slot,
            crate::items::EquipmentSlot::Armor | crate::items::EquipmentSlot::Module
        ) {
            continue;
        }
        // No bench means the scavenged tier, which is the other test's job.
        let Some(craftable) = def
            .craftable
            .as_ref()
            .filter(|c| c.requires_structure.is_some())
        else {
            continue;
        };
        assert!(
            craftable
                .cost
                .iter()
                .any(|(id, _)| factory_made.contains(id.as_str())),
            "{} is bench-gated gear whose recipe names no factory-made ingredient, so it can be built without a production line",
            def.id.as_str()
        );
        checked += 1;
    }
    assert_eq!(
        checked, 13,
        "expected the six armour and seven module recipes; an item that lost its bench would drop out of this scan unnoticed"
    );
}

/// The other side of that policy, and the one it could destroy by accident:
/// scavenged gear stays craftable with no base standing at all. It is what a
/// fresh run — or a run whose base was raided flat — equips out of, so an
/// intermediate creeping into one of these recipes locks gear behind the very
/// thing the player has just lost.
#[test]
fn scavenged_gear_stays_benchless_and_fragment_only() {
    let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.structure_defs();
    let factory_made: std::collections::HashSet<&str> = structures
        .iter()
        .filter_map(|def| def.assembles.as_ref())
        .map(|assembles| assembles.item.as_str())
        .collect();

    let mut checked = 0;
    for def in game.item_defs() {
        let Some((slot, _)) = &def.equipment else {
            continue;
        };
        if !matches!(
            slot,
            crate::items::EquipmentSlot::Armor | crate::items::EquipmentSlot::Module
        ) {
            continue;
        }
        let Some(craftable) = def
            .craftable
            .as_ref()
            .filter(|c| c.requires_structure.is_none())
        else {
            continue;
        };
        for (id, _) in &craftable.cost {
            assert!(
                !factory_made.contains(id.as_str()),
                "{} is the benchless fallback but wants {}, which only a production line makes",
                def.id.as_str(),
                id.as_str()
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked, 4,
        "expected the two scavenged armour and two scavenged module recipes"
    );
}

/// `cache_drop` is rolled once per cache for every item that declares one,
/// so the expected haul is the *sum* across the item set, not a pick from a
/// list — which means any one item's number is a change to what every cache
/// in the game holds. `assets/items/README.md` tells modders the shipped set
/// "totals a little over one item per cache" and asks them to keep their own
/// numbers low on that basis; nothing checked it, and dropping the Power
/// Cell's 0.4 on 2026-08-04 moved the total from 1.55 to 1.30.
#[test]
fn a_stack_cache_holds_a_little_over_one_item_on_average() {
    let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let expected: f32 = game
        .item_defs()
        .iter()
        .filter_map(|def| def.cache_drop)
        .sum();
    assert!(
        (1.0..=1.6).contains(&expected),
        "shipped caches now average {expected} items, which is no longer the \
         'a little over one' the items README promises a modder"
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

/// The twenty-five hunt-only routines are reachable exactly one way: off a wild
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
    assert_eq!(wild_only.len(), 25, "twenty-five routines are hunt-only");

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

/// The scope word an ability's `name` must end in, given what it targets.
/// `OneAlly` and `OneEnemyGroupFront` share "Single" — one recipient either
/// way, and which side it lands on is never in doubt from the picker.
fn scope_word(target: crate::abilities::AbilityTarget) -> &'static str {
    use crate::abilities::AbilityTarget::*;
    match target {
        OneAlly | OneEnemyGroupFront => "Single",
        WholeParty => "Party",
        WholeEnemyGroup => "Group",
        AllEnemies => "Everyone",
    }
}

/// Strips a trailing ` vN.N` version tag, which is how two abilities in the
/// same family at the same scope are told apart by magnitude.
fn without_version_tag(name: &str) -> &str {
    let Some((base, tag)) = name.rsplit_once(' ') else {
        return name;
    };
    let is_tag = tag.strip_prefix('v').is_some_and(|v| {
        v.split_once('.')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
            && v.chars().all(|c| c.is_ascii_digit() || c == '.')
    });
    if is_tag { base } else { name }
}

/// An ability's display name is `<Family> <Scope>`, plus an optional
/// `vN.N` when two in one family share a scope and differ only in
/// magnitude. The suffix is the whole point: the picker shows the name
/// before it shows anything else, so "does this hit one thing, a group, or
/// the field" has to be readable without opening the description.
///
/// This is a naming *policy*, not a schema rule — `AbilityDb` loads a
/// badly-named file happily, and a mod is free to ignore it. What the test
/// guards is the shipped set, where a new file that skips the suffix would
/// otherwise read as a different kind of thing from its 40 neighbours.
#[test]
fn every_shipped_ability_name_ends_in_the_scope_it_targets() {
    let game = Game::new(3303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        let expected = scope_word(def.target);
        let base = without_version_tag(&def.name);
        assert!(
            base.ends_with(expected) && base.len() > expected.len(),
            "ability {:?} is named {:?}; targeting {:?} it should end in {:?}",
            def.id,
            def.name,
            def.target,
            expected
        );
    }
}

/// Two abilities sharing a display name is invisible in the picker — the
/// player sees two identical rows and picks by luck. The version tag exists
/// so same-family, same-scope siblings stay distinguishable; this is what
/// makes forgetting one a failure rather than a shrug.
#[test]
fn no_two_shipped_abilities_share_a_display_name() {
    let game = Game::new(3304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut seen: std::collections::HashMap<&str, &crate::abilities::AbilityId> =
        std::collections::HashMap::new();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if let Some(other) = seen.insert(def.name.as_str(), &def.id) {
            panic!(
                "abilities {:?} and {:?} both display as {:?}",
                other, def.id, def.name
            );
        }
    }
}

/// The family an ability's display name declares — everything before the
/// scope word, with any version tag already gone. `"Fork Bomb Group"` is
/// `"Fork Bomb"`, and so is `"Fork Bomb Everyone"`.
fn family(def: &crate::abilities::AbilityDef) -> String {
    let base = without_version_tag(&def.name);
    base.trim_end_matches(scope_word(def.target)).trim().into()
}

/// A family that reaches the whole field must also reach one group. The
/// wide scope is the *prize* of a family, so a hole underneath it is a
/// player meeting an effect they can never use at the scale they'd want it
/// — and a hole is invisible in a directory listing, since the files are
/// named for flavour rather than for the family they belong to.
///
/// Pipeline Stall shipped exactly that way: `bus_fault` hit everything and
/// nothing hit one group. Deliberately vacuous on the ally-facing families
/// — Patch, Hyperthread and Bastion top out at Party, because there is no
/// wider ally scope for them to be missing.
#[test]
fn no_ability_family_reaches_everyone_without_reaching_group() {
    use crate::abilities::AbilityTarget;
    let game = Game::new(3305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs: Vec<_> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .collect();
    for def in defs.iter().filter(|d| d.target == AbilityTarget::AllEnemies) {
        let fam = family(def);
        assert!(
            defs.iter()
                .any(|d| d.target == AbilityTarget::WholeEnemyGroup && family(d) == fam),
            "{:?} is {fam:?} at Everyone scope, but nothing in that family hits one group",
            def.id
        );
    }
}

/// Reaching every hostile on the field is the top of the scope ladder, and
/// the shipped set charges for it consistently: no `AllEnemies` routine
/// costs under 15 Fatigue or comes off cooldown in under 4 rounds. That
/// ladder is what every Everyone-tier magnitude was derived against, and
/// nothing else gates it — a new file could otherwise undercut a whole
/// family's Group tier while reaching further than it.
#[test]
fn every_everyone_scope_routine_pays_the_everyone_tier_price() {
    use crate::abilities::AbilityTarget;
    let game = Game::new(3306, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .filter(|d| d.target == AbilityTarget::AllEnemies)
    {
        assert!(
            def.fatigue_cost >= 15.0,
            "{:?} reaches the whole field for {} Fatigue; the Everyone tier starts at 15",
            def.id,
            def.fatigue_cost
        );
        assert!(
            def.cooldown >= 4,
            "{:?} reaches the whole field on a {}-round cooldown; the Everyone tier starts at 4",
            def.id,
            def.cooldown
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
    "stealth_protocol",
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
        checked, 9,
        "the shipped chains are Refinery, Winding Node and Assembly Bay, plus the \
         Lathe, Transcriber and Disk Press, plus the Compiler, plus the Armory and \
         Fabricator, which assemble one gear item apiece while staying benches for \
         the rest — if that changes, change this count deliberately rather than \
         letting the check go vacuous"
    );
}

/// A machine runs the assembled item's own `craftable.cost` (see
/// `systems::assembly_recipe`), so a machine whose product named a
/// *different* bench would build something its own owner could not craft by
/// hand — and hand-crafting is the manual fallback for a machine you own,
/// not a way around building one.
///
/// Naming no bench at all is the other legal answer, and the Compiler is why:
/// its product, the ICE Breaker, is one of the three consumable starters a
/// run is expected to be able to compile from turn one. Automating a starter
/// must not retroactively gate it. What stops that permission spreading is
/// `only_the_starters_and_scavenged_gear_need_no_research_or_bench`, which
/// pins the ungated set by name — so a *new* machine whose product went
/// benchless to satisfy this test would fail that one instead.
#[test]
fn no_shipped_assembler_builds_another_benchs_product() {
    let game = Game::new(904, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<ItemDb>();
    for def in game
        .world
        .resource::<crate::structures::StructureDb>()
        .all()
    {
        let Some(assembles) = &def.assembles else {
            continue;
        };
        let recipe = items
            .get(assembles.item.as_str())
            .and_then(|d| d.craftable.as_ref())
            .unwrap_or_else(|| panic!("{} assembles an item with no recipe", def.id));
        if let Some(other) = recipe.requires_structure.as_deref()
            && other != def.id.as_str()
        {
            panic!(
                "{} builds {:?}, whose recipe points at {other}'s bench",
                def.id, assembles.item
            );
        }
    }
}
