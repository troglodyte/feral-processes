# Routine Research Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move routines out of the base research tree into a tree derived
from `AbilityDb`. The player discovers a family by extracting any of its
rungs, then researches it one rung at a time.

**Architecture:** Routine nodes are synthesised `ResearchDef`s tagged
`ResearchTree::Routines`, and the existing Research Node project machinery
runs them. For a routine node, "researched" means the routine is in
`KnownRoutines`. One visibility filter feeds both `research_nodes` and
`research_graph`. Extraction writes the new `DiscoveredRoutines` resource
instead of teaching. Concealment is done in the engine, and gui only draws
the `?`.

**Tech Stack:** Rust, standalone `bevy_ecs` engine, app-core state machine,
Bevy + egui renderer through `Painter`.

**Spec:** `docs/superpowers/specs/2026-09-16-routine-research-tree-design.md`.
Read it in full before any task. It is the argument, and this plan only
cuts it into tasks.

## Global Constraints

- Branch `feat/routine-research-tree`. **Never push, never merge, never
  tag.** Commit per green task with explicit paths (never `git add -A`).
- **No `git checkout`/`stash`/`reset` of work you did not make.**
- TDD: write the failing test, watch it fail for the right reason, then
  implement. For every behavioural test, confirm it fails with the fix
  removed.
- After every task, run `cargo fmt`, `cargo clippy --workspace --all-targets`
  (clean) and the task's targeted tests. At each **phase boundary**, also run
  `cargo test --workspace`. Cargo's exit code is lost through `| tail`, so
  check it with `; echo $?` or `set -o pipefail`.
- Every new asset field is `#[serde(default)]`. Every schema change updates
  the matching `assets/*/README.md` in the same task.
- New save fields use `#[serde(default)]` and need a **save→load** test. The
  RON round-trip alone cannot catch a skipped field.
- Synthesised ids are exactly `"routine/<ability_id>"`.
- Refusals land before anything is spent, with one test per refusal.
- Tuning constants go in `crates/engine/src/tuning.rs`, and every curve is
  linear.
- `render/` draws only through `Painter`. A new `Mode` is added to
  `ALL_MODES` by hand.
- Player-facing strings, verbatim:
  - *"Recovered an unfamiliar routine — see routine research."*
  - *"nothing unfamiliar to recover"*
  - *"that routine is already familiar"*
  - *"Recover routines from downed programs to open research here."*
  - *"??? — a routine you haven't seen"*
  - menu row *"Routine research"*
- Do not edit `docs/manual.md`, the root `README.md` or `TODO.md`.
  `CHANGELOG.md` is written at the merge, not here.

## File map

| File | Responsibility in this change |
|---|---|
| `crates/engine/src/routine_tree.rs` (new) | family / scope rank / version parse; which abilities get a node; prerequisite derivation; synthesis of `ResearchDef`s |
| `crates/engine/src/research.rs` | `ResearchTree`, `teaches`, `opens_routine_tree`; drop `unlocks_abilities`; call synthesis in `load_dir` |
| `crates/engine/src/abilities.rs` | `AbilityDef::research_zone` |
| `crates/engine/src/tuning.rs` | `routine_research_cost` constants |
| `crates/engine/src/resources.rs`, `save.rs` | `DiscoveredRoutines` and `discovered_routines` |
| `crates/engine/src/game/unlocks.rs` | researched-means-known helper, visibility filter, tree argument, select refusal, settle |
| `crates/engine/src/game/routines.rs`, `game/extraction.rs` | discovery instead of teaching; candidates; concealment in `routine_view` |
| `crates/engine/src/game/combat_damage.rs` | death line concealment |
| `crates/engine/src/views.rs` | `RoutineSlotView::unseen`, `EntityView::unseen_routine`, `ExtractionPreview::Routine` shape |
| `crates/engine/src/game/lifecycle.rs` | load clears a stale `active_research` |
| `assets/research/*.ron`, `assets/abilities/*.ron` | delete nine nodes, re-point `deep_analysis`, move `routine_reader`, flag `routine_fabrication`, set `research_zone` |
| `crates/app-core/src/...` | `Mode::RoutineResearch`, menu row, key handling (mirror `Mode::Research`) |
| `crates/gui/src/render/mod.rs`, `render/research*`, `render/base.rs`, `gui/src/lib.rs` | draw the new mode; Alt held state; `?` |
| tests | `crates/engine/src/tests/routine_tree.rs` (new), `tests/research.rs`, `tests/extraction.rs`, `tests/routines.rs`, `tests/assets.rs`, `tests/save*.rs` |

