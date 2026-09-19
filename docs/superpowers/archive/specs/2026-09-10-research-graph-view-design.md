# Research: a graph view to pick from

## Context

The research screen (`T`) is a `Row`-based `Large` popup —
`render/progression.rs::draw_research_menu`, selection a `usize` index into
`Game::research_nodes()`, lowercase letters picking rows. It reads as a list
of 34 unrelated purchases. Nothing on it draws the fact that the nodes form a
tree: a player wanting Mesh Plating cannot see that it costs four nodes and a
breach to get there, and `ResearchState::Locked { missing }` names only the
prerequisites *still* missing, so the shape is invisible even one node back.

This adds a second view of the same screen: the tree drawn as a flow chart,
navigated with the arrow keys, bought with `Enter`.

## Decisions taken

Settled in brainstorming on 2026-09-10; recorded so they are not
relitigated.

| Question | Decision |
|---|---|
| Replace the list? | No. Both views, one key toggles. |
| Fit? | Whole tree visible at once; short labels in the boxes, full detail in a side panel. No panning. |
| Where does the layout live? | The engine derives it; app-core and gui both read the one derivation. |
| Reading direction | Tiers are columns, left to right. |
| A new `Mode`? | No — a view flag, dispatched ahead of the `Mode` match. |

## The tree, measured

Derived from the 34 files in `assets/research/`, not remembered:

| Tier | Count | Nodes |
|---|---|---|
| 0 | 4 | automation, commerce, paging, power_grid |
| 1 | 7 | armor_bench, cache_coherence, dispatch, fortification, program_refactoring, routine_fabrication, teardown |
| 2 | 5 | charge_density, firewall, segmentation, self_exec, weapon_bench |
| 3 | 9 | ablative, capacitance, field_ops, neural_amp, overclock, process_detachment, runtime_patching, symbolic_links, virtual_memory |
| 4 | 6 | adaptive_plating, cortex, deep_analysis, kernel_privileges, monofilament, shard_mapping |
| 5 | 3 | address_translation, cold_archive, mesh_plating |

**Six tiers by nine slots**, four roots, and exactly one diamond:
`segmentation` requires both `paging` (tier 0) and `cache_coherence` (tier 1),
so it is the one edge that skips a tier.

The widest name is 19 characters (`Routine Fabrication`, `Program
Refactoring`, `Address Translation`) and the longest unbreakable word is 11
(`Fabrication`). A box must fit an 11-character word on one line — a wrap
cannot split a word — which is the fit constraint the census test asserts.

Tiers as columns rather than rows because six columns leave roughly 130px per
box against nine columns' 85px at 1280x720, and 85px does not fit
`Fabrication`. The exact figures are the test's to establish, not this
document's.

## What the engine exposes

A new `views::ResearchGraph`, from `Game::research_graph()`:

- one cell per node — its `tier` and its `slot`;
- the edge list, each edge a (prerequisite, dependent) pair;
- `tiers` and `widest`, so a renderer can size a cell without folding the
  cells itself.

**Keyed by `ResearchId`, never by index into `research_nodes()`.** That vec is
sorted by `ResearchState` (Available, then Locked, then Unlocked), so its
order changes as the player buys things, and any future filter would silently
re-index it. An index-parallel structure would be correct on the day it
shipped and wrong the first time someone filtered the list. Both readers
already hold `ResearchStatus::id`.

**Tier is the longest path from a root**, so every edge points strictly
rightward and the diamond's short leg stretches rather than pointing
backwards. **Slot orders by the first-listed parent's slot, then by id** —
deterministic, and it keeps a subtree contiguous. A cycle authored by a mod
(`a` requires `b` requires `a`) must be dropped with a logged warning, the
`.ron` loader's rule, not a stack overflow.

**`ResearchGraph::step(from, dir) -> ResearchId`** is the one rule for what an
arrow key does, and it lives beside the cells rather than in app-core. This is
the whole reason the layout is derived in the engine: if app-core computed
neighbours and gui computed positions from the same data by two different
rules, the cursor and the boxes would drift, which is
`balance_sim.rs`'s recurring failure in a new place. Up and down move within a
tier; left and right move between tiers, landing on the nearest slot. `step`
is total — it returns the node it was given rather than falling off the grid.

## The screen

A new `crates/gui/src/render/research_graph.rs`, drawn **instead of** the
research popup, dispatched in `render::draw` from a flag on `App` —
`render/icon_editor.rs`'s existing arrangement, which is what keeps this out
of `Mode` and out of every mode census.

Graph on the left, detail panel down the right side. The panel is built from
`material_rows`, `description_rows` and `conversion_rows` — the *same*
functions `research_menu_rows` calls, so the two views cannot describe a node
differently. Box colour is `row_color`, reused for the same reason: cyan,
green and dim have to mean on the graph what they mean in the list.

A box carries the node's name (wrapped, elided if a modded name will not fit)
and its cost. The selected box takes the highlight the list's selected row
takes.

Edges draw as orthogonal elbows in the gutter between tiers: dim by default,
bright for the selected node's own prerequisites. **No edge router.** The one
tier-skipping edge may cross a column, and the selection highlight is what
makes it readable — a router is a large amount of code for one edge in the
shipped tree.

Cell pitch is fit-to-window, derived from `tiers` and `widest`. A modded tree
deeper or wider than the shipped one squeezes and elides rather than
scrolling; there is no viewport offset and no scroll state. This is a
deliberate YAGNI: panning is the feature to add if a mod ever needs it, and
adding it later costs one offset.

## Input and state

- The flag on `App` toggles with an **uppercase** key (`G`). Lowercase is a
  row selector on the list side, so a lowercase toggle would both pick a row
  and flip the view.
- `App::selected` stays the one selection, an index into `research_nodes()`.
  The graph converts it to an id, steps, and converts back — a scan of 34
  entries per keypress. Two cursors that could disagree is the thing this
  avoids; toggling either way lands on the node the player was looking at.
- `Enter` calls `Game::unlock_research`, the same door the list calls, and
  reports the same outcome through `App::report`.
- `Esc` closes the screen from either view.

## Testing

Engine, against the real 34 files:

- the tier census above, so a content change that reshapes the tree is a
  visible diff rather than a squeezed screen;
- every edge points strictly rightward;
- `step` is total in all four directions from every node;
- the layout is deterministic across two calls on one `Game`;
- a mod authoring a cycle loads with a warning and no node.

gui, headless through `paint::with_painter`, which measures real text:

- the widest shipped name's longest word fits a box at the minimum window
  size;
- six tiers by nine slots fit the graph pane at that size;
- the detail panel draws the selected node's materials and conversions —
  the rows the list draws.

app-core:

- the toggle preserves the selected node in both directions;
- `Enter` on an unaffordable node refuses without spending, and the refusal
  is the list's refusal.

## Out of scope

- Panning, zooming and edge routing.
- Any change to what a node costs, unlocks or requires.
- The perk and talent screens, which are ladders rather than graphs.
