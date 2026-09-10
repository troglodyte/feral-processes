//! The research tree's flow-chart layout — `Game::research_graph`.

use super::support::*;
use crate::*;
use std::collections::BTreeMap;

fn graph(seed: u32) -> ResearchGraph {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir())
        .unwrap()
        .research_graph()
}

/// The shipped tree's shape, so a content change that reshapes it is a
/// visible diff here rather than a squeezed screen nobody looked at. Six
/// tiers by nine slots is what the graph pane is sized against.
#[test]
fn the_shipped_tree_has_the_shape_the_screen_is_sized_for() {
    let g = graph(901);
    let mut by_tier: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for cell in &g.cells {
        by_tier.entry(cell.tier).or_default().push(&cell.id);
    }
    for ids in by_tier.values_mut() {
        ids.sort();
    }
    assert_eq!(
        by_tier.get(&0).map(Vec::len),
        Some(4),
        "tier 0 is the four roots"
    );
    assert_eq!(
        by_tier.get(&0),
        Some(&vec!["automation", "commerce", "paging", "power_grid"])
    );
    assert_eq!(by_tier.get(&1).map(Vec::len), Some(7));
    assert_eq!(by_tier.get(&2).map(Vec::len), Some(5));
    assert_eq!(by_tier.get(&3).map(Vec::len), Some(9));
    assert_eq!(by_tier.get(&4).map(Vec::len), Some(6));
    assert_eq!(by_tier.get(&5).map(Vec::len), Some(3));
    assert_eq!(g.cells.len(), 34, "every shipped node gets a cell");
    assert_eq!(g.tiers, 6);
    assert_eq!(g.widest, 9, "tier 3 is the crowded one");
}

/// Tier is the *longest* path from a root, so the diamond's short leg
/// stretches rather than pointing backwards. `paging` -> `segmentation` is
/// the one shipped edge that spans more than one tier, and asserting it by
/// name is what stops a "simplification" to shortest-path shipping silently.
#[test]
fn every_edge_points_strictly_rightward() {
    let g = graph(902);
    let tier_of: BTreeMap<&str, usize> = g.cells.iter().map(|c| (c.id.as_str(), c.tier)).collect();
    for (from, to) in &g.edges {
        let a = tier_of[from.as_str()];
        let b = tier_of[to.as_str()];
        assert!(
            b > a,
            "{from} (tier {a}) -> {to} (tier {b}) does not point rightward"
        );
    }
    assert_eq!(tier_of["paging"], 0);
    assert_eq!(tier_of["cache_coherence"], 1);
    assert_eq!(
        tier_of["segmentation"], 2,
        "the diamond's deeper parent is what sets the tier"
    );
    let spans: Vec<_> = g
        .edges
        .iter()
        .filter(|(from, to)| tier_of[to.as_str()] - tier_of[from.as_str()] > 1)
        .collect();
    assert_eq!(
        spans.len(),
        1,
        "exactly one shipped edge skips a tier, and no edge router is drawn for it: {spans:?}"
    );
}

