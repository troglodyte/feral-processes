# Traps

- **A trap's quality is authored on the item def, never rolled per copy.**
  An instanced consumable cannot stack in `Inventory` (whose `count`/`take`
  read the *first* matching row), cannot sit in a `Stock` output buffer
  (keyed by a bare `ItemId`), and cannot be the subject of a work order — so
  a per-copy quality roll would need a fourth player store on
  `DownedPrograms`' precedent to buy nothing the player can see. A better
  trap is a second `.ron` file behind a later research node, which costs no
  Rust at all. `TrapDef` is a struct rather than a bare `Option<Rarity>` for
  exactly that: a second tier's own period or capture chance lands in it.
- **A trap clamps the rarity it rolled before it rolls a condition.**
  `downed_program_for`'s boss-rarity-floor rule, second caller:
  `DownedProgram::roll_condition` prices condition against the rarity the
  program actually ships with, and `DownedProgram::grade()` folds both — so a
  condition rolled against the *pre*-clamp rarity leaves the grade overstated
  for exactly the catches the ceiling exists to hold down. The order is
  chance → species → rarity → **clamp** → condition, and `roll_condition` is
  a pure integer formula that spends no draw despite the name.
- **A trap spends no `GameRng` until its period elapses, and traps roll in
  `(x, y)` order.** The countdown is not pacing, it is isolation: a per-tick
  roll would put fifty traps' worth of draws into the shared stream and move
  every seeded test in the engine, which reads as an unrelated intermittent
  failure elsewhere. The sort is `assembler_system`'s rule — bevy's query
  iteration order is not stable, so two traps elapsing on the same tick would
  consume the stream in an order that varies between runs. `run_traps`
  deliberately does **not** call `Game::field_escalation`: those terms scale a
  spawned body's `Stats` and a `DownedProgram` has none, so the term would be
  a number with nothing to apply to and would misreport that function's
  caller census.
- **A trap resolves its item def live by id, against `ActiveContract`'s
  precedent.** `ActiveContract` stores the whole resolved def because an
  agreement already signed must not be rewritten under the player;
  `restore_nests` resolves by id because a nest is a thing standing in the
  world. A trap is the second kind, so a retuned `.ron` retunes one already
  on the ground and a deleted one drops it silently on load. `TrapSave` is a
  **named** struct for the same family of reason: field-named RON protects an
  added field, and a positional tuple gains a legacy slot the next time one is
  added.
