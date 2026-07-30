# Field Routines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Abilities that arm a long-lived buff outside combat, which keeps running once a fight starts — plus the two bugs that currently make any such buff impossible.

**Architecture:** A new `FieldBuff` component alongside the untouched `CombatBuff`, holding a `Vec` of tick-durationed buffs that `clear_battle_status_effects` never reaches and the save does persist. A new `AbilityEffect::FieldBuff` variant is the marker for a field-only ability; scope lives on `FieldBuffKind` rather than on a new `AbilityTarget`.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine, standalone), bincode saves, RON assets, egui/Bevy renderer.

**Spec:** `docs/superpowers/specs/2026-07-30-field-routines-design.md` — read it before Task 1. It carries the *why* for every decision below and the verified file:line citations.

## Global Constraints

- Read `CLAUDE.md` first. It overrides anything here.
- **TDD, always.** Failing test first, minimal implementation, green, commit. Every task.
- **Content is data, difficulty is code.** Ability magnitudes and durations are authored in `.ron`. Global knobs go in `crates/engine/src/tuning.rs` as documented `pub const`s. Never hardcode a tuning number in a formula.
- **A malformed `.ron` is skipped with a logged warning, never a panic.** Follow `AbilityDb::load_dir`.
- **New `AbilityDef`/`ItemDef` fields get `#[serde(default)]`** so existing mods keep parsing.
- **`FieldBuffKind` and `BuffSource` variant order is save format** — bincode encodes enums positionally. Append, never reorder.
- **`SAVE_FORMAT_VERSION` → 15**, once, in Task 3. Don't bump it twice.
- **The renderer never touches the ECS `World`.** `Game` is the whole API. `crates/gui/src/paint.rs` is the only file allowed to name a graphics library; `render/` draws through `Painter`.
- **A read-only screen's rows are built in the engine**, because app-core derives the row count and gui draws the rows. See `Game::message_history` for the precedent.
- **Doc comments may not claim to "mirror" or "match" another module's formula.** Extract a shared function and call it. This has bitten this repo four times.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.
- Per-task gate: `cargo test -p feral-processes-engine <relevant>`. Final gate: `cargo test --workspace`.
- Commit at every green step. Branch is `feat/field-routines`; do not push.

---

### Task 1: `FieldBuff` component and insert policy

**Files:**
- Modify: `crates/engine/src/components.rs` — add `FieldBuffKind`, `FieldScope`, `BuffSource`, `ActiveFieldBuff`, `FieldBuff`
- Modify: `crates/engine/src/game/combat_status.rs` — add `arm_field_buff`
- Modify: `crates/engine/src/game/lifecycle.rs:91,238` — spawn the player holding `FieldBuff::default()`
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces produced:**

```rust
pub enum FieldBuffKind { Regen, Coolant, Trickle, Def, Atk, Mitigation, CaptureBoost, XpBoost, EncounterDamp, DropBoost }
pub enum FieldScope { Creature, Run }
pub enum BuffSource { Consumable, Routine }

// Serialize/Deserialize too — Task 3 stores this type directly in the save
// rather than a parallel tuple that could drift from it.
pub struct ActiveFieldBuff {
    pub kind: FieldBuffKind,
    /// Display name of the ability or item that armed it, captured at cast.
    /// Stored rather than derived from `kind`: two different routines can
    /// arm the same kind, and the buff list has to tell them apart.
    pub name: String,
    pub power: i32,
    pub remaining: u32,
    pub source: BuffSource,
}

#[derive(Component, Default)]
pub struct FieldBuff { pub active: Vec<ActiveFieldBuff> }

impl FieldBuffKind {
    pub fn scope(self) -> FieldScope;
    pub fn affinity_kind(self) -> Option<crate::abilities::AffinityKind>;
    pub fn magnitude_label(self, power: i32) -> String;       // "DEF+2", "HP+1/t", "XP+15%"
}

impl Game {
    pub(crate) fn arm_field_buff(&mut self, entity: Entity, buff: ActiveFieldBuff);
    pub(crate) fn field_buff_power(&self, entity: Entity, kind: FieldBuffKind) -> i32;  // 0 when absent
}
```

Scope and affinity assignments are in the spec's routine table — copy them from there, don't invent. `Regen`/`Def`/`Atk`/`Mitigation` are `Creature`; the other six are `Run`. Only the five flat kinds carry an affinity (`Heal` for the three over-time, `Buff` for `Def`/`Atk`/`Mitigation`); the four rate kinds return `None`, same call as `Cleanse`.

