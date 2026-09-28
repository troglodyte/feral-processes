---
paths:
  - "**/base_ledger*"
  - "**/telemetry*"
  - "**/*output*"
---

# Load-bearing seams: Instrumentation

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **Every production seam reports through one door, `base_ledger::emit`, and
  the counter is a reader of the event rather than a sibling of it.**
- **The fold is unconditional and the record is built inside a closure, and
  nothing in the compiler holds that at a bevy seam.**
- **A source is recorded and never folded; a sink is both** — `Acquire`
  against `Consume`, and `grant_loot`'s eighteen callers are what make
  provenance one parameter.
- **A stall hangs on `set_machine_status`, which already speaks only on
  transition**, and `StallSite` is plain borrows because `power_grid_system`
  is exclusive.
- **`Record::BaseSnapshot` is the denominator**, once per
  `base_ledger::BUCKET_TICKS`, with its counts built inside the closure.
- **A haul is keyed to the post, not the worker, and a `Tend` writes
  nothing.**
- **The base output page is one derivation, `Game::base_output_report`, and
  its MINED/COMPILED split follows recorded provenance rather than the
  structure defs.**
- **`BASE_OUTPUT_MAX_ROWS` is a layout constraint** — the page has no scroll.
