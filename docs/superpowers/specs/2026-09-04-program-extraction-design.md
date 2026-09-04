# Program extraction

**Status:** approved, unimplemented

Kills stop paying materials directly. A defeated wild program is left as a
carried, instanced **downed program**; carried **tools** extract materials,
parts and routines from it. The extraction replaces `SpeciesDef::
work_resource`'s drop; gear drops are untouched until phase 5.

## Decisions taken

Recorded so they are not relitigated. Each was chosen against a named
alternative.

1. **A downed program is instanced, not a stackable item.** It carries the
   kill's species, level, rarity, boss flag and condition, so a level-30
   Prismatic kill is not interchangeable with a level-2 one. Rejected: an
   `ItemId` per species (stacks, but every kill of a type is identical).
2. **It lives in a third store, not in `Inventory`.** `Inventory` is
   `Vec<(ItemId, u32)>` whose `count`/`take` read the *first* matching row
   (`components.rs:549`, `:573`), and `components.rs:477` states the seam:
   `Inventory` is by definition the plain-copy store, which is what lets
   recipes, `Stock`, `assembler_system`, hauling and banking read it with no
   instance rule. `GearCopies` is the existing sibling for exactly this
   problem; `DownedPrograms` is the second.
3. **One tool consumes the whole program.** Choosing the tool *is* the
   decision. Rejected: layer-stripping (richest, most per-program state).
4. **The tool decides what kinds come out; the program's quality decides how
   good and how many; the tool's tier decides both again.** No tool x species
   table.
5. **Species contributes one bonus part**, `rich_in`, defaulting to that
   species' existing `work_resource` — zero new authoring across the 17
   shipped species, and the already-balanced mapping survives.
6. **Tools mirror routines rung for rung** (catalogue, component, slot
   formula, research unlock, forge, install). The pattern is proven here and
   documented; a second shape for the same idea is a second thing to get
   wrong.
7. **Extraction works anywhere** — field, base, Stack. A structure improves
   yield and speed; it is not a gate. The starting tool is useless otherwise.
8. **Phase 1 is drop-neutral by construction.** The starter tool's expected
   yield per kill is tuned to today's `WORK_RESOURCE_DROP` expected value.
   Replacing material drops routes the whole early economy through one
   untested door; a mistuned starter tool is a dead run, not difficulty.
9. **A full store refuses the drop with a log line**, rather than discarding
   the worst held program. Never destroy something the player chose to keep.
10. **"Extract" is the verb.** `Game::extract_routine` already breaks a
    program down with that word; "decompile" is taken (it means capture).

## 1. The object

```rust
// crates/engine/src/items.rs, beside GearCopy
pub struct DownedProgram {
    pub species: SpeciesId,
    pub level: u32,
    pub rarity: Rarity,
    pub boss: bool,
    pub condition: u8,   // 0..=100
}
```

`components::DownedPrograms(pub Vec<DownedProgram>)`, player-only, capped at
`tuning::MAX_DOWNED_PROGRAMS`.

**Condition** is rolled once, at the kill:

```
condition = clamp(CONDITION_BASE
                  + CONDITION_PER_RARITY_STEP * rarity.index()
                  + boss as u8 * CONDITION_BOSS_BONUS
                  + FIGHT_CONDITION_WEIGHT * overkill_term,
                  0, 100)
```

`overkill_term` is how far the killing blow went past zero as a fraction of
`max_hp`, negated — a clean kill leaves more. `FIGHT_CONDITION_WEIGHT` is a
`tuning.rs` constant and **may be 0.0**, which switches the fight axis off
without removing the field. Ships at 0.0 until played.

`DownedProgram::grade() -> f32` is the one derivation of "how good is this
program", folding condition, rarity and level. Every yield formula calls it;
none re-folds the axes.

## 2. Tools

New moddable catalogue `assets/tools/*.ron`, loaded by `ToolDb::load_dir`
following `AbilityDb`'s pattern exactly — malformed file skipped with a
logged warning, absent directory loads silently empty, `iter` sorted by id.

