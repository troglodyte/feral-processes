# Routine Crafting — Plan Index

> **For agentic workers:** use superpowers:subagent-driven-development (or
> superpowers:executing-plans). Read **this file plus your one phase file**,
> nothing else from `plans/`. Steps use `- [ ]`.

**Goal:** player routines are built at the Workbench from Subroutines (one
Core + modifiers), compiled to an `AbilityDef` by one pure function. Etching,
authored player routines and the creation Routine step go away.

**Spec:** `docs/superpowers/specs/2026-10-24-routine-crafting-design.md` — the
source of truth for every table (Cores, modifiers, fit matrix, cap, costs).
Phase files cite its section names; they do not restate them.
**ADR:** `docs/adr/0029-…` — **Proposed. Phase 1 does not start until the user
marks it Agreed** (CLAUDE.md: never build on a Proposed record).

## Phases

| # | File | Crates | Ends with |
|---|---|---|---|
| 1 | `…-phase-1-catalogue.md` | engine | `routine_craft/` pure core: catalogue, `compile`, `decompose`, Heal over time. Nothing wired. |
| 2 | `…-phase-2-engine-wiring.md` | engine | Crafted tier in `AbilityDb`, part items, `Game` API, research, extraction, save v39, new game. Old paths still present. |
| 3 | `…-phase-3-screens.md` | app-core, gui | Workbench pages; routine screens use the new API. |
| 4 | `…-phase-4-removal.md` | all + assets | Etching, known/discovered routines, starters, 31 files gone; census "no authored grant". |
| 5 | `…-phase-5-land.md` | — | Full gate, opus review, `fred` save, CHANGELOG. |

Additive first (2, 3), deletion last (4), so every commit compiles and every
crate stays green. Each phase is one subagent dispatch (sonnet); the phase's
"Gate" section is its handback contract.

## Global constraints (every phase, every dispatch prompt)

- Worktree on branch `routine-crafting` (ask the user before creating it —
  they play from the primary checkout). `git fetch` and compare `origin/main`
  at each phase boundary; merge main in before a phase builds on a moved API.
- Never push, `git checkout` uncommitted work, `git stash`, `git add -A`, or
  `reset --hard`. Stage explicit paths. Commit at each green step.
- TDD: failing test first. `cargo fmt` + `cargo clippy --workspace
  --all-targets` clean at every commit.
- Read the rules files the phase lists before touching that subsystem; read
  the `seams` skill entry if a rule reads as arbitrary.
- Functional core / imperative shell for every new `impl Game` method
  (gather → pure decide → apply). RNG draws sorted by stable id.
- Constants only in `tuning/routine_craft.rs`. No new `assets/` kind, no new
  `.ron` field (ADR 0029). No `world` accessor.
- No player-facing tick vocabulary; plain-language generated descriptions.
- Don't touch `docs/manual.md`, `README.md`, `TODO.md`. Do update
  `assets/abilities/README.md` (phase 4), `assets/help/` (phase 3), CHANGELOG
  (phase 5).
- Unset `FERAL_DEV_NO_SIEGES` before tests.
- **Stop and ask** (don't invent) when: a boss/wild routine fits no Core; a
  material choice breaks the research-bill rule; a spec number turns out
  unworkable against the validators.

## Review focus (phase 5 opus review, and each phase's self-check)

1. A built routine that loads but cannot run — `compile` output must pass the
   same validators `AbilityDb::load_dir` applies (census, phase 1).
2. A lookup site that misses a crafted id — `AbilityDb::get` must answer both
   tiers; grep for any direct map access or authored-only iteration.
3. A crafted def outliving its routine — `set_crafted` rebuilt after every
   build / install / uninstall / companion-leave and on load.
4. Any remaining path that grants the player an authored routine.
5. Save→load round trip of a built routine on player **and** companion (a
   RON round trip alone cannot catch a skipped field).
6. RNG stream: new resources/components can shift query order — check
   `balance_sim` and suite-only flakes against that first.
