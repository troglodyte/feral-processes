# One instrumentation layer, two consumers

**Status:** open, unbuilt. Nothing named here exists in `crates/` yet.
Verified against `main` at `0d0f2a83` (v0.13.82) on 2026-09-02.

**Date:** 2026-09-02
**Source of the problem:** [`docs/base-economy-audit.html`](../../base-economy-audit.html),
plus the correction record in **Provenance** below.

## The problem

The base economy audit is entirely theoretical. Every figure in it — feeder
ratios, duty cycles, "≈41,600 ticks for the research tree" — is derived from
`tuning.rs` constants and asset files. Nothing was measured, and
[`docs/measurements/`](../../measurements/) has no base-throughput entry at
all.

Two consequences. The player has no way to see what their base did for them,
and the developer has no way to check whether an audit finding is true. Both
want the same numbers.

## What this change does

Every extract and every craft emits one event. That event does two things:

1. **Always** folds into a bucketed counter that persists in the save, which
   the player screen reads.
2. **When `FERAL_DEV_LOG` is set**, appends to the existing JSON Lines
   telemetry file, which loads into sqlite for analysis.

One emission, two consumers, so the screen and the analysis cannot disagree
about what happened.

## Non-goals

- **No new telemetry framework.** `crates/engine/src/telemetry.rs` already
  exists and this extends it. See **The layer already exists** below.
- **No `serde_json` in the engine.** app-core is deliberately the only crate
  that names it; the engine derives `Serialize` and hands over values.
- **No balance change.** This measures; it moves no constant.
- **Not all seams at once.** Phase 1 alone answers the two questions that
  would change what gets built next. See **Phasing**.

---

## Decide these two before writing any code

Both are cheap now and expensive later, because retrofitting either makes
every run recorded before the decision unusable.

### 1. The zone stamp

**Every bucket carries the `ZoneLevel` it opened in.** One `u32`.

Counters roll across a breach — `enter_next_zone` does not touch the base,
which is the whole point of a base that travels — so without a stamp the
history is a blend of sectors and nothing can be attributed. The player
screen's "this sector / this run" split is built on it, and so is every
measurement question that asks whether a rate moves with depth.

Add it later and every earlier run's data answers no question worth asking.

### 2. The bucket count

**64 buckets of 1,000 ticks, plus a lifetime total that never rolls off.**

This is a save-size decision, not a display decision, and it is the one thing
here with a real cost. See **Bucketed counters** for the arithmetic. Halving
it later is easy; doubling it later invalidates the window every earlier save
recorded.

---

## The layer already exists

Do not build a second one. What is already in the tree:

| Piece | Where | What it does |
| --- | --- | --- |
| `telemetry::Record` | `crates/engine/src/telemetry.rs` | `#[serde(tag = "t")]` enum, five battle variants, no `World` and no IO |
| `resources::BattleTelemetry` | `crates/engine/src/resources.rs:845` | `on: bool`, `records: Vec<Record>`, fight counter |
| `Game::record` | `crates/engine/src/game/telemetry.rs:40` | **lazy closure**, builds nothing when disarmed |
| `Game::take_battle_telemetry` | same file, `:23` | drains the buffer |
| `append_records` | `crates/app-core/src/app/telemetry.rs` | JSON Lines, one object per line, creates the directory |
| `FERAL_DEV_LOG` | `crates/app-core/src/app/lifecycle.rs:126` | the gate |
| `App::flush_battle_telemetry` | `crates/app-core/src/app/lifecycle.rs:522` | every tick, **above** the arena guard |

Three properties of that layer are load-bearing and this change must not
break any of them:

- **`Game::record` takes a closure, and there is deliberately no eager
  variant.** Its doc explains why: an eager form allocates three `String`s
  per swing even when disabled, and `train` pays that 1.9M times a session.
- **`serde_json` stops at app-core.** That is what keeps
  `cargo check --workspace` at ~1.8s and the engine's dependency list at
  seven crates.
- **A failed write reports on the status line and the run carries on.** A dev
  log must never take a run down with it.

### The one architectural decision

