# 0041 Routine parts are found before they are researched

- Status: Proposed
- Decided by: user, on Claude's proposal
- Date: 2026-11-03
- Pattern: none new (reuses the base tree's `discoverable` + `DiscoveredResearch`)
- Depends on: [0029](0029-player-routines-are-compiled-from-a-rust-part-catalogue.md)
- Amends: [0029](0029-player-routines-are-compiled-from-a-rust-part-catalogue.md) (its spec's "visibility is zone-gated only")

## Context

Since 0.20.0 the routine research tree lists every Subroutine part and capacity step the moment its zone is reached (`routine_tree.rs::part_node` sets `discoverable: false`; `Game::routine_node_visible` is the zone gate alone).
The routine crafting spec removed the old discovery layer on purpose.
The user wants it back for parts: the list should start nearly empty and fill as you meet programs, with **a chance** to find a part rather than a guarantee, through both **extraction/teardown** and **Research Station study**.
They also want a couple of routines usable from a new game: **Heal** and **Boost Attack**.

## Decision

**A routine part node is hidden until found, and found by chance from a program that carries it.**

What is hidden:
- Every synthesised part node is `discoverable: true` except the **starting parts**, Heal v1.0 and Boost Attack v1.0, which stay `discoverable: false`.
- **Capacity nodes stay `discoverable: false`**: zone-gated and visible, as today. They belong to no program.
- A routine node is listed when the tree is open and it is researched, or zone-allowed and (not `discoverable`, or in `DiscoveredResearch`).
  This is `base_node_visible`'s rule plus the existing zone gate. `routine_node_visible` calls the shared predicate rather than copying it.
  `select_research` refuses a hidden routine node through the same predicate.

Starting parts:
- A new game researches Heal v1.0 and Boost Attack v1.0. `grant_starting_routine` already inserts Heal's node; Boost Attack's joins it.
  Patch stays the one starting built routine.
- The two ids are a named constant in `tuning/routine_craft.rs` that both `synthesise_part_nodes` (for `discoverable: false`) and the new-game grant read.

How a part is found:
1. **Extraction and teardown.** `Game::take_routine` is the one door both use (a `Routines` tool on a downed program and `extract_routine` on a tamed one), so the roll goes there.
   After the parts are granted, each part of that routine whose node is **findable** rolls `ROUTINE_PART_DISCOVERY_CHANCE` independently (new, `tuning/`, starting at 1/3; my call).
   *My call:*
   - The roll set is the distinct parts, in `Subroutine`'s `Ord` order: the order `parts_used` (which `grant_parts` already uses) yields, so a duplicate part rolls once.
   - Findability is computed once, before any roll, so a v2.0 found in this extraction does not make the v3.0 beside it findable until the next one.
   - One `GameRng` draw per findable part, and **no draw at all** when none is findable. An extraction of fully known parts leaves the stream exactly as today.
2. **Research Station study.** `eligible_discoveries` gains a `ResearchTree::Routines` arm, admitted only while the routine tree is open. It is narrowed to the findable part nodes of the pinned subject's routines (its `Routines`, through `parts_of_routine`).
   One pool, one roll at `STUDY_DISCOVERY_CHANCE`, one pick, and a hit spends the subject. This is unchanged from today.

**Findable** means `discoverable`, not discovered, not researched, inside the zone gate, and for a Core v2.0/v3.0, the version below it is **known**.
**Known** means researched, or listed: not `discoverable` (Heal v1.0 and Boost Attack v1.0 always count), or in `DiscoveredResearch`.
Modifiers have no prerequisite.
The Routines arm of `eligible_discoveries` uses this predicate **instead of** the base arms' `requires.all(is_researched)`. One pure predicate serves both doors.
So a program carrying Heal v3.0, met with nothing known, can turn up v1.0. (Heal v1.0 is a starting part, so in practice it turns up v2.0.)
It can never turn up a version that skips a rung.
The rung must be found, not researched. The found node may arrive locked behind its unresearched prerequisite. This is a deliberate step away from the base tree's "a discovery is immediately researchable", because a version chain is visible as a chain.

A find calls `discover_research`. *My call:* the function gains a source parameter (study or extraction) that picks the log line, so there is one door and no double log.
A find from either source raises the `ResearchDiscovered` notification, as a study find does today.
The extraction line is plain language, e.g. "Taking it apart shows you how {part} is made." The exact wording is the builder's.

Boost Attack is a versioned Core, so `part/` Boost Attack v1.0 is already a synthesised node. A new game researches it and builds nothing from it. Patch (Heal) stays the only routine built at the start.

**No save change.** `DiscoveredResearch` is already saved and holds any research id.
An older save loads with no part nodes discovered. `node_researched` keeps everything already researched listed, and the two starting parts are visible as `discoverable: false`.
*My call:* no load-time grant. On an older save Boost Attack v1.0 is listed and waits to be researched, so that save's list differs from a new game's by that one node.

## Alternatives considered

- **A per-part reveal on every extraction (no chance).** The user asked for a chance.
- **A new `DiscoveredParts` save field.** `DiscoveredResearch` already answers it, and a second set would need its own save round trip and migration.
- **Hiding capacity nodes too.** No program carries a capacity step, so there would be nothing to find them from.
- **Gating a find on the rung below being *researched*, as the base pool does.** That makes a strong program worthless until you had researched up the chain. The user agreed to "found".

## Consequences

- The routine list starts with Heal v1.0, Boost Attack v1.0 and the capacity steps, and grows from fighting and study.
- Extraction draws more `GameRng` after a routine comes out. Study draws differently while the routine tree is open. Seeded tests that extract or study can shift (`rng-stream-shift-exposes-seed-luck-tests`). `balance_sim` models neither, so it does not move.
- The census `every_discoverable_research_node_is_reachable_from_the_visible_tree` (`tests/assets/structures_research.rs`) and `exactly_the_named_research_nodes_are_discoverable` see synthesised part nodes for the first time. A part is reachable through any program that carries it, not through prerequisites, so the routine tree is excluded from the first. The second names the routine set as "every part node but the starting parts".
- A part no shipped program decomposes to can never be found. A census asserts every discoverable part node is carried by some shipped species' routine at a zone it spawns in, or names the exceptions.

## Evidence

- `crates/engine/src/routine_tree.rs` — `part_node`, `synthesise_part_nodes`.
- `crates/engine/src/game/unlocks.rs` — `routine_node_visible`, `routine_node_reachable`, `base_node_visible`, `listed_research`, `select_research`, `eligible_discoveries`, `settle_study`, `discover_research`.
- `crates/engine/src/game/routines.rs` — `take_routine`, `parts_of_routine`.
- `crates/engine/src/game/routine_craft.rs` — `grant_starting_routine`.
- `crates/engine/src/resources.rs` — `DiscoveredResearch`.
- `crates/engine/src/tuning/meta.rs` — `STUDY_DISCOVERY_CHANCE`.
