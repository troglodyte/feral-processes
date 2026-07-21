# Research Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gate structures and craft recipes behind a data-driven research tree paid for with a new Research Data resource produced by a cronjob-worked Research Node.

**Architecture:** A new `crates/engine/src/research.rs` module mirrors `structures.rs` — `ResearchDef`/`ResearchDb` loaded from `assets/research/*.ron`, malformed files skipped with a warning. A `Research(HashSet<ResearchId>)` world resource holds what's unlocked. `Game` gains `research_nodes` / `unlock_research` / `is_researched`, filters the build menu through `buildable_structure_defs`, rejects unresearched ids in `place_structure`, and builds `craft_recipes` from unlocked research instead of hardcoded constants. A new `Mode::Research` in app-core renders through both peer renderers.

**Tech Stack:** Rust, standalone `bevy_ecs`, `serde` + `ron` for assets, `bincode` for saves, `ratatui` (TUI) and `macroquad` (GUI).

## Global Constraints

- The engine's `Game` struct is the entire public API surface both renderers talk to via app-core. Neither renderer touches the ECS `World` directly.
- New `ResearchDef` / `ResearchRecipe` fields must be `#[serde(default)]` so existing `.ron` files, including third-party mods, keep parsing.
- A malformed `.ron` file must be skipped with a returned warning, never a panic that crashes startup.
- `assets/research/README.md` and `assets/structures/README.md` are the schema reference for modders; update them in the same task that changes the schema.
- Comments explain *why*, never *what*.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG. Background systems (habitat spawning, nests) will interfere with naive assertions — place structures explicitly.
- Prefer `Result`/`?` over panics in engine code. `unwrap()`/`expect()` are for tests and truly-infallible invariants.
- Run `cargo fmt` and `cargo clippy --workspace` after every change; fix warnings rather than silencing them.
- `cargo test --workspace` is the final gate, not just the tests you wrote.
- Never `git commit` unless the human partner explicitly asks. The commit step in each task below means: **stop and ask** whether to commit, then do it only on a yes.

**Spec correction:** the spec says the save format bumps 5 → 6. `SAVE_FORMAT_VERSION` is *already* 6 (`crates/engine/src/save.rs:164`). The real bump is **6 → 7**.

**Spec correction:** the spec worries that `difficulty.rs` pre-places a Terminal and Data Cache that are now research-gated. That code is a *unit test* (`crates/engine/src/difficulty.rs:96-107`) that calls `world.spawn` directly, bypassing `place_structure` entirely. No change is needed there.

---

## File Structure

**Created:**
- `crates/engine/src/research.rs` — `ResearchId`, `ResearchRecipe`, `ResearchDef`, `ResearchDb`. Loading, validation, deterministic ordering. Nothing about game state.
- `assets/structures/research_node.ron` — the Research Node structure.
- `assets/research/*.ron` — 13 tree nodes, one file each.
- `assets/research/README.md` — modder-facing schema reference.

**Modified:**
- `crates/engine/src/items.rs` — `ItemId::ResearchData`.
- `crates/engine/src/resources.rs` — `Research` resource.
- `crates/engine/src/structures.rs` — build-menu pin order.
- `crates/engine/src/lib.rs` — module decl, re-exports, `Game::new`/`load` wiring, `ResearchStatus`/`ResearchState`, the three new `Game` methods, structure gating, recipe gating, save/load.
- `crates/engine/src/save.rs` — `SaveData.researched`, version bump.
- `crates/app-core/src/lib.rs` — `Mode::Research`, `T` key, `handle_research_key`.
- `crates/tui/src/ui.rs` — `render_research_menu`, help text.
- `crates/gui/src/render.rs` — `draw_research_menu`, help text.
- `assets/structures/README.md` — Research Node + research gating note.

---

## Task 1: Research Data item and the Research Node structure

**Files:**
- Modify: `crates/engine/src/items.rs:4-15` (enum), `:17-31` (`display_name`), `:79-83` (`equipment` catch-all)
- Modify: `crates/engine/src/structures.rs:199-213` (`all()` pin order)
- Create: `assets/structures/research_node.ron`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/items.rs` (existing `mod tests`), `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `ItemId::ResearchData`; a loadable structure with id `"research_node"`; `StructureDb::all()` pin order `home, mining_node, research_node, compiler`.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/items.rs`, inside the existing `mod tests`, add:

```rust
#[test]
fn research_data_is_a_plain_resource() {
    assert!(
        ItemId::ResearchData.equipment().is_none(),
        "Research Data is spent on the tree, never worn"
    );
    assert_eq!(ItemId::ResearchData.display_name(), "Research Data");
}
```

In `crates/engine/src/lib.rs`, inside the existing `mod tests`, add:

```rust
#[test]
fn the_research_node_is_pinned_fourth_in_the_build_menu() {
    let game = test_game();
    let ids: Vec<String> = game.structure_defs().into_iter().map(|d| d.id).collect();
    assert_eq!(
        &ids[..4],
        &[
            "home".to_string(),
            "mining_node".to_string(),
            "research_node".to_string(),
            "compiler".to_string()
        ],
        "the early-game build sequence must be stable across sessions"
    );
}

#[test]
fn the_research_node_is_a_cronjob_worked_research_data_source() {
    let game = test_game();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "research_node")
        .expect("research_node.ron should load");
    let work = def.work.expect("the Research Node must be workable");
    assert_eq!(work.produces, ItemId::ResearchData);
}
```

Find the existing test-game helper in `crates/engine/src/lib.rs`'s `mod tests` (it builds a `Game` against `env!("CARGO_MANIFEST_DIR")`'s assets dir) and use that name in place of `test_game()` if it differs.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine research`

Expected: FAIL — `no variant named ResearchData found for enum ItemId`.

- [ ] **Step 3: Add the item variant**

In `crates/engine/src/items.rs`, add `ResearchData,` to the `ItemId` enum after `PortalFragment`; add `ItemId::ResearchData => "Research Data",` to `display_name`; and add `ItemId::ResearchData` to the `None` arm of `equipment`:

```rust
            ItemId::CoreFragment
            | ItemId::PowerCell
            | ItemId::IceBreaker
            | ItemId::PortalFragment
            | ItemId::ResearchData => None,
```

- [ ] **Step 4: Add the structure file**

Create `assets/structures/research_node.ron`:

```ron
(
    id: "research_node",
    name: "Research Node",
    glyph: 'R',
    color: Cyan,
    build_cost: [(CoreFragment, 10)],
    work: Some((produces: ResearchData, ticks_per_unit: 14, capacity: 4, level: Some(1))),
)
```

- [ ] **Step 5: Pin it in the build menu**

In `crates/engine/src/structures.rs`, change the `priority` closure inside `all()`:

```rust
        let priority = |id: &str| match id {
            "home" => 0,
            "mining_node" => 1,
            "research_node" => 2,
            "compiler" => 3,
            _ => 4,
        };
```

Update that method's doc comment to name `research_node` in the pinned sequence — the comment currently lists only three ids, and it explains *why* the pin exists, so it has to stay accurate.

- [ ] **Step 6: Update the structures README**

