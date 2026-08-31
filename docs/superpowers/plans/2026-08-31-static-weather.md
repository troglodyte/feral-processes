# Static Weather Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship phase 2 of the environment layer — biome-wide weather ("Static") derived from the clock — and in the same change bring phase 1's ground layer out of `assets/environment/` into Rust and give both halves the readout phase 1 never wired.

**Architecture:** `crates/engine/src/environment.rs` becomes a compile-time catalogue on `notifications.rs`'s shape: two enums, each with an exhaustive `def()`. `EnvironmentEffect` stops being a one-of enum and becomes an additive struct, so ground and weather fold into one clamped answer. `Game::terrain_at` is the single door that holds the zone-1 and `Platform` gates and returns the folded result. Which event is live is `f(world seed, zone, biome, epoch)` with no stored state.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine), `bevy` + `bevy_egui` (gui). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-08-31-static-weather-design.md` — read it first; this plan argues from it and does not restate its reasoning.

## Global Constraints

Every task's requirements implicitly include these.

- **No `SAVE_FORMAT_VERSION` bump.** Nothing here reaches the save. The one latch that persists is `achievements::Profile::seen_notifications` in `profile.ron`, which is not the save file.
- **Nothing here may draw from `resources::GameRng`.** Worldgen must not: a draw does not survive a save/load and shifts every later roll in the run. Every derivation folds and reduces through `derive::index`.
- **`derive::index`, never `%`.** `%` on a small pool reads only the low bits and anti-correlates neighbouring pools. Reaching the high bits is the caller's problem — fold multi-byte values a byte at a time, as `sectors::sector_seed` (`crates/engine/src/sectors.rs:358`) does.
- **Every magnitude goes in `crates/engine/src/tuning.rs`**, in a labelled section, never inline in a match arm or a formula.
- **Both `def()` matches must be exhaustive matches, not table lookups with a fallback.** `cell_mark`'s rule: a lookup ships a new variant blank; a match fails to compile until somebody writes the words.
- **`palette::THREAT` is reserved for inbound harm** and is what `fx.rs` paints a raid's flash with. Weather takes `palette::ATTENTION`. Do not add a second meaning to `THREAT`.
- **No occult naming in game content** — no daemon/demon/ghost/wraith/phantom in any player-facing string.
- **Commit per green step.** Branch is `feat/static-weather`; check `git branch --show-current` before every commit (a concurrent session has fast-forwarded and deleted a branch mid-task before). **Never push.**
- **`balance_sim` gates none of this** and must not be edited. It models no walking.
- Run `cargo fmt` and `cargo clippy --workspace` at the end of every task; fix warnings rather than silencing them.

---

## File map

**Engine — rewritten**

- `crates/engine/src/environment.rs` — the catalogue. Loses `EnvironmentDb`, `EnvironmentDef`, `load_dir`, `fault`. Gains `EnvironmentEffect` (struct), `GroundCondition`, `StaticEvent`, `ConditionDef`, `StaticDef`.
- `crates/engine/src/game/environment.rs` — the resolvers and the one door. Loses `ground_effect`. Gains `Terrain`, `Game::terrain_at`, `Game::static_at`, `Game::static_epoch`, `Game::terrain_row`.
- `crates/engine/src/tests/environment.rs` — loader tests become table and derivation tests.

**Engine — modified**

- `crates/engine/src/game/turn.rs` — the hook at the `walkable` branch; the ambush multiplier in `maybe_ambush`; the four log triggers.
- `crates/engine/src/game/lifecycle.rs` — four sites removing `EnvironmentDb` (`:111`, `:148`, `:377`, `:428` field/insert pairs; `:1976` struct field; `:2050` load call).
- `crates/engine/src/resources.rs` — `SeenConditions`, session-only.
- `crates/engine/src/notifications.rs` — `FirstStatic` variant, `all()` 7 → 8, `def()` arm, `latch_key()` arm.
- `crates/engine/src/tuning.rs` — the Static section; `MAX_ENVIRONMENT_*` doc comments lose the half about what a file might write.
- `crates/engine/src/tests/assets.rs` — the two shipped-file censuses are deleted; their intent moves to `tests/environment.rs` as table censuses.

