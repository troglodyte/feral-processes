# Upgrading as a build request

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

`Game::upgrade_structure` stops charging the player's pack and stops
applying the tier. It files a `BuildSite` on the structure's own tile, and
the same crew that raises a deploy fetches the bill by hand and works the
site until the tier lands.

## Why

Build orders shipped on `feat/build-orders`: a deploy is a *request*, and
the Home is the only build the player's own hands finish. `upgrade_structure`
was deliberately left alone, because the goal said "deployable structures"
and upgrading is a different menu.

It is now **the only structure cost still paid from the pack**. The
complaint arrives the moment you stand on a full Depot beside a Mk1 Lathe
and are told "Not enough Cache Grain" — the base is holding it, the crew
can already fetch it for a deploy, and the verb reads the wrong store. The
stock strip says the base can build; the upgrade menu disagrees.

## Decisions

Settled in the brainstorm, recorded so they are not relitigated:

- **A full build request**, not a fetch-only job and not an instant charge
  against base stock. The crew fetches, sets down and works for
  `required_ticks`. Identical to a deploy end to end, which is the whole
  point: one crew, one scheduler want, one set of latches, one refund.
- **The machine keeps running while its upgrade stands.** Its worker stays
  posted and it produces at its old tier until the tier lands. Standing a
  machine down for its own upgrade would bring back the deadlock class
  build orders just closed, on a base that files three upgrades at once.
- **One `BuildSite` carrying a goal**, not a second `UpgradeSite`
  component. See below.
- **The tile keeps drawing the machine.** The site carries no `Glyph`; the
  structure's own view carries the pending row. The machine is still there
  and still working, so a build frame over it would be a lie about the
  tile.
- **The upgrade menu's cost display moves to `build_cost_display`** (pack
  plus base shelves) in the same change. Left on `cost_display` the menu
  quotes a store the verb no longer reads. This was the one open question
  at design time and is taken as included; it is a few lines in
  `render/building.rs`.

## The component

**The axis of change is what a finished site does, and nothing else.**
The bill, the fetching, the walk, the delivery, the dry and stuck latches,
the reachability check above the truncation, the refund on cancel and the
never-free-a-`Carrying`-holder rule are identical for both jobs. Exactly
one step differs — `raise_one_tick`'s completion.

The null hypothesis, a second component with its own crew pass, loses:
it duplicates all four rules build orders established, including the two
the suite missed the first time, and the copy that drifts is the one
nobody runs.

```rust
pub enum BuildGoal {
    New,
    Upgrade { to_tier: u32 },
}
```

`BuildSite::structure` **stays** and keeps its meaning — which structure
kind this site is about — because `structure_name`, the crew's log lines
and `count_build_requests` already read it that way. The goal sits beside
it. That makes the save field purely additive and keeps the site naming a
**tile** rather than an `Entity`: the structure being upgraded is resolved
by position at completion, so there is no `Entity` in the save, no
load-order dependency between structures and sites, and nothing to dangle
when the machine is destroyed under it.

## Filing

`Game::upgrade_structure` keeps every refusal it has, in the same order —
game-over/battle, `require_base`, structure gone, unknown def, no upgrade
path, `max_tier`, then the zone ceiling. It **drops** the `Inventory`
shortfall check and the charge, and it no longer inserts `StructureTier`
or touches `ResourceNode`.

It gains one refusal, distinct rather than folded in: an upgrade is
already on order at this structure. Distinct because it leaves the player
a different errand — the others need a breach or a different machine, this
one needs the standing request calling off.

Then it spawns a `BuildSite` at the structure's own `Position` with:

- `structure` — the machine's kind
- `goal: BuildGoal::Upgrade { to_tier: tier + 1 }`
- `cost` — `upgrade.cost` scaled by the tier being reached, exactly as
  priced today, snapshotted at filing for `BuildSite::cost`'s stated
  reason
- no `Glyph`

`required_ticks` is derived from that bill, so a Mk3 takes longer than a
Mk2 with no new constant and no second curve.

It still `tick()`s, as filing a deploy does.

## The crew and the scheduler

Untouched: `run_build_crew`, `step_one_builder`, `builder_errand`,
`walk_builder`, `pick_up_for_site`, `set_load_down`, `put_back_load`,
`build_is_workable`, `build_wants`, both announcement latches,
`can_walk_to_build` and `cancel_build_request`.

A builder stands on an **orthogonal neighbour** of its site
(`hauling::at_station` → `touching`) and never on the site tile, so an
occupied tile is a non-event for the walk: `blocked` is `structure_tiles()`
and the destination was never required to be walkable.

