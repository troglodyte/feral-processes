# Auto-resolve combat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `[R]` on either battle screen plays the rest of the fight out inside one engine call and lands on that model's results screen.

**Architecture:** One engine door, `Game::auto_resolve_battle`, loops the driver each model already has: a group step extracted out of `arena::run::run_rep` (so the arena and the key share it), and `tactical_drive_turn` for battle maps. A round cap in `tuning.rs` turns a stalemate back to the player. App-core binds `R` on both screens; gui draws it.

**Tech Stack:** Rust, bevy_ecs (engine), app-core state machine, bevy_egui renderer.

**Spec:** `docs/superpowers/specs/2026-09-16-auto-resolve-combat-design.md` — read it first; this plan argues from it.

## Global Constraints

- No save change, no schema change, no `.ron` change.
- The party fights exactly as `[A]` does: basic swings, no routines. Never add routine use (that is #103).
- The stop rule is `!has_active_battle() || is_game_over().is_some()` — **never** the player's HP (see `run_rep`'s comment).
- The group step is **one** method both `run_rep` and `auto_resolve_battle` call. A second copy is forbidden (CLAUDE.md, "a call, not a copy").
- `R` must not be a hidden key (`W`/`T`/`Z`, `crates/engine/EASTER_EGGS.md`); lowercase `r` stays unbound on both screens (`r_is_not_a_second_way_into_the_picker` must keep passing unedited).
- Stall refusal text, verbatim: `Couldn't settle it — finish by hand.`
- `docs/manual.md` and root `README.md` are not touched. CHANGELOG is written at the merge (deploy skill), not on the branch.
- Gates after every task: `cargo fmt`, `cargo clippy --workspace --all-targets` clean, then the task's targeted tests. `cargo test --workspace` at the end of Task 4.
- Commit per green task. **Never push.**

## Facts already verified (2026-09-17)

- `arena/run.rs:46` `run_rep` holds the group step inline: `battle_plan_remaining(Attack { group: 0 })` (Err ⇒ break), `battle_round_ready()` (false ⇒ break), `battle_resolve_round()`. Bracing (`bracing_slots`) runs before it and stays in `run_rep`.
- `arena/run.rs:100` `run_tactical_rep` loops `tactical_drive_turn()` (false ⇒ break); its private `tactical_round` reads `TacticalBattle::round`.
- `game/combat.rs:143` `Game::fight_round(&self) -> Option<u32>` already answers the round for **either** model. It is private; make it `pub(crate)` and use it for the cap — don't write a third reader. `run_tactical_rep`'s private `tactical_round` may then be replaced by it (`fight_round().unwrap_or(0)`), which is the same answer.
- `tactical/ai.rs:491` `tactical_drive_turn` is `pub(crate)`, doc says "Its only caller is `arena::run`".
- Longest fight in `docs/measurements/` is 21 rounds (`2026-08-19-combat-model-slice-1.md`), so `AUTO_RESOLVE_ROUND_CAP = 200` is ~10× headroom. Use 200.
- `PartyCommandKind` (`battle.rs:547`) is matched in app-core `run_party_command` only; gui reads `.label` alone (`render/battle.rs:504`).
- Two tests pin the party key list: `crates/engine/src/tests/combat.rs:300` and `crates/app-core/src/tests/battle.rs:96` (`vec!['A', 'D', 'j']`). Both must gain `'R'` in whatever position `battle_party_commands` emits it.
- The group action line (`render/battle.rs:~504`) is drawn unclipped with **no width test**. The tactical bar has one (`render/tactical.rs:1639`, `the_action_bar_fits_the_log_pane`).

---