Anchors from the 2026-09-16 review (line numbers drift, so search by name):
- `unlocks.rs`:
  - `is_researched` 228, `missing_prereqs` 234
  - `research_nodes` 351, `research_graph` 445 (Kahn at 454)
  - `grant_research_knowledge` 534, `has_research_tree` 649
  - `select_research` 667, `settle_research` 812
- `research.rs` `load_dir` 106. `lifecycle.rs` `load_asset_dbs` ~2673–2708
  loads abilities, then species, then research, then the etched-disk
  synthesis.
- `routines.rs`:
  - `RoutineTaken` 13, `routine_view` 123, `knows_routine` 200
  - `routine_is_permanent` 238, `extractable_routines` 567
  - `extract_routine` 624 (the refusal at ~648), `take_routine` ~682
- `extraction.rs`: `routine_candidates` 321, the preview at ~546.
- `combat_damage.rs` `announce_program_death` 438.
- `views.rs`:
  - `ResearchState` 123, `ResearchGraph::step` 201, test helper at 77
  - `EntityView` 986, `RoutineSlotView` 1858, `ExtractionPreview` 3086
- `tests/assets.rs`:
  - `without_version_tag` 936, name/scope census 966
  - `family` 1062, `scope_rank` 1071
  - field-routine obtainability 1557
- `tests/research.rs`: `no_research_node_is_left_unlocking_nothing` 991, and
  the `unlocks_abilities` use at 668. `tests/routines.rs` 621.
- `gui/src/lib.rs`: `SHIFT_KEYS`/`CTRL_KEYS` 91. `render/base.rs`:
  `ConRead` 80, earmark 1037. `render/mod.rs`: `ALL_MODES` 1452, the
  Research draw at 1360. `app-core/src/app/group_menu.rs`: Research row
  163. `app-core progression.rs` 114 (graph stepping).

---

## Phase 1 — Engine

### Task 1: The family / rung derivation

**Files:** Create `crates/engine/src/routine_tree.rs` (register it in
`lib.rs`). Modify `tests/assets.rs`. Test in
`crates/engine/src/tests/routine_tree.rs` (new, registered in `tests/mod.rs`).

**Interfaces — Produces:**
- `pub fn family(def: &AbilityDef) -> String`. Moved from `tests/assets.rs`
  together with its `scope_word` and `without_version_tag` helpers.
- `pub fn scope_rank(target: AbilityTarget) -> u8`, read off the target as
  the test does today.
- `pub fn version(name: &str) -> (u32, u32)`. New. A name with no tag is
  `(1, 0)`.
- `pub fn gets_node(abilities: &AbilityDb, def: &AbilityDef) -> bool`. False
  for exclusive, permanent (the same predicate `Game::routine_is_permanent`
  uses — extract it to a free function if that method needs `Game`),
  passive, and `Summon`.
- `pub fn routine_prereq(abilities: &AbilityDb, def: &AbilityDef) -> Option<AbilityId>`,
  following spec §2 "Prerequisites".

**Tests:**
- `version` orders `v1.1` after `v1.0` and `v2.0` after `v1.1`, and returns
  `(1, 0)` for an untagged name.
