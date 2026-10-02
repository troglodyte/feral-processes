# Plan: derived programs

Spec: `docs/superpowers/specs/2026-10-02-derived-programs-design.md` (the
argument; not restated here). Branch `derived-programs`. Delete this file at
landing.

Each phase is one subagent dispatch (sonnet), sized to one context, TDD, a
commit per green step, ending at the phase gate. Phases run in order. Every
dispatch: read the spec section named, the `.claude/rules/` file for the
subsystem, and this phase only; **never push**; stage explicit paths.

Phase gate unless stated: `cargo test -p feral-processes-engine` (or the
phase's crate) + `cargo clippy --workspace --all-targets` + `cargo fmt`.
Mutation-check every new test (comment out the fix, see it fail).

## Facts verified while planning

- `derive` computes `round(base + sum)` per stat then clamps. Seating sets
  `base = target − round(sum)`; since `target` is already in range,
  `derive(base) == target` and no clamp can bite. Only risk is a `.5` tie
  with a negative sum; every `per_point` on an integer stat is whole today.
  Phase 1 adds an assert/test for that and the clamp round-trip test.
- All 18 species: `attributes.analysis == base_int` (or both absent).
- `Growth::Auto` sites on tamed programs: `game/combat_rewards.rs:1163`
  (party), `systems.rs:1547` (posted workers, an ECS system with no
  `&mut Game`), `arena/mod.rs:103` (companion pre-levelling, before seating).
- Hardcoded `DerivedBase::player()`: `game/derived.rs` `derived_stats`,
  `app-core/src/app/stat_allocation.rs:154`.
- Player-only readers to generalise: `player_attributes`,
  `player_stat_bonus` (`game/derived.rs`), `open_stat_allocation`
  (`stat_allocation.rs:259`).
- Power regen query `systems.rs:2154` is `With<Player>`. Phase 2 checks
  whether a program `PowerReserve` regenerates anywhere; if not, leave it.

## Phase 1 — base and seating (engine)

Spec: Seating, `ProgramBase`, Recompute.

- `progression.rs`: `pub fn attribute_contribution(attrs, db) -> DerivedStats`
  (unclamped, rounded sums over a zero base); `derive` calls it, then adds
  base and clamps. `DerivedBase::program(stats: &Stats, contribution)` or a
  `seat_base` fn: integer stats = stats − contribution, secondaries =
  `DerivedBase::player()`'s.
- `components.rs`: `ProgramBase(pub DerivedBase)` (Component, Clone, Copy;
  `DerivedBase` gains `Serialize/Deserialize` if save needs it, phase 5),
  `HoldPoints(pub bool)`.
- `game/derived.rs`: `derived_base(entity) -> DerivedBase`;
  `derived_stats` reads it; `seat_derived(entity)`: lift gear
  (`apply_equipment_delta(-1)`), subtract `BoughtStats`, build base, insert
  `ProgramBase, Derived, StatPoints(0), HoldPoints(false)`, put gear back,
  `recompute_derived`. Idempotent guard: no-op if already seated or not
  `Tamed`.
- Call `seat_derived` last in: `decompile_body`, `decompile_squad`,
  `adopt_program`, `adopt_program_pinned`, `grant_starting_program`,
  `arena::spawn_companion`. (Fusion is phase 3.)
- Generalise `player_attributes`/`player_stat_bonus` to
  `attributes_of(entity)` / `stat_bonus(entity)`; keep player wrappers only
  if callers stay simpler.

Tests (`tests/derived_programs.rs`, new): `Stats` identical before/after
seating at each door, with gear worn and `BoughtStats` present where the
door allows; clamp case (low Footprint, mitigation 0) round-trips; wild
creature never gets `ProgramBase`; seated high-Entropy program has higher
`Derived::crit`; high-Persistence program gets shorter status via
`arm_status`. Prefer `dev-saves/` templates over `Game::new`.

## Phase 2 — level-ups, hold, spending, attention (engine)

Spec: Level-ups, Spending, Attention, Secondary stats.

- `progression.rs`: `pub fn program_level_points(g: f32, roll: f32) ->
  (u32, u32)` using `CANONICAL_PARITY_PER_LEVEL`/`_ANALYSIS_` (never
  restate 4/2). `Growth::ProgramPoints { multiplier, roll }`; `add_xp` on
  that variant changes no stats, accumulates `LevelGain::{parity,
  analysis}` (new fields) and does **not** heal (max HP is stale until
  recompute). Unit test: equals `scaled_growth(HP/ATK_PER_LEVEL, g)` for
  g ∈ {1.0, 1.25, 1.5, 2.0}, roll 1; const/unit assert tying to
  `HP_PER_LEVEL`, `ATK_PER_LEVEL` and the asset effects (like
  `canonical_spend_is_the_old_per_level_growth`).
- `game/derived.rs`: `apply_program_levels(entity, gain)`: unheld → add to
  `Attributes`, held → `StatPoints`; recompute; full-heal. `LevelGain`
  reports `max_hp/atk` as the derived delta so log sites stay correct.
- Switch the party site (`combat_rewards.rs:1163`) to `ProgramPoints` when
  seated. Posted workers (`systems.rs:1547`) can't reach `Game`: record the
  gain in a queue resource drained by `Game` right after the schedule runs
  (check how `tick` runs systems; pick the existing drain pattern if one
  exists). Unseated/wild stay `Auto`.
- `StatOwner::Program(Entity)`; `spend_stat_points` resolves owner,
  `NoSuchTarget` if no `ProgramBase`. `set_hold_points(entity, bool)`;
  turning off spends the bank: whole levels' worth via
  `program_level_points(g, 1.0)`'s ratio, remainder to Parity.
- `inspection.rs` `attention`: one row "N programs have points to spend"
  naming the Manifest.

Tests: unheld level raises Parity/Analysis and heals; held banks; hold-off
strands nothing; spend on unseated and overspend write nothing; attention
row appears/disappears; posted worker level lands through the drain.

## Phase 3 — refactor, rarity guard, fusion (engine)

Spec: Changes after seating.

- **Reproducer first:** fused child's talent respec refunds exactly — must
  fail on current code; commit the failing test only with the fix.
- `refactor.rs` `refactor_companion`: when seated, add `refactored()`'s
  delta to `ProgramBase`, recompute.
- `combat_teardown.rs` `retier_rarity`: `debug_assert!` no `ProgramBase`;
  `#[should_panic]` test under `debug_assertions`.
- `party.rs` `fuse_companions`: parents' `Stats` minus gear minus
  `BoughtStats`; child inherits dominant parent's `BoughtStats`; then
  `seat_derived`. Seating round-trip test for fusion.

## Phase 4 — mining and `base_int` removal (engine, gui, assets)

Spec: Mining.

- Posted program passes its own Analysis (`Attributes`, falling back to the
  catalogue base) and `extraction = 0.0`: `outposts.rs:435` and the
  `systems.rs` mining site(s) (`rg base_int crates`).
- Delete `SpeciesDef::base_int`, `default_base_int`, its validation and
  tests (`species.rs` ~887–1000, 1594–1630, 2026); `base_int:` lines from
  all 18 species `.ron`. `DEFAULT_BASE_INT`: delete if no reader, else
  replace with the Analysis catalogue base.
- `views.rs`/`inspection.rs` manifest view and `gui/render/manifest.rs:809`
  read the program's Analysis attribute; fix gui tests.
- `assets/species/README.md` (field gone, Analysis drives mining),
  `assets/attributes/README.md` (attributes act on tamed programs).
- Rewrite remaining test usages (`tests/{building,chains,extraction,
  inspection,perks,support}.rs`, `needs.rs`) to set Analysis.

Test: posted program's success chance follows its Analysis; no Extraction
double count. Gate: `cargo test --workspace`.

## Phase 5 — save v34 (engine)

Spec: Save. Read `.claude/rules/` save rules first.

- `save.rs` `CreatureSave`: `base: Option<DerivedBase-save>`,
  `stat_points`, `hold_points`, all `#[serde(default)]`. Stat fields for a
  seated program: choose skip-when-seated (`skip_serializing_if` with a
  load path that recomputes) unless a split type is clearly simpler;
  record the choice in the commit message.
- Load restores `ProgramBase`, `Derived` marker, points, then
  `recompute_derived`; never seats.
- `SAVE_FORMAT_VERSION` 33 → 34; regenerate `dev-saves/` via savetool
  (`capture`/load-save path per `dev-saves/README`).
- Save→load test (not RON round-trip): held program with banked points,
  a refactor, a talent and gear → identical `Stats`, `Derived`, points,
  hold flag.

Gate: `cargo test --workspace`.

## Phase 6 — balance_sim and measurement (engine) — **user checkpoint**

Spec: Balance and measurement. Read `docs/measurements/README.md` and
`2026-09-29-level-up-stat-spend.md` first.

- `balance_sim.rs` `companion_stats`: seat base from `wild_stats_at_zone`
  at the species' authored attributes, spend `program_level_points(g, 1.0)`
  per level, `derive`. Test: HP/ATK/mitigation equal old
  `stats_after_levels` for every species × modelled zone × level ≤ cap.
- Model companion crit/fumble from species Entropy. Run
  `cargo test -p feral-processes-engine balance_sim`. **If any curve moves,
  stop and report the moved curves to the user before touching an
  expectation.**
- Arena: levels 10 and 20, ≥3 species of distinct `g`; default vs
  all-Parity / all-Footprint / all-Bandwidth. Plus a one-species mining
  spread table. One file `docs/measurements/2026-10-0X-derived-programs.md`.
  **If one spend dominates, stop and bring numbers to the user.**

## Phase 7 — app-core and GUI

Spec: App-core, GUI. Read `drawing-seam.md` and app-core rules.

- `stat_allocation.rs`: preview struct takes a `DerivedBase` (no hardcoded
  player); `open_stat_allocation(owner: StatOwner, origin)`;
  `commit_allocation` passes owner; `AllocationOrigin::Manifest { subject,
  tab }` returns there on `Esc`/commit.
- Manifest Stats tab, program subject: `H` → `set_hold_points`; `S` →
  `Mode::AllocateStats`, `App::refuse` at 0 points.
- `render/points.rs`: title/footer name the owner. `render/manifest.rs`
  `program_sections`: attribute values, banked points, *holding points*
  marker, `H`/`S` hints, through `Painter`.
- Tests (app-core): `H` toggles; `S` opens for program / refused at 0;
  `Esc` keeps points and returns to same subject+tab; commit spends on the
  program not the player. Screenshot a program's Points screen
  (`--template … --keys … --screenshot`) and Read the PNG.

Gate: `cargo test --workspace` + clippy.

## Finish

1. Final whole-branch review (opus), diff given as a file; fix findings;
   review the fixes.
2. Major release: workspace version bump, `CHANGELOG.md` section at the digit its preamble gives a save break
   (saves break), INDEX.md row → shipped, delete this plan, annotated tag.
   Merge → tag → push `--follow-tags` only on the user's ask.