```ron
(
    id: "salvage_clamp",
    name: "Salvage Clamp",
    description: "Prises the loose material off a downed process.",
    category: Materials,                 // Materials | Parts | Cores | Routines
    yields: [("core_fragment", 1.0), ("bytecode_block", 0.4)],
    tier: 1,
    ticks: 20,
)
```

`category` is a fixed enum — it groups the tool screen and is what "a tool
reaches a different part" means. `yields` is `(ItemId, weight)`; weights pick
which item per unit drawn. A `Routines`-category tool has an empty `yields`
and takes the routine branch instead.

**Slots.** `components::Tools(pub Vec<ToolId>)` on the player, bounded by
`tools::player_tool_slots(level)` — the same shape as
`abilities::player_routine_slots`, with `TOOL_SLOT_BASE`,
`TOOL_SLOT_PER_LEVEL`, `TOOL_SLOT_CAP` in `tuning.rs`. No Power cost.

**Acquisition**, mirroring routines:

| Routines | Tools |
|---|---|
| `ResearchDef::unlocks_abilities` | `ResearchDef::unlocks_tools` |
| `KnownRoutines` | `KnownTools` |
| `etch_disk` -> `ItemId::etched(id)` | `forge_tool` -> `ItemId::tool(id)` |
| `install_disk` consumes into a slot | `install_tool` consumes into a slot |

`ItemId::etched` is already a synthetic id with no `.ron` behind it, so tool
carrier items need no item files either. `uninstall_tool` does not hand the
carrier back — what is in the slot *is* the tool, `install_disk`'s rule.

The starter tool (`tuning::STARTER_TOOL_ID`) is forged into slot 1 at
`Game::new`, the way `grant_starting_program` works. It is granted at
creation and never at `Game::load`, the profile rule.

## 3. Yield

```
Game::extraction_yield(&DownedProgram, &ToolDef, structure_tier: u32)
    -> Vec<(ItemId, u32)>
```

One derivation, called by the act and by the preview the screen draws — the
`BuildOrderRow` rule, so a quoted figure and a granted figure cannot differ.

```
units = round(TOOL_BASE_UNITS * tier_scale(tool.tier + structure_tier)
              * program.grade())
```

Each unit draws one item from `yields` by weight. Then `rich_in` adds
`RICH_IN_UNITS` of the species' bonus part, on top, from any tool.

`SpeciesDef::rich_in: Option<ItemId>` is `#[serde(default)]` and **falls back
to `work_resource`** when absent, so no shipped species file changes.

## 4. The act

```
Game::extract_program(index: usize, tool: &ToolId) -> Result<(), String>
```

The one door. Order, following `commit_caravan_basket`: every refusal lands
before anything is spent — game-over, active battle, no such program, tool
not installed, routine branch's "already known". Then the program is removed,
the yield granted through `grant_loot` with a new `LootSource::Extract`, the
line logged, and `self.tick()` spends `tool.ticks`.

Standing on a structure whose `StructureDef::extracts_programs` is set passes
its `StructureTier` as `structure_tier`. Reuses `can_extract_routines`'
"is one standing?" rule (`routines.rs:456`) rather than proximity.

**Routine category.** A `Routines` tool run on a downed program takes
`extract_routine`'s two branches — ordinary yields the knowledge into
`KnownRoutines`, exclusive pops the etched disk back out. Verified safe:
`routine_is_exclusive` reads `AbilityDef::exclusive` alone
(`routines.rs:188`), so nothing about the one-copy invariant rests on the
program having been tamed. The tamed-program path stays; both call one shared
inner function, not two copies.

## 5. Sources

| Site | Today | After |
|---|---|---|
| `award_loot` (`combat_rewards.rs:544`) | `roll_work_resource_drop` | `leave_downed_program` |
| `grant_nest_cache` (`zone.rs:88`) | same `work_resource` roll | leaves `NEST_CACHE_PROGRAM_COUNT` programs |
| boss kill | gear / fragments / ring | those, plus a program at a rarity and condition floor |
| sortie | `Sortie::loot` | plus `Sortie::programs` |

