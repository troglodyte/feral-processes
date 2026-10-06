---
paths:
  - "crates/engine/src/implants.rs"
  - "crates/engine/src/game/implants.rs"
  - "assets/implants/**"
---

# Load-bearing seams: Implants

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **Implant effects are read only at the sites in `game/implants.rs`'s callers (stats, Power drain, routine slots, capture odds, Trace, drops, XP, battle start, Dead Man's Switch), always through `ImplantDb`, and a def that is missing contributes nothing.**
- **A player with no implants draws nothing from `GameRng`.**
- **`DeadMansSwitch` hooks `apply_damage`, not `lower_hp`, so `kill_outright` still kills.**
