# Depot Auto-Feed Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Checkbox steps.

**Goal:** A Depot beside a producer auto-filters to its output once at build, pulls that output from it while it is worked, holds off haulers, and is drawn joined to it.

**Spec:** `docs/superpowers/specs/2026-10-13-depot-auto-feed-design.md`. Read it first. This plan does not restate its decisions table.

**Phases:** one. It is engine-only, has no schema or save change, and fits one context. **Dispatch one subagent for Tasks 1–5.** The controller does Task 6 (docs and seam) and the review. Anchor: `E=crates/engine/src`. Line numbers date from 0.18.23.

## Spec corrections (code differs from the spec)

- **`structures::output_item` already exists** as `systems::produced_item` (`E/systems.rs:812`), and `catalog.rs`, `lines.rs` and `hauling.rs` already call it. Use it and add no new function. Its doc says a banked product never reaches `output`. Check `deliver_payout` (`E/systems.rs:629`) for the "banks" predicate, and make the auto-filter skip a producer whose item banks, so it never writes a useless filter.
- **Hook point: call the auto-filter at the end of `Game::spawn_structure` (`E/game/base/building.rs:852`), not at `construction.rs:569`.** The test fixture `raise_pending_builds` (`E/tests/support.rs:990`, used by `place_now`) calls `spawn_structure` directly and skips `construction.rs`, so a hook placed there would never fire in tests. The other callers are the Home (`building.rs:268`, neither a Depot nor a producer) and a few tests that build Depots with nothing beside them. Upgrades (`construction.rs:572`) do not spawn, so a filter still fires only once.
- **`assembler_system` is at `E/systems.rs:2014`.** Its worked test is at :2086. A third inline copy sits in `idle_machine_system` (:1278, as `.any`), and there may be one in `hauling.rs:1181`. Extract `fn worked_by(tasks: impl Iterator<Item = (Entity, &Task)>, machine) -> Option<Entity>` (or the shape that fits), and have every copy that asks "who works this machine" call it.
- **`depot_accepts`** is `E/game/base/depot_filter.rs:33`. Extract `pub(crate) fn filter_accepts(filter: Option<&DepotFilter>, item: &ItemId) -> bool`, and have `depot_accepts`, the intake system and the hauler check call it.
- **Map join:** `linked_edges_by_structure` (`E/game/inspection.rs:1541`) works per def (`feeds_ingredient`). The Depot edge depends on the entity's `DepotFilter`, so add a second test in the same loop: neighbour is a Depot (`is_depot`) and `filter_accepts(its filter, produced_item(def))`. **`lines.rs` stays untouched.** A Depot does not join a production line, because lines decide what one worker runs.
- **Haulers:** the `attached` computation is at `E/game/base/hauling.rs:1508`. Change it to `consumer_beside(..) || depot_beside_accepting(..)`. It needs the Depots' positions and filters, so check what `depots` already carries before adding a query.

## Global Constraints

- Engine only. No `Game.world` accessor, no new `StructureDef` field, no save change.
- One definition each for producer (`produced_item`), worked (the extracted fn), accepts (`filter_accepts`) and orthogonal (`collect::ORTHOGONAL`). Follow CLAUDE.md's "mirror = call" rule.
- TDD: write each spec test failing first. Mutation-check the intake's worked gate, its filter test and its room cap (`skills:mutation-testing`, with the fix committed). A green step gets `cargo fmt`, `cargo clippy --workspace --all-targets` and a commit.
- Unset `FERAL_DEV_NO_SIEGES`. Iterate with `cargo test -p feral-processes-engine <name>`. Background systems (nests, spawning) interfere, so prefer `place_now`/`spawn_machine_at` fixtures or a `dev-saves/` base (`chains.ron`).
- Subagents never push, never `git checkout`, `git stash` or `git add -A`, and stage explicit paths only.
- Read `.claude/rules/seams-base.md` before editing `game/base/`.

---

## Phase 1: Depot auto-feed (engine)

New test file `E/tests/depot_feed.rs`, registered in `E/tests/mod.rs`. Spec test numbers are in brackets.

### Task 1: Shared predicates (refactor, no behaviour change)
- [ ] Extract `filter_accepts` and the worked fn (see corrections) and switch every existing site to them. The existing suite stays green, and this is its own commit.

### Task 2: Auto-filter
- [ ] `Game::auto_filter_depots(built: Entity)` in `depot_filter.rs`, called at the end of `spawn_structure`. Its apply rule is in the spec. It writes only through `set_all_depot_filters`/`set_depot_filter`.
- [ ] Tests [1] Depot beside a node, [2] node beside an existing Depot, [3] a manual filter only gains the item, [4] two producers allow both. Add one for a banked producer, which writes no filter.

### Task 3: Intake system
- [ ] `depot_intake_system` in `systems.rs`. Register it in `lifecycle.rs:767` chained after `assembler_system`. Walk Depots in `(x, y)` order using `feeders_by_tile`, `filter_accepts`, the worked fn, the `is_dark` guard and `Stock::output_room` (`components.rs:914`). Plan against a snapshot, then apply through `hauling::take_from` (`hauling.rs:57`).
- [ ] Tests [5] worked vs unstaffed, [6] full Depot stops and the buffer stays, [7] a refused item is not pulled, [9] Mining Node → Depot → Lathe reaches the Lathe's input with no carrier. Run `balance_sim` once here.

### Task 4: Haulers
- [ ] Extend `attached` (see corrections). Test [8]: a hauler does not lift a node's output while an accepting Depot is beside it. Add one where the Depot refuses the item, and there the hauler does lift it.

### Task 5: Map join
- [ ] Add the Depot edge to `linked_edges_by_structure`, from both ends. Test [10] covers accept vs refuse. Find an existing `linked_edges` test in `E/tests/inspection.rs` to model it on.
- [ ] Gate: `cargo test --workspace`, clippy clean. Hand back the commit list and the mutation results.

### Task 6: Docs and seam (controller)
- [ ] `assets/structures/depot*.ron` descriptions (grep them for repeated text), and `assets/help/62-production-lines.md` and `60-your-base.md`.
- [ ] New seam, in the `seams` skill's three-write order (graph, then skill, then `.claude/rules/seams-base.md`): "a Depot pulls only what it accepts, only from a worked producer; the auto-filter fires once, at spawn".

**Review Focus (opus, whole branch):** check that every "worked", "accepts" and "producer" question goes through the one shared function, and that no copy is left. Check that the intake cannot pull past `output_room` or from a dark or unstaffed producer. Check that the auto-filter never narrows an existing filter and fires only from `spawn_structure`. Check that a base with no Depot behaves exactly as before in hauling, and that the drawn join and the pull agree. Then land via `skills:deploy` (patch bump and `CHANGELOG.md`) and add the work to `docs/superpowers/INDEX.md`. Ask the user before landing.
