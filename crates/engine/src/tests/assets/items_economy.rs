use crate::tests::support::*;
use crate::*;

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
                ability.level >= 1 && ability.level <= crate::tuning::TALENT_START_LEVEL,
                "species {:?}: ability {:?} unlocks at level {}, outside 1..={}",
                species.id,
                ability.id,
                ability.level,
                crate::tuning::TALENT_START_LEVEL
            );
            declared += 1;
        }
    }
    assert!(
        declared >= 10,
        "the shipped roster should actually use the ability system, found {declared}"
    );
}

/// `spawn_tamed`'s companion must be blank, and the shipped roster is not
/// the place to find a blank species — it used to be construct, which
/// tethered 233 call sites to whatever the roster's first kitless entry
/// happened to be, and would have handed every fixture construct's
/// affinities the moment the roster gained any.
///
/// The properties asserted are the ones a later change could take away
/// silently. Absence is checked against the asset **files** rather than
/// against `Game::species_defs`, because `load_asset_dbs` puts the fixture
/// into every test-built db by design — that is what makes it survive a
/// `Game::load`.
#[test]
fn the_generic_fixture_species_is_blank_and_unshipped() {
    let shipped = std::fs::read_dir(test_assets_dir().join("species")).unwrap();
    for entry in shipped {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let def: crate::species::SpeciesDef =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_ne!(
            def.id, GENERIC_SPECIES_ID,
            "{GENERIC_SPECIES_ID} must not be a shipped species — the fixture exists \
             precisely so the test companion is not one of the roster's programs"
        );
    }

    let fixture = generic_species();
    assert!(
        fixture.abilities.is_empty(),
        "the fixture companion must declare no abilities, or install_innate_routines \
         stops yielding FALLBACK_ABILITY_ID and every routine-slot fixture shifts"
    );
    assert!(
        fixture.affinities.non_neutral().is_empty(),
        "the fixture companion must be affinity-neutral, or every ability run in \
         every spawn_tamed test is silently multiplied"
    );
    assert!(
        fixture.habitats.is_empty(),
        "the fixture companion must have no habitat: habitat_matches is indexed into \
         by the spawn roll, so a fixture species in a pool moves what a seeded \
         Game::new spawns"
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

/// An upgrade item's magnitudes are `.ron` data, so nothing in Rust stops one
/// shipping at zero effect or with no text saying what it does — and this is
/// the one item class whose whole purpose is a number the player never sees
/// applied twice. A dud reads on screen exactly like a real one.
///
/// The finiteness half is belt-and-braces over `ItemDef::non_finite_field`,
/// which already refuses such a file at load: that guard would make a broken
/// upgrade item *vanish* rather than misbehave, and a silently absent item is
/// the harder failure to trace back to its file.
#[test]
fn every_shipped_upgrade_item_says_what_it_does() {
    let game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.item_defs() {
        let Some(upgrade) = def.upgrade else {
            continue;
        };
        let id = def.id.as_str();
        for (field, pct) in [
            ("hp_percent", upgrade.hp_percent),
            ("atk_percent", upgrade.atk_percent),
            ("def_percent", upgrade.def_percent),
        ] {
            assert!(pct.is_finite(), "{id}: {field} is not a finite number");
            assert!(pct >= 0.0, "{id}: {field} is negative — a downgrade");
        }
        assert!(
            upgrade.spends_a_slot() || upgrade.zone_bump,
            "{id} declares an upgrade that does nothing at all"
        );
        assert!(
            !def.description.is_empty(),
            "{id} upgrades a program without saying so anywhere the player can read"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 7,
        "expected the Recompile Kernel, the three craftable buffs and the three rare \
         drops; an upgrade item that lost its `upgrade` field would drop out of this \
         scan unnoticed"
    );
}

/// A production line is a straight line: raw producer → refiner → bench, each
/// machine pulling from exactly one upstream. That is a property of the
/// *recipes*, not of the machines — an assembling machine runs its product's
/// own `craftable.cost`, so a second ingredient added to any of those items
/// silently turns its machine back into a corner puzzle needing two lines
/// stood up before anything comes out. That shape is what the flatten removed.
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
        let Some(machine) = &def.machine else {
            continue;
        };
        if machine.role != crate::structures::MachineRole::Assemble {
            continue;
        }
        for recipe in &machine.recipes {
            let cost = items
                .iter()
                .find(|i| i.id == recipe.produces)
                .and_then(|i| i.craftable.as_ref())
                .unwrap_or_else(|| panic!("{} lists {} with no recipe", def.id, recipe.produces));
            assert_eq!(
                cost.cost.len(),
                1,
                "{} makes {} out of {} ingredients — a machine on two feeders needs two lines",
                def.id,
                recipe.produces.as_str(),
                cost.cost.len()
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 12,
        "expected the twelve shipped assembling recipes (five factory, five assembler, two printer); one that lost its recipe would drop out of this scan unnoticed"
    );
}

/// `StructureDef::first_free` is authored by exactly one shipped structure,
/// and it is the Broker.
///
/// A `#[serde(default)]` schema field can be documented, read and tested
/// while no shipped file authors it — which is how `AbilityDef::spread`
/// shipped across several releases used by none of 77 ability files. The
/// waiver is only reachable in a real run through this one asset, so the
/// asset is what the census is over.
///
/// The bill it waives is asserted alongside: `first_free` on a structure
/// that costs nothing is a flag with nothing to do, and the free half of
/// `construction::the_first_broker_is_free_and_the_next_one_is_not` would
/// pass against it.
#[test]
fn the_broker_is_the_one_structure_the_first_of_which_is_free() {
    let game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let free: Vec<String> = game
        .structure_defs()
        .into_iter()
        .filter(|def| def.first_free)
        .map(|def| def.id)
        .collect();
    assert_eq!(
        free,
        vec!["contract_broker".to_string()],
        "the Broker is the run's onboarding, and nothing else is handed out"
    );
    let broker = game
        .structure_defs()
        .into_iter()
        .find(|def| def.id == "contract_broker")
        .expect("the shipped assets carry a Contract Broker");
    assert!(
        !broker.build_cost.is_empty(),
        "a waiver over an empty bill waives nothing"
    );
}

/// The other half of that press, and the one the recipe ceiling above can't
/// see: a `work.produces` structure makes its item out of nothing on a
/// timer, so the item's *value* is a Credit-per-tick rate. A flat-payout
/// printer (8 ticks) would out-earn an Extractor nearly fourfold if
/// Breach Kits were priced at their 3-Fragment recipe. Anything a base can
/// print sits at the floor price; worth comes from what you can't print.
#[test]
fn every_base_produced_item_sits_at_the_floor_price() {
    let game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.structure_defs() {
        for production in def.productions().filter(|p| p.extracts) {
            let recipe = production.recipe;
            assert_eq!(
                game.item_value(&recipe.produces),
                tuning::DEFAULT_ITEM_VALUE,
                "{} prints {} every {} ticks, so pricing it above the floor makes the structure a Credit press",
                def.id,
                recipe.produces.as_str(),
                recipe.ticks_per_unit
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 5,
        "expected the Research Node's data and the Extractor's four recipes; the press ceiling has to cover every one"
    );
}

/// Every item an assembling machine lists a recipe for. Derived from the
/// machines, never a list in a test, so a mod that adds a refiner extends
/// the censuses below for free.
fn assembled_items(
    structures: &[crate::structures::StructureDef],
) -> std::collections::HashSet<&str> {
    structures
        .iter()
        .filter_map(|def| def.machine.as_ref())
        .filter(|m| m.role == crate::structures::MachineRole::Assemble)
        .flat_map(|m| m.recipes.iter())
        .map(|recipe| recipe.produces.as_str())
        .collect()
}

/// Standard and premium armour and modules are made *out of* the factory, so
/// the base is the way gear happens rather than an option beside the fragment
/// economy. Fourteen `.ron` files agreeing with each other is not a policy —
/// any one of them reverted to raw Core Fragments would opt that item back out
/// in silence, and nothing else in the suite would notice.
///
/// "Factory-made" is derived from what the assembling machines actually list, never
/// a list here, so a mod that adds a refiner extends this for free. Weapons
/// are deliberately outside it: they stay on the fragment economy, and moving
/// them is a decision to make, not a gap to fill.
#[test]
fn standard_and_premium_gear_is_made_from_intermediates() {
    let game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.structure_defs();
    let factory_made = assembled_items(&structures);

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
        // Fragment-only is the scavenged tier, which is the other test's job.
        let Some(craftable) = def
            .craftable
            .as_ref()
            .filter(|c| c.requires_structure.is_some() && !is_fragment_only(c))
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

/// The scavenged tier is the recipes paid in raw fragments alone.
fn is_fragment_only(c: &crate::items_db::CraftableDef) -> bool {
    c.cost
        .iter()
        .all(|(id, _)| id.as_str() == ids::CORE_FRAGMENT)
}

/// The other side of that policy: scavenged gear is paid in raw fragments
/// alone and needs only a Workbench, never a production line. An
/// intermediate creeping into one of these recipes locks the cheap tier
/// behind a line the player may have just lost.
#[test]
fn scavenged_gear_is_fragment_only() {
    let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.structure_defs();
    let factory_made = assembled_items(&structures);

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
        let Some(craftable) = def.craftable.as_ref().filter(|c| is_fragment_only(c)) else {
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

/// Weapons are designed at the Workbench, never built from a fixed recipe, so an
/// authored weapon is a drop, an enemy's gear or a trader's stock. A
/// `craftable` block on one would be a second way to make a weapon that
/// bypasses `weapon_bench` and the design cap.
#[test]
fn no_authored_weapon_is_craftable() {
    let game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.item_defs() {
        let Some((crate::items::EquipmentSlot::Weapon, _)) = &def.equipment else {
            continue;
        };
        assert!(
            def.craftable.is_none(),
            "{} is an authored weapon with a craftable recipe; weapons are designed, not crafted",
            def.id.as_str()
        );
        checked += 1;
    }
    assert!(
        checked >= 15,
        "expected the fifteen authored weapons, scanned {checked}"
    );
}

/// `cache_drop` is rolled once per cache for every item that declares one,
/// so the expected haul is the *sum* across the item set, not a pick from a
/// list — which means any one item's number is a change to what every cache
/// in the game holds. `assets/items/README.md` tells modders the shipped set
/// "totals about one item per cache" and asks them to keep their own numbers
/// low on that basis; nothing checked it, and dropping the Power Cell's 0.4
/// on 2026-08-04 moved the total from 1.55 to 1.30. Retiring the Access
/// Shard's 0.35 on 2026-08-08, when a sealed door stopped needing a key,
/// moved it again to 0.95 — the band tracks what the shipped set actually
/// pays, and a *deliberate* move recentres it rather than widening it.
#[test]
fn a_stack_cache_holds_about_one_item_on_average() {
    let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let expected: f32 = game
        .item_defs()
        .iter()
        .filter_map(|def| def.cache_drop)
        .sum();
    assert!(
        (0.8..=1.3).contains(&expected),
        "shipped caches now average {expected} items, which is no longer the \
         'about one' the items README promises a modder"
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
