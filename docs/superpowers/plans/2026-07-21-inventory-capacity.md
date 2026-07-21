# Inventory Capacity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cap the player's carried inventory at 20 units, expandable by 10 per deployed Data Cache, with Research Data exempted into a separate 200-unit bank.

**Architecture:** Capacity is derived on every read from `BASE_INVENTORY_CAPACITY` plus a new `inventory_bonus` field summed over deployed structures — never stored, so a destroyed Data Cache shrinks the buffer with no invalidation step and the save format is untouched. A single `ItemId::bank_limit() -> Option<u32>` distinguishes banked currency from cargo, so there is no second predicate to keep in sync. Unsolicited income clamps and logs; paths where the player pays an input cost refuse up front.

**Tech Stack:** Rust, `bevy_ecs` (standalone), `ron` for data files, `ratatui` (TUI peer), `macroquad` (GUI peer).

## Global Constraints

- **Never hardcode `"data_cache"` in Rust.** Capacity comes from the `inventory_bonus` field on `StructureDef`, per CLAUDE.md's moddability rule.
- **New `StructureDef` fields MUST be `#[serde(default)]`** so existing and modded `.ron` files keep parsing.
- **Update `assets/structures/README.md`** in the same task that adds a schema field.
- **Comments explain *why*, never *what*.**
- **Run `cargo fmt` and `cargo clippy --workspace` after every task**; fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate** — 230 tests currently pass. Passing only your own new tests is not evidence of correctness.
- **Do not commit.** `git commit` is denied by this repo's settings. Each task ends with `git add` only; the user commits.
- If many tests suddenly fail with `NotFound` on an assets path, that is stale build artifacts from the `petmud` → `feral-processes` directory rename, not real breakage. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` (never a full `cargo clean` — `target/` is ~4 GB).

## File Structure

| File | Responsibility |
|---|---|
| `crates/engine/src/items.rs` | `ItemId::bank_limit`, `RESEARCH_DATA_BANK_LIMIT` — the single source of truth for cargo-vs-currency |
| `crates/engine/src/structures.rs` | `inventory_bonus` schema field; `BASE_INVENTORY_CAPACITY`; `inventory_capacity_for` free function shared by `Game` and the cronjob system |
| `crates/engine/src/components.rs` | `Inventory::add_capped`, `Inventory::cargo_used` |
| `crates/engine/src/lib.rs` | `Game::inventory_capacity`, `Game::inventory_used`, `Game::check_room`; clamp/refuse at each add site; two new `PlayerStatus` fields |
| `crates/engine/src/systems.rs` | `task_progress_system` clamps cronjob output |
| `crates/app-core/src/lib.rs` | `Mode::EraseQuantity` and its key handler |
| `crates/tui/src/ui.rs`, `crates/gui/src/render.rs` | Buffer line, erase-quantity page, `Research Data: n/200` |
| `assets/structures/data_cache.ron` | cost 10, `inventory_bonus: 10` |
| `assets/research/cold_storage.ron` | deleted |

---

### Task 1: `ItemId::bank_limit`

Establishes the cargo-vs-currency distinction every later task depends on.

**Files:**
- Modify: `crates/engine/src/items.rs` (add constant after the imports at line 1; add method inside `impl ItemId`, after `display_name` which ends at line 33)
- Test: `crates/engine/src/items.rs` (existing `#[cfg(test)] mod tests` at the bottom)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub const RESEARCH_DATA_BANK_LIMIT: u32`, and `ItemId::bank_limit(self) -> Option<u32>`. Later tasks rely on `bank_limit().is_none()` meaning "counts against inventory capacity".

- [ ] **Step 1: Write the failing test**

Append inside the existing `mod tests` block at the bottom of `crates/engine/src/items.rs`:

```rust
    #[test]
    fn research_data_is_banked_and_everything_else_is_cargo() {
        assert_eq!(
            ItemId::ResearchData.bank_limit(),
            Some(RESEARCH_DATA_BANK_LIMIT),
            "Research Data is a currency with its own ceiling"
        );
        for item in [
            ItemId::CoreFragment,
            ItemId::PowerCell,
            ItemId::IceBreaker,
            ItemId::PortalFragment,
            ItemId::OverclockCore,
            ItemId::FirewallPlating,
            ItemId::NeuralAmplifier,
            ItemId::MonofilamentWhip,
            ItemId::AblativePlating,
            ItemId::CortexHack,
        ] {
            assert_eq!(
                item.bank_limit(),
                None,
                "{} is cargo and should count against inventory capacity",
                item.display_name()
            );
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine research_data_is_banked`
Expected: FAIL to compile — `no method named 'bank_limit' found for enum 'ItemId'` and `cannot find value 'RESEARCH_DATA_BANK_LIMIT'`.

- [ ] **Step 3: Write the implementation**

Add near the top of `crates/engine/src/items.rs`, immediately after the `use serde::{Deserialize, Serialize};` line:

```rust
/// Hard ceiling on banked Research Data. Chosen against a full research
/// tree cost of 275, so the bank deliberately cannot fund every node at
/// once — research has to be spent along the way rather than hoarded to
/// the end — while staying far above the priciest single node (45).
pub const RESEARCH_DATA_BANK_LIMIT: u32 = 200;
```

Add inside `impl ItemId`, directly after the closing brace of `display_name`:

```rust
    /// `Some(ceiling)` for a banked currency: exempt from the shared
    /// inventory capacity and limited only by its own hard cap. `None` for
    /// ordinary cargo, which counts against `Game::inventory_capacity`.
    ///
    /// Sharing the cargo cap would let an unrelated pile of Core Fragments
    /// starve a Research Node's output, so the currency is measured
    /// separately.
    pub fn bank_limit(self) -> Option<u32> {
        match self {
            ItemId::ResearchData => Some(RESEARCH_DATA_BANK_LIMIT),
            _ => None,
        }
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p feral-processes-engine research_data_is_banked`
Expected: PASS, `1 passed`.

- [ ] **Step 5: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/engine/src/items.rs
```

Expected: clippy prints nothing. Do not commit.

---

### Task 2: `inventory_bonus` schema field and capacity function

**Files:**
- Modify: `crates/engine/src/structures.rs` (add field to `StructureDef`, which ends around line 140; add constant and free function after the `StructureDef` struct)
- Modify: `assets/structures/data_cache.ron`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/structures.rs` (add a `#[cfg(test)] mod tests` block at the bottom if none exists; otherwise append)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `StructureDef::inventory_bonus: u32`; `pub const BASE_INVENTORY_CAPACITY: u32 = 20`; `pub fn inventory_capacity_for<'a>(deployed: impl Iterator<Item = &'a str>, db: &StructureDb) -> u32`. Tasks 4 and 7 both call `inventory_capacity_for`.

