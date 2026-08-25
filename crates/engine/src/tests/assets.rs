//! Startup validation of the shipped and modded asset directories.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::tuning::{AFFINITY_MAX, AFFINITY_MIN, AFFINITY_NEUTRAL};
use crate::*;

/// `build_radius_bonus` and `claims_ground` are gone from `StructureDef` —
/// deleted along with the Heaps that were their only shipped readers (Task
/// 6) and with `resources::Platform`, the last engine-side reader (this
/// task). Neither field's removal would fail a normal `ron::from_str` on a
/// file that still declares one: an unknown key is silently ignored rather
/// than refused, so a stray `build_radius_bonus: 4` left in a `.ron` would
/// parse clean and simply do nothing forever. A text census over the raw
/// shipped files, not a parsed-struct assertion, is the only thing that can
/// catch that.
#[test]
fn no_shipped_structure_declares_a_retired_platform_field() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/structures");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        checked += 1;
        for retired in ["build_radius_bonus", "claims_ground"] {
            assert!(
                !text.contains(retired),
                "{} still declares `{retired}`, a field deleted with resources::Platform — \
                 serde would silently ignore it rather than refuse the file",
                path.display()
            );
        }
    }
    assert!(
        checked > 0,
        "the census must actually walk assets/structures, or this passes vacuously"
    );
}

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

/// The moddability guarantee for the two new grid fields: a structure file
/// written before they existed — the common case, since every shipped file
/// predates them too — must still parse, reading both as `0` (draws
/// nothing, supplies nothing) rather than failing to load.
#[test]
fn a_structure_file_without_the_power_fields_still_parses() {
    const NO_POWER_FIELDS_STRUCTURE: &str = r#"(
        id: "no_power_fields_structure",
        name: "No Power Fields Structure",
        glyph: '?',
        color: White,
        build_cost: [],
        work: None,
    )"#;
    let dir = assets_dir_with_extra_structure(
        "no_power_fields_structure",
        "no_power_fields_structure.ron",
        NO_POWER_FIELDS_STRUCTURE,
    );
    let game = Game::new(905, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let db = game.world.resource::<crate::structures::StructureDb>();
    let def = db
        .get("no_power_fields_structure")
        .expect("the fixture structure loaded");

    assert_eq!(def.power_draw, 0, "an old-format file should draw nothing");
    assert_eq!(
        def.power_supply, 0,
        "an old-format file should supply nothing"
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
        "the fixture companion must be affinity-neutral, or every ability cast in \
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
        checked, 11,
        "expected the eleven shipped assemblers; one that lost its recipe would drop out of this scan unnoticed"
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
        checked, 5,
        "expected the five producing structures; the press ceiling has to cover every one"
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

/// The twenty-eight hunt-only routines are reachable exactly one way: off a wild
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
    assert_eq!(wild_only.len(), 28, "twenty-eight routines are hunt-only");

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
/// means something for an ability that can enter a battle — but there it is
/// now the *whole* price of a Special, on both sides. A battle ability with a
/// cooldown of 0 is completely unthrottled: a hostile carrier fires it every
/// single round, and the party side pays nothing else for it either.
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
        if def.effect.field_only() {
            assert_eq!(
                def.cooldown, 0,
                "ability {:?} is field-only, so cooldown should be left at its default",
                def.id
            );
            continue;
        }
        assert!(
            def.cooldown >= 1,
            "ability {:?} has no cooldown, so nothing throttles it at all — a wild \
             carrier would fire it every round and the party could spam it",
            def.id
        );
    }
}

