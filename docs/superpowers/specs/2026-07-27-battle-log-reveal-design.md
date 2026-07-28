# Paced battle narration and a results-only handoff

**Date:** 2026-07-27
**Branch:** `feat/battle-log-reveal`
**Status:** Design, approved

## Problem

Three related complaints about the battle log pane, all consequences of one
fact: the battle screen (`render/battle.rs:265`) and the base screen
(`render/base.rs:163`) render the *same* rolling `MessageLog`, tail-first.

**A round arrives all at once.** `Game::battle_resolve_round` pushes every
line of a round's narration in one call, so the pane jumps from one wall of
text to another. Nothing paces the reading, and the `MessageKind::Round`
marker — added precisely because "the narration of six rounds reads as one
undifferentiated block" — only separates the blocks, it doesn't slow them
down.

**Battles bleed into each other.** The pane shows the log's tail, not this
battle's lines, so a fight opens showing the end of the previous one.

**The blow-by-blow follows the player out.** When the battle ends and the
mode flips to `Mode::Playing`, the base screen's pane is still showing the
same shared log — so the last thing the player reads on the map is a list of
individual attacks, with the rewards buried in it.

## Scope

1. Battle narration reveals at a fixed, tunable pace, blocking the next
   action until it finishes, with any key skipping to the end.
2. The battle pane shows only the current battle's lines.
3. Only a battle's *results* survive onto the non-battle screen.

Not in scope: scrollback in either pane, a player-facing speed setting (the
skip key covers the impatient case), per-line reveal within a line
(typewriter-by-character), and any change to what a battle awards.

## Decisions

Four questions were settled before design:

- **Pacing blocks, with a skip key.** A purely cosmetic reveal is decoration
  — a fast player never sees it. Any key press dumps the rest and hands
  control back.
- **Results are engine-tagged.** A new `MessageKind::Outcome` marks the lines
  that are genuinely results. Today they are a mix: drops are `Loot`, levels
  are `LevelUp`, but the kill line, `"You gain {n} XP."` and the decompile
  success/failure lines are plain `Info`, so nothing in the code can pick
  them out.
- **Reset means the pane is battle-scoped**, not that the log is cleared.
  Clearing the shared log at battle start would destroy the raid or level-up
  the player was mid-read when an ambush hit.
- **The pace is a named constant in code.** Explicitly *not* `tuning.rs`:
  CLAUDE.md scopes that file to difficulty ("how hard the game is, is not"
  moddable), and reveal speed is presentation.

## Architecture

### Engine — marks and tags, no pacing

The engine contributes what only it can know, and nothing about timing.

`MessageKind` gains an `Outcome` variant, applied to the result pushes in
`game/combat_rewards.rs` and `game/combat_round.rs`: the kill line
(`"The rogue program crashes and deletes itself!"`), the XP line, and the
decompile outcomes (`"ICE breached! …"`, `"The program's ICE holds …"`,
`"No taming catalyst left …"`). `Loot` and `LevelUp` already tag themselves
and stay as they are.

`MessageLog` gains a **monotonic sequence counter**. A battle-start mark
cannot be a raw index into `lines`: the log drains its oldest entries at
`MESSAGE_LOG_CAP = 100` (`resources.rs:56`), so an index silently comes to
point at the wrong line in any battle long enough to overflow it. The
counter counts lines ever pushed; converting a mark to a slice clamps
against what has since been drained.

New `MessageLog` surface:

- `mark(&self) -> MessageMark` — the sequence number of the next line.
- `since(&self, mark) -> &[(MessageKind, String)]` — lines from that mark on,
  clamped if the mark has been drained past.
- `retain_outcomes_since(&mut self, mark)` — drops the non-result lines from
  that mark on, keeping `Outcome`, `Loot` and `LevelUp`. It touches *only*
  the range from the mark, so nothing logged before the battle is affected —
  a `Raid` line from before the ambush is not a battle result and is not
  pruned by it.

Battle start records the mark on `BattleState`. `Game::battle_log(n)` returns
this battle's lines, alongside the existing `Game::message_log(n)`.

### Pruning at battle end

When a battle ends, the engine calls `retain_outcomes_since` on that battle's
range. The shared log is then left holding the results and nothing else, so
**the base screen needs no filter at all** — it keeps calling
`message_log(n)` and is simply correct.

The alternative was to leave history intact and filter in `render/base.rs`,
but then every screen that reads the log has to know the rule, and a screen
added later will not. Pruning answers the question in one place. Nothing
reachable is lost: neither pane has scrollback, and the battle pane has
already shown the full narration live, at the pace this feature exists to
set.

### app-core — the reveal state machine

`App` owns a `BattleReveal`:

