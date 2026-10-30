use super::abilities::AFFINITY_SPECIES;
use crate::tests::support::*;
use crate::*;

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

    let compiled: Vec<crate::abilities::AbilityDef> = crate::routine_craft::Core::ALL
        .iter()
        .map(|core| {
            crate::routine_craft::compile(&crate::routine_craft::CraftedRoutine {
                id: format!("crafted/{}", core.key()),
                name: core.name().to_string(),
                parts: vec![crate::routine_craft::Subroutine::Core(
                    *core,
                    crate::routine_craft::Version::V1,
                )],
            })
        })
        .collect();
    for def in db.all().chain(compiled.iter()) {
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
    // every magnitude the species ever runs.
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: NaN");
    let dir = crate::tests::support::modded_assets_dir(
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
/// A species file is what a companion's kit comes from; an exclusive routine's `boss_drop` is what a
/// boss leaves behind. Either counts as reachable.
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
        .resource::<SpeciesDb>()
        .all()
        .flat_map(|s| s.abilities.iter())
        .map(|a| a.id.as_str())
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
        "these field routines are in no species kit, \
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
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let authored = items
        .get(ids::POWER_CELL)
        .and_then(|d| d.craftable.as_ref())
        .expect("power_cell ships with a recipe");

    let def: crate::structures::StructureDef = ron::from_str(
        r#"(
            id: "test_assembler", name: "Test Assembler", glyph: 'A', color: Cyan,
            build_cost: [],
            work: None,
            machine: Some((role: Assemble, recipes: [(produces: "power_cell", ticks_per_unit: 8)])),
        )"#,
    )
    .expect("a machine block must parse");
    let cell = ItemId(ids::POWER_CELL.to_string());

    assert_eq!(
        crate::systems::assembly_recipe(
            crate::structures::Placed {
                def: &def,
                recipe: Some(&cell)
            },
            items
        ),
        Some(authored.cost.as_slice()),
        "the machine's recipe is the item's own, not a second copy of it"
    );
}

