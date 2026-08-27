# Upgrade build requests — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Game::upgrade_structure` stops charging the player's pack and
files a `BuildSite` on the structure's own tile; the existing build crew
fetches the bill and works the site until the tier lands.

**Architecture:** One component covers both jobs. `BuildSite` gains a
`goal` discriminator — `New` or `Upgrade { to_tier }` — and exactly one
step branches on it: `raise_one_tick`'s completion. The crew, the walk,
the scheduler wants, both announcement latches, the reachability check
and the cancel refund are untouched. The site names a **tile**, never an
`Entity`, so the structure is resolved by position at completion and
there is nothing to dangle when it is destroyed underneath.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), serde/RON
saves.

**Spec:** `docs/superpowers/specs/2026-08-27-upgrade-build-requests-design.md`
— read it first. It carries the argument; this plan carries the sequence.

## Global Constraints

Every task's requirements implicitly include these.

- **No `SAVE_FORMAT_VERSION` bump.** The one save change is an additive
  field behind `#[serde(default)]`. If you find yourself needing to remove
  a field or change one's meaning under the same name, stop and report —
  that is a different piece of work.
- **No new `tuning.rs` constants.** `BUILD_TICKS_PER_MATERIAL` already
  prices this; an upgrade's bill scales with tier, so its duration does
  too, for free.
- **No new game content in Rust.** Nothing here adds a structure, item or
  ability; the upgrade paths are `assets/structures/*.ron` and stay there.
- **No version bump and no `CHANGELOG.md` section on the branch.** Both
  happen once, at the merge.
- **Do not write to `TODO.md`.**
- `cargo fmt` and `cargo clippy --workspace` after every task; fix
  warnings rather than silencing them.
- Commit at every green step. Check `git branch --show-current` before
  each commit — a concurrent session has fast-forwarded and deleted a
  branch mid-task before.
- **Two fixture facts that will otherwise waste an hour each:**
  `upgrade_ceiling` is `min(def.max_tier, zone)`, so **every upgrade is
  refused at zone 1** — a fixture must `set_zone(&mut game, 2)` or higher
  before filing, exactly as `upgrading_a_node_costs_materials_and_raises_
  its_tier` already does. And `support::deploy_upgradeable_node` gives the
  Mining Node, whose `upgrade` is `(max_tier: 5, cost: [("core_fragment",
  10), ("cache_grain", 1)])` — so reaching Mk2 is 20 Core Fragment and 2
  Cache Grain, 22 units, 44 ticks at the current rate.

## File structure

| File | Responsibility after this change |
|---|---|
| `crates/engine/src/components.rs` | `BuildGoal`; `BuildSite::goal` and an `upgrade` constructor beside `new` |
| `crates/engine/src/save.rs` | `BuildSiteSave::goal`, additive |
| `crates/engine/src/game/lifecycle.rs` | writes the goal; on load, **withholds the `Glyph` from an upgrade site** |
| `crates/engine/src/game/base/building.rs` | `upgrade_structure` files rather than charges; `count_build_requests` counts `New` only; `remove_structure` clears a pending site |
| `crates/engine/src/game/base/construction.rs` | `raise_one_tick` branches on the goal |
| `crates/engine/src/game/base/upkeep.rs` | `damage_structure` clears a pending site on the destroyed branch |
| `crates/engine/src/game/inspection.rs` | `build_order_row` carries the goal; `build_views` splits the job mark and hangs the pending row on the machine |
| `crates/engine/src/views.rs` | `BuildOrderRow::goal` |
| `crates/engine/src/tests/support.rs` | `raise_pending_builds` handles both goals; new `upgrade_now` |
| `crates/engine/src/tests/construction.rs` | the new behaviour's tests |
| `crates/gui/src/render/building.rs` | the upgrade menu draws through `build_cost_display` |
| `crates/app-core/src/app/building.rs` | refusal wording only |

---

### Task 1: `BuildGoal` on the component and in the save

Pure plumbing. Nothing changes behaviour; the point is that the field
exists, round-trips, and that an upgrade site is born without a `Glyph`.

**Files:**
- Modify: `crates/engine/src/components.rs` (`BuildSite`, around 1661-1700)
- Modify: `crates/engine/src/save.rs` (`BuildSiteSave`, around 510)
- Modify: `crates/engine/src/game/lifecycle.rs` (write ~1398-1409, load ~734-756)
- Test: `crates/engine/src/tests/construction.rs`

**Interfaces produced** — later tasks depend on these exact names:

```rust
pub enum BuildGoal {
    New,
    Upgrade { to_tier: u32 },
}

impl BuildSite {
    pub fn new(structure: StructureId, cost: Vec<(ItemId, u32)>) -> Self;      // goal: New
    pub fn upgrade(structure: StructureId, cost: Vec<(ItemId, u32)>, to_tier: u32) -> Self;
}
pub struct BuildSite { /* … */ pub goal: BuildGoal }
```

`BuildSite::structure` **keeps its meaning** — which structure kind the
site is about — and every existing reader of it stays correct.
`BuildGoal` needs `Clone`, `Copy`, `Debug`, `PartialEq`, `Serialize`,
`Deserialize`, and `Default` returning `New` so the save field's
`#[serde(default)]` lands on the right arm.

- [ ] **Step 1: Write the failing test.** In `tests/construction.rs`: a
      base with a hand-spawned upgrade `BuildSite` (position, a two-item
      cost, some `delivered`, a non-zero `progress`, `to_tier: 3`) is
      saved to a real file and loaded back; assert the goal, cost,
      delivered and progress all survive, **and that the loaded entity has
      no `Glyph`**. Model the save/load mechanics on the existing
      `a_part_supplied_request_survives_a_reload`. Assert the glyph half in
      the same test — the round-trip half alone passes against a load path
      that still attaches one.
- [ ] **Step 2: Run it and watch it fail.** `cargo test -p
      feral-processes-engine construction::` — expect a compile error on
      `BuildGoal`.
- [ ] **Step 3: Add the enum, the field and the `upgrade` constructor.**
- [ ] **Step 4: Add `BuildSiteSave::goal`** behind `#[serde(default)]`,
      with a doc comment stating why it costs no version bump.
- [ ] **Step 5: Thread it through both halves of `lifecycle.rs`.** The
      load path currently spawns `(BuildSite, Position, Glyph)` as one
      tuple with the glyph unconditional. An upgrade site must spawn
      **without** the `Glyph`, or the map draws two views on one tile.
      This is the non-obvious half of the task.
- [ ] **Step 6: Run the test — green.** Then `cargo test -p
      feral-processes-engine` for the crate.
- [ ] **Step 7: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 2: Filing and completion — the headline

The behaviour change, and the task that makes the suite green again.
Filing without completion would leave a request nothing can ever finish,
so the two ship together.

**Files:**
- Modify: `crates/engine/src/game/base/building.rs` (`upgrade_structure`
  ~449-520, `count_build_requests` ~397)
- Modify: `crates/engine/src/game/base/construction.rs` (`raise_one_tick`
  ~429-459)
- Modify: `crates/engine/src/tests/support.rs`
  (`raise_pending_builds` ~640, new `upgrade_now`)
- Modify: the 18 existing call sites — `tests/building.rs` (13),
  `tests/raids.rs` (3), `tests/research.rs` (1), `tests/base_space.rs` (1)
- Test: `crates/engine/src/tests/construction.rs`

**Interfaces consumed:** `BuildGoal`, `BuildSite::upgrade` from Task 1.

**Interfaces produced:**

```rust
// crates/engine/src/tests/support.rs
pub(super) fn upgrade_now(game: &mut Game, structure: Entity) -> Result<(), String>;
```
Files the request through the real `Game::upgrade_structure` and then
drives `raise_pending_builds`, so an existing test that only wants a Mk2
node changes by one line rather than growing a crew.

**What `upgrade_structure` keeps, in this order:** game-over/battle,
`require_base`, structure gone, unknown def, no upgrade path, `max_tier`,
zone ceiling. **What it drops:** the `Inventory` shortfall check, the
charge, the `StructureTier` insert, the `ResourceNode::level` write.
**What it gains:** a refusal when an upgrade is already on order at this
structure — its own sentence, because it leaves the player a different
errand from every refusal above it. It still `tick()`s.

**`count_build_requests` must count `BuildGoal::New` only.** Left alone,
every pending upgrade counts against that kind's `max_deployed` and a
legitimate deploy is refused with a figure the player cannot account for.
This is trap 1 in the spec and it belongs in this task, not a later one —
shipping Task 2 without it ships the bug.

**`raise_one_tick`'s `Upgrade` arm:** resolve the structure at the site's
`Position`, insert `StructureTier(to_tier)`, and write
`ResourceNode::level` **only where it is already `Some`** — a node that
always succeeds must stay that way. Then despawn the site and remove the
builder's `Task`, exactly as the `New` arm does. Where it cannot commit —
the machine is gone, or the tier now exceeds the ceiling — **leave the
site standing** rather than despawning: the `New` arm's missing-def
precedent, and the materials are still on the cell.