In `assets/structures/README.md`, add `ResearchData` to the `ItemId` list near line 20, and document the Research Node alongside the other example structures.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine research`

Expected: PASS. The pin-order test at `crates/engine/src/lib.rs:6085` (`structure_defs_order_pins_home_mining_node_compiler_first_and_is_stable_across_sessions`) will now FAIL — it asserts the old three-id order. Update its expectation to the new four-id order and rename it to `..._pins_home_mining_research_compiler_first_...`.

- [ ] **Step 8: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings, all tests pass.

- [ ] **Step 9: Ask whether to commit**

If yes:

```bash
git add crates/engine/src/items.rs crates/engine/src/structures.rs crates/engine/src/lib.rs assets/structures/research_node.ron assets/structures/README.md
git commit -m "feat: add Research Data item and Research Node structure"
```

---

## Task 2: The `ResearchDb` module

**Files:**
- Create: `crates/engine/src/research.rs`
- Modify: `crates/engine/src/lib.rs:1-14` (module declarations)
- Test: `crates/engine/src/research.rs` (new `mod tests`)

**Interfaces:**
- Consumes: `ItemId::ResearchData` (Task 1), `structures::{StructureDb, StructureId}`.
- Produces:
  - `pub type ResearchId = String`
  - `pub struct ResearchRecipe { result: ItemId, cost: Vec<(ItemId, u32)>, requires_structure: Option<StructureId> }`
  - `pub struct ResearchDef { id: ResearchId, name: String, description: String, cost: u32, requires: Vec<ResearchId>, unlocks_structures: Vec<StructureId>, unlocks_recipes: Vec<ResearchRecipe> }`
  - `pub struct ResearchDb` with `load_dir(dir: &Path, structures: &StructureDb) -> std::io::Result<(Self, Vec<String>)>`, `get(&self, id: &str) -> Option<&ResearchDef>`, `all(&self) -> impl Iterator<Item = &ResearchDef>`

- [ ] **Step 1: Write the failing tests**

Create `crates/engine/src/research.rs` containing *only* this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `files` as `.ron` into a fresh temp dir and loads a `ResearchDb`
    /// from it against a `StructureDb` built from the real assets — so
    /// `unlocks_structures` validation runs against real structure ids.
    fn load(files: &[(&str, &str)]) -> (ResearchDb, Vec<String>) {
        let dir = std::env::temp_dir().join(format!(
            "feral_research_{}_{}",
            std::process::id(),
            files.len()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join("structures");
        let (structures, _) = StructureDb::load_dir(&assets).unwrap();
        let result = ResearchDb::load_dir(&dir, &structures).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    const VALID: &str = r#"(
        id: "automation",
        name: "Automation",
        description: "Self-running compile jobs.",
        cost: 8,
        unlocks_structures: ["compiler"],
    )"#;

    #[test]
    fn a_valid_node_loads_with_defaulted_optional_fields() {
        let (db, warnings) = load(&[("automation", VALID)]);
        let def = db.get("automation").expect("valid node should load");
        assert_eq!(def.name, "Automation");
        assert_eq!(def.cost, 8);
        assert!(def.requires.is_empty(), "requires defaults to empty");
        assert!(def.unlocks_recipes.is_empty(), "recipes default to empty");
        assert_eq!(def.unlocks_structures, vec!["compiler".to_string()]);
        assert!(warnings.is_empty(), "a valid node warns about nothing");
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_rest_still_load() {
        let (db, warnings) = load(&[("automation", VALID), ("broken", "(this is not ron")]);
        assert!(
            db.get("automation").is_some(),
            "one bad mod file must not take the others down"
        );
        assert!(db.get("broken").is_none());
        assert_eq!(warnings.len(), 1, "exactly the bad file should warn");
        assert!(warnings[0].contains("broken"));
    }

    #[test]
    fn a_node_requiring_an_unknown_node_is_skipped_with_a_warning() {
        let dangling = r#"(
            id: "dangling",
            name: "Dangling",
            description: "Requires something that does not exist.",
            cost: 5,
            requires: ["nonexistent"],
        )"#;
        let (db, warnings) = load(&[("automation", VALID), ("dangling", dangling)]);
        assert!(
            db.get("dangling").is_none(),
            "a node with an unreachable prereq would be permanently unresearchable"
        );
        assert!(db.get("automation").is_some());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("nonexistent"));
    }

    #[test]
    fn a_node_unlocking_an_unknown_structure_is_skipped_with_a_warning() {
        let bad = r#"(
            id: "ghost_bench",
            name: "Ghost Bench",
            description: "Unlocks a structure that does not exist.",
            cost: 5,
            unlocks_structures: ["not_a_structure"],
        )"#;
        let (db, warnings) = load(&[("ghost_bench", bad)]);
        assert!(db.get("ghost_bench").is_none());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not_a_structure"));
    }

    #[test]
    fn removing_an_invalid_node_also_removes_whatever_depended_on_it() {
        let bad = r#"(
            id: "ghost_bench",
            name: "Ghost Bench",
            description: "Unlocks a structure that does not exist.",
            cost: 5,
            unlocks_structures: ["not_a_structure"],
        )"#;
        let dependent = r#"(
            id: "dependent",
            name: "Dependent",
            description: "Hangs off a node that gets dropped.",
            cost: 9,
            requires: ["ghost_bench"],
        )"#;
        let (db, warnings) = load(&[("ghost_bench", bad), ("dependent", dependent)]);
        assert!(db.get("ghost_bench").is_none());
        assert!(
            db.get("dependent").is_none(),
            "a node whose prereq was dropped is just as unreachable"
        );
        assert_eq!(warnings.len(), 2, "each dropped node explains itself");
    }

    #[test]
    fn all_is_ordered_by_cost_then_id() {
        let cheap = r#"(id: "cheap", name: "Cheap", description: "d", cost: 1)"#;
        let mid_b = r#"(id: "b_mid", name: "B", description: "d", cost: 5)"#;
        let mid_a = r#"(id: "a_mid", name: "A", description: "d", cost: 5)"#;
        let (db, _) = load(&[("cheap", cheap), ("b_mid", mid_b), ("a_mid", mid_a)]);
        let ids: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["cheap", "a_mid", "b_mid"],
            "HashMap order is randomized per instance; the menu must not be"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine --lib research::`

Expected: FAIL to compile — `cannot find type ResearchDb in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend this to `crates/engine/src/research.rs`, above the test module:

```rust
use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::structures::{StructureDb, StructureId};

pub type ResearchId = String;

/// A craft recipe a research node unlocks. Recipe *data* lives in the
/// research `.ron` files rather than in Rust so a mod can ship a structure,
/// its research node and its recipes as pure data — `ItemId` itself stays a
/// Rust enum (see `CLAUDE.md`), but nothing about a recipe has to be.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchRecipe {
    pub result: ItemId,
    pub cost: Vec<(ItemId, u32)>,
    /// If set, the recipe only appears in the compile menu while a structure
    /// of this kind is actually deployed — researching the blueprint isn't
    /// enough on its own, you still need the bench. `#[serde(default)]` so a
    /// recipe with no bench requirement can just omit the field.
    #[serde(default)]
    pub requires_structure: Option<StructureId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchDef {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    /// Research Data spent to unlock this node.
    pub cost: u32,
    #[serde(default)]
    pub requires: Vec<ResearchId>,
    #[serde(default)]
    pub unlocks_structures: Vec<StructureId>,
    #[serde(default)]
    pub unlocks_recipes: Vec<ResearchRecipe>,
}

