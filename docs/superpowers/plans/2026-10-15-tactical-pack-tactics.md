# Tactical pack tactics Implementation Plan

> **For agentic workers:** use superpowers:subagent-driven-development, one subagent per numbered dispatch below. Steps use checkbox (`- [ ]`) syntax.

**Goal:** the hostile side of a battle-map fight rolls one pack tactic (`Rush`, `Together`, `Hold`; then `Formation`, `Flank`) that adds pack-aware terms on top of each body's personality.

**Spec:** `docs/superpowers/specs/2026-10-15-tactical-pack-tactics-design.md`. Read it first; this plan does not restate it. Its **Testing** list is the acceptance list.

## Why these dispatches

The spec lands two releases. Each release is: one implementation dispatch (engine only, so one crate in context), an opus review, then a cheap measurement dispatch. Measurement comes after review because a bug found after an arena batch redoes the batch. Phase 2 adds enum arms, terms and odds rows to interfaces phase 1 fixed, so its dispatch starts from the phase 1 code, not the spec's whole design.

| # | Dispatch | Model | Output |
|---|---|---|---|
| 1 | Phase 1 implementation | sonnet | commits on the branch |
| R1 | Phase 1 review (whole diff since the spec commit, given as a file) | opus | findings → fixes → fix review |
| M1 | Phase 1 measurement | sonnet | `docs/measurements/` note; weights adjusted if outside the bar |
| — | Release phase 1 (`deploy` skill) | inline | |
| 2 | Phase 2 implementation | sonnet | commits |
| R2 | Phase 2 review | opus | |
| M2 | Phase 2 measurement | sonnet | note appended or a second note |
| — | Release phase 2 | inline | |

## Corrections to the spec (from the source)

- Engine tests live in `crates/engine/src/tests/`, not `tests/`: the new file is `src/tests/tactical_pack.rs`, registered in `src/tests/mod.rs` beside `mod tactical_policy;` (`:132`). Fixtures are `src/tests/support.rs`.
- There is **one** seat for a rolled fight: `Game::open_tactical_battle_at` (`src/tactical/turn.rs:76`); `open_tactical_battle` and `arena::stage` (`src/arena/mod.rs:364`) both go through it. Roll there, after the bodies are seated.
- `TacticalBattle::open(spec, board)` (`src/tactical/mod.rs:262`) builds every battle, including hand-built test boards (`tests/tactical.rs`, `defences.rs`, `combat_rewards.rs`) and sieges (`game/siege/mod.rs:207`, `siege/persist.rs:174`). It initialises `pack` to `Rush`. Sieges never call the seat, so they never roll and the restored siege is `Rush` with no save field. Assert that rather than adding a siege branch to `roll`; keep `siege_pack > 0 → Rush` in `roll` only if the seat can ever see a siege.
- **Arena override:** `arena::stage` sets the profile override before seating (`src/arena/mod.rs:302`, `Behaviours::set_override`). The tactic needs the same order, because the roll and its log line happen inside the seat. Add the override the same way: a field the roll reads, set by `stage` before it seats, refused at parse on an unknown name (serde enum, no string fallback). Add `("tactic", self.tactic.is_some())` to the scenario's field census beside `behaviour` (`scenario.rs:443`).

## Anchors

`src/tactical/`: `mod.rs:120` `TacticalBattle` (`round` `:174`, `siege_pack` `:242`, `cells_of` `:401`); `ai.rs` `cell_merit` `:285`, `tactical_forecast` `:458`, `tactical_sides` `:1030`, `scored_cells` `:1136`, `walk_risk` `:1235`; `policy.rs:221` `choose_hostile_intent`; `reach.rs` `gap` `:249`, `line_of_sight` `:329`. Elsewhere: `behaviours.rs:210` `behaviour_of`; `game/combat.rs:167` `swing_range`; `game/combat_damage.rs:362` `apply_damage` (do not hook it); `tactical/map.rs:75` `BattleSpec`. Tests to reuse: `tests/tactical_policy.rs:261` (forecast, loop over tactics) and `:385` (Rush unchanged).

## Global constraints (every dispatch prompt carries these)

- Engine crate only. No asset, schema or save-format change; no `assets/behaviours/*.ron` edit. The tactic data is Rust (`PackTactic`) plus `tuning.rs` constants, by the user's decision.
- **The roll never draws `GameRng`.** Seed from `BattleSpec`'s `world_seed`, `site`, `tick`, with its own salt distinct from the board's.
- All pack terms enter through the planner the turn runs (`scored_cells`/`cell_merit`, closing scale, the intent bonus). No second decision path; `tactical_forecast` must keep following without its own copy. Read `.claude/rules/` for `tactical`/combat seams before editing (`ls .claude/rules/`).
- `Rush` adds no terms and a closing scale of exactly 1.0, so every existing tactical test passes unedited. A test that needs editing is a defect report, not a port.
- Names: follow the spec's constants (`TACTICAL_COHESION_WEIGHT`, …) and `PACK_TACTIC_ODDS`. Comments say why. Log lines use `MessageKind::Info` and the spec's draft text ("programs", never "monsters").
- TDD: red → green → commit per step. Each tactic test must fail with its term removed (mutation check, with the fix committed first).
- Gates: `cargo fmt`; `cargo clippy --workspace --all-targets` clean; `cargo test -p feral-processes-engine`; `balance_sim` once (must not move). **Unset `FERAL_DEV_NO_SIEGES`** first. Don't pipe cargo when the exit code matters.
- Git: branch `pack-tactics` (the spec commit `f560227e` is on it). **Ask the user before creating a worktree**; the primary checkout is where they play. Commit at green, explicit paths only (never `git add -A`). Never push; never `git checkout`, `stash` or `reset`.
- Docs: never the manual, README or TODO.md. CHANGELOG at release.

