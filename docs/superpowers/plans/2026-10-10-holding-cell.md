# Holding Cell — plan

There is no separate spec. Sections 1–2 of the design were approved in chat
on 2026-10-10 and are restated under **Design**. Sections 3–4 (UI, tests)
were not presented, so their choices are listed under **Decisions** and
open to correction. Branch `holding-cell`. The worktree is undecided: ask
the user before creating one.

**Every phase:**
- Use TDD and commit at each green step. Mutation-check the odds, the
  attempt counter, the breakdown yield and the save field: each test must
  fail with the write removed.
- Leave `cargo fmt` and `cargo clippy --workspace --all-targets` clean.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Never push. Never `git checkout`, `stash` or `reset` over uncommitted
  work. Stage explicit paths only.
- Before editing, read `.claude/rules/seams-base.md`, `seams-items.md`
  and `content-schema.md`. Phase B also reads `seams-screens.md`,
  `seams-notifications.md` and `drawing-seam.md`.
- Each phase fits one fresh subagent. Hand it this file, not the history.
  The phases run in order.

## Design (approved)

A **Holding Cell** is a new base structure with one pen cell. You stand
beside it, pick a `DownedProgram` and spend one Reinitialization Protocol.
The program boots as a **live body pinned in the pen**. It jitters like a
research subject and has the role `ProgramRole::Jailed`. A posted
**warden** worker advances progress by 1 per beat. Every
`attempt_ticks` beats, one decompile roll is made:
- On success, the program joins the roster.
- After `JAIL_MAX_ATTEMPTS` failures it **breaks down**: the body
  despawns and a reduced, random extraction-style yield lands in the
  cell's `Stock::output`.

Each failure raises the next attempt's odds through
`TargetResistance::prior_attempts`.

`Game::reinitialize_program`, the reinitialize-anywhere door, is
**removed**.

- **Pin:** you pick from the player's own `DownedPrograms`. It's refused
  for a boss, an unknown species, no protocol, a full roster, or no free
  cell. The body keeps its rarity and carried routine (`SpawnPins`) and
  starts at level 1. The prisoner counts against roster room from the
  pin, so a success can never fail on a full roster.
- **Role:** `Jailed { cell, attempts, progress }` sits on the body.
  `role_of` checks it where `UnderStudy` is checked, before the `Staff`
  fallback.
  **Rule: Jailed behaves exactly like UnderStudy at every site unless
  this plan says otherwise.** That means no `Task`, no sortie, no post,
  and it can't be moved. A cell's occupant is derived with a query, never
  stored on the cell.
- **Odds:** `jail_odds(prisoner)` is the only function that assembles
  `TargetResistance`. It uses `hp_fraction 1.0`, `threat_ratio 1.0`,
  `prior_attempts = attempts` and the species' `taming_difficulty`. It
  also uses `player_decompiler_bonuses()`, and `item_potency =
  JAIL_BASE_POTENCY`, so no catalyst is spent. The UI and the roll both
  call it.
- **Breakdown yield:** a pure `breakdown_yield(...)` **calls**
  `extraction_band` and `extraction_yield` scaled by
  `JAIL_BREAKDOWN_SCALE`. It does not copy them.
- **Save:** `CreatureSave` gets an additive `#[serde(default)] jailed:
  Option<JailedSave { cell_pos, attempts, progress }>`. The cell is
  resolved to a `Position`, as `UnderStudy::station` is
  (`lifecycle.rs:2570`). **No `SAVE_FORMAT_VERSION` bump.** It needs a
  save→load test, not only a RON round trip.
- **RNG:** each attempt and each breakdown takes one `GameRng` draw, and
  only while a cell is running. A save with no cell keeps its stream.

## Decisions (not yet reviewed by the user — flag in the hand-back)

1. **The structure def** gets `#[serde(default)] holds_prisoner:
   Option<PrisonSpec { attempt_ticks: u32 }>` and is documented in
   `assets/structures/README.md`.
   - The asset is `assets/structures/holding_cell.ron`, with footprint 2
     and a cost and tier in line with `research_node.ron`. Copy that
     asset's shape.
   - The starting `attempt_ticks` is 40.
2. **The pen cell:** pull the diagonal-pen rule out of `study::study_pen`
   (`game/base/study.rs:33`) into one shared function that both
   `studies` and `holds_prisoner` structures call.
3. **Tuning starting values:** `JAIL_MAX_ATTEMPTS = 5` and
   `JAIL_BREAKDOWN_SCALE = 0.4`. `JAIL_BASE_POTENCY` equals the shipped
   `ice_breaker`'s potency (read it from the asset and cite it).
   - An odds-curve test asserts that attempt 1 is below 50% for a
     mid-difficulty species, that the chance rises strictly with each
     attempt, and that the total over 5 attempts is above 60%.
   - Re-derive these numbers. Don't trust the targets.
4. **Breakdown tool:** draw one tool uniformly from `ToolDb`, sorted by
   id so the draw is stable, and scale it at tier 1 with no bench bonus.
   Its `yields` decide the items.
