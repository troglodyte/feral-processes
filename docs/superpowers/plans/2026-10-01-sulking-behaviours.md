# Sulking behaviours (G) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Checkbox steps. Per project rule, this plan carries files, interfaces, test intent and gates — **not finished code**; read the cited source before writing.

**Goal:** Give the `Sulking` rung visible behaviour — slanted talk, freezing out rivals, petty sabotage seen by witnesses.

**Architecture:** One predicate (`Game::sulks`) feeds four engine-only changes: an interaction weight, a gossip filter, two freeze-out filters, and a new `note_sabotage` pass on the interaction period. All choices are hash folds; no save change.

**Tech stack:** Rust, `bevy_ecs` (engine crate only), RON assets.

**Spec:** `docs/superpowers/specs/2026-10-01-sulking-behaviours-design.md` — read it first; section numbers below (§) refer to it.

## Global constraints

- Engine crate + `assets/` only. No GUI, no save change, `SAVE_FORMAT_VERSION` untouched → patch release (at landing, not on the branch).
- Never draw `GameRng`. Seeds via `crate::derive::{fold, unit, index}` (`derive.rs:46/98/108`).
- New asset field `#[serde(default = …)]`; update `assets/interactions/README.md` and `assets/memories/README.md` (if it lists defs) in the same commit.
- Empty `assets/memories/` and `assets/interactions/` stay supported installs.
- "Mirrors"/"same as" → a shared function, never a copy (CLAUDE.md).
- Each task: failing test first, `cargo test -p feral-processes-engine <name>`, then `cargo fmt` + `cargo clippy --workspace --all-targets` clean, commit explicit paths (never `git add -A`). Never push.

## Review focus (each pinned by a test in the owning task)

1. **Two sulkers, one unit left** — the second must re-read stock; exactly one unit spoiled, no zero-qty consume, no second log line. (T7)
2. **Saboteur is never its own witness**, and a witness is staff only. (T7)
3. **Output item or structure with no def** (modded content removed) — log line falls back to the id string, no panic. (T7)
4. **Every interaction def at `sulking: 0.0`** — a sulker's `pick` returns `None`, no divide-by-zero. (T2)
5. **Rival beside the post who is not walking the base** (out on a sortie / not staff) does not trigger refusal — same neighbour filter as `BesideRival`. (T5)

---

## Phase 1 — predicates (one implementer context)

### T1: `sulks`, `resents_structure`, `is_beside`

**Files:** `crates/engine/src/game/base/morale.rs` (around `refuses_post`, :156), `crates/engine/src/situations.rs` (:238-249), test `crates/engine/src/tests/respite.rs` (has a `sulk(game, who)` helper at :72) or a new `tests/sulking.rs` registered in `tests/mod.rs`.

**Produces:**
- `Game::sulks(&self, e: Entity) -> bool` — `Disgruntled.grievance >= Grievance::Sulking` (check `Grievance` derives `Ord`; add it if not).
- `Game::resents_structure(&self, worker: Entity, kind: &StructureKind) -> bool` — the body of `refuses_post`'s last two lines; `refuses_post` now calls it (behaviour unchanged).
- `pub(crate) fn is_beside(a: Position, b: Position) -> bool` (Chebyshev ≤ 1) in `situations.rs`; `assess`'s `neighbours` closure calls it.

**Tests:** `sulks` true at Sulking/DownedTools/LashingOut, false at milder rungs and with no `Disgruntled`. Existing `refuses_post` and situations tests pass unedited (pure refactor).

- [ ] failing test → implement → green → fmt/clippy → commit "Sulking: sulks predicate, resents_structure, is_beside"

---

## Phase 2 — the slanted voice (§3)

### T2: `InteractionDef::sulking`

**Files:** `crates/engine/src/interactions.rs` (`InteractionDef`, `weight_for` :135, `schema_fault`, `pick` :284), `crates/engine/src/game/memories.rs` (`note_interactions` :484 — the `pick` call), `assets/interactions/{slight,insult,complain,gossip,small_talk,shop_talk,joke,compliment,thanks}.ron`, `assets/interactions/README.md`, test `crates/engine/src/tests/interactions.rs`.

**Produces:** field `#[serde(default = "one")] pub sulking: f32`; `weight_for(&self, speaker: Disposition, band: Bond, sulks: bool) -> f32`; `pick(db, speaker, band, sulks: bool, gossip_ok, seed)` passes it through. `note_interactions` passes `self.sulks(speaker)`. Values per spec §3 table (`commiserate` omitted = 1.0).

**Tests:** `weight_for` multiplies `sulking` only when `sulks`; `schema_fault` rejects negative and NaN/inf `sulking`; over a fixed seed sweep (same seeds both sides) a sulking speaker picks `slight`/`insult` strictly more often than unsulking; **RF4:** a db where every def has `sulking: 0.0` → `pick(…, sulks=true, …)` is `None`.

- [ ] failing tests → implement → green → fmt/clippy → commit

### T3: negative gossip

**Files:** `crates/engine/src/game/memories.rs` (`tellable` :633, caller :537), test `tests/interactions.rs` (or wherever existing `tellable`/gossip tests live — `rg -n "tellable|heard_ill_of" crates/engine/src/tests`).

**Produces:** `tellable(&self, speaker, speaker_id, listener_id, sulker: bool)`; when `sulker`, candidates restricted to defs with `valence < 0.0`, ordering/tie-break unchanged. Caller passes `self.sulks(speaker)`.

**Tests:** sulker with stronger positive + weaker negative memory tells the negative; sulker with only positives tells nothing (`gossip_ok` false); non-sulker still tells the stronger positive.

- [ ] failing tests → implement → green → fmt/clippy → commit

---

## Phase 3 — freezing out (§4)