### Task 1: Extract the group step; the arena calls it

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs` (new method beside `battle_resolve_round`)
- Modify: `crates/engine/src/game/combat.rs:143` (`fight_round` → `pub(crate)`)
- Modify: `crates/engine/src/arena/run.rs` (`run_rep` calls the step; `run_tactical_rep` may use `fight_round`)

**Interfaces:**
- Produces: `pub(crate) fn battle_auto_round(&mut self) -> bool` on `Game` — plans `Attack { group: 0 }` for every unplanned commanded slot, resolves the round if ready, returns `true` iff a round was resolved. `false` means the plan was refused (fight over) or the round is not ready.
- Produces: `pub(crate) fn fight_round(&self) -> Option<u32>`.

This is a pure refactor, so the test is a **record comparison**, not a new red test.

- [ ] **Step 1: Capture the arena baseline before touching code.**
  ```sh
  cargo run --release --bin arena -- dev-arenas/opening-fight.ron --out $SCRATCH/before-group.ron
  ```
  plus `dev-arenas/tactical-full-group.ron` to `$SCRATCH/before-tactical.ron`. Use a scratch directory, not the repo.
- [ ] **Step 2: Add `battle_auto_round`** with a doc comment that says it is `[A]`'s path and names both callers (`run_rep`, `auto_resolve_battle`) — the arena's published numbers rest on it being the game's own step.
- [ ] **Step 3: Rewrite `run_rep`'s body** to: stop check → bracing loop → `if !game.battle_auto_round() { break; }` → `watch.observe(game)`. Keep the existing comments that still apply; move the two "an `Err` here means…" comments into the new method.
- [ ] **Step 4: Make `fight_round` `pub(crate)`** and replace `run_tactical_rep`'s `tactical_round` helper with it (delete the helper).
- [ ] **Step 5: Re-run both arena commands to `after-*.ron` and `diff`.** Expected: identical. Any difference is a behaviour change — stop and report it, don't adjust.
- [ ] **Step 6: Run** `cargo test -p feral-processes-engine arena` — all pass (`the_default_party_plan_never_braces`, `the_same_scenario_run_twice_reports_the_same_thing`, the tactical ones).
- [ ] **Step 7: Commit** `Engine: the group auto step is one method the arena calls`.

---

### Task 2: `Game::auto_resolve_battle`

**Files:**
- Create: `crates/engine/src/game/auto_resolve.rs` (register in `game/mod.rs` alongside the other `combat_*` modules)
- Modify: `crates/engine/src/tuning.rs` (new const in the combat/battle section)
- Modify: `crates/engine/src/tactical/ai.rs:474-490` (doc of `tactical_drive_turn` only)
- Modify: `crates/engine/src/lib.rs` (re-export `AutoResolve` the way `PartyCommandKind` is re-exported, line ~69)
- Test: `crates/engine/src/tests/auto_resolve.rs` (register in `tests/mod.rs`)

**Interfaces:**
- Consumes: `battle_auto_round`, `fight_round` (Task 1); `tactical_drive_turn`.
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AutoResolve { Finished, Stalled }
  pub fn auto_resolve_battle(&mut self) -> AutoResolve
  pub const AUTO_RESOLVE_ROUND_CAP: u32 = 200;
  ```

**Behaviour.** Read the start round with `fight_round()`. Loop while `fight_round() - start < AUTO_RESOLVE_ROUND_CAP`: stop check first (return `Finished`); then step the model that is open — `TacticalBattle` resource present ⇒ `tactical_drive_turn()`, else `battle_auto_round()`. A step returning `false` while the fight is still open and no game over is set ⇒ `Stalled` (nothing more this loop can do). Falling out on the cap ⇒ `Stalled`. After a step, re-check the stop rule before re-reading the round (a fight that ended has no round: `fight_round()` is `None`, which must read as finished, not as round 0).

**`tactical_drive_turn`'s doc:** replace "Its only caller is `arena::run`…" with: two callers, the arena and `auto_resolve_battle`; the second drives companions only because the player pressed `[R]` and asked for it, which is the consent the `tactical_ai_actor` gate otherwise stands in for.

