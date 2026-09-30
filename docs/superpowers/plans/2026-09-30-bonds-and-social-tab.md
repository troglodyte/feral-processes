# Plan: bonds and the SOCIAL tab

Spec: `docs/superpowers/specs/2026-09-30-bonds-and-social-tab-design.md`
(§ numbers below are the spec's). Branch `bonds-social-tab`.

**Every task:** TDD — failing test first, then the code; `cargo fmt`,
`cargo clippy --workspace --all-targets` clean; commit at green. Iterate
with `cargo test -p <crate> <name>`. No push. Placeholder numbers are the
spec's; do not tune.

## Phases

Three phases, each a fresh context (one subagent per phase). A phase
reads the spec, this plan, and only the files it names.

| Phase | Tasks | Crates | Gate at the end |
|---|---|---|---|
| 1 | 1–3 | engine, assets | `cargo test -p feral-processes-engine` |
| 2 | 4–5 | engine, assets | `cargo test -p feral-processes-engine` |
| 3 | 6–8 | app-core, gui, docs | `cargo test --workspace` |

Then: final whole-branch review (opus), fixes, `balance_sim` check.

---

## Phase 1: bands, avoidance, witnessing, departures (engine)

### Task 1. `Bond` and `Game::bond` (§2)

- New `crates/engine/src/bonds.rs`, `pub mod bonds;` in `lib.rs`.
  `enum Bond { Enemy, Rival, Neutral, Friend, Close }`,
  `pub fn band(f32) -> Bond`, queries `avoids`, `grieves`, `relieved`,
  `label` — exhaustive matches, no `_` arm. Model on
  `settlements/relations.rs:154-330`.
- `tuning.rs`: `BOND_ENEMY_AT`, `BOND_RIVAL_AT`, `BOND_FRIEND_AT`,
  `BOND_CLOSE_AT`, beside `MEMORY_AVOIDANCE_THRESHOLD` (~4483).
- `Game::bond(holder: Entity, about: ProgramId) -> Bond` in
  `game/memories.rs` beside `opinion_of` (606).
- Tests (in `bonds.rs`): half-open at all four edges; a zero opinion is
  Neutral. One engine test that `Game::bond` reads `opinion_of`.

### Task 2. Avoidance and witnessing (§3)

- Avoidance: `drift_idle_staff` (`game/base/work_orders.rs:2234`), a
  rejection after the `BaseTile` one (~2432): skip a tile 8-adjacent to a
  staff body whose `ProgramId` the worker's `bond(..).avoids()`. Use
  `drift_idle_staff_for_test` (2459) in tests.
- Witnessing: `close_brawl` in `game/base/tantrum.rs` (~120-145). Hoist
  the aggressor `ProgramId` read; write `saw_turn_on` on every other staff
  body within `BOND_WITNESS_REACH` (tuning, 2) Chebyshev of the victim.
- Asset `assets/memories/saw_turn_on.ron` (subject Program, valence −3.0,
  strike_cap 3; copy `turned_on_me.ron`'s shape). Add to
  `MEMORY_TRIGGERS` (`tests/assets.rs:2957`).
- Tests (`tests/tantrums.rs`, and the file holding drift tests —
  `rg -l drift_idle_staff_for_test crates/engine/src/tests`): rival
  neighbour declined, friend neighbour accepted, declined body stays put;
  witness in reach gets it, out of reach / aggressor / victim do not.

### Task 3. Departures (§4)

- `bonds.rs`: `pub enum Departure { Fell, LetGo, Fused }`.
- `game/memories.rs`: a `remember` sibling taking an explicit
  `subject_name` (look at `remember` at 64 and `remembered_name` ~658);
  `Game::note_departure(id: ProgramId, name: &str, how: Departure,
  exclude: &[ProgramId])` — holders from `owned_pets()`, minus `id` and
  `exclude` (the fusion co-parent); holders with no memory about `id`
  skipped; `grieves()` → the kind's def, `relieved()` → `rid_of`.
  Its doc lists the six doors.
- Assets: `lost_in_battle`, `let_go`, `became_part_of`, `rid_of` (§4
  table, strike_cap 1). `MEMORY_TRIGGERS` rows.
- Wire the six doors (§4 table). Read the id and name **before** despawn:
  - `dissolve_tamed_program` (`game/trade.rs:589`) gains a `Departure`
    arg; callers `sell_companion` (~733), `extract_routine`
    (`game/routines.rs:~762`) pass `LetGo`, `bench_or_dissolve`'s
    Permadeath arm (652) passes `Fell`. Fix its false doc comment.
  - `fuse_companions` (`game/party.rs:~1481`): each parent `Fused`,
    excluding the other.
  - `consume_site` (`game/base/construction.rs:466`): from the site's
    `BuildSite.program` snapshot, `LetGo`. `return_build_holdings`
    (building.rs:1214) writes nothing.
  - `settle_research` (`game/unlocks.rs:~1334`): `LetGo`.
- Tests (new `tests/bonds.rs`, registered like its siblings): per door, a
  friend gets the right def, a rival `rid_of`, a neutral holder nothing;
  commit-then-refund writes nothing, finished build writes `let_go`;
  Forgiving battle death writes nothing; fusion co-parent not a holder;
  `GameRng` stream unmoved across a brawl + departure (find the
  `run_routes` stream test for the pattern); empty memory catalogue →
  departures write nothing and do not panic.
- **Mutation check** the door wiring: remove one door's call, confirm its
  test fails.

**Phase 1 gate:** `cargo test -p feral-processes-engine`.

---

## Phase 2: known-for and the view (engine)

### Task 4. `known_for` (§5)

- `MemoryDef` (`memories.rs:75-105`): `known_for: Option<String>`,
  `#[serde(default)]`. Add to four defs (§5 table).
- `Game::known_for(e) -> Vec<String>`, ≤ 2, distinct phrases, heaviest
  magnitude first, summing only **other** owned programs' `Program(this)`
  memories via the opinion read.
- Tests: reads others not self; two defs same phrase → one entry; cap 2;
  none → empty.

### Task 5. `Game::social` (§6)

- `views.rs`: `SocialView`, `RelationshipRow { name, bond, opinion, gone }`.
  `Game::social(e) -> Option<SocialView>`: `None` unless owned program;
  rows = memories grouped by `Program` subject, sorted by `|opinion|`
  desc; `name` live short label, else stored `subject_name` with
  `gone: true`.
- Tests: `None` for player/wild; ordering; a departed friend row is
  `gone` and keeps its name.

**Phase 2 gate:** `cargo test -p feral-processes-engine`.

---

## Phase 3: tab, page, docs (app-core, gui)

### Task 6. app-core tab state (§6)

- `App::manifest_tab: ManifestTab { Stats, Social }` near
  `pending_manifest` (`lib.rs:~2632`). `GameKey::Tab` in
  `handle_manifest_key` (`app/inspection.rs:267`) toggles only when
  `game.social(subject).is_some()`; paging keeps it; `leave_manifest`
  resets. `ALL_MODES` / transition table untouched.
- Tests: toggle; paging keeps; leave resets; no-op on player/wild.

### Task 7. gui (§6)

- `render/manifest.rs` `draw_header` (182): right-aligned tab strip on
  the title line when `social` is `Some`. When `manifest_tab == Social`,
  `draw_manifest` (50) calls new `render/social.rs` instead of the section
  grid. Draw only through `Painter` (read `.claude/rules/drawing-seam.md`).
- Censuses (model on `the_real_worst_case_pages_fit_the_tightest_window`,
  manifest_layout.rs:529, and `no_memories_row_is_cut_to_fit_its_column`,
  manifest.rs:1754): tallest SOCIAL page fits; no relationship row cut;
  every shipped `known_for` fits; widest title + strip fits the header at
  the narrowest window.
- Screenshot check: capture a dev-save with bonds, run
  `--template <name> --keys "…" --screenshot` and Read the PNG.

### Task 8. Docs and dev-save

- `assets/memories/README.md`: `known_for`, the five new defs.
- `cargo run --bin savetool -- capture` a save with a friend, a rival and
  a departed program → `dev-saves/` (do not edit the root README).
- `docs/superpowers/INDEX.md`: flip the spec row to built.
- CHANGELOG at release, not here.

**Phase 3 gate:** `cargo test --workspace`, clippy clean.

## After the phases

Final whole-branch review (opus), diff given as a file. Then
`cargo test -p feral-processes-engine balance_sim` (should be unmoved —
no tuning of combat). Not played at the keyboard until the user does.
