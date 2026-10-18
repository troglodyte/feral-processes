# 0007 Back-navigation is a stack

Status: Proposed (2026-10-16)

## Context

Where Esc returns to is held in ten separate `App` fields — `menu_origin`, `etch_return`, `memories_origin`, `command_return_row`, `hub_last` among them — plus twelve hand-listed `keeps_highlight` pairs (`crates/app-core/src/app/input.rs:66`), `base_popup_parent` (`lib.rs:2889`), and a run of "on entering this mode" checks at the end of `handle_key`.
Each new screen that can be reached from more than one place adds a field and a pair.

A `Mode { screen, popup }` split was considered and is a weaker fit: only three popups use `base_popup_parent`, while the origin fields are about return paths, not nesting.

## Decision

Replace the origin fields with one stack of `(Mode, cursor)` entries on `App`.
Entering a screen pushes the caller; Esc pops and restores the cursor; hubs are the stack's floor.
`keeps_highlight` becomes "the stack remembers the cursor", not a list.

## Consequences

Ten fields and two tables go, and a new screen's return path is whatever pushed it.
Depends on nothing but is easier after ADR 0006 moves per-screen cursors into per-screen structs, since the stack then stores the struct rather than a bare index.
Deep stacks need a cap or a hub reset so a player cannot build an unbounded history by cycling screens.

## Evidence

- `crates/app-core/src/app/input.rs:66` — `keeps_highlight`.
- `crates/app-core/src/lib.rs:2889` — `base_popup_parent`.
