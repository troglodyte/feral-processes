# Companion Progression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A companion can be developed past `CREATURE_MAX_LEVEL` by spending a
scarce boss-drop item on it, and every level so earned pays one point into a
per-class talent tree, so a developed program is individual and expensive to
replace.

**Architecture:** A `KernelRing` component raises one companion's level
ceiling; `Game::companion_level_cap` is the single expression of that ceiling
and the four sites reading `CREATURE_MAX_LEVEL` as a cap go through it or
through the absolute maximum. A `Talents` component records nodes taken from a
data-driven per-class tree in `assets/talents/`; points are derived from level,
never stored, and a `Stat` node bakes into `Stats` at purchase exactly as a
refactor does.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone in the engine), `ron` for
assets and saves, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-19-companion-progression-design.md`
— read it before Task 1. It carries the argument for every decision this plan
executes, including several that look arbitrary here.

## Global Constraints

Copied from `CLAUDE.md` and the spec. Every task's requirements include these.

- **Do not write implementation code into this plan's steps.** This plan gives
  you the file list, the interface to produce, the intent of each test and the
  gate to run. Code blocks appear only where the thing is genuinely non-obvious.
  That is a deliberate project rule, not an omission.
- **TDD, always.** Failing test first, watch it fail, minimal implementation,
  watch it pass, commit. Every task, every size.
- **A test that passes with the fix removed is not a test.** Delete your
  implementation and confirm the test goes red before you commit it.
- **`cargo test --workspace` is the final gate.** Iterate with
  `cargo test -p feral-processes-engine <name>`. Note that a single-crate run
  and a workspace run are *different builds* and can shift a seeded RNG stream,
  so a seeded test failing in one and passing in the other is a known trap, not
  a new bug.
- **`cargo fmt` and `cargo clippy --workspace` after every change.** Fix
  warnings, never silence them.
- **Never hardcode content in Rust that can be data.** Talent trees are
  `assets/talents/*.ron`. Difficulty knobs are `pub const` in
  `crates/engine/src/tuning.rs`, documented and grouped, never inline in a
  formula.
- **A malformed `.ron` is skipped with a logged warning, never a panic.**
  Follow `PerkDb::load_dir` (`crates/engine/src/perks.rs:186`).
- **Every new field on a loaded def is `#[serde(default)]`,** so third-party
  files keep parsing untouched.
- **No `SAVE_FORMAT_VERSION` bump.** Both save fields are additive on a
  field-named RON struct. If you find yourself wanting a bump, stop and ask.
- **Commit on green.** Branch is `companion-progression`; do not push, do not
  bump the workspace version, do not write to `TODO.md`.
- **Do not update `docs/manual.md` or the root `README.md`.** Both are
  explicitly carved out. `CHANGELOG.md` and `assets/*/README.md` still apply.

### Names fixed across tasks

Use these exactly. A later task consuming a different spelling is a bug.

| Name | Where | Shape |
|---|---|---|
| `components::KernelRing` | `crates/engine/src/components.rs` | `pub struct KernelRing(pub u32)`, absent means 0 |
| `components::Talents` | same | `pub struct Talents(pub Vec<TalentId>)`, absent means none |
| `tuning::KERNEL_RING_MAX` | `tuning.rs` | `u32`, proposed `3` |
| `tuning::LEVELS_PER_RING` | `tuning.rs` | `u32`, proposed `2` |
| `tuning::MAX_TALENT_STAT_PERCENT` | `tuning.rs` | `f32`, proposed `15.0` |
| `tuning::absolute_companion_level_cap()` | `tuning.rs` | `pub const fn () -> u32` |
| `Game::companion_level_cap` | `crates/engine/src/game/party.rs` | `pub fn (&self, Entity) -> u32` |
| `Game::open_kernel_ring` | `crates/engine/src/game/refactor.rs` | `pub fn (&mut self, Entity) -> Result<(), String>` |
| `Game::ring_cost` | same | `pub fn (ring: u32) -> u32`, returns `ring + 1` |
| `items::ids::PRIVILEGE_RING` | `crates/engine/src/items.rs` | `&str`, `"privilege_ring"` |
| `talents::TalentId` | `crates/engine/src/talents.rs` | string newtype, mirrors `ItemId` |
| `talents::TalentDb` | same | `Resource`, `load_dir(&Path) -> io::Result<(Self, Vec<String>)>` |
| `Game::talent_tree` | `crates/engine/src/game/talents.rs` | `pub fn (&self, Entity) -> Option<&TalentTree>` |
| `Game::talent_points` | same | `pub fn (&self, Entity) -> TalentPoints` |
| `Game::take_talent` | same | `pub fn (&mut self, Entity, &TalentId) -> Result<(), String>` |
| `Game::talent_options` | same | `pub fn (&mut self, Entity) -> Vec<TalentOption>` |
| `views::PetInfo::ring` / `::talents` | `crates/engine/src/views.rs` | `u32` each |
| `Mode::Develop` / `Mode::DevelopProgram` | `crates/app-core/src/lib.rs` | picker, then the one screen |
| `App::pending_develop_target` | same | `Option<Entity>` |
| `render/talents.rs` | `crates/gui/src/render/` | `draw_develop`, `draw_develop_program` |
| `CreatureSave::ring` / `::talents` | `crates/engine/src/save.rs` | `u32` / `Vec<String>`, both `#[serde(default)]` |

---