- `gets_node` is false for `decompile`, for each of the six passives named in
  the spec, for both Fork summons, and for an exclusive ability. It is true
  for `hot_patch`.
- Against the shipped `AbilityDb`, the Patch family gives: Single v2 → Single
  v1, Party v1 → Single v1, Party v1.1 → Party v1.0. The Single v1 root has
  no prerequisite.
- Using an in-test `AbilityDb`, a family with only Group rungs has a
  parentless Group root.

**Steps:**
- [ ] Write the tests, then run
  `cargo test -p feral-processes-engine routine_tree` and see them fail to
  compile or fail.
- [ ] Implement. Make
  `every_battle_ability_family_is_contiguous_from_single_upward` and the
  name/scope census call these functions, and delete the test-local copies.
- [ ] Run the tests above plus `cargo test -p feral-processes-engine assets`,
  and confirm they pass.
- [ ] Commit.

### Task 2: Synthesised nodes and the new schema fields

**Files:**
- Modify:
  - `research.rs`
  - `abilities.rs`
  - `tuning.rs`
  - `routine_tree.rs`
  - `assets/research/README.md`
  - `assets/abilities/README.md`
- Test: `tests/routine_tree.rs` and `tests/assets.rs`.

**Interfaces:**
- **Consumes:** Task 1.
- **Produces:**
  - `ResearchTree { Base, Routines }`, with `Base` as the `Default`;
  - on `ResearchDef`:
    - `pub tree: ResearchTree`,
    - `pub teaches: Option<AbilityId>`,
    - `pub opens_routine_tree: bool`, all serde-default;
  - `AbilityDef::research_zone: u32`, serde-default, where 0 reads as 1;
  - `tuning::routine_research_cost(scope_rank: u8, version: (u32, u32)) -> u32`;
  - `routine_tree::synthesise_nodes(abilities: &AbilityDb) -> Vec<ResearchDef>`,
    appended inside `ResearchDb::load_dir`. Its nodes have:
    - no `materials`;
    - `requires` holding only the prerequisite rung's synthesised id;
    - `min_zone` from `research_zone`;
    - `name` equal to the ability's display name.

**Cost fitting:** read `cost` on the eleven routine-granting nodes in
`assets/research/`. Fit a linear
`BASE + SCOPE_STEP * scope_rank + VERSION_STEP * (major - 1)` so that the
first rung of each moved routine lands near its old node's cost. Put the
table you fitted against in the doc comment on the constants.

**Tests:**
- The census **every ability for which `gets_node` is true has exactly one
  node with id `routine/<id>` and `teaches == Some(id)`, and no other node
  teaches** runs over the real assets.
- A synthesised node's `tree` is `Routines`, and a `.ron` node's tree is
  `Base`.
- `min_zone` follows `research_zone`, and an ability with no
  `research_zone` gives zone 1.
- `routine_research_cost` is monotonic in both arguments.

**Steps:** failing tests → implement → targeted tests green → commit. The
nine old nodes still exist at this point, so routines are granted twice.
Task 3 removes the duplicate.

### Task 3: Researched means known, and the base tree loses its routines

**Files:**
- Modify:
  - `game/unlocks.rs`
  - `research.rs` (delete `unlocks_abilities` and fix its stale doc)
  - `views.rs` (test helper)
  - `assets/research/`:
    - delete the nine nodes;
    - `deep_analysis.requires` → `routine_fabrication`;
    - `routine_fabrication`: `opens_routine_tree: true`, plus
      `unlocks_tools: ["routine_reader"]`;
    - `cortex`: drop `routine_reader`;
  - `assets/abilities/`: set `research_zone` per the spec §2 table.
- Tests to rewrite:
  - `tests/research.rs`: the use at 668, plus
    `no_research_node_is_left_unlocking_nothing`, which now counts
    `unlocks_tools`;
  - `tests/assets.rs`: 663, plus the field-routine obtainability census,
    whose research source is now the synthesised node;
  - `tests/routines.rs` 621;
  - the `research.rs` unit tests.

