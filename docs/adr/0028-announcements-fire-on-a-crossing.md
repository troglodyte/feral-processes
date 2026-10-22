# 0028 Announcements fire on a crossing

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-22
- Pattern: Edge trigger
- Depends on: [0020](0020-functional-core-imperative-shell.md)

## Context

Several announcements must fire once, when a state is entered, not every tick it holds.
Each does it with its own latch and its own reload policy, argued in prose beside the field:
`DigSite::announced_stuck` and `Caravan::announced_stuck` (not saved, so a reload re-announces), `Needs::stalled_announced` (not saved), `Outpost::announced` (not saved, but re-seeded after load by `Game::reseed_outpost_announcements` so a reload stays quiet).
The comparison itself is written inline in the `Game` method (`if already == Some(trend) { return }` in `game/outposts.rs`; `if !caravan.announced_stuck` in `game/caravan.rs`), so it is tested only through a whole `Game`.
Two latches with opposite reload behaviour look the same at the field.

## Decision

Whether to announce is a pure function of the value before and the value after: `fn entered(before, after) -> Option<…>`, in the domain module, tested directly.
Where the apply step of [0020](0020-functional-core-imperative-shell.md) has both values in hand, it snapshots `before`, applies, calls the function, and stores no latch.
Where the state is entered gradually across ticks (a stall, a trend), the stored latch *is* `before`; it stays, and the same pure function compares it.
Every latch field states its reload policy in one word on its doc comment: `re-announces` or `re-seeded`, naming the re-seed function when it is the second.

Existing latches change when a hand is already in that method.

## Alternatives considered

- Leave each site as written. The reload policy stays a paragraph per field, and the comparison stays untested.
- Level-check every tick with a dedupe in the alert queue. Hides the transition in the queue, and a dedupe window is wall-clock-like state.
- Save every latch. Changes the save format and loses the deliberate re-announce after a reload.

## Consequences

The fire-once rule is a unit test per feature.
Some latches disappear where before and after meet in one call; the rest gain a stated policy.
No save-format change.
The pattern index gains "Edge trigger".

## Evidence

- `crates/engine/src/game/outposts.rs` — the `announced` comparison and `Game::reseed_outpost_announcements`.
- `crates/engine/src/game/caravan.rs` — `announced_stuck`.
- `crates/engine/src/components/needs.rs` — `Needs::stalled_announced`.
- `crates/engine/src/components/world.rs`, `components/structures.rs` — `announced_stuck` fields and their reload prose.
- `crates/engine/src/alerts.rs` — `AlertKind::OutpostStale`, "posted only on the transition".