**The counter must be a reader of the event, not a sibling of it.**

The tempting shortcut is to increment the ledger at the seam and separately
emit a record. That is two copies of one rule, and `CLAUDE.md`'s doc-comment
rule applies verbatim: the copy that drifts is the one nobody runs, and here
that would be the player's screen quietly disagreeing with the analysis the
tuning was done from.

So: **one `emit(Event)` per seam**, which folds into the ledger
unconditionally and appends to the log only when armed.

Note the cost this admits. The ledger fold is unconditional, so `emit` is
never fully free — the closure trick protects only the log half. It is a
`BTreeMap` increment per production *cycle*, not per tick, so it is
affordable; but it is a new unconditional cost and should be stated rather
than assumed away.

---

## The seams

### Production and consumption

| Seam | File | Why it is the right one |
| --- | --- | --- |
| `task_progress_system`, at the `resolve_gather_cycle` call | `systems.rs:~930` | **Both arms.** The `None` arm is the fizzle — the only empirical route to `mining_success_chance`, and the whole of B5. The `Some` arm has `structure`, `tier`, `zone`, `creature.species` and `clock.tick` in scope. |
| the `deliver_payout` return at that call site | `systems.rs:449` | `deliver_payout` itself is a free function with no clock and no entity, so it cannot build a record — emit at the *call site* and read its return. What it gives free is **`payout` vs `landed`**: the clamp against `output_room()` is the clog loss, and produced ≠ landed is a number the player screen wants. It also routes banked items (`ItemDef::banked`, i.e. Research Data) to the bank instead of the buffer, so both landing paths are covered by one read. |
| `assembler_system` completion branch | `systems.rs:~1370` | The single `*stock.output.entry(product).or_default() += 1` write, with the input drain immediately above it in the same scope — so consumption and production are one event. |
| `set_machine_status` | `systems.rs` | Already logs **only on transition** (a named seam in `CLAUDE.md`, three callers). Hang stall events here and the edges are free: `Starved`, `Clogged`, `Unstaffed`, `Stranded`, `Unpowered`, with no duplicate suppression to write. |
| `advance_hand_craft`'s completion (`job.completed += 1`) | `game/crafting.rs:440` | The hand-craft seam. **Check `Game::craft` (`crafting.rs:575`) too** — it still exists alongside the timed path, and whichever routes a given caller must be instrumented or the split is wrong. |
| `Game::grant_loot` | `game/turn.rs:1118` | **17 call sites**, one door: kill drops, base rock at 25%/swing, nest caches, Stack loot, contract rewards, caravan and Market buys, routine etching. Already returns `landed`. Phase 2. |
| `stock::spend_from_base` | `game/base/stock.rs:91` | Build and upgrade materials leaving shelves. Without it, output appears to vanish. Phase 2. |
| `power_grid_system`'s fuel spend | `systems.rs:~576` | **New since the audit.** `StructureDef::power_upkeep` + `components::PowerFuel` burn Power Cells to keep a supplier up. This is now a major standing consumer of a chain product and the ledger will not balance without it. Phase 2. |
| the breach wipe | `game/zone.rs:~448` | `let spendable = [self.currency(), self.craft_currency()]` destroys Core Fragments and Portal Fragments from the player's `Inventory`. Phase 2. |
| `Errand::Collect` / `Errand::Load` / `take_haul_load` / `deposit` | `game/base/hauling.rs:~730-800` | Not production — this is where throughput is *lost*, and the only place to observe the corrected B3. Phase 3. |

### What is missed if you only do the obvious two

Instrumenting `task_progress_system` and `assembler_system` alone covers
roughly 40% of the flow, and the missing 60% is where every interesting
question lives:

- **Hand-crafting** is the whole of B2. Without it the base looks productive
  while the player makes everything from their pack.
- **`grant_loot`** is B5's competing supplies. The Mining Node's actual share
  of Core Fragments is unknowable without it.
- **The fuel burn** is a consumption path that did not exist when the audit
  was written.
- **Hauling** is the corrected B3.

## Phasing

