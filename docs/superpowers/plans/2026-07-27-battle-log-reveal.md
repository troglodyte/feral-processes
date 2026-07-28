# Paced Battle Narration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Battle narration scrolls into the log pane at a fixed, tunable pace instead of arriving as a wall of text; each battle's pane starts empty; only a battle's results follow the player onto the map.

**Architecture:** The engine tags result lines with a new `MessageKind::Outcome`, records a sequence mark where each battle's narration began, and prunes that range down to its results when the battle ends. app-core owns the pacing: a `BattleReveal` counter advanced by an injected `dt`, gating how many lines either pane may show, with any key press skipping to the end. The gui changes by three lines and no drawing code.

**Tech Stack:** Rust, standalone `bevy_ecs` (engine), Bevy + bevy_egui (gui). No new dependencies.

## Global Constraints

- `REVEAL_LINES_PER_SECOND: f32 = 12.0` lives in `crates/app-core/src/lib.rs`, **not** `crates/engine/src/tuning.rs` — CLAUDE.md scopes that file to difficulty, and reveal pace is presentation.
- Timing is **injected**, never read: the reveal takes `dt: f32`. No `Instant::now()`, no `sleep`, no wall-clock dependence in any test.
- `BattleReveal` is transient presentation state. It is **not persisted** — no save-format bump.
- The `Painter` seam holds: no file under `crates/gui/src/render/` may name a graphics library, and `crates/gui/src/paint.rs` is not touched.
- Engine code prefers `Result`/`?` over panics; `unwrap()`/`expect()` only in tests.
- Run `cargo fmt` and `cargo clippy --workspace --all-targets` after every task; fix warnings rather than silencing them.

## Deviations from the approved spec

Two corrections found while planning. Both are folded into the tasks below.

1. **`retain_outcomes_since` also keeps `MessageKind::Raid`.** The spec listed only `Outcome`/`Loot`/`LevelUp`. But `systems.rs:35,146,236` and `difficulty.rs:19` write to `MessageLog` directly via `ResMut<MessageLog>`, bypassing `Game::log_kind`, and they run during the `tick()` that battle actions trigger. A raid alert that lands mid-battle is world news, not battle narration, and must survive the prune.
2. **The battle mark is not cleared when the battle ends.** The spec had it living on `BattleState`, which `end_battle` removes (`combat_status.rs:483`). The mark instead lives on `MessageLog` and is replaced at the *next* `start_battle`, so the post-battle results reveal still has a range to slice.

## File Structure

| File | Responsibility |
|---|---|
| `crates/engine/src/resources.rs` | `MessageKind::Outcome`; `MessageLog` gains the sequence counter, battle mark, battle id, `since`, `retain_outcomes_since` |
| `crates/engine/src/game/turn.rs` | `Game::battle_log`, `Game::battle_log_id` accessors |
| `crates/engine/src/game/combat.rs` | `start_battle` opens a new narration range |
| `crates/engine/src/game/combat_status.rs` | `end_battle` prunes the range to results |
| `crates/engine/src/game/combat_rewards.rs`, `combat_round.rs` | tag result lines `Outcome` |
| `crates/app-core/src/lib.rs` | `REVEAL_LINES_PER_SECOND`; `BattleReveal`; `App` field |
| `crates/app-core/src/app/battle.rs` | reveal restart when a battle ends |
| `crates/app-core/src/app/input.rs` | `advance_reveal`; skip-on-keypress |
| `crates/gui/src/lib.rs` | one `advance_reveal` call |
| `crates/gui/src/render/battle.rs` | battle pane reads the revealed log; action bar suppressed while revealing |
| `crates/gui/src/render/base.rs` | base pane hides the unrevealed tail |

---

### Task 1: Tag result lines in the engine

**Files:**
- Modify: `crates/engine/src/resources.rs:32-42`
- Modify: `crates/engine/src/game/combat_round.rs:419`
- Modify: `crates/engine/src/game/combat_rewards.rs:134,216,247,265`
- Modify: `crates/engine/src/game/combat_status.rs:58,61,68`
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `MessageKind::Outcome` — the variant Tasks 2 and 5 filter on.

