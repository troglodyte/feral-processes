# Research Tree — Design

Date: 2026-07-21

## Problem

Structure and recipe unlocks are currently implicit and shallow. Every
structure is buildable from the first turn if you can afford it, and the only
gate on gear crafting is "have you built an Armory / Fabricator". Three
structures — Armory, Fabricator, Data Cache — have no capability at all
(`work: None`, no trade/rest/process); the first two exist purely as recipe
gates and the third gates nothing.

The result is a flat progression: Core Fragments are the only real pacing
mechanism, and there is nothing to spend a mid-game economy on.

## Goal

A data-driven research tree that gates structures and craft recipes behind a
new resource the player must deliberately invest in producing. Research gates
*power*, not *progression* — the Zone Portal stays outside the tree so a
player can rush zones or stop and tech up.

## Non-goals

- Giving the Data Cache a real capability. It stays as-is for now; a later
  change makes it a storage device.
- Research-gating perks, species, or zone progression.
- Migrating existing v5 saves.

## Design

### Research Data economy

A new `ItemId::ResearchData` ("Research Data") — a plain resource with no
`equipment()` mapping. It comes from exactly one place: a **Research Node**
worked by an assigned tamed creature.

```ron
// assets/structures/research_node.ron
(
    id: "research_node",
    name: "Research Node",
    glyph: 'R',
    color: Cyan,
    build_cost: [(CoreFragment, 10)],
    work: Some((produces: ResearchData, ticks_per_unit: 14, capacity: 4, level: Some(1))),
)
```

Slower than the Mining Node's 10 ticks with a smaller buffer, so research is
the long-horizon investment rather than the fast one. Research Data has no
other use: it is not a craft input, and it sells at the iso Market's flat
`sell_rate` like anything else, which is deliberately a bad trade.

The Research Node pins third in the build menu — `StructureDb::all`'s
priority list becomes `home, mining_node, research_node, compiler`.

### Data model

A new `ResearchDb` in `crates/engine/src/research.rs`, loaded from
`assets/research/*.ron` exactly like `StructureDb`: one `.ron` per node,
malformed files skipped with a returned warning, never a panic.

```rust
pub type ResearchId = String;

pub struct ResearchDef {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    pub cost: u32,                                  // Research Data
    #[serde(default)] pub requires: Vec<ResearchId>,
    #[serde(default)] pub unlocks_structures: Vec<StructureId>,
    #[serde(default)] pub unlocks_recipes: Vec<ResearchRecipe>,
}

pub struct ResearchRecipe {
    pub result: ItemId,
    pub cost: Vec<(ItemId, u32)>,
    #[serde(default)] pub requires_structure: Option<StructureId>,
}
```

Recipe *data* moves out of Rust and into the research files. This is what
lets a mod ship a structure, its research node, and its recipes as pure data.
`FIREWALL_PLATING_PORTAL_COST`, `OVERCLOCK_CORE_PORTAL_COST` and the
`has_structure("armory")` / `has_structure("fabricator")` branches in
`Game::craft_recipes` are deleted. The two starter recipes — ICE Breaker and
Power Cell — stay hardcoded in `craft_recipes()` as the always-available
base, keeping their existing `balance.rs` constants.

Validation is at load time, and a failing node is skipped with a warning
rather than loaded:

- a `requires` entry naming an unknown node id
- an `unlocks_structures` entry naming an unknown structure id

A dangling prereq would otherwise leave a node permanently unresearchable
with no explanation. Validation therefore runs after both `StructureDb` and
every research file are loaded, not during per-file parsing.

### The tree

Default-unlocked, outside the tree: **Home, Mining Node, Research Node,
Recharger Node, Zone Portal**, plus the ICE Breaker and Power Cell recipes.

| Node | Cost | Requires | Unlocks |
|---|---|---|---|
| `automation` Automation | 8 | — | Compiler |
| `power_grid` Power Grid | 10 | — | Terminal, Power Conduit |
| `commerce` Isometric Commerce | 12 | — | iso Market |
| `fortification` Fortification | 15 | `power_grid` | Turret |
| `weapon_bench` Weapon Fabrication | 18 | `automation` | Fabricator |
| `armor_bench` Reactive Armor | 18 | `automation` | Armory |
| `cold_storage` Cold Storage | 20 | `commerce` | Data Cache |
| `overclock` Overclock Cores | 22 | `weapon_bench` | recipe: Overclock Core — 6 Portal Fragment @ Fabricator |
| `firewall` Firewall Plating | 22 | `armor_bench` | recipe: Firewall Plating — 6 Portal Fragment @ Armory |
| `neural_amp` Neural Interfacing | 25 | `weapon_bench` | recipe: Neural Amplifier — 6 Portal Fragment @ Fabricator |
| `monofilament` Monofilament Edge | 40 | `overclock` | recipe: Monofilament Whip — 12 Portal Fragment @ Fabricator |
| `ablative` Ablative Lattice | 40 | `firewall` | recipe: Ablative Plating — 12 Portal Fragment @ Armory |
| `cortex` Cortex Hacking | 45 | `neural_amp` | recipe: Cortex Hack — 12 Portal Fragment @ Fabricator |

