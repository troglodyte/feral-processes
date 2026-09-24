# Outposts

`OutpostDb::load_dir` loads every `*.ron` file in this directory. A missing
directory is silent — no outposts exist and the founding kit refuses with a
message — and a malformed file is skipped with a logged warning rather than
crashing startup. **The first file by filename wins**: v1 ships exactly one
def, and a second file is a mod's own problem to resolve.

## Schema

```ron
OutpostDef(
    name: "Outpost",       // display name
    glyph: '⌂',             // drawn on the zone map
    kit: "outpost_kit",     // the item id that founds one when used
    repair_cost: [("logic_wafer", 4)],   // paid from the pack by [R] at zero integrity
    tiers: [
        (
            yields: {
                // Biome -> item ids this tier can produce there. A biome
                // with no row here falls back to this tier's first listed
                // biome, so a new `Biome` variant still yields something.
                Deadlock: ["raw_trace"],
                NullSector: ["raw_trace"],
            },
        ),
        // ...one entry per growth tier, in order (index 0 is tier 1).
    ],
    features: [],   // reserved for future extension points; always empty
)
```

Every field but `tiers` is `#[serde(default)]`, so a file that omits one
still parses. `repair_cost` follows the same refuse-the-whole-bill-before-
spending rule as a research node's own `materials:`.

`kit` names an item id, and nothing about founding one is content on the
item side beyond that: the founding kit is meant to be research-gated (see
`assets/research/README.md`'s `unlocks_recipes`), never given its own
`craftable` field on the item — a `craftable` recipe is always available
from turn one, which defeats the gate.

Tier 3 (the highest, "complex") should name at least one item this run's
material census (`ZONE_MATERIALS` in `crates/engine/tests/assets.rs`) treats
as a zone material, the same rule a zone-gated gear recipe follows.

Everything else about an outpost — growth rates, crew caps, integrity,
stock capacity, raid odds — is tuning, not content, and lives in
`crates/engine/src/tuning.rs`'s `// Outposts` section.