**Interfaces — Produces:**
- `Game::node_researched(&self, def: &ResearchDef) -> bool`, the one
  helper. For a node with `teaches`, it answers from `KnownRoutines`
  (§1). Otherwise it reads `Research`.
- A prerequisite rung is met when it is known, or when any **higher version
  at the same scope** in its family is known. That is a
  `routine_tree::rung_satisfied(abilities, known, prereq_id) -> bool`.
- `is_researched`, `missing_prereqs` and `select_research`'s
  already-researched refusal all route through the helper.
- `settle_research` inserts into `KnownRoutines`, and not `Research`, for a
  `teaches` node.
- `grant_research_knowledge` no longer grants abilities.

**Tests:**
- Completing `routine/<id>` inserts into `KnownRoutines`, leaves `Research`
  untouched, and the node reads `Unlocked`.
- A game that knows a routine (with nothing researched) reports its node as
  researched.
- Knowing Patch Single v2.0 satisfies the Patch Party v1.0 prerequisite.
- No base node teaches.
- The nine ids are absent.
- `deep_analysis` requires `routine_fabrication`.
- `routine_reader` is unlocked by `routine_fabrication`.
- The zone-gate table is covered by one assertion per row.

**Steps:** failing tests → implement → run
`cargo test -p feral-processes-engine research routines assets` → commit.
Watch the materials census, "a bill names only what its prerequisites
make", on `deep_analysis`. The review says it stays green.

### Task 4: `DiscoveredRoutines` and discoverability

**Files:** Modify `resources.rs`, `save.rs`, `routine_tree.rs`, and
`game/routines.rs` (or `unlocks.rs`). Test in `tests/routine_tree.rs` and
the save tests.

**Interfaces — Produces:**
- `resources::DiscoveredRoutines(pub BTreeSet<AbilityId>)`, inserted at
  `Game::new`, saved as `discovered_routines: Vec<AbilityId>` with
  serde-default, and restored on load.
- `Game::family_discovered(&self, family: &str) -> bool`, true when any rung
  is in `DiscoveredRoutines` or in `KnownRoutines`.
- `Game::family_is_discoverable(&self, family: &str) -> bool`, computed at
  query time from `AbilityDb` (`wild_weight > 0`) and `SpeciesDb` (kits).
  `ResearchDb` has no `SpeciesDb`.

**Tests:**
- A save→load round trip preserves `DiscoveredRoutines`.
- A pre-change save (no field) loads with an empty set.
- Knowing a rung makes its family discovered.
- Hyperthread is discoverable.
- A field-routine family is not discoverable.
- The census: every discoverable family has at least one carried rung.

**Steps:** failing tests → implement → green → commit.

### Task 5: Visibility, the tree argument, and the gate

**Files:**
- Modify:
  - `game/unlocks.rs`
  - `views.rs`, if `ResearchGraph` needs it
  - every caller of `research_nodes` / `research_graph` /
    `has_research_tree` in app-core and gui, which pass
    `ResearchTree::Base` for now so the workspace still builds
- Test: `tests/research.rs` or `tests/routine_tree.rs`.

**Interfaces:**
- **Consumes:** Tasks 2–4.
- **Produces:**
  - `Game::research_nodes(&self, tree: ResearchTree)`,
    `Game::research_graph(&self, tree: ResearchTree)` and
    `Game::has_research_tree(&self, tree: ResearchTree)`;
  - `fn listed_research(&self, tree) -> Vec<&ResearchDef>`, the one filter
    both functions call (§2 "Visibility");
  - `Game::routine_tree_open(&self) -> bool`: some researched node carries
    `opens_routine_tree`, or no loaded node carries it.

