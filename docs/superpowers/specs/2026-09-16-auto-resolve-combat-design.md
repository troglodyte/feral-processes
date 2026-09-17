# Auto-resolve combat

**Date:** 2026-09-16
**Status:** design approved, not implemented.
**Todo:** #105, "auto resolve combat".

On their own turn in any fight, the player presses `[R]`. The rest of the
fight is played out immediately, inside engine calls, and the player lands
on that model's normal results screen. The party fights the way `[A]`
already fights for it. The cost of skipping the fight is fighting worse
than a player would by hand.

## Decisions taken in brainstorming

| Question | Answer |
|---|---|
| What it does | Plays the fight to its end with no pacing and opens the results screen |
| Which models | Both, group and tactical |
| When | Any time the player holds the turn, including the opening turn |
| What the party does | Whatever `[A]` does today: basic swings, no routines. Picks up #103 for free |
| A preference that resolves every fight | Not in scope |

## 1. Engine: `Game::auto_resolve_battle`

```rust
pub enum AutoResolve { Finished, Stalled }
pub fn auto_resolve_battle(&mut self) -> AutoResolve
```

The method checks which model is open and loops that model's existing
driver. It stops when `!has_active_battle() || is_game_over().is_some()`.
It never stops on the player's HP. A Forgiving defeat is rebooted inside the
round that lands it, and `battle_resolve_round` does nothing forever once a
Permadeath game over is set. This is `arena::run::run_rep`'s own stop rule,
and its comment explains why.

- **Group model:** one step is "plan `Attack { group: 0 }` for every
  unplanned slot, then resolve the round if it is ready." `run_rep` holds
  that step inline today. It moves into one `pub(crate)` Game method, and
  `run_rep` and `auto_resolve_battle` both call it. `run_rep`'s bracing
  stays in `run_rep`, placed before the call. That keeps the arena on the
  game's own plan: the arena's published numbers rest on `[A]`'s path being
  the game's path, so a second copy of the step is exactly what CLAUDE.md's
  "a call, not a copy" rule forbids.
- **Tactical model:** one step is `tactical_drive_turn()`, which is already
  the arena's loop body and runs whichever side holds the turn.
  `tactical_auto_beat` is the same beat loop split one beat per call, so
  auto-resolving is the fight `[A]` would have paced. `tactical_drive_turn`'s
  doc comment says "only caller is `arena::run`", because a real fight must
  not walk a companion by itself unasked. That comment is rewritten to name
  the second caller and to say that the player asked.

**The cap.** Neither live model has a round cap, and a fight with no
reachable target just hands turns on forever (`run_tactical_beat`'s
empty-targets branch). `tuning::AUTO_RESOLVE_ROUND_CAP` bounds the loop, in
rounds for both models. The tactical model counts
`TacticalBattle::round` changes, as `run_tactical_rep` does. The value is
far above any real fight but low enough that a stalemate does not flood the
log. Start at 200 and check the longest fights in `docs/measurements/`
before settling on it. Hitting the cap returns `Stalled` with the fight
still open, and every round already fought stays fought.

No save change and no schema change: a battle never persists mid-fight.

## 2. App-core: the key

- `[R]` is free on both battle screens and is not one of the hidden keys in
  `EASTER_EGGS.md`. `no_battle_action_or_party_command_claims_a_hidden_key`
  still has to pass.
- **Group:** a fourth `PartyCommand` (`PartyCommandKind::AutoResolve`,
  `'R'`, `"[R]esolve"`, `needs_target: false`) in
  `battle_party_commands`, so the roster draws it with no gui change.
  `handle_battle_key` matches party commands on the raw character before
  its lowercase retry, and no per-slot action is `r`, so `R` needs no
  special handling.
- **Tactical:** handled in `handle_tactical_key` after the auto-attack stop
  and the not-your-turn wait, so `[R]` pressed while `[A]` is running only
  stops `[A]`. That matches the rule that any key stops it.
- **After `Finished`:**
  - Group: `settle_after_round`. That sends an arena fight to
    `finish_arena_fight`, and a normal fight to `BattleResult`, which calls
    `restart_reveal`. Then `finish_reveal()` so the results appear at once.
    Then `push_battle_outcome_sounds`, which also calls `check_game_over`.
  - Tactical: `settle_tactical_end`, which leads to `TacticalResult` and
    calls `check_game_over`.
- **After `Stalled`:** the mode is unchanged and `App::refuse` reports
  "Couldn't settle it — finish by hand." on the status line alone —
  `Game::note_refusal` is deliberately silent while a battle is open, and a
  `Stalled` fight is still open, so this does not also reach the log.

## 3. Gui

- The tactical `action_bar` gains an `[R]esolve` entry beside `[A]`.
- `assets/help/20-controls.md` and CHANGELOG get one line each.
  `docs/manual.md` and the root README are left alone.

## 4. Tests

Written failing first.

**Engine**
- A winnable group fight returns `Finished` and leaves no battle open.
- A winnable tactical fight returns `Finished` and leaves no battle open.
- A Permadeath loss stops the loop with the game over set, rather than
  spinning.
- A fight nobody can end returns `Stalled` at the cap with the battle still
  open. Build it by giving both sides nothing they can reach, or by
  pinning HP, whichever an existing fixture already supports.
- The arena's `opening-fight` and tactical reps are identical before and
  after the extraction, by comparing records at a fixed seed.

**App-core**
- `[R]` on the group roster reaches `BattleResult` with nothing left
  unrevealed.
- `[R]` on the player's tactical turn reaches `TacticalResult`.
- `[R]` while `[A]` is running only stops `[A]`.
- A stall leaves the mode on the battle screen and shows the status line.

## Out of scope

- A profile option to auto-resolve every fight.
- A win-odds preview before pressing `[R]`.
- Routine use by the party. That is #103.
