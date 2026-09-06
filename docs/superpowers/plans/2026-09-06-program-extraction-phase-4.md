# Program extraction phase 4: the Teardown Rig — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A structure the player loads with downed programs, which strips them
into plain items while the party is away.

**Architecture:** A new `Teardown Rig` structure carries a private `Hopper`
holding `(DownedProgram, ToolId)` entries. A per-tick `&mut Game` pass
(`Game::run_teardown_rigs`) advances the head entry and pays its yield into the
rig's ordinary `Stock::output`, where haulers, depots and `collect` pick it up
with no change whatever. The instance never leaves the hopper; what leaves is
plain items.

**Tech Stack:** Rust, `bevy_ecs` (world + a hand-run schedule), RON assets,
`serde`. Workspace crates: `feral-processes-engine`, `feral-processes-app-core`,
`feral-processes-gui`.

**Spec:** `docs/superpowers/specs/2026-09-04-program-extraction-design.md` — §10
is this phase and is the authority. §3 (one derivation), §7 (per-refusal
testing) and decision 2 (the plain-copy seam) are load-bearing on it. Read §10
before Task 1.

## Global Constraints

- **A `DownedProgram` exists in exactly two places: the player's pack and one
  machine's private hopper.** Spec 10.1. Nothing in this plan may put one into
  `Inventory`, `Stock`, `Carrying`, a depot or a work order.
- **Work orders do not change.** Spec 10.2. No file under
  `crates/engine/src/game/base/work_orders.rs` is touched by this plan.
- **One derivation.** The rig computes its payout by calling
  `Game::extraction_yield` and its pace by calling `Game::extraction_ticks` —
  the same two functions the player's own extraction calls. Never re-derive
  either. Spec §3, and it is why the rig is a `&mut Game` pass rather than a
  bevy system (spec 10.5).
- **Only pool-yielding tools may be queued.** `ToolCategory::Routines` and
  `ToolCategory::Gear` refuse at the deposit. Spec 10.4.
- **Nothing is destroyed.** An over-ask clamps, and a rig that cannot hold a
  whole payout holds the program. Spec decision 9.
- **Uppercase for new screen actions.** `App::selected_index`
  (`crates/app-core/src/app/input.rs:108`) returns `None` for anything that is
  not lowercase or a digit, so uppercase is free by construction. A lowercase
  binding would make one keypress both pick a row and fire an action.
- **No `SAVE_FORMAT_VERSION` bump.** Every new save field is additive behind
  `#[serde(default)]`, phases 1-3's practice. A RON round-trip cannot see a
  skipped field, so the hopper gets a real save-then-load test.
- **Determinism.** Any walk over structures sorts by tile before acting —
  bevy's query iteration order is not stable, and two rigs finishing in a
  different order between runs would reorder log lines and RNG draws.
- **Commit after every task.** Run `cargo test --workspace` before each commit;
  a single-crate run shifts the RNG stream and is a different build.

---

### Task 1: The rig exists and can be built

Deliverable: `teardown_rig` is a shipped, research-gated structure that stands,
carries a `MachineStatus`, and is reached by `idle_machine_system`. It does
nothing yet.

**Files:**
- Modify: `crates/engine/src/structures.rs` — add `StripDef`, the
  `StructureDef::strips` field, extend `runs_a_job`, extend `category`
