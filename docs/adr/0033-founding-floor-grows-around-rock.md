# 0033 — Founding floor grows around rock

- Status: Agreed
- Decided by: user, 2026-10-30
- Date: 2026-10-30
- Pattern: —
- Bends: [0032](0032-base-returns-to-the-field.md)'s **Founding** bullet ("floors exactly one ring, the 8 cells around it")

## Context

0032 had founding lay the 3x3 around the Home and refuse outright when any
of its nine cells was rock or refused floor (`Game::floor_conflict`). Nine
cells is too small a start, and a refusal for one blocked corner pushed the
player off ground they had chosen.

## Decision

- Founding lays `tuning::FOUNDING_FLOOR_CELLS` (25) cells, grown from the
  Home by `footprint::founding_cells`: repeatedly the open cell four-way
  beside the patch nearest the Home in a straight line, ties on `(y, x)`.
- Open is walkable ground with no `floor_conflict`. Rock, Stack links,
  settlements and outposts are grown around, never refused.
- On open ground that is exactly the 5x5. Against rock the missing cells go
  to the nearest reachable open cells, so the patch bulges away from it.
- A short fill (a pocket with fewer than 25 open cells) lays what it found.
  Founding refuses only when the Home's own tile cannot take floor.
- Grown through the patch, so the floor is one piece that 0032's
  "new floor touches floor" growth can extend.

## Rejected

- **Refuse below a minimum.** Rejected: a sliver base is still a choice the
  player made, and they can mine outward.
- **Stay inside the 5x5.** Rejected: founding against rock would start
  smaller for no reason the player can see.
- **Square growth (Chebyshev order).** Rejected for rounded overflow; on
  open ground both give the 5x5.
