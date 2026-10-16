# Factory overhaul: configurable production machines

**Date:** 2026-10-16

## Intent

Today every recipe is its own structure. A Lathe makes blank substrate, a
Refinery makes bytecode blocks, and so on. The build menu carries one entry
per recipe, and changing what a spot in the base produces means tearing a
structure down and building another.

This overhaul replaces every structure that extracts or transforms materials
with four generic machine kinds. You place one, then pick its recipe from a
menu. A fifth structure, the **Workbench**, folds the four hand-compile
benches into one station.

## Decisions (the user's)

- **There are four machine kinds, sorted by role:**
  - **Extractor:** pulls a raw resource, with no inputs.
  - **Factory:** turns raw extracted material into a material or item.
  - **Assembler:** turns items a Factory made into an item or a more complex
    material.
  - **Printer:** makes consumables.
- **Every machine is placed with no recipe.** Until you configure it, it
  shows a small question mark and does nothing.
- **Recipes are the player's to set.** Work orders never change a machine's
  recipe. An order is served only by machines already set to make its item.
  If there are none, the order waits.
- **The machines replace only material-in, item-out structures.** Service
  structures are untouched: rechargers, repair bays, depots, defrag bays,
  holding cells and the like.
- **Research Station stays as it is.** Conceptually it is not an extractor.
- **Power Conduit becomes a recipe on the Extractor.**
- **Workbench** replaces the Compiler, Armory, Fabricator and Refactor Bench
  as benches:
  - It is hand-compile only. No program can be posted to it, and nothing is
    automated there.
  - You must be standing next to the Workbench to compile a Workbench
    recipe. Owning one somewhere in the base is not enough. This replaces
    today's ownership-based bench check.
  - Recipes that never needed a bench can still be compiled anywhere,
    including in the field. These are the ones with no `requires_structure`
    today: ice_breaker, power_cell and the starter kit. The Compile menu
    stays `Locality::Anywhere`; only bench-gated rows need the touch.
  - Every hand-compile recipe gated on a removed structure moves to the
    Workbench and keeps its research gate. This includes the products of
    the old production machines (blank_substrate, bytecode_block and the
    rest) and the four Winding Node research recipes (buffered_cell,
    sustain_cell, capacitor_array, backfeed_cell). Nothing hand-compilable
    today stops being so.
  - The Workbench has Mk1–5 upgrade tiers. The tier of the Workbench you are
    standing at sets the crafted-gear quality bonus (`QUALITY_BENCH_PER_TIER`),
    replacing today's best-tier-in-base lookup for compiling.
- **Saves become incompatible.** Bump `save::SAVE_FORMAT_VERSION`. There is
  no migration. The release takes the breaking (minor, at `0.x`) bump.
- **Power draw per kind:**
  - Extractor 1
  - Factory 2
  - Assembler 3
  - Printer 2
- **Recipes can be data or code, whichever is cleaner.** The cleaner option
  below keeps them as data, as the moddability rule requires.

## Calls made in the brainstorm (Claude's, unchallenged by the user)

- **The recipe menu lists only researched recipes.** Locked recipes are
  hidden, the same as the build menu.
- **Each recipe has its own cycle time.** Recipes keep the pace of the
  structures they replace.
- **Upgrade tiers (Mk1–5) stay on the Extractor.**
  - They keep their current yield effect.
  - Factory, Assembler and Printer have no tiers at first.
  - The Compiler, Armory and Fabricator lose their tiers.
- **Changing a recipe empties the machine.** Its held input and output go to
  the nearest depot, or onto the floor if there is no depot, and its cycle
  progress resets.
- **Each machine kind has one build cost**, set near the cheapest structure
  it replaces.
- **Combining recipes are out of scope.**
  - No recipe today takes more than one kind of input. The Assembler
    launches with single-input recipes.
  - Nothing may block adding multi-input recipes later. Item `craftable.cost`
    is already a list, so this mostly comes for free.

## Calls made after the builder check (Claude's)

- **Research gates on hand-compile recipes.** If moving a recipe to the
  Workbench removes its research gate (because the gate came from the old
  structure), add a research field to `CraftableDef` with
  `#[serde(default)]`. Each recipe keeps the gate it effectively had.