#[derive(Resource, Default)]
pub struct ResearchDb {
    nodes: HashMap<ResearchId, ResearchDef>,
}

impl ResearchDb {
    /// Loads every `*.ron` research node in `dir`, then drops any node that
    /// can never be reached or acted on: one naming an unknown prereq, or an
    /// unknown structure in `unlocks_structures`. Both would otherwise sit in
    /// the menu forever with no explanation. Dropping cascades — a node whose
    /// prereq was itself dropped is equally unreachable — so validation runs
    /// to a fixpoint. Malformed files are skipped with a warning rather than
    /// aborting the load, so one bad mod file can't crash startup.
    pub fn load_dir(dir: &Path, structures: &StructureDb) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ResearchDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ResearchDef>(&text) {
                Ok(def) => {
                    db.nodes.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid research file {path:?}: {e}")),
            }
        }

        loop {
            let mut dropped = Vec::new();
            for def in db.nodes.values() {
                if let Some(missing) = def
                    .unlocks_structures
                    .iter()
                    .find(|s| structures.get(s).is_none())
                {
                    dropped.push((
                        def.id.clone(),
                        format!(
                            "skipped research {:?}: unlocks unknown structure {missing:?}",
                            def.id
                        ),
                    ));
                    continue;
                }
                if let Some(missing) = def.requires.iter().find(|r| !db.nodes.contains_key(*r)) {
                    dropped.push((
                        def.id.clone(),
                        format!("skipped research {:?}: unknown prerequisite {missing:?}", def.id),
                    ));
                }
            }
            if dropped.is_empty() {
                break;
            }
            for (id, warning) in dropped {
                db.nodes.remove(&id);
                warnings.push(warning);
            }
        }

        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&ResearchDef> {
        self.nodes.get(id)
    }