Result lines are currently a mix: drops are already `Loot` and level-ups already `LevelUp`, but the kill line, the XP line and the decompile outcomes are plain `Info`, so nothing in the code can pick them out.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/tests/combat_status.rs`, append to the existing `mod tests`:

```rust
#[test]
fn the_kill_line_and_xp_award_are_tagged_as_outcomes() {
    let mut game = support::battle_game();
    let player = game.player_entity();
    game.finish_member(0, 0, player);

    let tagged: Vec<String> = game
        .message_log(50)
        .into_iter()
        .filter(|(kind, _)| *kind == MessageKind::Outcome)
        .map(|(_, line)| line)
        .collect();

    assert!(
        tagged.iter().any(|l| l.contains("crashes and deletes itself")),
        "the kill line was not tagged an outcome: {tagged:?}"
    );
    assert!(
        tagged.iter().any(|l| l.contains("XP")),
        "the XP award was not tagged an outcome: {tagged:?}"
    );
}
```

If `support::battle_game()` does not already exist, use the same construction `crates/engine/src/tests/support.rs:159` uses to insert a `BattleState`, and add a helper named `battle_game` there returning that `Game`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine the_kill_line_and_xp_award_are_tagged_as_outcomes`
Expected: FAIL — `no variant named Outcome found for enum MessageKind`.

- [ ] **Step 3: Add the variant**

In `crates/engine/src/resources.rs`, inside `enum MessageKind`, after `Round`:

```rust
    /// A line that is a *result* of a battle rather than narration of it —
    /// the kill, the XP, the loot, the decompile verdict, the jack-out.
    /// `MessageLog::retain_outcomes_since` keeps exactly these when a
    /// battle ends, which is what stops the blow-by-blow following the
    /// player onto the map.
    Outcome,
```

- [ ] **Step 4: Tag the result lines**

`crates/engine/src/game/combat_round.rs:419` — change

```rust
        self.log("The rogue program crashes and deletes itself!");
```

to

```rust
        self.log_kind(
            MessageKind::Outcome,
            "The rogue program crashes and deletes itself!",
        );
```

`crates/engine/src/game/combat_rewards.rs:134` — change `self.log(format!("You gain {amount} XP."));` to

```rust
        self.log_kind(MessageKind::Outcome, format!("You gain {amount} XP."));
```

`crates/engine/src/game/combat_rewards.rs:216,247,265` — change each of

```rust
        self.log("No taming catalyst left — the decompile attempt fizzles.");
        self.log("The program's ICE holds — decompile failed!");
        self.log("ICE breached! The program now runs under your control.");
```

to the `self.log_kind(MessageKind::Outcome, …)` form with the same string.

`crates/engine/src/game/combat_status.rs:58,61,68` — the jack-out verdict is a result too. Change

```rust
            self.log("You jack out, but not before taking a parting counter-strike!");
            self.log("You jack out safely.");
                self.log(format!("Bailing out costs you {xp_lost} XP."));
```

to the `self.log_kind(MessageKind::Outcome, …)` form with the same strings.

Add `MessageKind` to the `use` list at the top of any of these files that does not already import it.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-engine the_kill_line_and_xp_award_are_tagged_as_outcomes`
Expected: PASS

Run: `cargo test -p feral-processes-engine`
Expected: PASS — no existing test asserts on these lines' `MessageKind`, so nothing should break. If one does, it was asserting `Info` on a line that is now `Outcome`; update the assertion.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets
git add crates/engine/src
git commit -m "feat: tag battle result lines with MessageKind::Outcome"
```

---

### Task 2: Battle-scoped log ranges and the end-of-battle prune

**Files:**
- Modify: `crates/engine/src/resources.rs:26-66` (`MessageLog`), `:131-141` (unchanged, read only)
- Modify: `crates/engine/src/game/turn.rs:20-30`
- Modify: `crates/engine/src/game/combat.rs:154-161`
- Modify: `crates/engine/src/game/combat_status.rs:467-484`
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces:**
- Consumes: `MessageKind::Outcome` (Task 1).
- Produces:
  - `Game::battle_log(&self) -> Vec<(MessageKind, String)>` — the current (or just-ended) battle's lines, oldest first.
  - `Game::battle_log_id(&self) -> u64` — increments on every `start_battle`; `0` before any battle. Task 3 uses this to detect a new battle.

The mark cannot be a raw index into `MessageLog::lines`: the log drains its oldest entries at `MESSAGE_LOG_CAP = 100` (`resources.rs:56`), so an index silently comes to point at the wrong line in any battle long enough to overflow it. A sequence counter of lines-ever-pushed does not drift.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/engine/src/tests/combat_status.rs`:

```rust
#[test]
fn the_battle_log_holds_only_the_current_battle() {
    let mut game = support::battle_game();
    game.log("mid-battle narration");

    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();
    assert!(
        battle.iter().any(|l| l == "mid-battle narration"),
        "the battle's own line is missing: {battle:?}"
    );
    assert!(
        !battle.iter().any(|l| l == "before the fight"),
        "a pre-battle line leaked into the battle log: {battle:?}"
    );
}

