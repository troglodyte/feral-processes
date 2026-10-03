# Data-defined status effects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Checkbox steps. Per project rule, this plan carries files, interfaces, test intent and gates — **not finished code**; read the cited source before writing.

**Spec:** `docs/superpowers/specs/2026-10-03-data-defined-status-effects-design.md` — read it first; § refers to it.

**Goal:** Statuses become `assets/statuses/*.ron`; a body carries several at once; Poison stacks; Throttled and Locked ship.

**Architecture:** `StatusDb` (new `crates/engine/src/statuses.rs`) holds `StatusDef`s; `StatusId(String)` replaces `StatusKind`; `StatusEffects.active` becomes a `Vec`; behaviour queries replace kind checks. Views carry structured tags; gui draws them.

## Global constraints

- No save change (`StatusEffects` stays unserialized), `SAVE_FORMAT_VERSION` untouched → patch/minor per `CHANGELOG.md` preamble, **at landing only**.
- New asset fields `#[serde(default)]`; malformed `.ron` → logged warning, skipped (copy the `MemoryDb::load_dir` shape: returns `(db, warnings)`, absent dir is not an error).
- "Mirrors"/"same as" → one shared function (CLAUDE.md).
- `arm_status` stays the sole writer besides Cleanse and `clear_battle_status_effects`.
- Each task: failing test first → `cargo test -p feral-processes-engine <name>` → `cargo fmt` + `cargo clippy --workspace --all-targets` clean → commit explicit paths (never `git add -A`). Never push.
- Gates at each phase end: `cargo test --workspace`; `cargo test -p feral-processes-engine balance_sim` must not move (§6).

## Source anchors (verified 2026-10-03)

| What | Where |
| --- | --- |
| `StatusKind`, `ActiveStatus`, `StatusEffects` | `components.rs:1134/1169/1199` |
| `arm_status`, `is_stunned`, `tick_status_effects` | `game/combat_status.rs:66/91/105` |
| `combatant_profile` (Exposed cut at :151) | `game/combat_damage.rs:132`; fumble ladder ~:243/:256 |
| `restore_hp` | `game/combat_damage.rs:430` |
| `clear_battle_status_effects` | `game/combat_teardown.rs:164` |
| `status_label` (party panel; used by views) | `game/party.rs:947`, callers `party.rs:569`, `combat_round.rs:894/935`, `inspection.rs:2205/2331` |
| ability landing line | `combat_round.rs` ~1647; `apply_status_effect` :1698 |
| policy features | `game/combat_policy.rs:230,266` |
| view fields `status_effect: Option<String>` | `views.rs:1915/1946/2590` |
| gui readers | `gui/src/render/battle.rs:34` (`status_tag`), **`gui/src/render/manifest.rs:290`** (spec omits it), `render/tactical.rs` beside `TamperTag` (:1002) |
| production DB load | `game/creation.rs:378`; other loaders: `app-core/src/app/arena.rs:53`, `sprite_forge.rs:372`, `launcher/src/tuner/eval.rs:145`, `balance_sim.rs:293/953`, `components.rs:3418` |
| `EXPOSED_EVASION_PERCENT` | `tuning.rs:4299` (=50), `battle.rs:203` doc |
| species "class" | derived — `species.rs:394` `AffinityClass::{Saboteur,Leech}` from affinity, not a field |
| `StatusKind` users | 19 files / 58 refs in `crates/engine`; 35 `.ron` files `kind: Bleed|Stun|Exposed` |

---

## Phase 1 — schema and DB, additive (one context, engine only)

### T1: `StatusDef` / `StatusDb`

**Files:** new `crates/engine/src/statuses.rs` (+ `lib.rs` mod), new `assets/statuses/{bleed,stun,exposed}.ron`, new `assets/statuses/README.md`.

**Produces:** `StatusId(pub String)` (serde transparent, `Hash/Eq/Clone`, `From<&str>`); `StatusDef { id, name, tag, stacking: StatusStacking, behaviours: Vec<StatusBehaviour>, inflict, #[serde(default)] tick, expire }`; `StatusStacking::{Refresh, Stack{max}}`; `StatusBehaviour::{DamagePerRound, SkipTurn, EvasionCut(i32), AtkPercent(i32), MitigationPercent(i32), HealBlock}`; `StatusDb::load_dir(&Path) -> Result<(StatusDb, Vec<String>), _>`, `get(&StatusId)`. Duplicate id → warning, first wins. Three files per §4 with today's messages (copy from `combat_status.rs` match arms); `exposed.ron` uses `EvasionCut(50)`.

**Tests (in `statuses.rs`):** shipped dir loads all three with no warnings; malformed file skipped + warned; duplicate id warned, first wins; absent dir → empty db.

### T2: load it into `Game`

**Files:** `game/creation.rs:378` and wherever `AbilityDb` lives as a resource; any other constructor above that builds a `Game` from DBs.

**Produces:** `StatusDb` resource in the world. No behaviour reads it yet.

- [ ] T1 → T2, commit each. Phase gate.

---

## Phase 2 — ids and the Vec, behaviour-preserving (one context; the big rename)

Compile-breaking as a unit; do it as one task, commit once green.

### T3: delete `StatusKind`

**Files:** all 19 engine files above; 35 `assets/species|abilities/*.ron` (`kind: Bleed` → `kind: "bleed"` etc. — `rg -l` then `sed`, review the diff).

