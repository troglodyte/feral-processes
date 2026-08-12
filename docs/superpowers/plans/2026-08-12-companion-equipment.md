# Companion Equipment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let any program the player owns wear the same three gear slots the
player wears, drawn from and returned to the same cargo.

**Architecture:** `Game::equip`/`unequip` gain a wearer `Entity` — the player
stops being implicit and becomes the entity app-core passes. Gear bonuses keep
being written straight into `Stats` (as they are today), so the correctness
work is making sure no stats *operation* ever runs while a bonus is sitting
there.

**Spec:** `docs/superpowers/specs/2026-08-12-companion-equipment-design.md` —
read it first. It records the settled decisions and the trap this shape exists
to avoid.

**Tech stack:** Rust, `bevy_ecs` 0.19 (engine only), bincode + RON saves.

## Global Constraints

- **TDD.** Failing test first, minimal implementation, green, commit. Every
  task ends green.
- **No code blocks in this plan are finished implementations.** They pin an
  ordering, a signature or a formula. Write the rest yourself in the
  surrounding style.
- **Read `CLAUDE.md` before starting.** The load-bearing seams it lists are
  the reason several steps below are ordered the way they are.
- Run `cargo fmt` and `cargo clippy --workspace` after each task; fix
  warnings rather than silencing them.
- Iterate with `cargo test -p feral-processes-engine <name>` (~3s). Save
  `cargo test --workspace` for task boundaries.
- **Do not push, tag, merge, or bump the workspace version.** Committing
  freely on the branch is expected; everything outward-facing needs an
  explicit ask. The version bump happens at the merge (Task 6), not before.
- Branch is `companion-equipment`, already created, spec already committed.

## File map

| File | Responsibility after this change |
|---|---|
| `crates/engine/src/game/crafting.rs` | `equip`/`unequip`/`fuse_item` — now wearer-parameterized. Gains `gear_bonus`, `strip_gear`. |
| `crates/engine/src/game/turn.rs:26` | `player_entity` becomes `pub`. |
| `crates/engine/src/components.rs:195` | `Equipment`'s "Player-only" doc stops being true. |
| `crates/engine/src/game/refactor.rs` | Lifts the gear bonus around `refactored()`. |
| `crates/engine/src/game/trade.rs` | `dissolve_tamed_program` strips; `sell_companion` strips before appraising. |
| `crates/engine/src/game/party.rs` | `fuse_companions` strips both parents before the `Stats` snapshot; `wield_program`'s `unequip` call gains the player. Gains `Game::worn`. |
| `crates/engine/src/save.rs` | `CreatureSave.equipment`; `SAVE_FORMAT_VERSION` 28. |
| `crates/engine/src/game/lifecycle.rs` | Save/load of a creature's `Equipment`. |
| `crates/app-core/src/lib.rs` | `Mode::CompanionEquip`, `pending_swap_target`, `equip_swap_rows(game, wearer, slot)`. |
| `crates/app-core/src/app/party.rs` | `E` on the roster; the companion slot page's key handler. |
| `crates/app-core/src/app/inventory.rs` | Passes `player_entity()`; `EquipSwap` Esc routing. |
| `crates/gui/src/render/party.rs` | `companion_help` gains `[E]quip`; draws the new screen. |
| `crates/gui/src/render/mod.rs:637` | Dispatches `Mode::CompanionEquip`; passes the target to `draw_equip_swap`. |

---

### Task 1: Wearer-parameterized equip/unequip

**Files:**
- Modify: `crates/engine/src/game/crafting.rs:271` (`equip`), `:335`
  (`unequip`)
- Modify: `crates/engine/src/game/turn.rs:26` — `pub(crate) fn player_entity`
  → `pub fn player_entity`
- Modify: `crates/engine/src/components.rs:195` — `Equipment`'s doc comment
- Modify call sites: `crates/engine/src/game/party.rs:434`
  (`wield_program`), `crates/engine/src/arena/setup.rs:47`,
  `crates/app-core/src/app/inventory.rs:105,106,198`
