# Weapon Crafting — Plan Index

> **For agentic workers:** use superpowers:subagent-driven-development (or
> superpowers:executing-plans). Read **this file plus your one phase file**,
> nothing else from `plans/`. Steps use `- [ ]`.

**Goal:** player weapons are designed at the Workbench from weapon parts (one
Frame + modifiers), saved as a `GearDesign`, compiled to an `ItemDef` by one
pure function, and built into ordinary `GearCopy`s. The generic layer is
shaped so armour and modules add only a part enum and a `compile`.

**Spec:** `docs/superpowers/specs/2026-10-26-weapon-crafting-design.md` — the
source of truth for every table (frames, modifier categories, fits, cap,
sources, research). Phase files cite its section names; they do not restate
them.
**ADR:** `docs/adr/0031-…` — **Agreed** (2026-10-26); the spec's calls are all
answered under "Decided by the user".
**Model to copy:** `crates/engine/src/routine_craft/` + `game/routine_craft.rs`
(ADR 0029). When unsure how to shape something, do what routine crafting did.

## Phases

| # | File | Crates | Ends with |
|---|---|---|---|
| 1 | `…-phase-1-core.md` | engine | `gear_craft/` pure core: design value, weapon catalogue, `compile`, `decompose`, `design_power`, censuses. Nothing wired. |
| 2 | `…-phase-2-items-and-designs.md` | engine | `ItemDb` crafted tier, part items, `Game` design/build/delete API, save fields, views. |
| 3 | `…-phase-3-research-sources-combat.md` | engine | Weapons tree, salvage, discovery, shelves, rider + exploit in combat. |
| 4 | `…-phase-4-screens.md` | app-core, gui, assets/help | Forge, New design, Weapon parts pages. |
| 5 | `…-phase-5-authored-and-land.md` | engine assets, all | `craftable` blocks off authored weapons; full gate, opus review, CHANGELOG. |

Additive first (1–4), removal last (5), so every commit compiles and every
crate stays green. Phases are sequential (2 needs 1's types; 3 needs 2's door;
4 needs 2–3's API). Each phase is one sonnet dispatch; its "Gate" section is
the handback contract. Phase 5's review is opus.

## Global constraints (every phase, every dispatch prompt)

- Work in a worktree on branch `weapon-crafting` (ask the user before creating
  it — they play from the primary checkout; `EnterWorktree` before spawning
  subagents). `git fetch` and compare `origin/main` at each phase boundary;
  merge main in before a phase builds on a moved API.
- Never push, `git checkout` uncommitted work, `git stash`, `git add -A`, or
  `reset --hard`. Stage explicit paths. Commit at each green step.
- TDD: failing test first. `cargo fmt` + `cargo clippy --workspace
  --all-targets` clean at every commit. Unset `FERAL_DEV_NO_SIEGES`.
- Read the rules files the phase lists before touching that subsystem; read
  the `seams` skill entry if a rule reads as arbitrary.
- Functional core / imperative shell for every new `impl Game` method. RNG
  draws sorted by stable id; draw order of existing paths unchanged.
- Constants only in `tuning/gear_craft.rs`. No new `assets/` kind, no new
  `.ron` field: rider/exploit are `#[serde(skip)]` on `ItemDef`, set only by
  `compile`. No `world` accessor.
- Only weapons get code. Armour/modules: no stub variants, no empty modules
  (YAGNI) — the `slot` match has one arm plus a typed refusal.
- Plain-language generated names/descriptions; no tick vocabulary; lists are
  highlight + Enter.
- Don't touch `docs/manual.md`, `README.md`, `TODO.md`. Do update
  `assets/items/README.md` (phase 5), `assets/help/` (phase 4), CHANGELOG
  (phase 5).
- **Stop and ask** (don't invent) when: an authored weapon maps to no Frame;
  a compiled design can't pass a validator at spec-shaped numbers; the
  ceiling and no-dominance tests can't both hold; a part's material breaks the
  research-bill rule.

## Review focus (phase 5 opus review, and each phase's self-check)

1. A design that compiles but can't load — `compile` output must pass
   `ItemDb::load_dir`'s validators (census over `all_designs_up_to_cap()`).
2. A lookup that misses `crafted_gear/<n>` — `ItemDb::get` answers both tiers;
   grep every direct map access / authored-only iteration over items.
3. A crafted def missing on load — `set_crafted_gear` runs before inventory,
   equipment, companions and buyback shelves restore.
4. Rider/exploit lost on save→load (runtime-only fields re-derived by
   `compile`, never saved). Save→load test, not a RON round trip.
5. Exploit damage applied inside `apply_damage*` (must be before it), or a
   rider arming outside the existing status-arming path.
6. A dominant design: no-dominance and ceiling censuses use `design_power`,
   not `copy_power`.
7. RNG: rider rolls are new draws — confirm `balance_sim` is unmoved (it uses
   authored weapons with no rider).