    /// Every loaded node, cheapest first and ties broken by `id`. `HashMap`
    /// iteration order is randomized per instance, so without this the tree's
    /// `[1]`, `[2]`, ... numbering would shuffle between sessions even though
    /// nothing about the files changed — the same digit could mean an 8-cost
    /// node one session and a 45-cost node the next.
    pub fn all(&self) -> impl Iterator<Item = &ResearchDef> {
        let mut defs: Vec<&ResearchDef> = self.nodes.values().collect();
        defs.sort_by(|a, b| a.cost.cmp(&b.cost).then_with(|| a.id.cmp(&b.id)));
        defs.into_iter()
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/engine/src/lib.rs`, add `pub mod research;` to the module list, keeping it alphabetical (after `pub mod progression;`, before `pub mod resources;`).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine --lib research::`

Expected: PASS, 6 tests.

- [ ] **Step 6: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

- [ ] **Step 7: Ask whether to commit**

If yes:

```bash
git add crates/engine/src/research.rs crates/engine/src/lib.rs
git commit -m "feat: add ResearchDb loader with load-time tree validation"
```

---

## Task 3: The tree content and its modder documentation

**Files:**
- Create: `assets/research/automation.ron`, `power_grid.ron`, `commerce.ron`, `fortification.ron`, `weapon_bench.ron`, `armor_bench.ron`, `cold_storage.ron`, `overclock.ron`, `firewall.ron`, `neural_amp.ron`, `monofilament.ron`, `ablative.ron`, `cortex.ron`
- Create: `assets/research/README.md`
- Test: `crates/engine/src/research.rs` (new test in existing `mod tests`)

**Interfaces:**
- Consumes: `ResearchDb::load_dir` (Task 2).
- Produces: 13 node ids the later tasks' tests reference by name: `automation`, `power_grid`, `commerce`, `fortification`, `weapon_bench`, `armor_bench`, `cold_storage`, `overclock`, `firewall`, `neural_amp`, `monofilament`, `ablative`, `cortex`.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/research.rs`'s `mod tests`, add:

```rust
#[test]
fn the_shipped_tree_loads_clean() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
    let (db, warnings) = ResearchDb::load_dir(&assets.join("research"), &structures).unwrap();
    assert!(
        warnings.is_empty(),
        "the shipped tree must not warn: {warnings:?}"
    );
    assert_eq!(db.all().count(), 13, "13 nodes ship with the game");
    assert_eq!(
        db.get("cortex").map(|d| d.cost),
        Some(45),
        "cortex is the deepest node"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine --lib research::the_shipped_tree_loads_clean`

Expected: FAIL — `No such file or directory` for `assets/research`.

- [ ] **Step 3: Create the structure-unlocking nodes**

`assets/research/automation.ron`:

```ron
(
    id: "automation",
    name: "Automation",
    description: "Self-running compile jobs. Unlocks the Compiler.",
    cost: 8,
    unlocks_structures: ["compiler"],
)
```

`assets/research/power_grid.ron`:

```ron
(
    id: "power_grid",
    name: "Power Grid",
    description: "Routed power. Unlocks the Terminal and the Power Conduit.",
    cost: 10,
    unlocks_structures: ["terminal", "power_conduit"],
)
```

`assets/research/commerce.ron`:

```ron
(
    id: "commerce",
    name: "Isometric Commerce",
    description: "Barter protocols. Unlocks the iso Market.",
    cost: 12,
    unlocks_structures: ["market"],
)
```

Note the id is `market`, not `black_market` — `assets/structures/black_market.ron` declares `id: "market"`, and the id is what matters.

`assets/research/fortification.ron`:

```ron
(
    id: "fortification",
    name: "Fortification",
    description: "Automated perimeter defense. Unlocks the Turret.",
    cost: 15,
    requires: ["power_grid"],
    unlocks_structures: ["turret"],
)
```

`assets/research/weapon_bench.ron`:

```ron
(
    id: "weapon_bench",
    name: "Weapon Fabrication",
    description: "A bench for weapon and module work. Unlocks the Fabricator.",
    cost: 18,
    requires: ["automation"],
    unlocks_structures: ["fabricator"],
)
```

`assets/research/armor_bench.ron`:

```ron
(
    id: "armor_bench",
    name: "Reactive Armor",
    description: "A bench for plating work. Unlocks the Armory.",
    cost: 18,
    requires: ["automation"],
    unlocks_structures: ["armory"],
)
```

`assets/research/cold_storage.ron`:

```ron
(
    id: "cold_storage",
    name: "Cold Storage",
    description: "Persistent local storage. Unlocks the Data Cache.",
    cost: 20,
    requires: ["commerce"],
    unlocks_structures: ["data_cache"],
)
```

- [ ] **Step 4: Create the recipe-unlocking nodes**

`assets/research/overclock.ron`:

```ron
(
    id: "overclock",
    name: "Overclock Cores",
    description: "Compile Overclock Cores at a Fabricator.",
    cost: 22,
    requires: ["weapon_bench"],
    unlocks_recipes: [(
        result: OverclockCore,
        cost: [(PortalFragment, 6)],
        requires_structure: Some("fabricator"),
    )],
)
```

`assets/research/firewall.ron`:

```ron
(
    id: "firewall",
    name: "Firewall Plating",
    description: "Compile Firewall Plating at an Armory.",
    cost: 22,
    requires: ["armor_bench"],
    unlocks_recipes: [(
        result: FirewallPlating,
        cost: [(PortalFragment, 6)],
        requires_structure: Some("armory"),
    )],
)
```

`assets/research/neural_amp.ron`:

```ron
(
    id: "neural_amp",
    name: "Neural Interfacing",
    description: "Compile Neural Amplifiers at a Fabricator.",
    cost: 25,
    requires: ["weapon_bench"],
    unlocks_recipes: [(
        result: NeuralAmplifier,
        cost: [(PortalFragment, 6)],
        requires_structure: Some("fabricator"),
    )],
)
```

`assets/research/monofilament.ron`:

```ron
(
    id: "monofilament",
    name: "Monofilament Edge",
    description: "Compile Monofilament Whips at a Fabricator.",
    cost: 40,
    requires: ["overclock"],
    unlocks_recipes: [(
        result: MonofilamentWhip,
        cost: [(PortalFragment, 12)],
        requires_structure: Some("fabricator"),
    )],
)
```

`assets/research/ablative.ron`:

```ron
(
    id: "ablative",
    name: "Ablative Lattice",
    description: "Compile Ablative Plating at an Armory.",
    cost: 40,
    requires: ["firewall"],
    unlocks_recipes: [(
        result: AblativePlating,
        cost: [(PortalFragment, 12)],
        requires_structure: Some("armory"),
    )],
)
```

`assets/research/cortex.ron`:

```ron
(
    id: "cortex",
    name: "Cortex Hacking",
    description: "Compile Cortex Hacks at a Fabricator.",
    cost: 45,
    requires: ["neural_amp"],
    unlocks_recipes: [(
        result: CortexHack,
        cost: [(PortalFragment, 12)],
        requires_structure: Some("fabricator"),
    )],
)
```

- [ ] **Step 5: Write the modder README**

Create `assets/research/README.md`:

````markdown
# Research nodes

Every `*.ron` file in this directory is one node of the research tree. Drop
a file in, it becomes a node — no code change needed. A malformed file is
skipped with a warning in the message log rather than crashing startup.

Research Data is the currency. It comes from a Research Node structure
worked by an assigned tamed program.

## Schema

```ron
(
    // Unique id. Other nodes reference this in their `requires`.
    id: "weapon_bench",

    // Shown in the research menu.
    name: "Weapon Fabrication",
    description: "A bench for weapon and module work. Unlocks the Fabricator.",

    // Research Data spent to unlock this node.
    cost: 18,

    // Optional. Node ids that must be unlocked first. Defaults to none.
    requires: ["automation"],

    // Optional. Structure ids this node makes buildable. Defaults to none.
    // A structure named by NO research file is buildable from the start.
    unlocks_structures: ["fabricator"],

    // Optional. Craft recipes this node makes available. Defaults to none.
    unlocks_recipes: [(
        // An ItemId — see crates/engine/src/items.rs.
        result: OverclockCore,
        // What one unit costs, as (ItemId, quantity) pairs.
        cost: [(PortalFragment, 6)],
        // Optional. The recipe only appears while a structure of this kind
        // is deployed. Researching the blueprint isn't enough on its own.
        requires_structure: Some("fabricator"),
    )],
)
```

## Rules

- **A structure named by no research file is buildable by default.** That's
  how Home, the Mining Node, the Research Node, the Recharger Node and the
  Zone Portal stay available from turn one, and it means a structure mod
  that ships no research file keeps working unchanged.
- A node naming an unknown prerequisite, or an unknown structure in
  `unlocks_structures`, is dropped at load time with a warning — it could
  never be reached or acted on. Dropping cascades to anything that required
  it.
- The ICE Breaker and Power Cell recipes are always available and are not
  defined here.
- Nodes are listed cheapest-first, ties broken by id.
````

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine --lib research::the_shipped_tree_loads_clean`

Expected: PASS.

- [ ] **Step 7: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

- [ ] **Step 8: Ask whether to commit**

If yes:

```bash
git add assets/research crates/engine/src/research.rs
git commit -m "feat: add the 13-node research tree and its modder docs"
```

---

## Task 4: Unlock state and the `Game` research API

**Files:**
- Modify: `crates/engine/src/resources.rs` (add `Research`)
- Modify: `crates/engine/src/lib.rs` — imports, re-exports, `Game::new` (~`:463-483`), `Game::load` (~`:538-558`), and a new methods block near `craft_recipes` (`:1288`)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `ResearchDb`, `ResearchDef` (Task 2); the shipped node ids (Task 3); `ItemId::ResearchData` (Task 1).
- Produces:
  - `resources::Research(pub HashSet<ResearchId>)`
  - `Game::is_researched(&self, id: &str) -> bool`
  - `Game::research_nodes(&self) -> Vec<ResearchStatus>`
  - `Game::unlock_research(&mut self, id: &str) -> Result<(), String>`
  - `pub struct ResearchStatus { id: ResearchId, name: String, description: String, cost: u32, state: ResearchState, affordable: bool }`
  - `pub enum ResearchState { Unlocked, Available, Locked { missing: Vec<String> } }`

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/lib.rs`'s `mod tests`, add:

```rust
/// Gives the player exactly `n` Research Data, bypassing the Research Node
/// so the test doesn't depend on tick timing or a tamed worker.
fn grant_research_data(game: &mut Game, n: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::ResearchData, n);
}

#[test]
fn nothing_is_researched_at_the_start_of_a_game() {
    let game = test_game();
    assert!(!game.is_researched("automation"));
    assert!(
        game.research_nodes().iter().all(|n| n.state != ResearchState::Unlocked),
        "a fresh game starts with an entirely locked tree"
    );
}

#[test]
fn unlocking_research_consumes_exactly_its_cost() {
    let mut game = test_game();
    grant_research_data(&mut game, 20);
    game.unlock_research("automation").unwrap();
    assert!(game.is_researched("automation"));
    let left = game
        .player_status()
        .inventory
        .iter()
        .find(|(i, _)| *i == ItemId::ResearchData)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    assert_eq!(left, 12, "automation costs 8 of the 20 granted");
}

#[test]
fn unlocking_research_fails_without_enough_research_data() {
    let mut game = test_game();
    grant_research_data(&mut game, 7);
    let err = game.unlock_research("automation").unwrap_err();
    assert!(err.contains("Research Data"), "got: {err}");
    assert!(!game.is_researched("automation"));
}

#[test]
fn unlocking_research_fails_while_a_prerequisite_is_missing() {
    let mut game = test_game();
    grant_research_data(&mut game, 500);
    let err = game.unlock_research("weapon_bench").unwrap_err();
    assert!(
        err.contains("Automation"),
        "the error should name the missing prereq: {err}"
    );
    assert!(!game.is_researched("weapon_bench"));
}

#[test]
fn a_locked_node_reports_which_prerequisites_are_missing() {
    let game = test_game();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| n.id == "weapon_bench")
        .unwrap();
    assert_eq!(
        node.state,
        ResearchState::Locked {
            missing: vec!["Automation".to_string()]
        }
    );
}

#[test]
fn a_prerequisite_free_node_is_available_immediately() {
    let game = test_game();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| n.id == "automation")
        .unwrap();
    assert_eq!(node.state, ResearchState::Available);
    assert!(
        !node.affordable,
        "available is about prereqs; affordability is separate"
    );
}

#[test]
fn researching_the_same_node_twice_is_rejected() {
    let mut game = test_game();
    grant_research_data(&mut game, 40);
    game.unlock_research("automation").unwrap();
    let err = game.unlock_research("automation").unwrap_err();
    assert!(err.contains("already"), "got: {err}");
}

#[test]
fn unknown_research_is_rejected() {
    let mut game = test_game();
    assert!(game.unlock_research("not_a_node").is_err());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine --lib research`

Expected: FAIL to compile — `no method named is_researched found for struct Game`.

- [ ] **Step 3: Add the `Research` resource**

In `crates/engine/src/resources.rs`, add near the other small resources:

```rust
/// Which research nodes the player has unlocked (see `research::ResearchDb`).
/// Empty at the start of a run — every node in the tree begins locked.
#[derive(Resource, Default)]
pub struct Research(pub std::collections::HashSet<crate::research::ResearchId>);
```

If `resources.rs` doesn't already `use bevy_ecs::prelude::Resource;`, add that import.

- [ ] **Step 4: Wire the resource and db into `Game::new` and `Game::load`**

In `crates/engine/src/lib.rs`, add to the imports near the `structures` import:

```rust
use research::{ResearchDb, ResearchDef, ResearchId};
```

and add `Research` to the existing `use resources::{...}` list.

In `Game::new`, after the `structure_db` load and before `world` is built:

```rust
        let (research_db, research_warnings) =
            ResearchDb::load_dir(&assets_dir.join("research"), &structure_db)?;
        load_warnings.extend(research_warnings);
```

and alongside the other `world.insert_resource` calls:

```rust
        world.insert_resource(research_db);
        world.insert_resource(Research::default());
```

Make the identical two additions in `Game::load`, except the resource insert restores the saved set — Task 7 changes that line; for now insert `Research::default()` in both.

- [ ] **Step 5: Add the view types and the three methods**

In `crates/engine/src/lib.rs`, add near the other view structs (e.g. beside `PlayerStatus`):

```rust
/// One node of the research tree as the menus see it — see
/// `Game::research_nodes`.
pub struct ResearchStatus {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub state: ResearchState,
    /// Whether the player can pay `cost` right now. Independent of `state`:
    /// a node can be `Available` but unaffordable, or affordable but
    /// `Locked`.
    pub affordable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchState {
    Unlocked,
    Available,
    /// Display names of the prerequisites still missing — the menu shows
    /// *why* a node can't be taken rather than just greying it out.
    Locked { missing: Vec<String> },
}
```

Add `research::{ResearchDef, ResearchId}` types to the `pub use` block so app-core and the renderers can name them:

```rust
pub use research::{ResearchDef, ResearchId, ResearchRecipe};
```

Then add these methods to `impl Game`, immediately before `craft_recipes`:

```rust
    pub fn is_researched(&self, id: &str) -> bool {
        self.world.resource::<Research>().0.contains(id)
    }

    /// Display names of `def`'s prerequisites that aren't unlocked yet, in
    /// the order the file lists them.
    fn missing_prereqs(&self, def: &ResearchDef) -> Vec<String> {
        let db = self.world.resource::<ResearchDb>();
        def.requires
            .iter()
            .filter(|id| !self.is_researched(id))
            .map(|id| {
                db.get(id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| id.clone())
            })
            .collect()
    }

    /// Every research node, ordered the way the menu shows them: available
    /// first, then locked, then already-unlocked, each group cheapest-first
    /// (see `ResearchDb::all`). Ordering lives here rather than in each
    /// renderer so both peers agree on what `[3]` means.
    pub fn research_nodes(&self) -> Vec<ResearchStatus> {
        let held = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(ItemId::ResearchData))
            .unwrap_or(0);
        let mut nodes: Vec<ResearchStatus> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .map(|def| {
                let state = if self.is_researched(&def.id) {
                    ResearchState::Unlocked
                } else {
                    let missing = self.missing_prereqs(def);
                    if missing.is_empty() {
                        ResearchState::Available
                    } else {
                        ResearchState::Locked { missing }
                    }
                };
                ResearchStatus {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    cost: def.cost,
                    state,
                    affordable: held >= def.cost,
                }
            })
            .collect();
        let rank = |s: &ResearchState| match s {
            ResearchState::Available => 0,
            ResearchState::Locked { .. } => 1,
            ResearchState::Unlocked => 2,
        };
        nodes.sort_by_key(|n| rank(&n.state));
        nodes
    }

    /// Unlocks `id`, consuming its Research Data cost. Fails with an
    /// explicit message when the id is unknown, it's already unlocked, a
    /// prerequisite is missing, or the player can't pay.
    pub fn unlock_research(&mut self, id: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let def = self
            .world
            .resource::<ResearchDb>()
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown research.".to_string())?;
        if self.is_researched(id) {
            return Err(format!("{} is already researched.", def.name));
        }
        let missing = self.missing_prereqs(&def);
        if !missing.is_empty() {
            return Err(format!("Requires {} first.", missing.join(", ")));
        }
        let player = self.player_entity();
        let held = self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::ResearchData);
        if held < def.cost {
            return Err(format!(
                "Not enough Research Data ({held}/{}).",
                def.cost
            ));
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ItemId::ResearchData, def.cost);
        self.world.resource_mut::<Research>().0.insert(def.id.clone());
        self.log(format!("Research complete: {}.", def.name));
        Ok(())
    }
```

`ResearchDb::all()` returns `impl Iterator` built from a sorted `Vec`, so `sort_by_key` on the rank alone preserves cheapest-first inside each group — `slice::sort_by_key` is stable.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine --lib research`

Expected: PASS.

- [ ] **Step 7: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

- [ ] **Step 8: Ask whether to commit**

If yes:

```bash
git add crates/engine/src/resources.rs crates/engine/src/lib.rs
git commit -m "feat: add Research resource and Game research API"
```

---

## Task 5: Gate structures behind research

**Files:**
- Modify: `crates/engine/src/lib.rs` — new `buildable_structure_defs` beside `structure_defs` (`:4214`), guard in `place_structure` (`:1608`)
- Modify: `crates/app-core/src/lib.rs:826`
- Modify: `crates/tui/src/ui.rs:522`
- Modify: `crates/gui/src/render.rs:602`
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Game::is_researched` (Task 4), the shipped node ids (Task 3).
- Produces: `Game::buildable_structure_defs(&self) -> Vec<StructureDef>`. `Game::structure_defs` keeps its existing unfiltered behavior — it's the lookup used all over the engine tests, not a menu.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/lib.rs`'s `mod tests`, add:

```rust
#[test]
fn a_structure_named_by_no_research_file_is_buildable_from_the_start() {
    let game = test_game();
    let ids: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    for id in ["home", "mining_node", "research_node", "recharger_node", "portal"] {
        assert!(
            ids.contains(&id.to_string()),
            "{id} is named by no research file and must stay available"
        );
    }
}

#[test]
fn a_research_gated_structure_is_hidden_from_the_build_menu_until_researched() {
    let mut game = test_game();
    let hidden: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(!hidden.contains(&"fabricator".to_string()));

    grant_research_data(&mut game, 40);
    game.unlock_research("automation").unwrap();
    game.unlock_research("weapon_bench").unwrap();

    let shown: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(shown.contains(&"fabricator".to_string()));
}

#[test]
fn placing_an_unresearched_structure_is_rejected_even_when_called_directly() {
    let mut game = test_game();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::CoreFragment, 200);
    game.place_structure("home", 1, 0).unwrap();
    let err = game.place_structure("fabricator", 0, 1).unwrap_err();
    assert!(
        err.contains("researched"),
        "filtering the menu is not a gate: {err}"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine --lib buildable`

Expected: FAIL to compile — `no method named buildable_structure_defs found for struct Game`.

- [ ] **Step 3: Add the gating helper and the filtered accessor**

In `crates/engine/src/lib.rs`, immediately after `structure_defs`:

```rust
    /// Whether `structure_id` may be built right now. A structure named by
    /// no research file is unlocked by default — that's what keeps Home, the
    /// Mining Node, the Research Node, the Recharger Node and the Zone
    /// Portal available from turn one without a hardcoded whitelist, and
    /// what keeps a structure mod that ships no research file working
    /// unchanged.
    fn structure_unlocked(&self, structure_id: &str) -> bool {
        let db = self.world.resource::<ResearchDb>();
        let mut gates = db
            .all()
            .filter(|def| def.unlocks_structures.iter().any(|s| s == structure_id))
            .peekable();
        if gates.peek().is_none() {
            return true;
        }
        gates.any(|def| self.is_researched(&def.id))
    }

    /// The structures the build menu offers: `structure_defs` minus anything
    /// still behind unfinished research. `structure_defs` itself stays
    /// unfiltered — it's the general lookup, not the menu.
    pub fn buildable_structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| self.structure_unlocked(&def.id))
            .cloned()
            .collect()
    }
```

- [ ] **Step 4: Enforce it in `place_structure`**

In `crates/engine/src/lib.rs`, in `place_structure`, immediately after the `def` lookup (`ok_or_else(|| "Unknown structure".to_string())?;`) and before the Home checks:

```rust
        if !self.structure_unlocked(structure_id) {
            return Err(format!("{} hasn't been researched yet.", def.name));
        }
```

- [ ] **Step 5: Point the three build menus at the filtered list**

- `crates/app-core/src/lib.rs:826` — change `let defs = game.structure_defs();` to `let defs = game.buildable_structure_defs();`
- `crates/tui/src/ui.rs:522` — same change
- `crates/gui/src/render.rs:602` — same change

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine --lib buildable`

Expected: PASS.

- [ ] **Step 7: Fix the fallout in existing tests**

Run: `cargo test --workspace`

Existing engine tests that build an Armory or Fabricator (`crates/engine/src/lib.rs:4663`, `:4676`, `:4712`, `:4718`, `:4731`, `:4774-4775`, `:4812`, `:4840`) now fail — those structures are research-gated. Add the research first in each, using the existing `grant_research_data` helper:

```rust
    grant_research_data(&mut game, 60);
    game.unlock_research("automation").unwrap();
    game.unlock_research("armor_bench").unwrap();   // for "armory"
    game.unlock_research("weapon_bench").unwrap();  // for "fabricator"
```

`crates/app-core/src/lib.rs:1595` asserts a build-menu count against `structure_defs().len()`; change it to `buildable_structure_defs().len()` so it measures the same list the menu shows.

Work through the failures one at a time and re-run until green. Do not weaken an assertion to make it pass — if a test breaks for a reason other than "this structure now needs research", stop and investigate.

- [ ] **Step 8: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

- [ ] **Step 9: Ask whether to commit**

If yes:

```bash
git add crates/engine/src/lib.rs crates/app-core/src/lib.rs crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "feat: gate structure building behind research"
```

---

## Task 6: Gate craft recipes behind research

**Files:**
- Modify: `crates/engine/src/lib.rs:53-57` (delete two constants), `:1278-1312` (`craft_recipes`), `:1314-1322` (`has_structure` doc)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Game::is_researched` (Task 4), `ResearchRecipe` (Task 2), the recipe nodes (Task 3).
- Produces: `Game::craft_recipes` sourced from unlocked research. `FIREWALL_PLATING_PORTAL_COST` and `OVERCLOCK_CORE_PORTAL_COST` no longer exist.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/lib.rs`'s `mod tests`, add:

```rust
#[test]
fn the_two_starter_recipes_need_no_research() {
    let game = test_game();
    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(results.contains(&ItemId::IceBreaker));
    assert!(results.contains(&ItemId::PowerCell));
    assert_eq!(results.len(), 2, "nothing else is free");
}

#[test]
fn a_researched_recipe_stays_hidden_until_its_bench_is_built() {
    let mut game = test_game();
    grant_research_data(&mut game, 80);
    game.unlock_research("automation").unwrap();
    game.unlock_research("weapon_bench").unwrap();
    game.unlock_research("overclock").unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::OverclockCore),
        "the blueprint alone isn't enough — you still need the Fabricator"
    );

    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::CoreFragment, 200);
    game.place_structure("home", 1, 0).unwrap();
    game.place_structure("fabricator", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(results.contains(&ItemId::OverclockCore));
}

