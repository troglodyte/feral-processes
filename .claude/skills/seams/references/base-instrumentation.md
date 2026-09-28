# Seams: The base: instrumentation

- **Every production seam reports through one door, `base_ledger::emit`, and
  the counter is a *reader* of the event rather than a sibling of it.** The
  shortcut is to increment the ledger at the seam and build a log record
  beside it — two copies of one rule, and the copy that drifts is the
  player's screen quietly contradicting the analysis a retune was made from.
  The fold is unconditional (a `BTreeMap` increment per production *cycle*,
  which every player pays); the record is built inside a closure a disarmed
  run never calls. **The trap is that nothing in the compiler holds that at a
  bevy seam**: `Game::record`'s closure needs a `&Game` and
  `task_progress_system`, `assembler_system` and `set_machine_status` have
  none, so they take the resources directly and an eager record would compile
  clean. `a_stall_builds_nothing_when_the_log_is_disarmed` is that property
  asserted rather than constructed. `Game::report_base` is the `Game`-side
  door, because `emit` wants both resources at once and a `&mut World` lends
  one at a time — five sites were otherwise writing the same `resource_scope`
  dance.

- **A source is recorded and never folded; a sink is both.** `Consume` folds
  because a sink is the other half of the ledger's own arithmetic — without
  it the produced side is a stream with no drain and an assembler's inputs
  look like the only thing a base ever spends. `Acquire` must **not**: the
  ledger feeds `Mode::BaseOutput`, whose MINED/COMPILED split is a claim
  about what the *base* made, and a kill's Core Fragments folded there read
  on the page as a machine's work. `grant_loot`'s eighteen callers are what
  make provenance one parameter rather than eighteen seams, and `LootSource`
  / `ConsumeSource` are enums with one `as_str` each for
  `MachineStatus::as_str`'s reason — a mistyped tag at one of eighteen sites
  invents a source no analysis would notice was wrong.

- **A stall hangs on `set_machine_status`, which already speaks only on
  transition** — so the edges are free, there is no duplicate suppression to
  write, and no site can report a stall the base log never announced. What B4
  needs is transitions to `Clogged` per 1,000 ticks rising with zone, which
  is an edge count and not a state dump. `StallSite` is a bundle of plain
  borrows and **deliberately not a `SystemParam`**: `power_grid_system` is
  exclusive, holds `&mut World`, and can take no resource parameter at all.
  It carries the def **id**, never the display name, which is prose a mod may
  rewrite.

- **`BaseSnapshot` is the denominator and its counts are built inside the
  closure.** Every other base record is a count; without this one they are
  absolutes, and B7 asks for rate per *posted program*. It is also the only
  source of ticks-per-sector. The walk is a pass over every entity, so
  building it above the closure would charge a disarmed run a full base
  census every thousand ticks — which forces `iter_entities` over
  `World::query`, the latter wanting `&mut World` that the closure's `&Game`
  cannot give. Stamped with the tick that **opens** the window, taken before
  the clock advances; machines and depots key on `runs_a_job` and `stores`
  and never on an id.

- **A haul is keyed to the post, and a `Tend` writes nothing.** The audit's
  claim that a machine can only be fed by its four neighbours is wrong —
  `Errand::Collect` walks to a Depot — so adjacency is a throughput
  multiplier and the cost falls on the **extractor**, which produces nothing
  while its worker walks (`task_progress_system` gates on `at_station`) while
  a consumer keeps working off its hopper. The record carries the Chebyshev
  distance because that is what a stall fraction is plotted against. Keyed to
  the machine because by the time an errand acts the worker is standing on
  the destination; a `Tend` is dropped because a busy base would otherwise
  log a row per tick for re-deciding the same errand, and `qty` is what
  *moved*, not what was wanted.

- **The base output page is one derivation and its row count is a layout
  constraint.** `Game::base_output_report` is all `Mode::BaseOutput` reads,
  and `Game::attention` is reached through it rather than recomputed. **The
  trap is the MINED/COMPILED split**: keyed off the structure defs it reads
  the same at a glance and is wrong for the one case that matters — a Power
  Cell has a `work` structure that produces it *and* is hand-compilable, so a
  def lookup files every unit the player pressed out by hand as a machine's
  work, which is the whole of B2. It follows recorded provenance instead, one
  row per item on the dominant one, with the machine/hand columns carrying
  the breakdown. The page has no scroll, so `BASE_OUTPUT_MAX_ROWS` is bounded
  by `the_tallest_base_output_page_fits_its_popup`: 21 rows at 600px less ten
  of chrome. Both sections carry all four figures — dropping the hand column
  under MINED hides exactly the units the instrument exists to count.
