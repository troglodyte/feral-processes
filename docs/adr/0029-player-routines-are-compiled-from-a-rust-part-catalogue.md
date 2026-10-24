# 0029 Player routines are compiled from a Rust part catalogue

- Status: Agreed
- Decided by: the user, 2026-10-24
- Date: 2026-10-24
- Pattern: Builder (parts compiled into a plain value)
- Depends on: [0019](0019-content-defaults-to-rust.md), [0023](0023-saves-are-field-named-ron.md), [0017](0017-game-is-the-entire-api.md)

## Context

Every routine the player can run is an authored file in `assets/abilities/`, reached through the routine research tree (`routine_tree::synthesise_nodes`), etched onto a disk (`Game::etch_disk`) and installed (`Game::install_disk`).
An installed routine is only an id: `components::Routines(Vec<AbilityId>)`, resolved through `AbilityDb::get` at about 43 sites and cloned out by `Game::actor_abilities`.
No per-instance ability exists; the only runtime-built defs are `AbilityDef::charged` and `species::basic_attack_ability`.

The design in `docs/superpowers/specs/2026-10-24-routine-crafting-design.md` replaces the player's ready-made routines with routines built from parts (Subroutines): one Core plus modifiers.
Wild programs, species kits and companion talents keep their authored routines.
That needs two decisions: where the part catalogue lives, and how a built routine reaches the 43 lookup sites.

## Decision

**The part catalogue is Rust.** A `Subroutine` enum (Cores with a version, and modifiers) and its numbers live in a new `crates/engine/src/routine_craft/` subsystem with constants in `tuning/routine_craft.rs`.
No new `assets/` kind and no new `.ron` fields.
Part items are synthesised into `ItemDb` at load from the enum, the way `ItemDb::synthesise_etched_disks` derives etched disks today.

**A built routine is a recipe, saved; its `AbilityDef` is derived.**
A `CraftedRoutine { id, name, parts }` is what the save holds.
A pure `routine_craft::compile(&CraftedRoutine) -> AbilityDef` produces the def, and it is the only place that turns parts into numbers.
Compiled defs are registered in `AbilityDb` under ids of the form `crafted/<n>` through one door, `AbilityDb::set_crafted`, called whenever the crafted set changes and on load.
`AbilityDb::get` answers for both authored and crafted ids, so the existing lookup sites, slots (`Routines`), battle resolution and the save's routine id lists work unchanged.

**Heal gains an in-battle over-time form**, built only by `compile`: no authored file uses or gains a field for it.

**Extraction runs the other way.** A pure `routine_craft::decompose(&AbilityDef) -> Vec<Subroutine>` maps an authored routine to the parts it hands out when extracted.

## Alternatives considered

- Parts as a new `assets/subroutines/` kind. Fails 0019: a few dozen entries, tuned together as a curve, read by no one but this subsystem.
- A separate `CraftedRoutines` resource consulted beside `AbilityDb`. Every one of the ~43 lookup sites would need a second branch, and a site that forgot it would silently drop a crafted routine from a menu or a fight.
- Saving the compiled `AbilityDef` instead of the recipe. Freezes old numbers into the save, so a tuning change never reaches a routine already built, and the save carries a field set the game does not own.
- Keeping the authored player routines as fixed recipes beside crafting. The user ruled this out: player routines go away.

## Consequences

Tuning a Core moves every routine built from it, on the next load, because the def is re-derived.
`AbilityDb` stops being purely load-time content: it gains a second, mutable tier. This stretches the Proposed [0004](0004-content-loads-once-as-a-value.md); if 0004 is agreed, the crafted tier moves out to the `Content` value's owner and `get` keeps its one-door shape.
The research tree's routine nodes, `KnownRoutines`, `DiscoveredRoutines`, etching and the creation Routine step lose their player-side meaning and are replaced (see the spec).
The save format changes; `SAVE_FORMAT_VERSION` is bumped and old saves are not migrated.

## Evidence

- `crates/engine/src/abilities.rs` — `AbilityDef`, `AbilityEffect`, `AbilityDb`, `AbilityDef::charged`.
- `crates/engine/src/components/staff.rs` — `Routines`.
- `crates/engine/src/game/combat.rs` — `Game::actor_abilities`.
- `crates/engine/src/items_db.rs` — `ItemDb::synthesise_etched_disks`.
- `crates/engine/src/routine_tree.rs` — `synthesise_nodes`.
- `crates/engine/src/save.rs` — `SAVE_FORMAT_VERSION`, `PlayerSave.routines`, `SaveData.known_routines`.
