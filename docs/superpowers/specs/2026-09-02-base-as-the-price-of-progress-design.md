# A working base is the price of progress

**Date:** 2026-09-02
**Branch:** `feat/base-as-the-price-of-progress`
**Source of the problem:** [`docs/base-economy-audit.html`](../../base-economy-audit.html),
read from `main` at `2586067a`.

## The problem

The audit traced every structure, recipe and rate in `assets/` and
`crates/engine/` and found that the breach path never touches the base. A
Home costs nothing; a Zone Portal costs 10 Portal Fragments, which drop from
Stack guardians alone. Neither is behind research. The eleven-machine
production chain consumes nothing on that path and is therefore optional —
and `Game::craft` lets the player hand-compile every item those machines make,
at the identical recipe cost, in **one tick**, with no worker, no adjacency,
no power and no proximity to the bench.

The base is a factory nobody has to visit.

## What this change does

Three independent edits, each closing one half of that:

1. **Hand-crafting costs real time**, an order of magnitude more than the
   machine that exists to do it. The recipe stays available; the convenience
   does not.
2. **The Zone Portal's bill demands one terminal product from every base
   chain**, in multiples, and the bill grows as sectors open.
3. **The power grid burns Power Cells to stay up**, so grid capacity is a
   production rate rather than a one-time purchase.

Together the answer to "do I need a base?" changes from no to yes, every
sector.

## Non-goals

Deliberately out of scope, each named so it is not smuggled in:

- **The other nine audit levers.** `max_deployed` caps on the Recharger Node
  and Data Cache, halved first-stage recipes, contract reward rises, cargo
  capacity, tier-scaled assembler output, the three inert `power_draw`
  values, `flat_payout`, `access_shard`. All still open; none are this change.
- **Blocking hand-crafting outright.** A `machine_only` flag was considered
  and rejected: slow is the chosen answer, so every recipe stays reachable.
- **Closing the pre-funding route.** Chain products survive a breach (see
  Risks). The per-zone ramp is the only pushback and that is accepted.
