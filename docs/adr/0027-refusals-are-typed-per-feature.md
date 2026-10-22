# 0027 Refusals are typed per feature

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-22
- Pattern: Refusal enum
- Depends on: [0017](0017-game-is-the-entire-api.md), [0020](0020-functional-core-imperative-shell.md)

## Context

A `Game` action that can be turned down says why in one of two ways.
Eight features return a typed reason: `SortieRefusal`, `ContractRefusal`, `RouteRefusal`, `BreedRefusal`, `JailBlock`, `BuildBlock`, `SpendError`, `HelpBlock`.
app-core words them for the screen with an exhaustive `match` (`app/dispatch.rs` for `SortieRefusal`), so a new reason does not compile until someone writes its sentence.
About 167 other sites return `Result<_, String>`; the heaviest files are `game/crafting.rs`, `game/party.rs` and `game/base/building.rs`.

A string carries a wording chosen for one screen into every caller.
`Game::ability_unavailable` returns battle-row fragments ("no taming catalyst"); `adopt_orphan` reused them on `App::status_line`, where they read as nothing happening, and the player reported the key as unbound.
A string also cannot be tested by variant, only by substring.

## Decision

A new refusal is a per-feature enum, named `<Feature>Refusal`, returned as `Result<_, <Feature>Refusal>`.
A variant carries the data a sentence needs (`Unprovisioned { item, need, held }`), never the sentence.
app-core turns it into words with an exhaustive `match`, one per surface that shows it; the engine returns no player-facing wording for a refusal.
Variants stay distinct when they leave the player a different errand, `SortieRefusal`'s rule.

Existing `String` refusals change when a hand is already in that method, not as a migration.
In a decide step under [0020](0020-functional-core-imperative-shell.md), the refusal is part of the concrete `Outcome`.

## Alternatives considered

- Keep `String`. Simplest, and the cost above stays.
- One crate-wide `Refusal` enum. Grows every variant of every feature, the reason 0020 rules out a generic `Effect`.
- `thiserror` with `Display` on the engine type. Puts wording back in the engine, where only one surface's wording can live.
- Migrate all 167 sites now. Churn across most of `game/` with no bug to fix.

## Consequences

Each refusal is worded once per surface, and the compiler finds a missing wording.
Tests assert `Err(SortieRefusal::NoSquad)`, not a substring.
app-core gains one wording function per feature; both shapes coexist while the String sites remain.
The pattern index gains "Refusal enum".

## Evidence

- `crates/engine/src/game/sortie.rs` — `SortieRefusal` and its doc comment.
- `crates/engine/src/game/contracts.rs` — `ContractRefusal`.
- `crates/app-core/src/app/dispatch.rs` — the `SortieRefusal` match.
- `crates/engine/src/game/combat.rs` — `Game::ability_unavailable`.
- `crates/engine/src/game/crafting.rs` — `Game::craft`, `Game::equip` return `Result<_, String>`.