## Milestone A — rings (Tasks 1–4)

At the end of Task 4 the feature is playable: kill an underground boss, take
the drop, spend it on a companion, watch that companion level past 6. Stop and
play it before starting Milestone B.

### Task 1: `KernelRing`, the one cap function, and the four cap sites

**Files:**
- Modify: `crates/engine/src/components.rs` — add `KernelRing`
- Modify: `crates/engine/src/tuning.rs` — `KERNEL_RING_MAX`, `LEVELS_PER_RING`,
  `absolute_companion_level_cap()`
- Modify: `crates/engine/src/game/party.rs` — add `Game::companion_level_cap`
- Modify: `crates/engine/src/game/combat_rewards.rs:685` — the `award_party_xp`
  cap
- Modify: `crates/engine/src/arena/mod.rs:75` — the staged-companion cap
- Modify: `crates/app-core/src/app/arena.rs:578` — the level stepper's clamp
- Modify: `crates/engine/src/save.rs` — `CreatureSave::ring`, plus the write
  and the read
- Modify: `crates/engine/src/lib.rs` — re-export `KernelRing` alongside the
  other components
- Test: `crates/engine/src/tests/level_up.rs`

**Do NOT modify `crates/engine/src/systems.rs:676`.** It passes
`Some(CREATURE_MAX_LEVEL)` too, but its `exp.level < WORK_XP_LEVEL_CAP` guard
(cap 5) stops it well below 6 already. Leaving it alone is what keeps structure
work from grinding a developed program — that is the whole of how the ring
stays inside "progression is earned by fighting". A test below pins it.

**Interfaces:**
- Consumes: `progression::add_xp(&mut Experience, &mut Stats, u32, f32,
  Option<u32>, u32) -> LevelGain` — already takes the cap as `Option`, so no
  signature changes.
- Produces: `Game::companion_level_cap(&self, e: Entity) -> u32`, returning
  `CREATURE_MAX_LEVEL + ring * LEVELS_PER_RING` where `ring` is `KernelRing`'s
  value or 0 when absent. `tuning::absolute_companion_level_cap() -> u32`
  returning `CREATURE_MAX_LEVEL + KERNEL_RING_MAX * LEVELS_PER_RING`, for the
  two arena sites which have no entity to read.

- [ ] **Step 1: Write the failing tests**

Four, in `tests/level_up.rs`:
1. A companion with no `KernelRing` still stops at `CREATURE_MAX_LEVEL` after
   being fed far more XP than it needs. Guards against the refactor lifting the
   cap for everyone.
2. A companion with `KernelRing(1)` fed the same XP reaches
   `CREATURE_MAX_LEVEL + LEVELS_PER_RING` and stops there.
3. A companion with `KernelRing(KERNEL_RING_MAX)` stops at
   `absolute_companion_level_cap()`.
4. A cronjob worker with `KernelRing(KERNEL_RING_MAX)` still stops at
   `WORK_XP_LEVEL_CAP`. Drive this through `task_progress_system` cycles, not
   by calling `add_xp` directly — the guard being tested is in `systems.rs`,
   and calling `add_xp` would test nothing.

Spawn companions with `spawn_tamed` from `tests/support.rs` and feed XP through
`Game::award_party_xp` so the real call site is exercised. Test 4 needs
`spawn_machine_at`, `work_node_parts` and `park_at_post` — a fixture that
hand-spawns a work node without `work_node_parts()` reads as a payout curve
that moved rather than as a fixture short something.

- [ ] **Step 2: Run them and confirm all four fail**

`cargo test -p feral-processes-engine level_up`

Tests 2 and 3 fail to compile (`KernelRing` does not exist). Test 1 and 4
should pass already — that is correct and expected; they are regression pins,
and their value is that they must *still* pass at the end.

- [ ] **Step 3: Add the component and the tuning constants**

`KernelRing` derives the same set `Refactors` does (`Component, Clone, Copy,
Debug, Default, PartialEq, Eq`). Document on it that absent means zero, citing
`Refactors` and `PurchasedTiers` as the precedent.

Both constants get the full `tuning.rs` doc treatment: what the knob does, why
that value, and what moves if it changes. Put them in a new labelled section.

- [ ] **Step 4: Add `Game::companion_level_cap` and wire the four sites**

The two arena sites take `absolute_companion_level_cap()`, not a per-entity
value — an arena scenario authors its own composition and has no `KernelRing`
to read. Document that on both, because it looks like a bug otherwise: the
reason is that `Ability`, `Affinity` and `RoutineSlot` talents are invisible to
`balance_sim`, so the arena is the only instrument that can see them, and an
arena clamped at 6 could not stage the fight the tree exists to change.

- [ ] **Step 5: Run the tests and confirm all four pass**

`cargo test -p feral-processes-engine level_up`

- [ ] **Step 6: Add the save field and its test**

`CreatureSave::ring: u32` behind `#[serde(default)]`, written from `KernelRing`
and read back into it on load. Insert the component only when the value is
non-zero, matching how the absent-means-zero components already round-trip.

The test must be a **save → load → assert**, not a RON round-trip. A RON round
trip cannot catch a field that fails to travel — that is exactly what
`#[serde(skip)]` looks like from the round trip's side. Assert the ring
survives *and* that a loaded companion's level cap is still lifted.

