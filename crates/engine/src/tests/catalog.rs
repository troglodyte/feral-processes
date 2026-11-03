//! Ordering and stability of the asset-database listings the menus read.

use super::support::*;
use crate::*;

#[test]
fn structure_defs_are_grouped_by_category_and_stable_across_sessions() {
    use crate::structures::StructureCategory;

    // StructureDb is backed by a HashMap, whose iteration order is
    // randomized per-instance — without an explicit sort, the build
    // menu's [1], [2], ... numbering would shuffle between sessions
    // even though the mod files never changed. Multiple seeds (each a
    // fresh StructureDb/HashMap instance) should all agree.
    let seeds = [40, 41, 42, 43];
    let mut orders = Vec::new();
    for seed in seeds {
        let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let defs = game.structure_defs();

        assert_eq!(
            defs.first().map(|d| d.category()),
            Some(StructureCategory::Home),
            "Home leads the build menu — it is the one thing buildable first"
        );
        let categories: Vec<StructureCategory> = defs.iter().map(|d| d.category()).collect();
        let mut grouped = categories.clone();
        grouped.sort();
        assert_eq!(
            categories, grouped,
            "categories must appear in one contiguous run each, in variant order"
        );

        // Alphabetical by name *within* a group, which is what makes the
        // ordering reproducible for a modded structure that has no pinned
        // position of its own.
        for window in defs.windows(2) {
            if window[0].category() == window[1].category() {
                assert!(
                    window[0].name <= window[1].name,
                    "{} should not precede {} inside their group",
                    window[0].name,
                    window[1].name
                );
            }
        }

        // The chain groups together rather than scattering by id — the
        // patch-routine bench sorted third overall under the old id-pinned
        // order, ahead of every machine that feeds it. The three assembling
        // machines and the Assembler stand here by their `machine` role; the
        // Teardown Rig joins them on `category()` reading `strips`, since a
        // rig wants a posted program and pays items out, which is what
        // `Assembler` means here rather than a seventh variant every menu
        // would have to learn to draw.
        let assemblers: Vec<&str> = defs
            .iter()
            .filter(|d| d.category() == StructureCategory::Assembler)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(
            assemblers,
            ["assembler", "factory", "printer", "teardown_rig"]
        );

        orders.push(defs.into_iter().map(|d| d.id).collect::<Vec<_>>());
    }
    assert!(
        orders.windows(2).all(|w| w[0] == w[1]),
        "structure order should be identical across fresh sessions, got {orders:?}"
    );
}

#[test]
fn species_defs_order_is_sorted_by_id_and_stable_across_sessions() {
    let seeds = [44, 45, 46, 47];
    let mut orders = Vec::new();
    for seed in seeds {
        let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ids: Vec<String> = game.species_defs().into_iter().map(|d| d.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "species_defs() should already be sorted by id");
        orders.push(ids);
    }
    assert!(
        orders.windows(2).all(|w| w[0] == w[1]),
        "species order should be identical across fresh sessions, got {orders:?}"
    );
}

// ---------------------------------------------------------------------
// `Game::item_effects` — the line a listing screen prints under an item
// ---------------------------------------------------------------------

/// A worn passive is derived off the *ability*, never off the item's own
/// authored prose — the same argument `item_grant` makes, and it is a call
/// rather than a second read of `grants`.
#[test]
fn a_granted_passive_is_named_by_the_routine_it_carries() {
    let game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let lines = game.item_effects(&ItemId::from("watchdog_tap"));

    assert_eq!(lines.len(), 1, "one effect, one line: {lines:?}");
    assert!(
        lines[0].starts_with("Grants: "),
        "the line has to say what kind of effect it is: {lines:?}"
    );
    let (name, _) = game
        .item_grant(&ItemId::from("watchdog_tap"))
        .expect("watchdog_tap grants a routine");
    assert!(
        lines[0].contains(name),
        "and name the routine `item_grant` reports, not the item: {lines:?}"
    );
}

/// A consumable's pre-battle buff is priced through
/// `FieldBuffKind::magnitude_label`, so the line and the running buff list
/// cannot quote different numbers.
#[test]
fn a_consumables_prebattle_buff_reads_in_the_buff_lists_own_units() {
    let game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let lines = game.item_effects(&ItemId::from("patch_routine"));

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("Use: "), "{lines:?}");
    assert!(
        lines[0].contains(&components::FieldBuffKind::Mitigation.magnitude_label(10, 1)),
        "the magnitude must come from the one label fn: {lines:?}"
    );
    assert!(
        lines[0].contains("120t"),
        "and say how long it lasts: {lines:?}"
    );
}