**Phase 1** — extractor payout and fizzle, assembler completion,
`set_machine_status`, hand-craft completion. Plus the ledger, the save field
and the player screen. This answers B2 and B4, and half of B5.

**Phase 2** — `grant_loot` source tagging, `spend_from_base`, the fuel burn,
the breach wipe. Closes the ledger so it balances.

**Phase 3** — hauling errands with distance. Answers B3.

Do not start Phase 2 before Phase 1's data has been read. Two of the
questions below may make Phase 2 unnecessary.

---

## The event log

Extend `telemetry::Record` with new variants. Same enum, same file, same
JSONL output, same `FERAL_DEV_LOG` gate, same per-tick flush. Every new
variant carries `tick` the way every existing one carries `fight`.

| Variant | Fields | Answers |
| --- | --- | --- |
| `Extract` | `tick, zone, machine, kind, tier, worker_species, item, rolled, landed, ok` | B4, B5. `ok: false` is the fizzle; `rolled` vs `landed` is the clog loss |
| `Assemble` | `tick, zone, machine, kind, item, inputs: Vec<(String, u32)>` | B2, B4 |
| `MachineStall` | `tick, machine, kind, status` | B3, B4 — transitions only |
| `HandCraft` | `tick, item, qty, careful, bench: Option<String>, ticks_spent` | B2 |
| `Acquire` | `tick, item, qty, source` | B5 — Phase 2 |
| `Haul` | `tick, worker, errand, item, qty, distance` | B3 — Phase 3 |
| `BaseSnapshot` | `tick, zone, staff, posted, machines, depots, supply, draw` | B7 — once per bucket |

`BaseSnapshot` is the one nothing else can substitute for. Without it every
rate is an absolute, and **B7 is a question about rate per posted program**.
It is also where ticks-per-sector comes from, which is the number that
calibrates every other figure in the audit.

### Cost

Roughly ten running machines at one record per cycle is about 0.8
records/tick; 50,000 ticks is ~40,000 records, ~6–10 MB of JSONL. Cheap to
leave on for a whole session, trivial for `sqlite3` via `json_each`.

**The cost discipline does not transfer to the bevy systems.**
`Game::record`'s closure needs `&Game`, and `assembler_system` /
`task_progress_system` / `set_machine_status` are systems with no `Game`.
Those sites need a `ResMut<BattleTelemetry>` param and a manual
`if !on { … }` guard before building the record, and **nothing in the
compiler keeps them honest** — unlike the existing seam, where the closure
makes it impossible to get wrong. Write a test asserting no record is built
when disarmed.

### Where the output goes

The log itself is gitignored and disposable. **The deliverable of a dev run
is a `docs/measurements/` entry**, written to that directory's four-section
convention: the claim, how to reproduce it, the numbers, what it does not
say.

---

## Bucketed counters

### Shape

```
BaseLedger {
    lifetime:     BTreeMap<ItemId, ItemTotals>,   // never rolls off
    buckets:      VecDeque<Bucket>,               // 64
    bucket_start: u64,
}

Bucket {
    start_tick: u64,
    zone:       u32,                              // the now-or-never stamp
    produced:   BTreeMap<ItemId, u32>,
    consumed:   BTreeMap<ItemId, u32>,
    busy_ticks: u32,                              // for "machines idle N%"
    idle_ticks: u32,
}
```

`BTreeMap` keyed by `ItemId`, never `HashMap`. This is the rule `Stock`
already follows and `CLAUDE.md` states: iteration order feeds the save
encoding, and a `HashMap` makes the file differ run to run.

### Granularity

**1,000 ticks per bucket, 64 buckets.**

Bucket on `GameClock::tick`, never on wall time. Rest does not advance the
clock, so the tick is a clean monotonic action counter.

The anchors that justify 1,000: a Mining Node cycle is 10 ticks, a
Fabricator or Armory cycle 30, `STRUCTURE_REGEN_INTERVAL` 20,
`SORTIE_TRAVEL_BASE_TICKS` 150, `BASE_ENTROPY_REFILL_TICKS` 300. At 1,000 a
bucket is ~100 Mining Node cycles and ~33 Fabricator cycles — never
mostly-zero, and 64 of them is 64,000 ticks of visible history.

