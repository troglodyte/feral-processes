# Research nodes (mods)

Every `*.ron` file in this directory is one node of the research tree. Drop
a file in and it becomes a node the next time a game session starts — no
recompiling required. A malformed file is skipped with a warning logged
in-game rather than crashing startup.

Research Data is the currency. It comes from a Research Node structure
worked by an assigned tamed program, the same way a Mining Node produces
Core Fragments.

## Schema

```ron
(
    // Unique id across all research files. Other nodes reference this in
    // their `requires`.
    id: "weapon_bench",

    // Shown in the research menu (`T` in game).
    //
    // Say what the node *is*; what it hands over and what it lets the base
    // turn into what are both added for you, so a modded node gets those
    // lines free and no authored copy can go stale when a cost is retuned.
    //
    // Above the description the menu draws one cyan line naming everything
    // the node hands over, by display name: the structures, the results of
    // its recipes, and the tools, in that order. A base-tree node never
    // grants a routine directly any more — see "Routines are a separate
    // tree" below.
    //
    // Below it, one cyan line per conversion the node makes possible — a
    // recipe in `unlocks_recipes` below, or a structure in
    // `unlocks_structures` that sets `assembles` — derived from the recipe
    // itself.
    name: "Weapon Fabrication",
    description: "The bench where weapons and modules are made. It also turns Logic Wafers into Trace Sniffers.",

    // Research Data a project on this node has to accumulate before it
    // completes. Research Nodes feed it in while the node is the base's active
    // project; nothing is spent at selection.
    cost: 18,

    // Optional; defaults to none. Goods consumed alongside `cost`, as
    // (item id, quantity) pairs. Selecting the node files one work order per
    // line at the top of the queue, so the base makes them the way it makes anything else;
    // the bill is then paid off the base's own shelves — every Depot and every
    // machine output buffer, never the player's pack — and paid whole, so
    // nothing is spent unless the entire bill can be covered.
    //
    // A line nothing standing in the base could ever make refuses the
    // selection outright, with the same sentence the work-order screen shows.
    materials: [("logic_wafer", 6), ("bytecode_block", 8)],

    // Optional; defaults to 0, meaning available from turn one. The zone the
    // player must have reached before this node can be researched. Below it
    // the node is still listed, and says which zone it is waiting on.
    min_zone: 3,

    // Optional; defaults to none. Node ids that must already be unlocked
    // before this one can be taken.
    requires: ["automation"],

    // Optional; defaults to false. Marks this node as somewhere an opening
    // run should be heading: the research menu draws it green, and draws
    // everything it requires green too, so the green row is always one that
    // can actually be bought right now. Flag the destination only — the
    // chain to it is derived, so inserting a node into the middle of a
    // recommended path keeps the path intact.
    recommended: true,

    // Optional; defaults to none. Structure ids this node makes buildable.
    // A structure named by NO research file is buildable from turn one.
    unlocks_structures: ["fabricator"],

    // Optional; defaults to none. Craft recipes this node makes available.
    unlocks_recipes: [(
        // An item id — see assets/items/README.md for the schema, and the
        // top-level README's "Item ids" for the full set of shipped ids.
        result: "overclock_core",
        // What one unit costs, as (item id, quantity) pairs.
        cost: [("portal_fragment", 6)],
        // Optional; defaults to no bench requirement. The recipe only
        // appears in the compile menu while a structure of this kind is
        // deployed — researching the blueprint is not enough on its own.
        requires_structure: Some("fabricator"),
    )],

    // Optional; defaults to none. Tool ids this node hands over the
    // knowledge to forge — see assets/tools/README.md. What it hands over is
    // knowledge, not an item, so a forged tool still has to be installed
    // into a slot before it does anything.
    unlocks_tools: ["core_tap"],

    // Optional; defaults to false. Set this on the one node that should open
    // the routine research tree (the shipped tree sets it on
    // `routine_fabrication`). Until a loaded node carrying it is researched,
    // the routine tree lists nothing at all and researching any routine node
    // is refused. If no loaded node carries it, the routine tree is open
    // from the start — so deleting the flagged node (or the whole file) does
    // not strand a run behind a gate nothing can ever open.
    opens_routine_tree: true,

    // Optional; defaults to false. Refuses selection until a tamed program
    // is standing in a Research Station's pen — see `assets/structures/
    // README.md`'s `studies` field. The node stays listed either way
    // (`ResearchState::Locked` and the block reason are the disclosure, not
    // hiding the row); the refusal names the same sentence the menu already
    // marks the row blocked with. Any owned tamed program satisfies any
    // study — which one does not matter — and completion consumes it into a
    // `DownedProgram` record in your store, exactly as if it had been
    // defeated in the field. Every shipped node with `min_zone >= 2` sets
    // this; the eight that get a base running from turn one
    // (`automation`, `power_grid`, `commerce`, `teardown`, `fortification`,
    // `armor_bench`, `routine_fabrication`, `weapon_bench`) do not, so a
    // fresh run never needs a subject before it has a base worth studying
    // anyone at. A synthesised routine node never sets this — the routine
    // tree has its own economy already.

    // Optional; defaults to false. Set this on the one node that should
    // unlock fusing two tamed programs together (the shipped tree sets it
    // on `program_refactoring`). Until a loaded node carrying it is
    // researched, fusing is refused. If no loaded node carries it, fusion
    // is open from the start — `opens_routine_tree`'s exact lenient rule —
    // so deleting the flagged node (or the whole file) does not strand a
    // run behind a gate nothing can ever open. There is deliberately no
    // structure or locality requirement for fusion beyond this: the
    // research alone unlocks it.
    unlocks_fusion: true,
)
```

## Routines are a separate tree, and you cannot author one here

Every non-passive, non-exclusive, non-permanent, non-summon ability gets its
own research node automatically — one per ability, derived from the ability
catalogue rather than written as a `.ron` file. See
`assets/abilities/README.md` for what determines an ability's place in that
tree (its family, its rung and its zone gate) and `../../docs/superpowers/
specs/2026-09-16-routine-research-tree-design.md` for the design.

A synthesised node's id is `"routine/<ability id>"`, which is not a legal id
to give a file here — you cannot mint one, override one, or make a
`requires` entry that resolves to one from this directory. Its `tree` field
reads `Routines`; every node authored in this directory reads `Base`, the
default, so nothing here has to change to keep meaning what it always meant.
Its `teaches` field names the one ability it grants; a node in this
directory never sets it, and a research node granting a routine through any
means but its own synthesised node is retired — that is what
`unlocks_abilities` used to do, and the field is gone. An old `.ron` file (or
a `dev-saves/` template) still naming `unlocks_abilities` keeps loading —
this parser, like every other in the game, drops an unknown field silently
rather than refusing the file — it simply grants nothing through it any
more.

## Rules

- **A recommended node's whole prerequisite chain is recommended with it.**
  So flagging a deep node marks the way there, and there is no need — and no
  point — in flagging every step. A tree that flags nothing simply offers no
  advice; the menu then draws every available node the same.
- **A structure named by no research file is buildable by default.** That is
  how the Home, Mining Node, Research Node, Recharger Node and Zone Portal
  stay available from the start, and it means a structure mod that ships no
  research file keeps working unchanged.
- A node naming an unknown prerequisite, or an unknown structure in
  `unlocks_structures`, is dropped at load time with a warning — it could
  never be reached or acted on. Dropping cascades: anything that required
  the dropped node goes too.
- An unknown id in `unlocks_tools` is treated more gently: that id is
  dropped with a warning and the node itself still loads, because a node's
  structures and recipes are innocent of a bad tool id.
- **A node must not be gated below its own prerequisite.** If `min_zone` is
  lower than that of anything in `requires`, the prereq lock always outlives
  the zone lock and the gate can never be the reason the node is unbuyable —
  it reads in the menu as a reason that disappears without the node becoming
  available.
- **Nothing needed to breach may sit behind a gate.** A node naming the Zone
  Portal in `unlocks_structures` must leave `min_zone` at 0, or the run
  softlocks: the structure that opens the next zone would be waiting on the
  zone it opens. Researching the portal is fine; gating it is not.
- **A node may only ask for materials its own prerequisites can make.** The
  legal set is worked out from `requires` alone: structures no research file
  gates, plus those the node's prerequisites unlock, plus everything that set
  can craft. `min_zone` grants nothing — a zone number says the player
  breached, not that they took any particular node — so leaning on it makes a
  silent, unstated prerequisite, which is the failure the census
  `every_research_material_is_reachable_through_that_nodes_own_prerequisites`
  exists to catch. A node whose bill names something out of reach fails the
  build.
- A structure with a `work` block makes its product out of nothing on a timer
  and so counts as a source; one with `assembles` does not, because it runs
  its product's own `craftable.cost` and is already covered by that recipe.
- The ICE Breaker and Power Cell recipes are always available and are not
  defined here.
- Nodes are listed cheapest first, ties broken by id, so the menu numbering
  is stable across sessions.

The filename doesn't matter to the loader (only the `id` field does), but
name it after the node for readability, e.g. `weapon_bench.ron`.