/// Power Cell restores Power on use, which is the other `ConsumeDef` shape.
#[test]
fn a_consumable_that_only_restores_says_what_it_restores() {
    let game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let lines = game.item_effects(&ItemId::from(items::ids::POWER_CELL));

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].to_lowercase().contains("power"), "{lines:?}");
}

/// A refactor item's magnitudes are data, so the line quotes them rather
/// than the item's prose — **in the units the field is authored in**.
///
/// `CompanionUpgradeDef`'s percentages are percentage *points*
/// (`refactor::raised` divides by 100), and a second conversion here
/// reported a Buffer Extension's +5% HP as +500%. Asserted against every
/// shipped upgrade's own value rather than a literal, so a unit error
/// cannot pass by containing a `%`.
#[test]
fn a_companion_upgrade_quotes_its_own_percentages() {
    let game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let mut checked = 0;
    for def in game.item_defs() {
        let Some(u) = def.upgrade else { continue };
        let lines = game.item_effects(&def.id);
        assert_eq!(lines.len(), 1, "{}: {lines:?}", def.id.as_str());
        assert!(lines[0].starts_with("Refactor: "), "{lines:?}");
        for pct in [u.hp_percent, u.atk_percent, u.def_percent] {
            if pct != 0.0 {
                assert!(
                    lines[0].contains(&format!("+{pct:.0}%")),
                    "{} must quote its authored {pct}%, got: {lines:?}",
                    def.id.as_str()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "the shipped set defines percentage upgrades");
}

#[test]
fn a_taming_catalyst_says_what_it_adds_to_a_decompile() {
    let game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let lines = game.item_effects(&ItemId::from("ice_breaker"));

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("Decompile: "), "{lines:?}");
    assert!(
        lines[0].contains("40%"),
        "the potency, as a chance: {lines:?}"
    );
    // `taming::capture_chance` *multiplies* this by resistance, skill and
    // any running CaptureBoost, so a line reading as a flat addend would be
    // telling the player something the formula does not do.
    assert!(
        lines[0].contains("base"),
        "and must say it is a base, not a bonus: {lines:?}"
    );
}

/// Plain salvage and plain currency have nothing to say, and an empty list
/// is what tells a renderer to draw no extra line at all. The stat bonus on
/// a bare weapon is **not** an effect: it already rides the equip tag on the
/// row's own line, and printing it twice is the column twice.
#[test]
fn an_item_with_no_extra_effect_reports_no_lines() {
    let game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    assert!(
        game.item_effects(&ItemId::from(items::ids::CORE_FRAGMENT))
            .is_empty()
    );
    assert!(
        game.item_effects(&ItemId::from("arc_lance")).is_empty(),
        "a plain weapon's stats are the equip tag's job, not an effect line"
    );
    assert!(
        game.item_effects(&ItemId::from("nothing_by_this_name"))
            .is_empty(),
        "an unknown id answers rather than panicking"
    );
}

/// Every shipped item that declares one of the four effect fields gets a
/// line — the census that stops a new field being added to `ItemDef` and
/// quietly reaching no screen, which is exactly how `power_cost` shipped
/// reaching nothing.
#[test]
fn every_shipped_item_with_an_effect_field_gets_a_line() {
    let game = Game::new(66, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for def in game.item_defs() {
        let declares = def.grants.is_some()
            || def.consume.is_some()
            || def.upgrade.is_some()
            || def.taming_potency.is_some()
            || def.trap.is_some();
        assert_eq!(
            declares,
            !game.item_effects(&def.id).is_empty(),
            "{} declares an effect field but gets no line (or the reverse)",
            def.id.as_str()
        );
    }
}

/// A fixture item authoring a `trap:` — the shipped honeypot does not exist
/// until the assets land, and this is about the field rather than the item.
const TRAP_ITEM: &str = r#"(
    id: "fixture_trap",
    name: "Fixture Trap",
    description: "A test decoy.",
    value: Some(1),
    trap: Some((rarity_cap: Silver)),
)"#;

/// An item authoring no `trap:` at all, which is the `#[serde(default)]`
/// claim: without the default the field ships mandatory and every mod's
/// item files stop loading.
const PLAIN_ITEM: &str = r#"(
    id: "fixture_plain",
    name: "Fixture Plain",
    description: "Ordinary cargo.",
    value: Some(1),
)"#;

#[test]
fn an_item_authoring_a_trap_is_placeable_and_one_without_is_not() {
    let dir = super::support::modded_assets_dir(
        "trap_field",
        &[],
        &[
            ("fixture_trap.ron", TRAP_ITEM),
            ("fixture_plain.ron", PLAIN_ITEM),
        ],
        &[],
        &[],
        &[],
    );
    let game = Game::new(21, DifficultyMode::Forgiving, &dir).unwrap();

    let trap = ItemId::from("fixture_trap");
    let plain = ItemId::from("fixture_plain");
    assert!(
        game.item_defs().iter().any(|d| d.id == trap),
        "the trap fixture did not parse — a skipped file reads as the field doing nothing"
    );
    assert!(
        game.item_defs().iter().any(|d| d.id == plain),
        "the plain fixture did not parse"
    );

    assert!(game.is_placeable(&trap), "a `trap:` item is placeable");
    assert!(
        !game.is_placeable(&plain),
        "an item authoring no `trap:` is ordinary cargo"
    );
    assert!(
        !game.is_placeable(&ItemId::from("nothing_by_this_name")),
        "an unknown id is not placeable"
    );
}

#[test]
fn a_traps_effect_line_names_its_rarity_ceiling() {
    let dir = super::support::modded_assets_dir(
        "trap_effect_line",
        &[],
        &[("fixture_trap.ron", TRAP_ITEM)],
        &[],
        &[],
        &[],
    );
    let game = Game::new(22, DifficultyMode::Forgiving, &dir).unwrap();

    let lines = game.item_effects(&ItemId::from("fixture_trap"));
    // Asked of the type, never hand-typed, so a renamed tier moves the
    // test with the code.
    let tier = Rarity::Silver.label().expect("Silver reads as a tier");
    assert!(
        lines.iter().any(|l| l.contains(tier)),
        "the effect lines should name the ceiling, got {lines:?}"
    );
}

fn fresh_game() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn entry<'a>(chains: &'a [RecipeChain], product: &str) -> Option<&'a RecipeChain> {
    chains.iter().find(|c| c.product == product)
}

