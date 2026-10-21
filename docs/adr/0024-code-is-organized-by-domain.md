# 0024 Code is organized by domain

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: 2026-10-19 (077f90ae), hubs 2026-10-20 (a9e56289); recorded here 2026-10-21
- Pattern: Module per domain; Strategy

## Context

Hub files — one `tuning.rs`, one `components.rs`, one app-core `lib.rs` — collect every subsystem's pieces and become the file every change edits.
Behaviour that differs per variant, written as a `match` in each consumer, has to be found and extended in each place.

## Decision

A new engine subsystem is a directory, `crates/engine/src/<domain>/`, holding its types, rules, views and tests; `tactical/` and `settlements/` are the model.
Its constants go in `tuning/<domain>.rs`.
`tuning/`, `components/` and `views/` are split by domain and re-exported from `mod.rs`, so `tuning::X` paths never change; never grow `mod.rs`.
app-core has one module per mode in `app/<mode>.rs`; new mode logic never lands in `lib.rs`.
A hub is a `HubKind` impl in its own module: what a hub is lives there, never in a `match hub`, and gui names no hub.

## Alternatives considered

- Nested `tuning` modules (`tuning::base::X`). Rejected in [0011](0011-restructurings-considered-and-rejected.md); churn at every call site.
- Split hub files as a standalone project. Moves happen when a change is already in the file.
- A `match hub` per consumer. What the `HubKind` impls replaced.

## Consequences

The `tuning`, `components` and `views` split landed in 0.19.16.
app-core `lib.rs` and gui `render/mod.rs` are not there yet; they shrink as screens are changed ([0014](0014-per-screen-state-and-one-screen-table.md)).

## Evidence

- `crates/engine/src/tactical/`, `crates/engine/src/settlements/`.
- `crates/engine/src/tuning/mod.rs` — re-exports.
- `crates/app-core/src/app/hub.rs` — `HubKind`, `Hub::kind`.
- `.claude/rules/seams-screens.md` — the hub rule.