**Engine — deleted**

- `assets/environment/` entire (three `.ron` files and `README.md`).

**GUI — modified**

- `crates/gui/src/render/hud/map_frame.rs` — `Ground`, and `draw_map_frame` takes it.
- `crates/gui/src/render/base.rs` — both `draw_map_frame` call sites; the `"SECTOR MAP"` census at `:1790`.

**Docs**

- `CLAUDE.md`, `docs/seams.md`, `.claude/skills/seams/`, and four repointed doc comments in `rock.rs`, `perks.rs`, `crates/gui/src/sprites.rs`, `crates/gui/src/render/base.rs`.

---

## Task 1: The catalogue comes home

Pure refactor. **Behaviour must be identical to today at the end of this task** — same three conditions, same magnitudes, same log line, same bite. Nothing weather-related appears yet.

**Files:**
- Rewrite: `crates/engine/src/environment.rs`
- Rewrite: `crates/engine/src/game/environment.rs`
- Rewrite: `crates/engine/src/tests/environment.rs`
- Modify: `crates/engine/src/game/turn.rs` (the `walkable` branch hook, ~`:510-560`)
- Modify: `crates/engine/src/game/lifecycle.rs` (six sites, listed in the file map)
- Modify: `crates/engine/src/tests/assets.rs` (delete `no_shipped_environment_file_claims_the_base_slab` and `the_default_ground_stays_neutral`, and the magnitude census above them)
- Delete: `assets/environment/` (whole directory)

**Interfaces produced:**

```rust
// crates/engine/src/environment.rs
pub struct EnvironmentEffect {
    pub attrition_percent: f32,
    pub min_damage: i32,
    pub extra_ticks: u32,
    pub ambush_mult: f32,
}
impl EnvironmentEffect {
    pub const NONE: EnvironmentEffect;              // 0.0, 0, 0, 1.0
    pub fn is_none(self) -> bool;
    pub(crate) fn fold(self, other: EnvironmentEffect) -> EnvironmentEffect;
    pub(crate) fn clamped(self) -> EnvironmentEffect;
    pub fn bite(self, max_hp: i32) -> i32;
}

pub struct ConditionDef {
    pub name: &'static str,
    pub description: &'static str,
    pub effect: EnvironmentEffect,
}

pub enum GroundCondition { DanglingReads, ThermalLoad, LockContention }
impl GroundCondition {
    pub fn all() -> [GroundCondition; 3];
    pub fn for_biome(biome: crate::world::Biome) -> Option<GroundCondition>;
    pub fn def(self) -> ConditionDef;
}

// crates/engine/src/game/environment.rs
pub struct Terrain {
    pub biome: crate::world::Biome,
    pub condition: Option<GroundCondition>,
    pub effect: EnvironmentEffect,   // folded and clamped
}
impl Game {
    pub fn terrain_at(&mut self, x: i32, y: i32) -> Terrain;
}
```

`ambush_mult` is carried by the struct from this task but nothing reads it yet; that is deliberate, so Task 3 adds a reader rather than a field.

- [ ] **Step 1: Write the catalogue's failing tests**

In `crates/engine/src/tests/environment.rs`, replacing the loader tests wholesale. Test intents, one test each:

