# 0007 Back-navigation is a stack

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-10-16 (reworded 2026-10-21)
- Pattern: Navigation stack
- Depends on: [0014](0014-per-screen-state-and-one-screen-table.md)

## Context

Inside a hub, navigation already has one rule: `App::handle_hub_nav` walks the rail, `HubKind::leave` says where `Esc` on the rail goes, and `hub_last` remembers the page.
The three `Hub` variants (base, inventory, structure) are the only screens with a shared return path.

Outside the hubs, where `Esc` returns to is held in separate `App` fields — `menu_origin`, `etch_return`, `memories_origin`, `command_return_row` among them — plus the hand-listed pairs in `keeps_highlight` (`app/input.rs`), the `base_popup_parent` table, and a run of "on entering this mode" checks at the end of `handle_key`.
Each new screen reachable from more than one place adds a field and a pair.

## Decision

Replace the origin fields with one stack of entries on `App`, each a screen and its per-screen state from [0014](0014-per-screen-state-and-one-screen-table.md).
Entering a screen pushes the caller; `Esc` pops and restores it.
The hubs are the stack's floor: opening a hub through `App::open_hub` resets the stack to that hub, and `HubKind::leave` stays the rule for leaving it.
`keeps_highlight` becomes "the stack remembers the cursor", not a list.

## Alternatives considered

- `Mode { screen, popup }`. Only a few popups use `base_popup_parent`; the origin fields are about return paths, not nesting.
- Make every screen a hub page. Hubs are rails of sibling pages; a confirm popup or a quantity prompt is not a sibling.
- A stack of `(Mode, cursor)` before 0014. Works, but stores a bare index that 0014 then has to migrate.

## Consequences

The origin fields and two tables go, and a new screen's return path is whatever pushed it.
A hub reset caps depth, so cycling screens cannot build an unbounded history.
`handle_hub_nav` and the `HubKind` impls are unchanged.

## Evidence

- `crates/app-core/src/app/hub.rs` — `HubKind`, `App::handle_hub_nav`, `App::open_hub`, `Hub`.
- `crates/app-core/src/app/input.rs` — `keeps_highlight`.
- `crates/app-core/src/lib.rs` — `base_popup_parent`, `menu_origin`, `etch_return`, `memories_origin`.
- `.claude/rules/seams-screens.md` — the hub nav rule.
