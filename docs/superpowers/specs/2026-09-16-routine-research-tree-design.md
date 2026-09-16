# Routine research tree

**Date:** 2026-09-16
**Status:** design under review, not implemented.
**Todo:** #101, "routine research should be its own research tree".

Routines leave the base research tree and get a tree of their own. That
tree is derived from the ability catalogue, not authored. A routine that
something in the field carries is hidden until the player *discovers* it by
extracting any rung of its family. Research then climbs that family's
ladder one visible step at a time: Single v1 opens both Single v2 and
Group v1. Extraction stops teaching; it only discovers. Wild programs
carrying a family the player has never seen are marked with a `?` while
Alt is held, and the inspect sheet hides such routines' names.

## Decisions taken in brainstorming

| Question | Answer |
|---|---|
| Where routine nodes come from | Derived from `AbilityDb`, no authored files |
| The base nodes that grant routines today | All routine grants move to the routine tree |
| What extraction yields | A discovery record, not an item |
| Undiscovered families in the tree | Not shown at all — no `???` placeholder |
| Magnitude versions | Versions are rungs, branching off the base one |
| Routines nothing in the field carries | In the routine tree, visible once their zone gate is met |
| Zone gates | A new per-ability field; existing gates carried over, everything else zone 1 |
| How research is done | The existing Research Node project, one active project across both trees |
| Map marker | A `?` drawn while Alt is held |

## 1. Architecture

Routine nodes are **synthesised into `ResearchDb` at load**, the same way
`ItemDb::synthesise_etched_disks` mints one etched disk per ability. Each
synthesised `ResearchDef`:

- has id `"routine/<ability_id>"`, so the id is stable across a rename of
  the display name;
- carries `tree: ResearchTree::Routines` (base nodes default to
  `ResearchTree::Base`, `#[serde(default)]`);
- carries `teaches: Option<AbilityId>`, the one routine it grants.

`select_research`, the project filing, the material work orders, the
Research Node's labour and the one-active-project rule are all reused
unchanged. `Game::research_graph` and `Game::research_nodes` take a
`ResearchTree` and filter by it, so the graph layout and
`ResearchGraph::step` stay one derivation.

**Rejected:** a separate `RoutineResearchDb` with its own project path. It
duplicates the project machinery that shipped in
`2026-09-11-research-as-a-project-design.md`.

### Researched means known

A routine node is `Unlocked` exactly when `KnownRoutines` contains its
routine. Completing the project inserts into `KnownRoutines` and writes
nothing to `researched`. This has two consequences:

- A save made before this change needs no migration. Every routine the
  player already knows reads as researched, including a Group rung known
  without its Single rung.
- There are not two records of the same fact that could disagree.

`ResearchDef::unlocks_abilities` is **deleted**. A synthesised node's
`teaches` is the only path by which research grants a routine. Nothing in
the save or asset parsers sets `deny_unknown_fields`, so a mod file that
still names `unlocks_abilities` keeps parsing and the field is ignored.
`load_dir` logs a warning for it once.

## 2. Deriving the tree

### Family and rung

`AbilityDef::family()` moves out of `tests/assets.rs` into engine code, along
with `scope_rank` and the version-tag parse. The existing census
`every_battle_ability_family_is_contiguous_from_single_upward` becomes a
caller of these functions instead of keeping its own copy.

- **Family:** the display name minus its version tag and scope word.
  `"Patch Single v1.0"` and `"Patch Single v2.0"` are both family `Patch`.
- **Scope rank:** Single 0, Party/Group 1, Everyone 2.
- **Version:** the parsed `vN.N` tag. A name without a tag is version 1.

A node exists for every non-exclusive ability. **Exclusive routines get no
node** — they stay boss-drop and trader-only disks, exactly as today.

### Prerequisites

Within a family:

- Version *n* at a scope requires the **next lower version at the same
  scope**.
- The lowest version at a scope requires the **lowest version at the
  nearest lower scope that exists** in the family.
- The lowest version at the lowest existing scope requires nothing.

```
Single v1.0 ──► Single v2.0 ──► Single v3.0
     │
     └──────► Group v1.0 ──► Group v2.0
                   │
                   └──────► Everyone v1.0
```

So Group v1 can be researched before Single v2. An ally-facing family stops
at Party. A tamper family that ships a Group rung with no Single rung roots
at Group.

A synthesised node has **no base-tree prerequisite** except the gate in
"Opening the tree" below.

### Discoverable vs. always visible

A family is **discoverable** if any of its rungs has a carrier: a positive
`wild_weight`, or a place in some species' kit. Every other family is
**always visible**: field routines (`FieldBuff`, `Phase`, `Jump`,
`Symlink`), tamper routines, summons, and any family nothing carries.

This split is computed, not authored. That is what stops content from
going dead: a family nothing carries cannot be discovered, so it falls into
the visible half instead of being unreachable.

### Visibility

A node is **listed** in the routine tree when its zone gate is met and:

- for an **always-visible** family: always. A node whose prerequisites are
  not met shows as `Locked`, as base nodes do.
