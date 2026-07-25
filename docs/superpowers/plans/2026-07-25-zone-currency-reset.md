# Zone Currency Reset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Breaching into a new zone wipes the player's spendable currency, so each zone must fund its own exit — with a softer Portal cost ramp so the from-zero grind doesn't double-dip.

**Architecture:** Two independent changes in `crates/engine`. The cost ramp replaces the `qty * zone` multiplier in `Game::structure_build_cost` with a shared `zone_portal_cost` helper that the offline balance projection also calls. The wipe drops the `Currency` and `CraftCurrency` stacks from the player's `Inventory` at the end of `Game::enter_next_zone` and logs what was lost.

**Tech Stack:** Rust, `bevy_ecs` (standalone), RON data files under `assets/`.

Spec: `docs/superpowers/specs/2026-07-25-zone-currency-reset-design.md`

## Global Constraints

- Engine logic keys on `EconomyRole`, never on item ids. `items::ids::*` is
  for test setup and data-defined recipes only (`items.rs:35-38`). A mod
  that ships its own currency item must get the wipe for free.
- No hardcoded game content in Rust that could be data (`CLAUDE.md`).
- Named constants over magic numbers.
- Comments explain *why*, never *what*.
- `cargo fmt` and `cargo clippy --workspace` clean after every task.
- `cargo test --workspace` (433 tests, ~3s) is the gate before any commit.
- No save-format version bump: `Inventory` serialises as
  `Vec<(ItemId, u32)>` either way, and the wipe is state, not schema.

**Deviation from the spec, deliberate:** the spec put `zone_portal_cost` in
`balance.rs`. That module's own header declares it "Offline balance
projections … decoupled from the ECS", so a live pricing rule living there
would make gameplay depend on the projections module. The function and its
constant go in `lib.rs` beside `PORTAL_FRAGMENT_DROP_CHANCE` instead, and
`balance.rs` calls into it — projection depends on rule, not the reverse.
Everything else in the spec is unchanged.

---

### Task 1: Softer Portal cost ramp

**Files:**
- Modify: `crates/engine/src/lib.rs:229` (add constant + helper beside `PORTAL_FRAGMENT_DROP_CHANCE`)
- Modify: `crates/engine/src/lib.rs:6124` (`Game::structure_build_cost`)
- Modify: `crates/engine/src/balance.rs:137-161` (`ticks_to_afford_portal`)
- Modify: `crates/engine/src/lib.rs:15812` (existing test asserting the old ×zone numbers)
- Modify: `assets/structures/README.md:91-95`
- Test: `crates/engine/src/lib.rs` (the `#[cfg(test)] mod tests` block, alongside the existing portal-cost test)

**Interfaces:**
- Produces: `pub(crate) fn zone_portal_cost(base_qty: u32, zone: u32) -> u32` at crate root, and `const ZONE_PORTAL_COST_GROWTH_PERCENT: u32 = 50`. Task 2 does not use either.

- [ ] **Step 1: Write the failing test**

Add to the test module, next to `portal_build_cost_scales_with_current_zone_level`:

```rust
    #[test]
    fn portal_cost_grows_by_half_the_base_rate_per_zone() {
        let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let portal = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "portal")
            .expect("portal.ron should load");
        let fragments = |game: &Game, def: &StructureDef| {
            game.structure_build_cost(def)
                .into_iter()
                .find(|(item, _)| item.as_str() == ids::PORTAL_FRAGMENT)
                .map(|(_, qty)| qty)
                .expect("a portal is bought with portal fragments")
        };

        assert_eq!(fragments(&game, &portal), 10, "zone 1 pays the base rate");

        game.world.insert_resource(ZoneLevel(2));
        assert_eq!(
            fragments(&game, &portal),
            15,
            "each zone adds half the base rate, not another whole one"
        );

        game.world.insert_resource(ZoneLevel(5));
        assert_eq!(
            fragments(&game, &portal),
            30,
            "the ramp stays linear in the base rate all the way down"
        );

        let node = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "mining_node")
            .expect("mining_node.ron should load");
        assert_eq!(
            game.structure_build_cost(&node),
            node.build_cost,
            "only a zone-portal structure scales; everything else is flat at any depth"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine portal_cost_grows_by_half_the_base_rate_per_zone`
Expected: FAIL — `assertion left == right` with left `20`, right `15` (the current `qty * zone` doubles at zone 2).

- [ ] **Step 3: Add the constant and the shared helper**

