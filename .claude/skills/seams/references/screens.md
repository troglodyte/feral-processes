# Saves, logs and screens

- **The save is field-named RON, and that is what retired save migrations.**
  An additive change behind `#[serde(default)]` costs **no version bump**. A
  field *removed*, or one whose meaning changes under a name it keeps, still
  does. The version is the file's **first line**, so an unreadable file is
  refused *by version*. **A positional tuple is the one shape this does not
  save you from** — prefer a named struct, or the next property costs a legacy
  field.
- **A run that ends is written into its save, and `Game::load` is what
  refuses it.** `resources::GameOver` is a resource and was persisted
  nowhere, so a Permadeath flatline left the slot holding the last autosave
  — at most `AUTOSAVE_INTERVAL_TICKS` (50) behind the death and with no
  record of it — and reloading gave back a clean run. `SaveData::game_over`
  is the *reason*, not a flag: `history_summary` already proves that string
  is the sentence, and the load list spends it as `FLATLINED`. Additive
  behind `#[serde(default)]`, so no bump. Four placements are load-bearing.
  `Game::save` writes it **unconditionally** — `None` for a live run, so an
  autosave is as truthful as the seal and there is no branch to forget.
  **Nothing branches on `DifficultyMode`**: `death_handling_system`'s
  Permadeath arm is the only writer of a reason, so the check is spent, and
  a Forgiving reboot leaves its save loadable with no rule of its own. The
  refusal is in **`Game::load`, never `load_from_file`** — `list_saves`
  reads a dead save to label it and `savetool dump` must open one, so the
  refusal is the game's and not the format's, and a developer can still
  clear the field and pack a run back to life. And the **seal is
  app-core's**, since `current_save_path` is the `App`'s: it rides
  `check_game_over`'s `history_written` latch (same event, recorded once)
  and sits **below** its `in_arena()` return, or a lost arena fight against
  a Permadeath save would end the run that was never fought. A failed write
  is surfaced in `flush_profile_writes`' wording — it is the one failure
  that hands the run back. **Quitting is deliberately not closed**:
  `Mode::QuitRunConfirm` still offers "quit without saving", so a rewind of
  up to 50 ticks survives; save-on-exit and delete-on-load were weighed and
  left.
- **A log line carries two independent axes, `MessageKind` and
  `MessageSource`.** Kind has three consumers that each mean something
  different by it, which is why "this came from the base" is a second field.
  Power reserves are field on purpose. The tagging table lives in
  `MessageSource`'s doc comment.
- **A swing's outcome is a third axis on `LogLine`, `resources::SwingOutcome`,
  and it is keyed to the *raw* line the reveal releases, never to a condensed
  row.** `App::advance_reveal` fires a per-swing sound cue off
  `Game::battle_log()[revealed].outcome` as `revealed` advances — the same
  raw index `battle_rows` truncates by and `condense` folds only *after*, so
  a wipe that condenses three kills into one display row still fires three
  cues. Six call sites set it, all through the one door, `Game::log_swing`.
  **Three cues, not four**: `SwingOutcome::Fumble` plays the same clip as
  `Miss` (`app::input::swing_sound`), since the log line already carries the
  severity a fourth sound would only duplicate. A key pressed mid-reveal is
  heard as **one** cue for the loudest band it dumped (`App::skip_reveal`,
  ranking through `cue_rank`) — not one per blow, which is six clips in a
  frame, and not silence, which is what shipped first and made the feature
  inaudible: the player presses the action key again well inside the ~0.5s
  the first cue waits for its own line, so a fight fought at speed made no
  sound at all. **The cue rides the gesture, not the release** —
  `finish_reveal` stays silent because `finish_arena_fight` and the test
  fixtures call it as a transition.