- for a **discoverable** family: the family is discovered (§3), and the node
  is `Unlocked`, `Active`, or `Available`. A `Locked` node in a discoverable
  family is not listed.

Only the next rung is ever shown. Researching Single v1 reveals Single v2
and Group v1 at the same moment, and nothing further up.

`ResearchState` gains no variant. Visibility is a filter applied before
states are computed, inside the one derivation `research_graph` already
reads.

### Zone gate

New field `AbilityDef::research_zone: u32`, `#[serde(default)]`, where 0
reads as 1. It becomes the synthesised node's `min_zone`. The
implementation carries over the gates the current granting nodes impose:

| Ability | Today's node | `research_zone` |
|---|---|---|
| `deep_scan`, `trace_analysis`, `stealth_protocol`, `salvage_routine` | `deep_analysis` | 3 |
| `buffer_overrun`, `wild_jump` | `address_translation` | 3 |
| the eleven `model_inspection` routines | `model_inspection` | 3 |
| `hardened_shell_party` | `mesh_plating` | 3 |
| `null_route` | `kernel_privileges` | 3 |
| `hardened_shell`, `overclock`, `ablative_layer` | `adaptive_plating` | 2 |
| `hot_patch` | `runtime_patching` | 2 |
| everything else, including `symlink`, `detach`, `priority_boost`, `repair_loop`, `trickle_charge` | — | 1 (default) |

### Cost

`tuning::routine_research_cost(scope_rank, version)`: a linear labour cost,
following CLAUDE.md's "every difficulty curve is linear". There is **no
material bill**. That keeps synthesised nodes outside the census "a
research node's bill may only name what its own prerequisites can make",
which a derived node could not satisfy. The formula's constants are fitted
against the current routine-granting nodes' costs, so the first rung of a
moved routine costs about what its old node did.

### Opening the tree

`routine_fabrication` currently guarantees that no routine is taught before
there is a way to install one. That guarantee is kept as data: a new
`ResearchDef::opens_routine_tree: bool`, `#[serde(default)]`, set on
`routine_fabrication.ron`. Until a node carrying that flag is researched,
the routine tree lists nothing, and `select_research` refuses a routine node
with "research <name> first". If no loaded node carries the flag, the tree
is open from the start, so a mod that deletes `routine_fabrication` is not
stranded.

## 3. Discovery and extraction

### The store

New resource `resources::DiscoveredRoutines(BTreeSet<AbilityId>)`, saved as
`discovered_routines: Vec<AbilityId>`, `#[serde(default)]`. It stores
ability ids, not family names, so renaming a display name cannot orphan a
record.

`Game::family_discovered(family)` is the one predicate. A family is
discovered if any of its rungs is in `DiscoveredRoutines` **or** in
`KnownRoutines`. The second half is what makes starter routines, and every
routine known in an existing save, count as discovered with no write.

### Extraction stops teaching

`Game::take_routine` stays the one place a routine comes off a program. Its
ordinary branch changes:

- **Before:** insert into `KnownRoutines`, return `Learned`.
- **After:** insert into `DiscoveredRoutines`, return `Discovered`. The log
  line does not name the routine: *"Recovered an unfamiliar routine — new
  routine research is available."*

The exclusive branch is unchanged (`DiskPopped`).

### What extraction may pick

`routine_candidates` and `extractable_routines` filter on
`!family_discovered(family)` instead of `!knows_routine(id)`. A program
whose routines all belong to discovered families has no candidates, and:

- the downed-program tool door (`extract_program`, `Routines` category)
- the tamed-program door (`extract_routine` at the Compiler)

each refuse **before anything is spent**, per the existing per-refusal rule.
Each refusal gets its own test. The refusal text is *"nothing unfamiliar to
recover"*.

## 4. What the player sees

### The routine research screen

- A new `Mode::RoutineResearch`, opened from its own base-menu row
  ("Routine research") next to the existing research row. It draws through
  the same list and graph renderers as `Mode::Research`, passing
  `ResearchTree::Routines`.
- It goes into `ALL_MODES` by hand; that list does not fail to compile.
- The list view with no discovered and no visible nodes shows one line:
  *"Recover routines from downed programs to open research here."*

### The base research screen

The base research screen passes `ResearchTree::Base` and is otherwise
unchanged.

The base-tree nodes left unlocking nothing are **deleted**:

- `symbolic_links`
- `self_exec`
- `process_detachment`
- `mesh_plating`
- `runtime_patching`
- `field_ops`
- `kernel_privileges`
- `adaptive_plating`

`deep_analysis` (which still unlocks tools) and `model_inspection` (which
still unlocks recipes) stay. `deep_analysis`'s `requires` is re-pointed from
`field_ops` to `routine_fabrication`, the root the deleted chain hung from.
Any save whose `researched` names a deleted id keeps loading, because an
unknown id there is already inert.

### The inspect sheet

`Game::routine_view` returns a new `RoutineSlotView::unseen: bool`, true when
the slot's family is not discovered. For such a slot the renderer draws
*"??? — a routine you haven't seen"*, and the engine blanks `name` and
`description` so no renderer can leak them. The concealment is in the
engine, not the gui.