- Create: `assets/structures/teardown_rig.ron`
- Create: `assets/research/teardown.ron`
- Modify: `assets/structures/README.md` — document `strips`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct StripDef { pub hopper: u32 }` in `crates::structures`
  - `StructureDef::strips: Option<StripDef>`
  - `StructureDef::runs_a_job()` now true when `strips` is set
  - structure id `"teardown_rig"`, research id `"teardown"`

- [ ] **Step 1: Write the failing censuses**

In `crates/engine/src/tests/assets.rs`, following the style of
`every_shipped_tools_yields_resolve_to_real_items` (same file, ~line 3289):

```rust
/// **A rig with no hopper is a machine that can never be loaded.** `strips`
/// is the whole feature's gate, and a zero there ships a structure that
/// builds, staffs, draws power and refuses every deposit.
#[test]
fn every_structure_that_strips_declares_a_hopper_and_an_output() {
    let game = Game::new(4110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();

    let mut checked = 0;
    for def in structures.all() {
        let Some(strips) = &def.strips else { continue };
        assert!(
            strips.hopper > 0,
            "structure {:?} strips programs but holds none",
            def.id
        );
        assert!(
            def.capacity > 0,
            "structure {:?} strips programs but has nowhere to put the yield",
            def.id
        );
        checked += 1;
    }
    assert!(checked > 0, "no shipped structure strips programs at all");
}

/// The rig is a bench too, so manual extraction gains a second one. The
/// Compiler keeps its own flag — moving it would silently downgrade
/// manual extraction for a run in progress (spec 10.3).
#[test]
fn the_teardown_rig_is_also_an_extraction_bench_and_the_compiler_still_is() {
    let game = Game::new(4111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();

    let rig = structures
        .get(&StructureId::from("teardown_rig"))
        .expect("teardown_rig should be a shipped structure");
    assert!(rig.strips.is_some(), "the rig should strip programs");
    assert!(rig.extracts_programs, "the rig should be a manual bench too");

    let compiler = structures
        .get(&StructureId::from("compiler"))
        .expect("compiler should be a shipped structure");
    assert!(
        compiler.extracts_programs,
        "the Compiler keeps its bench flag — moving it downgrades a run in progress"
    );
}

/// **`runs_a_job` is doubly load-bearing.** `spawn_structure` only inserts
/// a `MachineStatus` for a structure that runs a job
/// (`game/base/building.rs:277`), and `idle_machine_system` skips one that
/// does not (`systems.rs:878`). A rig missing from both ships as a machine
/// that can never say it is unstaffed — green, and unreachable.
#[test]
fn a_structure_that_strips_runs_a_job() {
    let game = Game::new(4112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structures = game.world.resource::<StructureDb>();
    let rig = structures
        .get(&StructureId::from("teardown_rig"))
        .expect("teardown_rig should be a shipped structure");
    assert!(
        rig.runs_a_job(),
        "a rig that runs no job carries no MachineStatus and is skipped by idle_machine_system"
    );
}

/// The research node that hands the rig over resolves to it.
#[test]
fn the_teardown_node_unlocks_the_rig() {
    let game = Game::new(4113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<ResearchDb>();
    let node = research
        .get(&"teardown".to_string())
        .expect("teardown should be a shipped research node");
    assert!(
        node.unlocks_structures.contains(&StructureId::from("teardown_rig")),
        "the teardown node should unlock the rig"
    );
}
```

Adjust the accessor names to whatever `StructureDb` / `ResearchDb` actually
expose in this repo — read one neighbouring census in the same file first and
copy its lookup shape exactly rather than guessing.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p feral-processes-engine --lib tests::assets 2>&1 | tail -40
```

Expected: FAIL — `no field `strips` on type `StructureDef`` and
`teardown_rig should be a shipped structure`.

- [ ] **Step 3: Add the schema**

In `crates/engine/src/structures.rs`, beside `AssembleDef`:

```rust
/// A structure's automated-teardown capability — see
/// `StructureDef::strips` and `Game::run_teardown_rigs`.
///
/// Mirrors `AssembleDef` rung for rung, decision 6's rule that a second
/// shape for the same idea is a second thing to get wrong. It carries no
/// tick figure of its own: what a program costs to strip is
/// `Game::extraction_ticks`, the same derivation the player's own
/// extraction is priced by, so a rig and a hand cannot disagree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StripDef {
    /// How many downed programs the hopper holds before it refuses.
    ///
    /// Authored per machine rather than in `tuning.rs`, for
    /// `StructureDef::capacity`'s reason: it is how big *this* box is, and
    /// a modder's second rig should be able to differ.
    pub hopper: u32,
}
```

On `StructureDef`, beside `assembles`:

```rust
    /// If set, this structure takes downed programs the player hands it and
    /// strips them into its output buffer over ticks, once a program is
    /// assigned to it — see `Game::run_teardown_rigs` and spec §10.
    ///
    /// The instanced object never leaves this machine: what lands in
    /// `Stock::output` is plain items, which is what lets hauling, depots
    /// and `collect` carry the yield with no instance rule (decision 2).
    /// `#[serde(default)]` so every existing structure file, including any
    /// mod, keeps parsing as a machine that strips nothing.
    #[serde(default)]
    pub strips: Option<StripDef>,
```

Extend `runs_a_job` (`crates/engine/src/structures.rs:530`):

```rust
    pub fn runs_a_job(&self) -> bool {
        self.work.is_some() || self.assembles.is_some() || self.strips.is_some()
    }
```

Extend `category`, immediately after the `assembles` arm:

```rust
        // A rig consumes something and pays items out, given a program —
        // which is what `Assembler` means here. Deliberately not a seventh
        // `StructureCategory`: the variant is a build-menu grouping, and a
        // new one is a group every menu has to learn to draw.
        if self.strips.is_some() {
            return StructureCategory::Assembler;
        }
```

- [ ] **Step 4: Author the two assets**

`assets/structures/teardown_rig.ron` — the glyph `V` is free (censused
against every shipped structure's `glyph` on 2026-09-06; `$` is the only
one used twice, by the Mining Node and the Black Market):

```ron
(
    id: "teardown_rig",
    name: "Teardown Rig",
    description: "Hand it the programs you dropped and it strips them while you are out. Post a program here, keep it powered, and collect what it leaves in the buffer. It is an extraction bench too, so upgrading it pays your own teardowns as well.",
    glyph: 'V',
    color: Orange,
    // Guessed numbers, like every structure's and every tool's forge_cost.
    // Nothing in the design rests on any of them; they are the first thing
    // a play session should push on.
    build_cost: [("core_fragment", 20), ("bytecode_block", 6)],
    capacity: 20,
    // Deliberately smaller than tuning::MAX_DOWNED_PROGRAMS (10): a full
    // pack does not empty into one rig, so the clamp is a thing players
    // meet rather than a branch only tests reach.
    strips: Some((hopper: 6)),
    upgrade: Some((max_tier: 5, cost: [("core_fragment", 12), ("cache_grain", 1)])),
    extracts_programs: true,
    power_draw: 3,
)
```

`assets/research/teardown.ron`:

```ron
(
    id: "teardown",
    name: "Teardown",
    description: "A rig that strips downed programs without you standing over it. Unlocks the Teardown Rig.",
    cost: 12,
    requires: ["automation"],
    unlocks_structures: ["teardown_rig"],
)
```

Check both against the field lists in `assets/structures/README.md` and
`assets/research/README.md` and drop any field this repo's schema does not
have. If `bytecode_block` or `cache_grain` do not resolve as item ids, swap
in ones that do — `every_structure_build_cost_resolves`-style censuses in
`tests/assets.rs` will say so.

- [ ] **Step 5: Document the field**

Add a `strips` row to `assets/structures/README.md`'s field table, in the
same voice as the `assembles` row: what it does, that `hopper` is a program
count, and that the yield lands in the ordinary output buffer.

- [ ] **Step 6: Run the whole suite**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: PASS. Watch specifically for the research-to-zone censuses around
`tests/assets.rs:1713` and the onboarding-economy census at ~2660 — a new
research node changes the earliest zone some items are reachable in. If one
fails, the fix is the node's `requires`/`cost`, not the census.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/structures.rs crates/engine/src/tests/assets.rs assets/structures/teardown_rig.ron assets/structures/README.md assets/research/teardown.ron
git commit -m "feat(extraction): a Teardown Rig you can build"
```

---

### Task 2: The hopper, on the entity and in the save

Deliverable: a built rig carries an empty `Hopper`, and a loaded one survives
save and load.

**Files:**
- Modify: `crates/engine/src/components.rs` — add `HopperEntry` and `Hopper`
- Modify: `crates/engine/src/game/base/building.rs:246-300` (`spawn_structure`)
- Modify: `crates/engine/src/save.rs` — `StructureSave` fields, save and restore
- Modify: `crates/engine/src/tests/support.rs:1062` (`spawn_machine_at`)
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `StructureDef::strips` (Task 1).
- Produces:
  - `pub struct HopperEntry { pub program: DownedProgram, pub tool: ToolId }`
  - `pub struct Hopper { pub queue: Vec<HopperEntry>, pub progress: u64 }`
  - `StructureSave::hopper: Vec<HopperEntry>`, `StructureSave::hopper_progress: u64`

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/extraction.rs`:

```rust
/// A rig spawned without a `Hopper` refuses every deposit and strips
/// nothing, silently — `spawn_machine_at`'s own doc warns about exactly
/// this class of short fixture.
#[test]
fn a_built_rig_carries_an_empty_hopper() {
    let mut game = Game::new(4120, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let rig = spawn_machine_at(&mut game, "teardown_rig", 3, 3);
    let hopper = game
        .world
        .get::<Hopper>(rig)
        .expect("a rig that strips should carry a hopper");
    assert!(hopper.queue.is_empty());
    assert_eq!(hopper.progress, 0);
}

/// A RON round trip cannot see a `#[serde(skip)]`, so the hopper's
/// persistence is asserted through a real save and load.
#[test]
fn a_loaded_hopper_survives_save_and_load() {
    let mut game = Game::new(4121, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let rig = spawn_machine_at(&mut game, "teardown_rig", 3, 3);
    let entry = HopperEntry {
        program: DownedProgram {
            species: "scrapper".to_string(),
            level: 4,
            rarity: Rarity::Ordinary,
            boss: false,
            condition: 70,
            carried: None,
        },
        tool: ToolId("salvage_clamp".to_string()),
    };
    {
        let mut hopper = game.world.get_mut::<Hopper>(rig).unwrap();
        hopper.queue.push(entry.clone());
        hopper.progress = 5;
    }

    let reloaded = save_and_load(&mut game);
    let hopper = reloaded
        .world
        .iter_entities()
        .find_map(|e| e.get::<Hopper>())
        .expect("the rig should still stand, carrying its hopper");
    assert_eq!(hopper.queue, vec![entry]);
    assert_eq!(hopper.progress, 5);
}
```

Replace `save_and_load` with this repo's actual save-then-load helper —
find it by reading an existing save-then-load test — search
`crates/engine/src/tests/` for `SAVE_FORMAT_VERSION` or `fn *save*load*` and
copy the shape verbatim. `DownedProgram`'s exact field list is at
`crates/engine/src/items.rs:358`; use those fields, not this sketch's, if
they differ.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p feral-processes-engine --lib tests::extraction 2>&1 | tail -30
```

Expected: FAIL — `cannot find type `Hopper``.

- [ ] **Step 3: Add the components**

In `crates/engine/src/components.rs`, beside `DownedPrograms`:

```rust
/// One program waiting in a rig, and the tool the player chose for it.
///
/// A **named struct, never a tuple.** RON parses a `(` in a struct position
/// as the start of named fields, so a `Vec<(DownedProgram, ToolId)>` could
/// never be widened and could not be converted to a named struct with
/// defaulted trailing fields either — `WorkOrder`'s own doc records the two
/// shipped fields that had to be drained into named successors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HopperEntry {
    pub program: DownedProgram,
    pub tool: ToolId,
}

/// A Teardown Rig's private store of downed programs — see
/// `StructureDef::strips`, `Game::load_teardown_rig` and
/// `Game::run_teardown_rigs`.
///
/// **The whole instance boundary of phase 4.** A `DownedProgram` exists in
/// exactly two places in the game: `DownedPrograms` on the player, and this.
/// What leaves a rig is plain items in `Stock::output`, which is what lets
/// hauling, depots, `collect::plan_adjacent_take` and work orders carry the
/// yield with no instance rule at all — decision 2's seam, which
/// `components.rs`' `GearCopies` doc states and which spec §10.1 declines to
/// spend.
///
/// A `Vec` rather than a keyed store, `DownedPrograms`' own reason: two
/// equal-comparing programs are still two separate kills, and the queue is
/// ordered work rather than a bag of interchangeable rows.
///
/// `progress` is ticks spent on the head entry alone. It resets when an
/// entry completes, and it is saved: a rig eight ticks into a twenty-tick
/// program must not restart that program because the player quit.
#[derive(Component, Default, Clone, Debug)]
pub struct Hopper {
    pub queue: Vec<HopperEntry>,
    pub progress: u64,
}
```

- [ ] **Step 4: Insert it at the deploy**

In `crates/engine/src/game/base/building.rs`, inside `spawn_structure`,
beside the `if let Some(work) = &def.work` arm:

```rust
        if def.strips.is_some() {
            entity.insert(crate::components::Hopper::default());
        }
```

And in `crates/engine/src/tests/support.rs`'s `spawn_machine_at`, which
hand-builds an entity rather than calling `spawn_structure`, add the same
arm beside its `ResourceNode` one. Its own doc comment warns that a fixture
short a component is silently skipped and reads as a curve that moved; this
is that case.

- [ ] **Step 5: Save and restore it**

In `crates/engine/src/save.rs`, on `StructureSave`, after `stock_output`:

```rust
    /// This rig's queue of downed programs and its progress on the head one
    /// — see `components::Hopper`. Live player state: losing it would eat
    /// every kill the player handed over and had not yet been paid for.
    ///
    /// Additive behind a default, so **no `SAVE_FORMAT_VERSION` bump** — a
    /// save written before the rig existed loads with an empty hopper,
    /// which is what it had. `HopperEntry` is stored directly rather than
    /// through a parallel `*Save` type, `PlayerSave::downed_programs`'
    /// reason: it has no legacy shape to reconcile.
    #[serde(default)]
    pub hopper: Vec<crate::components::HopperEntry>,
    #[serde(default)]
    pub hopper_progress: u64,
```

Write them where `StructureSave` is built and read them where it is
restored — find both by searching `save.rs` for `stock_output`, and mirror
that field's handling exactly on both sides. Restore into the `Hopper` the
deploy already inserted rather than inserting a second one.

- [ ] **Step 6: Run to verify the tests pass**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/components.rs crates/engine/src/game/base/building.rs crates/engine/src/save.rs crates/engine/src/tests/support.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): a rig's hopper, on the entity and in the save"
```

---

### Task 3: The deposit door

Deliverable: `Game::load_teardown_rig` moves programs from the pack into an
adjacent rig, refusing correctly and destroying nothing.

**Files:**
- Modify: `crates/engine/src/game/extraction.rs` — the door and its helper
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `Hopper`, `HopperEntry` (Task 2); `StructureDef::strips` (Task 1);
  existing `Game::installed_tools`, `Game::base_pos`,
  `game::base::collect::ORTHOGONAL`, `Game::downed_program_label`.
- Produces:
  - `pub fn adjacent_teardown_rig(&self) -> Option<Entity>`
  - `pub fn load_teardown_rig(&mut self, indices: &[usize], tool: &ToolId) -> Result<(), String>`

- [ ] **Step 1: Write the failing tests, one per refusal**

Spec §7's rule: asserted **per refusal**, since one test over one path passes
against the others. In `crates/engine/src/tests/extraction.rs`:

```rust
/// A fixture: a rig at (3,3), the player beside it, and `n` programs in
/// the pack.
fn player_beside_a_rig_holding(n: usize) -> (Game, Entity) {
    let mut game = Game::new(4130, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let rig = spawn_machine_at(&mut game, "teardown_rig", 3, 3);
    let player = game.player_entity();
    let mut held = game.world.get_mut::<DownedPrograms>(player).unwrap();
    for level in 1..=n as u32 {
        held.0.push(DownedProgram {
            species: "scrapper".to_string(),
            level,
            rarity: Rarity::Ordinary,
            boss: false,
            condition: 70,
            carried: None,
        });
    }
    (game, rig)
}

fn clamp(id: &str) -> ToolId {
    ToolId(id.to_string())
}

#[test]
fn loading_a_rig_moves_the_named_programs_out_of_the_pack() {
    let (mut game, rig) = player_beside_a_rig_holding(3);
    game.load_teardown_rig(&[0, 2], &clamp("salvage_clamp")).unwrap();

    let player = game.player_entity();
    let left = &game.world.get::<DownedPrograms>(player).unwrap().0;
    assert_eq!(left.len(), 1, "one program should still be in the pack");
    assert_eq!(left[0].level, 2, "the one not named should be the one left");

    let queue = &game.world.get::<Hopper>(rig).unwrap().queue;
    assert_eq!(queue.len(), 2);
    assert!(queue.iter().all(|e| e.tool == clamp("salvage_clamp")));
}

/// An over-ask clamps rather than refusing — `take_from_adjacent`'s own
/// rule — and the remainder is still in the pack. Nothing is destroyed
/// (decision 9).
#[test]
fn a_bulk_load_past_the_hopper_clamps_and_leaves_the_rest_in_the_pack() {
    let (mut game, rig) = player_beside_a_rig_holding(10);
    let all: Vec<usize> = (0..10).collect();
    game.load_teardown_rig(&all, &clamp("salvage_clamp")).unwrap();

    let hopper_size = 6; // assets/structures/teardown_rig.ron
    let queue_len = game.world.get::<Hopper>(rig).unwrap().queue.len();
    assert_eq!(queue_len, hopper_size);

    let player = game.player_entity();
    let left = game.world.get::<DownedPrograms>(player).unwrap().0.len();
    assert_eq!(left, 10 - hopper_size, "the remainder stays in the pack");
}

#[test]
fn loading_with_no_rig_adjacent_refuses_and_spends_nothing() {
    let mut game = Game::new(4131, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 3, 4);
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0.push(DownedProgram {
        species: "scrapper".to_string(),
        level: 1,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    });

    assert!(game.load_teardown_rig(&[0], &clamp("salvage_clamp")).is_err());
    assert_eq!(game.world.get::<DownedPrograms>(player).unwrap().0.len(), 1);
}

#[test]
fn loading_with_an_uninstalled_tool_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_holding(1);
    assert!(game.load_teardown_rig(&[0], &clamp("no_such_tool")).is_err());

    let player = game.player_entity();
    assert_eq!(game.world.get::<DownedPrograms>(player).unwrap().0.len(), 1);
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
}

/// A Routine Reader teaches knowledge and a Harness Puller pays a
/// `GearCopy`; neither is a plain item, so neither can land in a
/// `Stock::output` (spec 10.4). Both stay hand work.
#[test]
fn a_routines_tool_refuses_at_the_deposit_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_holding(1);
    install_tool_for_test(&mut game, "routine_reader");
    assert!(game.load_teardown_rig(&[0], &clamp("routine_reader")).is_err());

    let player = game.player_entity();
    assert_eq!(game.world.get::<DownedPrograms>(player).unwrap().0.len(), 1);
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
}

#[test]
fn a_gear_tool_refuses_at_the_deposit_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_holding(1);
    install_tool_for_test(&mut game, "harness_puller");
    assert!(game.load_teardown_rig(&[0], &clamp("harness_puller")).is_err());

    let player = game.player_entity();
    assert_eq!(game.world.get::<DownedPrograms>(player).unwrap().0.len(), 1);
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
}

#[test]
fn loading_a_full_hopper_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_holding(10);
    let all: Vec<usize> = (0..6).collect();
    game.load_teardown_rig(&all, &clamp("salvage_clamp")).unwrap();

    let player = game.player_entity();
    let before = game.world.get::<DownedPrograms>(player).unwrap().0.len();
    assert!(game.load_teardown_rig(&[0], &clamp("salvage_clamp")).is_err());
    assert_eq!(game.world.get::<DownedPrograms>(player).unwrap().0.len(), before);
    assert_eq!(game.world.get::<Hopper>(rig).unwrap().queue.len(), 6);
}

#[test]
fn loading_during_a_battle_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_holding(1);
    /* open a battle — copy the shape from whichever fixture in
       `crates/engine/src/tests/extraction.rs` already proves
       `extract_program` refuses mid-battle; that refusal ships and its test
       is the one to mirror here. */
    assert!(game.load_teardown_rig(&[0], &clamp("salvage_clamp")).is_err());

    let player = game.player_entity();
    assert_eq!(game.world.get::<DownedPrograms>(player).unwrap().0.len(), 1);
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
}

#[test]
fn loading_no_such_program_refuses_and_spends_nothing() {
    let (mut game, rig) = player_beside_a_rig_holding(1);
    assert!(game.load_teardown_rig(&[9], &clamp("salvage_clamp")).is_err());
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
}
```

Write `install_tool_for_test` and `player_of` as tiny local helpers, or drop
them for whatever this repo already has — `crates/engine/src/tests/extraction.rs`
already installs tools for the phase-2 and phase-5 tests, so reuse that.
`ToolId`'s constructor is `ToolId(String)` (`crates/engine/src/tools.rs:25`).

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p feral-processes-engine --lib tests::extraction 2>&1 | tail -30
```

Expected: FAIL — `no method named `load_teardown_rig``.

- [ ] **Step 3: Implement the door**

In `crates/engine/src/game/extraction.rs`, inside `impl Game`:

```rust
    /// The Teardown Rig the player is standing beside, best-tiered first
    /// when a base somehow has two touching one tile.
    ///
    /// Adjacency rather than ownership, unlike `can_extract_routines`: this
    /// is a physical handover, and `take_from_adjacent` /
    /// `give_to_adjacent` are the rule for those. Sorted by tile before
    /// the pick, `adjacent_stock`'s reason — bevy's iteration order is not
    /// stable and an identical save must answer one keypress the same way
    /// on every run.
    pub fn adjacent_teardown_rig(&self) -> Option<Entity> {
        let (px, py) = self.base_pos()?;
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let p = e.get::<Position>()?;
                db.get(kind)?.strips.as_ref()?;
                Some((p.x, p.y, e.id()))
            })
            .filter(|(x, y, _)| {
                crate::game::base::collect::ORTHOGONAL
                    .iter()
                    .any(|(dx, dy)| (*x, *y) == (px + dx, py + dy))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).next()
    }

    /// The one door a downed program leaves the pack for a machine through
    /// — spec §10's deposit.
    ///
    /// Bulk-shaped so the per-row verb and the bulk verb share one set of
    /// refusals; a single index is a one-element slice. Refusals in order,
    /// all before anything moves (`commit_caravan_basket`'s rule): the run
    /// is over or a battle is active, no rig is adjacent, the tool is not
    /// installed, the tool's category cannot pay a plain item, the hopper
    /// has no room, `indices` names no held program.
    ///
    /// **An over-ask is clamped, not refused** — `take_from_adjacent`'s own
    /// rule. Ten into six free slots takes six and says so; the remainder
    /// stays in the pack and nothing is destroyed (decision 9).
    pub fn load_teardown_rig(
        &mut self,
        indices: &[usize],
        tool: &ToolId,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        let rig = self
            .adjacent_teardown_rig()
            .ok_or_else(|| "There is no rig here to load.".to_string())?;
        let tool_def = self
            .installed_tools()
            .into_iter()
            .find(|def| &def.id == tool)
            .ok_or_else(|| "That tool isn't installed.".to_string())?;
        if matches!(tool_def.category, ToolCategory::Routines | ToolCategory::Gear) {
            return Err(format!("The {} is work for your hands.", tool_def.name));
        }

        let capacity = {
            let kind = self.world.get::<Structure>(rig).unwrap().kind.clone();
            self.world
                .resource::<StructureDb>()
                .get(&kind)
                .and_then(|def| def.strips.as_ref())
                .map(|s| s.hopper as usize)
                .unwrap_or(0)
        };
        let room = capacity.saturating_sub(self.world.get::<Hopper>(rig).unwrap().queue.len());
        if room == 0 {
            return Err("The rig is full.".to_string());
        }

        let player = self.player_entity();
        let held = self.world.get::<DownedPrograms>(player).unwrap().0.len();
        // Deduplicated and sorted so a caller passing an index twice cannot
        // remove two programs, and so the removal below can walk backwards.
        let mut wanted: Vec<usize> = indices.iter().copied().filter(|i| *i < held).collect();
        wanted.sort_unstable();
        wanted.dedup();
        if wanted.is_empty() {
            return Err("No such downed program.".to_string());
        }
        let asked = wanted.len();
        wanted.truncate(room);
        let taking = wanted.len();

        // Highest index first, so every earlier index is still valid as the
        // removals happen. Pushed onto the queue in store order afterwards,
        // which is the order the player sees them in.
        let mut moved: Vec<DownedProgram> = Vec::with_capacity(taking);
        {
            let mut store = self.world.get_mut::<DownedPrograms>(player).unwrap();
            for index in wanted.iter().rev() {
                moved.push(store.0.remove(*index));
            }
        }
        moved.reverse();

        let labels: Vec<String> = moved.iter().map(|p| self.downed_program_label(p)).collect();
        {
            let mut hopper = self.world.get_mut::<Hopper>(rig).unwrap();
            for program in moved {
                hopper.queue.push(HopperEntry {
                    program,
                    tool: tool.clone(),
                });
            }
        }

        let left_behind = asked - taking;
        let line = if left_behind == 0 {
            format!("You load the rig with {}.", labels.join(", "))
        } else {
            format!(
                "You load the rig with {}. {left_behind} more stay in your pack — it is full.",
                labels.join(", ")
            )
        };
        self.log_base(line);

        // The turn `transfer_items` charges for a handover, and for its
        // reason: handing cargo across is the same errand. Not
        // `extraction_ticks` — the rig pays those, and charging both would
        // make automation cost more than doing it by hand.
        self.tick();
        Ok(())
    }
```

Import `Hopper`, `HopperEntry`, `Structure`, `StructureDb` and `Position` at
the top of the file if `use crate::*;` does not already bring them in.

- [ ] **Step 4: Run to verify the tests pass**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/extraction.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): the door a program leaves the pack for a rig through"
```

---

### Task 4: The step

Deliverable: a staffed, powered rig with a loaded hopper turns programs into
plain items in its output buffer, on its own, while the party is away.

**Files:**
- Create: `crates/engine/src/game/base/teardown.rs`
- Modify: `crates/engine/src/game/base/mod.rs` — declare the module
- Modify: `crates/engine/src/game/turn.rs` — call the pass
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `Hopper`, `HopperEntry` (Task 2); `Game::extraction_yield`,
  `Game::extraction_ticks` (shipped, `game/extraction.rs:148` and `:178`);
  `components::MachineStatus`, `systems::set_machine_status`,
  `components::Stock`, `TaskKind::GatherResource`, `resources::PowerGrid`.
- Produces: `pub(crate) fn run_teardown_rigs(&mut self)` on `Game`.

- [ ] **Step 1: Write the failing tests**

```rust
/// The fixture the step tests share: a powered rig with a posted worker
/// standing on it, one program loaded.
fn a_staffed_rig_loaded_with_one_program() -> (Game, Entity) {
    let (mut game, rig) = player_beside_a_rig_holding(1);
    // The posted worker, in `chains.rs`'s `staffed` shape (that helper is
    // private to its own test module, so this is a local copy rather than a
    // call). `required` is 1: nothing here reads `Task::required`, since the
    // rig's pace is `Game::extraction_ticks` and not a per-batch counter.
    let worker = spawn_tamed_for_test(&mut game, 3, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: rig,
        progress: 0,
        required: 1,
    });
    game.load_teardown_rig(&[0], &clamp("salvage_clamp")).unwrap();
    (game, rig)
}