`arm_field_buff` is the **only** writer, and enforces both rules on one `Vec`:

```
Consumable  → retain(|b| b.source != Consumable), then push   // one entry total, any kind
Routine     → retain(|b| !(b.source == Routine && b.kind == kind)), then push
```

`FieldBuff` is inserted on demand for companions the way `arm_buff` does it — only the player is spawned holding one.

**Steps:**

- [ ] Write failing tests: a second `Consumable` displaces the first even of a different kind; a second `Routine` of the same kind displaces only that kind; a `Routine` of a different kind coexists; an item buff and a routine buff coexist; `field_buff_power` returns 0 for an absent kind and the stored power for a present one.
- [ ] Run them, confirm they fail to compile / fail.
- [ ] Implement the types and `arm_field_buff`.
- [ ] `cargo test -p feral-processes-engine combat_status` — green.
- [ ] `cargo fmt && cargo clippy --workspace`, commit.

---

### Task 2: Aging in `tick_inner`, and the rest contrast

**Files:**
- Modify: `crates/engine/src/game/turn.rs:91` — call a new `tick_field_buffs` from `tick_inner`
- Test: `crates/engine/src/tests/turn.rs`

**Interfaces consumed:** Task 1's `FieldBuff`, `arm_field_buff`.

**Interfaces produced:** `Game::tick_field_buffs(&mut self)` — decrements every `remaining` on the player and every party member, drops entries reaching zero, and logs one line per expiry naming the buff from `ActiveFieldBuff::name`.

**The ordering constraint is the whole point of this task.** `tick_field_buffs` goes in `tick_inner` **outside** the `if age_temporary` guard, one line from `age_temporary_structures` which is inside it. A `Temporary` structure does not decay during `rest`; a field buff does. Leave a comment saying so, because the next reader will otherwise assume the two neighbours match.

Collect expired entities/kinds in a first pass and log in a second — `self.log` takes `&mut self` and cannot run inside a query borrow.

**Steps:**

- [ ] Write failing tests: a 5-tick buff is gone after 5 ticks and present after 4; expiry logs; **one test that arms a buff and deploys a `Temporary` structure, runs `rest`, and asserts the buff lost `REST_TICKS` while the structure lost none** — this pair is the regression guard for the ungated placement; buffs on a companion tick too.
- [ ] Run, confirm fail.
- [ ] Implement.
- [ ] `cargo test -p feral-processes-engine turn` — green. Commit.

---

### Task 3: Save format v15

**Files:**
- Modify: `crates/engine/src/save.rs:13` (`PlayerSave`), `CompanionSave`, `save.rs:211` (`SAVE_FORMAT_VERSION` → 15)
- Modify: wherever `PlayerSave`/`CompanionSave` are built and applied (`game/lifecycle.rs`)
- Test: `crates/engine/src/tests/turn.rs` or a save-focused module — follow whatever already tests round-trips

**Interfaces produced:** `field_buffs: Vec<ActiveFieldBuff>` on both save structs.

Store the component's own type rather than a parallel tuple — a tuple is a second shape to keep in sync, and the copy that drifts is the one nobody runs. `ActiveFieldBuff`, `FieldBuffKind` and `BuffSource` all need `Serialize, Deserialize`.

**Steps:**

Two behaviours the spec records as deliberately needing **no** wiring. Both get
a test here anyway, because "no wiring needed" is exactly the claim that rots:

- Field buffs are player state, not zone-local. They **survive a breach** —
  `enter_next_zone` must not clear them. This is the inverse of the
  `BuybackLedger` trap, where anything zone-local must be wiped by name.
- A companion that is sold, extracted, fused away or killed takes its
  `FieldBuff` with it when the entity despawns. Neither
  `dissolve_tamed_program` nor `fuse_companions` needs a hook.

**Steps:**

- [ ] Write failing tests: a buff armed on the player survives a save/load round-trip with kind, power, remaining and source intact; the same for a buff on a companion; a v14 file is refused with the existing incompatible-version error.
- [ ] Write two more: a buff survives `enter_next_zone`; selling a buffed companion leaves no orphaned buff behind (`world.get::<Stats>(e).is_none()` is the idiom for "gone").
- [ ] Run, confirm fail.
- [ ] Implement, bumping the version exactly once.
- [ ] Green. Commit.