#[test]
fn the_mark_survives_a_log_that_overflows_its_cap() {
    let mut game = support::battle_game();
    // MESSAGE_LOG_CAP is 100; overflow it several times over so a raw
    // index into `lines` would be pointing at the wrong entry by now.
    for i in 0..350 {
        game.log(format!("line {i}"));
    }
    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();

    assert_eq!(
        battle.last().map(String::as_str),
        Some("line 349"),
        "the newest line is not last: {:?}",
        battle.last()
    );
    assert!(
        battle.len() <= 100,
        "the battle log outgrew the log it slices: {}",
        battle.len()
    );
}

#[test]
fn ending_a_battle_keeps_results_and_drops_narration() {
    let mut game = support::battle_game();
    let player = game.player_entity();
    game.log("A hostile swings and misses.");
    game.log_kind(MessageKind::Outcome, "You gain 12 XP.");
    game.log_kind(MessageKind::Raid, "A raid hits your base!");

    game.end_battle(player, None);

    let after: Vec<String> = game.message_log(100).into_iter().map(|(_, l)| l).collect();
    assert!(
        !after.iter().any(|l| l.contains("swings and misses")),
        "blow-by-blow survived the prune: {after:?}"
    );
    assert!(
        after.iter().any(|l| l.contains("You gain 12 XP")),
        "the result was pruned away: {after:?}"
    );
    assert!(
        after.iter().any(|l| l.contains("A raid hits your base")),
        "a raid alert is world news, not battle narration: {after:?}"
    );
}

#[test]
fn a_second_battle_starts_with_an_empty_pane() {
    let mut game = support::battle_game();
    let first = game.battle_log_id();
    game.log("first battle narration");
    let player = game.player_entity();
    game.end_battle(player, None);

    let pack = support::spawn_hostile_pack(&mut game);
    game.start_battle(pack);

    assert_ne!(first, game.battle_log_id(), "the battle id did not advance");
    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();
    assert!(
        !battle.iter().any(|l| l == "first battle narration"),
        "the previous battle's narration is still in the pane: {battle:?}"
    );
}
```

`support::spawn_hostile_pack` may not exist. If it does not, add it to `crates/engine/src/tests/support.rs`, spawning one hostile creature the same way `battle_game` does and returning `vec![entity]`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine battle_log`
Expected: FAIL — `no method named battle_log found for struct Game`.

- [ ] **Step 3: Extend `MessageLog`**

In `crates/engine/src/resources.rs`, replace the `MessageLog` struct and impl:

```rust
/// Where a battle's narration begins in the log, as a count of lines ever
/// pushed. Deliberately not an index into `lines`: the log drains its
/// oldest entries at `MESSAGE_LOG_CAP`, so an index would come to point at
/// the wrong line in any battle long enough to overflow it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessageMark(u64);

#[derive(Resource, Default)]
pub struct MessageLog {
    pub lines: Vec<(MessageKind, String)>,
    /// Lines ever pushed, including those since drained.
    pushed: u64,
    /// Where the current — or most recently ended — battle's narration
    /// begins. Not cleared when a battle ends: the frontend is still
    /// revealing that battle's results after the fact, and needs the range
    /// to slice. Replaced by the next `start_battle`.
    battle_start: MessageMark,
    /// Bumped by every `start_battle`, so a frontend can tell one battle's
    /// narration from the next without comparing text.
    pub battle_id: u64,
}

impl MessageLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.push_kind(MessageKind::Info, line);
    }

    pub fn push_kind(&mut self, kind: MessageKind, line: impl Into<String>) {
        self.lines.push((kind, line.into()));
        self.pushed += 1;
        if self.lines.len() > MESSAGE_LOG_CAP {
            let excess = self.lines.len() - MESSAGE_LOG_CAP;
            self.lines.drain(0..excess);
        }
    }

    pub fn recent(&self, n: usize) -> &[(MessageKind, String)] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }

    /// Opens a new narration range at the next line pushed.
    pub fn open_battle(&mut self) {
        self.battle_start = MessageMark(self.pushed);
        self.battle_id += 1;
    }

    /// Lines from the battle mark on, oldest first. Clamps rather than
    /// panicking when the mark has been drained past — everything still in
    /// the log is then younger than the mark, so all of it belongs.
    pub fn since_battle(&self) -> &[(MessageKind, String)] {
        let drained = self.pushed - self.lines.len() as u64;
        let start = self.battle_start.0.saturating_sub(drained) as usize;
        &self.lines[start.min(self.lines.len())..]
    }

    /// Drops the blow-by-blow from the battle range, keeping what the
    /// player should still be reading on the map: the battle's results, and
    /// any world news (`Raid`) that happened to land mid-fight — background
    /// systems write to this log directly, so a raid alert can.
    pub fn retain_outcomes_since_battle(&mut self) {
        let drained = self.pushed - self.lines.len() as u64;
        let start = self.battle_start.0.saturating_sub(drained) as usize;
        let start = start.min(self.lines.len());
        let mut kept = self.lines[..start].to_vec();
        kept.extend(self.lines[start..].iter().filter(|(kind, _)| {
            matches!(
                kind,
                MessageKind::Outcome | MessageKind::Loot | MessageKind::LevelUp | MessageKind::Raid
            )
        }).cloned());
        self.lines = kept;
    }
}
```