/// One edge per `requires` entry, both ends naming a loaded node.
#[test]
fn the_edge_list_is_every_requires_entry_and_nothing_else() {
    let game = Game::new(903, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let g = game.research_graph();
    let ids: Vec<&str> = g.cells.iter().map(|c| c.id.as_str()).collect();
    for (from, to) in &g.edges {
        assert!(ids.contains(&from.as_str()), "{from} has no cell");
        assert!(ids.contains(&to.as_str()), "{to} has no cell");
    }
    assert!(
        g.edges
            .contains(&("paging".to_string(), "segmentation".to_string())),
        "the diamond's short leg is an edge like any other"
    );
    assert!(
        g.edges
            .contains(&("cache_coherence".to_string(), "segmentation".to_string()))
    );
}

/// A slot is a drawing position, so two nodes in one tier may never share
/// one, and the slots in a tier must be 0..n with no hole — the renderer
/// derives its vertical pitch from `widest` and indexes straight into it.
#[test]
fn slots_are_dense_and_unique_within_a_tier() {
    let g = graph(904);
    let mut by_tier: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for cell in &g.cells {
        by_tier.entry(cell.tier).or_default().push(cell.slot);
    }
    for (tier, mut slots) in by_tier {
        slots.sort();
        let want: Vec<usize> = (0..slots.len()).collect();
        assert_eq!(slots, want, "tier {tier} has a hole or a collision");
    }
}

/// `HashMap` iteration order is randomized per instance, which is what
/// `ResearchDb::all`'s own ordering exists to defeat one rung down. The
/// layout is drawn every frame and navigated with the arrow keys, so a
/// cell that moves between calls is a cursor that jumps.
#[test]
fn the_layout_is_deterministic_across_calls_and_across_games() {
    let game = Game::new(905, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.research_graph(),
        game.research_graph(),
        "two calls on one Game must agree"
    );
    let other = Game::new(906, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.research_graph(),
        other.research_graph(),
        "the layout is a property of the content, not of the run"
    );
}

/// The layout is keyed by id and derived off `ResearchDb`, which never
/// changes as the player buys things — where `research_nodes()` re-sorts by
/// state. Buying a node must move nothing.
#[test]
fn buying_a_node_does_not_move_the_layout() {
    let mut game = Game::new(907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = game.research_graph();
    set_inventory(&mut game, &[("core_fragment", 100)]);
    grant_research_data(&mut game, 500);
    let bought = game
        .research_nodes()
        .into_iter()
        .find(|n| n.state == ResearchState::Available && n.affordable)
        .expect("a fresh run can afford something");
    game.unlock_research(&bought.id).expect("it was affordable");
    assert_eq!(
        game.research_graph(),
        before,
        "the tree's shape is content, not progress"
    );
}

/// `step` is total. The cursor is driven by a key repeat, so a direction
/// with nowhere to go has to be a no-op and not a panic, a wrap, or a
/// `None` app-core would have to branch on.
#[test]
fn step_is_total_in_all_four_directions_from_every_node() {
    let g = graph(908);
    let ids: Vec<String> = g.cells.iter().map(|c| c.id.clone()).collect();
    for id in &ids {
        for dir in [
            GraphDir::Up,
            GraphDir::Down,
            GraphDir::Left,
            GraphDir::Right,
        ] {
            let landed = g.step(id, dir);
            assert!(
                g.cell(&landed).is_some(),
                "{id} stepped {dir:?} onto {landed:?}, which has no cell"
            );
        }
    }
    assert_eq!(
        g.step("no_such_node", GraphDir::Up),
        "no_such_node".to_string(),
        "an id with no cell is handed straight back"
    );
}

/// Up and down move within a tier and clamp at its ends — they do not wrap.
/// A list wraps because its ends are adjacent on screen; a column's are at
/// opposite edges of the pane, so wrapping there reads as the cursor
/// teleporting.
#[test]
fn up_and_down_move_within_a_tier_and_clamp() {
    let g = graph(909);
    let column: Vec<&ResearchCell> = {
        let mut c: Vec<&ResearchCell> = g.cells.iter().filter(|c| c.tier == 3).collect();
        c.sort_by_key(|c| c.slot);
        c
    };
    assert_eq!(column.len(), 9, "tier 3 is the nine-slot column");
    for pair in column.windows(2) {
        assert_eq!(
            g.step(&pair[0].id, GraphDir::Down),
            pair[1].id,
            "down goes to the next slot in the same tier"
        );
        assert_eq!(g.step(&pair[1].id, GraphDir::Up), pair[0].id);
    }
    let top = &column[0];
    let bottom = &column[column.len() - 1];
    assert_eq!(g.step(&top.id, GraphDir::Up), top.id, "the top clamps");
    assert_eq!(
        g.step(&bottom.id, GraphDir::Down),
        bottom.id,
        "the bottom clamps"
    );
}

/// Left and right move between tiers and land on the nearest slot, so the
/// cursor tracks roughly the height it was at rather than snapping to the
/// top of the next column.
#[test]
fn left_and_right_move_between_tiers_landing_on_the_nearest_slot() {
    let g = graph(910);
    let deep = g
        .cells
        .iter()
        .find(|c| c.tier == 3 && c.slot == 8)
        .expect("tier 3 has a slot 8");
    let landed = g.step(&deep.id, GraphDir::Right);
    let landed_cell = g.cell(&landed).expect("a cell");
    assert_eq!(landed_cell.tier, 4, "right moves exactly one tier");
    assert_eq!(
        landed_cell.slot, 5,
        "tier 4 has six slots, so the nearest to slot 8 is its last"
    );
    let root = g
        .cells
        .iter()
        .find(|c| c.tier == 0 && c.slot == 0)
        .expect("tier 0 has a slot 0");
    assert_eq!(
        g.step(&root.id, GraphDir::Left),
        root.id,
        "there is no tier left of the roots"
    );
    let deepest = g
        .cells
        .iter()
        .find(|c| c.tier == 5 && c.slot == 0)
        .expect("tier 5 has a slot 0");
    assert_eq!(
        g.step(&deepest.id, GraphDir::Right),
        deepest.id,
        "there is no tier right of the last"
    );
}

/// A tie breaks toward the lower slot, so the walk is reversible in the
/// common case and a held arrow key does not drift.
#[test]
fn a_tie_on_the_nearest_slot_breaks_toward_the_lower_one() {
    let g = graph(911);
    for cell in &g.cells {
        for (dir, target) in [
            (GraphDir::Left, cell.tier.checked_sub(1)),
            (GraphDir::Right, Some(cell.tier + 1)),
        ] {
            let Some(target) = target.filter(|t| *t < g.tiers) else {
                continue;
            };
            let landed = g.cell(&g.step(&cell.id, dir)).expect("a cell");
            let best = g
                .cells
                .iter()
                .filter(|c| c.tier == target)
                .map(|c| c.slot.abs_diff(cell.slot))
                .min()
                .expect("a non-empty tier");
            assert_eq!(
                landed.slot.abs_diff(cell.slot),
                best,
                "{} stepped {dir:?} onto a slot that is not nearest",
                cell.id
            );
            let lowest = g
                .cells
                .iter()
                .filter(|c| c.tier == target && c.slot.abs_diff(cell.slot) == best)
                .map(|c| c.slot)
                .min()
                .expect("at least the one it landed on");
            assert_eq!(landed.slot, lowest, "a tie breaks toward the lower slot");
        }
    }
}
