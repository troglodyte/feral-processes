//! Ordering and stability of the asset-database listings the menus read.

use super::support::*;
use crate::*;

#[test]
fn structure_defs_order_pins_home_mining_research_compiler_first_and_is_stable_across_sessions() {
    // StructureDb is backed by a HashMap, whose iteration order is
    // randomized per-instance — without an explicit sort, the build
    // menu's [1], [2], ... numbering would shuffle between sessions
    // even though the mod files never changed. Multiple seeds (each a
    // fresh StructureDb/HashMap instance) should all agree.
    let seeds = [40, 41, 42, 43];
    let mut orders = Vec::new();
    for seed in seeds {
        let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ids: Vec<String> = game.structure_defs().into_iter().map(|d| d.id).collect();
        assert_eq!(
            &ids[..4],
            ["home", "mining_node", "research_node", "compiler"],
            "the four starter structures should always lead the build menu"
        );
        let mut rest_sorted = ids[4..].to_vec();
        rest_sorted.sort();
        assert_eq!(
            ids[4..],
            rest_sorted[..],
            "everything after the pinned four should still be alphabetical"
        );
        orders.push(ids);
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