/// **The identity.** §3's one-derivation invariant, applied to the rig as a
/// third caller. It fails loudly the day anyone re-derives the yield
/// formula inside the step.
#[test]
fn what_a_rig_pays_equals_what_extraction_yield_quotes_for_the_same_pair() {
    let (mut game, rig) = a_staffed_rig_loaded_with_one_program();
    let entry = game.world.get::<Hopper>(rig).unwrap().queue[0].clone();
    let tool = game
        .installed_tools()
        .into_iter()
        .find(|d| d.id == entry.tool)
        .unwrap();
    let quoted = game.extraction_yield(&entry.program, &tool);
    let ticks = game.extraction_ticks(&tool);

    for _ in 0..ticks {
        game.tick();
    }

    let output = &game.world.get::<Stock>(rig).unwrap().output;
    for (item, qty) in &quoted {
        assert_eq!(
            output.get(item).copied().unwrap_or(0),
            *qty,
            "the rig paid a different figure than extraction_yield quoted for {item:?}"
        );
    }
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
}

#[test]
fn an_unstaffed_rig_advances_nothing() {
    // Loaded but never staffed — the fixture above posts a worker, so this
    // one builds up from the Task 3 fixture instead.
    let (mut game, rig) = player_beside_a_rig_holding(1);
    game.load_teardown_rig(&[0], &clamp("salvage_clamp")).unwrap();
    for _ in 0..50 {
        game.tick();
    }
    assert_eq!(game.world.get::<Hopper>(rig).unwrap().queue.len(), 1);
    assert_eq!(game.world.get::<Hopper>(rig).unwrap().progress, 0);
    assert!(game.world.get::<Stock>(rig).unwrap().output.is_empty());
}