**`support::raise_pending_builds` must branch on the goal.** It currently
spawns a structure for every `BuildSite` it finds; pointed at an upgrade
site it stands a duplicate machine on an occupied tile, and every test
that files an upgrade then calls a `place_now` goes strange in a way that
reads as the feature being broken.

- [ ] **Step 1: Write the failing tests** in `tests/construction.rs`, one
      behaviour each:
      - filing an upgrade charges the pack **nothing** and leaves
        `StructureTier` where it was, while a `BuildSite` with the right
        goal, `to_tier` and tier-scaled cost now stands on the machine's
        tile;
      - a base with a builder and the materials on a shelf fetches them
        and the tier lands — drive real ticks, do not hand-complete;
      - a `ResourceNode` whose `level` is `None` still has `None` after
        the upgrade lands;
      - a pending upgrade does **not** consume a `max_deployed` slot: a
        deploy of that same kind is still accepted (pick a kind whose
        `max_deployed` is 1 so the assertion can bite);
      - a second upgrade request on the same structure is refused, with a
        sentence of its own;
      - **the machine keeps producing while its upgrade stands** — a posted
        worker on a node with a pending upgrade still fills its buffer over
        the ticks the crew is fetching. This is the decision the whole
        "keeps running" answer rests on and nothing else asserts it;
      - `cancel_build_request` on an upgrade site refunds the delivered
        units and logs. The function is untouched by this change, which is
        exactly why it needs a test here — nothing else proves an upgrade
        site is a first-class build request.
      Every one needs `set_zone(&mut game, 2)`.
- [ ] **Step 2: Run them and watch them fail**, each for the right reason
      — not merely "does not compile". `cargo test -p
      feral-processes-engine construction::`
- [ ] **Step 3: Rewrite `upgrade_structure` to file.**
- [ ] **Step 4: Narrow `count_build_requests` to `New`.**
- [ ] **Step 5: Branch `raise_one_tick`.**
- [ ] **Step 6: Teach `raise_pending_builds` both goals; add
      `upgrade_now`.**
- [ ] **Step 7: Migrate the 18 existing call sites.** Most become
      `upgrade_now(&mut game, node).unwrap()`. Read each one before
      changing it: `tests/building.rs:1065-1066` upgrades **twice** in a
      row and needs the first to have landed before the second is filed,
      and `tests/raids.rs:1435` and `tests/base_space.rs:119` are asserting
      on the *refusal*, so they must keep calling `upgrade_structure`
      directly.
- [ ] **Step 8: Run the engine suite green.** `cargo test -p
      feral-processes-engine`