**Rules to implement exactly:**
- The tree is closed → nothing is listed.
- An always-visible family → listed once the zone gate is met.
- A discoverable family → listed if the family is discovered **and** its
  prerequisite rung is satisfied. Zone does not hide the node; it shows as
  `Locked { min_zone }`.
- The graph layout ignores any `requires` entry that is not itself listed.
- `select_research` refuses a routine node that is not listed, using the
  unknown-id text. While the tree is closed, it refuses with
  "research <name> first".

**Tests (one each):**
- An undiscovered family lists nothing.
- Discovering one rung lists only the root.
- Researching the root lists exactly two children.
- An always-visible family lists its nodes at the right zone and none below
  it.
- A discovered Patch in zone 1 lists `hot_patch` as `Locked { min_zone: 2 }`.
- A known Party v1.0 with a hidden parent still gets a graph cell and is
  reachable through `ResearchGraph::step`.
- The closed tree lists nothing and `select_research` refuses.
- With no flagged node loaded, the tree is open (use an in-test
  `ResearchDb`).
- `select_research` refuses an unlisted node, and nothing is filed.
- The Base tree's output is unchanged apart from the deleted nodes.

**Steps:** failing tests → implement → targeted tests → **`cargo build
--workspace`** → commit.

### Task 6: Extraction discovers instead of teaching

**Files:** Modify `game/routines.rs` and `game/extraction.rs`. Test in
`tests/extraction.rs`, `tests/routines.rs` and `tests/exclusive_routines.rs`.

**Interfaces — Produces:**
- `RoutineTaken::Learned` is renamed to `Discovered`.
- `take_routine`'s ordinary branch inserts into `DiscoveredRoutines` and
  logs the verbatim line.
- `routine_candidates` and `extractable_routines` filter on
  `!family_discovered`.
- The tool door refuses with "nothing unfamiliar to recover".
- The tamed door's row refusal becomes "that routine is already familiar".
- `pub fn routine_candidate_ids(&self, kit_level, species, carried) -> Vec<AbilityId>`,
  or whatever shared shape lets Task 12 ask the same question of a *live*
  wild creature. Extract it here and have `routine_candidates` call it.

**Tests:**
- Extracting writes `DiscoveredRoutines` and not `KnownRoutines`.
- The log line does not contain the routine's display name.
- Tool door: a program whose families are all discovered is refused, and
  the tool, the program and Power are unchanged.
- Tamed door: a familiar row is refused and nothing is spent.
- The exclusive branch still pops a disk.

**Steps:** failing tests → implement → targeted tests → commit.

### Task 7: A stale active project clears on load, plus docs and seams

**Files:**
- Modify:
  - `game/lifecycle.rs` (load path)
  - `assets/research/README.md`: `opens_routine_tree`, the synthesised
    `routine/*` ids (not authorable), and the removal of
    `unlocks_abilities`
  - `assets/abilities/README.md`: `research_zone`, and how family, scope
    and version are read off the display name
- Seam, three writes in the order the `seams` skill documents (invoke it):
  - the argument to the memory graph as
    `seam:routine-node-is-derived`;
  - the trap to the skill's reference file;
  - one sentence in CLAUDE.md under `### The research tree`.
- Test in the save tests.

**Behaviour:** on load, an `active_research` naming an id no loaded node has
is cleared. So is every work order whose `for_research` names it.

**Tests:**
- A save whose `active_research` is `"field_ops"` with one research work
  order loads with no active project and no such order.

**Steps:**
- [ ] Failing test → implement → green.
- [ ] Docs and seam.
- [ ] **Phase gate:** `cargo test --workspace`, clippy with
  `--all-targets`, `cargo test -p feral-processes-engine balance_sim`.
- [ ] Commit.

---

## Phase 2 — Screen

### Task 8: `Mode::RoutineResearch` in app-core and gui