### Save size — the honest number

64 buckets × ~15 tracked items × (produced + consumed) is ~7.7 KB of raw
counters. In pretty-printed RON that plausibly reaches **30–60 KB of text
against a real 190 KB save — a 20–30% growth**.

Not fatal: a full round trip on the real save is 1.46 ms in release. But it
is the kind of thing noticed after it ships. Shrink levers, in order of
preference: drop to 32 buckets; keep `consumed` lifetime-only and bucket
`produced` alone; prune items with zero lifetime total from the map
entirely.

---

## Save compatibility

**No `SAVE_FORMAT_VERSION` bump.** It stays at 32.

The reasoning, stated in full because the file this lives in is
booby-trapped:

`save_to_file` (`save.rs:1213`) writes `{SAVE_FORMAT_VERSION}\n{ron}`, and
`to_ron`'s doc says *"The on-disk form is field-named RON too, so this is a
pretty-printer rather than a decoder."* The save has been field-named RON
since 0.8.0. An additive `#[serde(default)]` field on `SaveData` therefore
loads as its default in an older file and costs no bump.

### The booby trap

`save.rs` field docs are littered with sentences like
*"`#[serde(default)]` does nothing for the bincode save — that is why it
required bumping `SAVE_FORMAT_VERSION`."*

**Those are historical.** They describe changes made against the positional
bincode format that pre-dates 0.8.0. Reading one in isolation and concluding
that this change needs a bump is the specific mistake to avoid.

The empirical confirmation is right here in the tree: `components::PowerFuel`,
`StructureDef::power_upkeep`, `StructureDef::zone_build_cost` and the whole
hand-craft rework all landed between v0.13.78 and v0.13.82, and
`SAVE_FORMAT_VERSION` is still 32.

### The test obligation

`a_save_survives_a_round_trip_through_ron_unchanged` **cannot catch a skipped
field** — a `#[serde(skip)]` leaves it green. A new save field needs its own
**save → load** assertion that the value comes back, not just a RON round
trip.

---

## What to measure first

**Measure ticks-per-sector before anything else.** Every tick-denominated
figure in the audit is unanchored — nobody knows how long a sector takes in
practice. One `BaseSnapshot` field calibrates the whole analysis, and without
it the rest are ratios with no scale.

Then, in the order that most changes a decision:

### Did the hand-craft time cost actually work? (B2)

Of every Blank Substrate, Bytecode Block, Charge Coil and Logic Wafer that
existed this run, what fraction came from an assembler versus from
`advance_hand_craft`?

Needs `Assemble` + `HandCraft`. This is now a **before/after on a shipped
change** rather than an open question — the hand-craft time cost landed in
v0.13.79–82 — which makes it a stronger measurement than it would have been
a week ago. If hand-crafting still dominates, the time cost was priced too
low.

### Is the Mining Node the worst source of Core Fragments? (B5)

Over one sector, what share came from nodes versus kills versus base rock
versus caches versus contracts?

Needs `Acquire` with source + `Extract`. Prediction from the constants: the
node loses badly. If it does not, the audit is wrong.

### Do assemblers clog as sectors deepen? (B4)

The observable signature is **not** the production rate. It is
`MachineStall` transitions to `Clogged` per 1,000 ticks rising with zone
while extractor output climbs. That is a far cheaper and cleaner test than
plotting rates against each other.

Needs `MachineStall` + `Extract`.

### What does Depot distance cost? (B3, corrected)

What fraction of each machine's ticks are `Starved`, split by adjacent-fed
versus Depot-fed, plotted against Depot Chebyshev distance?

Needs `Haul` + `MachineStall`. The corrected model predicts: adjacent
machines rarely starve; hub-fed assemblers starve in proportion to `2d + 2`
against their cycle length; and **unattached extractors show very low
at-station time**, because they ship one unit per round trip and
`task_progress_system` gates on `at_station`.

### Is roster size the throughput dial? (B7)

Units produced per *posted program* per 1,000 ticks, and how it moves when a
Data Cache goes up.