#[test]
fn a_dark_rig_advances_nothing() {
    let (mut game, rig) = a_staffed_rig_loaded_with_one_program();
    /* Put the rig in the dark. `crates/engine/src/tests/power.rs:673` is the
       file that knows how; copy its shape rather than writing to
       `resources::PowerGrid` by hand. */
    for _ in 0..50 {
        game.tick();
    }
    assert_eq!(game.world.get::<Hopper>(rig).unwrap().queue.len(), 1);
}

/// **The completion gate is room for the whole payout, not room for one
/// unit.** The assembler can use `> 0` because it makes one unit at a time;
/// a program pays several, and clamping to the room available would destroy
/// units. A rig that cannot hold the payout holds the program.
#[test]
fn a_rig_that_cannot_hold_the_whole_yield_holds_the_program() {
    let (mut game, rig) = a_staffed_rig_loaded_with_one_program();
    let entry = game.world.get::<Hopper>(rig).unwrap().queue[0].clone();
    let tool = game
        .installed_tools()
        .into_iter()
        .find(|d| d.id == entry.tool)
        .unwrap();
    let total: u32 = game
        .extraction_yield(&entry.program, &tool)
        .iter()
        .map(|(_, q)| *q)
        .sum();
    assert!(total > 1, "the fixture needs a payout bigger than one unit");

    // Fill the buffer so exactly one unit of room is left.
    {
        let mut stock = game.world.get_mut::<Stock>(rig).unwrap();
        let capacity = stock.capacity;
        stock
            .output
            .insert(ItemId::from("core_fragment"), capacity - 1);
    }

    for _ in 0..100 {
        game.tick();
    }
    assert_eq!(
        game.world.get::<Hopper>(rig).unwrap().queue.len(),
        1,
        "the program should still be waiting, not part-paid"
    );
    assert_eq!(
        *game.world.get::<Stock>(rig).unwrap().output.get(&ItemId::from("core_fragment")).unwrap(),
        game.world.get::<Stock>(rig).unwrap().capacity - 1,
        "nothing should have been added"
    );
    assert_eq!(
        *game.world.get::<MachineStatus>(rig).unwrap(),
        MachineStatus::Clogged
    );
}

