# 0014 Per-screen state and one exhaustive screen table

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-21
- Pattern: State; table-driven dispatch

## Context

`Mode` (`crates/app-core/src/lib.rs`) is a plain id with about 158 variants, and the data a screen needs — `pending_craft`, `fuse_name_input`, `pending_plot` and their kin — sits on `App` beside it.
So a screen can be open without its data.
Handlers recover with "missing data, go back to the map", and the gui census in `render::tests` keeps `NEEDS_PENDING_STATE`, a hand list of 29 screens it cannot draw from a fresh `App`.

What a mode is, is spread over several lists: the census's `ALL_MODES`, `Mode::is_battle`, the Shift/Ctrl fold list, `needs_status_banner`, `hub_of`, and `render::draw_mode_overlay`'s match with its `_ => {}` arm.
A missed entry in some of these is silent.

Hubs already solved this for their own screens.
Since 2026-10-20 a hub is a `HubKind` impl in its own module, `App::handle_hub_nav` is the one nav rule, and gui names no hub.
`RigToolScreen` and `DepotFilterScreen` already keep their state in a per-screen struct.

Split out of [0006](0006-per-screen-state-and-a-per-run-struct.md), part (b).

## Decision

Each screen's state is a struct in its `app/<mode>.rs` module, held in one enum beside `Mode`; `Mode` stays a plain `Copy` id because `hub_of` and the `HubKind` tables key on it.
What a screen is lives in one exhaustive `fn screen(mode) -> Screen` table: what draws underneath, whether a refusal goes to the banner, whether it is a battle mode, which painter draws it.
It is the `HubKind` idea applied to screens, and a hub page is a screen whose row names its hub.
`Mode::ALL` is generated rather than hand-listed.

## Alternatives considered

- A `ScreenKind` trait object per mode, as `HubKind` is per hub. A hub has behaviour (turning pages, leaving); most screens differ only in data, so a table row is enough.
- Fold state into `Mode` variants (`Mode::Craft(CraftState)`). Loses `Copy`, which the hub tables and every `self.mode == Mode::X` comparison rely on.

## Consequences

A screen cannot be entered without its data, so the recovery handlers and `NEEDS_PENDING_STATE` go.
Adding a screen is one module and one table row, and a missed row is a compile error.
The six `*Direction` and four `*Quantity` modes can each collapse to one screen with a return target.
Screens migrate one at a time behind a `Legacy` arm.
[0007](0007-navigation-is-a-stack.md) builds on this.

## Evidence

- `crates/app-core/src/app/hub.rs` — `HubKind`, `hub_of`, `App::handle_hub_nav`.
- `crates/app-core/src/app/rig_tool.rs`, `app/depot_filter.rs` — the per-screen precedent.
- `crates/gui/src/render/mod.rs` — `draw_mode_overlay`; `ALL_MODES`, `NEEDS_PENDING_STATE` in its tests.
- `.claude/rules/seams-screens.md` — the hub rule.