Needs `BaseSnapshot`. This decides whether capping the Data Cache is safe
alongside the portal bill that shipped.

### Two free ones

- Empirical `mining_success_chance` against the `0.4 + 0.1 × tier` formula.
  The fizzle records give it directly.
- How often a base actually runs `Unpowered`. B6 said never — but the fuel
  burn shipped after that was written, so it is now genuinely open.

---

## The player screen

### The constraint

The player has **infinite cargo** — `Inventory` has no capacity field and
`grant_loot` unconditionally returns the full quantity — and never interacts
with a Depot. So **never surface a quantity that lives in a buffer**: no
Depot fill, no machine buffer contents, no "units in store", no carry
capacity. All of it is invisible plumbing.

Show flows and totals, framed as what the base did *for you*.

### Sketch

```
BASE OUTPUT                        sector 3 · this run

  MINED                    sector      run
  Core Fragment               412    1,847   ▁▂▄▆▇▆▅
  Raw Trace                   128      501   ▁▁▃▄▄▃▃
  Cache Grain                  44       44   ▁▂▃▄

  COMPILED              machine  hand      run
  Bytecode Block             38    12      156
  Charge Coil                21     0       77
  Hardened Shell              9     3       31

  ──────────────────────────────────────────────
  cycles worked 1,284      machines idle 38%
  needs attention: Lathe — starved
```

### Decisions

- **Two time columns** is what the zone stamp buys, and it is the framing
  that makes the base read as a thing that travels with you.
- **"Mined" versus "Compiled"** maps exactly onto `work` versus `assembles` —
  the same split `StructureCategory` already makes — so the screen cannot
  disagree with the build menu.
- **Split machine versus hand.** A combined figure actively hides B2 from the
  player. One extra column, and it teaches them the machines are doing
  something.
- **"needs attention" must call `Game::attention`, not restate it.**
  `CLAUDE.md` names that as one derivation with three surfaces reading it; a
  fourth that computes its own answer is exactly the drift the seam exists to
  prevent.
- **Name the stalled machine.** "Lathe — starved" is actionable; a percentage
  is not.
- **No Credits.** That is trade, not production. Mixing them makes this a
  second economy panel.

### Three real problems

- **Row budget, not cell budget.** Sparklines are cheap in cells (8–12) but
  the binding constraint is rows: `MAX_BAND_ROWS` already went 4 → 3 to pay
  for the need rows, and the map's status column holds 38.5 monospace cells.
  Budget rows before designing them. If this is a full screen rather than a
  pane it inherits the no-scroll rule and needs a height census, the way the
  notification screen does.
- **A fresh save shows an empty screen**, which reads as broken. Show
  lifetime totals from tick one and let the sparklines fill in.
- **The Stack.** The base keeps running while the player is underground, so
  "this sector" includes time they were not there. That is correct, and it is
  the best thing the screen can teach — but it needs a word on screen or it
  reads as a counting bug.

---

## Harder than it looks

1. **`grant_loot`'s source parameter is a 17-site diff**, and each site needs
   a judgement call about which source it is. Mechanical, but a wrong answer
   is silent.
2. **No compiler help on the bevy-system seams.** The closure discipline that
   makes existing telemetry free does not reach `assembler_system`,
   `task_progress_system` or `set_machine_status`.
3. **Save growth of 20–30%** on a real save. Decide the bucket count
   deliberately, not by default.
4. **The zone stamp is now-or-never.** Retrofit it and every run recorded
   before it is unattributable.
5. **`BaseLedger` is a new `Resource`, and registering one shifts bevy's
   query iteration order.** Expect a failure in an untouched subsystem and
   recognise it as a latent unsorted-query test rather than chasing it as
   this change's regression.
6. **Scope creep is the real risk.** Phase 1 alone answers the two questions
   that would change what gets built next.

---

## Provenance, and one correction

This spec descends from
[`docs/base-economy-audit.html`](../../base-economy-audit.html), published as
artifact `15ef9fde` and read from `main` at `2586067a` (v0.13.78).

**That artifact is stale in two ways and must not be re-derived from.**