1. `for_biome` answers `DanglingReads` for `NullSector`, `ThermalLoad` for `Mainframe`, `LockContention` for `Deadlock`.
2. `for_biome` answers `None` for `OpenGrid`, `DataVoid`, `BlackIce` and `Platform`. (`the_default_ground_stays_neutral` and `no_shipped_environment_file_claims_the_base_slab`, moved off the filesystem.)
3. Census over `GroundCondition::all()`: every def has a non-empty `name` and `description`. Walk the array, not a hand-written list — the array length is what fails to compile when a variant is added.
4. Census over `GroundCondition::all()`: every authored `attrition_percent` is in `0.0..=MAX_ENVIRONMENT_ATTRITION`, every `min_damage >= 0`, every `extra_ticks <= MAX_ENVIRONMENT_DRAG_TICKS`, every `ambush_mult <= MAX_STATIC_AMBUSH_MULT`. **This replaces `EnvironmentDef::fault`'s three load-time refusals.**
5. `fold` adds attrition, adds the damage floor, adds drag, and **multiplies** the ambush term. Assert all four in one test against two hand-built effects — this is the test the spec names as the guard on the one-of → all-of shape change.
6. `clamped` cuts a hand-built effect that exceeds each ceiling down to it, one assertion per ceiling. Built by hand, so it does not depend on the shipped magnitudes staying where they are.
7. `bite` returns `max(round(max_hp * pct), min_damage)`, and returns `0` for `EnvironmentEffect::NONE` — a no-attrition effect must not deal the floor.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine environment` — expect compile errors naming the missing types.

- [ ] **Step 3: Write `environment.rs`**

Transcribe the three deleted `.ron` files exactly: `DanglingReads` (Null Sector, 0.02 / floor 1), `ThermalLoad` (Mainframe, 0.03 / floor 2), `LockContention` (Deadlock, drag 1). Names and descriptions carry over verbatim from the deleted files — do not rewrite the prose.

The magnitudes move to `tuning.rs` in this step, in a section headed for ground conditions. Add `MAX_STATIC_AMBUSH_MULT` here too (Task 3 needs it and the census in Step 1 already reads it).

Module doc must carry the argument for the file being Rust — `notifications.rs`'s module doc is the model, and the argument is in the spec's "Weather is a table in Rust" section. Do not paraphrase; the reasoning belongs in `docs/seams.md` (Task 7) and the module doc gets the short form.

- [ ] **Step 4: Run the tests and make them pass**

`cargo test -p feral-processes-engine environment`

- [ ] **Step 5: Rewrite `game/environment.rs` as the one door**

`terrain_at` holds, in this order: the zone-1 gate (`ZoneLevel <= 1` → `Terrain` with the biome and `EnvironmentEffect::NONE`), the biome lookup, the `Platform` refusal, then `GroundCondition::for_biome`. **Both gates stay inside this one function** — that is phase 1's seam and the reason the file exists.

The zone-1 gate returns a `Terrain` carrying the real biome, not an `Option`: the biome's *name* is deliberately outside the gate, because a zone-1 player must still learn the world's vocabulary. Getting this backwards is the mistake phase 1's `zone_one_takes_no_bite_but_still_names_the_ground` test exists to catch — port that test.

- [ ] **Step 6: Rewrite the `turn.rs` hook against the struct**

The `match effect { Some(Attrition{..}) => .., Some(Drag{..}) => .., None => {} }` block becomes one `terrain_at` call, one `bite` through `apply_damage`, and one `drag_ticks` assignment. Order is unchanged: after the `Position` write, before `maybe_ambush`, before `self.tick()`.

- [ ] **Step 7: Delete the loader and its wiring, and correct two doc comments**

`EnvironmentDb`, `EnvironmentDef`, `load_dir`, `fault`, the `AssetDbs` field, both `insert_resource` calls, both destructuring sites, the load call and its `warnings.extend`. Then `rm -r assets/environment/`.

`MAX_ENVIRONMENT_ATTRITION` and `MAX_ENVIRONMENT_DRAG_TICKS` keep their values and lose the half of their doc comments that argues about what a file might author — there are no files. What replaces that half is the reason the ceilings still matter: the **fold** can exceed what either half authored on its own.

- [ ] **Step 8: Port phase 1's behavioural tests**

From the deleted `tests/environment.rs`, these must survive unchanged in intent: attrition applies on a step onto claimed ground and not on a step that bounced off a wall; attrition that kills does not then start an ambush; a Mitigation field buff blunts attrition; `Drag` advances the clock by `1 + extra_ticks`; the party is untouched; `Platform` takes no effect; the crossing line fires on a biome change and not within one biome.

- [ ] **Step 9: Full gate**

`cargo test --workspace` — 3842 tests, minus the deleted loader tests, plus the new table tests. If many tests fail at once with `NotFound` on an assets path, that is stale build artifacts from the `petmud` rename, not this change: `cargo clean -p feral-processes-engine -p feral-processes-app-core` (never a full `cargo clean`, `target/` is ~4 GB).

Then `cargo clippy --workspace` and `cargo fmt`.

- [ ] **Step 10: Commit**

Stage explicit paths — never `git add -A`, it sweeps up another agent's worktree gitlink under `.claude/worktrees/`.

`refactor(environment): the ground's catalogue is a table in Rust, not files on disk`