- [ ] **Step 1: Write the failing tests** in `tests/auto_resolve.rs`:
  - `a_winnable_group_fight_resolves_and_closes` — `Game::new` + one weak wild (hp small) + `start_battle`; assert `Finished` and `!has_active_battle()`.
  - `a_winnable_tactical_fight_resolves_and_closes` — reuse `super::tactical::tactical_fight(&mut game, 1, 10)` (already `pub(super)`); same asserts. Don't wait for the player's turn first — the door must work from whoever holds it.
  - `a_permadeath_loss_stops_with_the_game_over_set` — copy the setup of `tests/combat.rs:723` (`a_round_that_kills_the_player_ends_the_battle`: Permadeath, a `construct` with hp/atk 100_000); assert `Finished`, `is_game_over().is_some()`. The test must return (it is the "doesn't spin" check).
  - `a_fight_nobody_can_end_stalls_at_the_cap_with_the_fight_open` — group model; wild hp/max_hp 10_000_000, atk 0; player hp/max_hp 10_000_000. Assert `Stalled`, `has_active_battle()`, and that the round advanced by exactly `AUTO_RESOLVE_ROUND_CAP` (read the round through the battle view the tests already use, or `fight_round` since tests are in-crate). If damage floors keep the player's side able to kill it, raise hp further rather than changing the rule.
  - The same stall on a battle map (`tactical_fight` then set the pack's hp/atk the same way), asserting `Stalled` and the fight open.
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine auto_resolve` — FAIL to compile (no `auto_resolve_battle`).
- [ ] **Step 3: Implement** the enum, const (doc comment: why 200, citing the 21-round measurement, and that the cap is a stalemate guard and never a difficulty lever), the method and the doc rewrite.
- [ ] **Step 4: Run** the tests — PASS. Then delete the stop check's `is_game_over` half and confirm the Permadeath test fails (hangs to the cap and returns `Stalled`, or asserts wrong); restore it. Delete the cap check and confirm the stall test would not return — don't actually hang: reason it from the code and note it in the report instead.
- [ ] **Step 5: Commit** `Engine: auto_resolve_battle plays a fight out under a round cap (todo #105)`.

---

### Task 3: `[R]` on the group roster

**Files:**
- Modify: `crates/engine/src/battle.rs:547` (`PartyCommandKind::AutoResolve`)
- Modify: `crates/engine/src/game/combat.rs:1534` (`battle_party_commands` emits it: `key: 'R'`, `label: "[R]esolve"`, `needs_target: false`; place it after `AllDefend`, before `JackOut`)
- Modify: `crates/engine/src/tests/combat.rs:300` (expected list gains `'R'`)
- Modify: `crates/app-core/src/app/battle.rs` (`run_party_command` arm; the comment at line ~48 listing the party keys)
- Test: `crates/app-core/src/tests/battle.rs` (expected list at line 96; new tests)

**Interfaces:**
- Consumes: `Game::auto_resolve_battle`, `AutoResolve` (Task 2).
- Produces: `App::auto_resolve(&mut self)` — `pub(crate)`, shared by Task 4.

**`App::auto_resolve`** (put it in `app/battle.rs`; Task 4 calls it from the tactical handler, so it must branch on which model finished):
- call `auto_resolve_battle`; on `Stalled` ⇒ `self.refuse("Couldn't settle it — finish by hand.")`, mode unchanged.
- on `Finished`, group model: `settle_after_round(false)`, then `finish_reveal()`, then `push_battle_outcome_sounds(None, false)`. Order matters: `settle_after_round` restarts the reveal, which `finish_reveal` then completes; the sounds call runs `check_game_over`.
- on `Finished`, tactical model: `settle_tactical_end()` (make it `pub(crate)` if needed). Decide the model **before** calling the engine — once the fight ends neither resource is left to ask. Read it off the current `self.mode` (`Mode::TacticalBattle` vs the group modes).
- Note `settle_after_round` is private to `app/battle.rs`; `settle_tactical_end` is private to `app/tactical.rs`. Widen only what the call needs.

- [ ] **Step 1: Write the failing app-core tests** (reuse `battling_app()`):
  - `r_resolves_the_fight_and_opens_the_results_with_nothing_unrevealed` — press `R`; assert `mode == Mode::BattleResult` (or `GameOver` — accept either, the seed decides), `!app.is_revealing()`, and `!has_active_battle()`. If the seed's fight can stall, pick a fixture that can't (weaken the wild in `battling_app_with`).
  - `a_stalled_resolve_stays_on_the_roster_and_says_so` — `battling_app_with` making both sides unkillable as in Task 2; press `R`; assert `mode == Mode::Battle` and `status_line == Some("Couldn't settle it — finish by hand.")`.
  - Update the two `['A', 'D', 'j']` pins. The existing `battle_action_keys_come_from_the_engine_…` loop then probes `R` for free.
- [ ] **Step 2: Run** `cargo test -p feral-processes-app-core battle` and `cargo test -p feral-processes-engine combat` — FAIL.
- [ ] **Step 3: Implement** the variant, the command, `App::auto_resolve`, and the arm.
- [ ] **Step 4: Run** both, plus `cargo test -p feral-processes-engine easter_eggs` — PASS.
- [ ] **Step 5: Width.** The group action line is unmeasured and now ~12 cells longer. Add a gui test beside the existing battle render tests that builds the line the renderer builds (extract the `actions.join("   ")` string builder into a small fn if needed, so the test measures what is drawn) and asserts it fits `screen_w - 2*margin` at 1280x720 via `paint::with_painter` (see the `the_action_bar_fits_the_log_pane` pattern). Delete `[R]esolve` and confirm it would have passed without it; if it fails **with** it, report the overflow rather than shortening other labels.
- [ ] **Step 6: Commit** `App-core: [R] resolves an abstract fight (todo #105)`.

---

### Task 4: `[R]` on the battle map, the bar, the help page

**Files:**
- Modify: `crates/app-core/src/app/tactical.rs:41` (`handle_tactical_key`)
- Modify: `crates/gui/src/render/tactical.rs:762` (`action_bar`)
- Modify: `assets/help/20-controls.md:94-99`
- Test: `crates/app-core/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `App::auto_resolve` (Task 3).

**Key placement:** in `handle_tactical_key`'s `match key`, which already sits after the auto-attack stop and the `tactical_player_turn()` wait — so `R` while `[A]` runs is swallowed by the stop, and `R` on a wild turn waits. Add `GameKey::Char('R') => self.auto_resolve(),` with the uppercase comment the neighbouring `E` arm carries.

**Bar:** add `("R", "resolve")` immediately **before** `("A", "auto-attack")`, keeping `A` last per its comment. `the_action_bar_fits_the_log_pane` measures this; if it now fails at 1280x720, report — don't drop rows by hand.

**Help:** group list gains `- R — resolve: play the rest of the fight out now`; the battle-map sentence gains `, and R resolves the fight` alongside `E ends the turn`. Check `assets/help/README.md`'s wrap/width census still passes (`cargo test -p feral-processes-engine help`).

- [ ] **Step 1: Write the failing tests** (reuse `fighting()` / `wait_for_the_player`):
  - `r_on_the_players_turn_opens_the_results` — press `R`; assert `mode == Mode::TacticalResult` (or `GameOver`).
  - `r_while_auto_attack_runs_only_stops_it` — arm with `A` (as `auto_attack_arms_and_stops_…` does), press `R`; assert `!app.tactical_auto`, `mode == Mode::TacticalBattle`, fight still open.
  - Leave `r_is_not_a_second_way_into_the_picker` untouched; it must still pass.
- [ ] **Step 2: Run** `cargo test -p feral-processes-app-core tactical` — FAIL.
- [ ] **Step 3: Implement** the arm, the bar row, the help lines.
- [ ] **Step 4: Run** `cargo test -p feral-processes-app-core tactical`, `cargo test -p feral-processes-gui tactical`, `cargo test -p feral-processes-engine help` — PASS.
- [ ] **Step 5: Full gate:** `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. Report the counts seen.
- [ ] **Step 6: Commit** `App-core/gui: [R] resolves a battle-map fight (todo #105)`.

---

## After the tasks

- Final whole-branch review (opus) against the spec and the Global Constraints above.
- Flip the spec's `**Status:**` and add rows to `docs/superpowers/INDEX.md`; add a CLAUDE.md seam line only if the review finds a real trap (candidate: "`auto_resolve_battle` and `run_rep` share `battle_auto_round`; a copy of the step is the arena's numbers drifting from the game").
- Not playtested by any agent — the user plays it.