### The Alt marker

- `EntityView::unseen_routine: bool`, true for a wild (non-tamed) creature
  that carries at least one non-exclusive routine whose family is not
  discovered. That is exactly "worth extracting from".
- gui: `crates/gui/src/lib.rs` reads `AltLeft`/`AltRight` held state beside
  `SHIFT_KEYS`/`CTRL_KEYS` and passes a `reveal: bool` into `render::draw`.
  app-core gets no new `GameKey`; this is a view gesture, not an action.
- While `reveal` is true, a tile with `unseen_routine` draws a `?` in the
  **top-left corner**, in place of the con earmark for that frame only.
  Every corner is already taken, and holding Alt is a deliberate question,
  so borrowing the earmark slot for the duration is acceptable.
- A tile whose con read is on the glyph has no earmark, so the `?` borrows
  an unused slot. The draw goes through `Painter`, in `render/base.rs`, with
  no backend call.
- Known risk: some Linux window managers take Alt+drag. Holding Alt without
  dragging does not trigger that.

## 5. Moddability and docs

- `assets/abilities/README.md`: document `research_zone`. Also document how
  a family and a rung are read off the display name, since that is now the
  research schema and not only a census.
- `assets/research/README.md`: remove `unlocks_abilities`, and document
  `opens_routine_tree` and the synthesised `routine/*` ids (a mod cannot
  author one).
- `CHANGELOG.md` at the merge. Not `docs/manual.md` or the root README.
- Seams: add a new seam entry for "a routine node is derived and researched
  means known", written to the memory graph, the `seams` skill, and a
  one-line rule in CLAUDE.md.

## 6. Testing

TDD with the failing test first, in `crates/engine/src/tests/`:

- **Derivation:** the `Patch` family yields Single v1 → Single v2 and
  Single v1 → Group v1, with the prerequisites above. A family rooted at
  Group has a parentless Group node.
- **Visibility:**
  - an undiscovered discoverable family lists nothing;
  - discovering one rung lists only the family's root;
  - researching the root lists exactly its two children;
  - an always-visible family lists all its nodes once its zone is met, and
    none below it.
- **Extraction:**
  - extracting an undiscovered rung writes `DiscoveredRoutines` and not
    `KnownRoutines`;
  - the log line does not contain the routine's name;
  - each of the two doors refuses a program with nothing unfamiliar, and
    nothing is spent.
- **Research completion:** completing a routine node inserts into
  `KnownRoutines`, and the node reads `Unlocked`.
- **Save:** a save→load test for `discovered_routines` (the RON round-trip
  alone cannot catch a skipped field). A pre-change save knowing a Group rung
  loads with that node `Unlocked` and its family discovered.
- **Gate:**
  - the routine tree is empty and refuses until `opens_routine_tree` is
    researched;
  - with no flagged node loaded, it is open.
- **Censuses** in `tests/assets.rs`:
  - every non-exclusive ability has exactly one synthesised node;
  - no base node grants a routine;
  - every routine node is reachable (its prerequisite chain roots in the
    family);
  - every discoverable family has at least one carried rung.
- **Inspect:** `routine_view` blanks `name` and `description` for an unseen
  slot.
- **gui:** a headless paint test asserts the `?` is drawn with reveal on and
  absent with it off. It also asserts the earmark is absent only while the
  `?` is drawn.

The full `cargo test --workspace` suite and `cargo clippy --workspace
--all-targets` pass at each phase boundary.

## 7. Phasing

1. **Engine:**
   - family and rung functions move out of the test;
   - synthesis, `ResearchTree`, `teaches`, `research_zone`,
     `opens_routine_tree`;
   - `DiscoveredRoutines` and its save field;
   - `take_routine` and candidate changes;
   - deleting the eight base nodes and removing `unlocks_abilities`;
   - censuses and README updates.
2. **Screen:** the `Mode::RoutineResearch` screen, its base-menu row, and
   the tree argument through app-core and the renderers.
3. **Concealment:** `RoutineSlotView::unseen`, `EntityView::unseen_routine`,
   Alt held state, and the `?` draw.

Each phase leaves the game playable.

## Risks and what the tests cannot see

- **Pacing moves and nothing gates it.** Twenty-eight routines, granted
  today by eleven base nodes, stop being bought with base research. The
  discoverable ones now need a hunt plus research, and `balance_sim` models
  no abilities. Only a playtest can say whether the early game still gets a
  support routine in time.
- **`priority_boost` stays reachable.** It is Hyperthread Single v1.0, the
  companion fallback. Nothing carries it directly, but its family is
  discoverable through `hyperthread` (v2.0, `wild_weight: 9`), and it is
  that family's root.
- **Display names are now schema.** A mod that renames `"Patch Single v1.0"`
  to `"Patch v1.0"` silently moves it to a different family and rung. The
  derivation census catches this only for shipped content.
- **Alt and the window manager.** This is not measurable from here; a
  playtest will show it.
- **A green suite is not evidence of play.** None of this will have been
  seen on a screen when phase 3 lands.
