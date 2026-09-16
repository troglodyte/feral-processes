# Routine research tree

**Date:** 2026-09-16
**Status:** design approved 2026-09-16 after a source review, not
implemented.
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
| Where a name is concealed | Only where a *wild or downed* program's routine is previewed; owned programs, the battle log and market disks keep real names |
| Passives and summons | No node for either; no content change |
| A discovered family whose next rung is zone-gated | Listed as `Locked` with its sector, not hidden |
| The first discovery door | `routine_reader` moves from `cortex` to `routine_fabrication` |

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
routine. `settle_research` inserts into `KnownRoutines` for a routine node
and writes nothing to `Research`. Every reader of "is this node researched"
— `is_researched`, `missing_prereqs`, `select_research`'s already-researched
refusal — answers a routine node from `KnownRoutines`, through one helper,
because today all three read `Research` alone.

A prerequisite rung counts as met when it is known **or any higher version
at the same scope is known**. A new game's starter Patch Single v2.0 would
otherwise leave Patch Party v1.0 waiting on a Single v1.0 nobody needs.

This has two consequences:

- A save made before this change needs no migration. Every routine the
  player already knows reads as researched, including a Group rung known
  without its Single rung.
- There are not two records of the same fact that could disagree.

`ResearchDef::unlocks_abilities` is **deleted**. A synthesised node's
`teaches` is the only path by which research grants a routine. Nothing in
the save or asset parsers sets `deny_unknown_fields`, so a mod file that
still names `unlocks_abilities` keeps parsing and the field is ignored.
Serde drops an unknown field silently, so there is no warning; keeping a
dead field only to warn about it is the cruft CLAUDE.md forbids.

`research_nodes` and `research_graph` both walk `ResearchDb::all()` today
and read no shared derivation. They are made to: one function decides
which nodes a tree lists (§2 "Visibility"), and both call it.

## 2. Deriving the tree

### Family and rung

`AbilityDef::family()` moves out of `tests/assets.rs` into engine code, along
with `scope_rank` and `without_version_tag`. The existing census
`every_battle_ability_family_is_contiguous_from_single_upward` becomes a
caller of these functions instead of keeping its own copy.

- **Family:** the display name minus its version tag and scope word.
  `"Patch Single v1.0"` and `"Patch Single v2.0"` are both family `Patch`.
- **Scope rank:** read off `AbilityTarget`, as the census does today, not
  off the scope word: Single 0, Party/Group 1, Everyone 2.
- **Version:** a `(major, minor)` pair parsed from the `vN.N` tag. This is
  new code — the test file only strips the tag. The minor part is needed:
  Patch Party ships v1.0 and v1.1 (`redundancy_sync`). A name without a tag
  is `(1, 0)`.

A simulation of `family()` over all 100 shipped abilities parsed cleanly
with no collisions (review, 2026-09-16).

**A node exists for every ability except four kinds:**

- **exclusive** — they stay boss-drop and trader-only disks, as today;
- **permanent** (`routine_is_permanent`, i.e. `decompile`) — every player
  already has it;
- **passive** — the six shipped ones (`clock_skew`, `core_dump`,
  `hot_spare`, `interrupt_request`, `parity_guard`, `quarantine`) are gear
  grants, and a node would make them researchable slot routines;
- **summon** — `Fork Cluster` and `Fork Program` have no source today, and
  this change does not add one.

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

Nothing is listed until the tree is open ("Opening the tree" below). After
that, a node is **listed** when:

- for an **always-visible** family: its zone gate is met. A node whose
  prerequisites are not met shows as `Locked`, as base nodes do.
- for a **discoverable** family: the family is discovered (§3), and the
  node's **prerequisite rungs are met**. The zone gate does not hide such a
  node: one whose sector is too low is listed as `Locked { min_zone }`, so
  a family discovered in zone 1 whose root is gated to zone 2 (Patch) shows
  the player what is waiting instead of nothing.

Only the next rung is ever shown. Researching Single v1 reveals Single v2
and Group v1 at the same moment, and nothing further up.

`ResearchState` gains no variant. Visibility is one filter function applied
before states are computed. `research_nodes` and `research_graph` both call
it, and **a listed node's unlisted parent is treated as absent** by the
graph layout. The layout's Kahn pass over `requires` would otherwise never
settle such a node, and it would get no cell. The reachable case is an old
save that knows Patch Party v1.0 while `hot_patch` is still gated.

`select_research` refuses a routine node that is not listed, with the same
text as an unknown id. Leaving a hidden node off the list is not enough.

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
stranded. `has_research_tree`, which gates the base-menu row, takes the tree
as an argument.

### The first discovery door

`routine_reader`, the Routines extraction tool, is unlocked by `cortex`
(zone 3) today. Before zone 3 the only way to discover would be to tame a
carrier and dissolve it at a Compiler. The tool moves to
`routine_fabrication`'s `unlocks_tools`, so recovering from downed programs
opens at the same moment as the tree. `cortex` keeps its other unlocks.

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
- **After:** insert into `DiscoveredRoutines`, return `Discovered`
  (`RoutineTaken::Learned` is renamed). The log line does not name the
  routine: *"Recovered an unfamiliar routine — see routine research."* It
  makes no promise that research is available now, because the next rung
  may be gated to a later sector.