- [ ] **Step 7: Run the gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "feat(progression): a Kernel Ring lifts one companion's level ceiling"
```

Stage explicit paths rather than `git add -A`.

---

### Task 2: The Privilege Ring item and its underground boss drop

**Files:**
- Create: `assets/items/privilege_ring.ron`
- Modify: `crates/engine/src/items.rs` — add `PRIVILEGE_RING` to the `ids`
  module
- Modify: `crates/engine/src/game/combat_rewards.rs` — the drop, beside
  `pay_stack_boss_fragments`
- Modify: `crates/engine/src/tuning.rs` — the drop rate constant
- Modify: `assets/items/README.md` if the item needs any schema note
- Test: `crates/engine/src/tests/combat_rewards.rs`

**Interfaces:**
- Consumes: `Game::award_loot`'s boss branch
  (`combat_rewards.rs:440`), which already splits on `self.stack_pos()` —
  `Some` means a lair guardian underground, `None` means a surface boss.
- Produces: a Privilege Ring in the player's inventory after an underground
  boss dies. Task 3 consumes it by `ItemId`.

- [ ] **Step 1: Write the failing tests**

Two, in `tests/combat_rewards.rs`:
1. An underground boss death yields at least one `privilege_ring` in the
   player's inventory.
2. A **surface** boss death yields none. This is the half that matters — the
   gate is `is_boss_creature` *and* underground, and a test of the first half
   alone passes against a drop wired into the wrong branch.

Look at how the existing Portal Fragment tests stage an underground boss kill
and follow them; `descend` and `stand_at` are in `tests/support.rs`.

- [ ] **Step 2: Run them and confirm both fail**

`cargo test -p feral-processes-engine combat_rewards`

Test 1 fails on the missing item. Test 2 passes vacuously — note that, and
re-check it after Step 4, since it is the one that catches a misplaced hook.

- [ ] **Step 3: Author the item**

Follow `assets/items/recompile_kernel.ron` for shape. It needs `id`, `name`,
`description`, and a `value`. It is **not** `craftable` — a Refactor Bench
recipe would make rings renewable on demand, which is the opposite of scarce
and is the decision the spec argues for at length. It carries no `role`: the
four currency roles are taken and a ring is loot, not a currency.

Write the description in the player's vocabulary. Note the project rule that
"Raid" is the code's word and "GC Entropy Sweep" is the player's — the same
noun-phrase discipline applies to new player-facing text.

- [ ] **Step 4: Add the drop**

Beside `pay_stack_boss_fragments`, in the same `Some(pos)` arm — the ring and
the fragments are both what the party went down for. Grant it through
`grant_loot` and record it with `record_drop`, matching the fragment path, so
it appears in the fight's loot line.

Draw any randomness from `GameRng` **inside this function** the way the
fragment payout does. This is combat loot, not world generation, so `GameRng`
is correct here — do not seed a local `StdRng`.

- [ ] **Step 5: Run the tests and confirm both pass, then delete the hook and
      confirm test 1 goes red**

- [ ] **Step 6: Run the gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add assets/items/privilege_ring.ron && git add -u
git commit -m "feat(items): a lair guardian drops a Privilege Ring"
```

Expect `tests/assets.rs`'s item censuses to have an opinion — the price bound
and the currency census both walk the real assets. If one fails, read what it
is protecting before changing anything; the recipe-ceiling bound exists because
a craftable worth more than its ingredients is an infinite Credit loop.

---

### Task 3: Opening a ring, and surfacing it on `PetInfo`

**Files:**
- Modify: `crates/engine/src/game/refactor.rs` — `open_kernel_ring`, `ring_cost`
- Modify: `crates/engine/src/views.rs` — `PetInfo::ring`
- Modify: `crates/engine/src/game/party.rs` — fill it in `owned_pets`
- Test: `crates/engine/src/tests/refactor.rs`

`refactor.rs` is the right home: it is already "permanent, player-driven
upgrades to a tamed program", and a ring is a third track alongside the zone
bump and the percentage buffs. Widen its module doc to say so.

**Interfaces:**
- Consumes: `items::ids::PRIVILEGE_RING`, `components::KernelRing`,
  `tuning::KERNEL_RING_MAX`.
- Produces: `Game::open_kernel_ring(&mut self, target: Entity) ->
  Result<(), String>` and `Game::ring_cost(ring: u32) -> u32`.
  `views::PetInfo::ring: u32`.

`ring_cost(ring)` returns `ring + 1`, so opening the first ring costs one and
the third costs three. Take the *current* ring as the argument, not the target
ring, so the caller never has to add one itself.

- [ ] **Step 1: Write the failing tests**

Five, in `tests/refactor.rs`:
1. With enough rings held, `open_kernel_ring` succeeds, `KernelRing` goes to 1,
   and exactly `ring_cost(0)` rings are consumed.
2. With none held, it returns `Err` and consumes nothing. Assert the inventory
   is untouched, not only that the result is `Err` — a refusal that has already
   spent the item is the bug worth catching.
3. Opening the second ring costs `ring_cost(1)`, i.e. more than the first.
4. At `KERNEL_RING_MAX` it refuses, and the error names the ceiling.
5. Opening a ring does **not** change the companion's level, stats or XP. The
   ring buys room, not power; a test that only checks the cap would pass
   against an implementation that also handed out a free level.

- [ ] **Step 2: Run them and confirm they fail**

`cargo test -p feral-processes-engine refactor`

- [ ] **Step 3: Implement `open_kernel_ring` and `ring_cost`**

