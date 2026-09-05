# Custom tools (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup. That
includes a file whose `yields` weight isn't a finite, positive number: RON
accepts bare `NaN` and `inf` literals, and a weight of zero or below can
never win a weighted pick, so either disqualifies the whole file.

Unlike `../abilities/`, an absent `tools/` directory is not an error — it
loads silently empty, the pre-extraction game.

A tool is what a downed program (`items::DownedProgram`, carried in
`components::DownedPrograms` after a kill) is extracted with, through the
one door, `Game::extract_program`. It sits in a player tool slot
(`components::Tools`, sized by `tools::player_tool_slots`); the starter
tool is forged into the first slot at creation, and a later phase adds
research, forging and installing more.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all tool files
    name: "Salvage Clamp",        // shown wherever a tool is listed
    description: "Prises the loose material off a downed process.",

    // Which part of a downed program this tool reaches. One of:
    //
    //   Materials   raw salvage — the starter tool's category
    //   Parts       intermediate components
    //   Cores       compiled cores
    //   Routines    the program's installed routine, not an item at all —
    //               see "The Routines category" below
    //
    // Fixed and closed: the engine groups the tool screen by this field, so
    // there is no free-text category to invent.
    category: Materials,

    // (item, weight) pairs. One unit drawn from a use of this tool picks an
    // item from this pool by weight, the same relative-weight idiom
    // `../abilities/README.md`'s `wild_weight` uses: an entry at 1.0 is
    // twice as likely as one at 0.4, and the pool is normalised at draw
    // time. Every id must name a real shipped item.
    //
    // Required and non-empty for every category except `Routines`, which
    // takes no `yields` at all — see below.
    yields: [("core_fragment", 1.0), ("bytecode_block", 0.4)],

    // Scales the unit count one use produces, alongside the structure the
    // extraction happens at (a later phase's `StructureDef::
    // extracts_programs`). Not a speed rating — two tools at different
    // tiers in the same category aren't "faster", they reach further into
    // the same pool.
    tier: 1,

    // Game ticks `Game::extract_program` spends on one use — the same
    // currency `power_cost` is to a routine but paid in ticks rather than
    // Power.
    ticks: 20,

    // Optional; defaults to empty. What `Game::forge_tool` spends to grant
    // one carrier of this tool, as (item id, quantity) pairs — a routine
    // spends a flat blank Routine Disk because every routine is the same
    // interchangeable object, but tools differ by tier, so a tier-2 tool
    // must be able to cost more than a tier-1 one. Knowing a tool
    // (`../research/README.md`'s `unlocks_tools`) is not enough on its own
    // to forge one if this is non-empty and the cost isn't held.
    forge_cost: [("core_fragment", 3)],
)
```

## The `Routines` category

A `Routines` tool has no `yields` — omit the field, or write `yields: []`;
both parse the same way, since it defaults to empty. Running one on a
downed program is a different act entirely: it takes the routine branch a
later phase adds (`extract_routine`'s two paths — ordinary teaches the
routine, exclusive pops the etched disk back out), never a weighted draw
from an item pool.

No `Routines` tool ships yet — the branch it needs doesn't exist until a
later phase, so the census holding every other category to a non-empty
`yields` (`assets.rs::every_non_routines_tool_has_a_non_empty_yield_pool`)
currently walks nothing in this category and passes vacuously. That's
correct, not a gap: the day a `Routines` file ships, the same census starts
checking it for real.

## The starter tool

`tuning::STARTER_TOOL_ID` (`crates/engine/src/tuning.rs`) names the tool a
new game forges into the player's first tool slot. `salvage_clamp` ships as
the value today. Renaming a shipped tool's `id` without also moving this
constant breaks that grant; the census
`assets.rs::starter_tool_id_resolves_to_a_shipped_tool` catches it.

Granted at creation only, on `abilities::DECOMPILE_ABILITY_ID`'s terms — a
new game gets it, a loaded save does not add it retroactively. Its own
`forge_cost` prices a *replacement* only — it never gates the one
`Game::new` grants.