**Changes:**
- `MoveEffect::kind`, `AbilityEffect::Debuff::kind`: `StatusId`.
- `ActiveStatus { id, remaining, power, stacks, landed_this_round }`; `StatusEffects { active: Vec<ActiveStatus> }`.
- `arm_status(entity, &StatusId, duration, power)`: unknown id → logged no-op; present+`Refresh` → max/max; absent → push `stacks: 1`. (No shipped status uses `Stack` until Phase 5; its arm lands in T5.)
- One query helper in `combat_status.rs`: `status_sum(&self, e, f: impl Fn(&StatusBehaviour) -> Option<i32>) -> i32` (sums `value × stacks` over entries) plus `has_behaviour(e, pred)`. `is_stunned` = has `SkipTurn`; `combatant_profile` evasion cut = `status_sum(EvasionCut)`, capped 100. Delete `EXPOSED_EVASION_PERCENT`; fix the `battle.rs:203` and `components.rs:1140` docs.
- `tick_status_effects`: iterate all entries; `DamagePerRound` uses `power × stacks`, logs def `tick` (`{target}`, `{n}`); expire with def `expire`.
- Inflict/expire/landing lines from def (`{target}` substitution — one fn, used by moves and abilities); ability-specific phrasing dropped (§1 Messages).
- `status_label`: `"{name} ({remaining})"` comma-joined over all entries.
- Fumble ladder: `tuning::FUMBLE_EXPOSED_STATUS = "exposed"`, `FUMBLE_CRASH_STATUS = "stun"`.
- Policy features (`combat_policy.rs:230,266`): "move carries / target has a status with `SkipTurn` / `DamagePerRound`".
- Startup cross-check after all DBs load: unresolved `StatusId` in a move/ability → warning, rider dropped; missing fumble ids → warned once.

**Tests:** existing bleed/stun/exposed tests pass with only the rename (if an assertion on an ability landing string changes, that is the accepted §1 drop — note it in the commit). `tests/combat_status.rs:1258` (Exposed term) must still bite. New: unknown id at arm → no-op; unknown id in a move → rider dropped + warning; two different statuses coexist and tick independently; Refresh max/max; Cleanse and teardown clear all.

- [ ] T3, commit. Phase gate + balance_sim unchanged.

---

## Phase 3 — new behaviours (one context, engine)

### T4: `AtkPercent`, `MitigationPercent`, `HealBlock`

**Files:** `combat_damage.rs` (`combatant_profile`, `restore_hp`), `tests/combat_status.rs`.

- `combatant_profile`: attack × `(100 + sum AtkPercent).max(0)/100`, same for mitigation.
- `restore_hp`: body has `HealBlock` → return 0 (caller logs). Check every in-battle heal/drain goes through it (`rg restore_hp`); a heal path that bypasses it is a finding, route it through.

**Tests:** each percent reaches `combatant_profile`; a −200% stack floors at 0; HealBlock zeroes a heal ability and a drain.

### T5: `Stack{max}`

**Files:** `combat_status.rs`, tests.

- Present + `Stack{max}` → `stacks = min(stacks+1, max)`, `remaining = new`, `power = new`, flag set.

**Tests:** stack adds, caps at max, damage = power × stacks; landed-this-round exemption is per entry (stack two different statuses in different rounds).

- [ ] T4, T5, commit each. Phase gate.

---

## Phase 4 — views and UI (one context; engine views + gui)

### T6: structured tags

**Files:** `views.rs:1915/1946/2590`, their builders (`combat_round.rs:894/935`, `inspection.rs:2205/2331`), `gui/src/render/battle.rs:34`, `render/manifest.rs:290` (+ its test fixtures :1394/:2388), `render/tactical.rs` near `tamper_tag_text`, plus `TacticalBody` in `tactical/view.rs` if it carries status.

**Produces:** `pub struct StatusTagView { pub tag: String, pub stacks: u32, pub remaining: u32 }`; field `statuses: Vec<StatusTagView>` replaces `status_effect`. One gui fn formats a tag (`PSN×3` when stacks > 1) and both battle and tactical call it. Draw through `Painter` only (`.claude/rules/drawing-seam.md`).

**Tests:** engine: view lists two statuses with stacks. gui: tag formatting pure-fn test. Optional screenshot: `--template` with a battle, Read the PNG.

- [ ] T6, commit. Phase gate.

---

## Phase 5 — content, census, docs, seams (one context)

### T7: content

- `assets/statuses/{poison,throttled,locked}.ron` per §4.
- `data_poisoning.ron` (+ a group sibling if `rg data_poisoning assets` finds one) → `kind: "poison"`, description updated.
- Two new Debuff abilities (`throttled`, `locked`): price against siblings — `memory_leak` 5.0/cd1, `deadlock` 6.0/cd2, `data_poisoning` 8.0/cd3, `hard_lock` 10.0/cd4. Add to one species whose derived class is `Saboteur` (Throttled) and one `Leech` (Locked); species-class census test stays green.

### T8: census tests

Every shipped status referenced by ≥ 1 move/ability; every `StatusId` in assets resolves in `StatusDb`.

### T9: docs + seams

`assets/statuses/README.md` (final), `assets/species/README.md`, `assets/abilities/README.md` (`kind` is an id), `docs/content-gaps.md:103-105`. Run the `seams` skill to record `arm_status` sole-writer and `restore_hp` HealBlock chokepoint (graph → skill → rules file). Not the manual, README or TODO.md.

- [ ] T7–T9, commit each. Phase gate.

---

## Phase 6 — measurement and review

- Read `docs/measurements/` first. One arena batch (`cargo run --bin arena -- …`) with a Poison-stacking species vs baseline; record whether the trained policy uses it. Write up only if it clears `docs/measurements/README.md`'s bar; otherwise report in the handback.
- Final whole-branch review (opus), diff as a file. Review focus: Refresh preserving old single-application numbers exactly; no heal path bypassing `restore_hp`; unresolved-id rider dropping at load, not panic; per-entry landed flag; balance_sim unmoved.
- Update `docs/superpowers/INDEX.md`; archive spec and delete this plan on landing. CHANGELOG + version at landing only.