The exclusive branch is unchanged (`DiskPopped`).

### What extraction may pick

`routine_candidates` and `extractable_routines` filter on
`!family_discovered(family)` instead of `!knows_routine(id)`. Each door
refuses **before anything is spent**, per the existing per-refusal rule, and
each refusal gets its own test:

- the downed-program tool door (`extract_program`, `Routines` category)
  refuses a program with no candidates: *"nothing unfamiliar to recover"*;
- the tamed-program door (`extract_routine` at the Compiler) refuses per
  selected row, as it does today. Its "You already know that routine"
  becomes *"that routine is already familiar"*.

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

The nine base-tree nodes left unlocking nothing are **deleted**:

- `address_translation`
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
Its bill already matches `self_exec`'s under `routine_fabrication`, so the
materials census stays green. The census `no_research_node_is_left_unlocking_nothing`
counts `unlocks_tools` too, or `deep_analysis` (tools only) fails it. The
eleven `model_inspection` routines lose their `cortex`/`neural_amp`
prerequisite chain. They keep `research_zone: 3`, and that is accepted.

Any save whose `researched` names a deleted id keeps loading, because an
unknown id there is already inert. An `active_research` naming an id no
loaded node has would stall forever, so load clears it, together with the
work orders filed for it (`WorkOrder::for_research`).

### Concealment

A routine is **unseen** when it belongs to a wild or downed program, not to
one the player owns, and its family is not discovered. Only surfaces that
preview such a program's routines hide the name, and the engine does the
hiding so no renderer can leak it:

- **The inspect sheet.** `Game::routine_view` returns a new
  `RoutineSlotView::unseen: bool`. For an unseen slot the engine blanks
  `name`, `description` and the ability id. The renderer draws *"??? — a
  routine you haven't seen"*. A slot on an owned program is never unseen,
  so the player's own routines menu is unchanged.
- **The extraction preview.** `ExtractionPreview::Routine` states how many
  unfamiliar routines there are and names none.
- **The death line.** `announce_program_death` does not name an unseen
  routine for a wild program.

The battle log, Stack market disks and the Compiler picker (an owned
program) keep real names. The battle log is the deliberate leak: a routine
you watched being run is one you have seen *happen*, even if it is not
recorded.

### The Alt marker

- `EntityView::unseen_routine: bool`, true for a wild (non-tamed) creature
  that would yield an extraction candidate. It is computed by **the same
  function `routine_candidates` calls** (the level-gated kit plus
  `carried`), not by reading live `Routines`. The two sets differ, and the
  marker must promise what extraction delivers.
- Surface map only. The tactical board also draws `ConRead`
  (`render/tactical.rs`), but a fight is not where a hunt is chosen, so it
  is out of scope.
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

- **Derivation:** the version parse orders `v1.1` after `v1.0`. No node
  exists for an exclusive, permanent, passive or summon ability. The `Patch` family yields Single v1 → Single v2 and
  Single v1 → Group v1, with the prerequisites above. A family rooted at
  Group has a parentless Group node.
- **Visibility:**
  - an undiscovered discoverable family lists nothing;
  - discovering one rung lists only the family's root;
  - researching the root lists exactly its two children;
  - an always-visible family lists all its nodes once its zone is met, and
    none below it;
  - a discovered family whose root is zone-gated lists the root as
    `Locked { min_zone }`;
  - knowing Single v2 satisfies the Party v1 prerequisite;
  - a listed node whose parent is unlisted still gets a graph cell and is
    reachable by `ResearchGraph::step`;
  - `select_research` refuses an unlisted routine node.
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
- **Old save:** an `active_research` naming a deleted node is cleared on load,
  along with its work orders.
- **Concealment:**
  - `routine_view` blanks name, description and id for an unseen slot on a
    wild program, and never on an owned one;
  - the extraction preview and the wild death line contain no unseen name;
  - `unseen_routine` agrees with `routine_candidates` for the same creature.
- **Existing tests to rewrite:** these read `unlocks_abilities` today:
  - `tests/assets.rs` near 663;
  - `every_shipped_field_routine_can_actually_be_obtained`, whose research
    source becomes the synthesised node;
  - `tests/research.rs` near 668;
  - `tests/routines.rs` near 621;
  - the test-only helper in `views.rs`;
  - the unit tests in `research.rs`.
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
   - deleting the nine base nodes and removing `unlocks_abilities`;
   - moving `routine_reader`, and the load-time clear of a stale
     `active_research`;
   - censuses and README updates.
2. **Screen:** the `Mode::RoutineResearch` screen, its base-menu row, and
   the tree argument through app-core and the renderers.
3. **Concealment:**
   - `RoutineSlotView::unseen`;
   - the extraction preview and the death line;
   - `EntityView::unseen_routine`;
   - Alt held state and the `?` draw.

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
- **Discovery pacing.** Moving `routine_reader` to `routine_fabrication`
  opens downed-program recovery early, but how soon a player meets a
  carrier of a support family is unmeasured.
