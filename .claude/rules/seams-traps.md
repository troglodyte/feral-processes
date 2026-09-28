---
paths:
  - "**/*trap*"
---

# Load-bearing seams: Traps

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **A trap's quality is authored on the item def, never rolled per copy.**
- **A trap clamps the rarity it rolled before it rolls a condition**,
  `downed_program_for`'s boss-floor rule with a second caller.
- **A trap spends no `GameRng` until its period elapses, and traps roll in
  `(x, y)` order.**
- **A trap resolves its item def live by id, against `ActiveContract`'s
  precedent** — it is a device standing in the world, like a nest.