### T4: no `idled_with` between avoiders

**Files:** `crates/engine/src/game/base/offshift.rs` (`note_idling` :413, the `company` filter chain), test file holding existing `idled_with` tests (`rg -n idled_with crates/engine/src/tests`).

**Change:** add `.filter(|&&other| other id → !self.bond(worker, id).avoids())` to `company` (needs the other's `ProgramId`; filter after the `filter_map` that yields it). Applies to every program, not just sulkers.

**Tests:** rivals sharing an amenity write no `idled_with`; a neutral pair still does; a fond pair still does.

- [ ] failing tests → implement → green → fmt/clippy → commit

### T5: sulker refuses a post beside a rival

**Files:** `crates/engine/src/game/base/morale.rs` (`refuses_post`), test `tests/respite.rs` or `tests/sulking.rs`.

**Consumes:** `resents_structure`, `is_beside` (T1). **Change:** after the `== Sulking` gate and `Excavate` exemption, refuse if `resents_structure(…)` **or** any other staff body that `party::walks_the_base(Some(ProgramRole::Staff), task_kind)` (the filter `situations::assess` uses) with `is_beside(its pos, post pos)` and `self.bond(worker, its id).avoids()`. Keep `!= Sulking` (spec §2).

**Tests:** sulker refuses with an avoided body beside the post, accepts once it moves away; non-sulker accepts; `Excavate` still never refused; **RF5:** an avoided program beside the post that does not walk the base does not cause refusal.

- [ ] failing tests → implement → green → fmt/clippy → commit

---

## Phase 4 — petty sabotage (§5)

### T6: plumbing — consume source, constants, memory def

**Files:** `crates/engine/src/base_ledger.rs` (`ConsumeSource` :186, `as_str`), `crates/engine/src/tuning.rs` (near `BOND_WITNESS_REACH` :4505), `assets/memories/saw_sabotage.ron`, `assets/memories/README.md` (if it lists defs), `crates/engine/src/tests/assets.rs` (`MEMORY_TRIGGERS` :2957).

**Produces:** `ConsumeSource::Sabotage` → `"sabotage"` (doc comment: spoiled by a sulking program); `SABOTAGE_REACH: i32 = 1`, `SABOTAGE_CHANCE: f64 = 0.25` (match `unit`'s return type), `SABOTAGE_SALT: u64` fresh odd word, `const _: () = assert!(SABOTAGE_CHANCE > 0.0 && SABOTAGE_CHANCE <= 1.0);`. Memory def fields per spec §5 table (copy an existing Program-subject def, e.g. `turned_on_me.ron`, for shape). `MEMORY_TRIGGERS` row `("saw_sabotage", K::Program)` commented `Game::note_sabotage`.

**Tests:** existing census tests (`MEMORY_TRIGGERS`, `spreads_as` rule, `known_for` width) go red until the row is added — that is the failing step. Check any exhaustive `ConsumeSource` test (`rg -n "ConsumeSource::" crates/engine/src/tests/base_ledger.rs`) covers the new variant.

- [ ] add asset → census red → add row → green → fmt/clippy → commit

### T7: `Game::note_sabotage`

**Files:** new `crates/engine/src/game/base/sabotage.rs` (register in `game/base/mod.rs`), `crates/engine/src/game/turn.rs` (call after `note_interactions` :355, with a one-line *why* comment like its neighbours), new `crates/engine/src/tests/sabotage.rs` (register in `tests/mod.rs`).

**Consumes:** `sulks`, `resents_structure` (T1); T6's constants and variant; `hauling::take_from(stock, item, qty) -> u32` (`hauling.rs:55`); `note_consumed(item, qty, source)` (`telemetry.rs:53`); `log_base`; `creature_label`; `remember(entity, def_id, subject)`; `bond(holder, about).avoids()`.

**Algorithm:** exactly spec §5 steps 1–6, gate copied from `note_interactions` (:489-495: `now.is_multiple_of(INTERACTION_PERIOD)` + base established — reuse its predicate, don't re-derive). Per body in `ProgramId` order; **build candidates per body against live stock** (RF1); consume only the qty `take_from` returned and skip log/witness if 0. Witnesses: base staff within `BOND_WITNESS_REACH` of the machine, excluding the saboteur (RF2), loop shaped like `close_brawl`'s (`tantrum.rs:118`). Names: item/structure def name, falling back to the id string (RF3).

**Tests** (use a `dev-saves/` template or the base fixtures in `tests/support.rs`; find a seed/tick that rolls under the chance by iterating ticks in the test, not by hardcoding):
- idle sulker beside a resented machine with output → one unit gone, ledger `Sabotage` consume of 1, one base-log line with the spec's wording, `saw_sabotage` on an in-reach witness and not an out-of-reach one;
- same with an amenity present (sulker on respite) — reachability;
- rival-worked machine (not resented) is a candidate;
- negatives: non-sulker, sulker with a `Task`, empty output, machine neither resented nor rival-worked → nothing;
- RF1 two sulkers, output qty 1; RF2 no self-memory; RF3 missing def name falls back;
- `rng_unadvanced_by` (`tests/support.rs:2888`) across a spoiling tick;
- empty memory catalogue: still spoils and logs.

- [ ] failing tests → implement → green → fmt/clippy → commit

---

## Final gate

- [ ] Mutation-check (fix committed first): revert each of T2/T4/T5/T7's core line once and confirm its test fails.
- [ ] `cargo test --workspace` (exit code not through a pipe); `cargo test -p feral-processes-engine balance_sim` unchanged.
- [ ] `docs/superpowers/INDEX.md`: spec row → built, roadmap row G → built.
- [ ] Whole-branch review (opus), diff given as a file. CHANGELOG/version/tag happen at landing on `main`, not here.