- Test: `crates/engine/src/tests/equipment.rs` (22 existing call sites need
  the new argument)

**Interfaces produced:**

```rust
pub fn player_entity(&self) -> Entity;
pub fn equip(&mut self, wearer: Entity, item: &ItemId, tier: u32) -> Result<(), String>;
pub fn unequip(&mut self, wearer: Entity, slot: EquipmentSlot) -> Result<(), String>;
```

**Design notes** (decided; do not re-litigate):

- Plain `Entity`, not a `Wearer` enum — it is the idiom `add_companion`,
  `refactor_companion`, `rename_companion`, `wield_program` and
  `sell_companion` already use. An `Entity` is inert outside `Game`; `world`
  stays private.
- The bodies barely change. `apply_equipment_delta` and
  `slot_occupant_with_mods` already take an entity; pass the wearer instead of
  the player. **`count_copies`/`take_copies`/`add_copies` keep resolving the
  player themselves** — that is the feature, not an oversight: gear comes from
  and returns to the player's cargo whoever wears it.
- New refusal, before anything moves: the wearer must be the player, or an
  entity with `Tamed { owner: player_entity() }`. Keep it above the first
  `take_copies` — the ordering rule `use_symlink` and `install_routine` follow.
- `Equipment` is inserted on demand (`entity_mut(wearer).insert_if_new(..)`,
  or an equivalent get-or-insert) rather than at every spawn site. Absence
  already reads as an empty loadout everywhere. Do **not** add
  `Equipment::default()` to `adopt_program`, decompile capture or
  `fuse_companions` — that is the third-copy trap `adopt_program`'s doc warns
  about.
- The log line branches on `wearer == player_entity()`: `You equip X` against
  `<label> equips X`, using the existing `entity_label`.
- `wield_program` (`party.rs:434`) passes the player. Its comment about the
  unequip coming last-that-can-fail still holds and stays.

**Steps:**

- [ ] **Step 1: Write the failing tests** in
  `crates/engine/src/tests/equipment.rs`. Five, each pinning one decision:
  1. A companion's ATK rises by the worn weapon's level-scaled bonus and
     returns to its original value on unequip. Use `spawn_tamed` from
     `tests/support.rs`.
  2. Gear on a companion leaves the player's `Stats` untouched, and gear on
     the player leaves the companion's untouched — assert both directions in
     one test.
  3. A copy equipped on the player, unequipped, then equipped on a companion
     produces the same bonus on the companion. This is the
     interchangeability the feature is named for.
  4. Equipping onto a wild creature (`spawn_wild_on_player_tile`) is refused
     with an error, and that creature's `Stats` are unchanged.
  5. A module whose only stat is `decompiler` equips onto a companion and
     changes none of its stats. This pins the settled "worn, bonus is dead"
     decision — companions have no `Decompiler` component. Use
     `trace_sniffer` or `handshake_forge` from the real assets.
- [ ] **Step 2: Run them and watch them fail** —
  `cargo test -p feral-processes-engine equipment`. They will fail to
  *compile* first (wrong arity). That counts: fix the arity on the new tests
  only, and confirm the five then fail on behaviour, not on types.
- [ ] **Step 3: Change the signatures and the bodies.** Make
  `player_entity` public, thread the wearer, add the ownership guard, insert
  `Equipment` on demand, branch the log line.
- [ ] **Step 4: Fix every call site.** The five listed above plus the 22 in
  `tests/equipment.rs` and the handful in `tests/wielded.rs` and
  `tests/inspection.rs`. In app-core, `game.equip(game.player_entity(), ..)`
  — note the borrow: read the entity into a local before the `&mut` call.
- [ ] **Step 5: Green** — `cargo test -p feral-processes-engine`, then
  `cargo test -p feral-processes-app-core`. Then `cargo fmt` and
  `cargo clippy --workspace`.
- [ ] **Step 6: Commit** — `feat(equipment): gear names the wearer it goes on`.

---

