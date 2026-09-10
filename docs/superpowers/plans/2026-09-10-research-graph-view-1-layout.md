# Research Graph View, Phase 1: the layout derivation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Derive the research tree's flow-chart layout once in the engine — a
cell per node, the edge list, and the one rule for what an arrow key does —
so app-core's cursor and gui's boxes read the same derivation.

**Architecture:** A new `views::ResearchGraph`, built by `Game::research_graph()`
off `ResearchDb`. Keyed by `ResearchId` throughout, never by index into
`research_nodes()`. Tier is the longest path from a root, so every edge points
strictly rightward; slot orders by the first-listed parent's slot then by id.
`ResearchGraph::step` is total in all four directions. Nothing user-visible
ships in this phase — Phase 2 is the screen.

**Tech Stack:** Rust 2024, `bevy_ecs` (engine only), `ron` assets.

**Spec:** [`docs/superpowers/specs/2026-09-10-research-graph-view-design.md`](../specs/2026-09-10-research-graph-view-design.md)

## Global Constraints

- **Keyed by `ResearchId`, never by index into `Game::research_nodes()`.**
  That vec is sorted by `ResearchState`, so its order changes as the player
  buys things.
- **No save-format change.** Nothing here is stored; `SAVE_FORMAT_VERSION`
  does not move.
- **No `.ron` schema change.** `ResearchDef` gains no field, so no
  `assets/research/README.md` edit is owed by this phase.
- **`assets/research/` is content and this plan does not touch it.** The tier
  census is asserted against the shipped files as they stand.
- Gates: `cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`.
  Iterate with `cargo test -p feral-processes-engine <name>`.

## Two spec figures were wrong; this plan carries the corrected ones

Verified against `assets/research/` and `crates/engine/src/text.rs` on
2026-09-10, not remembered:

1. **The longest unbreakable word is `Self-Execution` at 14 characters, not
   `Fabrication` at 11.** `text::wrap` splits on `split_whitespace()` only
   (`crates/engine/src/text.rs:16`) — a hyphen is not a break opportunity. The
   fit constraint is therefore 27% wider than the spec assumed. Phase 2 owns
   the consequence; it is recorded here so the number travels with the plans.
2. **The field is `App::menu_selected`, not `App::selected`**
   (`crates/app-core/src/lib.rs:2569`). Phase 2's concern, recorded for the
   same reason.

Everything else in the spec verified exactly: the six-tier census below, the
one tier-skipping edge (`paging` → `segmentation`), and the 19-character
widest name (`Address Translation`).

## File Structure

| File | Responsibility |
|---|---|
| `crates/engine/src/research.rs` (modify) | `load_dir` gains the cycle drop; its own `mod tests` gains the case |
| `crates/engine/src/views.rs` (modify) | `GraphDir`, `ResearchCell`, `ResearchGraph` and `impl ResearchGraph` |
| `crates/engine/src/game/unlocks.rs` (modify) | `Game::research_graph()`, beside `research_nodes` |
| `crates/engine/src/tests/research_graph.rs` (create) | The census and the `step` totality tests, against the real assets |
| `crates/engine/src/tests/mod.rs` (modify) | `mod research_graph;` |

`ResearchGraph` goes in `views.rs` — the spec's placement — rather than a
module of its own. `views.rs` already carries four `impl` blocks
(`RoutineScope`, `TalentPoints`, `StructureReport`, `BuildOrderRow`), so a
view with behaviour on it is the existing practice there, not a new one.

---

### Task 1: A cycle in `requires` is dropped at load

**Why this is first.** Task 2's tier fold is `1 + max(tier of prereqs)`. On a
mod-authored cycle that has no fixpoint, and a naive memoised walk recurses
forever. The spec's answer is the loader's rule — drop it with a warning —
and `ResearchDb::load_dir` already drops "a node with an unreachable prereq"
for exactly the reason a cycle qualifies: **both members of a cycle are
permanently unresearchable today**, because `missing_prereqs` can never empty
for either. So this closes a real gap in the loader's own stated rule rather
than inventing a new one for the graph's benefit.

`recommended_ids`' `seen`-guarded walk stays as it is. It is documented as
terminating on a cycle, and after this change it will simply never be handed
one — a guard that has become unreachable through the shipped loader is still
the right shape for a `pub` method.

**Files:**
- Modify: `crates/engine/src/research.rs` — `ResearchDb::load_dir`'s fixpoint
  loop, and the `a_cycle_in_requires_does_not_hang_the_walk` test at the foot
  of the file
