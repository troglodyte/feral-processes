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
        assert_eq!(
            assemblers,
            [
                "assembly_bay",
                "disk_press",
                "lathe",
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
