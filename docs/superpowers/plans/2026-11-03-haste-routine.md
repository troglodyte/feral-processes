# Haste routine: plan

**Spec:** `docs/superpowers/specs/2026-11-03-haste-routine-design.md` (read it
first; this plan doesn't repeat its rules).

**Branch:** `worktree-haste-routine`. **Worktree:**
`.claude/worktrees/haste-routine`.

## Ground rules

- Use TDD: write a failing test, make it pass, then commit. Commit at every
  green phase.
- Paths below are relative to `crates/engine/src` unless they say otherwise.
  The line numbers came from a research pass and may have drifted, so grep
  the symbol.
- `cargo fmt` and `cargo clippy --workspace --all-targets` must be clean
  after each phase.
- Unset `FERAL_DEV_NO_SIEGES` before you run tests.

## Phase 1: the Core, the effect and the state (engine only)

**Goal:** Haste can be compiled, validated and cast. Casting it arms a
`Hasted` component, and the end of the fight clears it. Nothing reads the
component yet.

### Files

`routine_craft/catalogue.rs`:

- Add `Core::Haste` as the **last** variant, because the bincode
  discriminant is in saves. It also goes last in `Core::ALL`, which grows to
  `[Core; 32]`.
- Its arms:
  - `name` returns "Haste".
  - `versioned` is true.
  - `facing` is `Ally`.
  - `is_field` is false.
  - `takes_range` and `takes_trickle` are false.
  - `areas` is `&[AreaKind::Group]`.
- Give it a `material` group next to Cloak's. `fits` should need nothing
  new; check this.

`abilities.rs`:

- Add `AbilityEffect::Haste { version: u8 }` as the **last** variant.
- Its arms in `field_only`, `affinity_category` (None), the "names nobody"
  set, and every other exhaustive match.
- Add a `haste_mismatch()` that refuses any target other than `OneAlly` or
  `WholeParty`, modelled on `cloak_mismatch`. Register it in the `validate`
  faults array.
- Refuse a version outside 1..=3.
- Update the `assets/abilities/README.md` effect list (the count of effects
  and a Haste entry), even though no asset uses it.

`tuning/routine_craft.rs`:

- Add `CRAFT_HASTE` using `figures(cooldown, power_cost, 0, 0, 0, 0)`.
  Base the cost and cooldown on `CRAFT_BOOST_ATTACK`.

`tuning/tactical.rs` (or a new `tuning/haste.rs`, re-exported):

- `HASTE_MOVE_BONUS: [u32; 3] = [4, 8, 8]`.
- `HASTE_DOUBLE_SWING_VERSION: u8 = 3`.

`routine_craft/compile.rs`:

- `core_figures` gets the `Core::Haste` arm.
- The effect match builds `Haste { version }`.

`routine_craft/decompose.rs`:

- `Haste { version }` maps to `(Core::Haste, Version::from_number)`.

`routine_craft/describe.rs`:

- One plain-language line per version. For example: "+4 movement on the
  battle map for the rest of the fight" and, at v3, "…and every swing
  strikes twice". No tick vocabulary.

`components/effects.rs`:

- `#[derive(Component)] pub struct Hasted { pub version: u8 }`. No serde.

`game/combat_status.rs`:

- `arm_haste(entity, version)`. It keeps the higher version: `max(existing,
  new)`.

`game/combat_round.rs`:

- The `use_ability` arm calls `arm_haste` for each recipient.
- Check that the classic `OneAlly` pick can name a companion slot (around
  `:1606`). If no picker reaches companions, stop and report; don't build
  one.

`game/combat_teardown.rs`:

- Add `unhaste` beside `uncloak`, called in the player, party and hostile
  loops.

### Tests

- `routine_craft/tests.rs`: add a `MATRIX` row, and fix the length and the
  matches near `:253` and `:263`.
- `tests/assets/recipes_power.rs`: whatever its `Core::ALL` census needs.
- New tests:
  - Each version compiles to the right effect and decomposes back.
  - Validate refuses an enemy target and version 0 or 4.
  - Casting arms `Hasted`.
  - Recasting v1 then v3 gives v3. Recasting v3 then v1 stays v3.
  - Teardown removes `Hasted`.
- Save safety: a save→load test containing a `CraftedRoutine` with an
  existing Core (for example `Cloak` and `Damage` v2). It must load
  unchanged. If `save/tests.rs` already covers this, extend it with a
  `Haste` part instead.

### Gate

```sh
cargo test -p feral-processes-engine routine_craft
cargo test -p feral-processes-engine save
```

Then clippy. Commit.

## Phase 2: movement

In `tactical/reach.rs`, `Game::movement_allowance` adds
`HASTE_MOVE_BONUS[v-1]` **after** `allowance(...)`. The `Charging → 0`
early return stays first.

Audit callers that treat `TACTICAL_MOVE_MAX` as a ceiling on a body's
movement:

```sh
rg TACTICAL_MOVE_MAX crates/
```

This covers buffers sized by it, AI scoring and view code. Fix anything
that would clip a range of 16.

**Tests:**

- Base speed v1 gives 8.
- A body at the cap hasted at v2 gives 16.
- A charging hasted body gives 0.
- `movement_field` for a hasted body reaches a cell 12 away on open ground.

**Gate:** `cargo test -p feral-processes-engine tactical`, then commit.

## Phase 3: swings

### Classic

`game/combat.rs`, `attacks_for`: multiply by 2 when the body is
`Hasted { version >= HASTE_DOUBLE_SWING_VERSION }`.

Read the swing loops at `game/combat_round.rs:~541` and
`game/combat_enemy.rs:~224`:

- Confirm nothing truncates the count to `MAX_ATTACKS_PER_ROUND`. Today
  that constant is used only in `battle::attacks_per_round`.
- Confirm the loop stops when the target dies, as it does for the
  Striker's second swing.

### Map

`tactical/turn.rs`, `tactical_attack`: after the first swing resolves, a
`Hasted` v3 actor whose target is still alive resolves one more ordinary
swing against the same target. It still spends one action.

- Extract the single-swing resolution into a helper if the body of
  `tactical_attack` makes calling it twice awkward.
- The second swing must not re-run the cloak-break side effects or the XP
  award twice. Check what happens on a kill.
- Update the "one swing a turn here" comment.

### Tests

- `attacks_for`:
  - Plain v3 gives 2.
  - A level 8 Striker with v3 gives 4.
  - v2 gives the unchanged count.
- A classic round with a hasted v3 member logs two swings.
- Map:
  - A hasted v3 attack lands two hits, and `actions_left` drops by 1.
  - A first swing that kills the target produces no second swing and no
    panic.

Use a seeded RNG, and give the defender high Integrity so that the
double-hit test doesn't depend on a lucky seed.

**Gate:**

```sh
cargo test -p feral-processes-engine combat
cargo test -p feral-processes-engine tactical
cargo test -p feral-processes-engine balance_sim
```

`balance_sim` should not move. Commit.

## Phase 4: markers (engine views, then gui)

### Classic

In `game/field.rs` (around `:590-640`), push an `ActiveBuffView` row
"Hasted vN" for a `Hasted` body, beside the Cloaked row. It has no
`remaining` count, so use whatever the view does for "until the fight
ends". Check how `gui/src/render/field.rs` shows `remaining`, and make sure
it prints nothing in rounds.

### Map

In `tactical/view.rs`, add `hasted: bool` to the body view, set beside
`cloaked`.

In `crates/gui/src/render/tactical/bodies.rs`, draw a small mark through
the `Painter` only, following `.claude/rules/drawing-seam.md`. A
distinct-colour pip or edge works; don't reuse the cloak wash.

### Tests

- A view test for each field.
- A headless gui test only if a similar one exists for `cloaked`.

### Screenshot

1. Make a dev arena with a companion that has a hasted v3 routine available
   to the player.
2. Run it with `FERAL_DEV_ARENA=1`.
3. Take a screenshot.
4. Read the PNG.

The `battle-screenshot-goes-through-the-arena` memory explains why `--keys`
can't walk you into a fight.

**Gate:** `cargo test --workspace`, then clippy. Commit.

## Phase 5: wrap-up

- Add a `CHANGELOG.md` entry under an unreleased heading. The version is
  bumped at deploy, not here.
- Add a row to `docs/superpowers/INDEX.md`.
- Seams: if `movement_allowance` or `attacks_for` has a `.claude/rules/`
  file, update its one-line rule (and the `seams` skill and graph) for the
  haste bonus.
- Run a final whole-branch review with Opus, giving it the diff as a file
  (`git diff main... > <scratchpad>/haste.diff`). Fix what it confirms, then
  re-run the gate.

## Out of scope

- Wild or species use.
- Field use.
- Balance tuning.
- Any `assets/` file other than the abilities README.
