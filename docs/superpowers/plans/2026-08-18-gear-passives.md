# Gear Passives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A worn item can grant a passive routine that fires in battle, through
the ability vocabulary that already exists as data.

**Architecture:** `ItemDef` gains `grants: Option<AbilityId>`.
`Game::ready_passives` gains a second source — the wearer's `Equipment` —
alongside the `Routines` it already reads, so a gear passive is **derived at
fire time and never stored**: unequipping ends it by omission. A third
`PassiveTrigger`, `RoundStart`, is added with its call site. No new effect
vocabulary, no new save fields, no save-format change.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-08-18-gear-passives-and-overclock-design.md`

Read the spec first. It carries the reasoning — in particular *why* the
passive is derived rather than written into `Routines`, and why feature C
(Overclock) is deliberately not in this plan. Its appendix is a record of a
decision, not work to do. **Do not implement anything from the appendix.**

## Global Constraints

- **No save-format change.** `SAVE_FORMAT_VERSION` must not move. `ItemDef`
  is asset data; `AbilityCooldowns` is battle-scoped and never persisted. If
  you find yourself adding a save field, stop — the design is being violated.
- **New `ItemDef`/`AbilityDef` fields are `#[serde(default)]`**, so existing
  mods keep parsing untouched.
- **A malformed `.ron` is skipped with a logged warning, never a panic** —
  follow the existing `load_dir` pattern.
- **Update `assets/items/README.md`** in the same commit as the schema change.
  It is the schema reference for modders.
- **Comment discipline:** comments explain *why*. The repo's doc comments
  carry arguments; match that register and do not restate what the code says.