- [ ] **Step 4: Add the `Game` accessors**

In `crates/engine/src/game/turn.rs`, after `message_log` (line 30):

```rust
    /// This battle's narration, oldest first — what the battle pane shows,
    /// so each fight opens with an empty pane instead of the tail of the
    /// last one. After the battle ends this is the pruned result set, which
    /// is what a frontend mid-reveal is still scrolling in.
    pub fn battle_log(&self) -> Vec<(MessageKind, String)> {
        self.world.resource::<MessageLog>().since_battle().to_vec()
    }

    /// Changes every time a battle starts. A frontend pacing the narration
    /// restarts when this moves.
    pub fn battle_log_id(&self) -> u64 {
        self.world.resource::<MessageLog>().battle_id
    }
```

- [ ] **Step 5: Open the range at battle start and prune at battle end**

In `crates/engine/src/game/combat.rs`, in `start_battle`, immediately **before** `self.world.insert_resource(BattleState {` (line 154):

```rust
        // Before the intercept line below, so it is the first thing the
        // battle pane shows.
        self.world.resource_mut::<MessageLog>().open_battle();
```

In `crates/engine/src/game/combat_status.rs`, in `end_battle`, immediately before `self.world.remove_resource::<BattleState>();` (line 483):

```rust
        self.world
            .resource_mut::<MessageLog>()
            .retain_outcomes_since_battle();
```

Both files need `MessageLog` in scope; add it to the existing `use crate::resources::{…}` list.

`end_battle` is `pub(crate)`. The tests above call it and `start_battle` and `finish_member` from `crate::tests`, which is inside the crate, so no visibility change is needed.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-engine battle_log`
Expected: PASS (all four new tests)

Run: `cargo test -p feral-processes-engine`
Expected: PASS

- [ ] **Step 7: Run the balance gate**

Run: `cargo test -p feral-processes-engine balance_sim`
Expected: PASS, curves unmoved — no `.ron` and no `tuning.rs` value changed. A moved curve here means something in combat resolution changed and must be understood before continuing.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets
git add crates/engine/src
git commit -m "feat: scope the battle log to one battle and prune it to results"
```

---

### Task 3: The reveal counter in app-core

**Files:**
- Modify: `crates/app-core/src/lib.rs:114-119` (constants), `:375-469` (`App` struct)
- Modify: `crates/app-core/src/app/input.rs`
- Test: `crates/app-core/src/tests/`

**Interfaces:**
- Consumes: `Game::battle_log()`, `Game::battle_log_id()` (Task 2).
- Produces:
  - `App::advance_reveal(&mut self, dt: f32)`
  - `App::is_revealing(&self) -> bool`
  - `App::revealed_battle_log(&self) -> Vec<(MessageKind, String)>` — the battle pane's lines, oldest first, truncated to what has been revealed.
  - `App::hidden_log_lines(&self) -> usize` — how many lines the base pane must chop off its tail. Task 5 uses both.

- [ ] **Step 1: Write the failing tests**

Create `crates/app-core/src/tests/reveal.rs` (and add `mod reveal;` to `crates/app-core/src/tests/mod.rs`):

