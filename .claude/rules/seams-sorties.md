---
paths:
  - "**/sortie*"
  - "**/route*"
  - "**/caravan*"
  - "**/dispatch*"
  - "**/outpost*"
---

# Load-bearing seams: Sorties

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **`ProgramRole` has a fourth variant, and a sortie's five consequences are
  omissions rather than checks.** `Sortie` sits **between `InParty` and
  `Staff`**, keeping `Staff` as what is left over.
- **`ProgramRole::Outpost` sits between `Sortie` and `UnderStudy`, and its
  crew's consequences are omissions in the same doors `Sortie`'s already
  are.**
- **A sortie battle is spawn, fight and despawn inside a single call, and
  that is the whole feature's load-bearing decision.**
- **The fights are real by construction, and the trained policy is
  deliberately not used.** `resolve_and_apply_attack` and `use_ability` are
  both `BattleState`-free, so the ladder, the bands, mitigation, affinity,
  Power and cooldowns are the ones a fight in front of the player uses.
- **A Relay is identified by `StructureDef::dispatches_sorties`, not by its
  id.** The research gate really is pure data, but `has_relay` is not —
  naming `"relay"` in Rust puts content in the engine and makes a mod's
  second dispatch structure impossible.
- **The board is derived and the whole record travels.**
- **`sortie_duration` reads the risk *offset* and has no term for the
  squad.** Against the absolute band every trip in a deep sector grows for
  no reason the player could name; with a strength term the feature becomes
  a throughput multiplier that scales with itself.
- **Membership rides `CreatureSave::sortie_index`; `SortieSave` carries no
  member list.** `party_slot`'s precedent — entity ids are not stable across
  a save.
- **Every dispatch refusal lands before anything is spent**,
  `commit_caravan_basket`'s rule, asserted **per refusal** — a single test
  over one of nine passes against eight paths that never spend anyway.
- **A squad's departure is a cue the engine queues and forgets, and it is
  base space's.** `dispatch_sortie` and `return_sortie` each queue one
  `resources::TransitCue` per body — a glyph and the cells it walks —
  drained by `Game::take_transits`, `take_effects`' counterpart.
- **`award_companion_xp` is an extraction, not a copy** — the growth roll,
  the cap, the XP buff, the tally and the routine unlocks, in one place a
  sortie's off-screen fight calls rather than restates.
- **A caravan route is one record with a `standing` flag, and a one-off is
  the flag turned off** — `routes::Route`, `WorkOrder`'s shape.
- **`Game::sever_route` clears `standing` and nothing else**, so the trip in
  flight arrives, sells and pays, and there is no refund path.
- **A route and a squad leave through the same door**, `Game::dispatch_reach`
  → `DispatchReach`, both gated on `StructureDef::dispatches_sorties`.
- **`Game::route_quote` is the one derivation the cargo picker's preview and
  the sale at the far end share**, reached from the screen through
  `route_manifest_quote`.
- **Route predation is a named query plus pure geometry** —
  `Standing::preys_on_routes` and `routes::settlements_near_route`, which
  measures to the **segment** anchor→destination and not to either end.
- **Predation is the only thing in `Game::run_routes` that may draw
  `GameRng`**, and a test asserts the tick draws nothing when nothing preys.
- **`routes::RouteEnd` is the caravan route's endpoint extension point, and
  `Route` itself is free to change even though `RouteSave` cannot.**
- **An outpost's standing route reloads through a fresh, empty outbound
  leg — never an instant same-tick pickup.**
