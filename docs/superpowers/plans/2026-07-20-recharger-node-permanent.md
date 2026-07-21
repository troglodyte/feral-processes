# Recharger Node: permanent structure — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Recharger Node a permanent structure (no more 20-tick collapse), relying on the existing Home-proximity build rule to keep it "near the base."

**Architecture:** One-field data change (`recharger_node.ron`) plus matching test updates in `crates/engine/src/lib.rs`. No engine logic changes — `Game::place_structure`'s existing `MAX_BUILD_DISTANCE_FROM_HOME` check already restricts every non-Home structure, Recharger Node included, to within 15 tiles of Home.

**Tech Stack:** Rust, `ron` data files, `cargo test` (workspace root).

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-20-recharger-node-permanent-design.md`
- The generic `temporary` structure field and `Temporary` component must stay in the engine/schema for other/modded structures — only stop using it on `recharger_node.ron`.
- No changes to `Game::place_structure`, `MAX_BUILD_DISTANCE_FROM_HOME`, or `enables_rest`.

---

### Task 1: Make Recharger Node permanent

**Files:**
- Modify: `assets/structures/recharger_node.ron`
- Modify: `crates/engine/src/lib.rs` (test helper `spawn_recharger_node_at_player`, and four tests around it)
- Modify: `assets/structures/README.md` (one stale sentence naming the Recharger Node as the `temporary` + `enables_rest` example)

**Interfaces:**
- Consumes: `StructureDef::temporary: Option<Temporary>` (existing field, `crates/engine/src/structures.rs`), `Game::structure_defs() -> Vec<StructureDef>` (existing).
- Produces: nothing new — this task removes behavior, it doesn't add an interface.

- [ ] **Step 1: Update the test helper to stop attaching `Temporary`**

Currently (`crates/engine/src/lib.rs`, inside `#[cfg(test)] mod tests`):

```rust
    /// Deploys a Recharger Node directly on the player's current tile —
    /// `Game::rest` requires one nearby, so tests exercising `rest` need
    /// this in place first. Spawned directly rather than through
    /// `place_structure` to sidestep its Home/cost/radius requirements,
    /// which aren't what these tests are about — but still attaches
    /// `Temporary` from the real `recharger_node.ron` schema, the same way
    /// `place_structure` would, so its lifespan behaves identically.
    fn spawn_recharger_node_at_player(game: &mut Game) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        let max_ticks = game
            .world
            .resource::<StructureDb>()
            .get("recharger_node")
            .and_then(|d| d.temporary.as_ref())
            .expect("recharger_node.ron should define `temporary`")
            .max_ticks;
        game.world.spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Temporary {
                ticks_remaining: max_ticks,
            },
        ));
    }
```

Replace it with:

```rust
    /// Deploys a Recharger Node directly on the player's current tile —
    /// `Game::rest` requires one nearby, so tests exercising `rest` need
    /// this in place first. Spawned directly rather than through
    /// `place_structure` to sidestep its Home/cost/radius requirements,
    /// which aren't what these tests are about. The real Recharger Node is
    /// a permanent structure (no `Temporary` component), so this doesn't
    /// attach one either.
    fn spawn_recharger_node_at_player(game: &mut Game) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
        ));
    }
```

- [ ] **Step 2: Update the schema-loading test to expect no `temporary`**

Find this test (still in `crates/engine/src/lib.rs`):

```rust
    #[test]
    fn recharger_node_structure_loads_with_the_expected_rest_and_temporary_schema() {
        let game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "recharger_node")
            .expect("recharger_node.ron should load");
        assert_eq!(def.build_cost, vec![(ItemId::CoreFragment, 5)]);
        assert_eq!(def.enables_rest.as_ref().unwrap().radius, 2);
        assert_eq!(def.temporary.as_ref().unwrap().max_ticks, 20);
    }
```

Replace it with:

```rust
    #[test]
    fn recharger_node_structure_loads_with_the_expected_rest_schema_and_is_permanent() {
        let game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "recharger_node")
            .expect("recharger_node.ron should load");
        assert_eq!(def.build_cost, vec![(ItemId::CoreFragment, 5)]);
        assert_eq!(def.enables_rest.as_ref().unwrap().radius, 2);
        assert!(
            def.temporary.is_none(),
            "the Recharger Node should be a permanent structure"
        );
    }
```