### Task 2: Keep the gear bonus out of every stats operation

**This is the correctness core of the feature.** Read the spec's "The trap
this design is shaped around" before writing anything.

**Files:**
- Modify: `crates/engine/src/game/crafting.rs` — add `gear_bonus`,
  `strip_gear`
- Modify: `crates/engine/src/game/refactor.rs:156-158`
- Modify: `crates/engine/src/game/trade.rs:412` (`dissolve_tamed_program`),
  `:449-454` (`sell_companion`)
- Modify: `crates/engine/src/game/party.rs:620-637` (`fuse_companions`'s
  `Stats` snapshot)
- Test: `crates/engine/src/tests/equipment.rs`,
  `crates/engine/src/tests/trade.rs`, `crates/engine/src/tests/party.rs`,
  and the combat teardown suite covering a companion's death

**Interfaces produced:**

```rust
pub(crate) fn gear_bonus(&self, wearer: Entity) -> items::EquipmentStats;
pub(crate) fn strip_gear(&mut self, wearer: Entity);
```

`gear_bonus` sums each worn slot's
`base.scaled_for_level(eq.level).fused_for_tier(eq.fusion_tier)`. It is the
single definition of "what is this entity's gear worth right now"; no site
below walks the slots itself. A slot whose item has dropped out of `ItemDb`
contributes nothing rather than erroring — `gear_bonus` is a read used inside
operations that must not fail halfway.

`strip_gear` unequips every worn slot into cargo. Idempotent on a bare entity.
It is *not* three `unequip` calls: `unequip` refuses during a battle and calls
`tick()`, and a companion dying mid-battle is precisely when this runs.

**The four sites:**

| Site | What to do |
|---|---|
| `refactor_companion` | `apply_equipment_delta(target, gear_bonus(target), -1)` before reading `Stats`, and `+1` after writing the result back. |
| `dissolve_tamed_program` | `strip_gear(creature)` at the top, before the `Party::retain`/`despawn`. Covers sale, extraction **and** battle death in one place. |
| `fuse_companions` | `strip_gear` on both parents **before** the `Stats` snapshot at `party.rs:620`. |
| `sell_companion` | `strip_gear(creature)` after the last refusal and before `program_payout`. |

Why `refactor_companion` lifts rather than strips: the program survives, so its
gear stays on. The recorded `EquippedItem` is untouched, so the add-back is
exact. `refactored()` multiplies (`*= ZoneLevel::tier_step(tier)`,
`raised(x, percent)`) — a bonus present during that call is scaled, and the
later unequip subtracts only the unscaled amount, welding the difference into
the program's base stats forever.

Why `sell_companion` strips explicitly even though `dissolve_tamed_program`
does: `program_payout` reads `Stats::power()` and runs *before* the dissolve.
Placed after the refusals so a refused sale leaves the loadout alone.

**Steps:**

- [ ] **Step 1: Write the failing tests.** Six:
  1. Refactoring a geared program raises its base stats only — then unequip
     and assert the program is back to exactly its pre-gear numbers. The
     second half is the one that catches the welded bonus; the first alone
     passes against the bug.
  2. Selling a geared program returns the gear to cargo **and** pays the same
     price as selling an identical bare program. Two programs, same species
     and level, one geared. (`tests/trade.rs`)
  3. Extracting a routine from a geared program returns its gear.
  4. Fusing two programs, one geared, returns that gear to cargo and produces
     a child whose stats equal a fusion of the same two programs bare.
     (`tests/party.rs`)
  5. A companion killed in battle returns its gear to cargo. Use
     `insert_battle` and `resolve_round_with` from `tests/support.rs`;
     remember `Game::apply_damage` is the only path that lowers HP.
  6. `strip_gear` on a program wearing nothing is a no-op.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement `gear_bonus` and `strip_gear`,** then wire the four
  sites.