/// The whole loop: the rig runs while the party is not standing there.
#[test]
fn a_rig_strips_while_the_party_is_in_a_zone() {
    let (mut game, rig) = a_staffed_rig_loaded_with_one_program();
    descend(&mut game);
    for _ in 0..100 {
        game.tick();
    }
    assert!(game.world.get::<Hopper>(rig).unwrap().queue.is_empty());
    assert!(!game.world.get::<Stock>(rig).unwrap().output.is_empty());
}
```

Two things to resolve before writing these:

- `spawn_tamed_for_test` stands in for whatever this repo's test-side
  "spawn a tamed program" helper is called. `crates/engine/src/tests/chains.rs`
  uses a private `spawn_tamed(game, x, y)`; find the shared one (or copy
  chains.rs's into `tests/extraction.rs`) and use its real name throughout.
- **Darkening a rig has no precedent in `chains.rs`.** `crates/engine/src/tests/power.rs:673`
  records that `Game::new` leaves `resources::PowerGrid` at its `Default`;
  read that file for how a test puts a machine in the dark and copy it. If
  no test does it directly, drive the Grid the way the game does — build the
  rig with no supply standing — rather than writing to `PowerGrid` by hand.

Do not leave a `/* … */` comment in a committed test.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p feral-processes-engine --lib tests::extraction 2>&1 | tail -30
```