```rust
struct BattleReveal {
    /// Lines of the current battle released to the pane so far.
    revealed: usize,
    /// Fractional carry, so a slow frame doesn't lose a partial line.
    accumulated: f32,
}
```

`battle_resolve_round` leaves it with lines pending. `advance_reveal(dt)`
adds `dt * REVEAL_LINES_PER_SECOND` to `accumulated` and releases whole lines
from it. `is_revealing()` reports whether any remain.

**Timing is injected, not read.** `update_realtime` calls `Instant::now()`
internally, but CLAUDE.md forbids wall-clock dependence in tests, so the
reveal takes `dt` as a parameter. The frontend supplies Bevy's
`delta_secs()`; tests supply whatever they like.

While `is_revealing()`, `handle_key` swallows the key and completes the
reveal instead of acting on it — that is the skip.

**The transition to `Mode::Playing` is not deferred** — see the amendment
below. The mode flips the moment the engine reports the battle over, as it
does today, and the *results* scroll into the map's log pane instead.

### gui — unchanged responsibilities

`frame` calls `advance_reveal(input.time.delta_secs())` once. `render/battle.rs`
reads `app.revealed_battle_log(capacity)` instead of
`game.message_log(capacity)`, and suppresses the action bar while revealing.
No drawing code changes and `paint.rs` is untouched, so the `Painter` seam
holds.

## The constant

```rust
/// How fast battle narration scrolls into the log pane, in lines per second.
/// Presentation, not difficulty, so it lives here and not in `tuning.rs`.
pub const REVEAL_LINES_PER_SECOND: f32 = 12.0;
```

In `crates/app-core/src/lib.rs`, beside `REALTIME_TICK_INTERVAL`. 12/sec puts
a six-line round at half a second. This is a starting guess and expected to
be adjusted once it has been seen on screen.

## Error handling

No new failure modes: no I/O, no parsing, no fallible conversion. Two edge
cases need a defined answer rather than error handling:

- **A battle that starts and ends in one round.** Mark and prune still
  bracket a valid, non-empty range; the results survive as they would from a
  ten-round fight.
- **Save/load during a reveal.** `BattleReveal` is transient presentation
  state and is not persisted. A loaded save resumes with nothing pending.
  **No save-format bump.**

## Testing

All headless, no sleeping, no wall-clock reads.

**Engine**

- Outcome tagging survives a full battle: every reward line comes back
  tagged, and no blow-by-blow line does.
- `battle_log` returns only the current battle's lines; a second battle opens
  with an empty pane.
- Pruning keeps rewards and drops the blow-by-blow.
- The mark stays correct across a log that overflows `MESSAGE_LOG_CAP` —
  the drift case a raw index would fail, and the reason the sequence counter
  exists.

**app-core**

- `advance_reveal` releases lines in proportion to `dt`, and a zero `dt`
  releases none.
- A keypress mid-reveal completes the reveal and does *not* act on the key.
- The `Mode::Playing` transition waits for the last line.
- The fractional carry doesn't lose a line across two half-line frames.

**gui**

- Existing battle-screen tests keep passing against the new accessor.

`balance_sim` is not expected to move — no `.ron` and no `tuning.rs` value
changes — but it will be run, since the engine changes sit in combat code.

## Amendments made while planning

Three corrections, all decided after this design was first approved. The
sections above have been edited to match.

**1. The killing round reveals on the map, not on the battle screen.**
`end_battle` removes `BattleState` (`combat_status.rs:483`), so
`battle_view()` returns `None` and `draw_battle` bails at line 152 — the
battle screen has no rosters left to draw. Three options were weighed: a
dedicated results screen, freezing the last roster state, or flipping to the
map and revealing the pruned results there. **The map was chosen**, as the
smallest build. The accepted cost: the final round's blow-by-blow is pruned
away and never read. The player sees that they won and what they got, not
the blow that won it.

This also dissolves a conflict the original design would have hit. Pruning
at `end_battle` would otherwise have yanked unrevealed narration out from
under an in-flight reveal — the one round that most needed reading. With the
final round's narration knowingly discarded, there is nothing left to
protect.

**2. `retain_outcomes_since_battle` also keeps `MessageKind::Raid`.**
`systems.rs:35,146,236` and `difficulty.rs:19` write to `MessageLog`
directly through `ResMut<MessageLog>`, bypassing `Game::log_kind`, and they
run during the `tick()` a battle action triggers. A raid alert that lands
mid-fight is world news, not battle narration, and must survive the prune.

**3. The battle mark lives on `MessageLog`, not `BattleState`.**
`end_battle` removes `BattleState`, which would take the mark with it while
the frontend is still revealing that battle's results. The mark is instead
replaced by the next `start_battle`. `MessageLog` also carries a
`battle_id`, bumped per battle, so app-core can tell one battle's narration
from the next without comparing text.