- **A refusal is one sentence on two surfaces, and `App::refuse` is the one
  door** — `App::status_line` for the popup the player typed into, and
  `Game::note_refusal` for the log they scroll back through. It is drawn
  **inside the popup**, under the title: `draw_popup` takes it as an
  argument, which is what makes 83 call sites decide rather than default,
  and `draw` is the one place that knows which surface is on top — where an
  arm draws two things the underlying one takes `None`. It is **counted by
  `popup_layout` and is not a `Row`**, so the panel grows a line rather than
  covering one and the `Row::Item` span a keypress indexes never moves.
  `needs_status_banner` names the four screens that draw **no popup** —
  `Battle`, `BattleResult`, `FrameMap`, `FieldCastCell`. **`note_refusal` is
  silent while a battle is open**: `since_round` slices by position and the
  reveal counts raw lines, so a refusal pushed from a battle submenu draws as
  narration and eats a keypress — which is why `Battle` keeps the strip.
  **Not every `status_line` write is a refusal**; a confirmation and an IO
  failure assign the field directly, or the history fills with the game
  saying yes. `App::report` is the verdict form 28 sites collapse into and
  takes a finished `Result`, so the `&mut self.game` borrow ends before
  `refuse` wants the whole of `self`. The census
  (`every_screen_draws_a_refusal_exactly_once`) drives all 86 `Mode`s
  through `draw` and counts what was painted.
- **`MessageSource` has two readers, and the battle pane's is not the
  filter.** `battle_rows` drops base news unconditionally, because
  `since_round` slices by *position*. The filter is app-core's and **not**
  `Game::battle_log`'s, since everything pacing the reveal counts *raw* lines;
  `battle_rows` truncates *before* it filters, the mirror image of `pane_rows`.
- **The reveal is gated on `Mode::is_battle`, and that gate keeps it off the
  map.** `MessageLog::round_start` is deliberately never closed. Ungated it
  paced the map's log forever and **swallowed one keypress per line the base
  logged**.
- **The map log pane is filtered; the history screen (`L`) is not.**
  `pane_rows`' stage order is load-bearing: unrevealed tail, then filter, then
  fold, then capacity.
- **All three log surfaces fold repeats, and `resources::condense` is the one
  fold.** It sits in `pane_rows` and `battle_rows` — on the rows about to be
  drawn, *after* the truncation — because `revealed`, `hidden_log_lines` and
  `battle_view_at` all count **raw** lines. A wipe still takes one beat per
  kill; the count ticks up. Adjacent-runs-only would fold nothing, since
  `finish_member` logs the pack's next program between kills.
- **The map log pane wraps, and the cut comes off the *oldest* end.**
  `Painter` clips nothing horizontally — `draw_row`'s rule from the other side
  of the screen — so an unwrapped entry drew clean across the info column
  beside it (x=2193.3 against an 846.9 pane edge at 1280x720). The wrap is
  `text::wrap` called, not copied, and `message_columns` divides the pane's
  remaining advance by a **measured** `"M"`, since the UI face is DejaVu Sans
  Mono and `Metrics` carries no advance. **The trap is the direction of the
  cut**: `pane_rows` hands rows over **oldest first**, and `draw_playing_base`
  counts the capacity it asks for in *entries*, so once an entry is several
  rows the old `break` at the floor dropped the **newest** news — the half the
  player is reading. Rows are built, counted by `rows_fitting`, then `drain`ed
  from the front; an entry taller than the pane keeps its own tail, which is
  what a terminal does. The tag rides the **first row of its entry alone**, or
  one line reads as five lines of traffic. `draw_message_line` split into
  `message_text` + `draw_message_text` rather than gaining a parameter, so
  `battle.rs` keeps one call and the styling logic has no second copy —
  `battle.rs` is still unwrapped on purpose, its box measures itself and
  widens.
- **`retain_outcomes_since_battle` keeps only `Outcome`, `Loot`, `LevelUp`,
  `Raid` and `Complete`.** A plain `log()` is `Info` and is pruned.
- **The map's status column cannot grow, and `draw_row` clips vertically
  only.** It holds 38.5 monospace cells; the widest shipped buff row already
  spends all but 3.8 of them, so a companion's `(holder)` tag drew 360px off
  the panel in silence. `TagStyle` on `buff_entries` is the fix — `Inline`
  for the battle box, which measures itself through `buff_panel_width` and
  widens, `OwnLine` for the column, which cannot. `cap_entries` takes
  `Vec<Vec<Row>>` because an entry may be two rows: a flat cap strands a
  holder line's name and miscounts "+N more" as lines rather than routines.
  A width test that skips non-`Item` rows measures nothing here.

- **A read-only screen's row count is owned by app-core and drawn by gui, so
  any per-row transform must live in the engine.** Both sides call
  `Game::message_history`; folding in the renderer opens the screen on a row
  that isn't drawn.
- **A group menu's rows are hidden dynamically**, so `base_menu_rows` /
  `party_menu_rows` must be the *only* source of them. A row survives two
  clauses, and `surface_only` is a flag in that table rather than a check
  inside each predicate, because what it must stay in step with is
  `require_surface`'s caller list.