- Test: `crates/engine/src/research.rs` (`mod tests`, the existing `load`
  fixture)

**Interfaces:**
- Consumes: nothing.
- Produces: `ResearchDb::load_dir` now guarantees the loaded node set is
  acyclic under `requires`. Task 2 relies on that and on nothing else.

- [ ] **Step 1: Replace the existing cycle test with the drop it now owes**

Replace `a_cycle_in_requires_does_not_hang_the_walk` wholesale — it currently
asserts `warnings.is_empty()`, which this task deliberately reverses.

```rust
    /// A mod can author `a` requires `b` requires `a`. Both nodes are
    /// permanently unresearchable — `missing_prereqs` can never empty for
    /// either — which is the same condition `load_dir` already drops a node
    /// with a dangling prereq for. Dropping them here is also what lets
    /// `Game::research_graph`'s tier fold be a plain `1 + max(parents)`: a
    /// cycle has no fixpoint under it.
    #[test]
    fn a_cycle_in_requires_is_dropped_with_a_warning() {
        let a = r#"(id: "a", name: "A", description: "d", cost: 1, recommended: true, requires: ["b"])"#;
        let b = r#"(id: "b", name: "B", description: "d", cost: 1, requires: ["a"])"#;
        let (db, warnings) = load("cycle", &[("a", a), ("b", b)]);
        assert!(db.get("a").is_none(), "a cycle member can never be bought");
        assert!(db.get("b").is_none());
        assert_eq!(warnings.len(), 2, "each dropped node explains itself");
        assert!(
            warnings.iter().all(|w| w.contains("cycle")),
            "the warning has to name what was wrong: {warnings:?}"
        );
        assert!(
            db.recommended_ids().is_empty(),
            "nothing survives to be recommended"
        );
    }

    /// The drop must not reach past the cycle. A node *depending* on one is
    /// already handled by the existing dangling-prereq cascade; a node the
    /// cycle depends on nothing of must survive untouched.
    #[test]
    fn a_cycle_takes_only_itself_and_its_dependents() {
        let a = r#"(id: "a", name: "A", description: "d", cost: 1, requires: ["b"])"#;
        let b = r#"(id: "b", name: "B", description: "d", cost: 1, requires: ["a"])"#;
        let downstream = r#"(id: "downstream", name: "Downstream", description: "d", cost: 2, requires: ["a"])"#;
        let (db, _) = load(
            "cycle_bystander",
            &[
                ("a", a),
                ("b", b),
                ("downstream", downstream),
                ("automation", VALID),
            ],
        );
        assert!(db.get("automation").is_some(), "a bystander is untouched");
        assert!(
            db.get("downstream").is_none(),
            "a node hanging off a dropped cycle is just as unreachable"
        );
    }
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p feral-processes-engine research::tests::a_cycle
```

Expected: `a_cycle_in_requires_is_dropped_with_a_warning` FAILS on
`db.get("a").is_none()` — the node loads today.

- [ ] **Step 3: Add the cycle drop to `load_dir`'s fixpoint loop**

`load_dir` already runs a `loop { … if dropped.is_empty() { break } … }`
fixpoint over two rules (unknown structure, unknown prereq). Add a third pass
inside that loop, **after** the two existing rules, that finds every node not
reachable in a topological order of what remains and drops it:

- Kahn's algorithm over `db.nodes`: seed a queue with every node whose
  `requires` are all already emitted, emit it, and repeat. Count what is
  emitted.
- Anything unemitted when the queue drains is on, or downstream of, a cycle.
  Push each onto `dropped` with
  `format!("skipped research {id:?}: a cycle in requires")`.

Running it inside the existing loop rather than after it is what makes the
cascade free: the dangling-prereq rule and this one relax against each other
until neither has anything left to say.

Do not recurse. The whole point of this task is that a cycle must not be able
to reach the stack.

- [ ] **Step 4: Run the whole research module green**

```bash
cargo test -p feral-processes-engine research::
```

Expected: PASS, `the_shipped_tree_loads_clean` included — the shipped tree is
acyclic, so it must warn about nothing and still load all 34 files.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/research.rs
git commit -m "fix(research): drop a cycle in requires at load

Both members of a requires cycle are permanently unresearchable —
missing_prereqs can never empty for either — which is the condition
load_dir already drops a dangling prereq for. Dropping them is also
what lets the graph's tier fold be a plain 1 + max(parents).

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: `ResearchGraph` — cells, edges and the census

