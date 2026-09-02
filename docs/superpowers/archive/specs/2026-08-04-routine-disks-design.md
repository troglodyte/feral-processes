# Routine Disks — design

**Date:** 2026-08-04
**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header.

## The problem

A routine is an item today. `ItemDb::synthesize_routines` mints one item per
loaded ability at startup, and researching a node that names abilities drops
free copies of those items straight into cargo. So the whole cost of a
routine is the research that unlocked it: the factory has nothing to do with
it, and the reward for a 46-point research node is three items appearing in
your bag.

This replaces that with a manufactured medium. Research grants *knowledge*;
the factory produces *blank Routine Disks*; installing a routine you know
spends one disk permanently. It closes the parked "researched routines should
be craftable" item and gives the base's production chains a second terminal
product.

## The model

A routine splits into two things that were previously one:

- **Knowledge** — a persisted set of ability ids the player has learned.
  Written by exactly two things: `unlock_research` (a node's
  `unlocks_abilities`) and `extract_routine`.
- **A Routine Disk** — a blank, fungible item. Not tied to any ability. One
  is consumed per install and is not recoverable.

Installing = knowledge + a disk + a free slot. Uninstalling frees the slot
and destroys nothing further — the disk is already gone. That makes slot
assignment a real commitment rather than free swapping, which is the point.

A species' innate routines are unaffected at spawn: `install_innate_routines`
still grants them directly, no knowledge or disk required. But popping one
out is permanent unless the player knows that ability, which is consistent
with `fusion_routine_losses`' existing warning that manually-installed
routines do not survive.

## Engine changes

### New

- `resources::KnownRoutines(BTreeSet<AbilityId>)` — persisted. A `BTreeSet`
  for the same reason `Stock` uses a `BTreeMap`: a `HashSet` would make the
  save encoding differ run to run.
- `Game::knows_routine(&AbilityId) -> bool`.
- `Game::installable_routines() -> Vec<KnownRoutineView>` — id, name,
  description, sorted by name. The install picker's rows.
- `Game::routine_disks_held() -> u32`.

### Changed

- `install_routine(entity, ability: &AbilityId)` — was `item: &ItemId`.
  Refusal order: game-over/battle, holder ownership, known, free slot, disk
  in cargo. The disk is spent last, after every check, for the same reason
  `use_symlink` clears the locale only after all checks pass.
- `uninstall_routine(entity, slot)` — no longer returns an item, so its
  `check_room` call goes with it.
- `extract_routine(creature, index)` — adds the ability to `KnownRoutines`
  instead of adding an item; `check_room` goes. **Refuses when the ability is
  already known** ("You already know that routine.") so a misclick cannot
  destroy a tamed program for nothing. `extractable_routines` gains a
  `known: bool` per row so the picker can mark them.
- `unlock_research` — adds `def.unlocks_abilities` to `KnownRoutines` instead
  of minting items. Its aggregated `check_room` block goes with them.
- `SAVE_FORMAT_VERSION` 20 → 21. The world save gains a
  `known_routines: Vec<AbilityId>` field beside `researched`, sorted on write
  for the same reason that one is — the encoded bytes must not depend on set
  iteration order. `PlayerSave` is untouched; installed routines already save
  as ability ids per holder. Old saves are refused outright, so there is no
  migration.

### Deleted

- `ItemDb::synthesize_routines` and its collision-warning path.
- `abilities::routine_item_id`.
- `ItemDef::routine` (and its `README.md` entry).
- `Game::is_routine`, `Game::loose_routines`, `views::RoutineItemView`.
- The `sell_item` refusal for routine items, which existed only because
  those items did.

Deleting `ItemDef::routine` is the load-bearing part: it is what makes the
old "a routine is an item" model unrepresentable rather than merely unused.

## The factory

Four new structures and four new items, all `.ron`. No engine code — the
chain is expressed entirely in the existing `work` / `assembles` /
`craftable` vocabulary.

```
Log Scraper ──► Raw Trace ───────► Transcriber ─► Logic Wafer ─────┐
                                                                    ├─► Disk Press ─► Routine Disk
Mining Node ──► Core Fragments ──► Lathe ───────► Blank Substrate ─┘
```

The two chains are deliberately unlike each other: the substrate side hangs
off the Mining Node the player already has, so half the chain is a
one-structure addition to an existing base; the logic side needs its own
producer, so it costs a cronjob slot and floor space of its own. Neither
touches Research Data — the research economy is left exactly as it is.

### Items