---

### Task 4: Move the item buff onto `FieldBuff` — both bugs die here

**Files:**
- Modify: `crates/engine/src/items_db.rs:46` — `PrebattleBuff` gains a `kind: FieldBuffKind`, and `rounds` becomes `ticks`
- Modify: `crates/engine/src/game/turn.rs:342` — `use_item` calls `arm_field_buff` with `BuffSource::Consumable`
- Modify: `crates/engine/src/game/combat.rs:140-143` — the carve-out comment in `start_battle` is now obsolete; the buff is no longer in the component that gets cleared. Delete it and say where the guarantee lives now.
- Modify: `assets/items/README.md`
- Test: `crates/engine/src/tests/turn.rs:345` — the existing `a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion` is the characterisation test; extend rather than replace.

**These are the two spec bugs.** Write both reproducers before touching implementation:

1. A buff armed on the map is **still running after a battle ends** (currently wiped by `clear_battle_status_effects`).
2. A buff armed on the map **survives a save/load** (currently absent from `PlayerSave`).

`PrebattleBuff::kind` is a widening from `BuffKind` to `FieldBuffKind` — an existing `.ron` naming `Atk`/`Def` still parses because those names exist in both enums, but no shipped item declares one, so there is nothing to migrate.

**Steps:**

- [ ] Write the two reproducers. Run them. Confirm both fail for the stated reason, not a compile error in the test itself.
- [ ] Implement the retarget.
- [ ] Both green, and `a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion` still green.
- [ ] Update `assets/items/README.md` for `kind` and `ticks`.
- [ ] `cargo test -p feral-processes-engine` — whole engine suite, since this touches a shared path. Commit.

---

### Task 5: `AbilityEffect::FieldBuff` and loader validation

**Files:**
- Modify: `crates/engine/src/abilities.rs` — the variant, `affinity_kind` arm, `load_dir` validation
- Modify: `crates/engine/src/game/combat_round.rs:770` — `unreachable!` arm next to `Decompile`'s
- Modify: `crates/engine/src/game/combat.rs:755` (`battle_special_options`) — filter field-only abilities out of the in-battle picker
- Modify: `crates/engine/src/game/combat_status.rs:148` — the hostile-side fallback already excludes `Decompile` when picking an ability to retaliate with; exclude `FieldBuff` the same way, or a wild carrier will try to cast one
- Modify: `assets/abilities/README.md`
- Test: `crates/engine/src/tests/combat_abilities.rs`, `crates/engine/src/tests/assets.rs`

**Interfaces produced:**

```rust
AbilityEffect::FieldBuff { kind: FieldBuffKind, power: i32, duration: u32, power_cost: f32 }
```

The variant **is** the field-only marker — there is no `field_cast: bool` on `AbilityDef`. Append it to the enum; `AbilityEffect` is authored in `.ron` by name, so order is not save format here, but keep it last for readability.

`AbilityDb::load_dir` gains two checks, both logged-warning-and-skip:

- A `Run`-scoped kind authoring anything but `AbilityTarget::WholeParty` is rejected. (`Creature`-scoped may author `OneAlly` or `WholeParty`; enemy targets are rejected for any field ability.)
- `power_cost` joins the `non_finite_field` check (`abilities.rs:317`) — RON accepts bare `NaN`.

`AbilityDef::cooldown` and `fatigue_cost` are dead on a field ability. Not an error; log a warning naming the file so a modder learns the value does nothing.

**Steps:**

- [ ] Write failing tests: `load_dir` skips a Run-scoped field ability targeting `OneAlly` and keeps loading the rest of the directory; skips one with `NaN` `power_cost`; accepts a Creature-scoped one targeting `OneAlly`; a field-only ability does not appear in `battle_special_options`; a wild program carrying one does not pick it to retaliate with.
- [ ] Run, confirm fail.
- [ ] Implement.
- [ ] Green. Update `assets/abilities/README.md` with the variant, every kind, and the scope-to-target rule. Commit.

---

### Task 6: `Game::cast_field_routine`

**Files:**
- Create: `crates/engine/src/game/field.rs`, wired into `crates/engine/src/game/mod.rs`
- Test: `crates/engine/src/tests/` — new `field.rs`, registered in `tests/mod.rs`

