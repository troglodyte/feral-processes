# Research: a node the base cannot work yet is picked and flagged, not refused

**Date:** 2026-10-17
**Status:** approved in chat; see `INDEX.md` for whether it shipped.

## Intent

Opening the research screen and being refused is annoying. Today
`Game::select_research` turns a row down for three reasons that are about
the *base*, not the *tree*: no Research Node standing, a material whose
machine is not built, or no program pinned in a Research Station's pen.
The player wanted to say "work this next" and go build the machine; instead
they are told to come back later.

After this change the player can pick any node that the tree allows next.
If the base cannot work it yet, the node still becomes the project and the
screen says so with an exclamation point and the reason. The project waits
until the missing thing exists and then proceeds on its own.

## Goals

- Picking a row succeeds whenever the *tree* allows it, whatever the base's
  state.
- Every surface that shows the project says, with a `!` and a sentence,
  that the base cannot work it yet: the research list, the graph view, and
  the corner readout over the map.
- Nothing else about research changes: what the tree allows next, one
  project at a time, materials as work orders, completion's gates.

## Non-goals

- A research queue. The base still works one project at a time, and
  picking a second node while one runs is still refused with "abandon it
  first".
- Relaxing prerequisite or zone gates. "Requires X first" and "Requires
  Zone N first" stay as they are.
- Auto-queuing prerequisites.

## Anti-goals

- A project that can never complete with nothing saying why. If the `!`
  can be on a row, the sentence under it must name the thing to build or
  do.
- The screen offering something the selection then refuses, or the
  selection taking something the screen marked as refused. The row's flag
  and the readout's flag must be the same derivation
  (`Game::research_block`).
- A material order that is silently never filed. Today the bill is filed
  once at selection and never topped up; that must stay true, so a line
  whose machine does not exist yet has to be *filed anyway*, not filed
  later.

## What changes, as decided

### 1. Selection (`Game::select_research`, `crates/engine/src/game/unlocks.rs`)

The `research_block` rung is removed from the refusal ladder. Every other
refusal stays, in its current order: game over / battle, no base, hidden or
unknown node, already researched, prerequisites, zone gate, another project
running.

### 2. The bill (`Game::queue_work_order`, `crates/engine/src/game/base/work_orders.rs`)

A research order (`WorkOrder::for_research`) is filed even when
`order_refusal` would turn a player's order for the same item down, so the
bill is still filed once, at selection. The order sits in the queue as
`OrderStanding::Stalled`, which already exists: the queue screen shows the
chain-break sentence under it, "Work order stalled" is logged once, and the
scheduler starts working it when the machine goes up.

Exception, builder's call on wording only: an order for an item that does
not exist (`"No such item."`) is still refused for research too; that is an
asset error, not a base state.

### 3. The row (`ResearchStatus`, `crates/engine/src/views.rs`; `research_nodes`)

`ResearchStatus::blocked_by` is filled for an `Active` node as well as an
`Available` or `Locked` one. Only `Unlocked` stays `None`. The field's doc
comment, which says it is "the same call `Game::select_research` refuses
on", is rewritten: it is the sentence the screen flags with, and selection
no longer refuses on it.

### 4. The readout (`ResearchReadout`, `crates/engine/src/views.rs`; `Game::research_readout`)

A new variant, `ResearchReadout::Blocked { name, reason }`, returned when
the active project's `research_block` names something. It is checked
*before* the existing `Stalled` check and does not depend on progress, so
it shows from the first tick. `reason` is `research_block`'s sentence
verbatim, so the corner and the row cannot disagree.

### 5. Drawing (`crates/gui/src/render/progression.rs`, `research_graph.rs`, `hud/stock_block.rs`)

- List row: `[1] ! Name - 40 Research Data` when `blocked_by` is `Some`;
  the amber sentence under the row stays as it is. The `!` sits between the
  key and the name. An `Active` row keeps `CYAN`; the `!` and the amber
  sentence carry the warning. An `Available` row keeps the amber it has.
- Graph view: the `!` goes in front of the box label's first line. The
  detail panel already draws `block_rows`.
- Corner readout: `Blocked` draws the header, the name, then
  `! <reason>` wrapped in `palette::ATTENTION`, the way `Stalled` draws
  `needs <short_of>`.

### 6. Help (`assets/help/60-your-base.md`)

The paragraph beginning "A node whose bill names something nothing in your
base could ever make is refused outright" is rewritten to say the node is
taken on and flagged with `!`, the flag names what is missing, and the
project waits for it. The sentence about "From sector 2 on, most nodes
refuse selection until a program is pinned" is rewritten the same way.

### 7. Seam notes

`.claude/skills/seams/references/base-needs.md`'s paragraph on
`select_research` lists six refusals and says `research_block` is one of
them; it becomes five, and the paragraph says the block is what the row,
the graph and the readout flag with instead. The rules file sentence
("every refusal before anything is filed") stays true and is not edited.
The memory graph entity
`seam:research-is-one-project-and-select-research-is-the-one-door` gets an
observation saying the same, with the date and this spec's path.

## Constraints

- Two crates (engine, gui) and no save-format change. `ActiveResearch` is
  not touched.
- `research_readout` runs every frame on the HUD. `research_block` for one
  node is a handful of `chain_break` calls, which is what the readout's
  existing `producers_of` already costs; no memo is needed. Do not call it
  across the tree.
- Keep the moddability rules: no content hardcoded that is data.
- `balance_sim` is unaffected; no `tuning.rs` change.

## Tests

Existing tests that assert a refusal on the block flip to assert the flag.
Known ones, by name, in `crates/engine/src/tests/research.rs`:
`a_base_with_no_research_node_cannot_take_a_project`,
`a_subject_gated_node_is_refused_without_a_pinned_subject`,
`every_subject_gated_node_refuses_selection_without_a_pinned_subject`,
`a_blocked_node_carries_the_sentence_that_would_refuse_it`,
`research_nodes_blocks_a_subject_gated_node_the_same_way_select_research_does`,
`the_zone_gate_is_refused_before_the_machines`,
`no_research_node_is_gated_below_its_own_prerequisite`. The builder greps
for the rest; app-core's `tests/research.rs` has a refusal test whose
refusal may be one of these.

New tests, intent only:

- Selecting with no Research Node standing succeeds, the project is active,
  the row is `Active` with `blocked_by` set, and `research_readout` is
  `Blocked` with the same sentence.
- A research line whose machine is not built is filed at selection and
  reads `Stalled` on the queue; building the machine makes it move; a
  player's order for the same item is still refused.
- Completion still waits: a project with full data and a missing machine
  does not complete; once the material arrives it does.
- A subject-gated node selects with nobody pinned and reads `Blocked`;
  pinning clears the flag.
- gui: the row label carries `!` exactly when `blocked_by` is `Some`, for
  `Active` and `Available` alike; the graph box label likewise; the stock
  block draws `Blocked`.
- Mutation check: with the `!` removed from the row label, a test fails.

## Left to the builder

- Exact sentence wording in the help page and the seam reference.
- Whether `research_header` ("Working on: X (n/cost)") also carries the
  `!`. Not required; the rows and the corner are the two surfaces asked
  for.
- Test fixture choice: `crates/engine/src/tests/support.rs` first, then
  `dev-saves/` templates, never a fresh `Game::new` when a template would
  do.