- [ ] **Step 1: Write the failing test**

Append to the bottom of `crates/engine/src/structures.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> StructureDb {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/structures");
        StructureDb::load_dir(&dir).expect("assets/structures should load").0
    }

    #[test]
    fn the_data_cache_is_the_only_structure_granting_buffer_space() {
        let db = test_db();
        let cache = db.get("data_cache").expect("data_cache.ron should load");
        assert_eq!(cache.inventory_bonus, 10);
        for def in db.all() {
            if def.id != "data_cache" {
                assert_eq!(
                    def.inventory_bonus, 0,
                    "{} should not grant buffer space",
                    def.id
                );
            }
        }
    }

    #[test]
    fn capacity_is_the_base_plus_every_deployed_bonus() {
        let db = test_db();
        assert_eq!(
            inventory_capacity_for(std::iter::empty(), &db),
            BASE_INVENTORY_CAPACITY,
            "an empty base is just the baseline"
        );
        assert_eq!(
            inventory_capacity_for(["data_cache"].into_iter(), &db),
            BASE_INVENTORY_CAPACITY + 10
        );
        assert_eq!(
            inventory_capacity_for(["data_cache", "data_cache", "home"].into_iter(), &db),
            BASE_INVENTORY_CAPACITY + 20,
            "caches stack; a Home contributes nothing"
        );
        assert_eq!(
            inventory_capacity_for(["no_such_structure"].into_iter(), &db),
            BASE_INVENTORY_CAPACITY,
            "an unknown kind is ignored rather than panicking"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine --lib structures::tests`
Expected: FAIL to compile — `no field 'inventory_bonus' on type 'StructureDef'`, `cannot find function 'inventory_capacity_for'`.

- [ ] **Step 3: Add the schema field**

In `crates/engine/src/structures.rs`, add to `StructureDef` immediately after the `raid_defense` field:

```rust
    /// How much this structure raises the player's inventory capacity while
    /// it's deployed (see `Game::inventory_capacity`). Stacks additively
    /// across every deployed structure that sets it, so several Data Caches
    /// each add their bonus. `#[serde(default)]` so existing structure
    /// files (including mods) contribute nothing, same as before it
    /// existed.
    #[serde(default)]
    pub inventory_bonus: u32,
```

- [ ] **Step 4: Add the constant and capacity function**

In `crates/engine/src/structures.rs`, after the closing brace of the `StructureDef` struct and before `impl StructureDb`:

```rust
/// Inventory capacity with no capacity-granting structures deployed.
pub const BASE_INVENTORY_CAPACITY: u32 = 20;

/// Total carrying capacity given every currently-deployed structure kind.
/// Takes the kinds rather than reading the ECS so `Game` and
/// `systems::task_progress_system` — which see the world through very
/// different borrows — can share one implementation. Unknown kinds are
/// ignored, which keeps a save referencing a since-removed mod structure
/// loadable.
pub fn inventory_capacity_for<'a>(
    deployed: impl Iterator<Item = &'a str>,
    db: &StructureDb,
) -> u32 {
    BASE_INVENTORY_CAPACITY
        + deployed
            .filter_map(|kind| db.get(kind))
            .map(|def| def.inventory_bonus)
            .sum::<u32>()
}
```

- [ ] **Step 5: Update the Data Cache asset**

Replace the whole of `assets/structures/data_cache.ron` with:

```ron
(
    id: "data_cache",
    name: "Data Cache",
    glyph: '=',
    color: Gray,
    build_cost: [(CoreFragment, 10)],
    work: None,
    inventory_bonus: 10,
)
```

- [ ] **Step 6: Document the field**

In `assets/structures/README.md`, insert inside the ```ron schema block, immediately after the `raid_defense: 4,` line and its comment:

```
    // Optional; can be left out entirely (defaults to 0). How much this
    // structure raises the player's inventory capacity while it's deployed.
    // Capacity is `20 + the sum of this across every deployed structure`,
    // so several of them stack. This is how the Data Cache works:
    // `inventory_bonus: 10` with no `work` recipe. Research Data is exempt
    // from the capacity system entirely and has its own separate cap.
    inventory_bonus: 10,
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine --lib structures::tests`
Expected: PASS, `2 passed`.

- [ ] **Step 8: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/engine/src/structures.rs assets/structures/data_cache.ron assets/structures/README.md
```

---

### Task 3: `Inventory::add_capped` and `cargo_used`

**Files:**
- Modify: `crates/engine/src/components.rs` (inside `impl Inventory`, which spans lines 234-276)
- Test: `crates/engine/src/components.rs` (append a `#[cfg(test)] mod tests` block at the bottom, or extend the existing one)

**Interfaces:**
- Consumes: `ItemId::bank_limit` from Task 1.
- Produces: `Inventory::cargo_used(&self) -> u32` and `Inventory::add_capped(&mut self, item: ItemId, qty: u32, capacity: u32) -> u32` (returns how many units actually landed). Tasks 5, 6 and 7 call these.

- [ ] **Step 1: Write the failing test**

Append to the bottom of `crates/engine/src/components.rs`:

```rust
#[cfg(test)]
mod inventory_capacity_tests {
    use super::*;
    use crate::items::RESEARCH_DATA_BANK_LIMIT;

    #[test]
    fn cargo_used_ignores_banked_currency() {
        let mut inv = Inventory::default();
        inv.add(ItemId::CoreFragment, 5);
        inv.add(ItemId::PowerCell, 3);
        inv.add(ItemId::ResearchData, 90);
        assert_eq!(inv.cargo_used(), 8, "Research Data is banked, not carried");
    }

    #[test]
    fn add_capped_clamps_cargo_to_the_capacity() {
        let mut inv = Inventory::default();
        inv.add(ItemId::CoreFragment, 18);
        let added = inv.add_capped(ItemId::PowerCell, 5, 20);
        assert_eq!(added, 2, "only the 2 units of room should land");
        assert_eq!(inv.count(ItemId::PowerCell), 2);
        assert_eq!(inv.cargo_used(), 20);
    }

    #[test]
    fn add_capped_at_a_full_buffer_adds_nothing() {
        let mut inv = Inventory::default();
        inv.add(ItemId::CoreFragment, 20);
        assert_eq!(inv.add_capped(ItemId::PowerCell, 3, 20), 0);
        assert_eq!(
            inv.count(ItemId::PowerCell),
            0,
            "a fully rejected add shouldn't leave an empty stack behind"
        );
    }

    #[test]
    fn add_capped_measures_banked_currency_against_its_own_limit() {
        let mut inv = Inventory::default();
        inv.add(ItemId::CoreFragment, 20);
        let added = inv.add_capped(ItemId::ResearchData, 50, 20);
        assert_eq!(
            added, 50,
            "a full cargo buffer must not block research income"
        );
        assert_eq!(inv.count(ItemId::ResearchData), 50);
    }

    #[test]
    fn add_capped_clamps_research_data_at_its_bank_limit() {
        let mut inv = Inventory::default();
        inv.add(ItemId::ResearchData, RESEARCH_DATA_BANK_LIMIT - 2);
        assert_eq!(inv.add_capped(ItemId::ResearchData, 10, 20), 2);
        assert_eq!(inv.count(ItemId::ResearchData), RESEARCH_DATA_BANK_LIMIT);
        assert_eq!(inv.add_capped(ItemId::ResearchData, 1, 20), 0);
    }

    #[test]
    fn add_capped_allows_going_over_an_already_exceeded_capacity_by_nothing() {
        // Mirrors a pre-cap save loaded with more than the buffer allows:
        // legal to hold, illegal to add to.
        let mut inv = Inventory::default();
        inv.add(ItemId::CoreFragment, 200);
        assert_eq!(inv.add_capped(ItemId::PowerCell, 1, 20), 0);
        assert_eq!(inv.count(ItemId::CoreFragment), 200, "existing stock is untouched");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine --lib inventory_capacity_tests`
Expected: FAIL to compile — `no method named 'cargo_used'`, `no method named 'add_capped'`.

- [ ] **Step 3: Write the implementation**

In `crates/engine/src/components.rs`, add inside `impl Inventory` after the `count` method:

```rust
    /// Total units of ordinary cargo held. Banked currencies (see
    /// `ItemId::bank_limit`) are excluded — this is the number measured
    /// against `Game::inventory_capacity`.
    pub fn cargo_used(&self) -> u32 {
        self.items
            .iter()
            .filter(|(item, _)| item.bank_limit().is_none())
            .map(|(_, qty)| *qty)
            .sum()
    }

    /// Adds as much of `qty` as fits and returns how many units actually
    /// landed, so a caller can log the shortfall. A banked currency is
    /// measured against its own ceiling and ignores `capacity` entirely;
    /// ordinary cargo is measured against `capacity`.
    ///
    /// Holding more than the ceiling is legal (a save predating the cap, or
    /// a Data Cache destroyed while full) — this only refuses to make it
    /// worse, hence the saturating subtraction.
    pub fn add_capped(&mut self, item: ItemId, qty: u32, capacity: u32) -> u32 {
        let (used, ceiling) = match item.bank_limit() {
            Some(limit) => (self.count(item), limit),
            None => (self.cargo_used(), capacity),
        };
        let added = qty.min(ceiling.saturating_sub(used));
        if added > 0 {
            self.add(item, added);
        }
        added
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine --lib inventory_capacity_tests`
Expected: PASS, `6 passed`.

- [ ] **Step 5: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/engine/src/components.rs
```

---

### Task 4: `Game` capacity accessors and `PlayerStatus` fields

**Files:**
- Modify: `crates/engine/src/lib.rs` — `PlayerStatus` struct (lines 278-306), `Game::player_status` (line 3623), and a new accessor block near `Game::inventory_capacity`
- Test: `crates/engine/src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: `inventory_capacity_for`, `BASE_INVENTORY_CAPACITY` (Task 2); `Inventory::cargo_used` (Task 3).
- Produces: `Game::inventory_capacity(&self) -> u32`, `Game::inventory_used(&self) -> u32`, `Game::check_room(&self, item: ItemId, qty: u32) -> Result<(), String>` (private), and `PlayerStatus::inventory_used` / `PlayerStatus::inventory_capacity`. Task 6 calls `check_room`; Task 10 reads the `PlayerStatus` fields.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests` in `crates/engine/src/lib.rs`:

```rust
    /// Deploys a Data Cache next to the player without going through
    /// `place_structure`, sidestepping its Home/cost/radius requirements —
    /// those aren't what the capacity tests are about.
    fn spawn_data_cache(game: &mut Game, offset: i32) {
        let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "data_cache".to_string(),
            },
            Position {
                x: pos.x + offset,
                y: pos.y,
            },
        ));
    }

    #[test]
    fn inventory_capacity_grows_with_each_deployed_data_cache() {
        let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(game.inventory_capacity(), 20);

        spawn_data_cache(&mut game, 1);
        assert_eq!(game.inventory_capacity(), 30);

        spawn_data_cache(&mut game, 2);
        assert_eq!(game.inventory_capacity(), 40, "caches stack");
    }

    #[test]
    fn destroying_a_data_cache_shrinks_the_capacity_back() {
        let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        spawn_data_cache(&mut game, 1);
        assert_eq!(game.inventory_capacity(), 30);

        let cache = game
            .world
            .iter_entities()
            .find(|e| e.get::<Structure>().is_some_and(|s| s.kind == "data_cache"))
            .map(|e| e.id())
            .expect("the spawned cache should be findable");
        game.world.despawn(cache);

        assert_eq!(
            game.inventory_capacity(),
            20,
            "capacity is derived, so a destroyed cache needs no invalidation"
        );
    }

    #[test]
    fn inventory_used_counts_cargo_but_not_research_data() {
        let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Starting inventory is 3 ICE Breaker + 3 Power Cell + 5 Core Fragment.
        assert_eq!(game.inventory_used(), 11);

        grant_research_data(&mut game, 90);
        assert_eq!(
            game.inventory_used(),
            11,
            "banked research must not consume carrying capacity"
        );

        let status = game.player_status();
        assert_eq!(status.inventory_used, 11);
        assert_eq!(status.inventory_capacity, 20);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine inventory_capacity_grows inventory_used_counts destroying_a_data_cache`
Expected: FAIL to compile — `no method named 'inventory_capacity'`, `no field 'inventory_used' on type 'PlayerStatus'`.

- [ ] **Step 3: Add the `PlayerStatus` fields**

In `crates/engine/src/lib.rs`, add to the `PlayerStatus` struct after the `inventory` field:

```rust
    /// Units of ordinary cargo currently carried — what
    /// `inventory_capacity` limits. Excludes banked currency (see
    /// `ItemId::bank_limit`), so it will not match the sum of `inventory`
    /// when Research Data is held.
    pub inventory_used: u32,
    /// The player's current carrying capacity, base plus every deployed
    /// structure's `inventory_bonus`.
    pub inventory_capacity: u32,
```

- [ ] **Step 4: Populate them in `player_status`**

`Game::player_status` (line 3623) holds a live `&Inventory` borrow in `inv`, so compute the capacity *before* that borrow starts. Add as the first line of the function body, immediately after `let player = self.player_entity();`:

```rust
        let inventory_capacity = self.inventory_capacity();
```

Then add these two fields to the `PlayerStatus { .. }` literal the function returns, next to `inventory`:

```rust
            inventory_used: inv.cargo_used(),
            inventory_capacity,
```

- [ ] **Step 5: Add the accessors**

In `crates/engine/src/lib.rs`, add inside `impl Game` immediately before `pub fn structure_build_cost` (around line 4406):

```rust
    /// How many units of cargo the player can carry right now: the base
    /// capacity plus every deployed structure's `inventory_bonus`. Derived
    /// on each call rather than cached, so a Data Cache lost to a raid
    /// shrinks the buffer with no invalidation step and the save format
    /// stays unchanged.
    pub fn inventory_capacity(&self) -> u32 {
        let kinds: Vec<StructureId> = self
            .world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>().map(|s| s.kind.clone()))
            .collect();
        let db = self.world.resource::<StructureDb>();
        structures::inventory_capacity_for(kinds.iter().map(|k| k.as_str()), db)
    }

    /// Units of cargo currently carried, excluding banked currency.
    pub fn inventory_used(&self) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.cargo_used())
            .unwrap_or(0)
    }

    /// `Ok(())` if `qty` more of `item` would fit. Used by the paths where
    /// the player pays an input cost — compiling, buying, unequipping —
    /// since clamping those would destroy value the player already spent.
    fn check_room(&self, item: ItemId, qty: u32) -> Result<(), String> {
        let capacity = self.inventory_capacity();
        let inv = self.world.get::<Inventory>(self.player_entity()).unwrap();
        let (used, ceiling, label) = match item.bank_limit() {
            Some(limit) => (inv.count(item), limit, "Research bank"),
            None => (inv.cargo_used(), capacity, "Buffer"),
        };
        if used + qty > ceiling {
            return Err(format!("{label} full ({used}/{ceiling})."));
        }
        Ok(())
    }