### B3 was wrong

The audit claimed that an assembler is fed only by orthogonally adjacent
neighbours, that four is therefore a physical ceiling, and that no
first-stage assembler can be fed at Mk1. **That is wrong.**

There is a second feed path the audit missed. `hauling.rs` defines
`Errand::Collect` and `Errand::Load`: when `missing_ingredient` finds an item
a machine cannot make a batch of from its own input plus its orthogonal
neighbours, the machine's own posted worker walks to the nearest **Depot**
holding it, takes up to `HAUL_CARRY_CAPACITY` (5), walks back, and writes into
the machine's `input` at `hauling.rs:767` — the one write to a machine's
input outside `assembler_system`.

So machines do **not** need to be adjacent. A Depot is a hub, and any layout
works. The audit's numbers describe a *depot-less* base accurately and were
wrongly generalised to all bases.

What is true instead, and what the corrected B3 measurement above tests:
adjacency is a large **throughput multiplier**, not a requirement. An
attached producer hoards and its neighbour pulls instantly and free; an
unattached producer ships one unit per round trip and, because
`task_progress_system` gates on `at_station`, **produces nothing while its
worker walks**. `assembler_system` has no such gate, so a consumer keeps
working off its hopper while its worker is away. The penalty falls on the
extractor, not the assembler.

Specifically wrong in the published artifact: finding B3 in full, the
"STARVED — adjacency allows 4" annotation on Figure 2, and the "Duty on 4"
column in the feeder table.

Findings B1, B2, B4, B6, B8–B13 survive untouched. B5 survives and gets
*stronger*: hub-fed nodes lose production time to walking, so the Mining Node
is a worse source than the audit said. B7 is promoted — every machine needs a
posted program and workers now also burn ticks walking, so roster size is the
throughput dial.

### The game moved

The audit read v0.13.78. Between then and v0.13.82,
[`2026-09-02-base-as-the-price-of-progress-design`](../archive/specs/2026-09-02-base-as-the-price-of-progress-design.md)
shipped and closed three of the audit's findings directly: hand-crafting now
costs real time (`begin_hand_craft` / `advance_hand_craft`), the Zone Portal
bill demands terminal products from every chain (with a new
`StructureDef::zone_build_cost` for per-sector tiering), and the power grid
burns Power Cells (`power_upkeep` / `PowerFuel`).

That is why the B2 question above is framed as a before/after rather than as
an open question, and why the fuel burn appears in the seam table at all.

**The artifact has not been updated.** Rewriting B3 and Figure 2 is a
separate job; until it is done, read this section first.

---

## Corrections from building Phase 1 (2026-09-03)

Both were found by tests while implementing, on branch
`feat/base-instrumentation`. Recorded here because Phase 2 and the screen
would otherwise be built from the wrong sentence above.

### The seam table is one seam short

**`player_gather_system` is an extract seam and is not listed.** `Game::
work_structure` puts the player on the same `Task` a posted worker carries,
running the same `resolve_gather_cycle` and the same `deliver_payout`, so a
run where the player cranks their own nodes would record nothing at all and
show an empty screen. Instrumenting `task_progress_system` alone covers the
posted worker only.

It is instrumented on both arms like the other, with `worker_species: None`
— the player has no `Creature`, and it is the one extract with nobody's
aptitude behind it.

### "Mined versus Compiled maps exactly onto `work` versus `assembles`" is wrong

**The Player screen section says the split keys on the structure defs. It
cannot.** A Power Cell has a structure whose `work` produces it *and* is
hand-compilable, so a def lookup files every unit the player pressed out by
hand as a machine's work — which is exactly the figure the page exists to
expose, and the whole of B2.

`Game::base_output_report` sections on **recorded provenance** instead:
`ItemTotals` already carries `mined`, `compiled` and `hand` separately, so
the ledger knows how each unit was actually made. One row per item on
dominant provenance, with the `machine`/`hand` columns carrying the
breakdown — two rows for one item would split the totals and leave neither
answering "how much have I made".

The claim that this keeps the page agreeing with the build menu does not
survive either: the build menu describes what a machine is *for*, and this
page describes what happened.