---

## Task 2: The Static catalogue and its derivation

Pure and testable on its own. Nothing reads `static_at` yet.

**Files:**
- Modify: `crates/engine/src/environment.rs` (add `StaticEvent`, `StaticDef`)
- Modify: `crates/engine/src/game/environment.rs` (add `static_at`, `static_epoch`, `static_seed`)
- Modify: `crates/engine/src/tuning.rs`
- Modify: `crates/engine/src/tests/environment.rs`

**Interfaces consumed:** `EnvironmentEffect`, `ConditionDef`'s shape (Task 1).

**Interfaces produced:**

```rust
// crates/engine/src/environment.rs
pub struct StaticDef {
    pub name: &'static str,
    pub description: &'static str,
    pub biomes: &'static [crate::world::Biome],
    pub weight: u32,
    pub effect: EnvironmentEffect,
}
pub enum StaticEvent { LeakingMemory, ThreadStorm, PacketFlood, SignalNoise }
impl StaticEvent {
    pub fn all() -> [StaticEvent; 4];
    pub fn def(self) -> StaticDef;
    pub fn claims(self, biome: crate::world::Biome) -> bool;
}

// crates/engine/src/game/environment.rs
impl Game {
    pub(crate) fn static_epoch(&self) -> u64;
    /// Ungated derivation, for an epoch the caller names. The zone-1 and
    /// `Platform` gates live in `terrain_at`, not here.
    ///
    /// **Takes the epoch rather than reading the clock**, because Task 4
    /// has to ask what was live in the epoch that just ended. A version
    /// that reads `current_tick()` internally cannot answer that, and the
    /// turnover announcement would have no way to know what cleared.
    pub(crate) fn static_in_epoch(
        &self,
        biome: crate::world::Biome,
        epoch: u64,
    ) -> Option<StaticEvent>;
    /// `static_in_epoch` at the current epoch. What `terrain_at` calls.
    pub(crate) fn static_at(&self, biome: crate::world::Biome) -> Option<StaticEvent>;
}
```

**Derives.** `GroundCondition` and `StaticEvent`: `Clone, Copy, Debug, PartialEq, Eq`. `EnvironmentEffect`: `Clone, Copy, Debug, PartialEq` — it carries `f32`, so no `Eq`. `Terrain` returns its condition and event by value, so `Copy` is load-bearing rather than a convenience.