```

If `StructureId` is not already in scope in `lib.rs`, add it to the existing `use crate::structures::{...}` import alongside `StructureDb`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine inventory_capacity_grows inventory_used_counts destroying_a_data_cache`
Expected: PASS, `3 passed`.

`check_room` is unused until Task 6 and will draw a `dead_code` warning. That is expected; Task 6 removes it. Do not add `#[allow(dead_code)]`.

- [ ] **Step 7: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^error" | head
git add crates/engine/src/lib.rs
```

Expected: no `error` lines. A `dead_code` warning for `check_room` is fine at this task only.

---

### Task 5: Clamp the income paths

Scan, battle loot, equipment drops, and both portal-fragment drops must take what fits and log the loss rather than blocking.

**Files:**
- Modify: `crates/engine/src/lib.rs` — `Game::forage` (line 1213), and the four `.add(...)` calls in the post-battle loot block (lines ~2460-2516)
- Test: `crates/engine/src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Inventory::add_capped` (Task 3), `Game::inventory_capacity` (Task 4).
- Produces: `Game::grant_loot(&mut self, item: ItemId, qty: u32) -> u32` (private) — the shared clamp-and-log helper. Task 7 does *not* use it (the ECS system can't reach `&mut Game`).

- [ ] **Step 1: Write the failing test**

Add inside `mod tests` in `crates/engine/src/lib.rs`:

```rust
    /// Fills the player's cargo to exactly the current capacity so the next
    /// pickup has nowhere to go.
    fn fill_buffer(game: &mut Game) {
        let capacity = game.inventory_capacity();
        let player = game.player_entity();
        let used = game.inventory_used();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, capacity - used);
    }

    #[test]
    fn foraging_into_a_full_buffer_loses_the_find_and_says_so() {
        let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill_buffer(&mut game);
        let before = game.inventory_used();

        // Forage until the RNG grants a find, so the assertion doesn't
        // depend on a specific seed's first roll.
        for _ in 0..200 {
            game.forage();
        }

        assert_eq!(
            game.inventory_used(),
            before,
            "a full buffer must not grow, however many finds are rolled"
        );
        assert_eq!(
            game.inventory_used(),
            game.inventory_capacity(),
            "and must stay exactly at capacity"
        );
    }

    #[test]
    fn a_partially_full_buffer_takes_only_what_fits() {
        let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let capacity = game.inventory_capacity();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, capacity - game.inventory_used() - 1);
        assert_eq!(game.inventory_used(), capacity - 1);

        let landed = game.grant_loot(ItemId::PortalFragment, 6);

        assert_eq!(landed, 1, "only the single unit of room should land");
        assert_eq!(game.inventory_used(), capacity);
        assert_eq!(game.player_status().inventory.iter().find(|(i, _)| *i == ItemId::PortalFragment).map(|(_, q)| *q), Some(1));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine foraging_into_a_full_buffer a_partially_full_buffer`
Expected: FAIL — `no method named 'grant_loot'`, and `foraging_into_a_full_buffer` fails its equality assertion because `forage` still calls the uncapped `add`.

- [ ] **Step 3: Add the shared helper**

In `crates/engine/src/lib.rs`, add inside `impl Game` immediately before `pub fn forage`:

```rust
    /// Awards unsolicited income — a scan find, battle loot, a boss cache —
    /// clamped to whatever room is left, returning how many units landed.
    /// Income clamps rather than refusing so a full buffer can never stall
    /// a battle from resolving or a cronjob worker from running; the loss
    /// is logged so it is never silent.
    fn grant_loot(&mut self, item: ItemId, qty: u32) -> u32 {
        let capacity = self.inventory_capacity();
        let player = self.player_entity();
        let added = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add_capped(item, qty, capacity);
        if added < qty {
            let lost = qty - added;
            let label = if item.bank_limit().is_some() {
                "Research bank"
            } else {
                "Buffer"
            };
            self.log(format!(
                "{label} full — {lost} {} lost.",
                item.display_name()
            ));
        }
        added
    }