**Interfaces consumed:** Task 1's `arm_field_buff`, Task 5's variant, `Game::routine_holders` (`game/routines.rs:57`), `abilities::scaled_power`, `Game::ability_affinity`.

**Interfaces produced:**

```rust
pub struct FieldRoutineView {
    pub ability: AbilityId,
    pub name: String,
    pub description: String,
    pub holder: Entity,
    pub holder_label: String,   // "You" or the program's name
    pub power_cost: f32,
    pub affordable: bool,
    pub needs_ally_target: bool,
}

impl Game {
    pub fn field_routines(&mut self) -> Vec<FieldRoutineView>;
    pub fn cast_field_routine(&mut self, index: usize, target: Option<Entity>) -> Result<(), String>;
}
```

`field_routines` walks the player and every party member's installed slots, keeping only `AbilityEffect::FieldBuff` abilities. Flat list; `holder_label` is what the UI shows.

`cast_field_routine`:

- **The holder is the caster.** Magnitude is `scaled_power(power, holder_level, holder_affinity_for(kind))`. Store the scaled value, so a later level-up does not retroactively change a running buff.
- Charges `power_cost` against the player's `Needs::hunger`. **Validate before mutating** — insufficient Power returns `Err` and spends nothing.
- Refused during a battle and after game over. **Not** gated on `require_surface` — it touches no zone-map state, so it works underground.
- `Run`-scoped kinds ignore `target` and land on the player. `Creature`-scoped honour the ability's `AbilityTarget`: `OneAlly` requires `target`, `WholeParty` arms every living member.

**Steps:**

- [ ] Write failing tests: casting arms the buff and deducts Power; insufficient Power returns `Err` and leaves Power and buffs untouched; casting during a battle is refused; casting underground succeeds; a routine held by a level-20 companion produces a larger magnitude than the same routine on a level-1 one; a `Run`-scoped kind lands on the player even when cast off a companion; `WholeParty` arms every living member; `field_routines` lists routines from both the player and party members and excludes non-field abilities.
- [ ] Run, confirm fail.
- [ ] Implement.
- [ ] Green. Commit.

---

### Tasks 7–10 are independent of each other and may be done in any order or in parallel. Each consumes only Task 1's `field_buff_power`.