In `crates/engine/src/lib.rs`, immediately after `PORTAL_FRAGMENT_DROP_CHANCE` (line 229):

```rust
/// How much of a zone-portal structure's base `build_cost` is added to its
/// price per zone below the current one. Breaching deeper costs more, but
/// currency does not survive the trip (see `Game::enter_next_zone`), so
/// this is a ramp on a from-zero grind rather than a tax on a stockpile —
/// which is why it adds half the base rate per zone instead of doubling.
const ZONE_PORTAL_COST_GROWTH_PERCENT: u32 = 50;

/// The quantity a zone-portal structure costing `base_qty` of an item
/// charges at `zone`. Shared with `balance::ticks_to_afford_portal` so a
/// projection can't drift from the price the game actually charges.
pub(crate) fn zone_portal_cost(base_qty: u32, zone: u32) -> u32 {
    base_qty + base_qty * ZONE_PORTAL_COST_GROWTH_PERCENT * zone.saturating_sub(1) / 100
}
```

`saturating_sub` because `ZoneLevel` is a `u32` starting at 1 and a zone 0
would otherwise underflow. Integer division comes last so the shipped
10-fragment Portal yields exactly 10 / 15 / 20 / 25 / 30.

- [ ] **Step 4: Rewrite `Game::structure_build_cost`**

Replace the whole body at `crates/engine/src/lib.rs:6124`:

```rust
    pub fn structure_build_cost(&self, def: &StructureDef) -> Vec<(ItemId, u32)> {
        if !def.zone_portal {
            return def.build_cost.clone();
        }
        let zone = self.world.resource::<ZoneLevel>().0;
        def.build_cost
            .iter()
            .map(|(item, qty)| (item.clone(), zone_portal_cost(*qty, zone)))
            .collect()
    }
```

Update its doc comment above (lines 6119-6123): it currently says "each
amount scaled by the current zone level". It should say each amount grows
by `ZONE_PORTAL_COST_GROWTH_PERCENT` of the base rate per zone.

- [ ] **Step 5: Run the new test to verify it passes**

Run: `cargo test -p feral-processes-engine portal_cost_grows_by_half_the_base_rate_per_zone`
Expected: PASS.

- [ ] **Step 6: Point the balance projection at the same helper**

In `crates/engine/src/balance.rs`, inside `ticks_to_afford_portal`, replace:

```rust
    // A Portal's build_cost is a per-zone-level rate (see
    // `StructureDef::zone_portal`), and fragments are bought with the
    // currency the base actually produces.
    let needed = (portal_fragment_rate * zone * market_price) as f64;
```

with:

```rust
    // Priced through the same helper the game charges with (see
    // `crate::zone_portal_cost`), and fragments are bought with the
    // currency the base actually produces.
    let needed = (crate::zone_portal_cost(portal_fragment_rate, zone) * market_price) as f64;
```

- [ ] **Step 7: Rewrite the existing cost test for the new numbers**

`portal_build_cost_scales_with_current_zone_level` (lib.rs:15812) tops up
to 19 expecting failure and to 20 expecting success at zone 2. Zone 2 now
costs 15. Replace the zone-2 half of the test (from the `// Zone 2:`
comment through the closing brace) with:

```rust
        // Zone 2: base rate plus half of it again (10 + 5 = 15), not double.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 14);
        assert!(
            game.place_structure("portal", 1, 0).is_err(),
            "14 fragments shouldn't be enough for a zone-2 portal"
        );
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 1);
        game.place_structure("portal", 1, 0).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
            0,
            "zone 2 portal should cost the base rate plus half again"
        );
    }
```

Also rename the test to `portal_build_cost_ramps_with_current_zone_level`
— "scales" now overstates what it does. Grep for the old name first in
case anything references it:
`rg -n portal_build_cost_scales_with_current_zone_level`.

- [ ] **Step 8: Update the modding schema doc**

In `assets/structures/README.md`, the comment block at lines 91-95 above
`zone_portal: true`. It documents that the structure is consumed on use;
add the pricing rule:

```
    // A zone-portal structure is consumed when the player steps onto it: it
    // does not travel to the next zone the way the rest of the base does
    // (see `enables_rest`/`Game::enter_next_zone`), so every breach costs a
    // fresh build. Its `build_cost` is a base rate: each entry grows by 50%
    // of that rate per zone level, so a 10-fragment portal costs 10 in zone
    // 1, 15 in zone 2, 20 in zone 3. No other structure's cost scales.
    zone_portal: true,
```