Three deliberate choices:

- **Three independent tier-1 roots** (`automation`, `power_grid`,
  `commerce`) so the first research decision is a real branch rather than a
  forced line.
- **The Fabricator is both the Weapon and Module bench.** The Armory is the
  Armor bench. Two benches, not three — the Data Cache is left alone.
- **Tier-2 gear becomes craftable at the tree's tips.** Monofilament Whip,
  Ablative Plating and Cortex Hack are boss-loot-only today. Research turns
  them into a deterministic but expensive alternative; the existing
  `equipment_drop` entries stay as the lucky shortcut.

### Unlock state and engine API

Two new world resources: `ResearchDb` (the loaded defs) and
`Research(HashSet<ResearchId>)` (what is unlocked). Both live in the ECS
world; neither renderer touches them directly.

```rust
pub fn research_nodes(&self) -> Vec<ResearchStatus>
pub fn unlock_research(&mut self, id: &str) -> Result<(), String>
pub fn is_researched(&self, id: &str) -> bool
```

`ResearchStatus` carries id, name, description, cost, whether it is
affordable right now, and:

```rust
pub enum ResearchState {
    Unlocked,
    Available,
    Locked { missing: Vec<String> },   // prereq display names
}
```

`Locked` names the unmet prereqs so the menu can say *why* a row is
unavailable rather than just greying it out.

`unlock_research` fails with an explicit message when the id is unknown, the
node is already unlocked, a prereq is unmet, or the player cannot pay. On
success it consumes exactly `cost` Research Data from the player's inventory
and logs the unlock.

**The key rule: a structure named by no research file is buildable by
default.** There is no hardcoded whitelist of starter structures in Rust —
Home, Mining Node, Research Node, Recharger Node and Zone Portal are
available because no file in `assets/research/` mentions them. This also
means a mod that drops in a structure without a matching research file keeps
working exactly as it does today.

Gating applies in two places, not one:

- `Game::buildable_structures` filters the build menu.
- `Game::place_structure` rejects an unresearched structure id outright.

Menu filtering alone is not a gate.

`Game::craft_recipes` becomes: the two base recipes, plus every recipe from
an unlocked node whose `requires_structure` (if set) is currently built. A
researched recipe whose bench is not built simply does not appear — the same
behavior the Armory gate has today.

### Save format

Version 6 → 7, adding `researched: Vec<ResearchId>` to the save data.
Version 6 saves are rejected cleanly as `(incompatible save)` through the
existing path. No migration shim.

`difficulty.rs` does spawn a Terminal and a Data Cache, but only inside a
unit test, via `world.spawn` directly — it never routes through
`place_structure`, so research gating does not affect it.

### UI

A new `Mode::Research` in app-core, opened with `T` from `Mode::Playing`
(`r` is rest and `R` is remove; `T` is unused). It lists every node sorted
available → locked → unlocked, showing each cost against the player's
current Research Data, with number-key selection and Enter to unlock.

It follows the existing `Mode::Perks` shape exactly, in both
`crates/tui/src/ui.rs` and `crates/gui/src/render.rs`. No new UI patterns.

## Testing

Engine unit tests, all deterministic — no `sleep()`, no wall-clock
dependence, no unseeded RNG. Structures are placed explicitly rather than
relying on background spawning.

- `ResearchDb::load_dir` parses a valid node.
- `ResearchDb::load_dir` skips a malformed `.ron` with a warning, and keeps
  loading the rest.
- A node with a dangling `requires` id is skipped with a warning.
- A node naming an unknown structure id is skipped with a warning.
- `unlock_research` fails when a prereq is unmet.
- `unlock_research` fails when the player has insufficient Research Data.
- `unlock_research` on success consumes exactly the node's cost.
- The Fabricator is absent from `buildable_structures()` before
  `weapon_bench` and present after.
- `place_structure("fabricator", ..)` errors when called directly while
  `weapon_bench` is unresearched.
- Overclock Core is absent from `craft_recipes()` when researched but with no
  Fabricator built, and present when both hold.
- A structure named by no research file stays buildable (mod compatibility).
- A save round-trip preserves the unlocked research set.

Full gate before the work is called done: `cargo test --workspace`,
`cargo fmt`, `cargo clippy --workspace`.

## Documentation

- New `assets/research/README.md` documenting the `ResearchDef` and
  `ResearchRecipe` schema for modders, matching the style of the existing
  species and structures READMEs.
- `assets/structures/README.md` gains the Research Node and notes that a
  structure named by a research file is gated behind it.
