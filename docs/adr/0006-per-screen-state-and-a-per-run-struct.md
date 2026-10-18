# 0006 Per-screen state structs and a per-run struct on `App`

Status: Proposed (2026-10-16)

## Context

`App` (`crates/app-core/src/lib.rs:2897`) has 160 fields and `Mode` (`lib.rs:1484`) has 157 variants.
About seventy fields — `pending_craft`, `fuse_name_input`, `pending_plot` and their kin — matter to one or two modes each, and `Mode` is a plain id, so a screen can be open without its data.
The code pays three ways.
26 handlers recover with "missing data, go back to the map" (`app/trade.rs:327` is one).
The gui keeps `NEEDS_PENDING_STATE`, a hand-written list of 29 screens it cannot draw without hand-set fields (`crates/gui/src/render/mod.rs:1936`).
And `draw_mode_overlay` opens with the copy blocks ADR 0005 describes.

Nothing is cleared between runs.
`leave_run` (`app/lifecycle.rs:412`) drops `game` and the status line and nothing else; `install_game` resets three fields.
`watching: Option<Entity>` is set once in the constructor (`app/lifecycle.rs:75`) and never cleared, so after watching a worker, quitting and loading another save, the camera can follow whatever entity now holds that id.
There are 257 `let Some(game) = &self.game` unwraps that exist because the run is optional on the same struct as the session.

A new screen costs six to eight edits across four or more files: `ALL_MODES: [Mode; 157]` with its literal length, the exhaustive `is_battle`, the Shift/Ctrl fold list (silent if missed), `needs_status_banner`, `hub_of`, and the overlay match whose second arm ends in `_ => {}` (`render/mod.rs:1635`).
Outside tests there are 37 `match …mode` sites, 31 of them in `render/mod.rs`, and 347 `self.mode = Mode::…` assignments.

`RigToolScreen` and `DepotFilterScreen` already keep their state in a per-screen struct and are the model.

## Decision

Split `App` into session state and a `Run` struct dropped with the run, so per-run handles cannot outlive it and the unwraps become one `Option<Run>`.
Give each screen its own state struct in its `app/*.rs` module, held in one enum beside `Mode`; `Mode` stays a plain id because the hub tables and `ALL_MODES` need a cheap `Copy` key.
Replace the per-mode flag lists with one exhaustive `fn screen(mode) -> Screen` table carrying what each mode needs: what draws underneath, whether a refusal goes to the banner, whether it is a battle mode, and which painter to call.
Generate `Mode::ALL` rather than hand-listing it.

The six `*Direction` and four `*Quantity` modes can each collapse to one screen with a return target once state is per-screen.

## Consequences

A screen cannot be entered without its data, so the 26 recovery handlers and `NEEDS_PENDING_STATE` go.
The stale-`Entity` carry-over is fixed by construction.
Adding a screen becomes one module plus one table row, and a missed row is a compile error instead of a silent `_ => {}`.
Tests set `app.mode` directly 142 times and write per-screen fields 195 times; moving those onto open-by-label helpers first makes the migration cheaper and is worth doing on its own.
Screens can migrate one at a time; the enum can hold `Legacy` until the last one moves.

## Evidence

- `crates/app-core/src/lib.rs:1484` — `Mode`; `:2897` — `App`.
- `crates/app-core/src/app/lifecycle.rs:75`, `:412` — `watching` init, `leave_run`.
- `crates/app-core/src/app/trade.rs:327` — recovery handler.
- `crates/gui/src/render/mod.rs:1635`, `:1936` — fallthrough, `NEEDS_PENDING_STATE`.