```

- [ ] **Step 4: Route `forage` through it**

In `Game::forage`, replace this block:

```rust
        if found {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(ItemId::CoreFragment, 1);
            self.log_kind(
                MessageKind::Loot,
                "You scan the sector and recover a core fragment.",
            );
```

with:

```rust
        if found {
            if self.grant_loot(ItemId::CoreFragment, 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    "You scan the sector and recover a core fragment.",
                );
            }
```

- [ ] **Step 5: Route the four battle-loot sites through it**

In the post-battle loot block (around lines 2460-2516), replace each `self.world.get_mut::<Inventory>(player).unwrap().add(X, Y);` with the `grant_loot` equivalent, keeping each log gated on something having landed.

Species work-resource drop:

```rust
        if let Some(resource) = species.work_resource {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(1..=2)
            };
            let landed = self.grant_loot(resource, qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It drops {} {}.", landed, resource.display_name()),
                );
            }
        }
```

Equipment drop:

```rust
            if roll && self.grant_loot(item, 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It also drops a {}!", item.display_name()),
                );
            }
```

Boss portal-fragment cache:

```rust
            let landed = self.grant_loot(ItemId::PortalFragment, qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("Its crash leaves behind a cache of {landed} portal fragments!"),
                );
            }
```

Ordinary portal-fragment drop:

```rust
            if portal_fragment_roll && self.grant_loot(ItemId::PortalFragment, 1) > 0 {
                self.log_kind(MessageKind::Loot, "It leaves behind a portal fragment.");
            }
```

Note the boss and work-resource messages now report `landed`, not the rolled `qty` — the log must describe what the player actually received.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine foraging_into_a_full_buffer a_partially_full_buffer`
Expected: PASS, `2 passed`.

- [ ] **Step 7: Run the full engine suite**

Run: `cargo test -p feral-processes-engine`
Expected: all pass. The existing `defeating_a_boss_guarantees_a_cache_of_portal_fragments` test (line ~9664) exercises this path — if it now fails, check whether its fixture starts near capacity; the fix is in the test's setup, not by reverting the clamp.

- [ ] **Step 8: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/engine/src/lib.rs
```

---

### Task 6: Refuse the paid paths

**Files:**
- Modify: `crates/engine/src/lib.rs` — `Game::craft` (around line 1518), `Game::unequip` (line 1645), `Game::buy_item` (around line 4560)
- Test: `crates/engine/src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Game::check_room` (Task 4), `fill_buffer` test helper (Task 5).
- Produces: nothing new.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests` in `crates/engine/src/lib.rs`:

```rust
    #[test]
    fn compiling_into_a_full_buffer_refuses_and_consumes_nothing() {
        let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill_buffer(&mut game);
        let player = game.player_entity();
        let cores_before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);

        let err = game
            .craft(ItemId::PowerCell, 1)
            .expect_err("a full buffer should refuse a compile");

        assert!(err.contains("Buffer full"), "got: {err}");
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::CoreFragment),
            cores_before,
            "a refused compile must not consume its inputs"
        );
    }

    #[test]
    fn unequipping_into_a_full_buffer_refuses_and_keeps_the_gear_equipped() {
        let mut game = Game::new(706, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 1);
        game.equip(ItemId::OverclockCore).unwrap();
        fill_buffer(&mut game);

        let err = game
            .unequip(EquipmentSlot::Weapon)
            .expect_err("a full buffer should refuse an unequip");

        assert!(err.contains("Buffer full"), "got: {err}");
        assert!(
            game.player_status().weapon.is_some(),
            "refused unequip must leave the gear equipped, not delete it"
        );
    }

    #[test]
    fn a_compile_still_works_with_exactly_enough_room() {
        let mut game = Game::new(707, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let capacity = game.inventory_capacity();
        let player = game.player_entity();
        // Two Core Fragments become one Power Cell: net -1 unit, so filling
        // to exactly capacity still leaves the compile viable.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, capacity - game.inventory_used() - 1);

        game.craft(ItemId::PowerCell, 1)
            .expect("a compile that nets out under capacity should succeed");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine compiling_into_a_full_buffer unequipping_into_a_full_buffer a_compile_still_works`
Expected: the two refusal tests FAIL (`expect_err` panics — the calls currently succeed).

- [ ] **Step 3: Gate `craft`**

In `Game::craft`, the existing affordability check is followed by the block that takes inputs and adds the result. Insert the room check immediately after the affordability `{ ... }` block closes and before the block that mutates:

```rust
        self.check_room(result, quantity)?;
```

- [ ] **Step 4: Gate `unequip`**

`Game::unequip` removes the item from the `Equipment` slot before adding it to the inventory, so the check must come *first* — otherwise a refusal would leave the gear in neither place. Insert immediately after the `is_game_over` / `has_active_battle` guard and before `let removed = { ... }`:

```rust
        let equipped_item = self
            .world
            .get::<Equipment>(self.player_entity())
            .and_then(|e| e.get(slot))
            .map(|eq| eq.item);
        if let Some(item) = equipped_item {
            self.check_room(item, 1)?;
        }
```

`Equipment::get(slot) -> Option<EquippedItem>` already exists at `crates/engine/src/components.rs:190`; no new accessor is needed.

- [ ] **Step 5: Gate `buy_item`**

In `Game::buy_item`, insert immediately after the "Not enough Core Fragments" check and before the block that takes payment:

```rust
        self.check_room(item, qty)?;
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine compiling_into_a_full_buffer unequipping_into_a_full_buffer a_compile_still_works`
Expected: PASS, `3 passed`.

- [ ] **Step 7: Run the full engine suite**

Run: `cargo test -p feral-processes-engine`
Expected: all pass, and the Task 4 `dead_code` warning for `check_room` is gone.

- [ ] **Step 8: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/engine/src/lib.rs crates/engine/src/components.rs
```

---

### Task 7: Clamp cronjob output

**Files:**
- Modify: `crates/engine/src/systems.rs` — `task_progress_system` (line 122)
- Test: `crates/engine/src/lib.rs` (`mod tests`) — the system is exercised through `Game::tick`

**Interfaces:**
- Consumes: `inventory_capacity_for` (Task 2), `Inventory::add_capped` (Task 3).
- Produces: nothing new.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests` in `crates/engine/src/lib.rs`:

```rust
    /// Tames a program and puts it to work on a node producing `resource`,
    /// so a cronjob is guaranteed to be running — the assertions below are
    /// vacuous if nothing is assigned.
    fn assign_worker_producing(game: &mut Game, resource: ItemId) {
        let worker = spawn_tamed(game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource,
                    amount: 20,
                    capacity: 20,
                    level: None,
                },
            ))
            .id();
        game.assign_cronjob(worker, structure).unwrap();
    }

    #[test]
    fn a_cronjob_worker_cannot_overfill_the_buffer() {
        let mut game = Game::new(708, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assign_worker_producing(&mut game, ItemId::CoreFragment);
        fill_buffer(&mut game);
        let capacity = game.inventory_capacity();

        for _ in 0..100 {
            game.tick();
        }

        assert_eq!(
            game.inventory_used(),
            capacity,
            "a working cronjob must fill the buffer to exactly capacity and stop"
        );
    }

    #[test]
    fn a_research_cronjob_keeps_banking_with_a_full_cargo_buffer() {
        let mut game = Game::new(709, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assign_worker_producing(&mut game, ItemId::ResearchData);
        fill_buffer(&mut game);
        let before = research_data_held(&game);

        for _ in 0..100 {
            game.tick();
        }

        assert!(
            research_data_held(&game) > before,
            "a full cargo buffer must not stop research from banking (was {before}, now {})",
            research_data_held(&game)
        );
    }
```

Both assertions are now strict: the first pins the buffer at exactly capacity (not merely `<=`, which a stalled worker would also satisfy), and the second requires research to have actually increased. `assign_cronjob` and the `spawn_tamed` helper already exist — see the existing test `assigning_cronjob_to_the_active_companion_clears_companion_status` at `crates/engine/src/lib.rs:6901` for the same spawn pattern.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine a_cronjob_worker_cannot_overfill a_research_cronjob_keeps_banking`
Expected: `a_cronjob_worker_cannot_overfill` FAILS — the uncapped `inv.add` pushes cargo past capacity, so `inventory_used()` exceeds it. If this test passes before the implementation, the worker is not actually running; fix the setup before continuing, because the assertion is worthless otherwise.

- [ ] **Step 3: Write the implementation**

In `crates/engine/src/systems.rs`, extend the signature and compute the capacity once per tick:

```rust
pub fn task_progress_system(
    mut tasks: Query<CronjobWorker>,
    mut nodes: Query<&mut ResourceNode>,
    mut inventories: Query<&mut Inventory>,
    structures: Query<&Structure>,
    structure_db: Res<StructureDb>,
    species_db: Res<SpeciesDb>,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    let capacity = crate::structures::inventory_capacity_for(
        structures.iter().map(|s| s.kind.as_str()),
        &structure_db,
    );
```

Then replace the award line:

```rust
        if let Ok(mut inv) = inventories.get_mut(tamed.owner) {
            inv.add(node.resource, 1);
```

with:

```rust
        if let Ok(mut inv) = inventories.get_mut(tamed.owner) {
            if inv.add_capped(node.resource, 1, capacity) == 0 {
                log.push(format!(
                    "A cronjob yields {} but there's no room to store it.",
                    node.resource.display_name()
                ));
            }
```

The node stock was already decremented above, so a rejected unit is genuinely lost — deliberate, matching the clamp rule for all unsolicited income.

Add `Structure` and `StructureDb` to the `use` statements at the top of `systems.rs` if not already imported.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine a_cronjob_worker_cannot_overfill a_research_cronjob_keeps_banking`
Expected: PASS, `2 passed`.

- [ ] **Step 5: Run the full engine suite**

Run: `cargo test -p feral-processes-engine`
Expected: all pass. Watch for a bevy_ecs panic about conflicting query access — `Query<&Structure>` and `Query<&mut ResourceNode>` target different components on the same entities, which is allowed; a conflict panic instead means `CronjobWorker` already includes `&mut Structure` and the new query needs to become `Query<&Structure, Without<...>>`.

- [ ] **Step 6: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/engine/src/systems.rs crates/engine/src/lib.rs
```

---

### Task 8: Make the Data Cache available from turn one

**Files:**
- Delete: `assets/research/cold_storage.ron`
- Test: `crates/engine/src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests` in `crates/engine/src/lib.rs`:

```rust
    #[test]
    fn the_data_cache_is_buildable_without_any_research() {
        let game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(
            game.buildable_structure_defs()
                .iter()
                .any(|d| d.id == "data_cache"),
            "buffer expansion must not be gated behind research the player \
             can't afford while the cap is at its tightest"
        );
    }

    #[test]
    fn no_research_node_is_left_unlocking_nothing() {
        let game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        for node in game.research_nodes() {
            let def = game
                .world
                .resource::<ResearchDb>()
                .get(&node.id)
                .expect("a listed node should exist in the db");
            assert!(
                !def.unlocks_structures.is_empty() || !def.unlocks_recipes.is_empty(),
                "{} unlocks nothing and is dead weight in the tree",
                node.id
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine the_data_cache_is_buildable no_research_node_is_left`
Expected: `the_data_cache_is_buildable_without_any_research` FAILS — the Data Cache is still gated by `cold_storage`.

- [ ] **Step 3: Delete the research node**

```bash
git rm assets/research/cold_storage.ron
```

Nothing else references it: its only content was `unlocks_structures: ["data_cache"]`, and no other node lists `cold_storage` in `requires`. Per `assets/research/README.md`, a structure named by no research file is buildable from turn one, so deleting the file is the entire implementation.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine the_data_cache_is_buildable no_research_node_is_left`
Expected: PASS, `2 passed`.

- [ ] **Step 5: Run the full engine suite**

Run: `cargo test -p feral-processes-engine`
Expected: all pass. Any test asserting a research-node count of 13 must be updated to 12 — that is the intended change, not a regression. Any test calling `unlock_research_chain(&mut game, "cold_storage")` must be deleted along with the node.

- [ ] **Step 6: Stage**

```bash
git add -A assets/research crates/engine/src/lib.rs
```

---

### Task 9: Erase-quantity mode in app-core

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode` enum (line ~78), `App` fields (line ~168), `App::new` initializer (line ~225), the `handle_key` dispatch match (line ~440), `handle_inventory_item_action_key` (line 1464)
- Test: `crates/app-core/src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Game::erase_item(item, qty)` (already exists, unchanged).
- Produces: `Mode::EraseQuantity`, `App::erase_quantity_input: String`, `App::pending_erase: Option<ItemId>`. Task 10 renders from these three.

- [ ] **Step 1: Write the failing test**

Add inside `mod tests` in `crates/app-core/src/lib.rs`:

```rust
    #[test]
    fn erasing_asks_for_a_quantity_and_removes_exactly_that_many() {
        let mut app = test_app(900);
        let player = app.game.as_ref().unwrap().player_entity();
        app.game
            .as_mut()
            .unwrap()
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, 10);
        let before = app
            .game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::CoreFragment)
            .map(|(_, q)| *q)
            .unwrap();

        app.pending_inventory_item = Some(ItemId::CoreFragment);
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('x'));
        assert_eq!(
            app.mode,
            Mode::EraseQuantity,
            "[X] should ask how many, not dump the whole stack"
        );

        app.handle_key(GameKey::Char('3'));
        app.handle_key(GameKey::Enter);

        let after = app
            .game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::CoreFragment)
            .map(|(_, q)| *q)
            .unwrap();
        assert_eq!(after, before - 3);
        assert_eq!(app.mode, Mode::Inventory);
    }

    #[test]
    fn erase_all_dumps_the_whole_stack() {
        let mut app = test_app(901);
        app.pending_inventory_item = Some(ItemId::CoreFragment);
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('x'));
        app.handle_key(GameKey::Char('a'));

        let held = app
            .game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::CoreFragment)
            .map(|(_, q)| *q);
        assert_eq!(held, None, "[A] should clear the stack entirely");
    }

    #[test]
    fn escaping_the_erase_prompt_erases_nothing() {
        let mut app = test_app(902);
        let before = app.game.as_ref().unwrap().player_status().inventory;
        app.pending_inventory_item = Some(ItemId::CoreFragment);
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('x'));
        app.handle_key(GameKey::Esc);

        assert_eq!(app.mode, Mode::Inventory);
        assert_eq!(app.game.as_ref().unwrap().player_status().inventory, before);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-app-core erasing_asks_for_a_quantity erase_all_dumps escaping_the_erase_prompt`
Expected: FAIL to compile — `no variant named 'EraseQuantity' found for enum 'Mode'`.

- [ ] **Step 3: Add the mode and state**

Add to the `Mode` enum, immediately after `InventoryItemAction`:

```rust
    /// Second page of the erase flow: asks how many units of
    /// `pending_erase` to destroy before calling `Game::erase_item`. A
    /// hard inventory cap makes partial erasure the common case — dumping a
    /// whole stack to free two units of room is not a real option.
    EraseQuantity,