**The magnitudes** (spec's table; all go in `tuning.rs`):

| variant | biomes | attrition | floor | drag | ambush | weight |
| --- | --- | --- | --- | --- | --- | --- |
| `LeakingMemory` | NullSector | +0.015 | +1 | — | — | 1 |
| `ThreadStorm` | Mainframe | — | — | +1 | x1.5 | 1 |
| `PacketFlood` | OpenGrid | — | — | +1 | x1.6 | 1 |
| `SignalNoise` | Deadlock, NullSector | — | — | — | x2.0 | 1 |

Plus `STATIC_EPOCH_TICKS: u64 = 150`, `STATIC_CLEAR_WEIGHT: u32 = 3`, `MAX_STATIC_AMBUSH_MULT: f32 = 2.5` (added in Task 1).

- [ ] **Step 1: Write the failing derivation tests**

1. Census over `StaticEvent::all()`: non-empty name and description; every magnitude inside its ceiling; `biomes` non-empty; no event claims `Platform`, `DataVoid` or `BlackIce`.
2. Census: every walkable biome that any event claims is one `classify` can actually produce. Specifically assert `SignalNoise` claims a biome other than `Deadlock` — the spec's reach argument, and the guard against shipping a second `LockContention` nobody meets.
3. `static_at` is **stable within an epoch**: same answer at tick `t` and tick `t + 1` when both fall in the same epoch.
4. `static_at` **changes across epochs**: over a walk of many consecutive epochs for one biome, more than one distinct answer appears (including `None`). Assert both that a live event is reachable and that clear is reachable.
5. `static_at` **draws no `GameRng`**: snapshot the RNG state, call `static_at` a hundred times, assert the state is byte-identical. This is the worldgen rule and the whole reason for the derivation.
6. `static_at` gives the same answer either side of a save/load round trip at the same tick.
7. Two adjacent biomes at the same epoch are decorrelated: over many epochs, `NullSector`'s and `Mainframe`'s answers are not the same sequence. This is what `derive::index`'s high-bit reduction buys and what `%` would silently break.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine environment`

- [ ] **Step 3: Implement the fold and the pick**

The one piece worth spelling out, because getting the fold wrong is silent — a value folded in as the last word, differing only in its low bits, never reaches bit 63, which is the bit `derive::index` reads:

```rust
fn static_seed(seed: u32, zone: u32, biome: Biome, epoch: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for word in [seed as u64, zone as u64, biome as u64, epoch] {
        for byte in word.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}
```

`Biome` needs a stable integer for this. Use an explicit `fn ord(self) -> u64` exhaustive match on `Biome` rather than `as u64` on the enum — the discriminant is save-adjacent and reordering variants must not silently re-roll every world's weather.

The pick: total = `STATIC_CLEAR_WEIGHT` + the weights of every event claiming this biome; roll `derive::index(seed, total)`; walk the clear slot first, then the events in `all()` order. Walking clear first means adding a fifth event does not reshuffle which epochs are clear.

- [ ] **Step 4: Run the tests and make them pass**

`cargo test -p feral-processes-engine environment`

- [ ] **Step 5: Full gate and commit**

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.

`feat(environment): Static is derived from the clock, not stored`

---

## Task 3: Weather reaches the player

Folds `StaticEvent` into `terrain_at` and wires the ambush multiplier. This is where behaviour changes.

**Files:**
- Modify: `crates/engine/src/game/environment.rs` (`Terrain` gains `event`; `terrain_at` folds)
- Modify: `crates/engine/src/game/turn.rs` (`maybe_ambush`, ~`:589-610`)
- Modify: `crates/engine/src/tests/environment.rs`

**Interfaces consumed:** `Game::terrain_at`, `Game::static_at`, `EnvironmentEffect::fold`/`clamped` (Tasks 1–2).

**Interfaces produced:**

```rust
pub struct Terrain {
    pub biome: crate::world::Biome,
    pub condition: Option<GroundCondition>,
    pub event: Option<StaticEvent>,          // new
    pub effect: EnvironmentEffect,           // now ground.fold(event).clamped()
}
```

- [ ] **Step 1: Write the failing tests**

1. **Stacking:** on ground with a condition and in an epoch with a live event, `terrain_at().effect` carries the sum of both attritions, the sum of both drags **and** the product of both ambush multipliers. Assert all three — a reader that takes only the bite is exactly the silent failure the shape change opens.
2. **One bite, not two:** the stacked attrition lands through `apply_damage` as a single call, so `apply_damage`'s floor-at-1 applies once. Assert the HP delta, not the call count.
3. **Clamped after folding:** force a ground/event pair whose sum exceeds `MAX_ENVIRONMENT_ATTRITION` and assert the effect is cut to the ceiling.
4. **Zone 1 takes no weather**, and still names the ground.
5. **`Platform` takes no weather.**
6. **The ambush multiplier reaches the roll:** run one seeded fixture across many steps with an event forced live, and the same seeded fixture with the epoch forced clear, and assert the live run ambushes strictly more often. Assert the **difference**, never an absolute rate — an absolute is a seed-luck test that will fail the day an unrelated change shifts the RNG stream, and `-p feral-processes-engine` and `--workspace` are different builds that shift it differently.
7. **Attrition that kills does not then start an ambush**, now with weather in the sum. The one lethal-edge interaction; `maybe_ambush` already checks `is_game_over` and this asserts it still holds.
8. **Mitigation blunts the stacked bite** — it goes through `apply_damage`.
9. **The party is untouched** by weather, as by ground.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Fold the event into `terrain_at`**

`ground.effect.fold(event.effect).clamped()`. The gates do not move.

- [ ] **Step 4: Wire the multiplier into `maybe_ambush`**

`maybe_ambush` currently resolves the tile's biome itself to skip `Platform`. Replace that lookup with one `terrain_at` call and use both halves: the `Platform` skip becomes the biome it returns, and the roll becomes `random_bool((RANDOM_ENCOUNTER_CHANCE * effect.ambush_mult as f64).clamp(0.0, 1.0))`.

Note the tile is already in `WorldMap`'s chunk cache from the move that preceded it, so the second resolution is not a second chunk generation.

- [ ] **Step 5: Run the tests and make them pass**

- [ ] **Step 6: Full gate and commit**

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.

`feat(environment): weather stacks on the ground and pulls at the ambush roll`

---

## Task 4: The log finally says what the ground is doing

Four triggers. This is the half that fixes the reported bug.

**Files:**
- Modify: `crates/engine/src/game/turn.rs`
- Modify: `crates/engine/src/resources.rs`
- Modify: `crates/engine/src/tests/environment.rs`

**Interfaces consumed:** `Terrain`, `GroundCondition::def`, `StaticEvent::def`, `Game::static_epoch` (Tasks 1–3).

**Interfaces produced:**

```rust
// crates/engine/src/resources.rs
/// Which ground conditions this **session** has already described.
///
/// Not saved, on `RunFeats`' precedent. A reload re-announces, which is
/// cheaper than a save field for flavour text.
#[derive(Resource, Default)]
pub struct SeenConditions(pub Vec<crate::environment::GroundCondition>);

// crates/engine/src/game/turn.rs
impl Game {
    /// Announces weather arriving or clearing under the player, if the
    /// tick just taken crossed an epoch boundary. Called once per player
    /// step, after every tick that step spent.
    pub(crate) fn note_static_turnover(&mut self, epoch_before: u64);
}
```

`GroundCondition` needs `PartialEq` for the `SeenConditions` check.

- [ ] **Step 1: Write the failing tests**

1. The crossing line names the condition: stepping into Null Sector logs a line containing both the biome name and `"Dangling Reads"`.
2. Unclaimed ground names nothing extra: stepping into Open Grid logs the biome and no condition name. Open Grid must read exactly as it does today.
3. The description fires **once per session** per condition: cross into Null Sector, leave, cross back, and assert the description text appears exactly once. Count with `resources::condense`'s `repeats` summed, not by counting entries — `message_history` folds repeats and an entry count would hide a line logged twice.
4. Weather arrival fires on the boundary tick, in the player's biome, carrying the event's description.
5. Weather clearing fires on the boundary the other way.
6. Turnover in a biome the player is **not** standing in is silent.
7. A save/load mid-epoch does not re-announce arrival: round-trip the game between two ticks inside one epoch and assert no new arrival line.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Extend the crossing line and add the description trigger**

Both in the `walkable` branch, beside the existing `"You cross into {}."`. The condition's name joins that line; the description is a second line, gated on `SeenConditions`.

- [ ] **Step 4: Add `note_static_turnover`**

Capture `static_epoch()` before `self.tick()` in `move_player`, and call `note_static_turnover(before)` **after the drag loop**, so a step that spent four ticks announces one turnover rather than four. The comparison is `static_in_epoch(biome, epoch_before)` versus `static_in_epoch(biome, static_epoch())`; both are pure calls, so nothing is stored. This is the reason Task 2's derivation takes an epoch instead of reading the clock.

- [ ] **Step 5: Run the tests and make them pass**

- [ ] **Step 6: Full gate and commit**

`feat(environment): the ground and the weather say what they are doing`

---

## Task 5: The first weather is a notification

**Files:**
- Modify: `crates/engine/src/notifications.rs`
- Modify: `crates/engine/src/game/turn.rs`
- Modify: `crates/engine/src/tests/environment.rs`

**Interfaces consumed:** `Game::notify` (`crates/engine/src/game/notify.rs:14`), `Terrain` (Task 3).

**Interfaces produced:** `NotificationKind::FirstStatic`, latch key `"tutorial_first_static"`.

- [ ] **Step 1: Write the failing tests**

1. `NotificationKind::all()` has 8 entries and contains `FirstStatic`. (The array length makes this a compile error too, which is the point — the test is for the containment.)
2. Standing on a tile with a live event queues `FirstStatic` once, and a second such step queues nothing.
3. The existing `tutorials_latch_and_milestones_do_not` census passes **without being edited** — `FirstStatic` is in the tutorial group and is `Repeat::OnceEver`. If that test needs editing, the variant is in the wrong group.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Add the variant**

Three arms: `all()`, `def()`, `latch_key()`. Place it with the tutorials, above the `// --- Milestones` comment. `latch_key` is `profile.ron`'s format and the variant name is not — pick the string once and do not rename it later; a retired key must be inert, not a parse error that costs the player their achievements.

Copy: the title and body are new prose. It must teach that weather is biome-wide, temporary, and rotates on its own — the three things a player cannot infer from one bite.

- [ ] **Step 4: Fire it from the movement hook**

At the same site the effect is applied, not at the epoch boundary — "you were told" and "it happened to you" must not come apart for a player standing three biomes away. `notify` is fired unconditionally; the once-only rule is `Repeat::OnceEver` and lives in `queue_notification`.

- [ ] **Step 5: Run the tests and make them pass**

- [ ] **Step 6: Check the height census**

`cargo test -p feral-processes-gui the_tallest_shipped_notification_fits_its_screen` — the screen has no scroll, so body length is a layout constraint. If the new body overflows, shorten the copy; do not change the cap.

- [ ] **Step 7: Full gate and commit**

`feat(notifications): the first Static takes the screen`

---

## Task 6: The map pane's border says where you are standing

**Files:**
- Modify: `crates/engine/src/game/environment.rs` (`terrain_row`)
- Modify: `crates/gui/src/render/hud/map_frame.rs`
- Modify: `crates/gui/src/render/base.rs` (both `draw_map_frame` call sites; the census at `:1790`)

**Interfaces consumed:** `Game::terrain_at` (Task 3).

**Interfaces produced:**

```rust
// crates/engine/src/game/environment.rs — the engine owns the derivation,
// the renderer draws it and derives nothing.
pub struct TerrainRow {
    pub biome: &'static str,
    pub condition: Option<&'static str>,
    pub event: Option<&'static str>,
}
impl Game {
    /// What the map pane's border reads. `None` underground — a Stack
    /// frame has no biome, which is the same reason the threat readout
    /// counts no hostiles down there.
    pub fn terrain_row(&mut self) -> Option<TerrainRow>;
}
// Re-exported from the crate root, as `AttentionRow` and `StockRow` are:
// gui imports it by `use feral_processes_engine::TerrainRow`, and the
// renderer never reaches into a module path.

// crates/gui/src/render/hud/map_frame.rs
pub(in crate::render) fn draw_map_frame(
    pane: Rect,
    ground: Option<TerrainRow>,
    threat: Threat,
    painter: &Painter,
    m: &Metrics,
);
```

- [ ] **Step 1: Write the failing tests**

1. **Width census**, through `paint::with_painter` so real DejaVu Sans Mono is measured (the UI face is DejaVu; the map face is unscii — this has been got backwards twice): the widest shipped weather-plus-ground pair fits the map pane's top border beside the widest `THREAT` readout at 1280x720. Build the widest pair by walking `GroundCondition::all()` and `StaticEvent::all()` rather than hardcoding a string.
2. `strip::fitting` drops the ground segment before the weather segment: hand it a budget that fits only one and assert the survivor is the weather.
3. Underground, `terrain_row` is `None` and the strip draws nothing where `SECTOR MAP` used to be.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Replace the title with the readout**

`Mount::TopLeft` takes `strip::fitting(&[weather_segment, ground_segment], avail, ..)`. `avail` is the pane width less the measured advance of the threat pieces — measure it, do not estimate it; the row has no wrap and no clip, and `Painter` clips vertically but never horizontally, so what does not fit must be **counted**, not drawn off the end.

Weather takes `palette::ATTENTION`, the ground name `palette::LABEL`. Not `THREAT`.

Keep `draw_map_frame`'s existing ordering rule: it is called **after** the pane's contents, because `border_strip` paints its own background and drawing it first lets the map's fill cut the labels in half.

- [ ] **Step 4: Update both call sites**

The Stack branch (`base.rs:652`) passes `None`. The surface branch passes `game.terrain_row()`, gathered **before** the painter borrow, the way `StatusBarState` is.

- [ ] **Step 5: Update the draw-order census at `base.rs:1790`**

It looks for the literal `"SECTOR MAP"`, which no longer exists. Repoint it at a label the new strip actually draws — walk `GroundCondition::all()` for one rather than hardcoding prose that will drift.

- [ ] **Step 6: Run the tests and make them pass**

- [ ] **Step 7: Full gate and commit**

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.

`feat(hud): the map pane's border reads the ground you are standing on`

---

## Task 7: The documentation obligations

No code. Do not skip: three of these are load-bearing and one is a set of four statements that are now false.

**Files:**
- Modify: `CLAUDE.md` (and `cp` it to `AGENTS.md` — gitignored twins, no tracking to catch drift)
- Modify: `docs/seams.md`
- Modify: `.claude/skills/seams/`
- Modify: `crates/engine/src/rock.rs:101`, `crates/engine/src/perks.rs:674`, `crates/gui/src/sprites.rs:13`, `crates/gui/src/render/base.rs:2165`

- [ ] **Step 1: Repoint the four false doc comments**

Each cites "deleting `assets/environment/`" as *the* example of the absent-is-silent rule. The directory no longer exists, so each is a claim a reader will check and find wrong. Repoint at `assets/sectors/`, which keeps both the rule and an example of it.

Verify with `rg -n 'assets/environment' -g '!docs'` returning nothing outside the spec and this plan.

- [ ] **Step 2: Write the argument to `docs/seams.md`**

Under the same titles the rules will take in `CLAUDE.md`. The argument, the measurement, what was tried and rejected — the spec is the source, but `seams.md` is where it lives permanently, because a spec's `**Status:**` header goes stale and fourteen of them already lie.

Three entries: the one door and its two gates; the fold being additive with a multiplied ambush term; the derivation and why there is no save field.

- [ ] **Step 3: Write the trap to the `seams` skill**

One reference file per subsystem, and this is the environment's. The trap, not the rule: a reader that takes only the attrition terms off `EnvironmentEffect` compiles clean and silently drops drag and the ambush multiplier.

- [ ] **Step 4: Write the rule to `CLAUDE.md`**

**One sentence each — that is a budget, not a style.** This file is loaded on every turn and reached 151 KB by letting each seam's trap creep in beside its rule. The existing `Game::ground_effect` line under "The ground" is replaced, not added to.

Then `cp CLAUDE.md AGENTS.md`.

- [ ] **Step 5: Verify the three places agree**

`rg -n 'ground_effect'` must return nothing. Every symbol named in the new `CLAUDE.md` lines must exist in the source — verify, do not remember; a seam doc has described unmerged code before.

- [ ] **Step 6: Commit**

`docs(seams): the environment is one door, one fold and one derivation`

---

## Landing

**Not part of any task, and not to be done without asking.**

`CHANGELOG.md` gets its `## X.Y.Z` section and the workspace version is bumped **at the merge**, not on this branch — a rebase or squash would otherwise invalidate a version already tagged. The section must say a mod-facing content directory was removed; that is the one thing in this change a third party could notice. Which digit moves is decided by `CHANGELOG.md`'s preamble: "breaking" means a player's save stops loading, and this change does not, so it is not a major.

Then merge, tag, push, clean up — and note that `git push` alone does not send tags.