Order the checks the way `Game::refactor_companion` does and the way the
structure upgrade path does: **every refusal before anything is spent.** Refuse
at the ceiling first, then on materials.

Return `Err(String)` with a player-readable message; the app-core handler puts
it straight in the status line, so it must read as a sentence, not a code.

- [ ] **Step 4: Run the tests and confirm they pass**

- [ ] **Step 5: Add `PetInfo::ring` and fill it**

Document the field the way `PetInfo::refactors` is documented, including that
absent means 0.

- [ ] **Step 6: Run the gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "feat(progression): spend Privilege Rings to open a companion's next ring"
```

---

### Task 4: The Develop screen — Milestone A playable

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode::Develop`, `Mode::DevelopProgram`,
  `App::pending_develop_target`, and the `Mode` classification lists around
  lines 1059 and 1069
- Modify: `crates/app-core/src/app/group_menu.rs` — one `PARTY_ROWS` entry
- Modify: `crates/app-core/src/app/input.rs` — dispatch both modes
- Modify: `crates/app-core/src/app/party.rs` — `handle_develop_key`,
  `handle_develop_program_key`
- Create: `crates/gui/src/render/talents.rs` — `draw_develop`,
  `draw_develop_program`
- Modify: `crates/gui/src/render/mod.rs` — declare the module and dispatch both
  modes
- Test: `crates/app-core/src/tests/` — a new `develop.rs`, modelled on
  `refactor.rs`

**Interfaces:**
- Consumes: `Game::owned_pets()`, `Game::open_kernel_ring`, `PetInfo::ring`.
- Produces: the screen Task 11 extends with the talent ladder. Build it so the
  ring section is one block on the page, not the whole page.

- [ ] **Step 1: Write the failing tests**

In a new `crates/app-core/src/tests/develop.rs`, modelled closely on
`tests/refactor.rs`:
1. The party menu's "Develop a program" row opens `Mode::Develop`; picking a
   program opens `Mode::DevelopProgram`; Esc backs out one page at a time.
2. The row is **hidden** when the player owns no programs, and the row test
   goes through `App::party_menu_rows` rather than through a bespoke predicate
   — rows are hidden dynamically and that table is the only source of them.
3. A refusal from `open_kernel_ring` lands in `status_line` and the page holds
   rather than backing out.

- [ ] **Step 2: Run them and confirm they fail**

`cargo test -p feral-processes-app-core develop`

- [ ] **Step 3: Add the modes, the row and the handlers**

The `PARTY_ROWS` entry is `surface_only: false`. Like the Refactor row, this
reaches no zone-map state through `Position`, so it works four frames down —
and `surface_only` is a column in that table rather than a check inside each
predicate precisely so it stays in step with `require_surface`'s caller list.

`available` is "the player owns at least one program".

Follow `handle_refactor_key` / `handle_refactor_item_key` for the two-page
shape, including that the second page **stays put** after a successful action
rather than returning to the picker.

- [ ] **Step 4: Draw the screen**

`render/talents.rs` may name **no** graphics library. Everything goes through
`Painter` and the local `Color`/`Rect`/`TextDims`/`TextRun` types. Follow
`render/party.rs`'s `draw_refactor_item` for the row-building idiom.

The page shows the program, its current ring, its current and maximum level,
how many Privilege Rings the player holds, and the cost of the next one. Leave
obvious room below for Task 11's ladder.

- [ ] **Step 5: Run the tests and confirm they pass**