fn last_makers(chain: &RecipeChain) -> &[RecipeMaker] {
    &chain.steps.last().unwrap().makers
}

fn structure(name: &str) -> RecipeMaker {
    RecipeMaker::Structure(name.to_string())
}

#[test]
fn recipe_entries_are_alphabetical_and_unlocked_only() {
    let game = fresh_game();
    let chains = game.recipe_chains();

    let names: Vec<&str> = chains.iter().map(|c| c.product.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);

    let kit = entry(&chains, "Breach Kit").expect("a Breach Kit is compiled by hand");
    assert!(
        !last_makers(kit).contains(&structure("Printer")),
        "the Printer's Breach Kit recipe waits on Automation"
    );
    assert!(entry(&chains, "Honeypot").is_none(), "Deception is locked");
}

#[test]
fn breach_kit_lists_the_printer_and_by_hand_once_after_automation() {
    let mut game = fresh_game();
    unlock_research_chain(&mut game, "automation");
    let chains = game.recipe_chains();

    assert_eq!(
        chains.iter().filter(|c| c.product == "Breach Kit").count(),
        1
    );
    let kit = entry(&chains, "Breach Kit").unwrap();
    assert_eq!(
        last_makers(kit),
        [structure("Printer"), RecipeMaker::ByHand],
        "structures first, by hand last, each once"
    );
}

#[test]
fn research_finishing_adds_entries_on_the_next_call() {
    let mut game = fresh_game();
    assert!(entry(&game.recipe_chains(), "Honeypot").is_none());

    unlock_research_chain(&mut game, "deception");
    let chains = game.recipe_chains();
    let honeypot = entry(&chains, "Honeypot").expect("Deception unlocks it");
    assert!(last_makers(honeypot).contains(&structure("Assembler")));
    assert!(last_makers(honeypot).contains(&structure("Workbench")));
}

#[test]
fn workbench_gear_is_listed_away_from_the_bench() {
    let game = fresh_game();
    assert!(
        game.touched_bench("workbench").is_none(),
        "the fixture stands nowhere near a Workbench"
    );
    let chains = game.recipe_chains();
    let ward = entry(&chains, "Scrap Ward").expect("scrap_ward.ron needs no research");
    assert_eq!(last_makers(ward), [structure("Workbench")]);
}