- **A Broker's board is derived, never stored** — seeded off
  `(world seed, zone, epoch)`. No save field, no `GameRng` draw, no
  save-scumming, and it rotates on its own. Readable from **anywhere**,
  underground included: it makes no claim about where the party is standing.
- **Reading a Broker's board and signing it are two questions, and
  `Game::broker_reach` is the one call that answers both.** `NoBroker` /
  `OffBase` / `AtBroker`; `board_defs` refuses on `NoBroker` alone, the two
  verbs require `AtBroker`, and the base menu's row test and the screen's
  header read the same value. Three states rather than two booleans for
  `NoPost::BoxedIn`'s reason, and `ContractRefusal::NotAtBroker` is distinct
  from `NotOffered` because the two leave the player different errands.
  `AtBroker` measures the **base** — `base_pos` then `BaseGrid::is_floor` —
  never the distance to the Broker. `CONTRACT_BOARD_RANGE_TILES` was deleted
  rather than widened: a constant would freeze the desk at the radius a base
  *starts* at. Two traps: the row test must call `broker_reach` and not
  `contract_board` (which rolls every template and walks the habitat ring
  before it can answer), and a fixture standing a Broker up must stand a
  **Home** up with it.
- **A `starter` contract jumps the board queue, and only in sector 1.**
  `board_defs` fills its three slots from unfinished starters first — three
  uniform draws out of fourteen made a new run's first job a coin flip, and
  `min_zone: 0` says a contract *may* be offered, not that it is offered
  first. The gate is `ZoneLevel <= 1`, so a mid-run board is untouched, and
  the second tier draws exactly as the old single loop did. Templates have no
  such field.
- **You *run* (or *invoke*) a routine; the noun is an *invocation*.** "Cast"
  and "spell" are the fantasy words this setting does not use, and unlike
  Raid the rename went all the way through the identifiers —
  `Game::run_field_routine`, `Mode::FieldRoutine*`, `FieldRoutineTarget`,
  `FieldBuffKind::scales_with_invoker`. **The two collisions are what pick
  the words**: "run" as a noun is a playthrough, and "runner" is a hauling
  body, so neither may carry the ability sense. `assets/` is gated by
  `no_player_facing_text_says_cast_or_spell`, which matches **tokens** — a
  substring rule fails `broadcast_storm` and "spelled out" while proving
  nothing. Player-facing strings built in Rust are held by review.
- **"Raid" is the code's word and "GC Entropy Sweep" is the player's.** The
  `.ron` fields are mod schema and deliberately kept their names. New
  player-facing text follows the player's word; note the noun-phrase trap.
- **`world.get::<Stats>(e).is_none()` is the idiom for "this entity is
  gone"** — don't reach for `World::get_entity`.
- **There is one place a runtime path is decided,
  `crates/launcher/src/paths.rs`**, and `main` reads nothing else: the loose
  asset tree, the player-data directory, and whether this build has a repo
  behind it. Installed-ness is **sniffed** (an `assets/` beside
  `current_exe()`, then a macOS bundle's `../Resources/assets`) rather than
  flagged, because a forgotten flag ships a zip that works only on the build
  machine. Player data goes to the **OS data directory in every layout**, a
  repo build included, so a dev build can reproduce a player's report. `dev`
  is `Some` **iff** the repo layout was chosen. **The trap is a second site
  resolving against `CARGO_MANIFEST_DIR`**: it works on the build machine,
  works nowhere else, and nothing fails to compile. **Bevy's `AssetPlugin` is
  exactly that second site** and had to be fed rather than left alone —
  `gui::asset_plugin` hands it the resolved path off the `App` already
  carrying it, never a parameter of its own. An absolute `file_path` survives
  bevy's `get_base_path().join()`, which is what makes it override the guess.
  The four dev bins keep `repo_root()` on purpose, so "one place" is about
  the *game's* paths. `resolve()` is **infallible** — a fallible lookup would
  ripple through `DevTemplates`' `fn` pointer and force an app-core change.
  `FERAL_ASSETS_DIR` is the one override and a second per-path override is
  refused; empty reads as unset. `migrate_from_repo` is a **move, not a
  copy**, and does nothing when the data directory already holds a `.bin`.
- **Engine test fixtures live in `crates/engine/src/tests/support.rs`.** Look
  there before writing a new one.
