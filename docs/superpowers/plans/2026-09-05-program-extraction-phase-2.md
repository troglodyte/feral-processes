# Program Extraction — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Tools become something the player *earns*. Research teaches a
tool, a forge turns that knowledge into a carrier, installing burns the
carrier into a slot, and the slots grow with level. One screen shows the kit.

**Architecture:** The routine acquisition chain, mirrored rung for rung onto
tools — spec decision 6. Phase 1 already shipped the catalogue (`ToolDb`),
the component (`components::Tools`), the slot formula
(`tools::player_tool_slots`) and the save field (`PlayerSave::tools`); this
phase fills the four gaps `tools.rs`'s own module doc enumerates, and nothing
else.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, serde.

**Spec:** `docs/superpowers/specs/2026-09-04-program-extraction-design.md` —
read sections 2 and 6 first. This plan argues from its ten numbered
decisions and does not restate them.

**Branch:** `feat/extraction-kit`, cut from `main` at 529be8ab (v0.13.99).

## Global Constraints

- **No `save::SAVE_FORMAT_VERSION` bump.** `known_tools` is additive behind
  `#[serde(default)]`. Note that `SaveData::known_routines` (`save.rs:1110`)
  is **not** defaulted — it is a base field from save version 21 — so do not
  copy that detail across; the tool field is new and must default.
- **No content in Rust.** A tool is `assets/tools/*.ron` and a research
  unlock is a field in `assets/research/*.ron`. Update the matching
  `assets/*/README.md` in the same change as any schema field.
- **Tuning values go in `crates/engine/src/tuning.rs`** as documented `pub
  const`, never inline in a formula.
- **Every refusal lands before anything is spent**, asserted **per refusal** —
  one test over one path passes against all the others.
- **Uppercase letters only for screen actions.** Lowercase are row selectors;
  a lowercase action makes one keypress both pick a row and fire it.
- Follow the repo's comment discipline: comments say *why*, never *what*.
- Gates for every task: `cargo fmt`, `cargo clippy --workspace` (no new
  warnings), `cargo test -p feral-processes-engine <name>` while iterating,
  and `cargo test --workspace` before the task's commit.
- The full plan is TDD: the failing test is written and *seen to fail*
  before the implementation.
- **Do not push.** Commit freely on the branch; the merge and the release are
  the user's call.

## Decisions this plan makes, that the spec left open

1. **Forging is priced per tool, on the def** — `ToolDef::forge_cost:
   Vec<(ItemId, u32)>`, `#[serde(default)]`. Routines spend a flat blank
   Routine Disk because every routine is the same object; tools differ by
   tier, so a tier-2 Core Tap must be able to cost more than the starter
   clamp. Rejected: spending a flat count of the existing `blank_substrate`
   (one price for every tier), and a new Tool Chassis blank item (a second
   feedstock chain to stock for no expressive gain over a cost list).
2. **The shipped kit is one tool per non-`Routines` category** — the two
   already shipped plus one Parts tool. A tier ladder waits until the loop
   has been played; every yield weight in it is currently a guess.
3. **`Mode::Tools` lists every *known* tool, not just the filled slots.** The
   spec calls it "the slots", but a screen that hides a tool you have
   researched and not yet forged gives the forge verb nowhere to live. The
   header carries slots used against `player_tool_slots(level)`.

---

### Task 1: Knowledge — `unlocks_tools` and `KnownTools`

**Files:**
- Modify: `crates/engine/src/research.rs` — `ResearchDef::unlocks_tools:
  Vec<ToolId>` beside `unlocks_abilities` (`:64-65`), and the load-time
  validation that mirrors `:104-113` exactly — an unknown id is dropped with
  a warning and **the node survives**, unlike an unknown `requires` or
  `unlocks_structures`, which drops the whole node
- Modify: `crates/engine/src/resources.rs` — `KnownTools(pub
  BTreeSet<ToolId>)` beside `KnownRoutines` (`:71-81`); `BTreeSet` for
  `KnownRoutines`' stated reason, deterministic save bytes