`raise_one_tick` branches on the goal at completion:

- `New` — `spawn_structure`, as now.
- `Upgrade { to_tier }` — find the structure at the site's `Position`,
  insert `StructureTier(to_tier)`, and set `ResourceNode::level` to the
  same figure **only when it is already `Some`** (a node that always
  succeeds stays that way — the rule `upgrade_structure` holds today).

Both arms re-check before committing and **leave the site standing**
rather than despawning when they cannot: the deploy arm's missing-def
precedent, extended to cover a machine that is gone and a tier the
ceiling no longer permits. Nothing the player paid for is destroyed by a
file they can put back, and the materials are still standing on the cell.

## Three silent traps

Each of these passes a green suite and is reachable by a player doing the
supported thing.

1. **`count_build_requests` counts by `structure` id.** Left alone, every
   pending upgrade counts against that kind's `max_deployed`, and a
   legitimate deploy is refused with a figure the player cannot account
   for. It must count `BuildGoal::New` only.
2. **Two destruction paths.** A machine destroyed by a raid
   (`upkeep::damage_structure`) or demolished by the player
   (`building::remove_structure`, including the Home cascade) must despawn
   its pending upgrade site and refund the delivered units through
   `return_material`. Wired into **both** — one alone strands goods on a
   cell nothing stands on.
3. **The job mark.** `build_views`' `attended` set pairs `Construct` with
   `GatherResource` on the stated grounds that a build site carries a
   glyph. An upgrade site does not, so that arm splits on the goal: the
   **builder wears the mark for the whole job**, `Excavate`'s rule and
   `Excavate`'s reason. Left alone, a machine's own worker and its builder
   fight over one mark and the rule "exactly one per posted program at
   every instant" lapses.

## What the player sees

- **The build-orders screen** picks upgrade requests up for free.
  `build_order_report` walks every `BuildSite` in tile order and
  `cancel_build_request` already works on one. `views::BuildOrderRow`
  gains the goal so a row reads `Lathe → Mk3` rather than `Lathe`.
- **The map** keeps drawing the machine and marks it. The structure's own
  `EntityView::build` carries the pending row, found by tile — the site
  has no glyph, so `view_entities` never produces two views for one cell.
- **The examine page** says what is still to be fetched, off that same
  row.
- **The upgrade menu** keeps listing a machine with a request standing and
  refuses on pick, rather than hiding it. Its costs draw through
  `build_cost_display`.
- **The log line** at filing says the crew has been told; the crew's own
  line at completion says the machine is now Mk*n*. `structure_name` is
  the one namer for both.

## Testing

TDD per task, failing reproducer first. The ones carrying weight:

- Filing an upgrade charges the pack nothing and leaves the tier alone.
- A posted crew fetches the bill and the tier lands at completion.
- A `ResourceNode` with `level: None` still has `None` afterwards.
- A pending upgrade does **not** consume a `max_deployed` slot — a deploy
  of that kind is still accepted.
- A raid destroying a half-supplied machine refunds the delivered units.
- Demolishing one does the same; the Home cascade takes every site with
  it.
- `cancel_build_request` on an upgrade site refunds and logs.
- The machine keeps producing while its upgrade stands.
- A save→load round trip through a half-delivered upgrade site, as a real
  save and load: a RON round trip cannot prove this on its own.
- A second upgrade request on the same structure is refused, with its own
  sentence.

Gates: `cargo test --workspace`, and
`cargo test -p feral-processes-engine balance_sim` — nothing here should
move a curve, so a moved one is the signal.

## Files

- `crates/engine/src/components.rs` — `BuildGoal`, `BuildSite::goal`
- `crates/engine/src/game/base/building.rs` — `upgrade_structure`,
  `count_build_requests`, `remove_structure`
- `crates/engine/src/game/base/construction.rs` — `raise_one_tick`
- `crates/engine/src/game/base/upkeep.rs` — `damage_structure`
- `crates/engine/src/game/inspection.rs` — `build_order_row`,
  `build_views`
- `crates/engine/src/views.rs` — `BuildOrderRow`
- `crates/engine/src/save.rs` — `BuildSiteSave::goal`, `#[serde(default)]`
- `crates/gui/src/render/building.rs` — the upgrade menu's cost display
- `crates/app-core/src/app/building.rs` — the refusal wording only

`SAVE_FORMAT_VERSION` is **not** bumped: the save change is one additive
field behind `#[serde(default)]`, which is exactly the case field-named
RON was adopted to make free.
