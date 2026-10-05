# Battle effects library — plan

Spec: `docs/superpowers/specs/2026-10-05-battle-effects-library-design.md`.
Branch `battle-effects-library`. Read the spec once; this plan only adds the
decisions and file list it left open.

**Execution:** 4 phases, one sonnet subagent each, run serially (each builds
on the last's types). Gates off per phase; one final opus whole-branch
review. Each phase: TDD, commit per green step, `cargo fmt`, `cargo clippy
--workspace --all-targets`, then its listed tests. Full `cargo test
--workspace` only at P1 end and P4 end. Every dispatch: **never push**, stage
explicit paths only (`art/` is the user's untracked work — never stage it).

## Decisions taken here

- **No new Resource.** `RoutineCue` lives in a second field of the existing
  `BoltQueue` (`routines: Vec<RoutineCue>`, same cap/drain), avoiding the
  `new-resource-shifts-query-iteration-order` trap.
- `BoltCue` gains `fx: Option<String>` and so loses `Copy` (keeps `Clone`);
  fix whatever call sites the compiler names.
- Routine cells come from `reach::shape_cells(&battle.board, from, aim,
  shape)` (`tactical/reach.rs:373`) — the same cells `recipients` uses.
- The tamper push (`game/tamper.rs:306`) passes `fx: None`: tamper is not a
  weapon swing.
- Unknown-id warning is once per id (a `HashSet<String>` on the library), not
  global.

## P1 — engine (crate: engine, app-core) — schema + cues

Files: `engine/src/items_db.rs` (`ItemDef`, :124), `engine/src/abilities.rs`
(`AbilityDef`, :862), `engine/src/resources.rs` (`BoltCue` :881,
`BoltQueue` :892, new `RoutineCue`), `engine/src/tactical/turn.rs` (push
sites :441 reaction, :669 swing; `run_tactical_routine` :1340),
`engine/src/game/tamper.rs:306`, `engine/src/game/base/upkeep.rs:234`
(`take_bolts`; add `take_routine_cues`), `engine/src/lib.rs` (re-export
`RoutineCue`), `app-core/src/app/battle.rs:262` (auto-resolve drain also
discards routine cues).

Interfaces:
```rust
// ItemDef, AbilityDef
#[serde(default)] pub fx: Option<String>,
// resources.rs
pub struct BoltCue { pub from, pub to, pub color: GlyphColor, pub fx: Option<String> }
pub struct RoutineCue { pub from: (i32,i32), pub aim: (i32,i32),
    pub cells: Vec<(i32,i32)>, pub color: GlyphColor, pub fx: Option<String> }
impl Game { pub fn take_routine_cues(&mut self) -> Vec<RoutineCue> }
```
- Swing/reaction `fx` = wielded weapon's `ItemDef::fx` (find the wielded
  item the way `swing_move_at` does); unarmed → `None`.
- `RoutineCue` pushed in `run_tactical_routine` **after** the cut-off
  (provoke) early return and **before** effects resolve, for every effect
  including Decompile. `from` = actor's cell; `color` = actor glyph colour.
- Rewrite the `BoltCue` doc comment's "No kind field" paragraph to the new
  rule (names a library effect by id; engine never reads it).

Tests (engine, `cargo test -p feral-processes-engine <name>`):
- swing with a weapon carrying `fx: Some("x")` → its `BoltCue.fx == Some("x")`;
  unarmed → `None`.
- tactical `Radius` routine → exactly one `RoutineCue`, `cells ==
  shape_cells(..)`, `fx` == ability's.
- a cut-off (provoked) routine queues no `RoutineCue`.
- items/abilities `.ron` without `fx` still parse (existing loaders' tests
  cover; add one assertion if none does).
- `balance_sim` unaffected (run it once).
Gate: `cargo test --workspace`.

## P2 — gui effect library (crate: gui) — data only, no drawing

Files: new `gui/src/effects.rs` (or `fx/library.rs` if `fx.rs` is split —
implementer's call by size), `gui/src/lib.rs` (load at Startup beside
`sprites::load`, :599), new `assets/effects/README.md` + the six starter
`.ron` files from the spec, `assets/items/README.md`,
`assets/abilities/README.md` (`fx` field), `.claude/rules/content-schema.md`
if it lists asset kinds.

Interfaces:
```rust
pub enum Travel { Streak, Pulses { count: u8 }, Beam { hold: f32 }, Zap, None }
pub enum Impact { Sparks, Explosion { radius: f32 }, Zap, Slash, Smoke }
#[derive(Deserialize)] pub struct EffectDef { id, #[serde(default)] color: Option<GlyphColor>,
    muzzle: bool, travel: Travel /*default Streak*/, impact: Vec<Impact> /*default [Sparks]*/, shake: f32 }
#[derive(Resource)] pub struct EffectLibrary { /* id -> EffectDef, always contains "streak" */ }
impl EffectLibrary { pub fn load_dir(dir) -> Self; pub fn get(&self, id: Option<&str>) -> &EffectDef }
```
`get(None)` and unknown ids → `streak` (unknown warns once). Follow
`sprites::load`'s read_dir pattern (`gui/src/sprites.rs:47`) and the engine
`*Db::load_dir` skip-and-warn rule.

Tests: defaults == today's streak; malformed file skipped; missing dir →
only `streak`; **census**: every `fx:` in `assets/items/` and
`assets/abilities/` resolves in `assets/effects/` (reads real assets; will be
vacuous until P4 assigns — that is expected).

## P3 — gui drawing + shake (crate: gui)

Files: `gui/src/fx.rs` (`Fx` :811, `FrameCues` :837, bolt draw ~:1262,
`camera_offset` :1422, `spark_scatter` :555 as the jitter precedent),
callers that build `FrameCues` (`render/tactical.rs:2435` test etc.). Draw
only through `Painter` (`.claude/rules/drawing-seam.md`). Read
`.claude/rules/seams-tactical.md` first.

- `FrameCues` gains `routines: Vec<RoutineCue>` (from `take_routine_cues`).
- Bolt/routine draw dispatches on the looked-up `EffectDef`: travel
  primitive along `from→to`/`from→aim`; at arrival, impacts at `to`/`aim`,
  area impacts (Explosion, Smoke) on every `cells` entry; muzzle at `from`.
  `Sparks` reuses today's `EffectKind::Hit` burst — don't fork it.
- Each travel primitive is a **pure timing/shape fn** of `(t / BOLT_SECONDS)`
  so tests need no painter. Retain-window for a cue = max(BOLT_SECONDS,
  longest impact lifetime).
- Shake: `shake_energy: f32` on `Fx`; impact adds `def.shake` (capped);
  exponential decay ~0.3 s; offset = hashed wobble × energy, ≤ a fraction of
  a tile (named const), added in `camera_offset` on the battle map only;
  zero when `!enabled`.

Tests (spec's list): pulses all finish inside `BOLT_SECONDS`; beam hold
inside it; explosion ring radius monotone; zap jitter stable for a fixed
(cell, index) across frames; shake stacks additively, caps, decays to 0;
extend `a_disabled_fx_draws_no_bolt` → no pulses/explosion, zero shake.
Gate: `cargo test -p feral-processes-gui` (or crate name per Cargo.toml).

## P4 — content, seams, screenshots, final gate

1. Assign `fx` per spec heuristics (ranged weapon → `laser_pulse`, melee →
   `slash`; `Radius` routine → `explosion`; status/debuff → `zap`; heavy
   single `Damage` → `beam`; buff/heal/utility → none). Append the resulting
   table (file → fx) to this plan under **Assignments**. Census now non-vacuous.
2. Seam update in all three places, order per the `seams` skill: graph
   `seam:` entity, `seams` skill entry, `.claude/rules/seams-tactical.md`
   ("BoltCue has no kind field" → "names a library effect by id; engine never
   reads it; travel finishes inside `BOLT_SECONDS` per primitive").
3. `CHANGELOG.md` unreleased notes only (no version bump on branch).
4. Screenshots: a ranged fight and a blast routine via `cargo run --
   --template <tactical template from dev-saves/README> --keys "…"
   --screenshot out.png`; Read the PNGs. Needs DISPLAY; if absent, say so.
5. `cargo test --workspace`, clippy, `balance_sim`.
6. Final opus review of `git diff main...` (diff written to a file).

## Assignments

_(filled in P4)_