#[test]
fn a_built_bench_alone_does_not_unlock_its_recipe() {
    let mut game = test_game();
    grant_research_data(&mut game, 80);
    game.unlock_research("automation").unwrap();
    game.unlock_research("weapon_bench").unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::CoreFragment, 200);
    game.place_structure("home", 1, 0).unwrap();
    game.place_structure("fabricator", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::OverclockCore),
        "the Fabricator is a bench now, not an unlock"
    );
}

#[test]
fn a_researched_recipe_carries_the_cost_from_its_ron_file() {
    let mut game = test_game();
    grant_research_data(&mut game, 80);
    game.unlock_research("automation").unwrap();
    game.unlock_research("weapon_bench").unwrap();
    game.unlock_research("overclock").unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::CoreFragment, 200);
    game.place_structure("home", 1, 0).unwrap();
    game.place_structure("fabricator", 0, 1).unwrap();

    assert_eq!(
        game.craft_cost(ItemId::OverclockCore),
        vec![(ItemId::PortalFragment, 6)]
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine --lib recipe`

Expected: FAIL — `the_two_starter_recipes_need_no_research` passes by luck, but `a_researched_recipe_stays_hidden_until_its_bench_is_built` fails because `unlock_research` doesn't influence `craft_recipes` yet.

- [ ] **Step 3: Rewrite `craft_recipes`**

In `crates/engine/src/lib.rs`, replace the whole `craft_recipes` method (doc comment included) with:

```rust
    /// The full list of things the player can compile right now: the two
    /// always-available starter recipes, plus every recipe from an unlocked
    /// research node whose bench (`ResearchRecipe::requires_structure`) is
    /// currently deployed. Recipe data lives in `assets/research/*.ron` so a
    /// mod can add one without touching Rust — only `ItemId` itself is a
    /// hardcoded enum (see `CLAUDE.md`).
    pub fn craft_recipes(&self) -> Vec<CraftRecipe> {
        let mut recipes = vec![
            CraftRecipe {
                result: ItemId::IceBreaker,
                cost: vec![(ItemId::CoreFragment, ICE_BREAKER_CORE_COST)],
            },
            CraftRecipe {
                result: ItemId::PowerCell,
                cost: vec![(ItemId::CoreFragment, POWER_CELL_CORE_COST)],
            },
        ];
        let unlocked: Vec<&ResearchDef> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .filter(|def| self.is_researched(&def.id))
            .collect();
        for def in unlocked {
            for recipe in &def.unlocks_recipes {
                let bench_ready = recipe
                    .requires_structure
                    .as_ref()
                    .is_none_or(|s| self.has_structure(s));
                if bench_ready {
                    recipes.push(CraftRecipe {
                        result: recipe.result,
                        cost: recipe.cost.clone(),
                    });
                }
            }
        }
        recipes
    }
```

If clippy rejects `is_none_or` on this toolchain, use `map_or(true, |s| self.has_structure(s))` and re-run clippy.

- [ ] **Step 4: Delete the dead constants**

In `crates/engine/src/lib.rs`, delete `FIREWALL_PLATING_PORTAL_COST` (`:53`) and `OVERCLOCK_CORE_PORTAL_COST` (`:57`) along with their doc comments. Keep `ICE_BREAKER_CORE_COST` and `POWER_CELL_CORE_COST` — the starter recipes still use them.

Update `has_structure`'s doc comment: it currently says it's "used to gate workbench-unlocked craft recipes (see `craft_recipes`)". It now backs `ResearchRecipe::requires_structure` instead. Reword it accordingly.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine --lib recipe`

Expected: PASS.

- [ ] **Step 6: Fix the fallout in existing tests**

Run: `cargo test --workspace`

`building_an_armory_unlocks_firewall_plating_crafting_for_portal_fragments` (`crates/engine/src/lib.rs:4826`) encodes exactly the behavior this task removes, and `:4849` references the deleted `FIREWALL_PLATING_PORTAL_COST`. Rewrite it as `researching_and_building_an_armory_unlocks_firewall_plating`: grant Research Data, unlock `automation` then `armor_bench` then `firewall`, build Home and the Armory, then assert Firewall Plating appears with cost `vec![(ItemId::PortalFragment, 6)]`.

Work through any other failures one at a time. Do not weaken an assertion to make it pass.

- [ ] **Step 7: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

- [ ] **Step 8: Ask whether to commit**

If yes:

```bash
git add crates/engine/src/lib.rs
git commit -m "feat: source craft recipes from unlocked research"
```

---

## Task 7: Persist unlocked research

**Files:**
- Modify: `crates/engine/src/save.rs` — `SaveData` (`:124-138`), `SAVE_FORMAT_VERSION` (`:164`), `sample_data()` in `mod tests`
- Modify: `crates/engine/src/lib.rs` — `Game::save` `SaveData` construction (`:869`), `Game::load` `Research` insert (Task 4, Step 4)
- Test: `crates/engine/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Research` resource (Task 4).
- Produces: `SaveData.researched: Vec<ResearchId>`; `SAVE_FORMAT_VERSION == 7`.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/lib.rs`'s `mod tests`, add:

```rust
#[test]
fn a_save_round_trip_preserves_unlocked_research() {
    let mut game = test_game();
    grant_research_data(&mut game, 40);
    game.unlock_research("automation").unwrap();
    game.unlock_research("weapon_bench").unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_research_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(loaded.is_researched("automation"));
    assert!(loaded.is_researched("weapon_bench"));
    assert!(!loaded.is_researched("commerce"));
}
```

Match the assets-path expression the existing save round-trip test in `mod tests` uses, if it differs.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine --lib a_save_round_trip_preserves_unlocked_research`

Expected: FAIL — both `is_researched` assertions are false; the set isn't persisted.

- [ ] **Step 3: Add the save field and bump the version**

In `crates/engine/src/save.rs`, add to `SaveData` after `spawn_point`:

```rust
    /// Which research nodes have been unlocked — see `research::ResearchDb`.
    /// Sorted on write so the encoded bytes don't depend on `HashSet`
    /// iteration order.
    pub researched: Vec<crate::research::ResearchId>,
```

Change `pub const SAVE_FORMAT_VERSION: u32 = 6;` to `= 7;`. Its doc comment already explains that any shape change means a bump — leave that text alone.

Add `researched: Vec::new(),` to `sample_data()` in `save.rs`'s `mod tests`.

- [ ] **Step 4: Write and restore the field**

In `crates/engine/src/lib.rs`'s `Game::save`, add to the `save::SaveData { ... }` literal, after `spawn_point`:

```rust
            researched: {
                let mut ids: Vec<ResearchId> =
                    self.world.resource::<Research>().0.iter().cloned().collect();
                ids.sort();
                ids
            },
```

In `Game::load`, change the `world.insert_resource(Research::default());` line added in Task 4 to:

```rust
        world.insert_resource(Research(data.researched.into_iter().collect()));
```

Place it after `data` is destructured but before `data` is otherwise consumed — if the borrow checker complains about `data` being partially moved, move this line earlier, next to the other `data.*` reads.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine --lib a_save_round_trip_preserves_unlocked_research`

Expected: PASS.

- [ ] **Step 6: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Any existing test constructing a `SaveData` literal needs the new field. Add `researched: Vec::new(),` to each.

- [ ] **Step 7: Ask whether to commit**

If yes:

```bash
git add crates/engine/src/save.rs crates/engine/src/lib.rs
git commit -m "feat: persist unlocked research; bump save format to v7"
```

---

## Task 8: The research menu in both renderers

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode` (`:71-117`), key dispatch (`:456`), `T` binding (near `:631`), new `handle_research_key`
- Modify: `crates/tui/src/ui.rs` — mode passthrough (`:65`), popup dispatch (`:197`), new `render_research_menu`, help text (`:1844`)
- Modify: `crates/gui/src/render.rs` — popup dispatch (`:591`), new `draw_research_menu`, help text (`:1661`)
- Test: `crates/app-core/src/lib.rs` (existing `mod tests`)

**Interfaces:**
- Consumes: `Game::research_nodes`, `Game::unlock_research`, `ResearchStatus`, `ResearchState` (Task 4).
- Produces: `Mode::Research`, reachable with `T` from `Mode::Playing`.

- [ ] **Step 1: Write the failing test**

In `crates/app-core/src/lib.rs`'s `mod tests`, add:

```rust
#[test]
fn t_opens_the_research_menu_and_esc_closes_it() {
    let mut app = started_app();
    app.handle_key(GameKey::Char('T'));
    assert!(matches!(app.mode, Mode::Research));
    app.handle_key(GameKey::Esc);
    assert!(matches!(app.mode, Mode::Playing));
}

#[test]
fn picking_an_unaffordable_research_node_reports_why_and_stays_open() {
    let mut app = started_app();
    app.handle_key(GameKey::Char('T'));
    app.handle_key(GameKey::Char('1'));
    assert!(
        matches!(app.mode, Mode::Research),
        "the menu stays open so several nodes can be taken in one visit"
    );
    assert!(
        app.status_line
            .as_ref()
            .is_some_and(|s| s.contains("Research Data")),
        "got: {:?}",
        app.status_line
    );
}
```

Use whatever helper the existing app-core tests use to build a started `App` in place of `started_app()` — check how the tests near `crates/app-core/src/lib.rs:1536` set one up.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-app-core research`

Expected: FAIL to compile — `no variant named Research found for enum Mode`.

- [ ] **Step 3: Add the mode, the key, and the handler**

In `crates/app-core/src/lib.rs`, add to `enum Mode`, after `Perks,`:

```rust
    /// The research tree (see `Game::research_nodes`). Stays open after each
    /// unlock so several nodes can be taken in one visit.
    Research,
```

Add to the key dispatch beside `Mode::Perks => self.handle_perks_key(key),`:

```rust
            Mode::Research => self.handle_research_key(key),
```

Add to the `Mode::Playing` key matches, beside the other menu openers:

```rust
            GameKey::Char('T') => {
                self.mode = Mode::Research;
                return;
            }
```

Add the handler next to `handle_perks_key`:

```rust
    /// Picks a numbered research node to unlock; stays open so several can
    /// be taken in one visit.
    fn handle_research_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(ids) = self.game.as_ref().map(|g| {
            g.research_nodes()
                .into_iter()
                .map(|n| n.id)
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, ids.len()) {
            let id = ids[idx].clone();
            let Some(game) = &mut self.game else { return };
            match game.unlock_research(&id) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }
```

Binding `ids` through `map` on `as_ref()` rather than a `let Some(game) = &self.game` block matters: the latter holds the borrow to the end of the function and collides with `self.selected_index`'s `&mut self`.

- [ ] **Step 4: Render it in the TUI**

In `crates/tui/src/ui.rs`, add `| Mode::Research` to the mode group at `:65` that routes to `render_playing`, and add to the popup dispatch at `:197`:

```rust
        Mode::Research => render_research_menu(f, area, game, selected),
```

Add the renderer next to `render_perks_menu`:

```rust
fn render_research_menu(f: &mut Frame, area: Rect, game: &mut Game, selected: usize) {
    use feral_processes_engine::ResearchState;

    let popup = centered_rect(70, 65, area);
    f.render_widget(Clear, popup);
    let held = game
        .player_status()
        .inventory
        .iter()
        .find(|(item, _)| *item == ItemId::ResearchData)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let nodes = game.research_nodes();
    let mut lines = vec![
        Line::styled(
            format!("Research Data: {held}"),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let (tag, mut style) = match &node.state {
            ResearchState::Unlocked => (" (researched)".to_string(), Style::new().fg(Color::Green)),
            ResearchState::Available if node.affordable => (String::new(), Style::new()),
            ResearchState::Available => (String::new(), Style::new().fg(Color::DarkGray)),
            ResearchState::Locked { missing } => (
                format!(" (needs {})", missing.join(", ")),
                Style::new().fg(Color::DarkGray),
            ),
        };
        let prefix = if i == selected {
            style = style.add_modifier(Modifier::REVERSED);
            "> "
        } else {
            "  "
        };
        lines.push(Line::styled(
            format!(
                "{prefix}[{}] {} — {} Research Data{tag}",
                i + 1,
                node.name,
                node.cost
            ),
            style,
        ));
        lines.push(Line::from(format!("    {}", node.description)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Pick a number to research it (Up/Down + Enter also work). Esc to close",
    ));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title("Research")),
        popup,
    );
}
```

Match the exact `Paragraph`/`Block` builder call `render_perks_menu` uses at `:1283-1287`, including whether it sets `.wrap(...)`; copy that shape rather than the sketch above if it differs. Add `ItemId` to the file's imports if it isn't already there.

- [ ] **Step 5: Add the TUI help line**

In `crates/tui/src/ui.rs`'s `render_help` (`:1844`), add after the `v` line:

```rust
        Line::from("T                   research tree: spend Research Data to unlock structures and recipes"),
```

- [ ] **Step 6: Render it in the GUI**

In `crates/gui/src/render.rs`, add to the popup dispatch at `:591`:

```rust
        Mode::Research => draw_research_menu(game, selected),
```

Add the renderer next to `draw_perks_menu`:

```rust
fn draw_research_menu(game: &mut Game, selected: usize) {
    use feral_processes_engine::ResearchState;

    let held = game
        .player_status()
        .inventory
        .iter()
        .find(|(item, _)| *item == ItemId::ResearchData)
        .map(|(_, n)| *n)
        .unwrap_or(0);
    let nodes = game.research_nodes();
    let mut rows = vec![
        Row::TextColored(format!("Research Data: {held}"), CYAN),
        text_row(""),
    ];
    for (i, node) in nodes.iter().enumerate() {
        let tag = match &node.state {
            ResearchState::Unlocked => " (researched)".to_string(),
            ResearchState::Available => String::new(),
            ResearchState::Locked { missing } => format!(" (needs {})", missing.join(", ")),
        };
        rows.push(item_row(
            format!(
                "[{}] {} - {} Research Data{tag}",
                i + 1,
                node.name,
                node.cost
            ),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", node.description)));
    }
    rows.push(text_row(""));
    rows.push(text_row("Pick a number to research it. Esc to close"));
    draw_popup("Research", PopupSize::Large, &rows);
}
```

Add `ItemId` to the file's imports if it isn't already there.

- [ ] **Step 7: Add the GUI help line**

In `crates/gui/src/render.rs`'s `draw_help` (`:1661`), change:

```rust
        text_row("f fuse   t trade   x perks   s save   q main menu"),
```

to:

```rust
        text_row("f fuse   t trade   x perks   T research   s save   q main menu"),
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-app-core research`

Expected: PASS.

- [ ] **Step 9: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings, all tests pass.

- [ ] **Step 10: Play-test**

Run: `cargo run -p feral-processes`

Confirm by hand: `T` opens the Research menu with 13 nodes and `Research Data: 0`; the three root nodes read as available and everything else shows a `(needs ...)` tag; `b` lists Home, Mining Node, Research Node, Recharger Node and Zone Portal but no Compiler, Fabricator, Armory, Turret, Terminal, Power Conduit, iso Market or Data Cache; `c` lists only ICE Breaker and Power Cell. Report what you actually saw.

- [ ] **Step 11: Ask whether to commit**

If yes:

```bash
git add crates/app-core/src/lib.rs crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "feat: add the research tree menu to both renderers"
```

---

## Self-review notes

**Spec coverage:** Research Data economy → Task 1. Data model → Task 2. The tree → Task 3. Unlock state and engine API → Task 4. Structure gating (both menu and `place_structure`) → Task 5. Recipe gating and constant deletion → Task 6. Save format → Task 7. UI in both renderers → Task 8. Documentation → Tasks 1 and 3. The spec's two factual errors (save version, `difficulty.rs`) are corrected in Global Constraints.

**Naming consistency:** `structure_unlocked` (private, Task 5) and `missing_prereqs` (private, Task 4) are each defined once and used under exactly that name afterward. `buildable_structure_defs` is the filtered menu accessor throughout; `structure_defs` stays the unfiltered lookup. `grant_research_data` is defined in Task 4 Step 1 and reused by Tasks 5, 6 and 7.