---

## Dispatch 1: Phase 1 — pack state, roll, Together, Hold

**Files:** `src/tactical/pack.rs` (new), `tactical/mod.rs` (`pub mod pack`, `PackState` field), `tactical/turn.rs` (roll at the seat + opening line), `tactical/ai.rs`, `tactical/policy.rs` (only if the intent bonus hook goes in now as a zero for non-Flank — optional), `tuning.rs`, `arena/scenario.rs`, `arena/mod.rs` (override before seat), `src/tests/tactical_pack.rs` (new), `src/tests/mod.rs`.

**Interfaces fixed here** (phase 2 only adds to them):
- `pub enum PackTactic { Rush, Together, Hold }` — serde-deserialisable for the arena; `Formation`/`Flank` arrive in phase 2.
- `PackState { tactic, hold_start_round, hold_hp: Vec<(Entity, i32)> }` (shape is the implementer's; it holds only what `Hold`'s triggers read).
- `pack::roll(spec, members, game) -> PackTactic`; `pack::cell_terms(game, body, cell) -> f32`; `pack::closing_scale(game, body) -> f32`; `pack::check_transition(game) -> Option<log line>` called at the start of each hostile turn, before the intent is chosen.
- `tuning::PACK_TACTIC_ODDS` keyed by behaviour id (`default`, `brute`, `hunter`, `skirmisher`, `cautious`), phase 1 columns only.

**Steps**
- [ ] `PackState` on `TacticalBattle`, `Rush` from `open`. Test: an existing test (`:385`) under explicit `Rush` passes; forecast test loops over the phase 1 tactics.
- [ ] Roll: deterministic per spec; no `GameRng` draw (stream position equal before/after); one body never rolls `Together`; pack profile is the toughest body's (ties → initiative order), unlisted id → `default`; opening log line (none for `Rush`).
- [ ] Arena `tactic:` override: wins over the roll; an unknown name fails to parse.
- [ ] `Together` cohesion term (slack 2): a fast body that would outrun a slow packmate stops within the slack.
- [ ] `Hold`: closing scale 0 + cover bonus; walks to cover then stays while the player is out of sight. Triggers — sighted, hit (HP snapshot), member gone, timeout round — each its own test; the triggering body acts under `Rush` the same turn; one "breaks cover" line.
- [ ] Mutation check each tactic test; gates.

**Handback:** commits, the shape of `PackState` and the four `pack::` signatures as built, any existing test that changed and why (should be none).

## Dispatch M1: Phase 1 measurement

Read `docs/measurements/README.md` and existing notes first. Scenarios: `dev-arenas/tactical-full-group.ron` and a new mixed melee-and-ranged scenario (also used by M2), `party_turns: AutoPlay`, one run set per tactic forced via `tactic:`, `Rush` as the baseline on the same build. Bar: party win rate within 10 points of `Rush`, party HP lost within 20%. Outside the bar → adjust that tactic's `tuning.rs` weights, re-run, note both. Write one note per the README. Handback: the table and any weight change.

## Dispatch 2: Phase 2 — Formation, Flank with focus

Starts from phase 1's handback; do not reopen phase 1 interfaces.

**Files:** `tactical/pack.rs`, `tactical/ai.rs` (only if a new hook is unavoidable), `tactical/policy.rs` (focus bonus in `choose_hostile_intent`, outside profile weights), `tuning.rs` (constants, odds columns), `src/tests/tactical_pack.rs`.

**Steps**
- [ ] Enum arms + odds columns; eligibility (`Formation` needs a front and a back body; both need ≥2 bodies). Roll tests extended.
- [ ] Ranks and slots as pure functions of positions and kit (`swing_range` / ready routine range vs `TACTICAL_MELEE_RANGE`); unit-test the slot assignment directly.
- [ ] `Formation` terms: a ranged body stays behind a melee packmate while both advance; a front body adjacent to a non-pack body leaves its slot; front rank gone → `Rush` + "The line breaks!".
- [ ] `Flank` cell terms (crowd, pincer; `walk_risk` untouched): a second attacker takes the sector opposite the first.
- [ ] Focus intent bonus (capped): picks the target a packmate is already on over an otherwise preferred one.
- [ ] Forecast loop covers all five tactics; mutation check; gates.

**Handback:** as dispatch 1.

## Dispatch M2

As M1, for `Formation` and `Flank`, on the mixed scenario (Formation must be eligible).

## Release (inline, each phase)

`deploy` skill. CHANGELOG section. Seam: the rule *the pack tactic is rolled from the `BattleSpec` and never draws `GameRng`* — graph, skill, rules file in the `seams` skill's order — with phase 1. `docs/superpowers/INDEX.md` entry.
