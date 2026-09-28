---
paths:
  - "**/save*"
  - "**/resources*"
  - "**/contracts*"
  - "**/help*"
  - "crates/app-core/**"
  - "crates/launcher/**"
  - "crates/gui/src/render/**"
---

# Load-bearing seams: Saves, logs and screens

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **The save is field-named RON, and that is what retired save migrations.**
  An additive change behind `#[serde(default)]` costs **no version bump**.
- **A run that ends is written into its save, and `Game::load` is what
  refuses it.** `SaveData::game_over`, written unconditionally by
  `Game::save`, sealed to disk by app-core's `App::seal_run`.
- **A log line carries two independent axes, `MessageKind` and
  `MessageSource`.** Kind has three consumers that each mean something
  different by it, which is why "this came from the base" is a second field.
- **A swing's outcome is a third axis on `LogLine`,
  `resources::SwingOutcome`, and it is keyed to the *raw* line the reveal
  releases, never to a condensed row.**
- **A refusal is one sentence on two surfaces, and `App::refuse` is the one
  door** — `App::status_line` for the popup the player typed into, and
  `Game::note_refusal` for the log they scroll back through.
- **`MessageSource` has two readers, and the battle pane's is not the
  filter.** `battle_rows` drops base news unconditionally, because
  `since_round` slices by *position*.
- **The reveal is gated on `Mode::is_battle`, and that gate keeps it off the
  map.** `MessageLog::round_start` is deliberately never closed.
- **The map log pane is filtered; the history screen (`L`) is not.**
  `pane_rows`' stage order is load-bearing: unrevealed tail, then filter,
  then fold, then capacity.
- **All three log surfaces fold repeats, and `resources::condense` is the
  one fold.** It sits in `pane_rows` and `battle_rows` — on the rows about
  to be drawn, *after* the truncation — because `revealed`,
  `hidden_log_lines` and `battle_view_at` all count **raw** lines.
- **The map log pane wraps, and the cut comes off the *oldest* end** —
  `pane_rows` hands its rows over oldest first and `draw_playing_base`
  counts the capacity it asks for in *entries*, so a `break` at the floor
  drops the newest news the moment one entry is several rows.
- **`retain_outcomes_since_battle` keeps only `Outcome`, `Loot`, `LevelUp`,
  `Raid` and `Complete`.** A plain `log()` is `Info` and is pruned.
- **The map's status column cannot grow, and `draw_row` clips vertically
  only.** It holds 38.5 monospace cells; the widest shipped buff row already
  spends all but 3.8 of them, so a companion's `(holder)` tag drew 360px off
  the panel in silence.
- **A read-only screen's row count is owned by app-core and drawn by gui, so
  any per-row transform must live in the engine.** Both sides call
  `Game::message_history`; folding in the renderer opens the screen on a row
  that isn't drawn.
- **A group menu's rows are hidden dynamically**, so `base_menu_rows` /
  `party_menu_rows` must be the *only* source of them.
- **A Broker's board is derived, never stored** — seeded off `(world seed,
  zone, epoch)`.
- **Reading a Broker's board and signing it are two questions, and
  `Game::broker_reach` is the one call that answers both.** `NoBroker` /
  `OffBase` / `AtBroker`; `board_defs` refuses on `NoBroker` alone, the two
  verbs require `AtBroker`, and the base menu's row test and the screen's
  header read the same value.
- **A `Deed` is the extension point for a new job kind; an `Objective`
  variant is for a parameterised one.**
- **How and where a contract is satisfied is its own derivation** —
  `Game::objective_hint`, because `objective_line` has fourteen characters
  of headroom on the widest shipped row.
- **A `starter` contract jumps the board queue, and only in sector 1.**
  `board_defs` fills its three slots from unfinished starters first — three
  uniform draws out of fourteen made a new run's first job a coin flip, and
  `min_zone: 0` says a contract *may* be offered, not that it is offered
  first.
- **You *run* (or *invoke*) a routine; the noun is an *invocation*.** "Cast"
  and "spell" are the fantasy words this setting does not use, and unlike
  Raid the rename went all the way through the identifiers —
  `Game::run_field_routine`, `Mode::FieldRoutine*`, `FieldRoutineTarget`,
  `FieldBuffKind::scales_with_invoker`.
- **"Raid" is the code's word and "GC Entropy Sweep" is the player's.** The
  `.ron` fields are mod schema and deliberately kept their names.
- **A fight's vocabulary is security, not swordplay** — an attempt lands,
  lands unchecked, fumbles or is refused, Bleed is a leak and Stun a stall,
  and the unit is Integrity rather than damage.
- **`world.get::<Stats>(e).is_none()` is the idiom for "this entity is
  gone"** — don't reach for `World::get_entity`.
- **There is one place a runtime path is decided,
  `crates/launcher/src/paths.rs`**, and `main` reads nothing else: the loose
  asset tree, the player-data directory, and whether this build has a repo
  behind it.
- **A path that spends ticks owes `after_tick()`**, and there are three:
  `handle_key`'s tail, `update_realtime` and `App::advance_compile`.
- **A screen that spends the engine's ticks paces them against `dt`, never
  one per rendered frame** — `COMPILE_TICKS_PER_SECOND`.
- **And it spends them at the world's own rate**, `COMPILE_TICKS_PER_SECOND`
  derived from `WORLD_SPEED_MULTIPLIER` rather than restated.
- **Engine test fixtures live in `crates/engine/src/tests/support.rs`.**
  Look there before writing a new one.