- Modify: `crates/engine/src/lib.rs` — re-export beside `KnownRoutines` (`:100`)
- Modify: `crates/engine/src/game/lifecycle.rs` — insert the resource on both
  paths, beside `KnownRoutines` at `:240` (new game) and `:506` (load), and
  write it out at `:1968-1974` (save)
- Modify: `crates/engine/src/save.rs` — `SaveData::known_tools:
  Vec<ToolId>`, `#[serde(default)]`
- Modify: `crates/engine/src/game/unlocks.rs` — `Game::unlock_research`
  (`:331-384`) teaches each `unlocks_tools` id after the abilities loop,
  logging only on a fresh insert, the ability arm's rule
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces produced:**
- `resources::KnownTools(pub BTreeSet<ToolId>)`
- `ResearchDef::unlocks_tools: Vec<ToolId>`
- `Game::knows_tool(&self, id: &ToolId) -> bool` — the one read, the
  `knows_routine` analog; every later gate calls it rather than touching the
  resource

**Steps:**

- [ ] **Test first**, four: an unknown tool id in `unlocks_tools` is dropped
  at load and the node survives (the `an_unknown_ability_id_is_dropped_but_
  the_node_survives` analog at `research.rs:288-310`); unlocking a node
  teaches its tools and logs once; unlocking it twice logs once; a
  **save→load** round trip preserves a set of known tools. The round trip
  must be save→load and not a RON round trip — a RON round trip cannot catch
  a `#[serde(skip)]`.
- [ ] Implement. `Game::new` starts with `KnownTools` **empty**: the starter
  tool is granted straight into the slot at `lifecycle.rs:150` and knowledge
  is not what put it there. Whether the starter is also *known* is a real
  fork — leave it out, and let task 3's "already installed" refusal be what
  stops a duplicate.
- [ ] Gates, then commit: `feat(extraction): research teaches a tool`

---

### Task 2: The forge

**Files:**
- Modify: `crates/engine/src/items.rs` — `ItemId::tool(id)` and its inverse
  `tool_id`, beside `etched`/`etched_ability` (`:41-51`), with a
  `TOOL_ITEM_PREFIX` constant beside `ETCHED_DISK_PREFIX` (`:57`). A
  synthetic id has no `.ron` behind it, so no item file ships.
- Modify: `crates/engine/src/tools.rs` — `ToolDef::forge_cost:
  Vec<(ItemId, u32)>`, `#[serde(default)]`
- Modify: `crates/engine/src/game/extraction.rs` (or a sibling in
  `crates/engine/src/game/`) — `Game::forge_tool`