**Files:**
- Modify: `crates/engine/src/views.rs` — add `GraphDir`, `ResearchCell`,
  `ResearchGraph` near `ResearchStatus` (around line 24-100)
- Modify: `crates/engine/src/game/unlocks.rs` — `Game::research_graph()`,
  beside `research_nodes` (around line 310)
- Create: `crates/engine/src/tests/research_graph.rs`
- Modify: `crates/engine/src/tests/mod.rs` — `mod research_graph;`, in
  alphabetical position

**Interfaces:**

- Consumes: `ResearchDb::load_dir`'s acyclicity guarantee (Task 1);
  `ResearchDb::all()` (cost-then-id order) and `ResearchDef::{id, requires}`.
- Produces, all reachable from gui and app-core as
  `feral_processes_engine::*` (`lib.rs:129` is `pub use views::*;`):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphDir { Up, Down, Left, Right }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchCell {
    pub id: ResearchId,
    pub tier: usize,
    pub slot: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchGraph {
    pub cells: Vec<ResearchCell>,
    /// (prerequisite, dependent), both `ResearchId`.
    pub edges: Vec<(ResearchId, ResearchId)>,
    /// One past the deepest tier; 0 for an empty tree.
    pub tiers: usize,
    /// The slot count of the most crowded tier; 0 for an empty tree.
    pub widest: usize,
}

impl ResearchGraph {
    pub fn cell(&self, id: &str) -> Option<&ResearchCell>;
}

impl Game {
    pub fn research_graph(&self) -> ResearchGraph;
}
```

`ResearchGraph::step` is Task 3's and is not called by anything in this task.

- [ ] **Step 1: Write the failing tests**

Create `crates/engine/src/tests/research_graph.rs`:

```rust
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
    let tier_of: BTreeMap<&str, usize> =
        g.cells.iter().map(|c| (c.id.as_str(), c.tier)).collect();
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
```

- [ ] **Step 2: Register the module and run the tests**

Add `mod research_graph;` to `crates/engine/src/tests/mod.rs` in alphabetical
position (between `mod research;` and whatever follows it).

```bash
cargo test -p feral-processes-engine tests::research_graph
```

Expected: FAIL to compile — `ResearchGraph` and `Game::research_graph` do not
exist.

- [ ] **Step 3: Add the types to `views.rs`**

Put `GraphDir`, `ResearchCell` and `ResearchGraph` immediately after
`ResearchState` (which ends around `views.rs:100`), so the research views sit
together. Signatures are in the **Interfaces** block above; write them
verbatim. `cell` is a linear scan of 34 entries — no index map, and say so in
its doc comment, because a map is the "optimization ahead of evidence" this
repo's principles name.

Document on `ResearchGraph` itself, in one sentence each: that it is keyed by
`ResearchId` and why (`research_nodes()` re-sorts by state), and that `tier`
is the longest path from a root and why (every edge points rightward, and the
diamond stretches rather than reversing).

- [ ] **Step 4: Build it in `Game::research_graph()`**

In `crates/engine/src/game/unlocks.rs`, immediately after `research_nodes`.
The derivation is short; what matters is the order of its three passes:

1. **Tiers.** Kahn over `ResearchDb::all()` (cost-then-id, so the walk is
   already deterministic). A node with no `requires` is tier 0; otherwise
   `1 + max(tier of its requires)`. Task 1 guarantees this terminates.
2. **Slots.** Within a tier, sort by `(first-listed parent's slot, id)` and
   number the result from 0. Tier 0 has no parents, so it sorts by `id`
   alone. Process tiers in ascending order so a parent's slot is always
   already known — including the diamond, whose first-listed parent
   (`paging`) is two tiers back, which the rule handles because it reads the
   parent's *slot* and never its tier. A parent named in `requires` but not
   loaded cannot happen (`load_dir` drops the node), so treat it as slot 0
   rather than writing a branch for it.
3. **Edges, `tiers` and `widest`**, folded off the finished cells.

Return the `ResearchCell`s in `(tier, slot)` order. Nothing depends on that
yet, but it is what makes a failing assertion readable.

- [ ] **Step 5: Run the tests green**

```bash
cargo test -p feral-processes-engine tests::research_graph
```

Expected: PASS, all six.

- [ ] **Step 6: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test -p feral-processes-engine
git add crates/engine/src/views.rs crates/engine/src/game/unlocks.rs \
        crates/engine/src/tests/research_graph.rs crates/engine/src/tests/mod.rs
git commit -m "feat(research): derive the tree's flow-chart layout

views::ResearchGraph, from Game::research_graph — a cell per node
carrying its tier and slot, the edge list, and the two figures a
renderer needs to size a cell. Keyed by ResearchId, never by index
into research_nodes(), which re-sorts by state as the player buys.

Tier is the longest path from a root, so every edge points strictly
rightward and the one tier-skipping edge stretches rather than
reversing.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: `ResearchGraph::step` — the one rule for an arrow key

**Files:**
- Modify: `crates/engine/src/views.rs` — `impl ResearchGraph`
- Test: `crates/engine/src/tests/research_graph.rs`

**Interfaces:**
- Consumes: `ResearchGraph` and `GraphDir` (Task 2).
- Produces:

```rust
impl ResearchGraph {
    /// Where an arrow key lands from `from`. Total: an id with no cell, or a
    /// step off the edge of the grid, returns `from` itself.
    pub fn step(&self, from: &str, dir: GraphDir) -> ResearchId;
}
```

Phase 2's app-core task calls exactly this and computes no neighbours of its
own. That is the whole reason the layout is derived in the engine: two rules
for "what is next to this" would drift, and the cursor would leave the boxes.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/research_graph.rs`:

```rust
/// `step` is total. The cursor is driven by a key repeat, so a direction
/// with nowhere to go has to be a no-op and not a panic, a wrap, or a
/// `None` app-core would have to branch on.
#[test]
fn step_is_total_in_all_four_directions_from_every_node() {
    let g = graph(908);
    let ids: Vec<String> = g.cells.iter().map(|c| c.id.clone()).collect();
    for id in &ids {
        for dir in [GraphDir::Up, GraphDir::Down, GraphDir::Left, GraphDir::Right] {
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
    // Tier 1 has seven slots and tier 2 has five, so stepping right from
    // tier 1 slot 3 is equidistant from nothing — the interesting tie is
    // stepping *left* from tier 2 into tier 1, which is wider. Assert the
    // rule directly instead: every landing is a minimum, and among minima
    // it is the lowest slot.
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
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p feral-processes-engine tests::research_graph
```

Expected: FAIL to compile — `step` does not exist.

- [ ] **Step 3: Implement `step`**

In `views.rs`'s `impl ResearchGraph`. Four lines of behaviour:

- Resolve `from` to a cell; if there is none, return `from.to_string()`.
- `Up`/`Down`: the cell in the **same tier** whose slot is `slot - 1` /
  `slot + 1`. Absent, return `from` — clamp, never wrap.
- `Left`/`Right`: the target tier is `tier - 1` / `tier + 1`. If it is out of
  range, return `from`. Otherwise `min_by_key` over that tier's cells on
  `(slot.abs_diff(from.slot), slot)`, which is the nearest-slot rule and the
  lower-slot tiebreak in one key.

Document that it is total and that clamping rather than wrapping is
deliberate — a column's ends are at opposite edges of the pane, so a wrap
reads as the cursor teleporting.

- [ ] **Step 4: Run the tests green**

```bash
cargo test -p feral-processes-engine tests::research_graph
```

Expected: PASS, all ten in the module.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/views.rs crates/engine/src/tests/research_graph.rs
git commit -m "feat(research): ResearchGraph::step, the one arrow-key rule

Up and down move within a tier and clamp; left and right move one
tier and land on the nearest slot, ties toward the lower. Total in
all four directions, so app-core branches on nothing.

It lives beside the cells rather than in app-core because two rules
for what is next to a node would drift from the boxes they steer.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Phase gate

Before Phase 2 starts:

```bash
cargo test --workspace
cargo clippy --workspace
```

`cargo test --workspace` is the final gate, not `-p feral-processes-engine` —
a single-crate run is a different build and shifts the RNG stream, so the
suite has to be seen whole.

Nothing user-visible has shipped. `Game::research_graph` and
`ResearchGraph::step` are `pub` and called only by tests, which is expected
at this gate and is Phase 2's first task to fix.

## Out of scope for this phase

- The screen, the toggle key and every pixel of drawing — Phase 2.
- Any change to what a node costs, unlocks or requires.
- `ResearchDef` gaining a field, and so `assets/research/README.md`.