/// Every routine that moves Integrity rolls a band rather than a fixed
/// figure. The mechanism (`AbilityEffect::spread`, `battle::DamageRange`)
/// defaults to a degenerate band so a mod's file keeps parsing untouched —
/// which is exactly why the *shipped* roster needs a census: all 34 of these
/// authored no spread for as long as the field existed, and a new one would
/// ship deterministic without anything failing.
///
/// The three variants are named rather than matched with a `_ =>` arm, on
/// `render/stack.rs::cell_mark`'s rule: an eleventh Integrity-moving effect
/// should fail to compile here rather than skip the check.
#[test]
fn every_shipped_integrity_routine_rolls_a_band() {
    use crate::abilities::AbilityEffect as E;
    let game = Game::new(3303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        let spread = match &def.effect {
            E::Damage { spread, .. } | E::Heal { spread, .. } | E::Drain { spread, .. } => *spread,
            E::Buff { .. }
            | E::Debuff { .. }
            | E::Cleanse
            | E::Decompile
            | E::FieldBuff { .. }
            | E::Phase
            | E::Jump => continue,
        };
        checked += 1;
        assert!(
            spread > 0,
            "ability {:?} deals a fixed figure every time — author a `spread` so it \
             rolls a band, the way every species move already does",
            def.id
        );
    }
    assert!(
        checked > 0,
        "the census read no Integrity-moving abilities at all, so it proves nothing"
    );
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

/// How far up the scope ladder a target reaches. The two sides share the
/// ladder rather than having one each: one recipient, one group, the field
/// — an ally-facing family simply has nowhere to go above rung 1, since
/// `WholeParty` already *is* everyone on your side.
fn scope_rank(target: crate::abilities::AbilityTarget) -> usize {
    use crate::abilities::AbilityTarget::*;
    match target {
        OneAlly | OneEnemyGroupFront => 0,
        WholeParty | WholeEnemyGroup => 1,
        AllEnemies => 2,
    }
}

/// A family occupies a contiguous run of scopes starting at Single. A hole
/// is invisible in a directory listing — the files are named for flavour,
/// so nothing about `bus_fault` sitting in `assets/abilities/` says it is
/// Pipeline Stall reaching the whole field with nothing between it and one
/// target, which is exactly how that family shipped. What the player meets
/// is an effect that exists only at a scale they can't always afford, or a
/// prize with no ladder leading up to it.
///
/// Field routines are excluded rather than exempted by accident: they are
/// cast from the map, never appear in the battle picker, and each is its
/// own thing — Deep Scan has no Single tier because a scope ladder is not
/// what that half of the set is organised by.
///
/// **Exclusive routines are excluded for the opposite reason.** The ladder
/// exists so that meeting a family at its widest rung means a cheaper rung
/// is also reachable; an exclusive routine has no cheaper rung anywhere by
/// design, because there is no path to it but a boss or a trader. Building
/// three rungs of Kernel Shear so it could sit in this list would triple the
/// exclusive pool for a reason no player would ever see.
#[test]
fn every_battle_ability_family_is_contiguous_from_single_upward() {
    let game = Game::new(3305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut scopes: std::collections::BTreeMap<String, std::collections::BTreeSet<usize>> =
        std::collections::BTreeMap::new();
    for def in game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .filter(|d| !d.effect.field_only() && !d.exclusive)
    {
        scopes
            .entry(family(def))
            .or_default()
            .insert(scope_rank(def.target));
    }
    for (fam, ranks) in &scopes {
        let expected: std::collections::BTreeSet<usize> = (0..ranks.len()).collect();
        assert_eq!(
            ranks, &expected,
            "{fam:?} occupies scopes {ranks:?}; a family runs from Single upward with no gaps"
        );
    }
}

/// `AbilityDef::validate` refuses a non-finite cost at load, and that check
/// has to survive the rename off `fatigue_cost` rather than be lost with it.
/// Asserted over the real files, since a negative cost would *pay* the caster
/// for casting and nothing downstream defends against one.
#[test]
fn every_shipped_power_cost_is_finite_and_non_negative() {
    let game = Game::new(3307, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        assert!(
            def.power_cost.is_finite() && def.power_cost >= 0.0,
            "{:?} authors power_cost {}",
            def.id,
            def.power_cost
        );
    }
}

/// The 2026-08-17 flip was a key rename, not an authoring pass: the numbers
/// were already in the files, priced back when `fatigue_cost` meant exactly
/// what `power_cost` means now. Three abilities from three different bands
/// pin that nothing moved in translation.
#[test]
fn the_flip_to_power_cost_carried_the_authored_numbers_over_unchanged() {
    let game = Game::new(3308, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();
    for (id, expected) in [
        // Taming stays free — it already spends an ICE Breaker.
        ("decompile", 0.0),
        ("wild_jump", 20.0),
        ("trickle_charge", 25.0),
    ] {
        let def = db.get(id).unwrap_or_else(|| panic!("{id} ships"));
        assert_eq!(def.power_cost, expected, "{id}");
    }
    // The five uncosted files inherit the 0.0 default and keep behaving
    // exactly as they did. `priority_boost` matters most: it is the fallback
    // every companion has when its species grants nothing, and a companion
    // whose only routine is unaffordable has nothing to pick but an attack.
    assert_eq!(db.get("priority_boost").unwrap().power_cost, 0.0);
}

/// Reaching every hostile on the field is the top of the scope ladder, and
/// the shipped set charges for it consistently: no `AllEnemies` routine
/// costs under 15 Power or comes off cooldown in under 4 rounds. That ladder
/// is what every Everyone-tier magnitude was derived against, and nothing
/// else gates it — a new file could otherwise undercut a whole family's
/// Group tier while reaching further than it.
///
/// The cost half is skipped for **passives**, and only for them: `power_cost`
/// is what running a routine costs its caster, and a passive is never run —
/// it fires on its trigger and takes no turn. The cooldown half still applies
/// to everything, passive or not, because a cooldown is what bounds how often
/// the effect lands, and that question does not care who asked for it.
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
            def.is_passive() || def.power_cost >= 15.0,
            "{:?} reaches the whole field for {} Power; the Everyone tier starts at 15",
            def.id,
            def.power_cost
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
    base_mitigation: 2,
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

/// The ten field routines this file ships, at least one per `FieldBuffKind`
/// variant — `Def` ships two, one per scope.
/// Mirrors the table in `assets/abilities/README.md`'s `FieldBuff` section.
const FIELD_ROUTINE_IDS: &[&str] = &[
    "repair_loop",
    "trickle_charge",
    "hardened_shell",
    "hardened_shell_party",
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

/// The `Def` field routine ships at both scopes, and the Party one is priced
/// *between* one Single and covering the whole party a Single at a time.
///
/// Both bounds are the reason the wide one exists. Under the cheaper bound it
/// is strictly better than the Single at every party size, which makes the
/// Single dead content the moment the research lands. Over the dearer one
/// nobody would ever run it: casting on each body in turn would cost less
/// Power *and* leave the same buffs standing, and the only thing the wide cast
/// would still buy is the turns — which are free.
///
/// Asserted as a relationship rather than as the three authored numbers, so a
/// Power retune moves them freely and only an inversion fails. The party size
/// comes from `Game::pet_capacity` (+1 for the player) rather than
/// `BASE_PET_CAPACITY`, because a deployed `pet_slot_bonus` widens the party
/// the cast has to beat.
#[test]
fn the_def_field_routine_ships_both_scopes_and_prices_the_wide_one_between() {
    use crate::abilities::{AbilityEffect, AbilityTarget};
    let game = Game::new(3309, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();

    let single = db.get("hardened_shell").expect("the Single ships");
    let party = db.get("hardened_shell_party").expect("the Party one ships");

    assert_eq!(single.target, AbilityTarget::OneAlly);
    assert_eq!(party.target, AbilityTarget::WholeParty);
    for def in [single, party] {
        assert!(
            matches!(
                def.effect,
                AbilityEffect::FieldBuff {
                    kind: crate::components::FieldBuffKind::Mitigation,
                    duration: 0,
                    ..
                }
            ),
            "{:?} should harden until rest, got {:?}",
            def.id,
            def.effect
        );
    }

    let bodies = (game.pet_capacity() + 1) as f32;
    assert!(
        party.power_cost > single.power_cost,
        "the Party cast ({}) must cost more than one Single ({})",
        party.power_cost,
        single.power_cost
    );
    assert!(
        party.power_cost < bodies * single.power_cost,
        "the Party cast ({}) must undercut {bodies} Singles ({})",
        party.power_cost,
        bodies * single.power_cost
    );
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
    let mut trickle = false;
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
                FieldBuffKind::Trickle => trickle = true,
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
    assert!(trickle, "no shipped ability grants a Trickle field buff");
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
/// companion's kit comes from; an exclusive routine's `boss_drop` is what a
/// boss leaves behind. Any of the three counts as reachable.
///
/// A boss drop counts as a door but an empty `boss_drop` does not: an
/// exclusive routine naming no species at all would still be listed on a
/// Stack trader's rare shelf, but the shelf draws from the whole exclusive
/// pool and a routine reachable *only* by a market roll is one most runs
/// never see. Requiring a boss keeps the fighting path open to every one of
/// them, which is what `progression is earned by fighting` means here.
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
        .filter(|def| def.effect.field_only())
        .filter(|def| {
            def.boss_drop
                .as_ref()
                .is_none_or(|sources| sources.is_empty())
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
        checked, 11,
        "the shipped chains are Refinery, Winding Node and Assembly Bay, plus the \
         Lathe, Transcriber and Disk Press, plus the Compiler, plus the Armory and \
         Fabricator, which assemble one gear item apiece while staying benches for \
         the rest, plus the Annealing Node and Refactor Bench — if that changes, \
         change this count deliberately rather than letting the check go vacuous"
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

// ---------------------------------------------------------------------------
// The power grid census — Task 4. `game::base::power::ledger` only ever
// reads what these three tests check the shipped set for: see
// `docs/superpowers/specs/2026-08-17-base-power-grid-design.md`, "Which
// structures draw, and the numbers".
// ---------------------------------------------------------------------------

/// The gate that stops the sixteenth machine shipping free: every shipped
/// structure that `runs_a_job()` — an extractor or an assembler, the one
/// predicate `ledger` and everything else in the base agree on — must
/// declare a non-zero `power_draw`, or it never enters the sum and runs for
/// free forever.
///
/// Filters on `runs_a_job()` itself rather than re-deriving `work.is_some()
/// || assembles.is_some()`, which is exactly the drift `runs_a_job`'s own
/// doc comment warns about.
#[test]
fn every_shipped_machine_declares_a_power_draw() {
    let game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let mut checked = 0;
    for def in db.all() {
        if !def.runs_a_job() {
            continue;
        }
        assert!(
            def.power_draw > 0,
            "{} runs a job but declares power_draw: 0 — it would run for \
             free off the grid",
            def.id
        );
        checked += 1;
    }
    assert_eq!(
        checked, 16,
        "the plan's table named 15 machines and the Cache Tap is the \
         sixteenth; if that count changed, change this deliberately rather \
         than letting the check go vacuous"
    );
}

/// Home alone has to open a base: a fresh base with nothing else standing
/// must be able to run its opening extractors — a Mining Node, a Log
/// Scraper and a Research Node — before a single Recharger is built.
/// Stated as a concrete sum because "covers the opening" is not something a
/// test can check any other way.
#[test]
fn home_alone_powers_a_new_bases_opening_extractors() {
    let game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let home = db.get("home").expect("home ships");
    let mining_node = db.get("mining_node").expect("mining_node ships");
    let log_scraper = db.get("log_scraper").expect("log_scraper ships");
    let research_node = db.get("research_node").expect("research_node ships");

    let opening_draw = mining_node.power_draw + log_scraper.power_draw + research_node.power_draw;
    assert!(
        home.power_supply >= opening_draw,
        "Home supplies {}, but a Mining Node, a Log Scraper and a Research \
         Node together draw {opening_draw} — a fresh base would open dark",
        home.power_supply
    );
}

/// A structure that both draws from the grid and supplies it is incoherent
/// — the two would net against each other inside a single building rather
/// than being separate roles on the base. Cheap enough to enforce outright
/// rather than merely report.
#[test]
fn no_shipped_structure_both_draws_and_supplies() {
    let game = Game::new(952, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    for def in db.all() {
        assert!(
            def.power_draw == 0 || def.power_supply == 0,
            "{} both draws {} and supplies {} — a structure must not do both",
            def.id,
            def.power_draw,
            def.power_supply
        );
    }
}

/// The materials a breach unlocks, in the order the zones hand them over.
/// A list here rather than a derivation, because "this item is what zone N
/// pays you" is a content decision and there is nothing in `ItemDef` that
/// could be read to recover it — see the two censuses below, which are what
/// stop the decision being reverted one `.ron` file at a time.
const ZONE_MATERIALS: &[&str] = &["cache_grain"];

/// A breach has to change *what* you can make, not only how fast. Every
/// gear recipe a zone-gated research node hands over is denominated in the
/// material of the zone that gates it, so the Cache Tap is on the path to
/// the gear rather than beside it.
///
/// Asserted over the recipes rather than the item files because that is
/// where the zone gate already lives: `ResearchDef::min_zone` and
/// `unlocks_recipes` are one file, so a node that keeps its gate and loses
/// its material is a single-line edit this catches.
#[test]
fn every_zone_gated_gear_recipe_asks_for_a_zone_material() {
    let game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.world.resource::<crate::research::ResearchDb>().all() {
        if def.min_zone < 2 {
            continue;
        }
        for recipe in &def.unlocks_recipes {
            assert!(
                recipe
                    .cost
                    .iter()
                    .any(|(id, _)| ZONE_MATERIALS.contains(&id.as_str())),
                "{} is gated on zone {} but its recipe for {} names no zone material, so breaching buys the blueprint and nothing has to be mined for it",
                def.id.as_str(),
                def.min_zone,
                recipe.result.as_str()
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 6,
        "expected the six zone-gated gear recipes; one that lost its recipe would drop out of this scan unnoticed"
    );
}

/// The upgrade half of the same payoff. `upgrade_ceiling` is already
/// `min(max_tier, zone)`, so tier 2 is unreachable before the second zone —
/// which is what makes naming the zone-2 material in an upgrade cost free of
/// any effect on zone 1, and what makes it the natural place to spend one.
#[test]
fn every_upgrade_path_asks_for_a_zone_material() {
    let game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.structure_defs() {
        let Some(upgrade) = &def.upgrade else {
            continue;
        };
        assert!(
            upgrade
                .cost
                .iter()
                .any(|(id, _)| ZONE_MATERIALS.contains(&id.as_str())),
            "{} upgrades on fragments alone, so every tier past the first is reachable the moment the zone number allows it",
            def.id.as_str()
        );
        checked += 1;
    }
    assert_eq!(
        checked, 8,
        "expected the eight upgradeable structures — the six nodes plus the two compile benches, whose tier is what a compiled copy's quality floor is built out of; one that lost its path would drop out of this scan unnoticed"
    );
}

/// The census over the shipped ambient effects. `EnvironmentDb::load_dir`
/// already refuses a file over either ceiling with a warning, so this is not
/// re-asserting the validator — it is asserting that nothing *shipped* is
/// riding on that refusal, which would be a file the game silently ignores.
#[test]
fn every_shipped_environment_file_loads_and_stays_inside_its_ceiling() {
    use crate::environment::EnvironmentEffect;
    use crate::tuning::{MAX_ENVIRONMENT_ATTRITION, MAX_ENVIRONMENT_DRAG_TICKS};

    let (db, warnings) =
        crate::environment::EnvironmentDb::load_dir(&test_assets_dir().join("environment"))
            .unwrap();
    assert!(
        warnings.is_empty(),
        "shipped environment files should all load: {warnings:?}"
    );
    assert!(db.all().next().is_some(), "the directory shipped empty");
    for def in db.all() {
        match def.effect {
            EnvironmentEffect::Attrition {
                hp_percent,
                min_damage,
            } => {
                assert!(
                    (0.0..=MAX_ENVIRONMENT_ATTRITION).contains(&hp_percent),
                    "{} authors hp_percent {hp_percent}",
                    def.id
                );
                assert!(
                    min_damage >= 0,
                    "{} authors min_damage {min_damage}",
                    def.id
                );
            }
            EnvironmentEffect::Drag { extra_ticks } => assert!(
                extra_ticks <= MAX_ENVIRONMENT_DRAG_TICKS,
                "{} authors extra_ticks {extra_ticks}",
                def.id
            ),
        }
    }
}

/// The base slab is the one safe ground in the game. A shipped file claiming
/// it would be skipped at load anyway, so this is really about the *content*
/// never trying.
#[test]
fn no_shipped_environment_file_claims_the_base_slab() {
    let (db, _) =
        crate::environment::EnvironmentDb::load_dir(&test_assets_dir().join("environment"))
            .unwrap();
    assert!(db.for_biome(world::Biome::Platform).is_none());
}

/// Open Grid is the default ground — the biome the neutral shape sorts most
/// of the map into. Leaving it neutral is what makes "ground that does
/// something" read as an exception rather than a tax on walking, and it is a
/// content decision nothing else in the code holds.
#[test]
fn the_default_ground_stays_neutral() {
    let (db, _) =
        crate::environment::EnvironmentDb::load_dir(&test_assets_dir().join("environment"))
            .unwrap();
    assert!(db.for_biome(world::Biome::OpenGrid).is_none());
}

/// The talent-tree censuses, over the **real** `assets/talents/` rather than a
/// fixture. Nothing in `TalentDb::load_dir` enforces any of this beyond the
/// shape — a mod's tree is never refused for being thin — so these are what
/// hold the shipped content to what the design says it is.
mod talents {
    use super::*;
    use crate::species::AffinityClass;
    use crate::talents::{CHOICES_PER_TIER, TalentDb, TalentNode, tiers_per_tree};
    use crate::tuning::MAX_TALENT_STAT_PERCENT;

    fn shipped() -> TalentDb {
        let (db, warnings) = TalentDb::load_dir(&test_assets_dir().join("talents")).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped trees must load clean: {warnings:?}"
        );
        db
    }

    /// Driven by the enum rather than by naming five files, so a sixth class
    /// fails this test the day it is added rather than silently inheriting the
    /// generic tree.
    #[test]
    fn every_class_has_a_tree_and_so_does_the_generic_fallback() {
        let db = shipped();
        for class in AffinityClass::ALL {
            assert!(
                db.get(Some(class)).is_some_and(|t| t.class == Some(class)),
                "{class:?} has no tree of its own"
            );
        }
        assert!(
            db.get(None).is_some_and(|t| t.class.is_none()),
            "a program with no readable class still spends its points somewhere"
        );
    }

    #[test]
    fn every_tree_is_the_full_depth_and_offers_two_choices_a_tier() {
        let db = shipped();
        for tree in db.trees() {
            assert_eq!(
                tree.tiers.len(),
                tiers_per_tree(),
                "{:?}'s tree is not one tier per level a ringed companion earns",
                tree.class
            );
            for (i, tier) in tree.tiers.iter().enumerate() {
                assert_eq!(
                    tier.0.len(),
                    CHOICES_PER_TIER,
                    "{:?} tier {} is not a decision",
                    tree.class,
                    i + 1
                );
            }
        }
    }

    /// Both halves: the id resolves, and it is a **battle** ability.
    /// `AffinityKind` is blind to the distinction — a `FieldBuff(kind: Mitigation)`
    /// reports `Buff` like any other buff while never appearing in the Special
    /// picker, which is the one place a granted routine is spent.
    #[test]
    fn every_ability_node_names_a_battle_routine_that_exists() {
        let db = shipped();
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
        for choice in db.all_nodes() {
            let TalentNode::Ability { id } = &choice.node else {
                continue;
            };
            let def = abilities
                .get(id)
                .unwrap_or_else(|| panic!("talent {} names no such ability {id:?}", choice.id));
            assert!(
                !def.effect.field_only(),
                "talent {} grants {id:?}, which never appears in the Special picker",
                choice.id
            );
        }
    }

    /// A developed companion already carries four multiplicative axes, and
    /// options compound far less dangerously than numbers — so a `Stat` node's
    /// percentage is bounded, and the trees are weighted away from them.
    #[test]
    fn no_stat_node_is_worth_more_than_its_ceiling() {
        let db = shipped();
        for choice in db.all_nodes() {
            if let TalentNode::Stat { percent, .. } = choice.node {
                assert!(
                    percent > 0.0 && percent <= MAX_TALENT_STAT_PERCENT,
                    "talent {} raises a stat by {percent}%, past MAX_TALENT_STAT_PERCENT",
                    choice.id
                );
            }
        }
    }

    /// `Game::take_talent` resolves a node by id against the whole tree, so two
    /// nodes sharing one id would make which of them a player bought depend on
    /// tier order.
    #[test]
    fn talent_ids_are_unique_across_every_tree() {
        let db = shipped();
        let mut seen = std::collections::HashSet::new();
        for choice in db.all_nodes() {
            assert!(
                seen.insert(choice.id.clone()),
                "{} appears in more than one tree",
                choice.id
            );
        }
    }

    /// The weighting rule, asserted rather than left to authorial memory: a
    /// tree of nothing but percentages is four more multiplicative axes on a
    /// companion that already has four.
    #[test]
    fn every_tree_spends_most_of_itself_on_options_rather_than_numbers() {
        let db = shipped();
        for tree in db.trees() {
            let nodes: Vec<_> = tree.tiers.iter().flat_map(|t| t.0.iter()).collect();
            let stats = nodes
                .iter()
                .filter(|c| matches!(c.node, TalentNode::Stat { .. }))
                .count();
            assert!(
                stats * 2 <= nodes.len(),
                "{:?}'s tree is {stats} stat nodes of {} — options compound less \
                 dangerously than numbers",
                tree.class,
                nodes.len()
            );
        }
    }
}

/// Every shipped species authors a mitigation percentage inside the band the
/// cap allows. A species at or past `MAX_MITIGATION_PERCENT` is immune
/// before gear or a buff is counted, which the cap would silently swallow.
#[test]
fn every_species_mitigation_leaves_room_under_the_cap() {
    let game = Game::new(3311, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for species in game.species_defs() {
        assert!(
            (0..crate::tuning::MAX_MITIGATION_PERCENT).contains(&species.base_mitigation),
            "{} authors base_mitigation {}, outside 0..{}",
            species.id,
            species.base_mitigation,
            crate::tuning::MAX_MITIGATION_PERCENT
        );
    }
}

/// Every shipped move authors a range that can actually vary, and none can
/// roll negative. A roster of degenerate ranges would ship the feature dark.
#[test]
fn every_shipped_move_authors_a_real_damage_range() {
    let game = Game::new(3312, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for species in game.species_defs() {
        for mv in &species.moves {
            let range = mv.range();
            assert!(
                range.min >= 0,
                "{} / {} can roll negative",
                species.id,
                mv.name
            );
            assert!(
                range.max > range.min,
                "{} / {} is a degenerate range — a shipped move must vary",
                species.id,
                mv.name
            );
        }
    }
}

/// The two defensive axes must be a real choice rather than one stat with a
/// second name: some shipped armour has to buy evasion instead of
/// mitigation, and some shipped weapons accuracy instead of damage. A field
/// nothing authors is an unused feature flag.
#[test]
fn the_shipped_gear_actually_authors_both_defensive_and_both_offensive_axes() {
    let game = Game::new(3313, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let equipment: Vec<_> = game
        .item_defs()
        .into_iter()
        .filter_map(|i| i.equipment)
        .collect();
    assert!(
        equipment.iter().any(|(_, stats)| stats.evasion > 0),
        "no shipped armour buys evasion"
    );
    assert!(
        equipment.iter().any(|(_, stats)| stats.mitigation > 0),
        "no shipped armour buys mitigation"
    );
    assert!(
        equipment.iter().any(|(_, stats)| stats.accuracy > 0),
        "no shipped weapon buys accuracy"
    );
    assert!(
        equipment
            .iter()
            .any(|(_, stats)| stats.damage != crate::battle::DamageRange::default()),
        "no shipped weapon authors a damage range"
    );
}

/// Every shipped weapon carries a damage range, and nothing else does. A
/// weapon **overrides** a natural attack rather than adding to it, so a
/// weapon with no range would silently disarm whoever equipped it.
#[test]
fn every_weapon_authors_a_range_and_nothing_else_does() {
    let game = Game::new(3314, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for item in game.item_defs() {
        let Some((slot, stats)) = item.equipment.as_ref() else {
            continue;
        };
        let has_range = stats.damage != crate::battle::DamageRange::default();
        assert_eq!(
            has_range,
            *slot == EquipmentSlot::Weapon,
            "{} is a {slot:?} and {} a damage range",
            item.id,
            if has_range { "has" } else { "lacks" }
        );
    }
}

/// The shipped manual, `assets/help/`. Every page parses and every link
/// resolves — `HelpDb::load_dir` skips a malformed page with a warning
/// rather than refusing to start, which is right for a modder's directory
/// and would otherwise let a shipped page go missing in silence.
#[test]
fn every_shipped_help_page_parses_and_every_link_resolves() {
    let (db, warnings) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    assert!(warnings.is_empty(), "{warnings:#?}");
    assert!(
        db.pages().len() >= 4,
        "the census must actually walk assets/help, or it passes vacuously"
    );
    assert!(
        db.pages().iter().any(|p| !p.links.is_empty()),
        "no shipped page cross-links another, so the further-reading list is \
         exercised by nothing but fixtures"
    );
}

/// A further-reading row is followed by typing its label's shortcut, and
/// `App::menu_shortcut` runs out of digits at nine.
#[test]
fn no_shipped_help_page_carries_more_than_nine_links() {
    let (db, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    for page in db.pages() {
        assert!(
            page.links.len() <= 9,
            "{} carries {} links; a tenth has no shortcut to type",
            page.id,
            page.links.len()
        );
    }
}

fn help_assets_dir() -> std::path::PathBuf {
    test_assets_dir().join("help")
}

/// The easter-egg census over the manual. It used to read `HELP_ROWS`, a
/// const in `crates/gui/src/render/meta.rs`; the manual is authored content
/// now, so it reads `assets/help/` and protects against the **user** editing
/// a page as well as against a developer editing a const.
///
/// Asserted on whitespace-delimited **tokens**, never on substrings.
/// `key name` is the binding idiom these pages use, so a token match catches
/// a real documentation of a key while staying satisfiable: the prose is
/// full of these letters inside ordinary words, and of the lowercase `t`
/// that legitimately binds trade.
///
/// Over parsed pages rather than raw files, so the directory's own
/// `README.md` — which names all three keys in the rule that forbids them —
/// is not itself a violation.
#[test]
fn no_shipped_help_page_names_a_hidden_key() {
    let (db, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    let mut rows = 0;
    for page in db.pages() {
        for row in help::page_rows(page, help::WRAP_COLUMNS) {
            rows += 1;
            for token in row.split_whitespace() {
                assert!(
                    !matches!(token, "W" | "T" | "Z"),
                    "help page {} names a hidden key: {row:?} — see \
                     crates/engine/EASTER_EGGS.md",
                    page.id
                );
            }
        }
    }
    assert!(rows > 0, "the census must actually walk the manual");
}

/// The Excavation plan is the one verb in the game with no engine-side
/// refusal to teach it: `m` opens a mode, and a mode nobody can find is a
/// feature nobody has. So the manual is where it is learned.
#[test]
fn the_manual_binds_the_excavation_plan_key() {
    let (db, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    // The key and the verb in one substring, deliberately: `"m "` alone is
    // already inside "collect from adjacent structures", so asserting on it
    // separately passes with every Excavation row deleted.
    let says = db.pages().iter().any(|page| {
        help::page_rows(page, help::WRAP_COLUMNS)
            .iter()
            .any(|row| row.contains("m — Excavation plan"))
    });
    assert!(says, "no help page binds the m key to the Excavation plan");
}

/// Two piles cannot share a tag on the base stock strip. The strip is one
/// row of `[TAG] qty` pairs and carries nothing else — no name, no colour
/// distinction — so a duplicate tag is a readout that lies about which pile
/// is filling, and it fails silently: both rows draw, both look right.
///
/// `ItemDef::tag` derives from the name, so a collision is a content
/// accident rather than a code fault, which is why this is a census over the
/// shipped assets rather than an assertion inside `load_dir`. A mod's own
/// collision is its author's to settle with `abbrev`; nothing here refuses
/// their file.
///
/// Restricted to what the strip actually lists — `Material` and `Currency`,
/// the two categories `Game::base_stock` keeps. Etched disks are excluded
/// through `ItemId::etched_ability`, the existing derivation: every disk
/// derives the same family tag by construction, and none of them can reach
/// a `Stock` in the first place.
#[test]
fn no_two_shipped_stock_items_share_a_tag() {
    let game = Game::new(921, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::items_db::ItemDb>();

    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut checked = 0;
    for def in db.all() {
        if def.id.etched_ability().is_some() {
            continue;
        }
        if !matches!(
            def.category(),
            ItemCategory::Material | ItemCategory::Currency
        ) {
            continue;
        }
        checked += 1;
        let tag = def.tag();
        assert!(
            !tag.is_empty(),
            "{} derives an empty stock tag from its name",
            def.id.as_str()
        );
        if let Some(other) = seen.insert(tag.clone(), def.name.clone()) {
            panic!(
                "\"{other}\" and \"{}\" both tag as [{tag}] on the stock strip — \
                 settle it with an `abbrev` on one of them",
                def.name
            );
        }
    }
    assert!(
        checked > 10,
        "the census walked {checked} stock items, which is too few to be reading \
         the shipped set at all"
    );
}

/// Every memory kind the engine actually writes, against the
/// `MemorySubjectKind` the trigger that writes it hands in.
///
/// The table is spelled out here rather than derived, because there is
/// nothing to derive it from: the catalogue is data and the triggers are
/// Rust, which is the seam `perks::Perk` sits on, and `MemoryDef` carries no
/// `trigger` field on purpose. Keeping the pairing in one census is what
/// stops a shipped def drifting into content nothing can ever write.
const MEMORY_TRIGGERS: &[(&str, crate::memories::MemorySubjectKind)] = {
    use crate::memories::MemorySubjectKind as K;
    &[
        // `Game::note_maul`, off `resolve_and_apply_attack`.
        ("mauled_by", K::Species),
        // `Game::form_victory_memories`, off `end_battle`.
        ("bonded_in_battle", K::Program),
        ("hard_won", K::Nothing),
        // `Game::note_strandings`, off `tick_inner`.
        ("stranded_at", K::BaseTile),
        // `Game::damage_structure`, on both branches. The one work memory
        // that is an edge rather than a stretch of service, because a sweep
        // is an event and a posting is a standing state.
        ("swept_here", K::Structure),
        // `Game::note_postings`, off `tick_inner` on a period. Two subjects,
        // because a posting is both a place and a kind of work.
        ("settled_in", K::Structure),
        ("jammed_here", K::Structure),
        ("cutting_rock", K::Activity),
    ]
};

/// A def whose declared `subject` no trigger can satisfy is dead content:
/// every `remember` of it is refused as `WrongSubject`, and nothing else in
/// the build says so. A def with no trigger at all is worse — it can never be
/// written, and reads as a memory the player simply never earns.
///
/// This census walks the *defs* and not the variants, which is what let
/// `MemorySubjectKind::Structure` and `::Activity` ship as variants with no
/// def and no trigger at all — a variant with no writer costs nothing, while
/// an enum that has to grow costs a migration.
#[test]
fn every_shipped_memory_def_is_reachable_from_a_trigger() {
    use crate::memories::MemoryDb;

    let (db, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    assert!(
        db.all().count() > 0,
        "the census must walk a real catalogue"
    );

    for def in db.all() {
        let id = def.id.as_str();
        let (_, writes) = MEMORY_TRIGGERS
            .iter()
            .find(|(name, _)| *name == id)
            .unwrap_or_else(|| {
                panic!(
                    "{id} ships in assets/memories but nothing in the engine                      writes it — add the trigger, or add it to MEMORY_TRIGGERS                      if this census has fallen behind"
                )
            });
        assert_eq!(
            def.subject, *writes,
            "{id} declares a subject its trigger never hands in, so every              write of it is refused"
        );
    }
}

/// The memory-catalogue census, over the **real** `assets/memories/` rather
/// than a fixture. `MemoryDb::load_dir` refuses nothing beyond a file that
/// will not parse — a mod's def is never turned away for being nonsense — so
/// this is what holds the shipped catalogue to what a memory has to be for
/// `Memory::intensity` to mean anything.
#[test]
fn every_shipped_memory_def_is_well_formed() {
    use crate::memories::MemoryDb;

    let dir = test_assets_dir().join("memories");
    let (db, warnings) = MemoryDb::load_dir(&dir).unwrap();
    assert!(
        warnings.is_empty(),
        "the shipped catalogue must load clean: {warnings:?}"
    );

    // Against the file count rather than against a set of ids, because the db
    // is keyed by id: two files claiming one id collapse into a single entry
    // and nothing else in the load would say so.
    let files = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .count();
    assert!(
        files > 0,
        "the census must actually walk assets/memories, or it passes vacuously"
    );
    assert_eq!(
        db.all().count(),
        files,
        "two shipped files claim one memory id, so one of them is unreachable"
    );

    for def in db.all() {
        let id = def.id.as_str();
        assert!(!def.name.is_empty(), "{id} has no name for a screen row");
        assert!(!def.blurb.is_empty(), "{id} has no blurb");
        assert!(
            def.valence.is_finite() && def.valence != 0.0,
            "{id} has a valence of {}, so remembering it would be worth nothing",
            def.valence
        );
        assert!(
            def.half_life > 0,
            "{id} has a half_life of 0, which divides by zero in intensity"
        );
        assert!(
            def.strike_cap >= 1,
            "{id} has a strike_cap of 0, so every strike of it is worth nothing"
        );
    }
}

/// The game has one word for the defensive stat, and it is not `Defense`.
///
/// `Stats::def` became `Stats::mitigation` in the combat model rewrite and
/// the authored prose did not follow it: ten shipped ability descriptions
/// still said `DEF`, and nine of those quoted a magnitude the very same
/// commit had tripled — `bastion` read "+4 DEF" against an authored power
/// of 12 for months. A screen's wording is held by the test that renders
/// it; nothing at all compiles a `.ron` description, so this is the half a
/// rename sweep forgets.
///
/// Scoped to authored player text — `description:` lines in every shipped
/// `.ron`, and the manual's pages, which are prose end to end. A `//`
/// comment naming the retired field is history and is deliberately left
/// alone, as is `raid_defense`, which is a structure's own separate stat
/// and a mod-facing field name besides. So is lower-case "defense": a
/// research node describing "automated perimeter defense" is using the
/// ordinary word and is not making a claim about anybody's stat block.
#[test]
fn no_shipped_description_calls_mitigation_defense() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("the shipped asset tree is readable") {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&test_assets_dir(), &mut files);
    assert!(!files.is_empty(), "the shipped asset tree is not empty");

    // The capitalised word and the bare token only. Lower-case "defense" is
    // ordinary English about a base holding a perimeter — Fortification's
    // description is exactly that — and reads as the stat to nobody.
    let stale = |line: &str| {
        line.contains("Defense")
            || line.match_indices("DEF").any(|(i, _)| {
                let before = line[..i].chars().next_back();
                let after = line[i + 3..].chars().next();
                let boundary =
                    |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
                boundary(before) && boundary(after)
            })
    };
    let mut offenders = Vec::new();
    for path in files {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_help = ext == "md" && path.parent().is_some_and(|p| p.ends_with("help"));
        if ext != "ron" && !is_help {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            let authored = is_help || line.trim_start().starts_with("description:");
            // `raid_defense` is a structure's own stat and a field name, so a
            // line carrying one says nothing about this rename.
            if authored && stale(line) && !line.contains("raid_defense") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "shipped player text still calls mitigation Defense/DEF:\n{}",
        offenders.join("\n")
    );
}