- **`Game::rest` spending zero ticks.** A real bug found in passing (its
  neighbour's doc cites a `REST_TICKS` constant that exists nowhere), logged
  here and fixed separately.

---

## 1 · Hand-crafting takes real time

### The number has one door

```
Game::hand_craft_ticks(&ItemId) -> u32
```

`HAND_CRAFT_TICK_MULT × cycle`, where `cycle` is:

1. the `ticks_per_unit` of the structure whose `assembles.item` is this item,
   if one exists; else
2. the `ticks_per_unit` of the structure whose `work.produces` is this item,
   if one exists; else
3. `HAND_CRAFT_DEFAULT_CYCLE`.

The crafting screen shows this call's result and the loop spends it. Per
CLAUDE.md's rule about doc comments that claim to mirror another formula,
there is no second copy of this arithmetic anywhere — the screen calls it.

At `HAND_CRAFT_TICK_MULT = 10` a Blank Substrate is 12t at a Lathe and 120t
by hand; a Hardened Shell is 30t at the Armory and 300t by hand.

### The loop

`Mode::CraftQuantity` gains a third step, `Mode::Compiling`. The engine holds
the in-flight order in `resources::HandCraft`, which is **not saved** —
`resources::RunFeats`' precedent, and safe because `Mode::Compiling` is a
blocking mode with exactly two exits and no save inside it.

- app-core calls `Game::advance_hand_craft()` once per frame. It spends one
  tick and reports progress.
- Any key calls `Game::abort_hand_craft()`.
- A battle starting or a game over ends the loop early — drag terrain's
  precedent (`move_player`'s `for _ in 0..drag_ticks` loop, which breaks on
  exactly those two conditions).

### Ingredients are spent per unit

At each unit's start, not once for the batch. The unit's quality is rolled
and the item granted at that unit's end, as today. So an abort keeps every
completed unit and refunds the in-flight one, and the only thing an abort
costs is the time already spent.

This follows the existing seam — *materials are not spent until the structure
is raised* — rather than inventing a second rule about partial payment. It
also closes an edge the deferred alternative opens: a build crew can take
from the player's pack while the party stands in base space
(`construction.rs`'s `Source::Pack`), so a craft that checked at the start and
spent at the end could find itself short after 300 ticks.

### Interface

| Symbol | Crate | Kind |
| --- | --- | --- |
| `tuning::HAND_CRAFT_TICK_MULT` | engine | new `pub const u32` |
| `tuning::HAND_CRAFT_DEFAULT_CYCLE` | engine | new `pub const u32` |
| `Game::hand_craft_ticks(&ItemId) -> u32` | engine | new, `pub` |
| `resources::HandCraft` | engine | new resource, unsaved |
| `Game::begin_hand_craft(&ItemId, u32, bool) -> Result<(), String>` | engine | new, `pub` — takes over `craft`'s refusals |
| `Game::advance_hand_craft() -> HandCraftProgress` | engine | new, `pub` |
| `Game::abort_hand_craft()` | engine | new, `pub` |
| `Mode::Compiling` | app-core | new variant |

### What becomes of `Game::craft`

It stays, with its signature and its refusal list unchanged, and is
**reimplemented on top of the loop**: begin, then drain to completion. That
keeps it the headless "compile this, right now" call every engine test
already uses, gives `begin_hand_craft` one place to hold the refusals, and
leaves no second copy of the spend-roll-grant sequence. `advance_hand_craft`
is the only code that spends a unit and grants it; `craft` and the UI are two
drivers of the same loop.

**Expect test fallout.** `craft` advances the clock by the full duration now
rather than one tick, so any existing test that crafts and then asserts on
something a background system touches may move. That is a real signal about
the change, not a test to paper over — but per the memory note about
RNG-stream shifts, a seeded assertion that moves should be re-grounded rather
than re-seeded.

---

## 2 · The portal's bill grows with the sector

### Schema

`StructureDef` gains one additive field:

```rust
#[serde(default)]
pub zone_build_cost: Vec<(u32, ItemId, u32)>,   // (min_zone, item, base_qty)
```

`Game::structure_build_cost` — already the one door every reader of a build
price goes through (build menu, deploy prompt, the filed request's stored
bill, the removal refund) — appends every `zone_build_cost` line whose
`min_zone <= zone`, and ramps **every** line, old and new, through
`zone_portal_cost(qty, zone - min_zone + 1)`. Lines in `build_cost` are
implicitly `min_zone: 1`, which makes that expression reduce to today's
`zone_portal_cost(qty, zone)` for them — so no existing structure reprices,
and a line introduced at sector 2 costs its authored base at sector 2 rather
than arriving pre-ramped.

The ramp remains gated on `zone_portal: true`, unchanged.

### Content

```ron
build_cost: [
    ("portal_fragment", 24),
    ("patch_routine",    4),
    ("hardened_shell",   3),
    ("routine_disk",     4),
],
zone_build_cost: [
    (2, "trace_sniffer",    2),
    (2, "cache_grain",     10),
    (3, "recompile_kernel", 3),
],
```

24 Portal Fragments is about four Stack guardians at the shipped
`STACK_BOSS_PORTAL_FRAGMENT_DROP` of `4..=8`, against roughly two today.

**Why these lines and not others.** The composition is constrained by
research, not taste. Cache Grain sits behind `cache_coherence` (40 RD,
zone 2) and the Recompile Kernel behind `program_refactoring` (75 RD,
zone 2), so neither can legally be demanded in sector 1 — that constraint is
the whole reason the schema field exists. What sector 1 *can* demand:

| Line | Chain | Research needed |
| --- | --- | --- |
| Patch Routine | Power Conduit → Winding Node → Assembly Bay | **none** |
| Hardened Shell | Mining Node → Refinery → Armory | `armor_bench` 24 RD |
| Routine Disk | Mining Node → Lathe → Disk Press | `routine_fabrication` 26 RD |

Trace Sniffer is deferred to sector 2 despite being legal in sector 1: adding
`weapon_bench` would take the opening research bill from 50 RD to 100, and
50 RD is already about 1,400 ticks on one Mk1 Research Node.

`assets/structures/README.md` documents the new field in the same change.

---

## 3 · The grid burns Power Cells

### Which suppliers burn

`StructureDef` gains a second additive field:

```rust
#[serde(default)]
pub power_upkeep: bool,
```

Authored `true` on `recharger_node.ron` and `line_driver.ron`. **Not** on
`home.ron` — the Home's free 4 is the bootstrap and must stay free, or a cold
start cannot run the Power Conduit that fuels the grid.

Data decides *which* suppliers burn; `tuning.rs` decides *how much*. That is
the moddability line as CLAUDE.md draws it: content is data, difficulty is
code.

### The mechanism

`components::PowerFuel { ticks_left: u32 }` rides every deployed structure
whose def sets `power_upkeep`. Each tick, `power_grid_system`:

1. decrements `ticks_left` on every fuelled supplier;
2. for any that reached zero, tries to take one Power Cell from an
   orthogonally adjacent output buffer, **reusing the assembler's existing
   pull helper rather than a second copy of it**; on success resets
   `ticks_left` to `POWER_UPKEEP_TICKS`;
3. computes the ledger with `power_supply` counted **only** for suppliers
   with `ticks_left > 0` or no `power_upkeep` at all.

A supplier that could not pay is announced once as `MachineStatus::Starved`
through `set_machine_status`, which is the one place a stall is announced and
logs only on transition. `Starved` is the right existing variant and no new
one is added — `MachineStatus`' matches are exhaustive by design, so a new
variant would cost every reader a case.

The existing `dark` cut in `ledger` then takes machines offline in the order
it already uses. Nothing about deficit handling changes.

### The rate

`tuning::POWER_UPKEEP_TICKS = 20`.

A Power Conduit at Mk1 in zone 1 yields one Power Cell per 6 ticks — 166 per
1,000 ticks. A burning supplier consumes 50 per 1,000 ticks. So one Conduit
sustains three Rechargers (+12 grid) while drawing 1 itself and occupying one
posted program. The loop closes: the Conduit is on the grid it feeds.

Cold start: Home's free 4 covers a Power Conduit (1) + a Mining Node (1) + a
Lathe (2) exactly.

### Save

`PowerFuel` persists on the structure save behind a `default_*` fn returning
`POWER_UPKEEP_TICKS`, so an existing save's suppliers load fuelled rather
than dry.

---

## Save format

**No `SAVE_FORMAT_VERSION` bump.** Every schema addition is
`#[serde(default)]` on a `.ron`-loaded def or on a save struct, which
CLAUDE.md's save seam says costs no version bump. `resources::HandCraft` is
not saved at all.

Per the memory note that a RON round-trip cannot catch a skipped field, the
`PowerFuel` field needs a save→load test of its own, not just a round-trip.

## Testing

Every task is TDD: failing reproducer first. Specific intents:

- **Hand-craft ticks.** `hand_craft_ticks` returns `10 ×` the Lathe's cycle
  for a Blank Substrate, `10 ×` the Power Conduit's for a Power Cell, and the
  default for an item no structure makes. A batch of 3 advances the clock by
  `3 × hand_craft_ticks`. An abort mid-unit refunds that unit and keeps the
  completed ones. A battle starting ends the loop early.
- **Portal cost.** At zone 1 the bill is exactly the four `build_cost` lines;
  at zone 2 it gains Trace Sniffer and Cache Grain, each at its authored base;
  at zone 3 it gains the Recompile Kernel at base while the sector-1 lines are
  at 2× and the sector-2 lines at 1.5×. A non-`zone_portal` structure with a
  `zone_build_cost` line gets it appended unramped. A filed request keeps the
  price it was filed at across a breach.
- **Power upkeep.** A Recharger Node beside a stocked buffer stays lit across
  `POWER_UPKEEP_TICKS × 2` and consumes exactly 2 cells. Beside an empty one
  it goes dark, contributes 0 supply, and logs `Starved` **once**. The Home
  never consumes and never goes dark. A save→load round trip preserves
  `ticks_left`.
- **Censuses.** `Mode::Compiling` must be added to every exhaustive census
  over `Mode`; the plan names them from the plumbing trace.

**Gates:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
`balance_sim` models no abilities, no crafting, no portal and no power, so it
gates none of this — run it anyway to confirm the curves have not moved.

## Risks

1. **Sector 1's research bill becomes ~50 RD before the first breach** —
   about 1,400 ticks on one Mk1 Research Node. This is the figure most likely
   to be wrong, and it is a one-line dial (drop `hardened_shell` or
   `routine_disk` from `build_cost`).
2. **Chain products survive a breach.** `enter_next_zone` wipes only the
   player's stacks of the two economy-role currencies (Portal Fragment,
   Core Fragment); structure buffers travel untouched, because the base is out
   of phase and a breach does not touch it. A sector can therefore be
   pre-funded from the previous one. Accepted: pre-producing still requires
   the base to have run, and the ramp taxes it.
3. **`HAND_CRAFT_TICK_MULT = 10` is unmeasured.** `docs/measurements/` has no
   entry for base throughput and the audit itself is blind to play. Nothing
   here has been observed in a session.
4. **The three inert `power_draw` values** on the Repair Bay, Log Analyzer Bay
   and Sandbox stay inert. Making the grid a real constraint sharpens the
   question of what they mean, but answering it is audit lever L10, not this
   change.

## Work breakdown

Four tasks. A, C and D are independent of each other and of everything else;
B depends on A's interface only.

| | Task | Crates | Depends on |
| --- | --- | --- | --- |
| **A** | Hand-craft timing: constants, `hand_craft_ticks`, `HandCraft`, begin/advance/abort | engine | — |
| **B** | Crafting UI: `Mode::Compiling`, key handling, progress bar, `Mode` censuses | app-core, gui | A |
| **C** | Portal recipe: `zone_build_cost`, `structure_build_cost`, `portal.ron`, README | engine, assets | — |
| **D** | Power upkeep: `power_upkeep`, `PowerFuel`, ledger, save field, assets | engine, assets | — |