- [ ] **Step 6: Run the gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/gui/src/render/talents.rs crates/app-core/src/tests/develop.rs && git add -u
git commit -m "feat(ui): a Develop screen for opening a companion's next ring"
```

- [ ] **Step 7: STOP and play it**

```sh
cargo run -- --template stack
```

`dev-saves/README.md` says what that template sets up. Get underground, kill a
lair guardian, spend the ring, and level the companion past 6. This project has
a standing problem of features shipping with a green suite and zero screen
time; Milestone A is small enough to actually check.

Report what it felt like before starting Task 5. Specifically: is one ring per
lair too many or too few, and does "open a ring" read as meaningful when it
pays nothing on its own?

---

## Milestone B — talents (Tasks 5–12)

### Task 5: `TalentDb`, the asset schema, and the censuses

Data and validation only. No node does anything yet, and that is deliberate:
this task is independently reviewable as "the trees load, are well-formed, and
every id in them resolves".

**Files:**
- Create: `crates/engine/src/talents.rs`
- Create: `assets/talents/striker.ron`, `bastion.ron`, `medic.ron`,
  `saboteur.ron`, `leech.ron`, `generic.ron`
- Create: `assets/talents/README.md`
- Modify: `crates/engine/src/lib.rs` — declare the module, re-export the types
- Modify: `crates/engine/src/game/lifecycle.rs` — load the db and
  `insert_resource` it alongside `perk_db` (around line 101), and surface its
  warnings the way the other dbs' are surfaced
- Test: `crates/engine/src/tests/assets.rs`, plus unit tests in `talents.rs`

**Interfaces:**
- Consumes: `species::AffinityClass` (`Striker`, `Bastion`, `Medic`,
  `Saboteur`, `Leech`), `abilities::AffinityKind`, `abilities::AbilityId`.
- Produces:
  - `TalentId` — string newtype, mirroring `items::ItemId`. Derive `Ord` only
    if something needs it; do not add it speculatively.
  - `TalentNode` — the four kinds: `Stat { stat, percent }`,
    `Affinity { kind, mult }`, `Ability { id }`, `RoutineSlot`.
  - `TalentChoice { id: TalentId, name: String, description: String, node:
    TalentNode }`.
  - `TalentTier(Vec<TalentChoice>)` — exactly two choices.
  - `TalentTree { class: Option<AffinityClass>, tiers: Vec<TalentTier> }`.
  - `TalentDb` — `Resource`, `load_dir(&Path) -> io::Result<(Self,
    Vec<String>)>`, `get(Option<AffinityClass>) -> Option<&TalentTree>` falling
    back to the generic tree when the class has no file, and `all_nodes()` for
    the censuses.

`TalentDb::get` taking `Option<AffinityClass>` rather than `AffinityClass` is
load-bearing. `SpeciesDef::affinity_class` returns `Option` and `None` means
*no base job* rather than a default class — a boss carries no affinities, and
so does a mod raising two axes. Both must land on the generic tree rather than
silently acquiring a Medic's.

- [ ] **Step 1: Write the failing loader tests**

Unit tests in `talents.rs`, using `tests::support::scratch_assets_dir` to write
throwaway `.ron` files:
1. A well-formed tree loads with no warnings.
2. A malformed file is **skipped with a warning**, and the other files in the
   directory still load. Never a panic.
3. A tier with one choice, or three, is skipped with a warning naming the tier.
4. A tree with the wrong number of tiers is skipped with a warning.

- [ ] **Step 2: Run them and confirm they fail**

`cargo test -p feral-processes-engine talents`

- [ ] **Step 3: Write the types and the loader**

Follow `PerkDb::load_dir` exactly for the skip-with-warning shape, including
that validation failures produce a warning and a `continue` rather than an
`Err`.

Every field is `#[serde(default)]` where a default is meaningful, so a mod's
file survives a later schema addition untouched.

- [ ] **Step 4: Run the loader tests and confirm they pass**

- [ ] **Step 5: Author the six shipped trees**

`KERNEL_RING_MAX * LEVELS_PER_RING` tiers each — six at the proposed constants
— two choices per tier.

**Author the `Stat` percentages small and weight each tree toward `Ability`,
`Affinity` and `RoutineSlot`.** Two reasons, and the second is the one that
matters: a developed companion already carries four multiplicative axes
(Recompile Kernel tiers, five refactor slots at ~1.28x power, ring levels, and
now talents), and options compound far less dangerously than numbers. Weighting
away from `Stat` also keeps more of the tree inside `balance_sim`'s reach —
`Ability`, `Affinity` and `RoutineSlot` nodes are invisible to it.

`assets/species/README.md`'s "Kits" section is the authority on what each class
means. A Medic tree that reads like a Striker's is a content bug even though
nothing will fail.

Every `Ability` node's id must exist in `assets/abilities/`. A kit entry must
be a **battle** ability — `AffinityKind` is blind to the distinction, so a
`FieldBuff(kind: Def)` reports `Buff` like any other buff while never appearing
in the Special picker, which is the one place a kit is spent.

- [ ] **Step 6: Write the censuses**

In `crates/engine/src/tests/assets.rs`, over the **real** assets, not fixtures:
1. Every `AffinityClass` variant has a tree file, and `generic.ron` exists.
   Drive this by iterating the enum, not by naming five files — a sixth class
   should fail this test the day it is added.
2. Every `Ability` node's id resolves in `AbilityDb`.
3. Every tree has exactly `KERNEL_RING_MAX * LEVELS_PER_RING` tiers, each with
   two choices.
4. Every `Stat` node's percentage is at or under `MAX_TALENT_STAT_PERCENT`.
5. `TalentId`s are unique across all trees.

- [ ] **Step 7: Write `assets/talents/README.md`**

The schema reference: every field, every node kind, what each means, and the
authoring guidance from Step 5. Match the tone and depth of
`assets/perks/README.md` and `assets/species/README.md` — these are the
reference for anyone modding the game, and the moddability rule requires the
doc in the same change as the schema.