### Task 7: Stat hooks — `Atk` and `Def`

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs:786` (`effective_atk`), `:811` (`effective_def`)
- Test: `crates/engine/src/tests/combat_targeting.rs`

Both currently read `CombatBuff` alone. Add the `FieldBuff` term so the two sources sum. Do not restructure the existing `CombatBuff` read — `is_defending` depends on its exact shape.

**Steps:**

- [ ] Write failing tests: a `Def` field buff raises `effective_def`; it **stacks with** a `CombatBuff` brace rather than replacing it; `is_defending` is still false for an entity carrying only a field `Def` buff of `DEFEND_DEF_BONUS` power (this is the landmine the spec names — it must not read as bracing).
- [ ] Run, confirm fail. Implement. Green. Commit.

---

### Task 8: Over-time hooks — `Regen`, `Coolant`, `Trickle`

**Files:**
- Modify: `crates/engine/src/game/turn.rs` — extend Task 2's `tick_field_buffs` to apply, not only age
- Test: `crates/engine/src/tests/turn.rs`

- `Regen` heals Integrity on whoever carries it, capped at `max_hp`. **This is a heal, not damage**, so it does not go through `apply_damage`.
- `Coolant` restores `Needs::fatigue`, `Trickle` restores `Needs::hunger`, both capped at `NEED_MAX`, both player-only (`Needs` is `With<Player>`).

Apply before decrementing, so a 1-tick buff still does its last tick of work.

**Steps:**

- [ ] Write failing tests: `Regen` heals per tick and does not exceed `max_hp`; `Regen` on a companion heals the companion; `Coolant` and `Trickle` raise their needs and clamp at `NEED_MAX`; a 1-tick buff applies once before expiring.
- [ ] Run, confirm fail. Implement. Green. Commit.

---

### Task 9: Mitigation hook

**Files:**
- Modify: `crates/engine/src/game/combat_status.rs:316` (`apply_damage`)
- Test: `crates/engine/src/tests/combat_status.rs`

`apply_damage` is the only path that lowers HP, which is why the percentage cut belongs here and not at call sites. `power` is percentage points. Round once. Damage must not go below 1 from mitigation alone — a chip hit stays a hit.

**Steps:**

- [ ] Write failing tests: a 25% `Mitigation` buff reduces a 20-point hit to 15; mitigation never reduces damage below 1; an entity with no buff takes full damage.
- [ ] Run, confirm fail. Implement. Green. Commit.

---

### Task 10: Rate hooks — `CaptureBoost`, `XpBoost`, `EncounterDamp`, `DropBoost`

**Files:**
- Modify: `crates/engine/src/taming.rs:33` (`capture_chance`), `crates/engine/src/progression.rs:55` (`add_xp`), `crates/engine/src/game/spawning.rs:357` (`maybe_spawn_wild_creature`), `crates/engine/src/game/combat_rewards.rs:18` (`equipment_drops_for`)
- Test: `crates/engine/src/tests/taming.rs`, `perks.rs` or `progression.rs`'s inline tests, `spawning.rs`, `combat_rewards.rs`

`capture_chance` and `add_xp` are pure module-level functions. **Keep them pure** — add the buff term as a parameter the caller passes in, so both stay testable without a `Game`. Do not reach into the world from inside them.

All four are percentage points. All four are `Run`-scoped, so they read the *player's* `FieldBuff` regardless of who cast.

`maybe_spawn_wild_creature` is seeded-RNG territory: assert on the computed spawn chance, not on whether a spawn happened, or the test will be flaky.

**Steps:**

- [ ] Write failing tests, one per hook: the buffed value differs from the unbuffed one in the right direction by the right amount. Four tests minimum.
- [ ] Run, confirm fail. Implement. Green.
- [ ] `cargo test -p feral-processes-engine balance_sim` — `XpBoost` touches the XP curve; a moved curve is a real signal, not a broken test. Investigate before adjusting anything.
- [ ] Commit.

---

### Task 11: `Game::active_buffs` view

**Files:**
- Modify: `crates/engine/src/views.rs`
- Test: `crates/engine/src/tests/` — the field test module from Task 6

**Interfaces produced:**

```rust
pub struct ActiveBuffView {
    pub name: String,          // ActiveFieldBuff::name, or the stat for a CombatBuff
    pub magnitude: String,     // FieldBuffKind::magnitude_label of the SCALED power
    pub remaining: u32,
    pub holder_label: Option<String>,  // Some(name) when on a companion, None for the player
}