- Modify: wherever `LootSource` lives — a `Forge` variant, the `Etch`
  variant's sibling. **Check whether `LootSource` is matched exhaustively
  anywhere** (the ledger's provenance split is the likely reader) before
  assuming a variant is free.
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces produced:**
- `Game::forge_tool(&mut self, tool: &ToolId) -> Result<(), String>`

**Steps:**

- [ ] **Test first.** One test **per refusal**, each asserting the materials
  are still held and no carrier was granted: game-over or an active battle;
  an id `ToolDb` cannot resolve; a tool that is not in `KnownTools`; a cost
  the player cannot pay. Then the success case: exactly one
  `ItemId::tool(id)` is granted and exactly the def's cost leaves
  `Inventory`.
- [ ] Implement in `etch_disk`'s order (`game/routines.rs:302-344`): every
  refusal, then spend, then `note_consumed(ConsumeSource::Craft)`, then
  `grant_loot(..., 1, LootSource::Forge)`, then the log line. Forging
  requires **no structure** — `etch_disk` requires none either, and spec
  decision 7 keeps the whole feature structure-free until phase 3.
- [ ] Gates, then commit: `feat(extraction): forging a tool carrier`

---

### Task 3: Install, uninstall, and the slot cap

**Files:**
- Modify: the same `game/` module as task 2 — `Game::install_tool`,
  `Game::uninstall_tool`
- Modify: `crates/engine/src/components.rs` — the `Tools` doc comment at
  `:562-571` currently says the cap is not enforced anywhere; that stops
  being true here
- Test: `crates/engine/src/tests/extraction.rs`

**Interfaces produced:**
- `Game::install_tool(&mut self, tool: &ToolId) -> Result<(), String>`
- `Game::uninstall_tool(&mut self, slot: usize) -> Result<(), String>`

**Steps:**

- [ ] **Test first**, one per refusal for install, each asserting the carrier
  is still held and the slot list is unchanged: game-over or battle; an
  unresolvable id; already installed; no free slot
  (`installed.len() >= tools::player_tool_slots(level)`); no carrier held.
  Then: a successful install writes the slot and burns the carrier
  (`ConsumeSource::Install`); the slot count grows at the level
  `TOOL_SLOT_PER_LEVEL` names and stops at `TOOL_SLOT_CAP`.
- [ ] **Test first** for uninstall: it frees the slot and **hands nothing
  back** — what is in the slot *is* the tool, `install_disk`'s rule and spec
  section 2. A test that asserts the carrier is *not* granted is the one that
  holds this; without it a later "fix" reads as generosity.
- [ ] Implement, mirroring `install_disk` (`game/routines.rs:357-392`). The
  player is the only tool holder, so there is no entity argument and no
  `owns_routine_holder` rung.
- [ ] Gates, then commit: `feat(extraction): installing and pulling a tool`

---

### Task 4: The shipped kit and its censuses

**Files:**
- Add: `assets/tools/<parts tool>.ron` — `category: Parts`, yields drawn from
  real part items (`logic_wafer`, `packet_buffer`, `static_mesh`,
  `charge_coil` are the shipped intermediates; pick two and say in a comment
  that the weights are untuned, `core_tap.ron`'s comment is the model)
- Modify: `assets/tools/salvage_clamp.ron`, `assets/tools/core_tap.ron` —
  `forge_cost` on each. The starter's cost is what a *replacement* costs;
  it does not gate the one `Game::new` grants.
- Modify: `assets/research/program_refactoring.ron` — `unlocks_tools` naming
  the Parts tool (zone 2, cost 75; pulling a program apart is the node's own
  subject)
- Modify: `assets/research/deep_analysis.ron` — `unlocks_tools: ["core_tap"]`
  (zone 3; "draws the compiled core out" is that node's subject). Both
  placements are a proposal — say so if a different node reads better.
- Modify: `assets/tools/README.md`, `assets/research/README.md` — the two new
  schema fields
- Test: `crates/engine/src/tests/assets.rs`

**Steps:**

- [ ] **Test first**, three censuses beside the five tool censuses already at
  `tests/assets.rs:3289-3422`: every `unlocks_tools` id across `ResearchDb`
  resolves to a shipped tool; every `forge_cost` item id resolves in
  `ItemDb`; and the reachability census — **every shipped tool other than
  `STARTER_TOOL_ID` is named by some research node's `unlocks_tools`**, the
  `every_shipped_field_routine_can_actually_be_obtained` analog at
  `tests/assets.rs:1268-1305`. That third one is what stops a tool shipping
  with no door into the game.
- [ ] Author the content. Do **not** ship a `Routines`-category tool — the
  routine branch is phase 3, and shipping one now makes
  `every_non_routines_tool_has_a_non_empty_yield_pool` (`:3321-3354`) start
  exercising an exclusion nothing implements.
- [ ] Gates, then commit: `feat(extraction): the shipped tool kit`

---

### Task 5: The `Mode::Tools` screen

**Files** — the six the one phase-1 screen touched, and the checklist here:
- Modify: `crates/app-core/src/lib.rs` — the `Mode::Tools` variant beside
  `DownedPrograms` (`:1361-1370`), **and its arm in `is_battle`'s exhaustive
  `=> false` branch** (`:1753`)
- Modify: `crates/app-core/src/app/input.rs` — the dispatch line (`:245` is
  `DownedPrograms`')
- Add: the key handler — a flat list, so copy only the *list-page* half of
  `crates/app-core/src/app/extraction.rs` (`:19-69`), never its
  `pending_downed_program_index` drill-down. Row count is re-read from the
  engine per keypress and never cached.
- Modify: the party menu's row builder — `party_menu_rows` must be the only
  source of the row, and the key that opens it must be free there
- Modify: `crates/gui/src/render/mod.rs` — the draw arm (`:1136-1138` is
  `DownedPrograms`') **and `ALL_MODES`, whose length is a `const` count
  (`:1358`) as well as a list**. Another branch adding a `Mode` merges
  cleanly in the entries and not in the count; `feat/settlement-aid` is live
  right now, so expect that conflict at the merge.
- Add: `crates/gui/src/render/tools.rs` (or a section of `extraction.rs`) —
  the draw, `PopupSize::Large`
- Modify: `crates/engine/src/views.rs` — the row derivation. **The engine
  owns the row count and gui draws it**; any per-row transform lives in the
  engine, the `message_history` rule.

**Interfaces produced:**
- `Game::tool_rows() -> Vec<ToolRow>` — one row per **known** tool plus any
  installed tool, carrying name, category, tier, ticks, the slot it occupies
  if any, and how many carriers are held. Every figure is a call; nothing is
  re-derived in the renderer.

**Steps:**

- [ ] **Test first**, in `crates/gui/src/render/`: the tallest shipped tool
  list fits its popup at 1280x720, and no row overflows the popup body at
  1280x720. Build the worst case by deriving the widest shipped tool name and
  category label from the catalogue rather than hardcoding — the phase-1
  fixtures at `render/extraction.rs:176-193` are the pattern. Verify the
  height test by mutation: it must fail when a row is added.
- [ ] **Test first**, in app-core: the row count the screen navigates equals
  `Game::tool_rows().len()`; `Esc` returns to the party menu; `F`, `I` and
  `X` reach `forge_tool`, `install_tool` and `uninstall_tool`; a refusal from
  any of the three lands on the screen's own refusal line exactly once.
- [ ] Implement. Actions are **uppercase** (`F` forge, `I` install, `X`
  uninstall); lowercase stays row selection. The screen has **no scroll**, so
  its height is a layout constraint.
- [ ] Join `ALL_MODES` only — **not** `needs_status_banner`. The spec's
  section 6 amendment of 2026-09-04 is explicit: a popup screen has a refusal
  slot of its own, and adding one to that allowlist makes
  `every_screen_draws_a_refusal_exactly_once` paint the refusal twice.
- [ ] Gates, then commit: `feat(extraction): the tools screen`

---

### Task 6: Documentation and the release

**Files:**
- Modify: `CHANGELOG.md` — a new `## X.Y.Z` section; the digit is decided by
  `CHANGELOG.md`'s own preamble. No save-format break here, so a minor at
  most. Write the heading against `origin/main`'s version, not the newest
  local tag.
- Modify: `Cargo.toml` — the workspace version bump, **at the merge, not on
  the branch**
- Modify: `CLAUDE.md` and `AGENTS.md` — one sentence per new seam. They are
  gitignored twins with no tracking to catch drift, so edit `CLAUDE.md` then
  `cp CLAUDE.md AGENTS.md`. **They cannot ride a branch out of a worktree** —
  land these in the primary checkout or hand them back.
- Modify: `docs/seams.md` and `.claude/skills/seams/` — the argument and the
  trap behind each new seam
- Do **not** touch `docs/manual.md` or the root `README.md`; both are carved
  out of the doc obligation. `assets/help/` is **not** carved out, but no
  help page covers extraction at all yet — that is its own piece of work, not
  a line bolted onto this one.

**Steps:**

- [ ] Three seam sentences: research knowledge, the forge and the install are
  three nouns and one chain; `forge_tool`/`install_tool` are the only writers
  of a slot after `Game::new`; a tool's price is on its def, so a tier can
  cost what it is worth.
- [ ] `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
- [ ] Commit: `docs(extraction): phase 2's seams`

---

## Not in this phase

Named so an implementer does not build them speculatively: the
`extracts_programs` structure and its tier scaling, the `Routines` tool
category and its unification with `extract_routine`, `Sortie::programs`, the
bulk work-order path, and gear drops moving behind extraction. Spec §8 has
their phases.