```rust
use super::support;
use crate::REVEAL_LINES_PER_SECOND;

/// The reveal must be driven by an injected delta, never a wall clock —
/// otherwise this test would need a sleep, and the suite forbids one.
#[test]
fn lines_are_released_in_proportion_to_the_elapsed_time() {
    let mut app = support::app_in_battle();
    app.restart_reveal();

    app.advance_reveal(0.0);
    assert_eq!(app.revealed_battle_log().len(), 0, "a zero dt released a line");

    // Exactly enough time for three lines.
    app.advance_reveal(3.0 / REVEAL_LINES_PER_SECOND);
    assert_eq!(app.revealed_battle_log().len(), 3);
}

/// A frame that covers less than a whole line must not lose the fraction:
/// two half-line frames make a line.
#[test]
fn the_fractional_carry_does_not_lose_a_line() {
    let mut app = support::app_in_battle();
    app.restart_reveal();

    let half = 0.5 / REVEAL_LINES_PER_SECOND;
    app.advance_reveal(half);
    assert_eq!(app.revealed_battle_log().len(), 0);
    app.advance_reveal(half);
    assert_eq!(app.revealed_battle_log().len(), 1, "the carry was dropped");
}

#[test]
fn the_reveal_stops_at_the_last_line_and_reports_done() {
    let mut app = support::app_in_battle();
    app.restart_reveal();
    let total = app.game.as_ref().unwrap().battle_log().len();

    app.advance_reveal(1_000.0);

    assert_eq!(app.revealed_battle_log().len(), total);
    assert!(!app.is_revealing(), "still revealing with every line out");
}

#[test]
fn a_new_battle_restarts_the_reveal() {
    let mut app = support::app_in_battle();
    app.advance_reveal(1_000.0);
    assert!(!app.is_revealing());

    support::start_second_battle(&mut app);
    app.advance_reveal(0.0);

    assert_eq!(
        app.revealed_battle_log().len(),
        0,
        "the new battle inherited the old one's revealed count"
    );
    assert!(app.is_revealing(), "a fresh battle has lines to reveal");
}
```

Add to `crates/app-core/src/tests/support.rs` (create the helpers if the module does not already have equivalents):

```rust
/// An `App` with a game already in a battle whose log holds several lines,
/// which is what the reveal paces.
pub fn app_in_battle() -> crate::App {
    let mut app = new_app();
    app.start_new_game(crate::DifficultyMode::Normal);
    let game = app.game.as_mut().unwrap();
    let pack = engine_support::spawn_hostile_pack(game);
    game.start_battle(pack);
    for i in 0..5 {
        game.log(format!("narration {i}"));
    }
    app.mode = crate::Mode::Battle;
    app
}

pub fn start_second_battle(app: &mut crate::App) {
    let game = app.game.as_mut().unwrap();
    let pack = engine_support::spawn_hostile_pack(game);
    game.start_battle(pack);
}
```

`Game::start_battle` and `Game::log` are `pub(crate)` to the engine, so app-core cannot call them. Make the two helpers instead drive a real battle through the public API — `app.handle_key` movement into a hostile — **or**, if that proves fragile to set up deterministically, add `#[cfg(feature = "test-support")]` public wrappers to the engine. Prefer the public-API route; only add wrappers if the movement setup cannot be made deterministic under a seeded RNG.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-app-core reveal`
Expected: FAIL — `no method named advance_reveal found for struct App`.

- [ ] **Step 3: Add the constant and the state**

In `crates/app-core/src/lib.rs`, after `REALTIME_TICK_INTERVAL` (line 119):

```rust
/// How fast battle narration scrolls into the log pane, in lines per
/// second. Presentation rather than difficulty, which is why it lives here
/// and not in the engine's `tuning.rs`.
pub const REVEAL_LINES_PER_SECOND: f32 = 12.0;

/// How much of the current battle's narration the player has been shown.
///
/// Transient presentation state, deliberately not saved: a loaded game
/// resumes with nothing pending.
#[derive(Default)]
struct BattleReveal {
    /// Lines released to the pane so far.
    revealed: usize,
    /// Sub-line carry, so a frame shorter than one line's worth of time
    /// isn't rounded away and lost.
    accumulated: f32,
    /// The `Game::battle_log_id` this count belongs to — when the engine's
    /// id moves on, a new battle has started and the count restarts.
    battle_id: u64,
}
```

Add the field to `struct App` (before `last_realtime_tick`, line 468):

```rust
    /// Paces battle narration into the log pane — see `App::advance_reveal`.
    reveal: BattleReveal,