**Files:**
- Modify:
  - app-core: the `Mode` enum, `group_menu.rs` (new row "Routine research"
    after Research, gated on `has_research_tree(Routines)`), the Research
    key handling in `progression.rs`, and `lib.rs` wherever `Mode::Research`
    is listed (e.g. 2069)
  - gui:
    - `render/mod.rs`: the draw match plus `ALL_MODES`
    - the research menu and graph renderers, which take a `ResearchTree`
      argument
- Tests: app-core and gui.

**Interfaces:**
- **Consumes:** the Task 5 signatures.
- **Produces:**
  - the new mode shares the research handling, parameterised by
    `ResearchTree`. Prefer one handler taking the tree over a copied
    handler;
  - the graph/list toggle works in both modes.

**Tests:**
- app-core:
  - the menu row opens `Mode::RoutineResearch`;
  - arrow and graph stepping in that mode move over routine nodes only;
  - Enter selects a listed routine node.
- gui:
  - with nothing listed, the list view draws the verbatim empty line;
  - `ALL_MODES` includes the mode, and its length test is updated;
  - the base Research screen still draws no `routine/` node.

**Steps:** failing tests → implement → targeted tests → **phase gate**
(workspace suite and clippy) → commit.

---

## Phase 3 — Concealment

### Task 9: Engine concealment

**Files:** Modify `game/routines.rs` (`routine_view`), `views.rs`,
`game/extraction.rs` (preview) and `game/combat_damage.rs`. Update the gui
and app-core readers of the changed view fields. Tests in the engine and
gui.

**Interfaces — Produces:**
- `RoutineSlotView::unseen: bool`.
- When `unseen`, the engine blanks `name` and `description`, and makes the
  ability id unavailable. Change the field to `Option<AbilityId>`, or blank
  it — pick one and fix every reader.
- A slot is unseen only when the holder is **not owned** (not `Tamed`, not
  the player) and its family is not discovered.
- `ExtractionPreview::Routine` carries a count of unfamiliar routines and no
  names.
- `announce_program_death` omits unseen routine names for a wild program.
- The gui inspect renderer draws "??? — a routine you haven't seen" for an
  unseen slot.

**Tests:**
- A wild carrier's slot is unseen and blank.
- The same species tamed shows its real name.
- The preview holds no display name of a candidate.
- The wild death line has no unseen name.
- A gui paint test shows the `???` line.

**Steps:** failing tests → implement → targeted → commit.

### Task 10: The Alt `?` marker

**Files:** Modify `views.rs` (`EntityView::unseen_routine`), the engine's
entity-view builder, `gui/src/lib.rs` (Alt held state beside
`SHIFT_KEYS`/`CTRL_KEYS`, passed as `reveal: bool` into `render::draw`), and
`render/base.rs`. No new `GameKey`.

**Behaviour:**
- `unseen_routine` is true for a wild creature for which the Task 6 shared
  candidate function returns non-empty.
- With `reveal` on, a flagged tile draws `?` in the top-left corner through
  `Painter`, and the con earmark is skipped for that tile on that frame
  only.
- On a tile whose con read is on the glyph, the `?` uses the same corner.
- Surface map only; the tactical board is unchanged.

**Tests:**
- Engine: `unseen_routine` agrees with the candidate function for the same
  creature (flagged and unflagged cases), and is false for a tamed program.
- gui headless paint test:
  - `?` is drawn with reveal on and absent with it off;
  - the earmark is absent only while the `?` is drawn.

**Steps:**
- [ ] Failing tests → implement → targeted tests.
- [ ] **Final gate:** `cargo test --workspace`, clippy with
  `--all-targets`, `cargo fmt`.
- [ ] Commit.

---

## After the last task

- An opus whole-branch review against the spec, given the diff as a file.
- Move the spec to `docs/superpowers/archive/specs/` on landing, and update
  `INDEX.md`.
- Nothing here has been seen on a screen. Hand the playtest (pacing, Alt vs
  the window manager) to the user before the deploy.