```

Add to the `App` struct, next to `pending_inventory_item`:

```rust
    /// The inventory item picked for erasure, awaiting a quantity from
    /// `Mode::EraseQuantity`.
    pub pending_erase: Option<ItemId>,
    /// Digits typed so far on the erase-quantity page.
    pub erase_quantity_input: String,
```

Add to the `App::new` initializer, next to `pending_inventory_item: None,`:

```rust
            pending_erase: None,
            erase_quantity_input: String::new(),
```

Add to the `handle_key` dispatch match, next to the `Mode::InventoryItemAction` arm:

```rust
            Mode::EraseQuantity => self.handle_erase_quantity_key(key),
```

- [ ] **Step 4: Route `[X]` to the new mode**

In `handle_inventory_item_action_key`, replace this arm:

```rust
            Some('x') => Some(game.erase_item(item, stack_qty)),
```

with:

```rust
            Some('x') => {
                self.pending_erase = Some(item);
                self.erase_quantity_input.clear();
                self.mode = Mode::EraseQuantity;
                self.pending_inventory_item = None;
                return;
            }
```

The `let Some(game) = &mut self.game else { return };` binding above that match borrows `self` mutably, so this arm must not also touch `self`. Move the `Some('x')` arm's handling above that binding: match on `idx.map(|i| actions[i])` for the `'x'` case first, returning early, and leave `'e'`/`'u'` in the existing block.

- [ ] **Step 5: Add the key handler**

Add after `handle_inventory_item_action_key`:

```rust
    /// Second page of the erase flow: how many units of `pending_erase` to
    /// destroy. `[A]` erases the whole stack, matching the pre-cap
    /// behavior. An empty input on Enter means 1.
    fn handle_erase_quantity_key(&mut self, key: GameKey) {
        let Some(item) = self.pending_erase else {
            self.mode = Mode::Inventory;
            return;
        };
        let stack_qty = self
            .game
            .as_ref()
            .map(|g| {
                g.player_status()
                    .inventory
                    .iter()
                    .find(|(i, _)| *i == item)
                    .map(|(_, q)| *q)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        match key {
            GameKey::Esc => {
                self.pending_erase = None;
                self.erase_quantity_input.clear();
                self.mode = Mode::Inventory;
            }
            GameKey::Backspace => {
                self.erase_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.erase_quantity_input.len() < 4 => {
                self.erase_quantity_input.push(c);
            }
            GameKey::Char('a') | GameKey::Char('A') => {
                self.commit_erase(item, stack_qty);
            }
            GameKey::Enter => {
                let quantity: u32 = if self.erase_quantity_input.is_empty() {
                    1
                } else {
                    self.erase_quantity_input.parse().unwrap_or(0)
                };
                self.commit_erase(item, quantity);
            }
            _ => {}
        }
    }

    /// Calls `Game::erase_item` and returns to the inventory screen. A
    /// quantity of 0 is a silent no-op rather than a round-trip to the
    /// engine for an error, matching `commit_craft`.
    fn commit_erase(&mut self, item: ItemId, quantity: u32) {
        self.pending_erase = None;
        self.erase_quantity_input.clear();
        self.mode = Mode::Inventory;
        if quantity == 0 {
            return;
        }
        if let Some(game) = &mut self.game {
            match game.erase_item(item, quantity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-app-core erasing_asks_for_a_quantity erase_all_dumps escaping_the_erase_prompt`
Expected: PASS, `3 passed`.

- [ ] **Step 7: Run the full suite**

Run: `cargo test --workspace`
Expected: all pass. Any existing test asserting that `[X]` erases immediately must be updated to go through the quantity page — that is the intended change.

- [ ] **Step 8: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/app-core/src/lib.rs
```

---

### Task 10: Renderers — buffer line, erase page, research ceiling

Both peers must stay in lockstep; a player switching renderers should see the same numbers.

**Files:**
- Modify: `crates/tui/src/ui.rs` — mode list (line ~48), dispatch match (line ~135), `render_inventory_screen` (line 1493), `render_research_menu` (line ~1281)
- Modify: `crates/gui/src/render.rs` — dispatch match (line ~553), `draw_inventory` (line 955), `draw_research_menu` (line 1414)

**Interfaces:**
- Consumes: `PlayerStatus::inventory_used` / `inventory_capacity` (Task 4), `RESEARCH_DATA_BANK_LIMIT` (Task 1), `Mode::EraseQuantity` / `App::pending_erase` / `App::erase_quantity_input` (Task 9).
- Produces: nothing.

- [ ] **Step 1: Add the buffer line to the TUI inventory screen**

In `render_inventory_screen` in `crates/tui/src/ui.rs`, replace this element of the `lines` vec:

```rust
        Line::styled(
            "Inventory (number to equip/erase):",
            Style::new().add_modifier(Modifier::BOLD),
        ),
```

with:

```rust
        Line::styled(
            format!(
                "Inventory — Buffer {}/{} (number to equip/erase):",
                status.inventory_used, status.inventory_capacity
            ),
            Style::new().add_modifier(Modifier::BOLD),
        ),
```

- [ ] **Step 2: Mirror it in the GUI inventory screen**

In `draw_inventory` in `crates/gui/src/render.rs`, replace:

```rust
        text_row("Inventory (number to equip/erase):"),
```

with:

```rust
        text_row(format!(
            "Inventory - Buffer {}/{} (number to equip/erase):",
            status.inventory_used, status.inventory_capacity
        )),
```

- [ ] **Step 3: Show the research ceiling in both peers**

In `crates/tui/src/ui.rs`, in the research menu, replace:

```rust
            format!("Research Data: {held}"),
```

with:

```rust
            format!("Research Data: {held}/{}", RESEARCH_DATA_BANK_LIMIT),
```

Make the same substitution in `draw_research_menu` in `crates/gui/src/render.rs`:

```rust
        Row::TextColored(
            format!("Research Data: {held}/{}", RESEARCH_DATA_BANK_LIMIT),
            CYAN,
        ),
```

Add `RESEARCH_DATA_BANK_LIMIT` to the existing `feral_processes_engine::items::{...}` import in each file.

- [ ] **Step 4: Render the TUI erase-quantity page**

Add `Mode::EraseQuantity` to the popup-mode list at line ~48 (the same list containing `Mode::CraftQuantity`), add this dispatch arm next to the `Mode::CraftQuantity` arm:

```rust
        Mode::EraseQuantity => render_erase_quantity_menu(
            f,
            area,
            game,
            app.pending_erase,
            &app.erase_quantity_input,
        ),
```

and add the render function next to `render_craft_quantity_menu`:

```rust
fn render_erase_quantity_menu(
    f: &mut Frame,
    area: Rect,
    game: &mut Game,
    item: Option<ItemId>,
    quantity_input: &str,
) {
    let popup = centered_rect(60, 40, area);
    f.render_widget(Clear, popup);
    let Some(item) = item else { return };
    let status = game.player_status();
    let held = status
        .inventory
        .iter()
        .find(|(i, _)| *i == item)
        .map(|(_, q)| *q)
        .unwrap_or(0);
    let shown = if quantity_input.is_empty() {
        "1".to_string()
    } else {
        quantity_input.to_string()
    };
    let lines = vec![
        Line::from(format!("Erase how many {}?", item.display_name())),
        Line::from(""),
        Line::from(format!("Quantity: {shown}")),
        Line::from(""),
        Line::from(format!(
            "You have: {held}        Buffer: {}/{}",
            status.inventory_used, status.inventory_capacity
        )),
        Line::from(""),
        Line::from("Type digits, Enter to erase"),
        Line::from("[A] Erase all   Esc to go back"),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Erase")),
        popup,
    );
}
```

- [ ] **Step 5: Render the GUI erase-quantity page**

Add this dispatch arm next to the `Mode::CraftQuantity` arm in `crates/gui/src/render.rs`:

```rust
        Mode::EraseQuantity => {
            draw_erase_quantity(game, app.pending_erase, &app.erase_quantity_input)
        }
```

and the function next to `draw_craft_quantity`:

```rust
fn draw_erase_quantity(game: &mut Game, item: Option<ItemId>, quantity_input: &str) {
    let Some(item) = item else { return };
    let status = game.player_status();
    let held = status
        .inventory
        .iter()
        .find(|(i, _)| *i == item)
        .map(|(_, q)| *q)
        .unwrap_or(0);
    let shown = if quantity_input.is_empty() {
        "1".to_string()
    } else {
        quantity_input.to_string()
    };
    let rows = vec![
        text_row(format!("Erase how many {}?", item.display_name())),
        text_row(""),
        text_row(format!("Quantity: {shown}")),
        text_row(""),
        text_row(format!(
            "You have: {held}        Buffer: {}/{}",
            status.inventory_used, status.inventory_capacity
        )),
        text_row(""),
        text_row("Type digits, Enter to erase"),
        text_row("[A] Erase all   Esc to go back"),
    ];
    draw_popup("Erase", PopupSize::Large, &rows);
}
```

`PopupSize` has only `Large` and `Small`. Use `Large`, matching `draw_craft_quantity` — the sibling prompt this page mirrors.

- [ ] **Step 6: Build both renderers**

Run: `cargo build --workspace`
Expected: compiles cleanly. Missing-import errors on `ItemId` or `RESEARCH_DATA_BANK_LIMIT` are the likely failure; add them to the existing engine imports at the top of each file.

- [ ] **Step 7: Run the full suite**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 8: Format, lint, stage**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | grep -E "^(warning|error)" | head
git add crates/tui/src/ui.rs crates/gui/src/render.rs
```

---

### Task 11: Manual verification

Automated tests cannot confirm the numbers actually read correctly on screen.

**Files:** none.

- [ ] **Step 1: Run the game**

```bash
cargo run -p feral-processes
```

- [ ] **Step 2: Confirm by hand and report what you actually saw**

- `i` shows `Inventory — Buffer 11/20` on a fresh game.
- `b` lists the Data Cache at 10 Core Fragments with no research completed, described as raising capacity.
- Scan (`g`) repeatedly with a full buffer: the log says `Buffer full — 1 Core Fragment lost.` and the buffer count never exceeds 20.
- `[X]` on a Core Fragment stack opens the Erase page; typing `3` and Enter drops the stack by exactly 3 and the Buffer count falls by 3.
- `[A]` on the Erase page clears the stack.
- Deploy a Home, then a Data Cache: the inventory header reads `Buffer n/30`.
- `T` (research) shows `Research Data: 0/200` and lists 12 nodes, with no Cold Storage entry.

Report the actual observed output for each, not the expectation.

---

## Notes for the implementer

- `Game::erase_item` calls `self.tick()`, so erasing advances the game clock. That is pre-existing behavior and is intentionally unchanged.
- The test helper `unlock_research_chain` grants 1000 Research Data via the uncapped `Inventory::add`, deliberately bypassing the 200 bank. Leave it alone — it exists so research tests don't have to model the economy.
- Equipped gear is not in `Inventory` (`Game::equip` removes it), so it never counts against capacity. This is exactly why Task 6 gates `unequip` before touching the equipment slot.
- Holding more than capacity is a legal state, not a bug: a pre-cap save, or a Data Cache destroyed while the buffer was full. Every add path uses saturating arithmetic so this degrades to "can't add more" rather than underflowing.