```

Initialize it as `reveal: BattleReveal::default(),` wherever `App` is constructed (search `crates/app-core/src/app/lifecycle.rs` for the struct literal).

- [ ] **Step 4: Implement the reveal**

In `crates/app-core/src/app/input.rs`, inside `impl App`:

```rust
    /// Releases battle narration into the log pane at
    /// `REVEAL_LINES_PER_SECOND`, so a resolved round reads as it arrives
    /// rather than landing as a block. Called once a frame by the frontend
    /// with that frame's delta.
    ///
    /// Takes the delta rather than reading a clock: the suite forbids
    /// wall-clock dependence, and an injected `dt` is what makes the pacing
    /// testable without a sleep.
    pub fn advance_reveal(&mut self, dt: f32) {
        let Some(game) = &self.game else { return };
        let id = game.battle_log_id();
        let total = game.battle_log().len();
        if self.reveal.battle_id != id {
            self.reveal = BattleReveal {
                battle_id: id,
                ..BattleReveal::default()
            };
        }
        if self.reveal.revealed >= total {
            return;
        }
        self.reveal.accumulated += dt * REVEAL_LINES_PER_SECOND;
        while self.reveal.accumulated >= 1.0 && self.reveal.revealed < total {
            self.reveal.accumulated -= 1.0;
            self.reveal.revealed += 1;
        }
    }

    /// Whether narration is still scrolling in. While this holds, the
    /// frontend suppresses the action bar and `handle_key` skips instead of
    /// acting.
    pub fn is_revealing(&self) -> bool {
        let Some(game) = &self.game else {
            return false;
        };
        self.reveal.revealed < game.battle_log().len()
    }

    /// The battle pane's lines: this battle's narration, truncated to what
    /// has been revealed. The pane shows the tail of this when it overflows,
    /// which is what makes lines scroll up as they arrive.
    pub fn revealed_battle_log(&self) -> Vec<(MessageKind, String)> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let mut lines = game.battle_log();
        lines.truncate(self.reveal.revealed);
        lines
    }

    /// How many lines the *base* screen must chop off the tail of
    /// `Game::message_log` — the battle results that have not scrolled in
    /// yet. Zero except in the moments after a battle ends.
    pub fn hidden_log_lines(&self) -> usize {
        let Some(game) = &self.game else { return 0 };
        game.battle_log().len().saturating_sub(self.reveal.revealed)
    }

    /// Starts this battle's narration over from nothing.
    pub(crate) fn restart_reveal(&mut self) {
        let id = self.game.as_ref().map_or(0, |g| g.battle_log_id());
        self.reveal = BattleReveal {
            battle_id: id,
            ..BattleReveal::default()
        };
    }
```

The tests call `restart_reveal`, which is `pub(crate)` — the test module is inside the crate, so that is fine.

`MessageKind` must be importable in app-core. Confirm `feral_processes_engine` re-exports it (`crates/engine/src/lib.rs:54` re-exports the resources); if not, add it to that re-export list and to app-core's `use feral_processes_engine::{…}`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-app-core reveal`
Expected: PASS

Run: `cargo test -p feral-processes-app-core`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets
git add crates/app-core/src
git commit -m "feat: pace battle narration with an injected-delta reveal counter"
```

---

### Task 4: Skip on keypress, and restart the reveal for the results

**Files:**
- Modify: `crates/app-core/src/app/input.rs:44-100` (`handle_key`)
- Modify: `crates/app-core/src/app/battle.rs:107-121`, `:150-155`, `:379-385`
- Test: `crates/app-core/src/tests/reveal.rs`

**Interfaces:**
- Consumes: `App::is_revealing`, `App::advance_reveal`, `App::restart_reveal` (Task 3).
- Produces: nothing new. Behaviour only.

Two behaviours. A key pressed mid-reveal completes the reveal and is *not* acted on — that is the skip. And when a battle ends, the reveal restarts, so the results scroll into the base screen's pane instead of appearing whole.

- [ ] **Step 1: Write the failing tests**

Append to `crates/app-core/src/tests/reveal.rs`:

```rust
#[test]
fn a_key_pressed_mid_reveal_skips_instead_of_acting() {
    let mut app = support::app_in_battle();
    app.restart_reveal();
    app.advance_reveal(1.0 / REVEAL_LINES_PER_SECOND);
    let mode_before = app.mode;
    let revealed_before = app.revealed_battle_log().len();
    assert!(app.is_revealing(), "test needs an unfinished reveal");

    app.handle_key(crate::GameKey::Esc);

    assert!(!app.is_revealing(), "the key did not finish the reveal");
    assert_eq!(
        app.mode, mode_before,
        "the skip key was also acted on — Esc changed the mode"
    );
    assert!(app.revealed_battle_log().len() > revealed_before);
}