- [ ] **Step 9: Verify the whole workspace**

Run, in order:

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: fmt silent, clippy no warnings, all tests pass. Pay attention to
`a_tiered_base_funds_deeper_portals_faster_than_a_fresh_one_funds_shallow_ones`
in `balance.rs` — it is the guard on this arithmetic. It should still pass
(cost now grows more slowly while node payout still doubles per zone). If
it fails, report the numbers rather than relaxing the assertion.

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src/lib.rs crates/engine/src/balance.rs assets/structures/README.md
git commit -m "feat: ramp portal cost by half the base rate per zone

Was a full multiple of the base rate per zone. With currency about to stop
surviving a breach, that would be a from-zero grind of 50 fragments at
zone 5; +50% per zone keeps the ramp without the double-dip."
```

---

### Task 2: Currency does not survive a breach

**Files:**
- Modify: `crates/engine/src/lib.rs:4446` (`Game::enter_next_zone`, at the end, before `spawn_initial_creatures`)
- Modify: `crates/engine/src/lib.rs:6019` (`Game::describe_structure`'s `zone_portal` line)
- Modify: `assets/structures/README.md` (the same `zone_portal` comment block Task 1 touched)
- Test: `crates/engine/src/lib.rs` (the `#[cfg(test)] mod tests` block, alongside the other `enter_next_zone` tests at lib.rs:15584-15711)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: no new public API. `Game::enter_next_zone` keeps its signature (`fn enter_next_zone(&mut self)`, private).

- [ ] **Step 1: Write the three failing tests**

Add to the test module, after
`zone_transition_carries_tamed_companions_and_the_base_but_leaves_wild_creatures_behind`:

```rust
    #[test]
    fn breaching_wipes_the_currency_and_craft_currency_stacks() {
        let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.add(ItemId::from(ids::PORTAL_FRAGMENT), 25);
            inv.add(ItemId::from(ids::CORE_FRAGMENT), 40);
        }

        game.enter_next_zone();

        assert_eq!(
            count_item(&game, ids::PORTAL_FRAGMENT),
            0,
            "the next zone's portal has to be funded in the zone you leave from"
        );
        assert_eq!(
            count_item(&game, ids::CORE_FRAGMENT),
            0,
            "and so does everything the base is bought with"
        );
    }

    #[test]
    fn breaching_keeps_everything_that_is_not_spendable_currency() {
        let mut game = Game::new(946, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.add(ItemId::from(ids::RESEARCH_DATA), 60);
            inv.add(ItemId::from(ids::POWER_CELL), 4);
        }
        game.world
            .get_mut::<ItemFusions>(player)
            .unwrap()
            .increment(ItemId::from(ids::ICE_BREAKER));

        game.enter_next_zone();

        assert_eq!(
            count_item(&game, ids::RESEARCH_DATA),
            60,
            "banked research is progress, not pocket money"
        );
        assert_eq!(
            count_item(&game, ids::POWER_CELL),
            7,
            "3 from the starting kit plus the 4 added; supplies are carried, not confiscated"
        );
        assert_eq!(
            count_item(&game, ids::ICE_BREAKER),
            3,
            "the starting kit's catalysts make the trip too"
        );
        assert_eq!(
            game.world
                .get::<ItemFusions>(player)
                .unwrap()
                .tier(&ItemId::from(ids::ICE_BREAKER)),
            1,
            "fusion progress is not currency"
        );
    }

    #[test]
    fn the_decohere_message_only_fires_when_there_was_something_to_lose() {
        let mut game = Game::new(947, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ItemId::from(ids::CORE_FRAGMENT), u32::MAX);

        game.enter_next_zone();

        assert!(
            !game
                .message_log(20)
                .iter()
                .any(|(_, m)| m.contains("decohere")),
            "an empty wallet shouldn't be announced as a loss"
        );

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 3);
        game.enter_next_zone();

        assert!(
            game.message_log(20)
                .iter()
                .any(|(_, m)| m.contains("3 Portal Fragments")),
            "a real loss is named and counted: {:?}",
            game.message_log(20)
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine breaching_wipes_the_currency && cargo test -p feral-processes-engine breaching_keeps_everything && cargo test -p feral-processes-engine the_decohere_message`

Expected: the first fails with left `25`, right `0`; the third fails on the
missing "3 Portal Fragments" message. The second **passes already** — it
guards behaviour the wipe must not break, so a pass here is correct, not a
sign the test is wrong.