- **A doc comment claiming to mirror other code must be a call, not a copy.**
- Every task ends green: `cargo fmt`, `cargo clippy --workspace` (fix
  warnings, don't silence), and the tests named in that task.
- **`balance_sim` passing is not evidence for any of this.** It models no
  abilities. Run it to catch collateral damage, not to validate the feature.
- Commit per green step. Do not push. Do not bump the version or write
  `CHANGELOG.md` — that happens once, at the merge.

## File Structure

| File | Responsibility |
|---|---|
| `crates/engine/src/items_db.rs` | `ItemDef::grants` field + its load-time refusal |
| `crates/engine/src/items.rs` | (read only) `ItemId`, `EquipmentSlot` — no change expected |
| `crates/engine/src/abilities.rs` | `PassiveTrigger::RoundStart` variant |
| `crates/engine/src/game/combat_round.rs` | `RoundStart`'s one call site, top of `battle_resolve_round` |
| `crates/engine/src/game/passives.rs` | `ready_passives` gains the `Equipment` source |
| `crates/engine/src/game/catalog.rs` | `Game::item_grant`, beside `item_description` |
| `crates/engine/src/tests/support.rs` | passive-battle fixtures, moved here from `exclusive_routines.rs` |
| `crates/engine/src/tests/gear_passives.rs` | **new** — every test in this plan except the asset ones |
| `crates/engine/src/tests/mod.rs` | registers the new test module |
| `crates/gui/src/render/inventory.rs` | one row on the item describe page |
| `assets/abilities/*.ron` | two new passive abilities |
| `assets/items/*.ron` | three new gear items |
| `assets/items/README.md` | documents `grants` |

---

### Task 1: `ItemDef::grants` and its load-time refusal

**Files:**
- Modify: `crates/engine/src/items_db.rs` (`ItemDef`, and the load path beside
  `non_finite_field`)
- Modify: `assets/items/README.md`
- Create: `crates/engine/src/tests/gear_passives.rs`
- Modify: `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Produces: `ItemDef::grants: Option<AbilityId>`, `#[serde(default)]`.
  Later tasks read it through `ItemDb`.

**The refusal, and why it is not in `ItemDef::non_finite_field`.** Validity
here needs the `AbilityDb`, which `ItemDef` cannot see. Skip the offending
**item** with a warning — an item naming an ability that cannot fire is an
authored thing that silently never runs, which is the failure
`passive_field_mismatch` already refuses for abilities.

There is an exact precedent for the shape: `SpeciesDb::load_dir(&dir,
&abilities)` and `ResearchDb::load_dir(&dir, &structures, &abilities)` take
the other database as a parameter for their own cross-db checks. Give
`ItemDb::load_dir` an `&AbilityDb` the same way. The load sequence in
`game/lifecycle.rs` already puts abilities first (`:1335`) and items after
(`:1355`), so nothing has to be reordered. Follow that precedent rather than
inventing a second load order.

Three refusal cases, one message each: the id names no ability; the ability
exists but `is_passive()` is false; the ability is field-only.

- [ ] **Step 1: Write the failing tests.** In the new `gear_passives.rs`, one
      test per refusal case plus one that a valid `grants` survives load.
      Build them from a temp asset dir the way the existing malformed-asset
      tests do — look at `tests/assets.rs` for the pattern before writing a
      new one. Assert on the *outcome* (the item is absent from `ItemDb`, or
      present for the valid case), not on log text.
- [ ] **Step 2: Run them and watch them fail** —
      `cargo test -p feral-processes-engine gear_passives`. Expect failure to
      compile on the missing field, which is a legitimate red.
- [ ] **Step 3: Add the field and the refusal.**
- [ ] **Step 4: Document `grants` in `assets/items/README.md`** — one entry
      matching the register of the fields already there, saying it must name
      an existing passive, non-field-only ability.
- [ ] **Step 5: Green.** `cargo test -p feral-processes-engine gear_passives`,
      then `cargo test -p feral-processes-engine assets`, then fmt + clippy.
- [ ] **Step 6: Commit.**

---

### Task 2: the `RoundStart` trigger

**Files:**
- Modify: `crates/engine/src/abilities.rs` (`PassiveTrigger`)
- Modify: `crates/engine/src/game/combat_round.rs` (`battle_resolve_round`)
- Modify: `crates/engine/src/tests/support.rs` (move fixtures in)
- Modify: `crates/engine/src/tests/exclusive_routines.rs` (import them)
- Modify: `crates/engine/src/tests/gear_passives.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `PassiveTrigger::RoundStart`. Task 4's content authors it.
- Produces (test-side): `support::passive_id`, `support::battle_with_a_passive_holder`,
  `support::resolve_a_planned_round`, `support::total_enemy_hp` — moved
  verbatim from `tests/exclusive_routines.rs:539-610`, visibility widened to
  `pub(crate)`, bodies unchanged. Engine test fixtures live in `support.rs`;
  two test modules needing them is exactly why.

**The call site.** Fires once at the top of `battle_resolve_round`
(`combat_round.rs:41`) for every **living party member**, before any action
resolves. Place it after the round header is logged (`combat_round.rs:70`) so
its narration reads under the round it belongs to, and before the plan is
read. Pass `self.living_party()` as the holders, the way `AllyDropped` does —
`RoundStart` is a fact about the round, not about one combatant.

Adding a variant means adding its call site in the same change; `passives.rs`'s
module doc states why. Do not add the variant without the call.

- [ ] **Step 1: Move the four fixtures to `support.rs`**, update
      `exclusive_routines.rs` to use them, and confirm nothing else moved:
      `cargo test -p feral-processes-engine exclusive_routines` must be green
      before you write a line of the feature. Commit this move on its own.
- [ ] **Step 2: Write the failing test.** A holder with a `RoundStart` passive
      installed as an ordinary routine lands its effect on round one, and an
      otherwise identical battle without the passive does not. **Both halves in
      one test** — the control is what stops this passing against a passive
      that never fires, and this repo has shipped two vacuous tests that
      lacked one. Author the test's passive as a fixture ability in the test
      assets, not as shipped content; Task 4 ships the real ones.
- [ ] **Step 3: Run it, watch it fail** for the right reason (no such variant).
- [ ] **Step 4: Add the variant and its call site.**
- [ ] **Step 5: Green** — the new test, `exclusive_routines`, `combat_abilities`,
      `assets`, then fmt + clippy.
- [ ] **Step 6: Commit.**

---

### Task 3: gear grants fire

This is the feature. Everything before it was scaffolding.

**Files:**
- Modify: `crates/engine/src/game/passives.rs` (`ready_passives`, ~line 90)
- Modify: `crates/engine/src/tests/gear_passives.rs`

**Interfaces:**
- Consumes: `ItemDef::grants` (Task 1), `PassiveTrigger::RoundStart` (Task 2),
  the moved `support` fixtures (Task 2).
- Produces: no new public API. `ready_passives` keeps its signature
  `fn ready_passives(&self, holder: Entity, trigger: PassiveTrigger) -> Vec<AbilityDef>`.

**The change.** `ready_passives` reads the holder's `Routines` today. It gains
a second source: the holder's `Equipment` — three slots, each resolved
`EquippedItem::copy.item` → `ItemDb` → `grants` → `AbilityDb::get` — filtered
by the same cooldown map and the same `triggers == Some(trigger)` predicate.
Installed routines first, gear after, so today's slot order is untouched.

**The stacking rule the code must produce:** *an ability fires once per source
per round; the cooldown is per id.*

- A gear grant and an installed routine of the same id **both** fire. Do not
  dedupe across the two sources.
- Two gear slots naming the same ability fire **once**. Dedupe within gear.
- Duplicate routines are impossible already (`install_disk`,
  `game/routines.rs:290`, refuses one). Write no guard for that.

The existing collect-up-front comment on `ready_passives` still applies and
still matters: firing one mutates the world under the borrow.

- [ ] **Step 1: Write the failing tests.** Six, all in `gear_passives.rs`:
      1. **Worn fires, stripped does not** — one test, both halves. The
         stripped half is what stops it passing with the hook deleted.
      2. **Cross-source double-fire** — a gear grant plus the same id
         installed as a routine lands the effect twice in one round, and the
         id carries exactly one cooldown entry afterwards.
      3. **Two slots, one fire** — the same ability granted by two worn items
         fires once.
      4. **A companion's gear passive fires.** Gear is wearable by any owned
         program (`Game::check_wearer`); this is the case most worth having.
      5. **Cooldown holds** — a gear passive that fired does not re-fire the
         next round.
      6. **Save then load** — equip, save, load, and the passive still fires,
         with no new field in the save. A RON round trip alone cannot catch
         this class of thing; the test must go through a real save→load.
      For 1-5, assert on an *observable effect* (enemy HP dropped, a status
      cleared), not on the return of `ready_passives` — a test asserting a
      function was called proves the plumbing and not the behaviour.
- [ ] **Step 2: Run them, watch every one fail.** Read each failure and check
      it fails because the gear source is missing, not because the fixture is
      short something. A fixture that never equipped anything reads as the
      feature not working.
- [ ] **Step 3: Implement the second source in `ready_passives`.**
- [ ] **Step 4: Green** — `gear_passives`, then `exclusive_routines`,
      `equipment`, `combat_abilities`, `wielded`, then fmt + clippy.
- [ ] **Step 5: Commit.**

---

### Task 4: the content

**Files:**
- Create: `assets/abilities/<weapon-passive>.ron`, `assets/abilities/<armor-passive>.ron`
- Create: three item files under `assets/items/`
- Modify: `crates/engine/src/tests/gear_passives.rs`

**Interfaces:**
- Consumes: `grants` (Task 1), `RoundStart` (Task 2), the firing (Task 3).
- Produces: three shipped items carrying `grants`. Nothing in Rust depends on
  their ids.

**Three new items, one per slot — not retrofits onto existing gear.** Adding
`grants` to an existing item changes what a copy already in a player's save is
worth, against a `value` priced for a stat line that no longer describes it.

- **Weapon** — grants a new `RoundStart` `Damage` passive, low power.
- **Armor** — grants a new `RoundStart` `Buff { kind: Def }` on the wearer.
- **Module** — grants **`watchdog`**, which already ships. No new file, and it
  is the case a modder hits most: naming an ability that already exists.

**Asset rules these must satisfy** — all are live tests in `tests/assets.rs`,
and three of them are not obvious:

- **Names end in the scope word** (`Single` / `Party` / `Group` / `Everyone`),
  per `every_shipped_ability_name_ends_in_the_scope_it_targets`.
- **A family runs contiguously from Single upward**
  (`every_battle_ability_family_is_contiguous_from_single_upward`), so author
  both new abilities at **Single** scope, each its own family. A lone
  Party-scope ability fails this.
- **Not `exclusive`.** That field claims to name a routine's *only* source —
  a boss or a trader — and gear is now a second one. This is also what puts
  the new abilities inside the contiguity test that `deadman` and `watchdog`
  sit outside.
- **A non-zero `cooldown`**, per
  `every_shipped_ability_but_decompile_and_field_routines_has_a_cooldown`.
  Model the numbers on `deadman`/`watchdog`, which carry 4 and say why in a
  comment.
- **`power_cost: 0.0`**, for the reason `deadman.ron` states: a cast cost is
  meaningless for something that costs no turn.
- **Every shipped item needs description text**
  (`every_shipped_item_and_structure_has_description_text`).
- **No two shipped abilities share a display name**
  (`no_two_shipped_abilities_share_a_display_name`) — check the directory
  before settling on one.
- **No occult naming in game content.** No daemon, demon, ghost, wraith or
  phantom; eight names were swept out for this. The vocabulary is
  computing-and-intrusion, and the files already there are the register to
  match.
- **Ship all three drop-only** — `droppable` and/or `cache_drop`, no
  `craftable`. Both gear-tier tests skip an item with no recipe, so this
  keeps the change out of the bench-versus-scavenged policy entirely. Give
  each a `value` above what its stat line alone would justify; it is carrying
  an effect as well.

- [ ] **Step 1: Write the failing test** — each of the three shipped items
      resolves its `grants` to a real, passive, non-field-only ability, read
      off the shipped assets rather than a fixture. This is the census that
      stops a later edit orphaning one.
- [ ] **Step 2: Run it, watch it fail.**
- [ ] **Step 3: Author the two ability files and the three item files.**
- [ ] **Step 4: Green** — `cargo test -p feral-processes-engine assets`
      (this is the real gate for this task), then `gear_passives`, then
      `cargo test -p feral-processes-engine balance_sim` to catch collateral
      damage only.
- [ ] **Step 5: Commit.**

---

### Task 5: a player can see what an item grants

**Files:**
- Modify: `crates/engine/src/game/catalog.rs` (beside `item_description:251`)
- Modify: `crates/gui/src/render/inventory.rs` (`draw_item_describe:262`)
- Modify: `crates/engine/src/tests/gear_passives.rs`

**Interfaces:**
- Consumes: `ItemDef::grants`.
- Produces: `pub fn item_grant(&self, id: &ItemId) -> Option<(&str, &str)>` —
  the granted ability's `name` and `description`, `None` when the item grants
  nothing or the id resolves to nothing. Modelled on `item_description`.

**Why the engine and not the renderer.** The item's authored `description` is
mod-controlled free text and cannot be trusted to stay in step with `grants`,
so the row is derived from the ability itself. Deriving it in the engine is
the standing rule for read-only screens — a per-row transform folded into gui
opens a screen on a row that is not drawn.

The renderer adds the ability's name and its description below the item's own
prose on the describe page, wrapped with the `wrap_text` /
`DESCRIBE_WRAP_COLUMNS` already in that function. Do not build the name in
the renderer.

- [ ] **Step 1: Write the failing test** — engine-side: `item_grant` returns
      the ability's authored name and description for one of Task 4's items,
      and `None` for an item that grants nothing.
- [ ] **Step 2: Run it, watch it fail.**
- [ ] **Step 3: Implement `Game::item_grant`.**
- [ ] **Step 4: Add the row in `draw_item_describe`.**
- [ ] **Step 5: Green** — `cargo test --workspace`. This is the task boundary
      where the full suite is the gate, not a spot check. Then fmt + clippy.
- [ ] **Step 6: Commit.**

---

## After the last task

The suite passing is not evidence the feature is any good, and this repo has
a standing record of features shipping green and unplayed. Two instruments,
both cheap:

1. **An arena scenario** in `dev-arenas/` with the gear equipped. `equip` is
   **top-level** in a scenario file, never inside `Fresh(...)` — an ignored
   input reads as a dead feature, and has here before.
2. **A session** — `FERAL_DEV_ARENA=1 cargo run`, main menu, `[R]` Arena. It
   is the only way a *companion's* passive is ever seen firing in an authored
   fight, and the companion case is the one this feature adds for free.

Report what the passives actually felt like at the table, and do not describe
either instrument as run unless it was.
