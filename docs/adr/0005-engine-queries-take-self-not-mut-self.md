# 0005 Engine queries take `&self`

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-16 (evidence refreshed 2026-10-21)
- Pattern: Command-query separation

## Context

Many `Game` methods are pure reads that take `&mut self` only because `self.world.query::<…>()` needs `&mut World`.
`Game::owned_pets`, `Game::view_entities` and `Game::contract_board` are examples.
The `&self` alternative, `World::try_query`, is already used by the floor and farm reads in `game/base/floor.rs` and `game/base/farm.rs`; on 2026-10-21 there were about 80 `self.world.query` sites against 3 `try_query`.

The cost lands in the renderer.
About 70 gui draw functions take `game: &mut Game`, so `render::draw` holds `&mut App`.
`render::draw_mode_overlay` opens with blocks of the form `let x = match app.mode { Mode::X => game.something(), _ => default }` that exist only to copy data out before borrows conflict.
Nothing in the type system says the renderer does not mutate the game; review holds the rule.

## Decision

Convert every `Game` method that only reads to `&self`, using `try_query` or a cached `QueryState` where a query is needed.
Then convert the gui draw signatures to `&Game` and `render::draw` to `&App`, with the per-frame mutations (`advance_*`, reveal, timers) moved ahead of the draw call.

## Alternatives considered

- Keep `&mut` and rely on review. The status quo; the copy blocks keep growing with each screen.
- Snapshot a view struct per frame for the renderer. A second API beside `Game`, which [0017](0017-game-is-the-entire-api.md) rules out.

## Consequences

"The renderer is read-only" becomes a compiler check.
The copy blocks in `draw_mode_overlay` can go.
`&self` reads are what [0008](0008-shared-derivations-as-world-functions.md)'s derivations are extracted from, so this lands first.
`QueryState` caching adds a field or two to `Game`; `try_query` is cheaper and is tried first.
Two crates are touched.

## Evidence

- `Game::owned_pets` (`game/party.rs`), `Game::view_entities` (`game/inspection.rs`), `Game::contract_board` (`game/contracts.rs`).
- `World::try_query` in `game/base/floor.rs`, `game/base/farm.rs`.
- `render::draw`, `render::draw_mode_overlay` in `crates/gui/src/render/mod.rs`.
