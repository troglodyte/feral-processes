# Tutorial Contract Chain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn onboarding into a fixed chain of contracts — one live at a time, handed to the player rather than offered, drawn green, suppressing the ordinary board until it is finished.

**Architecture:** A tutorial mission is an ordinary `ActiveContract` with an ordinary objective. `ContractDef` gains a `tutorial: Option<u32>` step; the chain is every def carrying one, in step order; the run's position in it is derived from the `ActiveContracts::done` the save already holds. `Game::ensure_tutorial_held` is the one writer and bypasses `accept_contract` entirely, so the cap, the Broker check and the offer filter are omissions rather than checks. Six new verbs arrive as one `Objective::Perform { deed }` over a closed engine enum fed by a third `RunFeats` queue; `Objective::Hold` is a sixth state-shaped objective so stock can be taught before a Broker stands.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), `serde`/`ron` for assets and saves, `bevy` + `bevy_egui` in `crates/gui`.

**Spec:** `docs/superpowers/specs/2026-08-30-tutorial-contract-chain-design.md` — read it before Task 1. It carries the argument for every decision below; this plan carries only the mechanics.

## Global Constraints

- **Branch:** `feat/tutorial-contract-chain`, already created from `origin/main`. Do **not** bump the workspace version or write a `CHANGELOG.md` section — per `CLAUDE.md`, that happens once at the merge, not on the branch.
- **No `SAVE_FORMAT_VERSION` bump.** Every save change here is additive behind `#[serde(default)]`, which is exactly what field-named RON retired migrations for. If you find yourself wanting a bump, stop and re-read the spec.
- **Moddability:** no game content in Rust. The eleven missions are `.ron` files. `Deed` is the one closed enum, and it is closed because a deed is an engine event a mod cannot emit.
- **`assets/contracts/README.md` is the schema reference** and must be updated in the same task as any schema change, never later.
- **Comments explain *why*.** A comment restating what well-named code already says is noise and will be rejected in review.
- **After every change:** `cargo fmt` and `cargo clippy --workspace`. Fix warnings; never silence them.
- **Final gate:** `cargo test --workspace`. Iterate with `cargo test -p feral-processes-engine <name>` (~6.7s); the workspace run is for task boundaries.
- **Never `git add -A`.** Stage explicit paths — the working tree has unrelated untracked files.
- **Do not push.** Commit freely; pushing needs the user's explicit ask.

---

### Task 1: `ContractDef::tutorial`, the load refusals, and the chain

**Files:**
- Modify: `crates/engine/src/contracts.rs` — `ContractDef`, `complaint`, `ContractDb::load_dir`, new `ContractDb::tutorial_chain`
- Modify: `assets/contracts/README.md`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `ContractDef::tutorial: Option<u32>` (public field, `#[serde(default)]`)
  - `ContractDb::tutorial_chain(&self) -> Vec<&ContractDef>`

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/contracts.rs`. `contract_db_from` is a new local helper — write it too; it writes bodies into a scratch directory and loads them, which is how the existing refusal tests in this file already work (see `shipped_contracts` and the tests around line 200 for the shape).

```rust
/// Loads a `ContractDb` from `bodies` written into a scratch directory.
/// A local helper rather than one in `support.rs`: only this file loads a
/// contracts directory in isolation, and `support`'s builders all copy the
/// whole shipped asset tree, which is the opposite of what these want.
fn contract_db_from(tag: &str, bodies: &[(&str, &str)]) -> (ContractDb, Vec<String>) {
    let dir = crate::tests::support::scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    for (name, body) in bodies {
        std::fs::write(dir.join(name), body).unwrap();
    }
    let loaded = ContractDb::load_dir(&dir).unwrap();
    loaded
}