- [ ] **Step 8: Register the db and run the gates**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add assets/talents crates/engine/src/talents.rs && git add -u
git commit -m "feat(talents): per-class talent trees as loadable assets"
```

---

### Task 6: `Talents`, derived points, `take_talent`, and the `Stat` node

**Files:**
- Modify: `crates/engine/src/components.rs` — `Talents`
- Create: `crates/engine/src/game/talents.rs`
- Modify: `crates/engine/src/game/mod.rs` — declare it
- Modify: `crates/engine/src/game/refactor.rs` — `raised` becomes `pub(crate)`
- Modify: `crates/engine/src/save.rs` — `CreatureSave::talents`
- Modify: `crates/engine/src/views.rs` — `PetInfo::talents`, `TalentOption`
- Test: `crates/engine/src/tests/` — a new `talents.rs`

**Interfaces:**
- Consumes: `TalentDb`, `Game::companion_level_cap`, `refactor::raised`.
- Produces:
  - `TalentPoints { earned: u32, spent: u32 }` with `unspent() -> u32`.
  - `Game::talent_points(&self, Entity) -> TalentPoints`.
  - `Game::talent_tree(&self, Entity) -> Option<&TalentTree>`.
  - `Game::take_talent(&mut self, Entity, &TalentId) -> Result<(), String>`.
  - `Game::talent_options(&mut self, Entity) -> Vec<TalentOption>` — one row per
    choice in the **next unspent tier**, plus whatever the UI needs to grey out
    tiers already taken. Task 11 consumes this.

**Points are derived, never stored.** `earned =
level.saturating_sub(CREATURE_MAX_LEVEL)`, `spent = talents.len()`. There is no
points field on the component and none on the save. Anything that stores a
count here can desync from the level and the list; nothing that derives it can.

- [ ] **Step 1: Write the failing tests**

In a new `tests/talents.rs`:
1. A level-6 companion has zero earned points; at `CREATURE_MAX_LEVEL + 2` it
   has two.
2. `take_talent` with no unspent points returns `Err` and records nothing.
3. `take_talent` on a node from tier 2 while tier 1 is untaken returns `Err` —
   tiers are taken in order.
4. `take_talent` on a `TalentId` not in this companion's tree returns `Err`.
   Stage it with a node that exists in a *different* class's tree, which is the
   case a naive "is this id known" check gets wrong.
5. A `Stat` node raises the named stat once, and taking it twice is refused.
6. A `Stat` node on a program whose stat is small still gains at least a whole
   point — this is `refactor::raised`'s floor, and the test exists to prove the
   talent path *calls* it rather than restating the arithmetic.
7. A companion with no `Potential`/class — use `generic_species` from
   `tests/support.rs` — gets the generic tree rather than `None`.

- [ ] **Step 2: Run them and confirm they fail**

`cargo test -p feral-processes-engine talents`

- [ ] **Step 3: Implement**

Order every refusal before anything is spent or recorded, as in Task 3.

Make `refactor::raised` `pub(crate)` and call it. Do **not** restate its
arithmetic. A doc comment claiming to mirror another module's formula must be a
call, not a copy — this repo has been bitten by that four times, all in
`balance_sim.rs`, and the copy that drifts is always the one nobody runs.

- [ ] **Step 4: Run the tests and confirm they pass**

- [ ] **Step 5: Add the save field and its test**

`CreatureSave::talents: Vec<String>` behind `#[serde(default)]`.

**Load must not re-apply a `Stat` node's effect.** `CreatureSave` already
writes `hp/max_hp/atk/def` directly, so a saved program's stats already carry
its talents — re-applying on load would compound them on every reload. This is
the same rule refactors follow today. Write the test as: take a `Stat` talent,
save, load, and assert the stat is unchanged from before the save. That test is
the whole reason this rule is stated three times.

Again a **save → load → assert**, not a RON round trip.

- [ ] **Step 6: Add `PetInfo::talents` and `views::TalentOption`**

`TalentOption` carries what a menu row needs and nothing more: the id, the
name, the description, a short tag naming the node kind, and whether it is
takeable right now. Building a display string in a renderer is what lets two
screens disagree; keep the assembly in the engine.

- [ ] **Step 7: Run the gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add crates/engine/src/game/talents.rs crates/engine/src/tests/talents.rs && git add -u
git commit -m "feat(talents): spend levels earned past the cap on tree nodes"
```

---

### Task 7: The `RoutineSlot` node

**Files:**
- Modify: `crates/engine/src/game/combat.rs:557` — `Game::routine_slots`
- Test: `crates/engine/src/tests/talents.rs`

`Game::routine_slots(&self, entity: Entity)` already takes an entity and
already branches player-vs-companion, so this is the seam. Add the talent's
contribution in the companion branch only. Do **not** change
`abilities::companion_routine_slots(level)` — it is a pure level function and
several tests and `balance_sim` read it as one.

**Interfaces:**
- Consumes: `Game::talent_tree`, `components::Talents`.
- Produces: nothing new; `routine_slots` keeps its signature.

- [ ] **Step 1: Write the failing tests**

1. A companion holding a `RoutineSlot` talent has exactly one more slot than an
   identical companion without it.
2. The **player's** slot count is unaffected, even with `Talents` somehow
   present. The player is not a companion and must not read a companion tree.

- [ ] **Step 2: Run them and confirm they fail**

- [ ] **Step 3: Implement in the companion branch**

- [ ] **Step 4: Run the tests and confirm they pass**

Be alert for fallout: every routine slot in the game starts full, so a test
elsewhere that assumes a companion's slots are all occupied may now see a free
one. If something in `tests/routines.rs` or `tests/exclusive_routines.rs`
moves, read it before changing it.

- [ ] **Step 5: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "feat(talents): a RoutineSlot node widens a companion's kit"
```

---

### Task 8: The `Ability` node

**Files:**
- Modify: `crates/engine/src/game/combat.rs` — `install_innate_routines`
  (line 585) and `install_unlocked_routines`
- Test: `crates/engine/src/tests/talents.rs`

A granted routine must behave **exactly** like a species-kit unlock: same
install path, same slot competition, same interaction with what the program was
carrying when it was decompiled. The way to guarantee that is to fold the
talent's abilities into the `declared` list those functions already build,
rather than adding a second install path beside them.

**Interfaces:**
- Consumes: `Game::talent_tree`, `components::Talents`.
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

1. Taking an `Ability` node installs that routine, and the companion can be
   commanded to use it — assert through `Game::actor_abilities`, not by reading
   `Routines` directly, since `actor_abilities` is what the battle menu and
   resolution both go through.
