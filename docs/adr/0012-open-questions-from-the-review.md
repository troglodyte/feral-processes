# 0012 Open questions from the review

- Status: Closed (answered by the user 2026-10-21)
- Decided by: user
- Date: 2026-10-16
- Pattern: —

Three questions the 2026-10-16 review could not settle without the user.
This record predates the rule that question lists get no number; it is kept so links hold.

## Roster views show raw `atk`

`PetInfo` (filled by `Game::owned_pets`) and `CompanionInfo` (`Game::companion_info`) take `atk` from `stats.atk`.
`PlayerStatus` and `PartySlotView` (`party_row`) take it from `Game::effective_atk`, which folds perks and gear.
In `game/combat_round.rs`, `enemy_row` fills `EnemyGroupView::atk` from `stats.atk` too.

**Answer:** effective atk everywhere.
The raw `stats.atk` reads are a bug, tracked in [backlog.md](backlog.md).

## Does the executor difference explain the suite-only flakes?

**Answer:** folded into [0001](0001-engine-test-executor-matches-the-gate.md), which settles it by reading each flaky test's code path, not by repeated runs.

## Should unknown fields in asset RON be rejected?

**Answer:** yes, for shipped asset kinds — [0016](0016-shipped-assets-deny-unknown-fields.md).