#[test]
fn a_key_pressed_after_the_reveal_acts_normally() {
    let mut app = support::app_in_battle();
    app.advance_reveal(1_000.0);
    assert!(!app.is_revealing());

    app.handle_key(crate::GameKey::Esc);

    assert_ne!(
        app.mode,
        crate::Mode::Battle,
        "Esc did nothing once the reveal was done"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-app-core reveal`
Expected: FAIL — `a_key_pressed_mid_reveal_skips_instead_of_acting` fails on "the key did not finish the reveal".

- [ ] **Step 3: Add the skip to `handle_key`**

In `crates/app-core/src/app/input.rs`, as the very first statement of `handle_key` (before `let mode_before = self.mode;`):

```rust
        // A key pressed while narration is still scrolling in dumps the
        // rest and is not acted on. Without this the pacing would be a tax
        // on anyone who reads faster than it scrolls.
        if self.is_revealing() {
            self.finish_reveal();
            return;
        }
```

And alongside `restart_reveal`:

```rust
    /// Releases every remaining line at once — the skip.
    pub(crate) fn finish_reveal(&mut self) {
        let total = self.game.as_ref().map_or(0, |g| g.battle_log().len());
        self.reveal.revealed = total;
        self.reveal.accumulated = 0.0;
    }
```

- [ ] **Step 4: Restart the reveal when a battle ends**

The `still_active` tail is repeated at three sites. Add one helper to `crates/app-core/src/app/battle.rs`, inside `impl App`:

```rust
    /// The shared tail of every action that can end a battle. A battle that
    /// just ended has had its log pruned to results, so the reveal restarts
    /// and those results scroll into the base screen's pane rather than
    /// appearing whole.
    fn settle_after_round(&mut self, still_active: bool) {
        self.mode = if still_active {
            Mode::Battle
        } else {
            Mode::Playing
        };
        if !still_active {
            self.restart_reveal();
        }
    }
```

Replace the three tails with a call to it:

`battle.rs:379-384` in `commit_battle_action` — replace

```rust
        let still_active = game.has_active_battle();
        self.mode = if still_active {
            Mode::Battle
        } else {
            Mode::Playing
        };
```

with

```rust
        let still_active = game.has_active_battle();
        self.settle_after_round(still_active);
```

`battle.rs:150-155` in `plan_every_slot` — the identical block, replaced the same way.

`battle.rs:108-111` in `run_party_command`'s `JackOut` arm — replace

```rust
                let still_active = game.has_active_battle();
                if !still_active {
                    self.mode = Mode::Playing;
                }
```

with

```rust
                let still_active = game.has_active_battle();
                if !still_active {
                    self.settle_after_round(still_active);
                }
```

A `&mut self` method cannot be called while `game` is borrowed. At each site the `game` borrow ends at the last use; if the borrow checker objects, move the `let still_active = game.has_active_battle();` line to just before the call and drop the `game` binding, rather than reaching for `.clone()`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-app-core reveal`
Expected: PASS

Run: `cargo test -p feral-processes-app-core`
Expected: PASS. `crates/app-core/src/tests/` has existing battle tests that drive `handle_key` through a whole fight; several will now fail, because the first key of each round is swallowed as a skip. That is the feature working. Fix each by calling `app.advance_reveal(1_000.0)` after any action that resolves a round — do **not** weaken the skip to make them pass.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets
git add crates/app-core/src
git commit -m "feat: skip the reveal on keypress and restart it for battle results"
```

---

### Task 5: Wire the frontend

**Files:**
- Modify: `crates/gui/src/lib.rs` (`frame`)
- Modify: `crates/gui/src/render/battle.rs:150-155`, `:263-271`, `:336-356`
- Modify: `crates/gui/src/render/base.rs:162-168`
- Test: `crates/gui/src/render/battle.rs` (test module)

**Interfaces:**
- Consumes: `App::advance_reveal`, `App::is_revealing`, `App::revealed_battle_log`, `App::hidden_log_lines` (Tasks 3-4).
- Produces: nothing.

- [ ] **Step 1: Feed the reveal a delta**

In `crates/gui/src/lib.rs`, in `frame`, immediately after `fe.app.update_realtime();`:

```rust
    fe.app.advance_reveal(input.time.delta_secs());
```

- [ ] **Step 2: Draw the battle pane from the revealed log**

In `crates/gui/src/render/battle.rs`, `draw_battle` binds `let Some(game) = &mut app.game` at line 151 and holds that borrow for the whole function, so the two `app` reads must be taken *before* it. Insert as the first two statements of `draw_battle`, above line 151:

```rust
    let revealed = app.revealed_battle_log();
    let revealing = app.is_revealing();
```

Then replace the log loop at line 265:

```rust
    for (kind, line) in game.message_log(capacity) {
```

with a tail slice of the revealed lines — the tail, so lines scroll up out of the pane as new ones arrive:

```rust
    for (kind, line) in revealed.iter().skip(revealed.len().saturating_sub(capacity)) {
```

and change the loop body's call to `draw_message_line(*kind, line, …)` to match the now-borrowed items.

- [ ] **Step 3: Suppress the action bar while revealing**

In the same function, wrap the action-bar block (lines 336-356, from `let mut actions: Vec<String> = view` through the `painter.ui(actions.join("   "), …)` call) in:

```rust
    if !revealing {
        // … existing action-bar block unchanged …
    }
```

`fx.draw_floats(painter, m);` stays outside the `if` — the damage numbers keep animating while narration scrolls.

- [ ] **Step 4: Hide the unrevealed tail on the base screen**

In `crates/gui/src/render/base.rs`, the pane at line 163 reads `game.message_log(capacity)`. The battle's results are at the tail of that log and must appear one at a time. Take the hidden count before the `game` borrow, in whichever function owns the `app` reference and passes `game` down, then replace the loop:

```rust
    let lines = game.message_log(capacity + hidden);
    let shown = lines.len().saturating_sub(hidden);
    for (kind, line) in lines.iter().take(shown).skip(shown.saturating_sub(capacity)) {
```

If `draw_base`'s signature does not carry `app`, thread `hidden: usize` in as a parameter from `render/mod.rs:138`'s `draw`, which does have `app`.

- [ ] **Step 5: Add a regression test for the pane**

Append to the test module in `crates/gui/src/render/battle.rs`:

```rust
/// The pane must show the *tail* of the revealed lines, not the head:
/// once a battle's narration outgrows the pane, new lines have to push old
/// ones up and out, which is the "scroll into view" this feature is for.
#[test]
fn the_pane_shows_the_newest_revealed_lines() {
    let revealed: Vec<String> = (0..10).map(|i| format!("line {i}")).collect();
    let capacity = 3;

    let shown: Vec<&String> = revealed
        .iter()
        .skip(revealed.len().saturating_sub(capacity))
        .collect();

    assert_eq!(shown, vec!["line 7", "line 8", "line 9"]);
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-gui`
Expected: PASS

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 7: Run the game and watch it**

Run: `cargo run -p feral-processes`

Walk into a hostile and resolve a round. Confirm by eye: narration scrolls in rather than landing whole; the action bar is absent while it scrolls and returns when it stops; a key press dumps the rest immediately; the next battle opens with an empty pane; and after a win the map's log pane shows the rewards and not the blow-by-blow.

`REVEAL_LINES_PER_SECOND = 12.0` is a guess. If it reads too fast or too slow, change that one constant.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets
git add crates/gui/src
git commit -m "feat: draw the battle pane from the paced reveal"
```

---

## Self-review

**Spec coverage.** Paced reveal — Task 3. Tunable constant — Task 3, Step 3. Blocks with a skip key — Task 4 (skip) and Task 5, Step 3 (action bar suppressed). Pane resets each battle — Task 2 (`open_battle`) and Task 3 (`battle_id` restart). Results-only handoff — Tasks 1 and 2 (tag and prune) and Task 5, Step 4. Testing section — covered across Tasks 1-5; the `MESSAGE_LOG_CAP` overflow case is Task 2, Step 1.

**Spec items deliberately dropped**, both from the final-round decision taken after the spec was written and approved: the deferred `Mode::Playing` transition, and the results-screen layout. The battle ends and the map takes over immediately, as it does today. The consequence, accepted knowingly: the killing round's blow-by-blow is pruned and never read.

**Naming consistency.** `open_battle` / `since_battle` / `retain_outcomes_since_battle` on `MessageLog`; `battle_log` / `battle_log_id` on `Game`; `advance_reveal` / `is_revealing` / `revealed_battle_log` / `hidden_log_lines` / `restart_reveal` / `finish_reveal` on `App`. Each is used under the same name in every task that consumes it.

**Known soft spots**, flagged rather than papered over:
- Task 3's test helpers need `Game::start_battle` and `Game::log`, both `pub(crate)` to the engine. The step names the preferred route (drive a real battle through the public API) and the fallback, but whoever implements it will have to make that call.
- Task 5, Step 4 depends on `draw_base`'s signature, which was not read in full while planning. The step says to thread `hidden` from `render/mod.rs:138` if `app` is not already in scope.