### Phase 1 is complete, and `BaseSnapshot` is in no phase (2026-09-03)

Everything Phase 1 names is built and green on `feat/base-instrumentation`:
the four production seams, `set_machine_status`, the ledger, the save field
and the player screen (`Mode::BaseOutput`, off the base menu's "Base output"
row).

**But the first thing this spec says to measure cannot be measured yet.**
"Measure ticks-per-sector before anything else" reads off `BaseSnapshot`,
and `BaseSnapshot` appears in the record table and nowhere in the phasing —
so it is in neither Phase 1, 2 nor 3. It has to be built before a dev run is
worth doing, or every rate the log produces is an absolute with no scale.

### The screen's columns and its row budget

Two things the sketch got wrong, both found by building it.

**Both sections carry all four figures**, not the `sector | run` pair the
sketch gave MINED alone. An item sits in a section on *dominant* provenance,
so a Power Cell whose machines outproduce the player lands under MINED — and
the sketch's columns would have dropped the hand count on exactly the item
that motivates B2.

**`BASE_OUTPUT_MAX_ROWS` is 5, not the 8 first written.** The page has no
scroll, so its height is a layout constraint: a `PopupSize::Large` holds 21
rows at 600px once a refusal's two lines are reserved, and the page's own
chrome — the sector line, the column header, two section headings, a blank,
the four rows `Game::attention` can hold at once and a footer — spends ten of
them. Ten item rows is what is left. The gate is
`the_tallest_base_output_page_fits_its_popup`.

### `set_machine_status` could not take a `ResMut` parameter

The spec says the bevy-system seams "need a `ResMut<BattleTelemetry>` param".
True of three of the four callers and impossible for the fourth:
`power_grid_system` is **exclusive** — it holds `&mut World` — and can take
no resource parameter at all. The shared shape is `systems::StallSite`, a
bundle of plain borrows (telemetry, tick, tile, def id) that all four callers
can produce, passed to the one door. The exclusive caller reaches it through
`world.resource_scope`, since the log and the telemetry buffer are two
resources and it may hold only one mutable world borrow at a time.

## Corrections from building Phase 2 (2026-09-03)

### `spend_from_base` is not where build materials leave

The seam table says `stock::spend_from_base` covers "build and upgrade
materials leaving shelves". **It does not, and B3's mistake is the shape of
this one too.** That function has one shipped caller — the dig crew's tile —
plus a sortie's outfitting.

Build materials take a different route entirely: `pick_up_for_site` takes
them off a shelf into a worker's `Carrying`, `set_load_down` delivers them
into `BuildSite::delivered`, and they stand on the cell, **refundable**,
until the structure is raised. So the consumption point is the despawn of the
site, not the take from the shelf — which is `CLAUDE.md`'s "materials are not
spent until the structure is raised" restated. It is `consume_site` now, one
function called from both `BuildGoal` arms, because two of the arms' paths
return early with the materials still owed back.

### Acquire is recorded and never folded

The spec says Phase 2 "closes the ledger so it balances". Half of it does:
the four **sinks** fold, because a sink is the other half of the ledger's own
arithmetic. `Acquire` does not, and must not — the ledger feeds the player's
page, whose whole MINED/COMPILED split is a claim about what the *base* made,
and a kill's Core Fragments folded there would read as a machine's work. B5
is an analysis question and lives in the log alone.

### One door from a `Game`, because `emit` wants two resources

`base_ledger::emit` takes the ledger and the telemetry buffer at once and a
`&mut World` lends one at a time, so every `Game`-side reporting site was
writing the same eight-line `resource_scope` dance — five of them by the end
of Phase 2. `Game::report_base` is that dance, written once;
`note_hand_craft` was the first copy and is its first caller.

### Two sinks Phase 2 does not cover

Neither is in the spec's list and both are real: a **hand-compile's
ingredients** (`Event::HandCraft` folds the product and nothing else), and a
**craft or install that spends a disk**. The ledger's consumed side is
therefore still short by whatever the player makes with their own hands,
which is the same figure B2 is about.