- **The Extractor's tier.**
  - It stays on the machine when its recipe changes.
  - The power_cell recipe always pays at tier 1, as the tierless Power
    Conduit did. Zone scaling still applies. This avoids an unplanned
    rebalance.
  - There is one upgrade cost per tier. Use the Mining Node's cost, the
    cheapest.
- **The extraction side jobs** (`extracts_routines`, `extracts_programs`)
  keep the `automation` research gate that the Compiler had. They are
  ownership-based on the Workbench once that research is done.
- **Depot auto-filter.** Setting a recipe on a machine counts as the moment
  it was placed: adjacent Depots that have no filter get auto-filtered for
  that output. A Depot that already has a filter is never rewritten, in
  keeping with "auto-setup applies to what was placed". If a later recipe
  change leaves a neighbouring Depot refusing the new output, the machine
  clogs visibly, as it does today.
- **Contracts that name removed structures.**
  - Objectives that just mean "build an extractor" become
    `Build(structure: extractor)`. This covers break_ground,
    tutorial_break_ground and keep_the_lights_on.
  - Contracts that mean "a machine making X" get a new objective variant,
    "a machine configured to make item X". This covers stand_up_a_refinery
    (bytecode_block) and commission_an_armory (now the Workbench, or an
    Assembler set to hardened_shell, whichever reads closer to the
    contract's text).
  - Update the contracts README.

## The recipe mapping

The output item's `craftable.cost` supplies each recipe's inputs, as it does
today. Cycle times are today's values.

| Machine | Recipe (output) | Inputs | Cycle | Replaces |
|---|---|---|---|---|
| Extractor | core_fragment | none | 10 | Mining Node |
| Extractor | raw_trace | none | 10 | Log Scraper |
| Extractor | cache_grain | none | 14 | Cache Tap |
| Extractor | power_cell | none | 6 | Power Conduit |
| Factory | blank_substrate | 4 core_fragment | 12 | Lathe |
| Factory | bytecode_block | 4 core_fragment | 12 | Refinery |
| Factory | annealed_core | 4 core_fragment | 12 | Annealing Node |
| Factory | logic_wafer | 4 raw_trace | 12 | Transcriber |
| Factory | charge_coil | 3 power_cell | 12 | Winding Node |
| Assembler | routine_disk | 2 blank_substrate | 20 | Disk Press |
| Assembler | honeypot | 2 ice_breaker | 20 | Decoy Bench |
| Assembler | recompile_kernel | 3 annealed_core | 20 | Refactor Bench (auto) |
| Assembler | hardened_shell | 3 bytecode_block | 30 | Armory (auto) |
| Assembler | trace_sniffer | 5 logic_wafer | 30 | Fabricator (auto) |
| Printer | ice_breaker | 3 core_fragment | 8 | Compiler (auto) |
| Printer | patch_routine | 3 charge_coil | 20 | Assembly Bay |

**Research gating moves from the structure to the recipe.** A recipe unlocks
with the research that unlocked the structure it replaces. For example,
`logic_wafer` unlocks with `routine_fabrication` and `power_cell` with
`power_grid`. A recipe whose old structure had no gate is available from the
start. The four machine kinds and the Workbench are buildable from the start.

**Hand-compile recipes** whose `requires_structure` names any removed
structure now name the Workbench. Each keeps the research gate of the
structure it came from. `ResearchRecipe::requires_structure` (`research.rs`)
gets the same change.

**Structures removed:**

| Group | Structures |
|---|---|
| Extractors | Mining Node, Log Scraper, Cache Tap, Power Conduit |
| Factories | Lathe, Refinery, Annealing Node, Transcriber, Winding Node |
| Assemblers | Disk Press, Decoy Bench |
| Printers | Assembly Bay |
| Benches | Compiler, Armory, Fabricator, Refactor Bench |

## Constraints (from the code, verified by the inventory)

- **A structure can only hold one recipe today.**
  - Extraction uses `StructureDef::work: Option<WorkDef>`.
  - Transformation uses `StructureDef::assembles: Option<AssembleDef>`.
  - `systems::produced_item` is the single answer to "what does this
    structure output". It must now read the placed structure's recipe, not
    the def.
- **The work-order scheduler stores nothing per machine.** `work_orders.rs`,
  `lines.rs` and `schedule_base_labour` recompute everything from live state.
  The `producers` map in `structures.rs` (around line 799) maps item →
  structure kinds from defs. It must instead map item → configured instances.
  A machine with no recipe serves no order and is never staffed.
- **Recipe pace is baked in when a worker is assigned.** `Task.required`
  holds it. Changing a recipe must re-bake the pace or unassign the worker.
- **Inputs arrive by adjacency.** An assembling machine pulls from the output
  buffers of the four orthogonally adjacent structures. That stays as it is.
- **Buffer capacity varies (10, 15 or 20).** It becomes per recipe or per
  kind. That is the builder's call.
- **The Compiler's side jobs need a new owner.** `extracts_routines`
  (`game/routines.rs`) and `extracts_programs` (`game/extraction.rs`) are
  ownership checks. Move both flags to the Workbench and keep them
  ownership-based. The touch rule above is only for compiling.
- **Moddability:** see the schema change below.

## Schema and save change

- **The recipe list goes on the machine's structure file:**
  - Something like `recipes: [(item, ticks_per_unit, research)]` on each
    machine's `.ron`.
  - Extractor entries carry `level` and `flat_payout`, the same as today's
    `WorkDef`.
  - New fields get `#[serde(default)]`.
  - A malformed entry is skipped with a warning.
  - `assets/structures/README.md` is updated in the same change.
- **One placed machine records its recipe.**
  - Add a component, e.g. `Recipe(Option<ItemId>)`.
  - Add a flat `recipe` field to `StructureSave`.
- **The old one-recipe fields get folded into the new list.** Whether `work`
  and `assembles` stay as a single-recipe shorthand is the builder's call;
  removing them is preferred. Remove the old structures' `.ron` files.
- **Bump `SAVE_FORMAT_VERSION`.**
- **Recreate every `dev-saves/` template in the new format.**
  - Each template's intent (README) should be preserved: its base should
    contain the equivalent configured machines.
  - The arena dev files under `dev-arenas/` are not saves, but check them for
    structure ids.

## UI

- **The question mark** is drawn over an unconfigured machine on the base
  map. It goes through the `Painter` in `render/` and follows the drawing
  seam.
- **The recipe menu** opens from the machine:
  - It lists the researched recipes, with inputs, output and cycle pace.
  - The pace is in player vocabulary, never "ticks".
  - Rows are picked with lowercase letters; actions use uppercase.
  - Picking a recipe on a machine that already has one asks for confirmation,
    because it empties the machine.
- **The build menu** loses the 17 removed structures and gains five: the
  four machine kinds and the Workbench.
- **The Workbench compile menu** opens only when you are next to it.

## Goals

- **Fewer entries in the build menu.** Any production spot can be
  repurposed without tearing it down.
- **Every current production chain stays buildable**, with the same inputs,
  outputs and pace.
- **The player always decides** what each machine makes.

## Non-goals

- New recipes, multi-input recipes, and rebalancing.
- Service structures.
- Research Station.
- Teardown Rig. It strips downed programs and is not a recipe machine.
- Save migration.

## Anti-goals (a failure even if it works)

- **The base changes a recipe on its own** through work orders, auto-setup
  or anything else.
- **A production chain gets lost:** something buildable before is no longer
  producible.
- **Content gets hardcoded in Rust** that used to live in `.ron`.
- **A machine with no recipe consumes anything:** a worker, power, or a work
  order's attention.

## Left to the builder

- Component and field names.
- Whether buffer capacity is set per recipe or per kind.
- Each kind's exact build cost, near the cheapest structure it replaces.
- Glyphs and colours for the four kinds, the Workbench and the question
  mark.
- How a re-baked pace interacts with a posted worker: re-bake the pace, or
  unassign the worker.
- Help pages under `assets/help/` that name removed structures. Achievements
  and talent trees that name removed structures: re-point or remove them.