- [ ] **Step 9: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 3: The two destruction paths

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs` (`damage_structure`
  ~420-482, the `destroyed` branch)
- Modify: `crates/engine/src/game/base/building.rs` (`remove_structure`
  ~532)
- Test: `crates/engine/src/tests/construction.rs`

A machine destroyed by a raid or demolished by the player must despawn
its pending upgrade site and refund the delivered units through
`Game::return_material` — the same door `cancel_build_request` uses,
Depots first and the pack second. **Wired into both paths**; one alone
strands goods on a cell nothing stands on, and nothing fails to compile
when only one is done.

`remove_structure` on the Home cascades to demolish every other
structure, so it must take every pending site with it. Prefer making the
per-structure clearing one private helper both paths call, rather than
two hand-written copies — the standing rule about a comment that claims
two places agree.

- [ ] **Step 1: Write the failing tests.** A half-supplied upgrade site
      whose machine is destroyed by `damage_structure` refunds exactly
      what was delivered and leaves no `BuildSite`; the same for
      `remove_structure`; and demolishing the **Home** clears a pending
      upgrade on a machine elsewhere in the base. Assert on the refunded
      quantity, not merely that the site is gone.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement, wiring both paths through one helper.**
- [ ] **Step 4: Run green**, plus `cargo test -p feral-processes-engine
      raids::` since `damage_structure` is that suite's subject.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 4: The job mark

**Files:**
- Modify: `crates/engine/src/game/inspection.rs` (`build_views`'
  `attended` set, ~776-810)
- Test: `crates/engine/src/tests/construction.rs`

`build_views` pairs `TaskKind::Construct` with `GatherResource` on the
stated grounds that a build site carries a glyph and so can wear the
mark. An upgrade site carries none, so that arm splits on the goal: the
**builder wears the mark for the whole job**, which is `Excavate`'s rule
and `Excavate`'s reason — there is no glyph at the other end of the
posting that belongs to the site.

The invariant to hold: **exactly one job mark per posted program at every
instant**. Left alone, a machine's own worker and its builder both claim
the machine's glyph.

- [ ] **Step 1: Write the failing test.** A machine with both a posted
      worker and a posted builder, ticked to several distinct moments
      (builder at a shelf, builder walking, builder adjacent to the site):
      at each, exactly one mark per posting. Update the doc comment on
      that arm in the same change — it currently asserts the glyph fact
      that is about to stop being universally true.
- [ ] **Step 2: Run and watch it fail.**
- [ ] **Step 3: Split the arm.**
- [ ] **Step 4: Run green.**
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 5: What the player sees

**Files:**
- Modify: `crates/engine/src/views.rs` (`BuildOrderRow` ~1933)
- Modify: `crates/engine/src/game/inspection.rs` (`build_order_row` ~550,
  `build_views`)
- Modify: `crates/gui/src/render/building.rs:754`
- Modify: `crates/app-core/src/app/building.rs` (refusal wording)
- Test: `crates/engine/src/tests/construction.rs`

- **`BuildOrderRow` gains the goal**, so a row reads `Lathe → Mk3` rather
  than `Lathe`. It stays the one derivation — the map, the examine line
  and `build_order_report` all read it, and every figure on it is a call.
- **The machine's own `EntityView::build` carries the pending row**,
  found by tile: the site has no glyph, so `view_entities` never selects
  it and never produces two views for one cell.
- **The upgrade menu draws through `build_cost_display`** (pack + base
  shelves) rather than `cost_display` (pack only). Left as it is, the menu
  quotes a store the verb no longer reads. `render/building.rs:83` and
  `:136` already show the call shape.
- **`app-core`'s `handle_upgrade_key` needs no logic change** — it already
  reports whatever `upgrade_structure` returns. Only wording that claims
  an instant upgrade needs revisiting.

- [ ] **Step 1: Write the failing tests.** Examining a machine with a
      pending upgrade says what tier is coming and what is still to be
      fetched; `build_order_report` lists an upgrade request alongside a
      deploy in the same stable tile order; the machine still draws as
      itself with the request standing (assert one view for that tile, not
      two).
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement the engine half.**
- [ ] **Step 4: Implement the gui and app-core half.** No test drives the
      renderer here; verify by reading `build_cost_display`'s signature at
      `render/mod.rs:656` and matching the two existing call sites.
- [ ] **Step 5: Run green.**
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 6: Gates and documentation

- [ ] **Step 1: `cargo test --workspace`.** The final gate. Passing only
      the tests you wrote is not evidence of correctness.
- [ ] **Step 2: `cargo test -p feral-processes-engine balance_sim`.**
      Nothing here should move a curve; a moved one is a signal, not a
      broken test.
- [ ] **Step 3: Prove one test non-vacuous.** Pick the `max_deployed` test
      from Task 2, revert `count_build_requests` to counting every goal,
      confirm it fails, restore. That trap is the one most likely to have
      shipped green.
- [ ] **Step 4: Update `CLAUDE.md`'s "The base" section and the matching
      `docs/seams.md` entry** under "A deploy is a request" — the rule in
      `CLAUDE.md`, the argument in `docs/seams.md`, same title. State that
      a `BuildSite` covers both goals, that the site names a tile rather
      than an entity, and name the three traps. Remember `CLAUDE.md` and
      `AGENTS.md` are gitignored twins: edit `CLAUDE.md`, then `cp` it.
- [ ] **Step 5: Check for claims this change falsifies.** `rg` the
      assets' schema READMEs and `docs/` for anything saying an upgrade is
      paid from the pack or is instant.
- [ ] **Step 6: Commit.**

---

## Notes for the executor

- **This is unplayed code you are extending.** Nothing on
  `feat/build-orders` has had a play session. Three bugs shipped past a
  green suite there and were found only by re-reading doc comments and
  asking whether they were still true. When a comment in this area claims
  a guard, a latch or an ordering, go and check it rather than trusting
  it.
- **Do not push.** Commits are free; pushing needs an explicit ask from
  the user.
- The user is remote and cannot play the game. Do not offer to launch it;
  state unplayed status plainly instead.