2. A routine the program was **carrying** when decompiled keeps its slot when a
   talent ability is granted. That carried routine is the prize the player
   decompiled it for; `install_innate_routines` documents this and the talent
   path must not quietly evict it.
3. Taking an `Ability` node whose routine the companion already knows does not
   duplicate it.

- [ ] **Step 2: Run them and confirm they fail**

- [ ] **Step 3: Implement by widening `declared`**

- [ ] **Step 4: Run the tests and confirm they pass**

- [ ] **Step 5: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "feat(talents): an Ability node grants a routine outright"
```

---

### Task 9: The `Affinity` node

**Files:**
- Modify: `crates/engine/src/game/combat.rs:764` — `Game::ability_affinity`
- Test: `crates/engine/src/tests/talents.rs`

**Interfaces:**
- Consumes: `Game::talent_tree`, `components::Talents`,
  `tuning::AFFINITY_NEUTRAL`, `tuning::AFFINITY_MAX`.
- Produces: nothing new.

`ability_affinity` already has two arms — the player's, which sums perk levels
and clamps to `AFFINITY_MAX`, and the creature's, which reads the species
value. Add the talent's contribution to the **creature arm only**, and clamp it
to `AFFINITY_MAX` the same way the player arm does.

- [ ] **Step 1: Write the failing tests**

1. A companion with an `Affinity` talent for `Damage` deals more with a Damage
   Special than an identical companion without it, and the same for `Heal`.
2. The talent does **not** raise a category it does not name.
3. The result is clamped at `AFFINITY_MAX` — stage a species already near the
   ceiling.
4. The **player's** affinity is unchanged. Perks are the player's axis and a
   companion's affinity is its species' business; the spec's `DamageAffinity`
   perk doc says a party-wide version would multiply against it.

- [ ] **Step 2: Run them and confirm they fail**

- [ ] **Step 3: Implement in the creature arm**

- [ ] **Step 4: Run the tests and confirm they pass, and run `balance_sim`**

`cargo test -p feral-processes-engine balance_sim`

It should be untouched — `balance_sim` models no abilities, so an affinity
change is invisible to it. If a curve moves here, something is wrong with the
change, not with the gate.

- [ ] **Step 5: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "feat(talents): an Affinity node sharpens a companion's specialism"
```

---

### Task 10: Fusion keeps the survivor's development

**Files:**
- Modify: `crates/engine/src/game/party.rs:724` — `fuse_companions`
- Test: `crates/engine/src/tests/talents.rs`

**The decision:** the surviving program keeps its own `KernelRing` and
`Talents`; the consumed program's are lost.

This is the task most likely to be silently wrong. `fuse_companions` is one of
four doors into the roster and the one that assembles its **own** component
list rather than going through `Game::roster_parts()`; it also does its own
`retain`/`despawn` and skips the detachment logging `dissolve_tamed_program`
performs. Nothing fails to compile when a component is missing from a
hand-written tuple, and the symptom — a fused companion that lost its
development — reads as "fusion is bad" rather than as a bug.

Note also that `fuse_companions` strips gear **before** its stats snapshot,
because no stats operation may run while a gear bonus is sitting in `Stats`.
The decision above sidesteps that entirely by never applying a talent during
fusion. Do not "improve" it into re-applying the consumed program's nodes.

- [ ] **Step 1: Write the failing tests**

1. Fusing a developed program with an undeveloped one leaves the survivor's
   ring and talents intact.
2. The survivor's **stats** still carry the talent bonuses afterwards — the
   nodes were baked in, and fusion's stat snapshot must not lose them.
3. The consumed program's ring and talents do not transfer.

- [ ] **Step 2: Run them and confirm they fail**

Read the failure. If test 1 passes already, confirm it fails when you remove
whatever is making it pass before trusting it.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the tests and confirm they pass**

- [ ] **Step 5: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "fix(fusion): a fused program keeps its ring and talents"
```

---

### Task 11: The talent ladder on the Develop screen — Milestone B playable

**Files:**
- Modify: `crates/gui/src/render/talents.rs` — extend `draw_develop_program`
- Modify: `crates/app-core/src/app/party.rs` — extend
  `handle_develop_program_key`
- Modify: `crates/gui/src/render/manifest.rs` — a ring/talents section
- Test: `crates/app-core/src/tests/develop.rs`,
  `crates/gui/src/render/manifest.rs`'s own layout tests

**Interfaces:**
- Consumes: `Game::talent_options`, `Game::talent_points`, `Game::take_talent`,
  `PetInfo::ring`, `PetInfo::talents`.
- Produces: nothing downstream.

**One page, both verbs.** The ring block from Task 4 stays; the ladder joins it
below. Opening a ring and spending a point are the same decision loop, and
splitting them would make the player back out to see what they just bought.

- [ ] **Step 1: Write the failing tests**

In `crates/app-core/src/tests/develop.rs`:
1. Picking a takeable node calls through and the node appears in `talents`.
2. Picking a node with no unspent points leaves `status_line` set and the page
   held.
3. The page shows the ring block **and** the ladder — assert both are
   reachable from the same mode, so a later refactor cannot quietly split them.

- [ ] **Step 2: Run them and confirm they fail**

- [ ] **Step 3: Extend the handler and the screen**

Still no graphics library named in `render/`. Everything through `Painter`.

- [ ] **Step 4: Add the manifest section**

`manifest_layout`'s fixtures must match `sections_for`'s **emission order**,
not merely its row count. A drifted fixture has already hidden a live overflow
behind a green suite in this repo once. Read
`sections_for_emits_moves_before_a_programs_equipment_and_routines`
(`render/manifest.rs:889`) before adding a section, and update the fixture in
the same step.

- [ ] **Step 5: Run the tests and confirm they pass**

- [ ] **Step 6: Gates and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "feat(ui): spend talent points on the Develop screen"
```

