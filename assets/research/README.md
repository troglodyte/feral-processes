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
    name: "Weapon Fabrication",
    description: "A bench for weapon and module work. Unlocks the Fabricator.",

    // Research Data spent to unlock this node.
    cost: 18,

    // Optional; defaults to none. Node ids that must already be unlocked
    // before this one can be taken.
    requires: ["automation"],

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

    // Optional; defaults to none. Ability ids this node compiles a routine
    // item for, straight into the player's cargo, once unlocked — see
    // assets/abilities/README.md. That's a separate act from using it: the
    // routine still has to be installed into a player slot (`m` in game)
    // before it does anything in battle, and it's refused up front if cargo
    // has no room for it. Companions never gain anything from this list —
    // their kit comes from their species file instead.
    unlocks_abilities: ["hot_patch"],
)
```

## Rules

- **A structure named by no research file is buildable by default.** That is
  how the Home, Mining Node, Research Node, Recharger Node and Zone Portal
  stay available from the start, and it means a structure mod that ships no
  research file keeps working unchanged.
- A node naming an unknown prerequisite, or an unknown structure in
  `unlocks_structures`, is dropped at load time with a warning — it could
  never be reached or acted on. Dropping cascades: anything that required
  the dropped node goes too.
- An unknown id in `unlocks_abilities` is treated more gently: that id is
  dropped with a warning and the node itself still loads, because a node's
  structures and recipes are innocent of a bad ability id.
- Two nodes may name the same ability. Each still deposits its own routine
  item on unlock — researching both stacks two copies in cargo rather than
  deduplicating, so a second copy is there to install into a companion or a
  future slot rather than being wasted.
- The ICE Breaker and Power Cell recipes are always available and are not
  defined here.
- Nodes are listed cheapest first, ties broken by id, so the menu numbering
  is stable across sessions.

The filename doesn't matter to the loader (only the `id` field does), but
name it after the node for readability, e.g. `weapon_bench.ron`.