- [ ] **Step 4: Green.**
- [ ] **Step 5: Mutation-check the two ordering-sensitive fixes.** A test that
  passes with the fix removed is not coverage:
  - Delete the lift-and-replace in `refactor_companion` → test 1 must fail.
    Restore it.
  - Move `strip_gear` in `fuse_companions` to *after* the `Stats` snapshot →
    test 4 must fail. Restore it.
  If either still passes, the test is vacuous — fix the test, not the fix.
  Record in the commit message that both mutations were run and what failed.
  Keep a copy of the file in the scratchpad before mutating; never
  `git checkout` to restore.
- [ ] **Step 6: Full engine suite** — `cargo test -p feral-processes-engine`.
- [ ] **Step 7: Commit** — `fix(equipment): no stats operation sees a gear
  bonus`.

---

### Task 3: Save format v28

**Files:**
- Modify: `crates/engine/src/save.rs:57` (`CreatureSave`), `:379`
  (`SAVE_FORMAT_VERSION`) and its migration-history doc comment
- Modify: `crates/engine/src/game/lifecycle.rs:436` (load) and `:724` (save)
- Test: `crates/engine/src/tests/save.rs`

**Interface produced:**

```rust
// CreatureSave
/// What this program is wearing — see `components::Equipment`. A Vec rather
/// than PlayerSave's nine flat fields for one reason that decides it: a
/// single defaulted field means an older RON dump packs with no hand-editing.
///
/// `#[serde(default)]` does nothing for the bincode save, which is positional
/// — that is why this bumped `SAVE_FORMAT_VERSION`. It is here for the
/// field-named RON `savetool dump`/`pack` round-trips through.
#[serde(default)]
pub equipment: Vec<(EquipmentSlot, EquippedItem)>,
```

Both types already derive `Serialize`/`Deserialize`. **Do not change
`PlayerSave`'s shape** — the migration in Task 6 depends on its existing keys
matching an older dump.

Add the `27 → 28` line to `SAVE_FORMAT_VERSION`'s history doc, in the style of
the entries above it: what gained what, and why it could not be compatible.

**Steps:**

- [ ] **Step 1: Write the failing tests.** Three:
  1. A geared companion survives save/load with its slots, `level` and
     `fusion_tier` intact, and its `Stats` still carry the bonus. Note the
     load path restores stats verbatim — assert the numbers, not just the
     slots.
  2. RON round trip: `to_ron` → `from_ron` on a save holding a geared
     companion is unchanged (extend the existing round-trip test's fixture).
  3. A v27-shaped RON string with no `equipment` key parses, and that
     creature loads wearing nothing. Write the RON inline in the test rather
     than reading a file, so the fixture cannot drift.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Add the field, bump the version, wire save and load.**
  Load must insert `Equipment` only when the Vec is non-empty, so an absent
  component keeps meaning "wears nothing" (Task 1's invariant).
- [ ] **Step 4: Green,** and check the existing
  `a_save_survives_a_round_trip_through_ron_unchanged` and the
  version-mismatch test still pass.
- [ ] **Step 5: Full engine suite.**
- [ ] **Step 6: Commit** — `feat(save): v28 carries a program's loadout`.

---

### Task 4: app-core — the companion slot page

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode::CompanionEquip`,
  `pending_swap_target`, `equip_swap_rows` signature
- Modify: `crates/app-core/src/app/party.rs` — `E` handling, new key handler
- Modify: `crates/app-core/src/app/inventory.rs:65-112` — pass the player,
  Esc routing
- Modify: `crates/app-core/src/app/input.rs:129` area — dispatch the new mode
- Modify: `crates/engine/src/game/party.rs` — add `Game::worn`
- Test: `crates/app-core/src/tests/party.rs`,
  `crates/app-core/src/tests/inventory.rs` (existing `equip_swap_rows` call
  sites)

**Interfaces produced:**

```rust
// engine
pub fn worn(&self, wearer: Entity, slot: EquipmentSlot) -> Option<EquippedItem>;