- [ ] **Step 7: STOP and play it**

```sh
cargo run -- --template stack
```

The three questions the spec leaves open are feel questions and this is the
only instrument for them:
1. Do `KERNEL_RING_MAX = 3` and `LEVELS_PER_RING = 2` give enough to chase?
2. Does a six-tier, two-choice ladder read as a *tree*, or as a list?
3. Is a developed companion now something you would be reluctant to replace?
   That is the whole point of the feature; if the answer is no, say so before
   Task 12 rather than after.

Report back before starting Task 12.

---

### Task 12: Measure, then document

**Files:**
- Modify: `crates/engine/src/balance_sim.rs` — only if a curve genuinely moved
- Create: `dev-arenas/developed-companion.ron`
- Modify: `docs/seams.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`
- Modify: `CHANGELOG.md`
- Create: `docs/measurements/<date>-companion-talent-power.md` if you run a
  sweep

- [ ] **Step 1: Run the balance gate and read what moved**

```sh
cargo test -p feral-processes-engine balance_sim
```

`balance_sim` fields a mid-grade party of three Scrappers, so extra companion
levels and any `Stat` talent move party strength and every zone-clearability
curve with it. **A curve that moves means progression changed — that is the
signal, not a broken test.** Read the direction and the magnitude, decide
whether it is the change you wanted, and only then update the hardcoded curves.

If a zone becomes unclearable, or clearable several levels earlier than before,
that is a tuning result to report — not something to paper over by editing the
expected numbers.

- [ ] **Step 2: Stage an arena scenario for what `balance_sim` cannot see**

`balance_sim` models **no abilities**, so `Ability`, `Affinity` and
`RoutineSlot` nodes are ungated by it entirely. `dev-arenas/README.md` is the
schema. Note the known trap: `equip` is top-level, not inside `Fresh(...)`, and
identical numbers across a sweep mean an ignored input rather than a dead
feature.

```sh
cargo run --bin arena -- dev-arenas/developed-companion.ron --out report.ron
```

Arena numbers compare **within one build only** — a moved baseline is a
reshuffled RNG stream, not a difficulty change. Compare deltas against a
same-build control, never absolutes against an older report.

- [ ] **Step 3: Check whether a developed program now sells for too much**

The spec's second open question. `Game::program_payout` pays a fraction of
`Stats::power()`, and a `Stat` talent raises it. `components::PurchasedTiers`
exists because buying Recompile Kernel tiers and selling the program printed
Credits — measured at zone 7, a zone-1 program bought up six tiers sold for 716
against 72 fragments' worth of kernels, and Credits are the one currency that
survives a breach.

Rings are boss drops rather than a renewable material chain, so this is **not**
the same printing press, and the spec's recommendation is to follow the
refactor precedent and *not* divide talents back out — five refactor slots are
at most a 1.28x on power and never repay the annealed cores they cost.

Measure it rather than assuming: sell a fully developed program and compare
against the rings and fights it took. If it clears its own cost, that is a hole
and it needs a `PurchasedTiers`-shaped receipt. Write the number down either
way — into `docs/measurements/` if you ran anything, per that directory's
`README.md`, whose bar is "something was run, the data is gone, and a decision
depends on it".

- [ ] **Step 4: Write the seam entries**

`docs/seams.md` gets the argument; `CLAUDE.md` gets the rule and the trap it
closes, in a line or two, under the same title. Four seams earned an entry:

1. **The ring buys room; fights buy the points.** `Game::companion_level_cap`
   is the one ceiling expression; the cronjob site deliberately does not use
   it, and the two arena sites take the absolute maximum instead.
2. **Talent points are derived, never stored.** Level minus the base cap, minus
   the list's length. A stored count can desync from both.
3. **A `Stat` talent bakes into `Stats` at purchase and load must not
   re-apply.** The `Talents` list is the receipt, exactly as `Refactors` is.
4. **Fusion keeps the survivor's ring and talents**, and `fuse_companions` is
   the door that silently drops a new component.

Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to
catch drift.

Add the spec to `docs/superpowers/INDEX.md`, which is the one-file answer to
"what shipped, and where is its argument". Do **not** edit the spec's own
`**Status:**` header — those are written at approval time and are stale for
fourteen specs in that directory already.

- [ ] **Step 5: Write the CHANGELOG entry**

A `## Unreleased`-style section is not the convention here — read
`CHANGELOG.md`'s preamble, which is the one statement of the versioning policy.
**Do not bump the workspace version and do not tag.** That happens once, at the
merge, so a rebase or squash cannot invalidate a version already tagged.

- [ ] **Step 6: Final gate and commit**

```bash
cargo fmt && cargo clippy --workspace && cargo test --workspace
git add -u && git commit -m "docs: record the companion progression seams"
```

- [ ] **Step 7: Whole-branch review**

The final whole-branch review is not optional, and it should be given the exact
seam rules above rather than a summary of them — reviewers check the rule you
give them. Give the diff as a **file**, never pasted into the prompt.