| Item | Made from | At | Value |
|---|---|---|---|
| `raw_trace` | (produced on a timer) | Log Scraper | **1** — pinned by the floor rule |
| `blank_substrate` | 4 × `core_fragment` | Lathe | 3 |
| `logic_wafer` | 4 × `raw_trace` | Transcriber | 3 |
| `routine_disk` | 1 × `blank_substrate` + 1 × `logic_wafer` | Disk Press | 5 |

`raw_trace` must sit at `DEFAULT_ITEM_VALUE` because a `work.produces`
structure prints it out of nothing on a timer — its value is a Credit-per-tick
rate the recipe ceiling cannot see. The other three sit under their
ingredients, so none of them is a Credit press.

### Structures

- **Log Scraper** — `work: (produces: "raw_trace", ticks_per_unit: 10,
  level: Some(1))`. Upgradeable to tier 5 like the other producers.
- **Lathe** — `assembles: (item: "blank_substrate", ticks_per_unit: 12)`,
  `capacity: 20`. Wants a Mining Node touching it.
- **Transcriber** — `assembles: (item: "logic_wafer", ticks_per_unit: 12)`,
  `capacity: 20`. Wants a Log Scraper touching it.
- **Disk Press** — `assembles: (item: "routine_disk", ticks_per_unit: 20)`,
  `capacity: 10`. Wants both feeders touching it, so like the Assembly Bay it
  wants a corner.

Each recipe sets `requires_structure` to its own machine, matching the
shipped Refinery/Winding/Assembly trio: hand-crafting stays the manual
fallback for a machine you own rather than a way around building it. That is
also what keeps them clear of
`only_the_starters_and_scavenged_gear_need_no_research_or_bench`.

Build costs follow the existing tier: producers around 12–14 Core Fragments,
assemblers around 18, the Disk Press 20 plus 4 Bytecode Blocks (mirroring the
Assembly Bay, so the press wants the older chain already running).

## Gating

A new research node `routine_fabrication`, requiring `automation`, unlocking
all four structures.

`field_ops` gains `routine_fabrication` as a second prerequisite (it already
requires `self_exec`). Without that, the first ability-granting node in the
tree hands the player knowledge they have no way to install, which reads as a
broken reward rather than as a goal. Every other ability node descends from
`field_ops`, so one edge covers the tree.

**Stated consequence:** the early game now has no installable routines at all
until `routine_fabrication` is taken and four machines are standing. That is
the intended trade — routines move from a research reward to a manufactured
good — but it is a real pacing shift, and nothing in `balance_sim` gates it.
It needs play, not a green suite.

## UI

- `crates/app-core/src/app/routines.rs` — the install picker's rows come from
  `installable_routines()` rather than `loose_routines()`, and the screen
  shows the disk count. Selecting with zero disks refuses with the engine's
  message rather than being hidden, so the player learns why.
- `crates/gui/src/render/routines.rs` — same swap, plus a "Disks: N" line.
- The extraction picker marks rows the player already knows.

## Testing

Engine (`crates/engine/src/tests/routines.rs`, largely rewritten — most of
its 863 lines assert item-based install):

- install refuses with no knowledge, with no disk, with no free slot, and on
  a holder the player does not own
- a successful install spends exactly one disk and no more
- uninstall frees the slot, returns no item, and leaves the ability known
- extraction teaches an unknown routine and destroys the program
- extraction refuses a routine already known, and the program survives
- research adds to `KnownRoutines` and puts no items in cargo
- innate routines still install at spawn with no disk and no knowledge
- `KnownRoutines` survives a save/load round trip

Assets (`crates/engine/src/tests/assets.rs`):

- `every_base_produced_item_sits_at_the_floor_price` — its `checked` count
  goes 4 → 5 for the Log Scraper
- new: every structure with `assembles` resolves a recipe whose
  `requires_structure` names that same structure, so a chain machine cannot
  ship pointing at a bench it is not
- the ceiling test picks the three craftables up with no change

Full `cargo test --workspace` is the gate, plus `cargo test -p
feral-processes-engine balance_sim` since new items enter the asset set.

## Docs

`assets/items/README.md` (drop the `routine` field), `assets/structures/
README.md`, `assets/research/README.md` if the new node needs a mention, the
root `README.md` and `CHANGELOG.md`, and the `CLAUDE.md` production-chain
notes — the "one converging assembler" description is about to be wrong.

## Out of scope

- Recovering a disk on uninstall, in any form.
- Pricing anything in disks other than an install.
- Any change to how `install_innate_routines` grants a species' kit.
- A second use for Raw Trace or Logic Wafers.
