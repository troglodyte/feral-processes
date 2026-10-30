use crate::tests::support::*;
use crate::*;

/// A charge routine winds up a damage hit, and a one-turn wind-up is an
/// ordinary routine with a delay. The loader skips a malformed `charge`, so
/// only a census over the shipped files notices one that quietly vanished.
#[test]
fn every_shipped_charge_routine_is_a_multi_round_damage() {
    let game = Game::new(3304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();
    let charged: Vec<_> = db.all().filter(|d| d.charge.is_some()).collect();
    assert!(
        charged.iter().any(|d| d.id == "haymaker"),
        "haymaker must load as a charge routine"
    );
    for def in charged {
        assert!(
            matches!(def.effect, crate::abilities::AbilityEffect::Damage { .. }),
            "{} charges a non-damage effect",
            def.id
        );
        assert!(def.charge.as_ref().unwrap().rounds >= 2, "{}", def.id);
    }
}

#[test]
fn the_default_behaviour_exists_and_every_shipped_species_names_a_real_profile() {
    let (db, warnings) =
        crate::behaviours::BehaviourDb::load_dir(&test_assets_dir().join("behaviours")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(db.default_profile().id, "default");
    let (abilities, _) =
        crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    let (species, _) = SpeciesDb::load_dir(&test_assets_dir().join("species"), &abilities).unwrap();
    let unknown = db.unknown_species_references(&species);
    assert!(unknown.is_empty(), "{unknown:?}");
}

#[test]
fn every_shipped_behaviour_names_only_known_features() {
    let (_, warnings) =
        crate::behaviours::BehaviourDb::load_dir(&test_assets_dir().join("behaviours")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn shipped_species_name_the_personalities_the_spec_assigns() {
    let (abilities, _) =
        crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    let (species, _) = SpeciesDb::load_dir(&test_assets_dir().join("species"), &abilities).unwrap();
    for (profile, ids) in [
        (
            "brute",
            ["rootkit", "botnet", "worm", "scrapper"].as_slice(),
        ),
        ("hunter", &["cipher", "zero_day", "trojan", "polymorph"]),
        ("skirmisher", &["glitch", "sprite"]),
        ("cautious", &["drone", "construct"]),
    ] {
        for id in ids {
            let named = species.get(id).and_then(|d| d.behaviour.as_deref());
            assert_eq!(named, Some(profile), "{id}");
        }
    }
}

/// `(machine, produces, ticks_per_unit, research, level, tierless)` — the
/// Global Constraints recipe table, restated so a retune of a `.ron` file
/// has to be deliberate.
type MachineRow = (
    &'static str,
    &'static str,
    u32,
    Option<&'static str>,
    Option<u32>,
    bool,
);

const MACHINE_TABLE: &[MachineRow] = &[
    ("extractor", "core_fragment", 10, None, Some(1), false),
    (
        "extractor",
        "raw_trace",
        10,
        Some("routine_fabrication"),
        Some(1),
        false,
    ),
    (
        "extractor",
        "cache_grain",
        14,
        Some("cache_coherence"),
        Some(1),
        false,
    ),
    ("extractor", "power_cell", 6, Some("power_grid"), None, true),
    (
        "factory",
        "blank_substrate",
        12,
        Some("routine_fabrication"),
        None,
        false,
    ),
    ("factory", "bytecode_block", 12, None, None, false),
    (
        "factory",
        "annealed_core",
        12,
        Some("program_refactoring"),
        None,
        false,
    ),
    (
        "factory",
        "logic_wafer",
        12,
        Some("routine_fabrication"),
        None,
        false,
    ),
    ("factory", "charge_coil", 12, None, None, false),
    (
        "assembler",
        "routine_disk",
        20,
        Some("routine_fabrication"),
        None,
        false,
    ),
    ("assembler", "honeypot", 20, Some("deception"), None, false),
    (
        "assembler",
        "recompile_kernel",
        20,
        Some("program_refactoring"),
        None,
        false,
    ),
    (
        "assembler",
        "hardened_shell",
        30,
        Some("armor_bench"),
        None,
        false,
    ),
    (
        "assembler",
        "trace_sniffer",
        30,
        Some("weapon_bench"),
        None,
        false,
    ),
    ("printer", "ice_breaker", 8, Some("automation"), None, false),
    ("printer", "patch_routine", 20, None, None, false),
];

fn shipped_game() -> Game {
    Game::new(21002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

#[test]
fn the_four_machines_carry_the_recipe_table() {
    let game = shipped_game();
    let db = game.world.resource::<StructureDb>();
    for kind in ["extractor", "factory", "assembler", "printer"] {
        let machine = db.get(kind).unwrap().machine.as_ref().unwrap();
        let expected = MACHINE_TABLE.iter().filter(|r| r.0 == kind).count();
        assert_eq!(
            machine.recipes.len(),
            expected,
            "{kind} lists exactly its table rows"
        );
    }
    for &(kind, produces, ticks, research, level, tierless) in MACHINE_TABLE {
        let recipe = db
            .get(kind)
            .unwrap()
            .machine_recipe(&ItemId::from(produces))
            .unwrap_or_else(|| panic!("{kind} does not list {produces}"));
        assert_eq!(recipe.ticks_per_unit, ticks, "{kind} {produces}");
        assert_eq!(recipe.research.as_deref(), research, "{kind} {produces}");
        assert_eq!(recipe.level, level, "{kind} {produces}");
        assert_eq!(recipe.tierless, tierless, "{kind} {produces}");
    }
}

#[test]
fn every_machine_recipe_research_names_a_shipped_node() {
    let game = shipped_game();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let mut checked = 0;
    for def in game.world.resource::<StructureDb>().all() {
        for production in def.productions().filter(|_| def.machine.is_some()) {
            if let Some(id) = &production.recipe.research {
                checked += 1;
                assert!(
                    research.get(id).is_some(),
                    "{} gates {} behind unknown research {id}",
                    def.id,
                    production.recipe.produces
                );
            }
        }
    }
    assert!(checked > 0, "the census must find gated recipes");
}

#[test]
fn the_machines_cost_and_draw_what_the_spec_says() {
    let game = shipped_game();
    let db = game.world.resource::<StructureDb>();
    // (kind, build cost, power draw, capacity)
    for (kind, cost, draw, capacity) in [
        ("extractor", 6, 1, 20),
        ("factory", 16, 2, 20),
        ("assembler", 18, 3, 15),
        ("printer", 16, 2, 15),
    ] {
        let def = db.get(kind).unwrap();
        assert_eq!(
            def.build_cost,
            vec![(ItemId::from("core_fragment"), cost)],
            "{kind}"
        );
        assert_eq!(def.power_draw, draw, "{kind}");
        assert_eq!(def.capacity, capacity, "{kind}");
        assert_eq!(def.room_tags, vec!["workshop".to_string()], "{kind}");
    }
    let up = db.get("extractor").unwrap().upgrade.as_ref().unwrap();
    assert_eq!(up.max_tier, 5);
    assert_eq!(
        up.cost,
        vec![
            (ItemId::from("core_fragment"), 10),
            (ItemId::from("cache_grain"), 1)
        ]
    );
    for kind in ["factory", "assembler", "printer"] {
        assert!(db.get(kind).unwrap().upgrade.is_none(), "{kind}");
    }
}

#[test]
fn every_configure_objective_names_an_item_some_machine_can_make() {
    let assets = test_assets_dir();
    let (contracts, _) = crate::contracts::ContractDb::load_dir(&assets.join("contracts")).unwrap();
    let (structures, _) =
        crate::structures::StructureDb::load_dir(&assets.join("structures")).unwrap();
    let mut seen = 0;
    for def in contracts.iter() {
        if let crate::contracts::Objective::Configure { item } = &def.objective {
            seen += 1;
            assert!(
                structures.all().any(|s| s.machine_recipe(item).is_some()),
                "{} asks for a machine set to make {item}, which no machine lists",
                def.id
            );
        }
    }
    assert!(seen > 0, "the census must find a shipped Configure job");
}

#[test]
fn the_shipped_power_cell_crop_produces_power_cells() {
    let game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let crops = game.crop_defs();
    let cell = crops
        .iter()
        .find(|c| c.id.as_str() == "power_cell")
        .expect("the shipped power_cell crop loads");
    assert_eq!(cell.produces.0, "power_cell");
    assert_eq!(cell.stages, 4);
    assert_eq!(cell.grow_ticks, 1200);
}

const FLOOR_GLYPHS: [char; 8] = ['.', '*', ',', '"', '\'', '`', '~', ':'];

#[test]
fn every_shipped_item_authors_a_floor_glyph_from_the_generic_set() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // item_defs includes the synthesised disks and carriers.
    for def in game.item_defs() {
        let g = def
            .glyph
            .unwrap_or_else(|| panic!("{} authors no glyph", def.id));
        assert!(FLOOR_GLYPHS.contains(&g), "{} glyph {g:?}", def.id);
    }
}

#[test]
fn an_item_without_a_glyph_floors_as_the_default() {
    let def: ItemDef = ron::from_str(r#"(id: "x", name: "X")"#).unwrap();
    assert_eq!(def.floor_glyph(), crate::items_db::DEFAULT_FLOOR_GLYPH);
}

fn shipped_items_and_species() -> (crate::items_db::ItemDb, SpeciesDb) {
    let assets = test_assets_dir();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
    let (species, _) = SpeciesDb::load_dir(&assets.join("species"), &abilities).unwrap();
    (items, species)
}

/// A typed weapon or move with no armour against its type is an attack the
/// player cannot answer, which defeats the point of a type (spec, § Stage 2).
/// Add the counter-armour in the same change as the first weapon or move of
/// a new type.
#[test]
fn every_damage_type_a_shipped_attack_carries_has_armour_resisting_it() {
    use crate::damage::DamageType;
    let (items, species) = shipped_items_and_species();

    let mut carried: Vec<(DamageType, String)> = Vec::new();
    for def in items.all() {
        if let Some((EquipmentSlot::Weapon, stats)) = def.worn_stats()
            && let Some(ty) = stats.damage_type
        {
            carried.push((ty, format!("weapon {}", def.id)));
        }
    }
    for def in species.all() {
        for mv in &def.moves {
            if let Some(ty) = mv.damage_type {
                carried.push((ty, format!("{} {}", def.id, mv.name)));
            }
        }
    }
    assert!(
        !carried.is_empty(),
        "the census must see typed attacks, or it passes vacuously"
    );

    for (ty, source) in carried {
        let countered = items.all().any(|def| {
            matches!(def.worn_stats(), Some((slot, stats))
                if slot == EquipmentSlot::Armor && stats.resist.get(ty) > 0)
        });
        assert!(
            countered,
            "{source} deals {} damage but no shipped armour resists it",
            ty.name()
        );
    }
}

#[test]
fn shipped_tags_reach_their_defs() {
    use crate::damage::DamageType;
    let (items, species) = shipped_items_and_species();

    let (_, lance) = items.get("arc_lance").unwrap().worn_stats().unwrap();
    assert_eq!(lance.damage_type, Some(DamageType::Voltage));
    let (_, plate) = items.get("ablative_plating").unwrap().worn_stats().unwrap();
    assert_eq!(plate.resist.get(DamageType::Thermal), 30);

    let shock = &species.get("scrapper").unwrap().moves;
    let shock = shock.iter().find(|m| m.name == "Shock").unwrap();
    assert_eq!(shock.damage_type, Some(DamageType::Voltage));

    let construct = species.get("construct").unwrap();
    assert_eq!(construct.resist.get(DamageType::Voltage), 30);
}

/// Blocking a side effect takes armour: no shipped species reaches the
/// threshold on its own natural resistance.
#[test]
fn no_shipped_species_blocks_a_side_effect_by_itself() {
    use crate::damage::DamageType;
    let (_, species) = shipped_items_and_species();
    let mut resisting = 0;
    for def in species.all() {
        for ty in DamageType::ALL {
            let pct = def.resist.get(ty);
            resisting += usize::from(pct > 0);
            assert!(
                !crate::damage::side_effect_blocked(pct),
                "{} resists {} by {pct} on its own",
                def.id,
                ty.name()
            );
        }
    }
    assert!(resisting > 0, "the census must see a species resistance");
}