#[test]
fn a_material_keeps_its_description_and_lists_its_uses() {
    let mut game = fresh_game();
    unlock_research_chain(&mut game, "armor_bench");
    let chains = game.recipe_chains();
    let block = entry(&chains, "Bytecode Block").expect("something makes bytecode blocks");

    assert!(block.description.is_some());
    assert_eq!(
        block.description.as_deref(),
        game.item_description(&ItemId::from("bytecode_block"))
    );
    assert!(!block.used_in.is_empty());
    let mut sorted = block.used_in.clone();
    sorted.sort();
    assert_eq!(block.used_in, sorted);
    for user in &block.used_in {
        assert!(
            entry(&chains, user).is_some(),
            "{user} is named as a use but has no entry"
        );
    }
}

/// Honeypots and Breach Kits are `Material` by category (no `consume`), so
/// the consumable that proves a description survives is the Power Cell.
#[test]
fn a_consumable_keeps_its_description_and_its_uses() {
    let game = fresh_game();
    let chains = game.recipe_chains();
    let cell = entry(&chains, "Power Cell").unwrap();

    assert_eq!(
        cell.description.as_deref(),
        game.item_description(&ItemId::from("power_cell"))
    );
    assert!(cell.used_in.contains(&"Charge Coil".to_string()));
}

/// The Extractor taps Power Cell from nothing while the Printer compiles it
/// from fragments; one step listing both would say the Extractor eats the
/// fragments.
#[test]
fn a_tap_and_a_recipe_for_one_item_are_separate_steps() {
    let game = fresh_game();
    let chains = game.recipe_chains();
    let cell = entry(&chains, "Power Cell").unwrap();
    let extractor = structure("Extractor");

    let recipe = cell.steps.last().unwrap();
    assert!(!recipe.inputs.is_empty());
    assert!(!recipe.makers.contains(&extractor), "{:?}", recipe.makers);
    assert!(
        cell.steps
            .iter()
            .any(|s| s.makers.contains(&extractor) && s.inputs.is_empty()),
        "no input-less Extractor step"
    );
}

const GATED_PRESS: &str = r#"(
    id: "gated_press",
    name: "Gated Press",
    description: "A modded assembler behind research, for tests.",
    glyph: 'G',
    color: Cyan,
    build_cost: [],
    capacity: 20,
    machine: Some((role: Assemble, recipes: [(produces: "gated_widget", ticks_per_unit: 3)])),
)"#;

const GATED_WIDGET: &str = r#"(
    id: "gated_widget",
    name: "Gated Widget",
    description: "Only the Gated Press makes these.",
    craftable: Some((cost: [("core_fragment", 1)], requires_structure: Some("gated_press"))),
)"#;

const GATED_RESEARCH: &str = r#"(
    id: "gated_unlock",
    name: "Gated Unlock",
    description: "Opens the Gated Press.",
    cost: 5,
    unlocks_structures: ["gated_press"],
)"#;

#[test]
fn a_recipe_whose_structure_is_locked_lists_no_maker() {
    let dir = modded_assets_dir(
        "gated_press",
        &[],
        &[("gated_widget.ron", GATED_WIDGET)],
        &[],
        &[("gated_unlock.ron", GATED_RESEARCH)],
        &[],
    );
    std::fs::write(dir.join("structures").join("gated_press.ron"), GATED_PRESS).unwrap();
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();

    assert!(!game.structure_unlocked("gated_press"));
    assert!(
        entry(&game.recipe_chains(), "Gated Widget").is_none(),
        "the press is locked, and it is the widget's only maker"
    );

    unlock_research_chain(&mut game, "gated_unlock");
    let chains = game.recipe_chains();
    let widget = entry(&chains, "Gated Widget").expect("unlocked now");
    assert_eq!(last_makers(widget), [structure("Gated Press")]);
}

#[test]
fn machine_products_names_only_unlocked_recipes() {
    let mut game = fresh_game();
    let defs = game.structure_defs();
    let def = |id: &str| defs.iter().find(|d| d.id == id).unwrap();

    assert_eq!(game.machine_products(def("printer")), ["Patch Routine"]);
    assert!(game.machine_products(def("depot")).is_empty());

    unlock_research_chain(&mut game, "automation");
    assert_eq!(
        game.machine_products(def("printer")),
        ["Breach Kit", "Patch Routine"]
    );
}