- [ ] **Step 3: Delete the two decay-behavior tests**

Delete these two tests entirely from `crates/engine/src/lib.rs` — both exercise lifespan/collapse behavior that no longer exists once `temporary` is removed from the `.ron` file:

```rust
    #[test]
    fn recharger_node_collapses_after_its_lifespan_of_ordinary_ticks() {
        let mut game = Game::new(402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        spawn_recharger_node_at_player(&mut game);

        for _ in 0..20 {
            game.wait();
        }

        let mut query = game.world.query::<&Structure>();
        assert!(
            query.iter(&game.world).all(|s| s.kind != "recharger_node"),
            "the node should collapse once its 20-tick lifespan elapses"
        );
    }

    #[test]
    fn resting_does_not_age_the_recharger_node() {
        let mut game = Game::new(403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        spawn_recharger_node_at_player(&mut game);

        game.rest(); // burns REST_TICKS (40) internally — past the node's own 20-tick lifespan

        let mut query = game.world.query::<&Structure>();
        assert!(
            query.iter(&game.world).any(|s| s.kind == "recharger_node"),
            "ticks spent resting shouldn't count against the node's own lifespan"
        );
    }
```

- [ ] **Step 4: Run the tests to confirm they fail against the still-unchanged `.ron` file**

Run: `cargo test -p feral-processes-engine recharger_node -- --nocapture`

Expected: `recharger_node_structure_loads_with_the_expected_rest_schema_and_is_permanent` FAILS — `def.temporary.is_none()` assertion fails, because `recharger_node.ron` still sets `temporary: Some((max_ticks: 20))`. (The other Recharger Node tests should still pass since the test helper no longer touches `Temporary` at all yet.)

- [ ] **Step 5: Remove `temporary` from the Recharger Node's `.ron` file**

Current contents of `assets/structures/recharger_node.ron`:

```ron
(
    id: "recharger_node",
    name: "Recharger Node",
    glyph: 'z',
    color: Orange,
    build_cost: [(CoreFragment, 5)],
    work: None,
    enables_rest: Some((radius: 2)),
    temporary: Some((max_ticks: 20)),
)
```

Replace with:

```ron
(
    id: "recharger_node",
    name: "Recharger Node",
    glyph: 'z',
    color: Orange,
    build_cost: [(CoreFragment, 5)],
    work: None,
    enables_rest: Some((radius: 2)),
)
```

- [ ] **Step 6: Run the full engine test suite to confirm everything passes**

Run: `cargo test --workspace`

Expected: all tests pass, including `recharger_node_structure_loads_with_the_expected_rest_schema_and_is_permanent`. No test named `recharger_node_collapses_after_its_lifespan_of_ordinary_ticks` or `resting_does_not_age_the_recharger_node` should appear in the output (they were deleted in Step 3).

- [ ] **Step 7: Fix the now-stale `temporary` example in `assets/structures/README.md`**

Current text (around the `temporary` field's doc comment):

```
    // Optional; can be left out entirely (defaults to a permanent
    // structure). If set, this structure automatically collapses once
    // `max_ticks` ordinary game-clock ticks have passed since it was
    // deployed — no refund, it just disappears. Ticks spent inside a
    // `Game::rest` cycle don't count toward this, so a structure that also
    // sets `enables_rest` (like the Recharger Node) isn't worn down any
    // faster by actually being used to rest than by sitting there idle.
    temporary: Some((max_ticks: 20)),
```

The Recharger Node no longer sets `temporary` (it's permanent now), so citing it here as the example is wrong. Replace with a generic explanation that doesn't name a specific structure:

```
    // Optional; can be left out entirely (defaults to a permanent
    // structure). If set, this structure automatically collapses once
    // `max_ticks` ordinary game-clock ticks have passed since it was
    // deployed — no refund, it just disappears. Ticks spent inside a
    // `Game::rest` cycle don't count toward this, so a structure that also
    // sets `enables_rest` isn't worn down any faster by actually being
    // used to rest than by sitting there idle.
    temporary: Some((max_ticks: 20)),
```

- [ ] **Step 8: Commit**

```bash
git add assets/structures/recharger_node.ron assets/structures/README.md crates/engine/src/lib.rs
git commit -m "Make the Recharger Node a permanent structure"
```
