# 0005 Engine queries take `&self`

Status: Proposed (2026-10-16)

## Context

Of `Game`'s 541 public methods, 197 take `&mut self`.
Many are pure reads that need the mutable borrow only because `self.world.query::<…>()` requires `&mut World` (90 uses): `owned_pets` (`game/party.rs:707`), `view_entities` (`game/inspection.rs:1186`), `contract_board` and others.
The `&self` alternative, `World::try_query`, is already used in `game/base/floor.rs:273` and `game/base/farm.rs:332`.

The cost lands in the renderer.
67 gui draw functions take `game: &mut Game`, so `render::draw` holds `&mut App`.
About thirty blocks of the form `let x = match app.mode { Mode::X => game.something(), _ => default }` at `crates/gui/src/render/mod.rs:941-1183` exist only to copy data out before the borrow on `app.game` conflicts with the borrow on the rest of `App`.
Nothing in the type system says the renderer does not mutate the game; the rule is held by review.

## Decision

Convert every `Game` method that only reads to `&self`, using `try_query` or cached `QueryState` where a query is needed.
Then convert the gui draw signatures to `&Game` and `render::draw` to `&App`, with the per-frame mutations (`advance_*`, reveal, timers) moved ahead of the draw call in `frame`.

## Consequences

"The renderer is read-only" becomes a compiler check instead of a reviewer's.
The thirty copy blocks in `draw_mode_overlay` can go, which is most of the cost of ADR 0006's dispatch table.
`&self` reads are what ADR 0008's `fn(&World)` derivations are extracted from, so this lands first.
`QueryState` caching adds a field or two to `Game`; `try_query` is the cheaper path and should be tried first.
This touches two crates and is the one structural refactor with the most downstream unlocks.

## Evidence

- `crates/engine/src/game/party.rs:707`, `inspection.rs:1186` — reads that take `&mut self`.
- `crates/engine/src/game/base/floor.rs:273`, `farm.rs:332` — `try_query` precedent.
- `crates/gui/src/render/mod.rs:941-1183` — copy blocks.