/// A structure that is not an assembling machine has no recipe to run — the
/// resolver must say so rather than falling back to something.
#[test]
fn a_structure_that_assembles_nothing_resolves_no_recipe() {
    let game = Game::new(904, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let extractor = db.get("extractor").expect("extractor ships");
    let core = ItemId(ids::CORE_FRAGMENT.to_string());

    assert!(
        crate::systems::assembly_recipe(
            crate::structures::Placed {
                def: extractor,
                recipe: Some(&core)
            },
            items
        )
        .is_none()
    );
}

/// Without this, a typo'd recipe ships a machine that can never run and
/// says nothing at all. The same trap a mod falls into.
#[test]
fn every_shipped_assembling_recipe_names_an_item_that_declares_a_recipe() {
    let game = Game::new(903, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let mut checked = 0;
    for def in game
        .world
        .resource::<crate::structures::StructureDb>()
        .all()
    {
        let Some(machine) = &def.machine else {
            continue;
        };
        if machine.role != crate::structures::MachineRole::Assemble {
            continue;
        }
        for recipe in &machine.recipes {
            assert!(
                items
                    .get(recipe.produces.as_str())
                    .is_some_and(|d| d.craftable.is_some()),
                "{} lists {:?}, which is not a craftable item",
                def.id,
                recipe.produces
            );
            assert!(
                recipe.ticks_per_unit > 0,
                "{} would produce a unit every tick",
                def.id
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 12,
        "the shipped assembling recipes are five on the Factory, five on the \
         Assembler and two on the Printer — if that changes, change this count \
         deliberately rather than letting the check go vacuous"
    );
}

/// The hand-compile gates that moved onto the Workbench, item by item: a
/// merged bench must not make a recipe cheaper to reach than it was.
#[test]
fn every_bench_recipe_moved_to_the_workbench_keeps_its_gate() {
    let game = Game::new(905, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let gates: [(&[&str], Option<&str>); 7] = [
        (
            &[
                "annealed_core",
                "recompile_kernel",
                "bounds_check",
                "buffer_extension",
                "inline_cache",
            ],
            Some("program_refactoring"),
        ),
        (
            &[
                "bastion_lattice",
                "hardened_shell",
                "null_weave",
                "nullsteel_plate",
                "phase_carapace",
                "static_mesh",
            ],
            Some("armor_bench"),
        ),
        (
            &[
                "entropy_damper",
                "kernel_key",
                "logic_probe",
                "oracle_core",
                "reinitialization_protocol",
                "singularity_matrix",
                "sync_governor",
                "trace_sniffer",
            ],
            Some("weapon_bench"),
        ),
        (
            &["blank_substrate", "routine_disk", "logic_wafer"],
            Some("routine_fabrication"),
        ),
        (&["honeypot"], Some("deception")),
        (&["patch_routine", "bytecode_block", "charge_coil"], None),
        (&[], None),
    ];
    let mut checked = 0;
    for (ids, gate) in gates {
        for id in ids {
            let c = items
                .get(id)
                .and_then(|d| d.craftable.as_ref())
                .unwrap_or_else(|| panic!("{id} should be craftable"));
            assert_eq!(c.requires_structure.as_deref(), Some("workbench"), "{id}");
            assert_eq!(c.research.as_deref(), gate, "{id}");
            checked += 1;
        }
    }
    assert_eq!(checked, 26);
}

#[test]
fn no_shipped_recipe_names_a_removed_bench() {
    let game = Game::new(906, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let removed = [
        "compiler",
        "armory",
        "fabricator",
        "refactor_bench",
        "mining_node",
        "log_scraper",
        "cache_tap",
        "power_conduit",
        "lathe",
        "refinery",
        "annealing_node",
        "transcriber",
        "winding_node",
        "disk_press",
        "decoy_bench",
        "assembly_bay",
    ];
    let items = game.world.resource::<crate::items_db::ItemDb>();
    for def in items.all() {
        if let Some(bench) = def
            .craftable
            .as_ref()
            .and_then(|c| c.requires_structure.as_deref())
        {
            assert!(!removed.contains(&bench), "{} names {bench}", def.id);
        }
    }
    for node in game.world.resource::<crate::research::ResearchDb>().all() {
        for r in &node.unlocks_recipes {
            if let Some(bench) = r.requires_structure.as_deref() {
                assert!(!removed.contains(&bench), "{} names {bench}", node.id);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The power grid census — Task 4. `game::base::power::ledger` only ever
// reads what these three tests check the shipped set for: see
// `docs/superpowers/archive/specs/2026-08-17-base-power-grid-design.md`, "Which
// structures draw, and the numbers".
// ---------------------------------------------------------------------------

/// The gate that stops the sixteenth machine shipping free: every shipped
/// structure that `runs_a_job()` — an extractor or an assembler, the one
/// predicate `ledger` and everything else in the base agree on — must
/// declare a non-zero `power_draw`, or it never enters the sum and runs for
/// free forever.
///
/// Filters on `runs_a_job()` itself rather than re-deriving `work.is_some()
/// || machine.is_some()`, which is exactly the drift `runs_a_job`'s own
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
        checked, 7,
        "the four configurable machines, the Research Node, the Teardown Rig and the \
         Holding Cell; if that count changed, change this deliberately rather \
         than letting the check go vacuous"
    );
}

/// Home alone has to open a base: a fresh base with nothing else standing
/// must be able to run its opening extractors — one Extractor on Core
/// Fragments, one on Raw Trace, and a Research Node — before a single Recharger is built.
/// Stated as a concrete sum because "covers the opening" is not something a
/// test can check any other way.
#[test]
fn home_alone_powers_a_new_bases_opening_extractors() {
    let game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let home = db.get("home").expect("home ships");
    let extractor = db.get("extractor").expect("extractor ships");
    let research_node = db.get("research_node").expect("research_node ships");

    let opening_draw = 2 * extractor.power_draw + research_node.power_draw;
    assert!(
        home.power_supply >= opening_draw,
        "Home supplies {}, but two Extractors and a Research \
         Node together draw {opening_draw} — a fresh base would open dark",
        home.power_supply
    );
}

/// The bootstrap has to reach the bootstrapper. `ledger` cuts the machines
/// that can restart the grid last, nearest the fuel last of all, so a base
/// collapsed to the Home's free supply recovers only if that supply covers
/// the rung that makes fuel out of nothing — an Extractor on Power Cells standing dark
/// behind a Printer that ate the Home's 4 is how a run used to die with its
/// own way out standing idle.
///
/// The rungs are named rather than merely counted: they are *derived* from
/// the item catalogue (`power::fuel_chain`), so a new `grid_fuel` or a new
/// ingredient in the Power Cell's recipe silently reorders them, and the
/// reordered set may no longer fit the Home. Change the table deliberately.
#[test]
fn the_home_alone_lights_the_machine_that_makes_grid_fuel() {
    let game = Game::new(954, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let chain = crate::game::base::power::fuel_chain(items);

    let mut rungs: Vec<(u32, String)> = db
        .all()
        .filter(|def| def.runs_a_job())
        .flat_map(|def| {
            def.productions()
                .map(move |p| (def, p.recipe.produces.clone()))
        })
        .filter_map(|(def, item)| {
            let rung = crate::game::base::power::grid_rung(
                crate::structures::Placed {
                    def,
                    recipe: Some(&item),
                },
                &chain,
            )?;
            Some((rung, format!("{}/{}", def.id, item.as_str())))
        })
        .collect();
    rungs.sort_unstable();
    assert_eq!(
        rungs,
        [
            (0, "extractor/power_cell".to_string()),
            (1, "extractor/core_fragment".to_string())
        ],
        "the machines that outrank the rest of the base for power, and in \
         what order; the Charge Coil is a Power Cell *sink* and must not be here"
    );

    let home = db.get("home").expect("home ships");
    let makers_draw: u32 = rungs
        .iter()
        .filter(|(rung, _)| *rung == 0)
        .map(|(_, id)| db.get(id.split('/').next().unwrap()).unwrap().power_draw)
        .sum();
    assert!(
        home.power_supply >= makers_draw,
        "Home supplies {}, but one of each machine that makes grid fuel draws \
         {makers_draw} — a blackout would be permanent",
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

/// The gate on the other axis: `power_upkeep` is a *supplier's* cost, so a
/// structure that declares it and supplies nothing pays a Power Cell every
/// `POWER_UPKEEP_TICKS` to change no number the player could find.
///
/// The second half is the bootstrap, and it is the one thing about this
/// feature that cannot be recovered from any other file: the Home has to
/// stay free, or a base holding no Power Cells could never run the Power
/// Conduit that makes the first one. Asserted here beside the upkeep census
/// rather than left to `home.ron`'s absence of a line, because an absence is
/// exactly what nothing fails on.
///
/// The third check is `power_upkeep`'s own reason for being `Option<ItemId>`
/// rather than a bare `bool` (Task E): a fuel id no `ItemDb` entry resolves
/// ships a supplier that can never be fed and never says why — nothing here
/// fails to compile on a typo, so this is the only thing that catches one.
///
/// A `#[serde(default)]` field with no census is a field authored nowhere —
/// see `every_shipped_machine_declares_a_power_draw` above, whose count this
/// one deliberately copies.
#[test]
fn every_burning_supplier_supplies_something_and_the_home_burns_nothing() {
    let game = Game::new(953, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let mut checked = 0;
    for def in db.all() {
        let Some(fuel) = &def.power_upkeep else {
            continue;
        };
        assert!(
            def.power_supply > 0,
            "{} burns a Power Cell every POWER_UPKEEP_TICKS but supplies \
             nothing — the upkeep buys the player no number at all",
            def.id
        );
        let resolved = items.get(fuel.as_str());
        assert!(
            resolved.is_some(),
            "{} declares power_upkeep {:?}, which no ItemDb entry resolves — \
             a supplier that can never be fed",
            def.id,
            fuel
        );
        // The declared id is the *gate* on the whole grid-fuel family (see
        // `systems::burn_grid_upkeep`): naming an item that is not fuel does
        // not fall back to one that is, it burns nothing. So an id that
        // resolves but declares no `grid_fuel` is the same silent supplier
        // the check above exists to catch, one step further in.
        assert!(
            resolved.and_then(|d| d.grid_fuel).is_some(),
            "{} declares power_upkeep {:?}, which is a real item but not grid \
             fuel — the supplier is gated on its own declared fuel, so it \
             will burn nothing at all",
            def.id,
            fuel
        );
        checked += 1;
    }
    assert_eq!(
        checked, 2,
        "the Recharger Node and the Line Driver are the two suppliers that \
         burn; if that changed, change this deliberately rather than letting \
         the check go vacuous"
    );

    let home = db.get("home").expect("home ships");
    assert!(
        home.power_upkeep.is_none(),
        "the Home's supply is the bootstrap — a base holding no Power Cells \
         could never run the Power Conduit that makes the first one"
    );
}
