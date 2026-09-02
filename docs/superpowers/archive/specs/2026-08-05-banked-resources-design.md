# Banked resources: research stops being a thing you carry

**Date:** 2026-08-05
**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header.

## The problem

Research Data is authored as an ordinary item and behaves like one in every
way that matters to the player: a Research Node drops it into its own
`Stock.output`, the player walks a tile adjacent and presses collect, and it
draws as a row on the inventory screen under `CUR`. None of that is true to
what it is. It is a score on a ladder, not cargo — you never carry it, never
choose to pick it up over something else, and never do anything with it but
spend it on one screen.

The plumbing was already half-aware of this. `ItemDef::bank_limit` exists and
its *presence* already marks Research Data as not-really-cargo, exempting it
from the cargo cap and from the zone payout curve. The flag was simply never
followed through to the two places the player actually notices.

It also currently leaks: the sell list is "inventory minus the trade
currency" (`app-core/src/app/trade.rs:96`), so Research Data is sellable and a
Research Node is a slow 1-Credit-per-14-ticks money printer.

## What `bank_limit` is doing today

Four jobs, all keyed on `.is_some()`, and only one reads the number:

| Site | Job | Reads the value? |
|---|---|---|
| `components.rs:272` (`cargo_used`) | exempt from the cargo cap | no |
| `systems.rs:187` (`resolve_gather_cycle`) | bypass the zone payout curve | no |
| `turn.rs:707` (`grant_loot`) | overflow log label | no |
| `add_capped` / `has_room` / `check_room` | enforce the cap | **yes** |

Research Data is the only banked item in the game, asserted by
`items_db.rs:396-399`.

## The design

`ItemDef::bank_limit: Option<u32>` becomes `ItemDef::banked: bool`. The cap is
deleted; the flag survives under an honest name and gains two consequences.
After this change `banked` means exactly four things:

1. Exempt from the cargo cap. *(existing)*
2. Does not scale with the zone payout curve. *(existing)*
3. Never enters a `Stock.output` — it is delivered straight to the player's
   bank. *(new)*
4. Is not an inventory row and is not a trade good. *(new)*

Research Data remains an `Inventory` entry keyed by `ItemId`, so
`unlock_research`'s `take()` is untouched and **no `SAVE_FORMAT_VERSION` bump
is needed**. It remains authored in `assets/items/research_data.ron`, so a mod
can still ship a research node and its currency as pure data — which is why
the rule hangs on a data flag rather than on `EconomyRole::ResearchCurrency`
or on a Rust `ResearchBank(u32)` resource. Both alternatives were considered
and rejected: the first puts the rule in engine code where a modded second
bank currency would not inherit it, and the second makes `work.produces`
unable to name it at all.

### The delivery seam

`resolve_gather_cycle` has two callers — the staffed path (`systems.rs:353`)
and the self-running path (`systems.rs:451`) — and each follows it with the
same three lines clamping the payout to `output_room()` and writing
`stock.output`. That duplicated tail is exactly where a banked branch would
drift apart, so the delivery moves into one helper both call:

- `banked` → add to the player's `Inventory`
- otherwise → `Stock.output`, clamped by `output_room()` as today

`resolve_gather_cycle` itself is unchanged; it already decides *what and how
much*, and this is about *where it lands*.

Delivery is **silent** — no log line per unit. A Research Node pays out every
14 ticks, so announcing each one would put a line in the base feed roughly
every 14 ticks forever, which is the fastest way to make the feed useless
(the same argument `set_machine_status` already makes about stalls). The
research screen is the readout, and it is one keypress away. This is what
"invisible" has to mean if it is to mean anything.

### Consequences that follow, and are intended

- **A banked item can never feed a neighbour machine.** It never reaches an
  `output`, and `output` is the only thing a puller reads. A bank is not a
  physical buffer, so nothing can pull from it. This makes `produced_item`'s
  claim to be "the whole answer to could this structure feed a neighbour"
  narrower and truer than it is today.
- **A Research Node can never clog and can never stall.** `output_room()` no
  longer applies to it, and with the cap gone there is no full-bank state
  either. There is deliberately no new `MachineStatus`.
- **It banks while the party is in the Stack.** Delivery touches `Inventory`
  and never `Position`, so the base keeps feeding research four frames down.
  Consistent with everything else the base does while you are underground.

### What the deleted cap takes with it

The cap is the only job that read the number, so removing it makes real
machinery unreachable. Unreachable code that always returns `Ok` is worse
than no code, so it goes:

- `Inventory::add_capped` and `Inventory::has_room`. With nothing capped,
  `add_capped` is `add`; its callers at `collect.rs:68` and `turn.rs:703`
  collapse to `add`.
- `Game::check_room` and its six `?` call sites in `game/trade.rs` (×4) and
  `game/crafting.rs` (×2).
- `grant_loot`'s entire `added < qty` overflow branch, including the
  `"Research bank"` / `"Buffer"` label at `turn.rs:707`.
- `Game::bank_limit_of` — `render/progression.rs:54` drops from
  `"Research Data: {held}/{bank_limit}"` to `"Research Data: {held}"`.

### Tests

Two die, and must be deleted rather than left asserting nothing:

- `add_capped_clamps_research_data_at_its_bank_limit` and its neighbours in
  `components.rs` that measure against a limit.
- `crates/engine/src/tests/trade.rs:146` — it guards a
  `check_room`-before-despawn ordering in `sell_companion` that ceases to
  exist once `check_room` does.

One survives but needs a **new argument**, not a reworded message:
`a_banked_resource_never_scales_with_zone_depth` (`tests/building.rs:1006`)
currently reasons "scaling it would fill the bank in ~13 cycles" — an
argument made entirely of the cap being deleted. The reason the behaviour
still holds is that the research tree is a fixed ladder whose deepest node
costs 45 (`cortex`), so a payout doubling per zone would collapse the tree
rather than accelerate it. The test's comment must say that instead.

New tests to write:

- A banked item produced by a staffed node lands in the player's inventory
  and leaves the node's `output` empty.
- The same for the self-running path, so the two delivery callers cannot
  diverge.
- A banked item does not appear in `PlayerStatus::inventory`.
- A banked item is absent from the sell list and `sell_item` refuses it.
- A banked item still does not scale with zone depth (the reworded existing
  test).
- A banked item banks while `Locale::Stack` is live.
- `items_db`'s "only Research Data is banked" census, rekeyed to `banked`.

### Documentation

`banked` is a schema field, so `assets/items/README.md` is part of this
change, not a follow-up — it currently documents `bank_limit`. CHANGELOG too.
Per standing carve-outs, `docs/manual.md` and the root `README.md` are not
updated.

## Blast radius

- **engine:** `items_db.rs`, `components.rs`, `systems.rs`, `game/catalog.rs`,
  `game/turn.rs`, `game/trade.rs`, `game/crafting.rs`, `game/collect.rs`
- **app-core:** `app/trade.rs` (sell list)
- **gui:** `render/progression.rs`, the inventory row source
- **assets:** `items/research_data.ron`, `items/README.md`

## Gates

- `cargo test --workspace`
- `cargo test -p feral-processes-engine balance_sim` — the payout curve is
  touched, so the curve tests are the regression gate
- `cargo clippy --workspace`, `cargo fmt`
- Play it. A green suite says nothing about whether a research economy with
  no collect trip and no visible counter still reads as a thing you are
  earning.
