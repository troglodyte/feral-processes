# 0034 Google's Rust style guide is the reference, with listed exceptions

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-30
- Pattern: —
- Depends on: [0017](0017-game-is-the-entire-api.md), [0024](0024-code-is-organized-by-domain.md)

## Context

The workspace has house rules in `CLAUDE.md` but no reference for line-level Rust style, so reviews argue taste case by case.
[Google's Rust style guide](https://google.github.io/styleguide/rustguide.html) is a published candidate.
About half of it is for Chromium, Fuchsia or Android, or for library APIs used across teams. It relaxes those rules for application code itself.
Much of the rest is already house style: rustfmt and clippy as the gate, comments that say what the code doesn't, calling functions through their module (`battle::attackers_in_group`), inherent impls over organising traits, and almost no `Rc`/`Arc` or `unsafe`.
The rules that would change code have a large backlog: about 1,270 glob imports, about 1,680 `unwrap`/`expect` calls outside test directories, and about 2,300 `assert!` calls with no message.

## Decision

The guide is the reference for style questions `CLAUDE.md` and the ADRs don't answer. Where they disagree, the house rule wins.
It applies to code a change already touches, never as a sweep, the same as the rule for splitting big files.
The rules worth gaining most: `expect("why")` over `unwrap` outside tests; a message on `assert!` when the condition alone won't explain a failure; `if let`/`let … else` instead of checking `is_some()` and then calling `unwrap()`; an enum instead of a `bool` parameter whose meaning isn't obvious at the call site; `#[must_use]` on pure functions whose result is easy to drop by mistake.

Excluded:

- Everything specific to Chromium, Fuchsia or Android: small crates, a flat file layout, `chromium::import!`, gtest, no `async`, `fuchsia_sync`.
- Rules for cross-team library APIs: `#[non_exhaustive]`, `missing_docs`, `impl Into` parameters, returning iterators instead of `Vec`.
- One module per `use` statement (`imports_granularity = Module`): the option only exists on nightly rustfmt, and the workspace builds on stable.
- No APIs in non-leaf modules: `Game` lives in the engine's `lib.rs` by [0017](0017-game-is-the-entire-api.md).
- No glob imports, for test modules, test support files and the `impl Game` files under `game/`. New non-test code outside `game/` imports by name.

## Alternatives considered

- Adopt the whole guide and sweep the workspace. Churn across hundreds of files with no bug to fix, and the import rule can't be enforced on stable.
- Copy four or five rules into `CLAUDE.md` and leave the guide out. Cheaper on every turn, but every style question not on the list starts from nothing again.
- Don't adopt it. Current rules cover the structural choices; line-level style stays unwritten.

## Consequences

Reviews can cite the guide by section instead of arguing taste.
`CLAUDE.md` gains one line under Rust idioms that links here; the exceptions live only in this record.
Both styles sit side by side for a long time, since nothing is swept.
No lint config changes: the gained rules need judgment (`clippy::unwrap_used` would flag every test).

## Evidence

- `crates/engine/src/lib.rs` — `Game` in a non-leaf module.
- `crates/engine/src/tests/`, `crates/app-core/src/tests/` — most `use crate::*` and `use super::*` imports.
- `crates/engine/src/game/` — `impl Game` split files that import `crate::*`.
- Root `Cargo.toml` — no `[workspace.lints]`; no `rustfmt.toml` in the workspace.
