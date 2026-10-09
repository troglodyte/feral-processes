# Plan: battle-map ground textures

Spec: `specs/2026-10-09-battle-ground-textures-design.md` (TODO #123).
Branch `battle-ground-textures`, **primary checkout** (the user plays from
it — no worktree). Three phases, each its own commit(s) at green. One
implementer can run all three in sequence; phase 2 needs phase 1's field,
phase 3 needs phase 2's keys.

Every dispatch: no push, no `git checkout`/`stash`/`reset` of uncommitted
work, stage explicit paths only.

## Phase 1 — engine: `TacticalView::ground`

Files: `crates/engine/src/tactical/view.rs`, tests in
`crates/engine/src/tests/tactical.rs` and `tests/siege.rs`.

- `TacticalView` gains `pub ground: Biome` (doc: renderer's textures only;
  a siege reads `Platform` because its board is the base's floor).
- Set in the builder (`view.rs` ~line 361, where `battle` is read):
  `if battle.siege_pack > 0 { Biome::Platform } else { battle.spec.biome }`.
  `siege_pack > 0` is **the** "is this a siege" signal
  (`.claude/rules/seams-sieges.md`) — do not add a flag or a resource.
  If `siege_pack` is `pub(crate)` it is already visible here.
- Check whether app-core re-exports or mirrors `TacticalView`; it should
  pass through untouched. Fix any struct literal that now fails to compile.

Tests (TDD, write first):
- Field fight on a Backplane `BattleSpec` → `view.ground == Backplane`.
- Siege (existing helper in `tests/siege.rs`, ~line 838) → `Platform`,
  although its spec biome is the surface tile's.
- Siege after `persist::assemble` → `restore` → still `Platform`
  (`restore` sets `siege_pack` from the save; assert it, don't assume).

Gate: `cargo test -p feral-processes-engine tactical siege`, clippy.

## Phase 2 — gui: draw the texture

Files: `crates/gui/src/render/tactical.rs` (cell loop ~557-600, tests
module ~1456), `crates/gui/src/render/hud/palette.rs`.

Interfaces (all pure, beside `cell_color`):
- `fn ground_sprite_keys(biome: Biome, kind: BattleCell) -> [String; 2]` —
  `["ground_<biome>_<kind>", "ground_<kind>"]`. One word per biome, so a
  key has exactly two underscores: `opengrid`, `deadlock`, `nullsector`,
  `backplane`, `platform`, `excavated`, `datavoid`, `blackice`, `entropy`;
  kinds `open`, `rough`, `cover`, `blocked` (`ground_backplane_cover`).
  Both mappings exhaustive matches, no `_` arm.
- `fn ground_tint(biome, kind) -> Color` = `ground_level(kind)` x
  `palette::biome_tint(biome)`, channel-wise, alpha 1.
- `ground_level(kind)`: grey levels, starting values Blocked 0.18, Open
  0.42, Rough 0.52, Cover 0.78 — tune by screenshot in phase 3.
- `palette::biome_tint(biome) -> Color`: exhaustive; low-saturation casts
  near 1.0 (e.g. Backplane faint cyan-green, Deadlock faint rust, NullSector
  faint violet, OpenGrid neutral-cool, Platform/Excavated neutral-warm);
  unwalkable biomes neutral `(1,1,1)`.

Draw: after the `cell_color` fill, `painter.sprite(key, px, py, tile_px - 1.0,
ground_tint(view.ground, kind), 0)` for each key until one returns `true`.
Nothing else in the loop changes; washes stay on top. Ignoring the bool
for the second key is fine (`#[must_use]` → `let _ =` or `||` chain).

Tests (write first, headless, follow the module's existing recording-Painter
tests):
- Key order for every `(biome, kind)`: biome key first, default second.
- For every walkable biome, luminance of `ground_tint` is strictly
  Blocked < Open < Rough < Cover.
- Every `biome_tint` saturation (max-min channel) under a named const
  bound, e.g. `0.12`.
- Draw: with a sprite registered under the biome key, the cell draws that
  sprite; with only the default registered, the default; with neither, only
  the fill rect — no sprite call succeeds. If the test painter cannot
  register sprites, assert on attempted keys instead and say so.

Gate: `cargo test -p feral-processes-gui`, `cargo clippy --workspace
--all-targets`, `cargo fmt`. Mutation-check the brightness-order test
(swap two levels → red).

## Phase 3 — art, docs, look at it

Files: new script `scripts/ground-tiles.py` (stdlib only: `zlib`,
`struct`; writes RGBA PNGs), 24 PNGs in `assets/sprites/`,
`assets/sprites/README.md`, `CHANGELOG.md` (unreleased section only if the
file's convention has one — else leave to release).

- Script writes 16x16 near-white tiles (values ~0.6-1.0, alpha 255 except
  Blocked's void may be darker), deterministic (seeded per name). Kind
  shapes and biome motifs per spec "Art". It refuses to overwrite an
  existing PNG unless `--force`, so a Sprite Forge redraw is not clobbered.
- Run it; commit the PNGs and the script together.
- README: a "Ground tiles" section — key pattern, the fallback order, that
  art is near-white and tinted, that a missing tile falls back to the flat
  colour.
- Screenshots (memory `battle-screenshot-goes-through-the-arena`):
  `DISPLAY=:0 FERAL_DEV_ARENA=1 cargo run -- --keys "R l Down x7 Enter f"
  --screenshot out.png` loads `full-group.ron` (Backplane field). For
  Deadlock, copy that scenario to the scratchpad with `encounter:
  Some(Field(biome: Deadlock))` if the arena menu can load an arbitrary path;
  otherwise a temporary edit not committed. Siege: `--template` with an
  active siege if one exists in `dev-saves/README.md`, else skip and say so.
  Read each PNG; judge kinds distinct and Move/Aim/Danger washes legible.
  Adjust `ground_level`/`biome_tint`/art, re-shoot. Send the final shots to
  the user.

Gate: `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`), clippy, fmt,
`balance_sim` untouched (no tuning change). Then the final whole-branch
review (opus), then `skills:deploy` on the user's word — patch bump.