// app-core
pub fn equip_swap_rows(game: &Game, wearer: Entity, slot: EquipmentSlot) -> Vec<SwapRow>;
// App
pub pending_swap_target: Option<Entity>,   // None = the player
```

`equip_swap_rows` currently reads the worn copy by matching on
`player_status()`'s three fields (`lib.rs:242-246`). Replace that match with
`game.worn(wearer, slot)`. Everything else in it stays: `status.inventory` is
the shared cargo and `status.zone` is the current zone, and both are right for
either wearer. This is what keeps the two-levels asymmetry `CLAUDE.md`
records — worn copy at *its* recorded level, candidates at the current zone's
— correct for the companion too.

`pending_swap_target: None` meaning the player keeps the inventory screen's
existing flow untouched; the roster sets it to the program.

**Behaviour:**

- `E` on `Mode::Companion` opens `Mode::CompanionEquip` for the highlighted
  program. Handle it **before** `selected_index`, the way `W` and `N` are, and
  keep it uppercase so it can never collide with `menu_shortcut`'s
  digits-then-lowercase scheme.
- `Mode::CompanionEquip` lists that program's three slots and opens
  `Mode::EquipSwap` with `pending_swap_target` set. Esc backs to
  `Mode::Companion`.
- Esc from `Mode::EquipSwap` returns to `Mode::CompanionEquip` when a target
  is set, `Mode::Inventory` when it is not. Clear `pending_swap_target` on
  every exit from the picker, including the commit path.
- A slot with nothing in cargo that fits it gets the same
  "Nothing in cargo fits…" status line the player's picker gives, worded for
  the program.

**Steps:**

- [ ] **Step 1: Write the failing tests** in
  `crates/app-core/src/tests/party.rs`. Five:
  1. `E` on the roster opens `CompanionEquip` for the highlighted program;
     Esc backs out to `Companion`.
  2. Picking a slot opens `EquipSwap` with `pending_swap_target` set to that
     program; Esc from there returns to `CompanionEquip`, **not** `Inventory`.
  3. Choosing a row equips onto the companion, not the player — assert on
     the companion's stats and on the player's being unchanged.
  4. The picker's rows are measured against the *companion's* worn copy: put
     different copies of one item on the player and the companion and assert
     the two row sets differ.
  5. A slot with nothing to fit it sets the status line and does not open an
     empty picker.
  Remember `app-core battles are always 1 group, 1 slot` — do not try to test
  multi-member battle behaviour here.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Implement.** Add `Game::worn` first (engine), then the
  app-core changes. Update the seven existing `equip_swap_rows` call sites in
  `tests/inventory.rs` and the one in `render/inventory.rs:195` — the gui
  needs the argument to compile even before Task 5 draws anything new.
- [ ] **Step 4: Green** — `cargo test -p feral-processes-app-core`, then
  `cargo build -p feral-processes-gui`.
- [ ] **Step 5: Commit** — `feat(ui): the roster equips the program it lists`.

---

### Task 5: gui — draw the slot page

**Files:**
- Modify: `crates/gui/src/render/party.rs:13` (`companion_help`),
  `:25` (`draw_companion_menu`)
- Create (in `render/party.rs`): the `Mode::CompanionEquip` screen
- Modify: `crates/gui/src/render/mod.rs:636-648` — dispatch, and pass
  `app.pending_swap_target` to `draw_equip_swap`
- Test: the existing gui test holding `companion_help`

**Notes:**

- `companion_help` returns `[String; 3]` and becomes `[String; 4]`. Check
  `draw_companion_menu`'s row budget before assuming a fourth line fits — a
  drifted layout fixture has hidden a live overflow in this repo before.
- The new screen draws the same three slot rows the inventory leads with,
  through the same `stat_summary` formatter. Do not write a second formatter.
- Header carries the program's name and one line: decompiler bonuses have no
  effect on programs.
- **`companion_help` must still never name `W`.** The wielded program is a
  deliberate easter egg and a gui test holds the text to it. Add `[E]quip`;
  do not "finish the list".
- `render/` must not name a graphics library — draw through `Painter`.

**Steps:**

- [ ] **Step 1: Update the `companion_help` test** to expect `[E]quip`, and
  keep its assertion that `W` is never named. Run it; watch it fail.
- [ ] **Step 2: Add the help line and the screen; dispatch the mode.**
- [ ] **Step 3: Green** — `cargo test -p feral-processes-gui`.
- [ ] **Step 4: Play it.** `FERAL_DEV_ARENA=1` is not the path here; run
  `cargo run -- --template extraction`, which starts with a party. Equip a
  weapon onto a companion, check the stat moves on the roster, fight
  something, then unequip. A green suite is not evidence of play.
- [ ] **Step 5: Commit** — `feat(ui): a program's three slots have a screen`.