5. **The warden:** a worker holding `TaskKind::GatherResource` on the
   cell, which is the Teardown Rig's gate (`game/base/teardown.rs:70`).
   Find the predicate assignment uses to staff the rig and add
   `holds_prisoner` there. Don't build a parallel staffing path.
   With no warden, the cell's `MachineStatus` is "No warden".
6. **Cell destroyed with a prisoner inside:** the body despawns and its
   `DownedProgram` record goes back to the player's list. The protocol is
   lost. This follows the `CarryingProgram` rule at `building.rs:680`,
   where destruction returns the program.
7. **On success:** remove `Jailed`, log a `MessageKind::Outcome` line
   ("X accepts its new parameters and joins your roster."), push a
   notification, and record `Deed::Tamed`. Unlike today's reinitialize,
   this one *is* a decompile.
8. **The UI follows Research Station pinning:** a new `Mode::PinPrisoner`
   built the way `Mode::PinSubject` is (`app-core/src/app/building.rs:519`,
   `gui render::draw_pin_subject`). Inspecting the cell shows the
   prisoner, `attempts/JAIL_MAX_ATTEMPTS`, progress, and the next
   attempt's odds from `jail_odds`. The jitter is `Fx::strain_jitter`,
   applied wherever the GUI applies it for `UnderStudy` today
   (`gui/src/render/base.rs:1140`).
9. **Item text:** the `reinitialization_protocol.ron` description becomes
   "Revives a downed program in a Holding Cell."

## Phase A — engine: pin, role, save (engine only)

**Files:** `components.rs`, `structures.rs`, `game/party.rs`, a new
`game/base/prison.rs` (registered in `game/base/mod.rs`),
`game/base/study.rs` (the pen pull-out), `save.rs`, `game/lifecycle.rs`,
`tuning.rs`, `assets/structures/holding_cell.ron` and its README,
`tests/prison.rs`.

**Interfaces:** `Jailed`, `ProgramRole::Jailed`, `PrisonSpec`,
`Game::jail_blocker(index) -> Option<JailBlock>`,
`Game::jail_program(index) -> Result<(), String>`,
`Game::cell_prisoner(cell) -> Option<Entity>`,
`Game::jail_odds(prisoner) -> Option<f32>`.

**Tests:**
- Each refusal leaves inventory, records and roster untouched.
- A pin spends exactly one protocol and removes exactly one record.
- The body is pinned with its rarity and routine.
- `role_of` returns `Jailed`.
- The prisoner gets no `Task` from the scheduler, joins no sortie, and
  counts against roster room.
- Save→load keeps `attempts` and `progress` on the same body in the same
  cell.
- A save with no cell is RNG-identical. Pin `GameRng` state before and
  after a few beats.

Walk every `UnderStudy` site (≈30, in `party.rs`, `turn.rs`,
`work_orders.rs`, `lifecycle.rs`, `inspection.rs` and `views.rs`). Give
each a Jailed arm or a written reason it needs none. List them in the
hand-back.

**Gate:** `cargo test -p feral-processes-engine`.

## Phase B — engine: the process, removing the old door, and the UI (all crates)

**Engine:**
- `Game::run_holding_cells()`, called from `game/turn.rs` beside
  `run_teardown_rigs` (`turn.rs:384`).
- The pure `breakdown_yield`.
- Decision 6's destruction handling.
- Delete `reinitialize_program`, `reinitialize_blocker` and `ReinitBlock`
  (`game/extraction.rs:904-1000`), with their tests in
  `tests/extraction.rs` (≈65 mentions). Move the refusal coverage that
  still applies to `tests/prison.rs`.
- Fix the doc references in `spawning.rs:303`, `items.rs:147` and
  `combat_rewards.rs`. `SpawnPins` stays, because the cell is its new
  caller.

**app-core:** remove the reinitialize row from `app/extraction.rs:48`
(and its `tests/support.rs` and `tests/extraction.rs` uses). Add
`Mode::PinPrisoner` and its key handler. Add the mode to every
exhaustive mode list; `all-modes` merge conflicts are a known trap.

**gui:** `draw_pin_prisoner`, the cell inspect panel, the jitter for
`Jailed`, and removing the reinitialize row in `render/extraction.rs`.

**Tests:**
- No warden means no progress.
- Each attempt fires at `attempt_ticks` and advances `attempts`.
- A forced success (seeded) gives roster membership, a `Deed`, and the
  marker removed.
- A forced run of failures gives a breakdown with output in `Stock`, the
  body gone, and nothing on the roster.
- The odds curve (decision 3).
- A destroyed cell returns the record.
- app-core: the pin picker spends a protocol on Enter and refuses with
  the blocker's text.
- gui: a headless screenshot through `--template` with a pinned
  prisoner. `capture` a `dev-saves/holding-cell` template for it.

**Gates:**
- `cargo test --workspace`.
- `cargo test -p feral-processes-engine balance_sim`, which must be
  unmoved.
- `cargo run -- --template holding-cell --screenshot out.png`, then Read
  the PNG.

## Final

- An opus whole-branch review, given the diff as a file. It re-derives
  the odds curve and checks the Jailed/UnderStudy site list against the
  source.
- Then CHANGELOG and the release at landing (`skills:deploy`). Update
  `docs/superpowers/INDEX.md`.
