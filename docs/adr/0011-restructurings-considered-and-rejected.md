# 0011 Restructurings considered and rejected

- Status: Accepted (2026-10-16; 0008 part 2 added 2026-10-21)
- Decided by: user, on Claude's proposal
- Date: 2026-10-16
- Pattern: —

These were examined in the 2026-10-16 review and found not worth doing, or not possible under the standing rules.
They are recorded so the next review does not start from zero; a rejection can be reopened by a later ADR that says what changed.

## Sub-facades on `Game`

`game.base()`, `game.battle()` and the like were considered as a way to cut the 541-method surface.
app-core calls 371 distinct `Game` method names; grouping them behind sub-structs renames them and removes no coupling.
The real reductions are [0005](0005-engine-queries-take-self-not-mut-self.md) (reads take `&self`) and the `pub(crate)` narrowing in [backlog.md](backlog.md).

## Engine tests as `tests/*.rs` integration binaries

The engine's `src/tests/` is about 180k lines compiled into one test target.
Moving it to `tests/` would make it an external crate, and the tests reach into the `World` 8,327 times (`Stats` read 736 times, `Position` 565).
That would need the `world()` accessor the architecture forbids.
Link cost is a build-setting question instead (see [backlog.md](backlog.md)).

## A content sub-crate

Splitting the `*Db` types and `tuning.rs` out of the engine was considered for compile time.
`tuning.rs:26` imports components and `abilities.rs` references `Game` and view types, so the split is not cheap and [0004](0004-content-loads-once-as-a-value.md) gets the testing benefit without it.

## Nested `tuning` modules

413 distinct constants are referenced, 46 from the frontends; nesting them by subsystem is churn at every call site for no reduction in the number a reader has to know.
The directory split that landed in 0.19.16 kept the flat namespace.

## gui sees only app-core

87 gui files import about 41 engine modules, all value types, enums and ids.
Routing them through app-core re-exports would be cosmetic; `Game.world` being private is what keeps the renderer honest, and that holds.

## `Painter` as a trait

There is one backend.
A trait with one implementation is an abstraction with no second caller to shape it; a test-only recording painter can be added if a test ever needs one.

## Separate telemetry crate

`serde_json` is in app-core for `app/telemetry.rs` alone.
One file does not justify a crate.

## Shared test-fixture crate

The engine's and app-core's `tests/support.rs` share five function names at different layers (one drives the World, the other drives `handle_key`).
They are not duplicates.

## Continuous integration

None, deliberately; `docs/releasing.md` explains the per-platform manual release.

## Bench, arena and `balance_sim` out of the engine

`bench/mod.rs:68` reads `game.world`, so bench and arena cannot leave the crate without the forbidden accessor.
`balance_sim` is runtime code called from `spawning.rs:1941`, `level_up.rs:133` and `gear_power.rs:56`.

## Hand-ordered tick passes move into the ECS schedule

The second half of the original [0008](0008-shared-derivations-as-world-functions.md): once a derivation is a `fn(&World, …)`, move each `tick_inner` pass that needed only that derivation into the schedule as an ordered system set.
Rejected on 2026-10-21.
It contradicts [0021](0021-plain-rust-over-ecs-for-discrete-logic.md): new discrete logic is plain Rust, and ECS is for many entities ticking.
It also breaks standing seam rules that make specific passes `Game` methods on purpose, such as `run_repair_bays` (`.claude/rules/seams-base.md`).