Expected: FAIL — the hopper never drains, because nothing runs it.

- [ ] **Step 3: Write the pass**

Create `crates/engine/src/game/base/teardown.rs`:

```rust
//! The Teardown Rig's own tick: what a loaded hopper turns into, and when
//! — see `docs/superpowers/specs/2026-09-04-program-extraction-design.md`
//! §10.
//!
//! **A `&mut Game` pass rather than a bevy system, and that is forced.**
//! `Game::extraction_yield` and `Game::extraction_ticks` are `&Game`
//! methods folding perks, `SpeciesDb`, `ItemDb` and `best_structure_tier`;
//! a bevy system cannot call them and would have to re-derive the formula,
//! which is exactly the crack §3's "one derivation" exists to prevent. So
//! this joins `run_dig_crew`, `run_build_crew`, `run_repair_bays`,
//! `run_sorties` and `run_routes` in `game/turn.rs` instead, and the rig
//! calls the same two functions the player's own extraction calls.

use crate::*;

impl Game {
    /// Every standing rig, in tile order.
    ///
    /// Sorted for `run_repair_bays`' and `assembler_system`'s shared
    /// reason: bevy's query iteration order is not stable, and two rigs
    /// finishing in a different order between runs would reorder their log
    /// lines and the `grant_loot` calls behind them.
    fn teardown_rigs(&mut self) -> Vec<Entity> {
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let p = e.get::<Position>()?;
                db.get(kind)?.strips.as_ref()?;
                e.contains::<Hopper>().then_some((p.x, p.y, e.id()))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// One tick of every rig — `run_repair_bays`' shape.
    ///
    /// The gates mirror `assembler_system`'s, each writing `MachineStatus`
    /// on the transition alone (`set_machine_status`' rule that entering a
    /// state is news and staying in it is not): dark on the Grid is skipped
    /// writing no status, exactly as the assembler leaves that to
    /// `idle_machine_system`; no posted worker is `Unstaffed`; an empty
    /// queue writes nothing, since `Idle` is `idle_machine_system`'s to
    /// announce; a payout that will not fit is `Clogged`.
    pub(crate) fn run_teardown_rigs(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        for rig in self.teardown_rigs() {
            self.step_teardown_rig(rig);
        }
    }

    fn step_teardown_rig(&mut self, rig: Entity) {
        if self.world.resource::<PowerGrid>().is_dark(rig) {
            return;
        }
        // The same predicate `assembler_system` uses (`systems.rs:1601`),
        // read through the world rather than a system's query — the shape
        // `run_repair_bays` takes at `game/base/repair.rs:117`.
        let staffed = {
            let mut posted = self.world.query::<&Task>();
            posted
                .iter(&self.world)
                .any(|t| t.target == rig && matches!(t.kind, TaskKind::GatherResource))
        };
        let Some(entry) = self.world.get::<Hopper>(rig).and_then(|h| h.queue.first().cloned())
        else {
            return; // Idle is `idle_machine_system`'s to announce.
        };
        if !staffed {
            self.set_rig_status(rig, MachineStatus::Unstaffed);
            return;
        }

        let Some(tool) = self
            .installed_tools()
            .into_iter()
            .find(|def| def.id == entry.tool)
        else {
            // The tool was uninstalled after the load. The queue is the
            // player's instruction and the program is not destroyed for it;
            // the rig simply cannot act on it, which reads as unstaffed
            // work rather than as a silent drain.
            self.set_rig_status(rig, MachineStatus::Unstaffed);
            return;
        };

        let granted = self.extraction_yield(&entry.program, &tool);
        let total: u32 = granted.iter().map(|(_, qty)| *qty).sum();
        let room = self.world.get::<Stock>(rig).map(|s| s.output_room()).unwrap_or(0);
        if room < total {
            self.set_rig_status(rig, MachineStatus::Clogged);
            return;
        }
        self.set_rig_status(rig, MachineStatus::Running);

        // Quoted once, before the spend — a bench demolished mid-strip must
        // not change what this program was already priced at, the rule
        // `extract_program` takes on its own tick loop.
        let due = self.extraction_ticks(&tool);
        let progress = {
            let mut hopper = self.world.get_mut::<Hopper>(rig).unwrap();
            hopper.progress += 1;
            hopper.progress
        };
        if progress < due {
            return;
        }

        {
            let mut hopper = self.world.get_mut::<Hopper>(rig).unwrap();
            hopper.queue.remove(0);
            hopper.progress = 0;
        }
        {
            let mut stock = self.world.get_mut::<Stock>(rig).unwrap();
            for (item, qty) in &granted {
                *stock.output.entry(item.clone()).or_default() += qty;
            }
        }

        // One line per completed program, unlike the assembler, which logs
        // no per-unit line. An ICE Breaker is a known constant output; a
        // program's yield is unique, unrepeatable information the player
        // has no other record of.
        let label = self.downed_program_label(&entry.program);
        let parts: Vec<String> = granted
            .iter()
            .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
            .collect();
        self.log_base_kind(
            MessageKind::Loot,
            format!("The rig strips {label}: {}.", parts.join(", ")),
        );
    }
}
```

