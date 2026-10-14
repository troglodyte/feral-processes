# Plan: Stack corridor lighting

Spec: `docs/superpowers/specs/2026-10-14-stack-corridor-lighting-design.md`
(read it first — this plan does not restate its maths). Branch:
`stack-lighting`, primary checkout, no worktree. TDD; commit at each green
step; stage explicit paths; never push.

Phases are sized so each fits one subagent context; phase 2 reads phase 1's
interface, not its code.

## Phase 1 — engine: the seed and the direction cross the view

**Files:** `crates/engine/src/views.rs` (`StackView`, ~2220),
`crates/engine/src/game/stack_view.rs` (`stack_view`, ~525),
`crates/engine/src/tests/stack.rs`, `crates/app-core/src/tests/stack.rs:7-15`,
`crates/gui/src/render/stack.rs` (heading at ~253 and the three `StackView`
literals in its tests, ~711/857/866 — compile fixes only).

**Interface:**
- `StackView.facing: stack::Dir` (was `&'static str`); doc says it is the
  party's world facing, `.label()` for display.
- `StackView.light_seed: u64` — doc: the frame's lighting seed, derived per
  frame, never stored; the engine attaches no meaning to it.
- `const LIGHT_SALT: u64` in `stack_view.rs`; seed =
  `self.frame_spec(depth, frames, entrance).salted(&[LIGHT_SALT])`. Pick a
  value distinct from `DESCRIPTION_SALT`/`PASSAGE_SALT` (grep them).
- Readers call `.label()`: gui heading, app-core test helper.
  `FrameMapView.facing` is untouched.

**Tests first (engine):**
1. `light_seed` unchanged across a step and a turn within one frame.
2. `light_seed` unchanged across `Game::save` → `Game::load` (real save path,
   not a ron round trip — look in `tests/support.rs` for a helper first).
3. Different seeds for two depths of one stack and for two entrances.
Start from a `dev-saves/` stack template if a test fixture exists for one
(`tests/support.rs`), not a fresh `Game::new`.

**Gate:** `cargo test -p feral-processes-engine stack`,
`cargo test -p feral-processes-app-core stack`, `cargo build -p
feral-processes-gui`, clippy `--all-targets`, fmt. Commit.

## Phase 2 — gui: light the corridor

**Files:** `crates/gui/src/render/lighting.rs` (visibility only),
new `crates/gui/src/render/stack_light.rs` (+ `mod` line in
`render/mod.rs`), `crates/gui/src/render/stack.rs`.

**Interface (`stack_light.rs`, all `pub(super)`):**
- `lighting::light_from_seed` → `pub(super)`. No other change to
  `lighting.rs`; `LEVEL_MIN`/`LEVEL_MAX`/`Lit` reused as-is.
- `StackLight::new(seed: u64, facing: Dir) -> StackLight` — holds the
  `Light` and the party basis (`facing.delta()`, `facing.right_delta()`).
- `fn wall(&self, side: Side) -> Lit` (`Side::{Left, Right}`; normals `+r`,
  `-r`), `fn face(&self) -> Lit` (normal `-f`),
  `fn floor(&self, depth: usize, lateral: i32) -> Lit` (gradient),
  `fn floor_shadow(&self, row_ahead/…) -> [Option<(Half, Lit)>]` — shape the
  shadow call around what `draw_cell` already has (`row`, `i`, and the
  neighbouring rows of `view.cells`); the rule is the spec's "solid
  orthogonal neighbour inside the cone, toward the light, deepest per half".
- Constants `WALL_SPAN`, `FLOOR_SPAN`, `SHADOW` with *why* comments.

**Wiring (`stack.rs`):** `draw_stack` builds one `StackLight` from
`view.light_seed`/`view.facing` and passes it into `draw_cell` (replace the
`(power, max_power)` tuple param with a small context struct if the arg list
passes clippy's limit). Every geometry colour becomes
`lit.apply(dim(base, s))`; the shadow half is a second `poly` over the
floor. Marks, heading, `standing_on`, `VOID`, `UNLIT` stay unlit.
`cell_mark` untouched.

**Tests first (`stack_light.rs`):**
1. North vs South swaps the brighter flank wall; East vs West likewise (pick
   seeds whose `toward` has a non-trivial component on that axis — search
   seeds in the test, don't hardcode a lucky one without asserting why).
2. Face ahead brighter with the light behind the party than in front.
3. Every `Lit` level in `[LEVEL_MIN, LEVEL_MAX]` over a sweep of seeds × four
   facings.
4. Floor shadow: solid neighbour toward the light shadows its half; away
   from the light, or absent from the cone, does not.
5. `stack.rs`: strengthen
   `the_unlit_fill_is_lighter_than_void_and_darker_than_the_far_wall` to the
   worst lit case (`LEVEL_MIN` × darkest `LIGHT_CASTS` channel); add lit
   `DOOR`/`SEALED` stay distinguishable from lit `WALL` by dominant channel
   across the band. Existing no-panic draw tests keep passing.

**Mutation check:** zero `WALL_SPAN`, then zero `SHADOW` (each committed
first, per memory) — a test must fail each time; revert.

**Gate:** `cargo test -p feral-processes-gui`, clippy `--all-targets`, fmt.
Commit.

## Phase 3 — see it, gate, review

- Screenshots: `cargo run -- --template stack --keys "Right Right"
  --screenshot a.png` and a second at another facing/depth; Read both. Lit vs
  shaded walls visible, marks still readable, no hard-black holes.
- `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`).
- Whole-branch review (opus), diff handed over as a file; tell the reviewer
  the rules to check: Stack seam "no `GameRng` in world gen", "mirrors must be
  a call" (one `light_from_seed`), drawing seam (no backend in `render/`),
  marks unlit.
- CHANGELOG entry text drafted for deploy (no version bump on the branch).
- Seams: if review agrees the "lighting is visual-only and seeded off the
  frame" rule is load-bearing, add it via the `seams` skill's three writes.

**Dispatch notes (if subagents):** Sonnet per phase; forbid `git push`,
`git checkout`/`stash` of others' work, `git add -A`.
