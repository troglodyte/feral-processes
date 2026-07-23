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
- The ICE Breaker and Power Cell recipes are always available and are not
  defined here.
- Nodes are listed cheapest first, ties broken by id, so the menu numbering
  is stable across sessions.

The filename doesn't matter to the loader (only the `id` field does), but
name it after the node for readability, e.g. `weapon_bench.ron`.