Write `set_rig_status` as a small private helper that calls
`systems::set_machine_status` with the rig's def name and a `StallSite`,
copying the argument shape from `assembler_system`'s `announce` closure.
`downed_program_label` is currently private to `game/extraction.rs` — widen
it to `pub(crate)` rather than writing a second label builder.

Declare the module in `crates/engine/src/game/base/mod.rs` beside `repair`,
and call it from `crates/engine/src/game/turn.rs` immediately after
`self.run_repair_bays();`:

```rust
        self.run_teardown_rigs();
```

- [ ] **Step 4: Run to verify the tests pass**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: PASS. If a save test elsewhere fails, it is the new `StructureSave`
fields from Task 2 meeting a fixture, not this task.

- [ ] **Step 5: Prove the identity test is not vacuous**

Temporarily change the rig's payout to `granted` with every quantity
doubled, run
`cargo test -p feral-processes-engine --lib what_a_rig_pays_equals`, and
confirm it FAILS. Revert. A test that passes with the behaviour removed is
coverage theatre.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/game/base/teardown.rs crates/engine/src/game/base/mod.rs crates/engine/src/game/turn.rs crates/engine/src/game/extraction.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): a rig strips its hopper while you are away"
```

---

### Task 5: The two verbs

Deliverable: the player can load the rig from the screen they already use.

**Files:**
- Modify: `crates/app-core/src/lib.rs` — the bulk-intent flag on `App`
- Modify: `crates/app-core/src/app/extraction.rs` — key handling
- Modify: `crates/gui/src/render/extraction.rs` — the header and the hint
- Test: `crates/app-core/src/tests/extraction.rs`

**Interfaces:**
- Consumes: `Game::load_teardown_rig` (Task 3),
  `Game::adjacent_teardown_rig` (Task 3), existing
  `Game::downed_program_rows`, `Game::extraction_options`,
  `App::selected_index`, `App::pending_downed_program_index`,
  `App::menu_selected`, `App::report`.
- Produces: `App::downed_programs_bulk: bool`.

- [ ] **Step 1: Write the failing tests**

In `crates/app-core/src/tests/extraction.rs`:

```rust
/// `L` from the list opens the tool page in bulk intent, and a tool row
/// there queues **every** held program rather than the highlighted one.
/// The fixture these three share. `test_app` and the `program(..)` builder
/// are already in `crates/app-core/src/tests/extraction.rs` (its top of
/// file) — this adds the rig, the adjacency and the held programs.
fn app_beside_a_rig_holding(n: usize) -> App {
    let mut app = test_app(9100);
    /* Stand the party in base beside a `teardown_rig`, install
       `salvage_clamp`, and push `n` programs built with this file's own
       `program("scrapper", 70, Rarity::Ordinary, level)` into
       `DownedPrograms`. The phase-1 tests in this file already reach into
       the game to seed held programs — reuse that, and reach the base
       through `crates/app-core/src/tests/support.rs`'s helpers rather than
       driving keys. */
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char('D'));
    app
}

#[test]
fn the_bulk_verb_queues_every_held_program() {
    let mut app = app_beside_a_rig_holding(3);
    app.handle_key(GameKey::Char('L'));
    assert!(app.downed_programs_bulk);
    app.handle_key(GameKey::Char('a')); // the first tool row

    let game = app.game.as_ref().unwrap();
    assert!(game.downed_program_rows().is_empty(), "the pack should be empty");
    assert!(!app.downed_programs_bulk, "bulk intent clears after the act");
}

/// `Q` on the tool page queues exactly the one program whose page it is.
#[test]
fn the_per_row_verb_queues_one_and_leaves_the_rest() {
    let mut app = app_beside_a_rig_holding(3);
    app.handle_key(GameKey::Char('1')); // open program 1's tool page
    app.handle_key(GameKey::Char('Q'));

    let game = app.game.as_ref().unwrap();
    assert_eq!(game.downed_program_rows().len(), 2);
}

/// The gate the whole binding rests on: lowercase still extracts by hand,
/// and the two new uppercase keys pick no row. `App::selected_index`
/// (`app/input.rs:108`) refuses anything not lowercase or a digit, so this
/// is a regression guard on that rule rather than on this screen.
#[test]
fn the_new_uppercase_keys_pick_no_row_and_lowercase_still_extracts() {
    let mut app = app_beside_a_rig_holding(1);
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('a'));

    let game = app.game.as_ref().unwrap();
    assert!(game.downed_program_rows().is_empty(), "lowercase should extract by hand");
}
```

`test_app(seed)` and `program(species, condition, rarity, level)` already
exist at the top of `crates/app-core/src/tests/extraction.rs` — note that
`DownedProgram::carried` is an `Option`, not a `Vec` (`items.rs:358`), and
`species` is a plain `String`. Fill the one remaining `/* … */` from the
phase-1 screen tests in the same file, and do not leave a comment in a
committed test.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p feral-processes-app-core 2>&1 | tail -30
```