---

### Task 6: Docs, save migration, release

**Files:**
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md` (gitignored twins, no
  tracking to catch drift)
- Modify: `CHANGELOG.md`, root `Cargo.toml`
- Check: `assets/items/README.md`
- Migrate: `saves/save_1786492847.bin`

**Steps:**

- [ ] **Step 1: Migrate the save.** The v27 RON dump was taken before any
  format change and is in the session scratchpad as
  `save_1786492847.v27.ron`. Pack it with the new build:

  ```sh
  cargo run --bin savetool -- pack <scratchpad>/save_1786492847.v27.ron \
      saves/save_1786492847.bin
  ```

  Then **verify it loads** — `cargo run` and load the save, or
  `savetool dump` it back and confirm v28. If the scratchpad dump is missing,
  stop and ask: the `.bin` is unreadable by the new build and there is no
  second chance at it.
- [ ] **Step 2: Confirm the three `dev-saves/*.ron` templates still load** —
  `cargo run --bin savetool -- template` to list, then
  `cargo run -- --template extraction`. They are field-named RON, so the
  defaulted key should cost them nothing; verify rather than assume.
- [ ] **Step 3: Update `CLAUDE.md`.** Two edits:
  - The "Destroying a tamed program has two paths" seam now has a third
    thing both paths must do.
  - A new seam for the rule in Task 2: no stats operation may run while a
    gear bonus sits in `Stats`, naming all four sites and why each takes the
    shape it does. Write it in the register of the entries around it —
    what breaks, not what the code does.
  Then `cp CLAUDE.md AGENTS.md`.
- [ ] **Step 4: Check `assets/items/README.md`** for any claim about who can
  wear gear, and fix it if it says the player. No schema change, so no
  further doc obligation. `docs/manual.md` and the root `README.md` are
  carved out and stay stale.
- [ ] **Step 5: Full gate** — `cargo test --workspace`,
  `cargo clippy --workspace`, `cargo fmt --check`.
- [ ] **Step 6: Version and changelog.** Save-format bump takes the minor:
  0.7.7 → **0.8.0** in the root `Cargo.toml`, and a `## 0.8.0` section in
  `CHANGELOG.md` written for a player — it must say saves from 0.7.x will not
  load. Commit.
- [ ] **Step 7: Stop.** Do not push, tag or merge. Report what was run and
  what it printed, and ask.

---

## Self-review

**Spec coverage.** Settled decisions: any owned program → Task 1's guard;
gear returns on destruction → Task 2's three destruction sites; decompiler
worn-but-dead → Task 1 test 5 and Task 5's header line; roster entry point →
Tasks 4 and 5. Trap: Task 2 in full. Design §1 → Task 1; §2 → Task 2; §3 →
Tasks 4 and 5; §4 → Task 3; §5 → Task 6. Every test named in the spec appears
in a task.

**Types.** `player_entity`, `equip`, `unequip`, `gear_bonus`, `strip_gear`,
`worn`, `equip_swap_rows`, `pending_swap_target` are each defined once and
used consistently. `equip_swap_rows`'s new first argument lands in Task 4,
which is also where every existing caller is updated — Tasks 1-3 do not touch
it.

**Known gap, deliberate:** arena scenarios cannot equip companions
(`arena/setup.rs` keeps equipping the player only). Out of scope per the spec.