- [ ] **Step 3: Implement the wipe and the log**

In `Game::enter_next_zone`, replace the tail of the function — the
existing breach `self.log(...)` and the `spawn_initial_creatures` call —
with:

```rust
        // Currency is zone-local: the next breach has to be funded in the
        // zone you leave from, so a stockpile can't chain breaches past
        // content it never engaged with. Keyed on economy role, so a mod's
        // own currency item resets without an engine change. Research Data
        // is banked progress rather than spending money and survives.
        let spendable = [self.currency(), self.craft_currency()];
        let player = self.player_entity();
        let lost: Vec<(ItemId, u32)> = {
            let mut inventory = self.world.get_mut::<Inventory>(player).unwrap();
            spendable
                .into_iter()
                .filter_map(|item| {
                    let qty = inventory.take(item.clone(), u32::MAX);
                    (qty > 0).then_some((item, qty))
                })
                .collect()
        };

        self.log(format!(
            "You breach the portal and materialize in a level {new_level} sector. Hostile signal strength has spiked."
        ));
        if !lost.is_empty() {
            let manifest = lost
                .iter()
                .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                .collect::<Vec<_>>()
                .join(" and ");
            self.log(format!(
                "Your caches decohere in transit — {manifest} lost to the breach."
            ));
        }
        self.spawn_initial_creatures(14);
    }
```

Note the ordering: `spendable` is bound before `self.world.get_mut`
because `Game::currency`/`craft_currency` borrow `self` immutably and
return owned `ItemId`s. `item_name` returns a `&str` borrowed from `self`,
so `manifest` is built into an owned `String` before `self.log` takes
`&mut self`.

Update the doc comment on `enter_next_zone` (lib.rs:4436-4445) — it lists
what travels and what is left behind, and currency now belongs in the
left-behind half.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine breaching_ && cargo test -p feral-processes-engine the_decohere_message`
Expected: PASS, including the other five `breaching_*` tests that were
already green.

- [ ] **Step 5: Update the player-facing structure description**

`Game::describe_structure` (lib.rs:6019) currently pushes:

```rust
            parts.push("breaches to the next zone; cost scales with zone level".to_string());
```

Replace with:

```rust
            parts.push(
                "breaches to the next zone; fragments and cores don't survive the trip".to_string(),
            );
```

The assertion at lib.rs:7910 only checks the string contains "next zone",
so it still holds — run `cargo test -p feral-processes-engine describe` to
confirm rather than assuming.

- [ ] **Step 6: Document the wipe in the modding schema**

Extend the `zone_portal` comment block in `assets/structures/README.md`
(the one Task 1 edited) with a final sentence:

```
    // Breaching also clears the player's Currency and CraftCurrency items
    // (the two economy roles in assets/items/*.ron): each zone funds its own
    // exit. ResearchCurrency is banked progress and is kept.
```

- [ ] **Step 7: Sweep the root docs for falsified claims**

Run: `rg -in "zone|fragment" README.md CHANGELOG.md`

Fix any sentence that says or implies resources carry between zones. If
`CHANGELOG.md` has an Unreleased section, add an entry naming both the
wipe and Task 1's cost ramp. If neither file makes such a claim, note that
and move on — do not invent doc changes.

- [ ] **Step 8: Verify the whole workspace**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: fmt silent, clippy no warnings, all tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/lib.rs assets/structures/README.md
git commit -m "feat: currency doesn't survive a zone breach

Portal Fragments and Core Fragments are cleared on breach, so a stockpile
farmed in an easy zone can't chain-breach past content it never engaged
with. Keyed on economy role, so a mod's own currency resets too. Research
Data is banked progress and is kept, as are gear, supplies, fusion tiers,
companions and the base."
```

---

## Notes for the implementer

- The `enter_next_zone` tests at lib.rs:15584-15711 assert on structures,
  durability, node stock, cronjob targets, and companions. None reads a
  currency count after a breach, so Task 2 should not disturb them. If one
  does break, that is a real finding — read it before changing it.
- `test_assets_dir()` (lib.rs:6448) resolves to the real `assets/` tree, so
  the `role:` tags the wipe keys on are live in tests. No fixture to update.
- If a mass of tests fails with `NotFound` on an assets path, that is stale
  build artifacts from the old `petmud` directory name, not a real
  regression. Fix with
  `cargo clean -p feral-processes-engine -p feral-processes-app-core`
  (not a full `cargo clean` — `target/` is ~4 GB).