Expected: FAIL — `no field `downed_programs_bulk``.

- [ ] **Step 3: Add the flag**

In `crates/app-core/src/lib.rs`, beside `pending_downed_program_index`:

```rust
    /// Whether `Mode::DownedPrograms`' tool page is showing *bulk* intent —
    /// entered with `L` from the list, where a tool row queues every held
    /// program at the adjacent rig instead of extracting one by hand.
    ///
    /// A flag on the page rather than a second page: the rows are the same
    /// rows and the preview is the same preview; only what a row key means
    /// changes, which is what the header says. Cleared on every exit from
    /// the page, so a bulk load cannot leak into the next hand extraction.
    pub downed_programs_bulk: bool,
```

- [ ] **Step 4: Handle the keys**

In `crates/app-core/src/app/extraction.rs`, inside
`handle_downed_programs_key`. In the `None` (list) arm, before the
`selected_index` call:

```rust
                if key == GameKey::Char('L') {
                    self.downed_programs_bulk = true;
                    self.pending_downed_program_index = Some(0);
                    self.menu_selected = 0;
                    return;
                }
```

In the `Some(program_index)` arm, before its `selected_index` call:

```rust
                if key == GameKey::Char('Q') {
                    let tool_id = options[self.menu_selected.min(options.len() - 1)]
                        .tool
                        .clone();
                    let outcome = self
                        .game
                        .as_mut()
                        .unwrap()
                        .load_teardown_rig(&[program_index], &tool_id);
                    self.report(outcome);
                    self.pending_downed_program_index = None;
                    self.downed_programs_bulk = false;
                    self.menu_selected = 0;
                    return;
                }
```

and make the existing row branch fork on the intent:

```rust
                let tool_id = options[tool_idx].tool.clone();
                let outcome = if self.downed_programs_bulk {
                    let all: Vec<usize> = (0..self
                        .game
                        .as_ref()
                        .map(|g| g.downed_program_rows().len())
                        .unwrap_or(0))
                        .collect();
                    self.game.as_mut().unwrap().load_teardown_rig(&all, &tool_id)
                } else {
                    self.game
                        .as_mut()
                        .unwrap()
                        .extract_program(program_index, &tool_id)
                };
                self.report(outcome);
                self.pending_downed_program_index = None;
                self.downed_programs_bulk = false;
                self.menu_selected = 0;
```

Clear `downed_programs_bulk` in the `Esc` branch too, beside the existing
`self.menu_selected = 0`, and wherever the screen is opened from the pack.
`options` is empty when nothing is installed, so guard the `Q` branch with
`if options.is_empty() { return; }` above it.

- [ ] **Step 5: Draw the difference**

In `crates/gui/src/render/extraction.rs`, on the tool page, make the header
say which intent is showing — the row keys mean different things under each,
and a player who cannot tell them apart will queue a program they meant to
strip. Add the two key hints (`Q` on the tool page, `L` on the list) in the
same place and voice this screen already puts hints.

Height: the list page's rows are unchanged in count and shape, so
`MAX_DOWNED_PROGRAMS`' existing no-scroll census still covers it. If the
hint costs a line, run the height census and check it still passes at
1280x720 rather than assuming.

- [ ] **Step 6: Run the whole suite**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: PASS, including the gui height and help-text censuses.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/lib.rs crates/app-core/src/app/extraction.rs crates/app-core/src/tests/extraction.rs crates/gui/src/render/extraction.rs
git commit -m "feat(extraction): two verbs for loading the rig"
```

---

### Task 6: The record

Deliverable: the manual page, the seam and the changelog say what shipped.

**Files:**
- Modify: `assets/help/75-extraction.md`
- Modify: `docs/seams.md`
- Modify: `CHANGELOG.md`

Not touched, deliberately: `docs/manual.md` and the root `README.md` are
both carved out of this repo's doc obligation.

- [ ] **Step 1: Extend the manual page**

`assets/help/75-extraction.md` already covers what a kill leaves, the store,
the tools, the bench and what the screen quotes. Add a section for the rig:
that you research Teardown and build one, that you stand beside it and press
`L` to hand it everything or `Q` on a tool page to hand it one, that it needs
a posted program and power like any machine, that it strips while you are
out and leaves the results in its buffer for you or a hauler to collect, that
its hopper holds fewer programs than your pack does, and that the Routine
Reader and the Harness Puller are still work for your hands.

Check the page's own censuses still pass — the in-game manual has four, and
one of them holds the help text to never naming the `W` key.

- [ ] **Step 2: Record the seam**

Add a `docs/seams.md` entry for the instance boundary, since that is the
rule a later change is most likely to break without noticing: **a
`DownedProgram` exists in exactly two places, `DownedPrograms` on the player
and `Hopper` on a rig, and what leaves a rig is plain items.** Say why —
decision 2's plain-copy seam is what lets hauling, depots, `collect` and
work orders carry the yield with no instance rule — and name
`Game::run_teardown_rigs` as the one place the conversion happens.

Add a second entry for the derivation: the rig is a `&mut Game` pass and not
a bevy system *because* `extraction_yield` and `extraction_ticks` are
`&Game` methods, and a system would have to re-derive them.

- [ ] **Step 3: Write the changelog section**

Add an unreleased section to `CHANGELOG.md` in the file's existing shape,
covering the Teardown Rig, the Teardown research node, the two screen verbs,
and the `strips` field for modders.

- [ ] **Step 4: Run the whole suite one more time**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add assets/help/75-extraction.md docs/seams.md CHANGELOG.md
git commit -m "docs(extraction): the rig, in the manual and the seams"
```

---

## What this plan does not do

Recorded so a later reader does not mistake absence for oversight:

- **Work orders are untouched** (spec 10.2). No file under
  `game/base/work_orders.rs` appears in any task.
- **`Carrying`, `Stock` and `plan_adjacent_take` are untouched** (spec 10.1).
  `Stock` is *read and written* by Task 4, but only as the plain-item buffer
  it already is; no instance enters it.
- **No worker hauls programs to the rig.** The player carries them.
- **No program can be pulled back out of a hopper.** Loading is one-way.
- **No tuning is fitted.** `build_cost`, `power_draw`, `capacity`, `hopper`
  and the research `cost` are guessed, `FIGHT_CONDITION_WEIGHT` stays at
  `0.0`, no gear door closes, and `MAX_DOWNED_PROGRAMS` stays a count. All of
  those want a play session, and this plan is not it.

## After the plan

Nothing here has been seen on a screen. The suite going green is not
evidence that the rig reads well, that six hopper slots against ten pack
rows is the right tension, or that one log line per program is the right
amount of noise. Those are the first things to look at with the game
running.