impl Game { pub fn active_buffs(&mut self) -> Vec<ActiveBuffView>; }
```

**One accessor, both screens.** It reads `FieldBuff` *and* `CombatBuff`, so in battle the list also shows a running Rally or brace; on the map `CombatBuff` is empty and the list is field buffs only. No branching, no second accessor. Item-armed buffs appear here too — same component.

The magnitude string is built in the engine because it is the *scaled* value, which gui cannot compute and must not try.

**Steps:**

- [ ] Write failing tests: a player buff has `holder_label: None`; a companion buff has `Some(name)`; a `CombatBuff` brace appears during a battle and nothing appears on the map; magnitude reflects the scaled power, not the authored one; an empty list when nothing is running.
- [ ] Run, confirm fail. Implement. Green. Commit.

---

### Task 12: The ten routine assets

**Files:**
- Create: ten files in `assets/abilities/` — `repair_loop.ron`, `coolant_flush.ron`, `trickle_charge.ron`, `hardened_shell.ron`, `overclock.ron`, `ablative_layer.ron`, `deep_scan.ron`, `trace_analysis.ron`, `ghost_protocol.ron`, `salvage_routine.ron`
- Test: `crates/engine/src/tests/assets.rs`

Kinds, scopes and effects are the spec's routine table. Durations in the 60–150 tick range, sized against `REST_TICKS` (40) so a rest is a real bite and not a wipe. `wild_weight: 0` on all ten — these are not found on wild programs. Author a real `description`; it is what the picker shows, and per repo preference descriptions live in the `.ron`, not derived in code.

How the player *obtains* these is out of scope: they load, they are installable, and they can be granted by a species or research file later.

**Steps:**

- [ ] Write a failing test asserting all ten load, and that every `FieldBuffKind` variant is exercised by at least one shipped ability (the mirror of the coverage the spec notes abilities already have).
- [ ] Run, confirm fail. Author the ten files. Green.
- [ ] `cargo test -p feral-processes-engine balance_sim` — new assets, so re-run the gate. Commit.

---

### Task 13: app-core — the cast flow

**Files:**
- Modify: `crates/app-core/src/lib.rs:201` — `Mode::FieldCast`, `Mode::FieldCastAlly`
- Modify: `crates/app-core/src/app/playing.rs` — bind `a` (verified free: `b c d e f g i m p q r s t u v w x B G L M R T U ? + - . < >` are taken)
- Modify: `crates/app-core/src/app/input.rs:120` — dispatch
- Create: `crates/app-core/src/app/field.rs`, wired into `app/mod.rs`
- Test: `crates/app-core/src/tests/` — new `field.rs`, registered in `tests/mod.rs`

Follow `Mode::BattleSpecial` → `Mode::BattleAlly` as the precedent: the routine and its target are separate choices, so they are separate modes. `FieldCastAlly` is entered only when `FieldRoutineView::needs_ally_target` is set.

Row counts come from `Game::field_routines`, not from app-core's own arithmetic.

**Steps:**

- [ ] Write failing tests: `a` from `Mode::Playing` opens `Mode::FieldCast`; picking a `WholeParty` routine casts and returns to `Playing`; picking an `OneAlly` routine opens `Mode::FieldCastAlly` and casting from there returns to `Playing`; Escape backs out at each step without casting; an unaffordable routine is refused with a message and does not change mode.
- [ ] Run, confirm fail. Implement. Green.
- [ ] `cargo test -p feral-processes-app-core`. Commit.

---

### Task 14: gui — the cast screen and the active-buff panels

**Files:**
- Create: `crates/gui/src/render/field.rs`, wired into `render/mod.rs`
- Modify: `crates/gui/src/render/base.rs` (map-screen panel), `crates/gui/src/render/battle.rs` (battle panel)
- Modify: `crates/gui/src/lib.rs` — mode dispatch for the two new modes

The cast screen is a popup listing `Game::field_routines`, following `render/routines.rs`'s `draw_routine_target` shape. The ally picker follows the existing battle ally picker.

The buff panel is a list of `Game::active_buffs` rows: name, magnitude, remaining, holder. A list is enough for now; no icons, no bars.

**No graphics-library calls in `render/`.** Everything goes through `Painter`. If a needed operation is missing from `Painter`'s thirteen, add it to `paint.rs` and use it — do not reach around the seam.

**Steps:**

- [ ] Implement the screen and both panels.
- [ ] `cargo build -p feral-processes-gui` and `cargo clippy --workspace`.
- [ ] **Launch the game and actually look at it** — cast a routine, confirm the panel shows on the map, start a fight, confirm it still shows and the number counts down. A green suite is not evidence the screen is readable.
- [ ] Commit.

---

### Task 15: Documentation and the final gate

**Files:**
- Modify: root `README.md`, `CHANGELOG.md`
- Verify: `assets/abilities/README.md` (Task 5), `assets/items/README.md` (Task 4)

**Steps:**

- [ ] `grep` both root docs for claims this change falsifies — in particular anything stating abilities are combat-only, and anything quoting the save format version.
- [ ] Add a CHANGELOG entry naming the save-format break: v14 saves will not load.
- [ ] Confirm the two asset READMEs were updated in their own tasks; if either was missed, fix it here.
- [ ] `cargo fmt`, `cargo clippy --workspace` — zero warnings.
- [ ] **`cargo test --workspace`** — the final gate. Report the actual count and any failures; do not claim green without the output.
- [ ] Commit.

---

## Notes for the implementer

- **`Game`'s `world` field is private with no accessor.** That compiler barrier is what holds the architectural rule. Do not add `world_mut()`.
- **Borrow scoping over `.clone()`.** Collecting entities in a first pass and mutating in a second is the established idiom here (see `age_temporary_structures`), not a workaround.
- **`world.get::<Stats>(e).is_none()` is the idiom for "this entity is gone."** Don't reach for `World::get_entity`.
- **Test fixtures live in `crates/engine/src/tests/support.rs`** — `spawn_tamed`, `spawn_wild_on_player_tile`, `insert_battle`, `set_level`, `test_assets_dir`. Look there before writing a new one.
- **No flaky tests.** No sleeps, no wall-clock, no unseeded RNG. Background systems (habitat spawning, nests) will interfere with a naive assertion.
- **Nothing in this feature has been playtested.** The durations, magnitudes and Power costs are arithmetic-plausible starting numbers. Say so when reporting done; don't call the balance good because the tests pass.
