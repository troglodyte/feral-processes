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

        // The chain groups together rather than scattering by id —
        // `assembly_bay` sorted third overall under the old id-pinned order,
        // ahead of every machine that feeds it.
        let assemblers: Vec<&str> = defs
            .iter()
            .filter(|d| d.category() == StructureCategory::Assembler)
            .map(|d| d.id.as_str())
            .collect();
        // The Armory and Fabricator sit here because `category()` reads
        // `assembles`, and that is the right group for them: they now want a
        // program and adjacent feeders like any machine. They are still the
        // hand-craft bench for the rest of their gear class, which no category
        // expresses — a structure is filed by what it needs, not by every use.
        // The Compiler moved here off the same rule when it stopped printing
        // catalysts from nothing; it is still the routine-extraction bench,
        // which is the same "filed by what it needs" case.
        assert_eq!(
            assemblers,
            [
                "annealing_node",
                "armory",
                "assembly_bay",
                "compiler",
                "disk_press",
                "fabricator",
                "lathe",
                "refactor_bench",
                "refinery",
                "transcriber",
                "winding_node"
            ]
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
            || def.taming_potency.is_some();
        assert_eq!(
            declares,
            !game.item_effects(&def.id).is_empty(),
            "{} declares an effect field but gets no line (or the reverse)",
            def.id.as_str()
        );
    }
}