`roll_work_resource_drop` is deleted, not left dead. Its `Perk::Teardown`
term moves into the yield formula — that perk must keep being worth
something, and it is the trap the function was extracted to close.

Save: `DownedPrograms`, `Tools`, `KnownTools`, `SortieSave::programs`, all
additive behind `#[serde(default)]`. **No `SAVE_FORMAT_VERSION` bump.**

## 6. Screens

- `Mode::DownedPrograms` — the store, reached from the pack. One row per
  program: species, level, rarity, condition, boss tag. Selecting one offers
  the installed tools and the yield each would give, then extracts.
- `Mode::Tools` — the slots, one row each, reached from the party menu.

Both are read-only row lists with **no scroll**, so height is a layout
constraint: `MAX_DOWNED_PROGRAMS` and `TOOL_SLOT_CAP` must both fit at
1280x720, asserted by test and verified by mutation. Both join
`ALL_MODES` and `needs_status_banner`.

The engine owns the row count and gui draws it; any per-row transform lives
in the engine, the `message_history` rule.

## 7. Testing

Censuses in `tests/assets.rs`, each failing the build:

- every shipped tool's `yields` resolve to real items
- every shipped tool's `category` is reachable and non-`Routines` tools have
  a non-empty `yields`
- `STARTER_TOOL_ID` resolves to a shipped tool
- every `rich_in` (authored or fallen back) resolves to a real item
- every `unlocks_tools` id resolves

Engine tests:

- a kill leaves exactly one program carrying that creature's species, level
  and rarity
- a full store refuses the drop, logs, and destroys nothing
- extraction removes the program and grants the yield; the previewed figure
  equals the granted one
- every refusal path spends nothing — asserted **per refusal**, since one
  test over one path passes against the others
- an exclusive routine extracted from a downed program leaves exactly one
  copy in the run
- `FIGHT_CONDITION_WEIGHT = 0.0` makes condition independent of the killing
  blow
- a save round-trip preserves a store of programs and a tool loadout (a RON
  round-trip alone cannot catch a `#[serde(skip)]`, so this is save->load)

`balance_sim` models no loot and gates none of this. The economy gate is
decision 8's drop-neutrality, asserted as a test comparing the starter
tool's expected yield to `WORK_RESOURCE_DROP`'s mean.

## 8. Phases

Each is its own plan and its own release.

1. **The loop.** Object, store, its screen, `ToolDb`, slots, starter tool,
   `extract_program`, drops replaced at kill and nest and boss. One or two
   shipped tools. No research, no structures, no routine category.
2. **The kit.** `unlocks_tools`, `KnownTools`, `forge_tool`, `install_tool`,
   the `Mode::Tools` screen, the full shipped tool set, slots growing with
   level.
3. **The base.** `extracts_programs` structure and its tier scaling; the
   routine category unified with `extract_routine`; sortie programs.
4. **Automation.** The bulk work-order path: `Carrying`, `Stock`,
   `collect::plan_adjacent_take` and work orders all become instance-aware.
   Larger than 1-3 together, and deliberately last. Its cost is re-estimated
   once 1-3 are real.
5. **The economy.** Gear drops move behind extraction — a `Parts`-category
   tool yielding gear, and `equipment_drops_for` retires.

## Open, deliberately

- Whether the fight axis (`FIGHT_CONDITION_WEIGHT`) is worth turning on.
  **Confirmed 2026-09-04: the field ships and the weight ships at 0.0**, so
  the axis exists, is tunable, and has no effect until played. Do not delete
  the field for being unused, and do not fit a non-zero value without a play
  session behind it.
- **The cap is a count now and a weight budget later.** Confirmed
  2026-09-04: `MAX_DOWNED_PROGRAMS` as a flat row count is what phase 1
  ships, with the intent to move to a carried-weight metric once the store
  has been lived with. That later move wants a `DownedProgram::weight()`
  *derived* from species and grade — the "derived, never stored" rule — so
  no save field is added now and none is needed then. Whatever replaces the
  cap keeps decision 9's refusal: a full pack refuses the drop and destroys
  nothing already held.
- Phase 4's real cost.