/// The chain is every def carrying a step, in step order — not file order,
/// not id order. Written with the files deliberately out of order so a
/// `read_dir` that happened to return them sorted cannot pass this by luck.
#[test]
fn the_tutorial_chain_is_every_stepped_contract_in_step_order() {
    let (db, warnings) = contract_db_from(
        "tutorial_chain_order",
        &[
            ("z_third.ron", r#"(id: "third", name: "Third", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(30))"#),
            ("a_first.ron", r#"(id: "first", name: "First", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#),
            ("m_plain.ron", r#"(id: "plain", name: "Plain", description: "d", objective: Breach(zone: 2), reward: [Xp(1)])"#),
            ("b_second.ron", r#"(id: "second", name: "Second", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(20))"#),
        ],
    );
    assert!(warnings.is_empty(), "all four are valid: {warnings:?}");
    let chain: Vec<&str> = db.tutorial_chain().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        chain,
        vec!["first", "second", "third"],
        "the chain is step order, and a contract with no step is not in it"
    );
}

/// A directory with no stepped contract has no chain, which is the
/// pre-tutorial game and a supported install.
#[test]
fn a_directory_with_no_stepped_contract_has_no_chain() {
    let (db, _) = contract_db_from(
        "tutorial_chain_empty",
        &[("plain.ron", r#"(id: "plain", name: "Plain", description: "d", objective: Breach(zone: 2), reward: [Xp(1)])"#)],
    );
    assert!(db.tutorial_chain().is_empty());
}

/// Two files claiming one step would run the chain in an order nobody
/// authored, and which of them won would depend on `read_dir`. The second is
/// skipped with a warning, exactly as a duplicate id is.
#[test]
fn a_duplicate_tutorial_step_is_refused() {
    let (db, warnings) = contract_db_from(
        "tutorial_dup_step",
        &[
            ("a.ron", r#"(id: "a", name: "A", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#),
            ("b.ron", r#"(id: "b", name: "B", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#),
        ],
    );
    assert_eq!(db.tutorial_chain().len(), 1, "one of the two is kept");
    assert_eq!(warnings.len(), 1, "and the other is warned about: {warnings:?}");
    assert!(
        warnings[0].contains("step"),
        "the warning names what collided: {warnings:?}"
    );
}

/// `load_dir` sorts its entries, so two files claiming one step resolve the
/// same way every run. Without the sort the survivor above is whichever
/// `read_dir` happened to yield first, and the shipped chain would differ
/// between machines.
#[test]
fn a_duplicate_tutorial_step_resolves_the_same_way_every_run() {
    for i in 0..4 {
        let (db, _) = contract_db_from(
            &format!("tutorial_dup_stable_{i}"),
            &[
                ("zzz.ron", r#"(id: "zzz", name: "Z", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#),
                ("aaa.ron", r#"(id: "aaa", name: "A", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#),
            ],
        );
        assert_eq!(
            db.tutorial_chain()[0].id.as_str(),
            "aaa",
            "the file that sorts first is the one that loads"
        );
    }
}

/// A tutorial mission is never offered, so a `starter` flag on one is a
/// claim about a board slot it can never occupy.
#[test]
fn a_tutorial_mission_may_not_also_be_a_starter() {
    let (db, warnings) = contract_db_from(
        "tutorial_and_starter",
        &[("a.ron", r#"(id: "a", name: "A", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10), starter: true)"#)],
    );
    assert_eq!(db.iter().count(), 0);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("starter"), "{warnings:?}");
}

/// The chain's position is derived from `done`, so a repeatable mission
/// would leave and re-enter it forever.
#[test]
fn a_tutorial_mission_may_not_be_repeatable() {
    let (db, warnings) = contract_db_from(
        "tutorial_and_repeatable",
        &[("a.ron", r#"(id: "a", name: "A", description: "d", objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10), repeatable: true)"#)],
    );
    assert_eq!(db.iter().count(), 0);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("repeatable"), "{warnings:?}");
}
```

You will need `use crate::contracts::ContractDb;` in scope — check the file's existing imports first; `ContractDb` is already used by `shipped_contracts`.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine tutorial
```

Expected: FAIL to compile — `no field 'tutorial' on type 'ContractDef'` and `no method named 'tutorial_chain'`.

- [ ] **Step 3: Add the field**

In `crates/engine/src/contracts.rs`, in `ContractDef`, immediately after the existing `starter` field:

```rust
    /// Which step of the onboarding chain this mission is, if any. Absent on
    /// an ordinary contract, which is every shipped contract but eleven.
    ///
    /// A **step, not an index**: the shipped missions are spaced 10 apart so
    /// inserting one later never renumbers the others. The chain itself is
    /// `ContractDb::tutorial_chain`, and the run's position in it is derived
    /// from `ActiveContracts::done` rather than stored — see
    /// `Game::ensure_tutorial_held`.
    ///
    /// Refused at load beside `starter` or `repeatable`: a tutorial mission
    /// is never offered, so a board-slot flag on one is a claim about
    /// something that cannot happen, and a repeatable one would leave and
    /// re-enter the chain forever.
    ///
    /// `min_zone` is not refused here but is inert — nothing gates a mission
    /// the player is handed.
    #[serde(default)]
    pub tutorial: Option<u32>,
```

- [ ] **Step 4: Add the two `complaint` refusals**

In `fn complaint`, immediately before the final `None`:

```rust
    if def.tutorial.is_some() && def.starter {
        return Some(
            "a tutorial mission is handed to the player, never offered, so it cannot \
             also be a starter — a starter flag on one claims a board slot it can \
             never occupy"
                .to_string(),
        );
    }
    if def.tutorial.is_some() && def.repeatable {
        return Some(
            "a tutorial mission cannot be repeatable: the chain's position is derived \
             from what has been finished, so a repeatable one would leave and re-enter \
             it forever"
                .to_string(),
        );
    }
```

- [ ] **Step 5: Sort the entries and refuse a duplicate step in `load_dir`**

`ContractDb::load_dir` currently walks `read_dir` in whatever order the filesystem yields, which the duplicate-**id** check already silently depends on. A duplicate *step* makes that visible, so sort first — the rule `MemoryDb` and `NeedDb` already follow.

Replace the `for entry in entries {` loop head with a collected, sorted pass:

```rust
        // Sorted before parsing, `MemoryDb::load_dir`'s rule: two files
        // claiming one id — or, now, one tutorial step — have to resolve the
        // same way on every machine, and `read_dir` gives no such promise.
        let mut paths: Vec<std::path::PathBuf> =
            entries.map(|e| e.map(|e| e.path())).collect::<std::io::Result<_>>()?;
        paths.sort();
        for path in paths {
```

(The body loses its `let path = entry?.path();` line; everything else in the loop is unchanged.)

Then, in the `match complaint(&def)` arms, add a third guard between the duplicate-id arm and the final `None` arm:

```rust
                    None if def.tutorial.is_some()
                        && db.defs.values().any(|d| d.tutorial == def.tutorial) =>
                    {
                        warnings.push(format!(
                            "skipped invalid contract file {path:?}: tutorial step {} is \
                             already taken",
                            def.tutorial.expect("guarded by is_some above")
                        ))
                    }
```

- [ ] **Step 6: Add `tutorial_chain`**

In `impl ContractDb`, beside `iter`:

```rust
    /// The onboarding chain: every def carrying a `tutorial` step, in step
    /// order. The one derivation of what the chain is.
    ///
    /// Sorted by step and then by id. The second key is unreachable while
    /// `load_dir` refuses a duplicate step; it is here so the order is total
    /// on its own rather than resting on that refusal.
    pub fn tutorial_chain(&self) -> Vec<&ContractDef> {
        let mut chain: Vec<&ContractDef> =
            self.defs.values().filter(|d| d.tutorial.is_some()).collect();
        chain.sort_by(|a, b| (a.tutorial, &a.id).cmp(&(b.tutorial, &b.id)));
        chain
    }
```

- [ ] **Step 7: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine tutorial
cargo test -p feral-processes-engine contracts
```

Expected: PASS, both.

- [ ] **Step 8: Update the README**

In `assets/contracts/README.md`, add to the schema block after `starter`:

```ron
    // Optional. Which step of the onboarding chain this is. See "The
    // tutorial chain" below. Absent on an ordinary contract.
    tutorial: Some(60),
```

And add a section after "Starters":

````markdown
## The tutorial chain

A contract carrying a `tutorial` step is an **onboarding mission**. The chain
is every such contract in step order, and it behaves unlike everything else
here:

- It is **handed to the player, never offered**. It appears under *Held* at
  the start of a run, with no Broker standing and no key pressed. There is
  nothing to accept and nothing to decline.
- **One is live at a time.** Finishing one hands out the next in the same
  tick.
- It **does not count against `MAX_ACTIVE_CONTRACTS`**, and it cannot be
  given back with `[A]`.
- While the chain is unfinished the **ordinary board is empty**. When the
  last mission completes the board fills normally, starters first.
- It draws **green** on the contracts screen.

The number is a *step*, not an index. The shipped missions are spaced 10
apart so a mission inserted later never renumbers the others. Two files
claiming one step is refused at load, as is `tutorial` beside `starter` or
`repeatable`. `min_zone` on a tutorial mission is inert.

The chain runs on new games only. A save made before this existed has every
mission filed as finished at load.

**A mission must be finishable, or onboarding stops for the rest of the
run.** The shipped chain is held to that by three tests over the real assets
in `crates/engine/src/tests/assets.rs`: its build costs never outrun its
payouts, every `Deed` it names has an emit site, and every id it names
resolves. A mod is not checked, which is the line every content directory
here draws.
````

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/contracts.rs crates/engine/src/tests/contracts.rs assets/contracts/README.md
git commit -m "feat(contracts): a contract may carry an onboarding step"
```

---

### Task 2: `ObjectiveState` and `Objective::Hold`

**Files:**
- Modify: `crates/engine/src/contracts.rs` — `Objective`, `Objective::target`, `Objective::already_met`, new `ObjectiveState`
- Modify: `crates/engine/src/game/contracts.rs` — `contract_system`, `Game::offerable`, `Game::objective_line`, new `Game::objective_state`
- Modify: `assets/contracts/README.md`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: Task 1's `ContractDef::tutorial` (only so the file compiles; unused here).
- Produces:
  - `contracts::ObjectiveState { depth: u32, zone: u32, standing: Vec<StructureId>, carried: Vec<(ItemId, u32)> }` with `pub fn count(&self, item: &ItemId) -> u32`
  - `Objective::already_met(&self, state: &ObjectiveState) -> bool` — **signature changed** from `(depth, zone, standing)`
  - `Objective::Hold { item: ItemId, count: u32 }`
  - `Game::objective_state(&self) -> ObjectiveState`

`Hold` is **latched and state-shaped**, not counted: `target()` is 1, and `already_met` is `carried >= count`. That is deliberate — every polled objective in this engine is a latch, `contract_system` advances them with `saturating_add`, and a counted `Hold` would either accumulate wrongly or need a second "set, don't add" path. Latching also means spending the stock right after does not un-finish the mission.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/contracts.rs`:

```rust
/// `Hold` is met by what is in the pack, needs no Broker, and is what lets
/// the chain teach "fighting pays in stock" before one is standing.
#[test]
fn hold_is_met_by_what_the_player_is_carrying() {
    let objective = Objective::Hold {
        item: ItemId::from(crate::items::ids::CORE_FRAGMENT),
        count: 12,
    };
    let mut state = crate::contracts::ObjectiveState {
        depth: 0,
        zone: 1,
        standing: Vec::new(),
        carried: vec![(ItemId::from(crate::items::ids::CORE_FRAGMENT), 11)],
    };
    assert!(!objective.already_met(&state), "eleven is not twelve");
    state.carried[0].1 = 12;
    assert!(objective.already_met(&state));
    state.carried[0].1 = 40;
    assert!(objective.already_met(&state), "more than asked still counts");
}

/// State-shaped, so it completes through the one `progress >= target` rule
/// with a target of 1 — the same shape `Build` and `Descend` have.
#[test]
fn hold_is_a_latch_with_a_target_of_one() {
    let objective = Objective::Hold {
        item: ItemId::from(crate::items::ids::CORE_FRAGMENT),
        count: 12,
    };
    assert_eq!(objective.target(), 1);
}

/// Carrying nothing of the item at all is the common case and must not
/// panic or read as met.
#[test]
fn hold_is_not_met_by_an_empty_pack() {
    let objective = Objective::Hold {
        item: ItemId::from(crate::items::ids::CORE_FRAGMENT),
        count: 1,
    };
    let state = crate::contracts::ObjectiveState {
        depth: 0,
        zone: 1,
        standing: Vec::new(),
        carried: Vec::new(),
    };
    assert!(!objective.already_met(&state));
}

/// A held `Hold` finishes off the run's own inventory, through the ordinary
/// tick path and the ordinary completion path.
#[test]
fn a_held_hold_completes_from_the_players_pack() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let item = ItemId::from(crate::items::ids::CORE_FRAGMENT);
    game.world
        .get_mut::<crate::components::Inventory>(game.player_entity())
        .unwrap()
        .add(item.clone(), 100);
    game.world
        .resource_mut::<crate::resources::ActiveContracts>()
        .active
        .push(crate::resources::ActiveContract {
            def: crate::contracts::ContractDef {
                id: "hold_test".into(),
                name: "Hold Test".to_string(),
                description: "d".to_string(),
                objective: Objective::Hold { item, count: 12 },
                reward: vec![Reward::Xp(1)],
                min_zone: 0,
                repeatable: false,
                starter: false,
                tutorial: None,
            },
            progress: 0,
            accepted_tick: 0,
        });
    game.tick();
    assert!(
        game.world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&"hold_test".into()),
        "a pack that already meets the objective finishes it on the next tick"
    );
}
```

There is **no** `test_game()` helper in this repo. The fixture idiom every
suite uses is `Game::new(<seed>, DifficultyMode::Forgiving, &test_assets_dir())`
— see `crates/engine/src/tests/work_orders.rs` for the shape. `test_assets_dir`
comes in through `use super::support::*;`. Give each test its own seed.
`Inventory::add` and `Game::player_entity` both exist; for advancing one tick,
use whatever the neighbouring contract tests already call.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine hold
```

Expected: FAIL to compile — no `Objective::Hold`, no `ObjectiveState`.

- [ ] **Step 3: Add `ObjectiveState`**

In `crates/engine/src/contracts.rs`, immediately above `impl Objective`:

```rust
/// Everything about the run a state-shaped objective can be asked against.
///
/// A struct rather than positional arguments because `Objective::already_met`
/// has two readers that must not drift — `contract_system` advances by it and
/// `Game::offerable` refuses a board slot on it — so every objective added
/// widened one signature at two call sites. A field costs neither, and the
/// next objective costs a field.
pub struct ObjectiveState {
    /// Stack depth, read from `resources::Locale` and never from `Position`,
    /// which is pinned to the surface entrance tile while underground.
    pub depth: u32,
    pub zone: u32,
    /// Every deployed structure's kind.
    pub standing: Vec<crate::structures::StructureId>,
    /// What the player is carrying, for `Objective::Hold`.
    pub carried: Vec<(ItemId, u32)>,
}

impl ObjectiveState {
    /// Units of `item` in the pack, 0 if none — carrying nothing of it is the
    /// common case, not an error.
    pub fn count(&self, item: &ItemId) -> u32 {
        self.carried
            .iter()
            .find(|(i, _)| i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }
}
```

- [ ] **Step 4: Add the `Hold` variant and rework the two methods**

In `enum Objective`, after `Build`:

```rust
    /// This many of an item are in the player's pack **at once**.
    ///
    /// Not `Deliver`: nothing is handed over and nothing is spent, so it
    /// needs no Broker and can be met four frames down. That is why it
    /// exists — the onboarding chain has to teach that fighting pays in
    /// stock before a Contract Broker has been built.
    ///
    /// State-shaped and **latched**, like `Build` and `Descend`: once met it
    /// stays met, so spending the stock on the next thing the chain asks for
    /// does not un-finish it.
    Hold { item: ItemId, count: u32 },
```

In `target()`, `Hold` joins the three state-shaped variants returning 1:

```rust
            Objective::Descend { .. }
            | Objective::Breach { .. }
            | Objective::Build { .. }
            | Objective::Hold { .. } => 1,
```

Replace `already_met` wholesale:

```rust
    pub fn already_met(&self, state: &ObjectiveState) -> bool {
        match self {
            Objective::Terminate { .. } | Objective::Deliver { .. } => false,
            Objective::Descend { depth } => state.depth >= *depth,
            Objective::Breach { zone } => state.zone >= *zone,
            Objective::Build { structure } => state.standing.contains(structure),
            Objective::Hold { item, count } => state.count(item) >= *count,
        }
    }
```

Keep the existing doc comment on `already_met` — its two-readers argument is still exactly right — and add one line to it noting that the arguments moved into `ObjectiveState` and why.

- [ ] **Step 5: Add `Game::objective_state` and rewire the two call sites**

In `crates/engine/src/game/contracts.rs`, in `impl Game`, beside `standing_structures`:

```rust
    /// One snapshot of everything a state-shaped objective can be asked
    /// about. Built per call rather than cached, matching what
    /// `standing_structures` already did — `offerable` is asked once per
    /// def and this is no more work than the walk it replaces.
    pub(crate) fn objective_state(&self) -> crate::contracts::ObjectiveState {
        let carried = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.items.clone())
            .unwrap_or_default();
        crate::contracts::ObjectiveState {
            depth: self.stack_depth(),
            zone: self.world.resource::<ZoneLevel>().0,
            standing: self.standing_structures(),
            carried,
        }
    }
```

`Inventory::items` may be private — check. If it is, add a `pub fn entries(&self) -> &[(ItemId, u32)]` accessor to `Inventory` in `components.rs` and use that; do not widen the field's visibility. `Game::stack_depth` may not exist under that name — the board is only read on the surface so `offerable`'s depth was hardcoded 0; if there is no such helper, inline the same `Locale` match `contract_system` uses and put it in one place both can call.

In `Game::offerable`, replace the final expression:

```rust
        !def.objective.already_met(&self.objective_state())
```

and delete the two-line comment above it about depth always being 0 — it is no longer true, and `objective_state` reads the real depth.

In `contract_system`, build the state once before the loop and use it in the catch-all arm:

```rust
    let state = crate::contracts::ObjectiveState {
        depth,
        zone: zone.0,
        standing,
        carried: player.single().map(|inv| inv.items.clone()).unwrap_or_default(),
    };
```

with a new system parameter `player: Query<&Inventory, With<Player>>`, and the catch-all arm becoming `state_shaped => u32::from(state_shaped.already_met(&state))`. Rename the arm binding so it does not shadow `state`.

Check `Query::single`'s exact return shape in this `bevy_ecs` version against a neighbouring system in `crates/engine/src/systems.rs` — several already take `Query<_, With<Player>>` and will show you the idiom.

- [ ] **Step 6: Word it for the screen**

In `Game::objective_line`, add an arm:

```rust
            Objective::Hold { item, count } => {
                format!("Hold {count} {}", self.item_name(item))
            }
```

- [ ] **Step 7: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine hold
cargo test -p feral-processes-engine contracts
cargo clippy --workspace
```

Expected: PASS. `already_met`'s signature change will have broken any test calling it positionally — fix those to build an `ObjectiveState`.

- [ ] **Step 8: Update the README**

Add a row to the `objective` table:

```markdown
| `Hold(item: "core_fragment", count: 12)` | you have that many in your pack at once |
```

and a paragraph after the `Deliver` note:

```markdown
`Hold` is not `Deliver`. Nothing is handed over and nothing is spent, so it
needs no Broker and is met wherever you are, four frames down included. It is
also **latched**: once you have held the count, spending it does not
un-finish the contract.
```

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/contracts.rs crates/engine/src/game/contracts.rs crates/engine/src/tests/contracts.rs assets/contracts/README.md
git commit -m "feat(contracts): Hold is met by the pack, and already_met takes one state"
```

---

### Task 3: `Deed`, `Objective::Perform`, and the queue

**Files:**
- Modify: `crates/engine/src/contracts.rs` — new `Deed`, `Objective::Perform`
- Modify: `crates/engine/src/resources.rs` — `RunFeats::deeds`
- Modify: `crates/engine/src/game/contracts.rs` — `Game::note_deed`, `contract_system`, `Game::objective_line`
- Modify: `assets/contracts/README.md`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: Task 2's `ObjectiveState` (the `Perform` arm of `already_met` returns false, so it takes no state).
- Produces:
  - `contracts::Deed` — `Examined | Tamed | TookFromContainer | QueuedStandingOrder | UnlockedPerk | PostedStaff`
  - `Objective::Perform { deed: Deed }`
  - `Game::note_deed(&mut self, deed: Deed)` — `pub(crate)`
  - `RunFeats::deeds: Vec<Deed>`

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/contracts.rs`:

```rust
/// A deed recorded this tick finishes a held `Perform`, through the same
/// system and the same completion path a kill goes through.
#[test]
fn a_deed_finishes_a_held_perform_contract() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveContracts>()
        .active
        .push(crate::resources::ActiveContract {
            def: crate::contracts::ContractDef {
                id: "perform_test".into(),
                name: "Perform Test".to_string(),
                description: "d".to_string(),
                objective: Objective::Perform {
                    deed: crate::contracts::Deed::Examined,
                },
                reward: vec![Reward::Xp(1)],
                min_zone: 0,
                repeatable: false,
                starter: false,
                tutorial: None,
            },
            progress: 0,
            accepted_tick: 0,
        });
    game.tick();
    assert!(
        !game
            .world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&"perform_test".into()),
        "nothing has been done yet"
    );
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    assert!(
        game.world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&"perform_test".into()),
    );
}

/// A deed of the wrong kind advances nothing. Without this the six deeds
/// would be one deed with six names.
#[test]
fn a_deed_of_another_kind_advances_nothing() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::resources::ActiveContracts>()
        .active
        .push(crate::resources::ActiveContract {
            def: crate::contracts::ContractDef {
                id: "perform_test".into(),
                name: "Perform Test".to_string(),
                description: "d".to_string(),
                objective: Objective::Perform {
                    deed: crate::contracts::Deed::PostedStaff,
                },
                reward: vec![Reward::Xp(1)],
                min_zone: 0,
                repeatable: false,
                starter: false,
                tutorial: None,
            },
            progress: 0,
            accepted_tick: 0,
        });
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    assert!(
        !game
            .world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&"perform_test".into()),
        "examining is not posting staff"
    );
}

/// The queue is drained every tick by `contract_system` and by nothing else.
/// A deed left in it would finish a contract accepted long afterwards.
#[test]
fn a_deed_does_not_survive_the_tick_that_drained_it() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    assert!(
        game.world.resource::<crate::resources::RunFeats>().deeds.is_empty(),
        "the queue is drained unconditionally"
    );
}
```

`Game::note_deed` is `pub(crate)`, which these tests can reach — they are in `crates/engine/src/tests/`.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine deed
```

Expected: FAIL to compile — no `Deed`, no `note_deed`, no `RunFeats::deeds`.

- [ ] **Step 3: Add `Deed`**

In `crates/engine/src/contracts.rs`, above `enum Objective`:

```rust
/// Something the player did, recorded for `Objective::Perform`.
///
/// A **closed engine enum, not a string**. A deed is an engine *event*, not
/// content: a mod cannot emit one, so the openness a string would buy is
/// openness onto nothing. What a string would buy instead is a mission
/// naming a deed that does not exist, loading with no warning and never
/// completing — the failure the README already documents for a `Terminate`
/// naming a species that is gone, and one there is no reason to repeat where
/// the vocabulary is closed.
///
/// A deed carries **no parameters**. `QueuedStandingOrder` does not name the
/// item and `PostedStaff` does not name the structure: the mission's
/// description is where the player is told what to order and where to post,
/// and a parameterised deed would be a second place the same instruction is
/// written. A mission that genuinely has to tell two postings apart is a new
/// variant here, not a field on an existing one.
///
/// Every variant must have a caller of `Game::note_deed` — asserted
/// exhaustively by `every_deed_has_an_emit_site`, `cell_mark`'s rule, so a
/// variant with no writer fails the build rather than shipping a mission
/// that can never complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Deed {
    /// `x` found something. `Game::find_target_in_direction`.
    Examined,
    /// A decompile succeeded. `Game::attempt_decompile`.
    Tamed,
    /// The transfer screen moved something *out* of a container.
    /// `Game::transfer_items`.
    TookFromContainer,
    /// A work order was queued with `standing` set.
    /// `Game::queue_work_order`.
    QueuedStandingOrder,
    /// A Perk Point was spent. `Game::unlock_perk`.
    UnlockedPerk,
    /// A program was posted to a machine. `Game::post_worker`.
    PostedStaff,
}
```

Add the `Objective` variant after `Hold`:

```rust
    /// The player did a particular thing. The one event-shaped objective
    /// besides `Terminate`, and the whole of the onboarding chain's new
    /// vocabulary — six verbs behind one variant, because a variant each
    /// would grow every match on `Objective` and make the seventh verb a
    /// schema change.
    Perform { deed: Deed },
```

`target()`: `Perform` returns 1 — put it with the state-shaped group's arm or its own, whichever reads better once written.

`already_met()`: `Perform` joins the `false` arm beside `Terminate` and `Deliver`. It is event-shaped, so it is never *already* true; a board would otherwise refuse to offer it forever.

- [ ] **Step 4: Add the queue**

In `crates/engine/src/resources.rs`, in `RunFeats`, after `kills`:

```rust
    /// Every `contracts::Deed` the player performed this tick, for a
    /// contract's `Objective::Perform`.
    ///
    /// A **third field**, not a widening of `kills`, for the reason the
    /// second one exists: each field having exactly one drainer is what
    /// removes any ordering dependency between the systems that read them,
    /// and a shared queue would make that unsound the moment one ate the
    /// other's events. This one's single drainer is
    /// `game::contracts::contract_system`.
    pub deeds: Vec<crate::contracts::Deed>,
```

- [ ] **Step 5: Add the door and the arm**

In `crates/engine/src/game/contracts.rs`, in `impl Game`:

```rust
    /// The one door a `Deed` is written through. The six triggers are
    /// **callers of this, not writers beside it** — `Game::remember`'s rule,
    /// and what keeps "which deeds exist" answerable by reading one file.
    pub(crate) fn note_deed(&mut self, deed: crate::contracts::Deed) {
        self.world.resource_mut::<RunFeats>().deeds.push(deed);
    }
```

In `contract_system`, add the arm beside `Terminate`:

```rust
            Objective::Perform { deed } => {
                feats.deeds.iter().filter(|d| *d == deed).count() as u32
            }
```

and drain it beside the kills, with the same unconditional comment applying:

```rust
    feats.kills.clear();
    feats.deeds.clear();
```

- [ ] **Step 6: Word it for the screen**

In `Game::objective_line`, add:

```rust
            Objective::Perform { deed } => match deed {
                Deed::Examined => "Examine something with [x]".to_string(),
                Deed::Tamed => "Decompile a wild program".to_string(),
                Deed::TookFromContainer => "Take stock out of a machine with [c]".to_string(),
                Deed::QueuedStandingOrder => "Place a standing work order".to_string(),
                Deed::UnlockedPerk => "Spend a Perk Point".to_string(),
                Deed::PostedStaff => "Post a program to a machine".to_string(),
            },
```

Exhaustive on purpose — a new `Deed` fails to compile here rather than shipping a row with no words.

- [ ] **Step 7: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine deed
cargo test -p feral-processes-engine contracts
cargo clippy --workspace
```

Expected: PASS.

- [ ] **Step 8: Update the README**

Add a row to the `objective` table:

```markdown
| `Perform(deed: Examined)` | you do that particular thing once |
```

and a section after it:

```markdown
`Perform` names a **deed**, which is a fixed list rather than an id from an
asset directory: `Examined`, `Tamed`, `TookFromContainer`,
`QueuedStandingOrder`, `UnlockedPerk`, `PostedStaff`. These are things the
engine emits, so unlike a species or an item they cannot be added by a mod,
and a name outside the list is refused by `ron` at load rather than costing
you a contract that never finishes.

A deed carries no parameters. `QueuedStandingOrder` does not say which item
and `PostedStaff` does not say which machine — the contract's `description`
is where the player is told, and putting it in both places is a copy that
drifts.
```

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/contracts.rs crates/engine/src/resources.rs crates/engine/src/game/contracts.rs crates/engine/src/tests/contracts.rs assets/contracts/README.md
git commit -m "feat(contracts): six verbs behind one Perform objective"
```

---

### Task 4: The six emit sites

**Files:**
- Modify: `crates/engine/src/game/inspection.rs` — `find_target_in_direction`
- Modify: `crates/engine/src/game/combat_rewards.rs` — `attempt_decompile`
- Modify: `crates/engine/src/game/base/transfer.rs` — `transfer_items`
- Modify: `crates/engine/src/game/base/work_orders.rs` — `queue_work_order`
- Modify: `crates/engine/src/game/unlocks.rs` — `unlock_perk`
- Modify: `crates/engine/src/game/base/building.rs` — `post_worker`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: Task 3's `Game::note_deed` and `contracts::Deed`.
- Produces: nothing new. Every site is one call.

Every one of these six functions is already `&mut self` — verified. No signature changes.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/contracts.rs`. Each asserts on `RunFeats::deeds` directly rather than on a contract, so a failure names the site rather than the plumbing.

```rust
/// Every emit site, one test each. They assert on the queue rather than on a
/// finished contract so a failure names the site that stopped writing rather
/// than reading as the contract system being broken.
mod deed_sites {
    use super::*;
    use crate::contracts::Deed;

    fn deeds(game: &Game) -> Vec<Deed> {
        game.world.resource::<crate::resources::RunFeats>().deeds.clone()
    }

    #[test]
    fn examining_something_writes_a_deed() {
        let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Put something on the ray east of the player, then look at it.
        // `crates/engine/src/tests/inspection.rs`'s
        // `find_target_in_direction_finds_the_nearest_match_along_the_line`
        // is the model for placing one; build the same fixture here.
        place_target_east_of_player(&mut game);
        let found = game.find_target_in_direction(1, 0, 5);
        assert!(found.is_some(), "the fixture has to put something there");
        assert!(deeds(&game).contains(&Deed::Examined));
    }

    #[test]
    fn examining_nothing_writes_no_deed() {
        let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // An empty ray. The mission is to teach that `x` reports something,
        // so pointing it at blank ground must not complete it.
        game.find_target_in_direction(0, -1, 1);
        assert!(!deeds(&game).contains(&Deed::Examined));
    }

    /// Taking teaches pulling stock *out* of a machine, so only the take
    /// side writes. `stocked` is `tests/transfer.rs`'s fixture — move it and
    /// `player_tile` into `support.rs` in this task so both files share one
    /// copy.
    #[test]
    fn taking_from_a_container_writes_a_deed() {
        let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let at = *game.world.get::<Position>(game.player_entity()).unwrap();
        stocked(
            &mut game,
            "mining_node",
            at.x + 1,
            at.y,
            50,
            &[(ids::CORE_FRAGMENT, 10)],
        );
        game.transfer_items(&[(ItemId::from(ids::CORE_FRAGMENT), 4)], &[]);
        assert!(deeds(&game).contains(&Deed::TookFromContainer));
    }

    /// The negative half, and the one that catches a `note_deed` written
    /// unconditionally at the top of `transfer_items`: a player who only put
    /// something in has not done what the mission asks.
    #[test]
    fn only_putting_into_a_container_writes_no_deed() {
        let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let at = *game.world.get::<Position>(game.player_entity()).unwrap();
        stocked(&mut game, "depot", at.x + 1, at.y, 50, &[]);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 10);
        game.transfer_items(&[], &[(ItemId::from(ids::CORE_FRAGMENT), 4)]);
        assert!(!deeds(&game).contains(&Deed::TookFromContainer));
    }

    /// The mission asks for a *standing* order, which is the thing that
    /// keeps working without being asked again.
    #[test]
    fn a_standing_work_order_writes_a_deed() {
        let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.queue_work_order(standing_order(ids::CORE_FRAGMENT, 20))
            .unwrap();
        assert!(deeds(&game).contains(&Deed::QueuedStandingOrder));
    }

    /// And a one-off is not one. Without this the mission completes on the
    /// first order of any kind and the lesson never lands.
    #[test]
    fn a_one_off_work_order_writes_no_deed() {
        let mut game = Game::new(25, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.queue_work_order(one_off_order(ids::CORE_FRAGMENT, 20))
            .unwrap();
        assert!(!deeds(&game).contains(&Deed::QueuedStandingOrder));
    }

    /// Modelled on `tests/perks.rs::unlock_perk_spends_points_and_can_be_
    /// bought_repeatedly`.
    #[test]
    fn unlocking_a_perk_writes_a_deed() {
        let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 5;
        game.unlock_perk(Perk::KeenScavenger).unwrap();
        assert!(deeds(&game).contains(&Deed::UnlockedPerk));
    }

    /// A refusal spends nothing and must record nothing — modelled on
    /// `tests/perks.rs::unlock_perk_rejects_without_enough_points`.
    #[test]
    fn a_refused_perk_writes_no_deed() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 0;
        assert!(game.unlock_perk(Perk::ExploitFocus).is_err());
        assert!(!deeds(&game).contains(&Deed::UnlockedPerk));
    }

    /// `park_at_post()` is the helper `CLAUDE.md` names for a test that posts
    /// a program; `tests/hauling.rs` and `tests/chains.rs` both show the
    /// whole fixture.
    #[test]
    fn posting_a_worker_writes_a_deed() {
        let mut game = Game::new(28, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (worker, machine) = a_program_and_a_machine(&mut game);
        game.post_worker(worker, machine);
        assert!(deeds(&game).contains(&Deed::PostedStaff));
    }
}
```

**Six of the fixtures above do not exist yet under those names.** Build each
from the file named beside it before writing the test — copying a fixture
blind is how a vacuous test gets written:

| Fixture | Model it on |
|---|---|
| `stocked`, `player_tile` | `crates/engine/src/tests/transfer.rs`, top of file. Move both into `support.rs` in this task and update `transfer.rs` to use them from there — three call sites in two files is where a fixture earns a shared home. |
| `standing_order`, `one_off_order` | `crates/engine/src/tests/work_orders.rs`. `WorkOrder`'s exact fields and the name of its standing flag are there; write the two constructors locally. |
| `place_target_east_of_player` | `crates/engine/src/tests/inspection.rs`, `find_target_in_direction_finds_the_nearest_match_along_the_line`. |
| `a_program_and_a_machine` | `crates/engine/src/tests/hauling.rs` or `chains.rs` — both post a program to a machine and both use `park_at_post()` and `work_node_parts()`, the two helpers `CLAUDE.md` names for this. |

Check `"depot"` is the right structure id for a put-only container against
`assets/structures/` before relying on it; any structure with a `Stock` will
do, and the point of the test is the *direction*, not the building.

The `Tamed` site is not tested here — it is tested in Task 8 alongside the forced roll, because the two are one behaviour from the player's side.

Note the **negative** test beside each positive one. A `note_deed` written unconditionally at the top of `transfer_items` passes the positive test and makes the mission complete when the player only *puts* something in, which is not what it asks for.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine deed_sites
```

Expected: FAIL — the assertions on `deeds` are empty.

- [ ] **Step 3: Add the six calls**

**`find_target_in_direction`** (`game/inspection.rs`) — on the hit only, not on the `None`. The function returns `Option<InspectTarget>`; write the deed where the target is resolved, before returning it:

```rust
        // Only on a hit. Pointing `x` at blank ground reports nothing, and
        // the mission that asks for this is teaching that the key *tells you
        // something* — a deed on a miss would complete it against an empty
        // corridor.
        if found.is_some() {
            self.note_deed(crate::contracts::Deed::Examined);
        }
```

**`attempt_decompile`** (`game/combat_rewards.rs`) — after the roll succeeds, beside the line that clears `decompile_verdict`:

```rust
        self.note_deed(crate::contracts::Deed::Tamed);
```

**`transfer_items`** (`game/base/transfer.rs`) — the function returns `(Moved, Moved)` for take and give. Write the deed when the **take** side moved anything:

```rust
        // The take side only. The mission teaches pulling stock *out* of a
        // machine; a player who only put something in has not done it.
        if took_anything {
            self.note_deed(crate::contracts::Deed::TookFromContainer);
        }
```

Read `Moved`'s shape before writing `took_anything` — it may be a count, a list or a bool. Use whatever the function's own "did anything move" expression already is; there is one, because the doc comment says an empty basket is a silent no-op costing no turn.

**`queue_work_order`** (`game/base/work_orders.rs`) — on the `Ok` path only, and only for a standing order:

```rust
        if order.standing {
            self.note_deed(crate::contracts::Deed::QueuedStandingOrder);
        }
```

Place it after every refusal has been passed, so a rejected order writes nothing. Check `WorkOrder`'s field name for the standing flag — `CLAUDE.md` calls it a `standing` flag.

**`unlock_perk`** (`game/unlocks.rs`) — after the points have been spent, on the success path:

```rust
        self.note_deed(crate::contracts::Deed::UnlockedPerk);
```

**`post_worker`** (`game/base/building.rs`) — it returns `()`, so the call goes at the end:

```rust
        self.note_deed(crate::contracts::Deed::PostedStaff);
```

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine deed_sites
cargo test -p feral-processes-engine
cargo clippy --workspace
```

Expected: PASS. The whole engine suite here, not just the new tests — six call sites in six subsystems is exactly where an unnoticed borrow or ordering change lands.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/inspection.rs crates/engine/src/game/combat_rewards.rs crates/engine/src/game/base/transfer.rs crates/engine/src/game/base/work_orders.rs crates/engine/src/game/unlocks.rs crates/engine/src/game/base/building.rs crates/engine/src/tests/contracts.rs
git commit -m "feat(contracts): six deeds reach the queue through one door"
```

---

### Task 5: The chain runs — `ensure_tutorial_held`, the abandon refusal, the row flag

**Files:**
- Modify: `crates/engine/src/game/contracts.rs` — new `Game::current_tutorial`, `Game::in_tutorial`, `Game::ensure_tutorial_held`; `Game::abandon_contract`; `Game::contract_row`
- Modify: `crates/engine/src/game/turn.rs:206` area — call after `settle_contracts`
- Modify: `crates/engine/src/game/lifecycle.rs:311` and `:1236` — call at the end of `new` and `load`
- Modify: `crates/engine/src/views.rs` — `ContractRow::tutorial`
- Modify: `crates/engine/src/tests/support.rs` — new `skip_tutorial` and `assets_with_fixture_chain`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: Task 1's `ContractDb::tutorial_chain` and `ContractDef::tutorial`.
- Produces:
  - `Game::current_tutorial(&self) -> Option<ContractDef>` — `pub(crate)`
  - `Game::in_tutorial(&self) -> bool` — `pub`, read by the board and by gui
  - `Game::ensure_tutorial_held(&mut self)` — `pub(crate)`
  - `views::ContractRow::tutorial: bool`
  - `tests::support::skip_tutorial(&mut Game)` — files every chain id into `done`
  - `tests::support::assets_with_fixture_chain(&str) -> ScratchAssets` — the shipped assets plus a three-step fixture chain

`skip_tutorial` matters beyond this task: from Task 9 onward, every existing test that expects a board will need it. Write it here, before there are assets to break anything.

- [ ] **Step 1: Write the failing tests**

The fixture builder goes in `crates/engine/src/tests/support.rs`; the tests
themselves go in `crates/engine/src/tests/contracts.rs`. They use a scratch
install with fixture missions, so they are independent of what Task 9 ships —
and Task 7, in another module, reuses the same builder.

```rust
/// A scratch install whose `contracts/` directory is the shipped set plus a
/// three-step fixture chain.
///
/// Built rather than leaning on the shipped missions, so these tests keep
/// testing the *mechanism* when the shipped wording changes — and so they
/// were already passing before Task 9 shipped a chain at all.
///
/// **Put this in `support.rs`, not in this file**: Task 7's save tests live
/// in another module and need the same install.
pub(super) fn assets_with_fixture_chain(tag: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    let contracts = dir.join("contracts");
    // A scratch chain of its own, on steps no shipped mission uses.
    for (n, step) in [(1u32, 9001u32), (2, 9002), (3, 9003)] {
        std::fs::write(
            contracts.join(format!("fixture_step_{n}.ron")),
            format!(
                r#"(id: "fixture_step_{n}", name: "Fixture {n}", description: "d",
                    objective: Perform(deed: Examined), reward: [Xp(1)], tutorial: Some({step}))"#
            ),
        )
        .unwrap();
    }
    dir
}

/// A new run has the chain's first mission in hand before anything is
/// ticked and with no Broker anywhere — which is the whole reason it is
/// handed out rather than offered.
#[test]
fn a_new_run_holds_the_first_mission_with_no_broker() {
    let dir = support::assets_with_fixture_chain("chain_first");
    let game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    let held: Vec<String> = game
        .active_contracts()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert!(
        held.contains(&"fixture_step_1".to_string()),
        "the chain's first step is in hand at tick 0: {held:?}"
    );
    assert_eq!(game.broker_reach(), BrokerReach::NoBroker, "and no Broker is standing");
}

/// Exactly one, never two. The property the whole feature rests on.
#[test]
fn exactly_one_mission_is_held_at_a_time() {
    let dir = support::assets_with_fixture_chain("chain_one");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    for _ in 0..3 {
        let count = game
            .active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count();
        assert_eq!(count, 1, "one onboarding mission is live at a time");
        game.note_deed(crate::contracts::Deed::Examined);
        game.tick();
    }
}

/// Finishing one hands out the next in the same tick, so the player never
/// sees an empty slot.
#[test]
fn finishing_a_mission_hands_out_the_next_one() {
    let dir = support::assets_with_fixture_chain("chain_next");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    let held: Vec<String> = game.active_contracts().iter().map(|r| r.id.to_string()).collect();
    assert!(held.contains(&"fixture_step_2".to_string()), "{held:?}");
    assert!(!held.contains(&"fixture_step_1".to_string()), "{held:?}");
}

/// When the last one is finished the chain is over and nothing is handed
/// out again.
#[test]
fn a_finished_chain_hands_out_nothing() {
    let dir = support::assets_with_fixture_chain("chain_end");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    for _ in 0..3 {
        game.note_deed(crate::contracts::Deed::Examined);
        game.tick();
    }
    assert!(!game.in_tutorial(), "three steps, three deeds, chain over");
    game.tick();
    assert_eq!(
        game.active_contracts().iter().filter(|r| r.tutorial).count(),
        0
    );
}

/// The chain cannot be given back. An unbreakable chain with a give-back key
/// is not a chain.
#[test]
fn an_onboarding_mission_cannot_be_abandoned() {
    let dir = support::assets_with_fixture_chain("chain_abandon");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(!game.abandon_contract(&"fixture_step_1".into()));
    assert_eq!(
        game.active_contracts().iter().filter(|r| r.tutorial).count(),
        1,
        "it is still in hand"
    );
}

/// The row says it is one, which is what the renderer colours on and what
/// app-core refuses on.
#[test]
fn an_onboarding_missions_row_is_flagged() {
    let dir = support::assets_with_fixture_chain("chain_flag");
    let game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    let row = game
        .active_contracts()
        .into_iter()
        .find(|r| r.id.as_str() == "fixture_step_1")
        .expect("held");
    assert!(row.tutorial);
}

/// An install with no chain is the pre-tutorial game exactly: nothing is
/// handed out and nothing is flagged.
#[test]
fn an_install_with_no_chain_hands_out_nothing() {
    let game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.in_tutorial());
    assert_eq!(game.active_contracts().iter().filter(|r| r.tutorial).count(), 0);
}
```

`ScratchAssets` derefs to a path — check how `assets_dir_with_talents`' callers pass it to `Game::new` and match that.

`an_install_with_no_chain_hands_out_nothing` uses the shipped assets, so it will start failing at Task 9 when the chain files land. That is expected and Task 9 fixes it — it becomes a scratch install with the chain files removed. Leave a `// Task 9 converts this to a scratch install without the shipped chain.` note on it.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine chain
```

Expected: FAIL — `no method named 'in_tutorial'`, `no field 'tutorial' on ContractRow`.

- [ ] **Step 3: Add the three chain methods**

In `crates/engine/src/game/contracts.rs`, in `impl Game`:

```rust
    /// The onboarding mission the run is on, or `None` once every step is
    /// finished.
    ///
    /// **Derived, never stored**: the first mission in
    /// `ContractDb::tutorial_chain` whose id is not in
    /// `ActiveContracts::done`. There is no cursor and no index, so nothing
    /// can disagree with `done` about where the player is — the rule
    /// `views::BuildOrderRow` and `Game::morale` already follow.
    ///
    /// Cloned rather than borrowed because every caller goes on to touch
    /// `&mut self`.
    pub(crate) fn current_tutorial(&self) -> Option<crate::contracts::ContractDef> {
        let done = &self.world.resource::<ActiveContracts>().done;
        self.world
            .resource::<crate::contracts::ContractDb>()
            .tutorial_chain()
            .into_iter()
            .find(|def| !done.contains(&def.id))
            .cloned()
    }

    /// Whether onboarding is still running. The board's suppression, the
    /// forced first decompile and the renderer's green row all read this one
    /// call rather than each deciding for themselves.
    pub fn in_tutorial(&self) -> bool {
        self.current_tutorial().is_some()
    }

    /// Puts the run's current onboarding mission in hand if it is not there
    /// already. **The one writer of a tutorial contract into
    /// `ActiveContracts`**, called from `Game::new`, `Game::load` and
    /// `Game::settle_contracts`.
    ///
    /// It never goes through `accept_contract`, and three things follow as
    /// **omissions rather than checks**, which is the point of routing it
    /// this way: `MAX_ACTIVE_CONTRACTS` never sees it, so the cap keeps
    /// meaning what it meant; `broker_reach` never sees it, which is what
    /// lets the first five missions exist before a Contract Broker does; and
    /// `offerable` never sees it, so no `min_zone` or `already_met` can hold
    /// the chain up.
    pub(crate) fn ensure_tutorial_held(&mut self) {
        let Some(def) = self.current_tutorial() else {
            return;
        };
        if self
            .world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .any(|c| c.def.id == def.id)
        {
            return;
        }
        let accepted_tick = self.current_tick();
        let name = def.name.clone();
        self.world
            .resource_mut::<ActiveContracts>()
            .active
            .push(crate::resources::ActiveContract {
                def,
                progress: 0,
                accepted_tick,
            });
        // `Outcome` rather than `Info`, `complete_contract`'s reason: a
        // mission can be handed out mid-fight, and the battle prune keeps
        // only four kinds.
        self.log_kind(MessageKind::Outcome, format!("ONBOARDING: {name}"));
    }
```

- [ ] **Step 4: Add the refusal and the row flag**

In `Game::abandon_contract`, after the `position` lookup and before the `remove`:

```rust
        // An onboarding mission cannot be given back. This is the invariant,
        // so it does not depend on a caller remembering to ask; the sentence
        // the player reads is app-core's, through `App::refuse`, because a
        // bare `false` cannot reach the log.
        if held.active[idx].def.tutorial.is_some() {
            return false;
        }
```

In `crates/engine/src/views.rs`, in `ContractRow`:

```rust
    /// Whether this is an onboarding mission — see
    /// `Game::ensure_tutorial_held`. The renderer draws these green and
    /// app-core refuses to give one back.
    pub tutorial: bool,
```

In `Game::contract_row`, add `tutorial: def.tutorial.is_some(),`.

- [ ] **Step 5: Wire the three callers**

In `crates/engine/src/game/turn.rs`, immediately after the existing `self.settle_contracts();` at line ~206:

```rust
        // After settling, so finishing a mission hands out the next one in
        // the same tick and the player never sees an empty slot.
        self.ensure_tutorial_held();
```

In `crates/engine/src/game/lifecycle.rs`, in `Game::new`, immediately before `Ok(game)` (~line 313):

```rust
        // Before the first tick, so the very first contracts screen a run
        // opens already has the chain's first mission in hand.
        game.ensure_tutorial_held();
```

and in `Game::load`, immediately before `Ok(game)` (~line 1236), the same call with:

```rust
        // A save taken mid-chain resumes with no seeding path of its own —
        // the position is derived from the `done` list the save carries.
        game.ensure_tutorial_held();
```

- [ ] **Step 6: Add the test helper**

In `crates/engine/src/tests/support.rs`:

```rust
/// Files every onboarding mission as finished, so the ordinary contract
/// board is live.
///
/// Every test that wants a board needs this, because a run with an
/// unfinished chain has an empty one by design. Cheaper and more honest than
/// each test playing the chain: what those tests are about is the board.
pub(super) fn skip_tutorial(game: &mut Game) {
    let ids: Vec<crate::contracts::ContractId> = game
        .world
        .resource::<crate::contracts::ContractDb>()
        .tutorial_chain()
        .iter()
        .map(|d| d.id.clone())
        .collect();
    let mut held = game.world.resource_mut::<crate::resources::ActiveContracts>();
    held.active.retain(|c| c.def.tutorial.is_none());
    for id in ids {
        if !held.done.contains(&id) {
            held.done.push(id);
        }
    }
}
```

- [ ] **Step 7: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine chain
cargo test -p feral-processes-engine
cargo clippy --workspace
```

Expected: PASS. `ContractRow` gaining a field will break any test or renderer constructing one literally — fix by adding `tutorial: false`.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/game/contracts.rs crates/engine/src/game/turn.rs crates/engine/src/game/lifecycle.rs crates/engine/src/views.rs crates/engine/src/tests/support.rs crates/engine/src/tests/contracts.rs
git commit -m "feat(contracts): the chain is handed out, one step at a time"
```

---

### Task 6: The board is onboarding's while the chain runs

**Files:**
- Modify: `crates/engine/src/game/contracts.rs` — `Game::board_defs`
- Test: `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: Task 5's `Game::in_tutorial` and `assets_with_fixture_chain`.
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

```rust
/// While the chain runs the board is empty — one mission at a time means
/// one, not one plus three the player cannot evaluate yet.
#[test]
fn the_board_is_empty_while_the_chain_runs() {
    let dir = support::assets_with_fixture_chain("board_suppressed");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    crate::tests::support::stand_up_a_broker(&mut game);
    assert!(game.in_tutorial());
    assert_eq!(
        game.contract_board(),
        Some(Vec::new()),
        "a Broker is standing, so the board exists and is empty — not `None`, \
         which is the claim that no Broker is standing at all"
    );
}

/// And fills the moment the chain is finished.
#[test]
fn the_board_fills_when_the_chain_is_finished() {
    let dir = support::assets_with_fixture_chain("board_freed");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    crate::tests::support::stand_up_a_broker(&mut game);
    crate::tests::support::skip_tutorial(&mut game);
    let board = game.contract_board().expect("a Broker is standing");
    assert!(!board.is_empty(), "the ordinary board is live again");
}

/// With no Broker the answer is still `None` and not an empty board. Two
/// readers depend on that difference.
#[test]
fn no_broker_still_answers_none_during_the_chain() {
    let dir = support::assets_with_fixture_chain("board_no_broker");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    assert_eq!(game.contract_board(), None);
}
```

`stand_up_a_broker` may not exist. Look for how the existing board tests in this file get a Broker standing — `rg -n "contract_broker" crates/engine/src/tests/` — and use that. If they each do it inline, extract it into `support.rs` as part of this task and use it from all of them; three copies of a fixture is the point at which it earns a name.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine board
```

Expected: FAIL — `the_board_is_empty_while_the_chain_runs` gets a populated board.

- [ ] **Step 3: Add the early return**

In `Game::board_defs`, immediately after the `NoBroker` check:

```rust
        // Onboarding owns the board while it runs. A new player choosing
        // between three offers they have no way to evaluate is what the
        // chain exists to replace, and the starter queue below was the
        // weaker first attempt at the same thing.
        //
        // `Some(vec![])` rather than `None`: the Broker is standing and
        // reachable, and `None` is the claim that it is not — a claim two
        // other readers act on.
        if self.in_tutorial() {
            return Some(Vec::new());
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine board
cargo test -p feral-processes-engine
```

Expected: PASS. The suppression is inert against the shipped assets until Task 9, so nothing else should move yet.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/contracts.rs crates/engine/src/tests/contracts.rs crates/engine/src/tests/support.rs
git commit -m "feat(contracts): the board is onboarding's while the chain runs"
```

---

### Task 7: New runs only

**Files:**
- Modify: `crates/engine/src/save.rs` — `PlayerSave::tutorial_seeded`, and its write site
- Modify: `crates/engine/src/game/lifecycle.rs` — the back-fill in `Game::load`
- Test: `crates/engine/src/tests/save.rs` (or wherever save round-trips live — `rg -l "fn.*save.*load" crates/engine/src/tests/`)

**Interfaces:**
- Consumes: Task 5's `ensure_tutorial_held` and `ContractDb::tutorial_chain`.
- Produces: `PlayerSave::tutorial_seeded: bool` (`#[serde(default)]`).

**No `SAVE_FORMAT_VERSION` bump.** Additive behind a default is exactly what field-named RON retired migrations for.

- [ ] **Step 1: Write the failing tests**

```rust
/// A save written before this feature existed carries no flag, and a run
/// forty hours old must not be told to build a Home it built long ago. The
/// whole chain is filed as finished at load.
#[test]
fn a_save_from_before_the_chain_never_starts_it() {
    let dir = support::assets_with_fixture_chain("seed_old_save");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    let path = save_path("old_save");
    game.save(&path).unwrap();

    // Strip the flag the way a file written by the previous build would
    // have been: dump to RON, remove the key, pack it back.
    strip_field_from_save(&path, "tutorial_seeded");

    let loaded = Game::load(&path, &dir).unwrap();
    assert!(!loaded.in_tutorial(), "the chain is filed as finished");
    assert_eq!(
        loaded.active_contracts().iter().filter(|r| r.tutorial).count(),
        0,
        "and nothing is in hand"
    );
}

/// A save written *by* this build carries the flag and resumes the chain
/// exactly where it was — the position is derived from `done`, so there is
/// nothing else to restore.
#[test]
fn a_run_saved_mid_chain_resumes_on_the_same_step() {
    let dir = support::assets_with_fixture_chain("seed_mid_chain");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    let path = save_path("mid_chain");
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &dir).unwrap();
    let held: Vec<String> = loaded.active_contracts().iter().map(|r| r.id.to_string()).collect();
    assert!(held.contains(&"fixture_step_2".to_string()), "{held:?}");
    assert!(loaded.in_tutorial());
}
```

`save_path` is a per-file local helper in this repo, not a shared one —
`crates/engine/src/tests/work_orders.rs` has the canonical three-line version
(a `temp_dir()` join carrying the tag and the pid so two tests cannot tread on
each other's file). Copy that.

`strip_field_from_save` you write: read the save, drop the `tutorial_seeded`
key, write it back. The save is **field-named RON**, which is the whole reason
that is legal — a positional format would make this test impossible to write.
If the save is packed rather than plain text, do the same through
`savetool dump` / `savetool pack`, which is what `CLAUDE.md` documents them
for.

**Both tests matter, and the first is the one that could be vacuous.** A `#[serde(default)]` field leaves the RON round trip green whether or not the load path reads it, so a test that only round-trips proves nothing. Confirm the first test fails before the back-fill exists — if it passes, the test is not reaching the code.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine seed
```

Expected: `a_save_from_before_the_chain_never_starts_it` FAILS with the chain live.

- [ ] **Step 3: Add the field**

In `crates/engine/src/save.rs`, in `PlayerSave`:

```rust
    /// Whether this run was started under the onboarding chain.
    ///
    /// `#[serde(default)]` to false, which is what a save written before the
    /// chain existed reads as — and `Game::load` files the whole chain as
    /// finished for those, so a run forty hours old is never told to build a
    /// Home it built long ago. Additive behind a default, so **no
    /// `SAVE_FORMAT_VERSION` bump**.
    #[serde(default)]
    pub tutorial_seeded: bool,
```

Set it `true` wherever `PlayerSave` is built for writing (around `lifecycle.rs:1673`, beside `perk_points`).

- [ ] **Step 4: Add the back-fill**

In `Game::load`, **before** the `game.ensure_tutorial_held()` added in Task 5:

```rust
        // A save from before the chain existed: file every mission as
        // finished so an established run is left alone. New runs are seeded
        // by `Game::new`, which sets the flag, so this fires exactly once
        // and only for a save the previous build wrote.
        if !data.player.tutorial_seeded {
            let ids: Vec<crate::contracts::ContractId> = game
                .world
                .resource::<crate::contracts::ContractDb>()
                .tutorial_chain()
                .iter()
                .map(|d| d.id.clone())
                .collect();
            let mut held = game.world.resource_mut::<ActiveContracts>();
            for id in ids {
                if !held.done.contains(&id) {
                    held.done.push(id);
                }
            }
        }
```

Check the local name for the deserialized save in `load` — the surrounding code uses `data.` for it.

- [ ] **Step 5: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine seed
cargo test -p feral-processes-engine save
cargo clippy --workspace
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/save.rs crates/engine/src/game/lifecycle.rs crates/engine/src/tests/
git commit -m "feat(save): the chain runs on new games only"
```

---

### Task 8: The first decompile cannot fail

**Files:**
- Modify: `crates/engine/src/game/combat_rewards.rs` — `attempt_decompile`, new `Game::tutorial_grants_capture`
- Test: `crates/engine/src/tests/combat_rewards.rs`

**Interfaces:**
- Consumes: Task 5's `Game::current_tutorial`, Task 3's `Deed::Tamed`.
- Produces: nothing public.

Keyed off the mission's **objective**, not its id, so it stays content-driven: a mod that writes its own `Perform(deed: Tamed)` mission gets the same guarantee, and renaming the shipped file changes nothing.

- [ ] **Step 1: Write the failing tests**

```rust
/// The chain's decompile mission cannot be failed — a run of bad rolls would
/// end onboarding permanently, which is the one thing an unbreakable chain
/// must not do. The catalyst is still spent, so the lesson that decompiling
/// is priced in catalysts still lands.
#[test]
fn the_chains_decompile_mission_cannot_be_failed() {
    // Build a fight the player would essentially never win the roll on:
    // a full-HP target and no decompiler bonuses. Model it on the existing
    // decompile tests in this file.
    let mut game = decompile_fixture_with_hopeless_odds();
    make_tamed_the_live_mission(&mut game);
    let catalysts_before = catalyst_count(&game);

    let ended = game.attempt_decompile(0, game.player_entity());

    assert!(taming_succeeded(&game), "the roll is forced while the mission is live");
    assert_eq!(
        catalyst_count(&game),
        catalysts_before - 1,
        "and the catalyst is still spent — that is the half of the lesson that stays"
    );
    let _ = ended;
}

/// Off the mission the formula is untouched. Without this the forced roll is
/// indistinguishable from having broken `capture_chance`.
#[test]
fn a_decompile_outside_the_mission_still_rolls() {
    let mut game = decompile_fixture_with_hopeless_odds();
    crate::tests::support::skip_tutorial(&mut game);
    game.attempt_decompile(0, game.player_entity());
    assert!(!taming_succeeded(&game), "hopeless odds still fail when nothing is forcing them");
}

/// A successful decompile writes its deed either way, which is what finishes
/// the mission.
#[test]
fn a_successful_decompile_writes_its_deed() {
    let mut game = decompile_fixture_with_certain_odds();
    crate::tests::support::skip_tutorial(&mut game);
    game.attempt_decompile(0, game.player_entity());
    assert!(game
        .world
        .resource::<crate::resources::RunFeats>()
        .deeds
        .contains(&crate::contracts::Deed::Tamed));
}
```

The four helpers named here are fixtures you write against the existing decompile tests in this file — find them with `rg -n "attempt_decompile" crates/engine/src/tests/`. `decompile_fixture_with_hopeless_odds` needs a target at full HP with high resistance and no `Perk`/gear decompiler bonuses; `..._with_certain_odds` the reverse. If `capture_chance` cannot be driven to a certain 0 or 1 by fixture alone, seed `GameRng` instead and assert on a seed you have confirmed fails — and say so in a comment, because a seeded assertion that stops meaning what it meant is how a test goes vacuous.

`make_tamed_the_live_mission` files every chain mission before the `Tamed` one into `done` — or, more simply against a scratch install, ships a one-mission fixture chain whose objective is `Perform(deed: Tamed)`. Prefer the second; it does not depend on what Task 9 ships.

**The second test is the one that proves the first is not vacuous.** Delete the forced-roll line and it must fail; delete it and if only the first test fails, the second is not reaching the formula.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-engine decompile
```

Expected: `the_chains_decompile_mission_cannot_be_failed` FAILS — the roll is honest.

- [ ] **Step 3: Add the predicate and force the roll**

In `crates/engine/src/game/combat_rewards.rs`, in `impl Game`:

```rust
    /// Whether the run's live onboarding mission is the one that teaches
    /// decompiling.
    ///
    /// The one place the chain changes a shipped formula's outcome, and it is
    /// bounded to a single mission of a single run: read off the live mission
    /// rather than a flag, so it disarms itself the moment the chain moves
    /// on and there is no state to leave set.
    ///
    /// Keyed on the **objective**, not the id, so it stays content-driven —
    /// a mod authoring its own `Perform(deed: Tamed)` mission gets the same
    /// guarantee, and renaming the shipped file changes nothing.
    fn tutorial_grants_capture(&self) -> bool {
        self.current_tutorial().is_some_and(|def| {
            matches!(
                def.objective,
                crate::contracts::Objective::Perform {
                    deed: crate::contracts::Deed::Tamed
                }
            )
        })
    }
```

In `attempt_decompile`, after the existing `let roll = { ... };` block:

```rust
        // The chain's decompile mission cannot be failed: a run of bad rolls
        // would end onboarding permanently. The catalyst above is already
        // spent, so only the roll is forced — the lesson that decompiling is
        // priced in catalysts is the half that stays.
        //
        // Below the odds read, deliberately, so what the battle screen has
        // been showing stays honest about what the roll would have been.
        let roll = roll || self.tutorial_grants_capture();
```

- [ ] **Step 4: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-engine decompile
cargo test -p feral-processes-engine
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/combat_rewards.rs crates/engine/src/tests/combat_rewards.rs
git commit -m "feat(contracts): the chain's first decompile cannot fail"
```

---

### Task 9: The eleven missions, and the three censuses that keep the chain finishable

**Files:**
- Create: eleven files in `assets/contracts/`
- Modify: `crates/engine/src/tests/assets.rs` — three censuses
- Modify: existing tests across the engine, app-core and gui suites that assumed an open board
- Test: `crates/engine/src/tests/assets.rs`, `crates/engine/src/tests/contracts.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–8.
- Produces: the shipped chain.

**This is the task where the feature becomes visible, and where existing tests break.** From here on `Game::new` against the shipped assets holds a mission and has an empty board. Expect fallout in the engine, app-core and gui suites; `tests::support::skip_tutorial` from Task 5 is the fix for anything that wants a board, and it is the *only* acceptable fix — do not weaken the suppression to make a test pass.

- [ ] **Step 1: Write the eleven mission files**

Costs, for the funding census: Home 5 Core Fragments, Contract Broker 5, Mining Node 12, Research Node 10. The run starts with 5 Core Fragments (`game/lifecycle.rs:260`).

Create each as `assets/contracts/<id>.ron`:

```ron
(
    id: "tutorial_first_light",
    name: "First Light",
    description: "Nothing gets raised out here until there is somewhere to raise it from. Put your Home down — it is the one build your own hands finish, and everything else is a request you file against it.",
    objective: Build(structure: "home"),
    reward: [Credits(10), Xp(30)],
    tutorial: Some(10),
)
```

```ron
(
    id: "tutorial_take_a_look",
    name: "Take a Look",
    description: "Guessing at what is standing in front of you is how a run ends early. Press [x] and pick a direction to read whatever is out there.",
    objective: Perform(deed: Examined),
    reward: [Credits(10), Xp(30)],
    tutorial: Some(20),
)
```

```ron
(
    id: "tutorial_scrap_run",
    name: "Scrap Run",
    description: "Everything out here starts as something somebody chipped loose. Put wild programs down and mine what you walk past until you are carrying twelve Core Fragments at once.",
    objective: Hold(item: "core_fragment", count: 12),
    reward: [Credits(20), Item("core_fragment", 6), Xp(80)],
    tutorial: Some(30),
)
```

```ron
(
    id: "tutorial_first_decompile",
    name: "First Decompile",
    description: "A wild program is worth more running beside you than lying in pieces. Take one into a fight, wear it down, and run Decompile — it spends a taming catalyst each time you try.",
    objective: Perform(deed: Tamed),
    reward: [Credits(25), Item("core_fragment", 6), Xp(90)],
    tutorial: Some(40),
)
```

```ron
(
    id: "tutorial_sign_here",
    name: "Sign Here",
    description: "Work does not find you out here. Stand up a Contract Broker and it will start posting jobs worth taking.",
    objective: Build(structure: "contract_broker"),
    reward: [Credits(25), Item("core_fragment", 8), Xp(90)],
    tutorial: Some(50),
)
```

```ron
(
    id: "tutorial_break_ground",
    name: "Break Ground",
    description: "Mining by hand is a way to spend a run. Stand up a Mining Node and let it work while you do something else — post a program to it, then stay in the base a while and watch your crew haul what it cuts across to a buffer on their own.",
    objective: Build(structure: "mining_node"),
    reward: [Credits(30), Item("core_fragment", 10), Xp(100)],
    tutorial: Some(60),
)
```

```ron
(
    id: "tutorial_collect",
    name: "Collect",
    description: "What a machine cuts sits in its own buffer until somebody moves it. Walk up to one and press [c] to open the transfer screen, then take what it is holding.",
    objective: Perform(deed: TookFromContainer),
    reward: [Credits(20), Item("core_fragment", 8), Xp(70)],
    tutorial: Some(70),
)
```

```ron
(
    id: "tutorial_standing_order",
    name: "Standing Order",
    description: "Telling the base what to hold beats telling it what to do. Open your work orders and place a standing order for twenty Core Fragments — it will keep topping them up without being asked again.",
    objective: Perform(deed: QueuedStandingOrder),
    reward: [Credits(25), Item("core_fragment", 6), Xp(80)],
    tutorial: Some(80),
)
```

```ron
(
    id: "tutorial_first_reading",
    name: "First Reading",
    description: "Nothing gets built twice out here unless somebody wrote down how. Stand up a Research Node and start banking readings.",
    objective: Build(structure: "research_node"),
    reward: [Credits(30), Xp(100)],
    tutorial: Some(90),
)
```

```ron
(
    id: "tutorial_man_the_node",
    name: "Man the Node",
    description: "A Research Node with nobody on it is furniture. Post one of your programs to it and leave them there — an unstaffed machine banks nothing.",
    objective: Perform(deed: PostedStaff),
    reward: [Credits(25), Xp(90)],
    tutorial: Some(100),
)
```

```ron
(
    id: "tutorial_spend_it",
    name: "Spend It",
    description: "Levelling pays you in Perk Points and they do nothing sitting unspent. Open your perks and buy one — that is the last thing anyone is going to walk you through.",
    objective: Perform(deed: UnlockedPerk),
    reward: [Credits(40), Xp(120)],
    tutorial: Some(110),
)
```

- [ ] **Step 2: Write the three censuses**

Append to `crates/engine/src/tests/assets.rs`:

```rust
/// **The shipped chain cannot stall on economy.**
///
/// An unbreakable chain fails in a way an optional contract does not: a
/// mission the player cannot afford to finish ends onboarding for the rest
/// of the run, with no key to press. This walks the chain in step order
/// carrying a Core Fragment balance — starting from what a run starts with,
/// crediting each mission's payout and debiting each `Build`'s cost — and
/// asserts it never goes negative.
///
/// It is deliberately blind to what the player spends on anything the chain
/// does not know about, which is why the shipped payouts carry headroom
/// rather than exactly clearing.
#[test]
fn the_tutorial_chain_can_always_afford_its_next_step() {
    let assets = test_assets_dir();
    let (contracts, _) = ContractDb::load_dir(&assets.join("contracts")).unwrap();
    let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
    let fragment = ItemId::from(crate::items::ids::CORE_FRAGMENT);

    // What `Game::new` puts in the pack — see `game/lifecycle.rs`.
    let mut balance: i64 = 5;
    for def in contracts.tutorial_chain() {
        if let Objective::Build { structure } = &def.objective {
            let cost = structures
                .get(structure)
                .unwrap_or_else(|| panic!("{} names a structure that does not exist", def.id))
                .build_cost
                .iter()
                .find(|(item, _)| *item == fragment)
                .map(|(_, n)| *n as i64)
                .unwrap_or(0);
            balance -= cost;
        }
        if let Objective::Hold { item, count } = &def.objective {
            assert!(
                *item == fragment,
                "{} holds something the balance does not track: {item}",
                def.id
            );
            assert!(
                balance >= 0,
                "{} asks the player to hold {count} with a balance of {balance}",
                def.id
            );
        }
        assert!(
            balance >= 0,
            "the chain runs dry at {}: {balance} Core Fragments",
            def.id
        );
        for reward in &def.reward {
            if let Reward::Item(item, n) = reward {
                if *item == fragment {
                    balance += *n as i64;
                }
            }
        }
    }
}

/// **Every `Deed` has an emit site.** Exhaustive over the enum, `cell_mark`'s
/// rule: a variant with no writer ships a mission that can never complete,
/// and this fails the build instead.
///
/// A source grep rather than a runtime check, because there is no runtime at
/// which "has anything ever written this" is answerable.
#[test]
fn every_deed_has_an_emit_site() {
    let src = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let mut body = String::new();
    for entry in walk_rust_files(src) {
        if entry.to_string_lossy().contains("/tests/") {
            continue;
        }
        body.push_str(&std::fs::read_to_string(&entry).unwrap());
    }
    for deed in [
        Deed::Examined,
        Deed::Tamed,
        Deed::TookFromContainer,
        Deed::QueuedStandingOrder,
        Deed::UnlockedPerk,
        Deed::PostedStaff,
    ] {
        let call = format!("note_deed(crate::contracts::Deed::{deed:?})");
        let short = format!("note_deed(Deed::{deed:?})");
        assert!(
            body.contains(&call) || body.contains(&short),
            "{deed:?} has no `Game::note_deed` caller outside the tests. A deed \
             with no emit site is a mission that can never complete."
        );
    }
}

/// The shipped chain's own well-formedness: contiguous-ish steps, ids that
/// resolve, and nothing carrying a flag its own load refuses.
#[test]
fn the_shipped_tutorial_chain_is_well_formed() {
    let assets = test_assets_dir();
    let (contracts, warnings) = ContractDb::load_dir(&assets.join("contracts")).unwrap();
    assert!(warnings.is_empty(), "the shipped set parses: {warnings:?}");
    let chain = contracts.tutorial_chain();
    assert!(
        chain.len() >= 11,
        "the shipped chain is authored, not empty: {}",
        chain.len()
    );

    let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
    let (abilities, _) = AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();

    let mut steps: Vec<u32> = Vec::new();
    for def in &chain {
        let step = def.tutorial.expect("in the chain");
        assert!(!steps.contains(&step), "{} repeats step {step}", def.id);
        steps.push(step);
        assert!(!def.starter, "{} is a mission, not a starter", def.id);
        assert!(!def.repeatable, "{} must not be repeatable", def.id);
        assert!(
            !def.description.is_empty(),
            "{} needs a description — it is the only place the player is told what to do",
            def.id
        );
        match &def.objective {
            Objective::Build { structure } => assert!(
                structures.get(structure).is_some(),
                "{} names a structure that does not exist: {structure}",
                def.id
            ),
            Objective::Hold { item, count } => {
                assert!(
                    items.get(item.as_str()).is_some(),
                    "{} asks for an item that does not exist: {item}",
                    def.id
                );
                assert!(*count > 0, "{} holds nothing", def.id);
            }
            _ => {}
        }
    }
}
```

`walk_rust_files` may not exist — check `tests/assets.rs` for an existing source-walking census (several of the repo's rules are enforced this way). If there is none, write a small recursive `fn walk_rust_files(dir: &Path) -> Vec<PathBuf>` local to the test module. Match the imports at the top of `assets.rs` for `ContractDb`, `StructureDb`, `ItemDb`, `AbilityDb`, `Objective`, `Reward`, `Deed`, `ItemId`.

- [ ] **Step 3: Run the censuses to verify they fail, then pass**

```sh
cargo test -p feral-processes-engine tutorial_chain
```

The funding census is the one to watch: if it fails, **raise a mission's `Reward::Item` payout**, do not lower a structure's `build_cost` — the costs are balance and the payouts are onboarding.

- [ ] **Step 4: Run the whole workspace and repair the fallout**

```sh
cargo test --workspace
```

Every failure here is a test that assumed an open board or an empty `active_contracts`. Expect them in at least:

- `crates/engine/src/tests/contracts.rs` — board tests
- `crates/app-core/src/tests/contracts.rs` — the screen's row indexing
- `crates/gui/src/render/contracts.rs` — the width census now measures more rows, which is what it is for
- `crates/gui/src/render/manifest.rs` — the `Contracts` stat counts one more

For each: if the test is about the **board**, add `support::skip_tutorial(&mut game)` after the fixture is built. If it is about **counting held contracts**, update the expected number and leave a comment saying the chain's mission is one of them. If it is about the **chain**, it is already right.

Also convert `an_install_with_no_chain_hands_out_nothing` from Task 5 into a scratch install whose `contracts/` directory has the eleven `tutorial_*.ron` files removed — it is the test that keeps "delete the chain and you get the old game back" true, and against the shipped assets it can no longer say that.

**Do not weaken `board_defs`' suppression to make a test pass.** If a test cannot be fixed with `skip_tutorial`, that is a finding worth raising, not a reason to change the engine.

- [ ] **Step 5: Run the full gate**

```sh
cargo test --workspace
cargo clippy --workspace
cargo fmt
```

Expected: PASS, clean.

- [ ] **Step 6: Commit**

```bash
git add assets/contracts/tutorial_*.ron crates/engine/src/tests/ crates/app-core/src/tests/ crates/gui/src/render/
git commit -m "feat(contracts): eleven onboarding missions, and three censuses that keep them finishable"
```

---

### Task 10: Green on the screen, and a refusal with a sentence

**Files:**
- Modify: `crates/gui/src/render/contracts.rs` — `contract_line`, `offered_header`, `draw_contracts`
- Modify: `crates/gui/src/render/mod.rs` — the `draw_contracts` call site, if the header needs a new argument
- Modify: `crates/app-core/src/app/contracts.rs` — the abandon refusal
- Test: `crates/gui/src/render/contracts.rs`, `crates/app-core/src/tests/contracts.rs`

**Interfaces:**
- Consumes: Task 5's `ContractRow::tutorial` and `Game::in_tutorial`.
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

In `crates/gui/src/render/contracts.rs`'s test module:

```rust
/// An onboarding mission draws green. It is the only row on this screen that
/// is coloured at all, so the axis carries exactly one meaning here.
#[test]
fn an_onboarding_missions_row_is_green() {
    let row = ContractRow {
        id: "tutorial_first_light".into(),
        name: "First Light".to_string(),
        description: "d".to_string(),
        objective_line: "Build a Home".to_string(),
        reward_line: "10 Credits".to_string(),
        progress: 0,
        target: 1,
        tutorial: true,
    };
    match contract_line(&row, 0, usize::MAX, true) {
        Row::Item { color, .. } => assert_eq!(color, GREEN),
        _ => panic!("contract_line builds an item row"),
    }
}

/// An ordinary contract is untouched.
#[test]
fn an_ordinary_contracts_row_is_not_green() {
    let row = ContractRow {
        id: "raw_stock".into(),
        name: "Raw Stock".to_string(),
        description: "d".to_string(),
        objective_line: "Deliver 6 Core Fragments".to_string(),
        reward_line: "20 Credits".to_string(),
        progress: 0,
        target: 6,
        tutorial: false,
    };
    match contract_line(&row, 0, usize::MAX, true) {
        Row::Item { color, .. } => assert_ne!(color, GREEN),
        _ => panic!("contract_line builds an item row"),
    }
}

/// A board that is empty *because onboarding owns it* has to say so. Under a
/// Broker the player just built, "Nothing on the board." reads as broken.
#[test]
fn the_board_header_says_when_onboarding_owns_it() {
    let line = offered_header(BrokerReach::AtBroker, true);
    assert!(line.starts_with("Offered"), "still recognisable as the board: {line:?}");
    assert!(
        line.len() > "Offered".len(),
        "and it names why there is nothing on it: {line:?}"
    );
    assert_eq!(
        offered_header(BrokerReach::AtBroker, false),
        "Offered",
        "an ordinary board is unchanged"
    );
}
```

In `crates/app-core/src/tests/contracts.rs`:

```rust
/// `[A]` on an onboarding mission refuses with a sentence, on both surfaces.
/// A silent no-op reads as the key being broken.
#[test]
fn giving_back_an_onboarding_mission_is_refused_with_a_sentence() {
    let mut app = /* the fixture the neighbouring abandon test uses */;
    app.open_contracts();
    // Highlight the onboarding mission — it is the first row under Held.
    app.handle_key(GameKey::Char('A'));
    let line = app.status_line.clone().expect("a refusal has a sentence");
    assert!(
        line.to_lowercase().contains("onboarding") || line.to_lowercase().contains("finish"),
        "the sentence says why: {line:?}"
    );
    assert_eq!(
        app.game.as_ref().unwrap().active_contracts().iter().filter(|r| r.tutorial).count(),
        1,
        "and it is still in hand"
    );
}
```

Build the fixture from the neighbouring abandon test in that file, and confirm the log side too — `App::refuse` writes to both, and asserting only `status_line` would pass against a bare `self.status_line = ...`.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p feral-processes-gui contracts
cargo test -p feral-processes-app-core contracts
```

Expected: FAIL to compile — `offered_header` takes one argument.

- [ ] **Step 3: Colour the row**

In `contract_line`, replace the bare `item_row(...)` with a coloured one. `Row::Item` already has a `color` field and `GREEN` is already defined in `render/mod.rs`:

```rust
    let row = item_row(
        format!(
            "[{}] {} - {}{progress} - pays {}",
            menu_shortcut(idx),
            contract.name,
            contract.objective_line,
            contract.reward_line
        ),
        idx == selected,
    );
    // Onboarding missions, and nothing else on this screen. `color` means
    // fusion tier on the gear screens and CRITICAL HP on the party screen;
    // the contracts screen has never used it, so green lands on a free axis
    // rather than becoming a second meaning on a loaded one.
    if contract.tutorial {
        recolour(row, GREEN)
    } else {
        row
    }
```

`recolour` may not exist — check `popup.rs` for an existing row-modifying helper (`with_icon` and `with_tag` are the shape). If there is none, add one there beside them with the same doc-comment discipline, or construct the `Row::Item` directly with `color: if contract.tutorial { GREEN } else { TEXT }`. The second is simpler and probably right; prefer it unless it duplicates `item_row`'s whole body.

- [ ] **Step 4: Say why the board is empty**

Change `offered_header` to take a second argument and add the arm:

```rust
/// The board's own header. Off the base the offers are still listed — they
/// are the sector's, not the tile's — so this is where the screen says they
/// cannot be signed from here, rather than leaving the player to press a key
/// and read a refusal.
///
/// `onboarding` is the same errand one step earlier: the board really is
/// empty, and under a Broker the player has just built, "Nothing on the
/// board." reads as the Broker being broken rather than as the game waiting
/// on them.
fn offered_header(reach: BrokerReach, onboarding: bool) -> String {
    if onboarding {
        return "Offered - finish your onboarding and the board opens up".to_string();
    }
    match reach {
        BrokerReach::AtBroker | BrokerReach::NoBroker => "Offered".to_string(),
        BrokerReach::OffBase => "Offered - return to your base to take one".to_string(),
    }
}
```

Thread the flag through `draw_contracts` and its call site in `render/mod.rs`. The cleanest source is `active.iter().any(|r| r.tutorial)` — derived from the rows the screen is already holding, so the header and the list cannot disagree. Do **not** ask the engine a second time from the renderer; the module doc on `draw_contracts` says why the two lists come in from app-core.

Add the new header line to the footer-overflow census in that file so it is measured against the real font like every other line on this screen.

- [ ] **Step 5: Add the refusal**

In `crates/app-core/src/app/contracts.rs`, inside the `GameKey::Char('A')` branch, before the `game.abandon_contract` call:

```rust
                if active[row].tutorial {
                    self.refuse(
                        "Onboarding missions cannot be given back — finish this one and \
                         the next arrives.",
                    );
                    return;
                }
```

`App::refuse` (`app/input.rs:355`) is the one door for a refusal's sentence and puts it on both the popup and the log.

- [ ] **Step 6: Run the tests to verify they pass**

```sh
cargo test -p feral-processes-gui contracts
cargo test -p feral-processes-app-core contracts
cargo test --workspace
cargo clippy --workspace
cargo fmt
```

Expected: PASS, clean. The gui width census now measures the eleven tutorial rows too, since `contract_catalogue` returns the whole shipped set — if one of them overflows, **shorten the mission's `name` or `reward` list**, not the census.

- [ ] **Step 7: Commit**

```bash
git add crates/gui/src/render/contracts.rs crates/gui/src/render/mod.rs crates/app-core/src/app/contracts.rs crates/app-core/src/tests/contracts.rs
git commit -m "feat(contracts): onboarding draws green and cannot be handed back"
```

---

## After the plan

Run the whole gate once more, then stop:

```sh
cargo test --workspace
cargo clippy --workspace
cargo fmt --check
```

**Do not bump the version, write a `CHANGELOG.md` section, or push.** Those happen at the merge, and pushing needs the user's explicit ask.

**Say plainly that the chain has never been played.** A green suite is not evidence of play, and this feature is entirely about how the first hour feels. The three things worth checking in a real run, in order:

1. **Step 30's length.** "Hold twelve Core Fragments" is the only mission with an unbounded time cost, and nothing measures it.
2. **Step 110's reachability.** Perks are last so a level has certainly happened, but that is reasoned from `PERK_POINTS_PER_LEVEL`, not observed.
3. **Whether the empty board reads as intentional.** The header line is the only thing saying so.

`cargo run` starts a new game, which is the only way to see the chain from the top.

## Seams to update at the merge

Per `CLAUDE.md`, a new seam is three writes, in this order: the argument to `docs/seams.md`, the trap to `.claude/skills/seams/`, the one-sentence rule to `CLAUDE.md`. Candidates from this work — decide with the user, do not add them unilaterally:

- **`Game::ensure_tutorial_held` is the one writer of an onboarding mission, and the cap, the Broker and the offer filter are omissions rather than checks.**
- **A deed is a closed engine enum and `Game::note_deed` is its one door.**
- **The chain's position is derived from `contracts_done`; there is no cursor.**
