# Sprites from the defs implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A species or structure can name a one-cell sprite, and the loader
finds it by reading `assets/sprites/` instead of a hardcoded list.

**Architecture:** One `#[serde(default)]` field on each of two def types,
resolved against the def's id by a single method; the resolved name rides
`EntityView` to the renderer, which cannot read asset databases; and the gui
loader scans the directory it already knows the path to.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine), bevy 0.19 + bevy_egui 0.41
(gui), serde/RON.

**Spec:** `docs/superpowers/specs/2026-09-03-sprites-from-the-defs-design.md`
— read it first; this plan argues from it and does not restate its reasoning.

## Global Constraints

- **This plan does not contain the implementation.** Per `CLAUDE.md`'s
  process-weight rule, it carries files, interfaces and the intent of each
  test. Write the code; do not re-emit this.
- **It must ship with no art and no visible change.** All 47 shipped defs
  fall back to their glyph, exactly as today. A test that only passes once
  someone draws something is the wrong test.
- **TDD, failing test first, at every task.** Commit per green step.
- **Moddability rules bind this change directly.** A new def field is
  `#[serde(default)]` so existing mod files keep parsing; a malformed asset
  is skipped with a logged warning, never a panic; and the matching
  `assets/*/README.md` is updated in the *same* change as the schema.
- **Sprites are 16x16 RGBA, nearest-sampled.** `ImageSampler::nearest()` is
  load-bearing — bevy_egui binds the image's own sampler and bevy's default
  is linear.
- **`crates/gui/src/paint.rs` is the only file that may name a graphics
  library.** `render/` draws through `Painter`.
- **The overdraw rule:** a sprite *substitutes* for a glyph and never draws
  beside it. Assert the absent glyph, not only the mesh.
- **No save-format change.** Nothing here is stored; `SAVE_FORMAT_VERSION`
  must not move.
- `cargo fmt` and `cargo clippy --workspace --all-targets` after every task.
- **`cargo test --workspace` is the final gate.** Known noise, not yours: 4
  pre-existing `unused Result` in `crates/engine/src/tests/construction.rs`,
  ~29 other pre-existing warnings in untouched files, and an intermittent
  failure in the engine's labour-scheduler family (`level_up::…`,
  `work_orders::…`, `chains::…`, `creation::an_unspent_allowance_holds_its_step`)
  caused by unstable bevy query iteration order — rerun, and say you saw it.
- Stage explicit paths. **Never `git add -A`.** **Never push, merge or tag.**

---

### Task 1: The field, and what a def's sprite is called

**Files:**
- Modify: `crates/engine/src/species.rs` (`SpeciesDef`, ~line 225, beside `glyph`)
- Modify: `crates/engine/src/structures.rs` (`StructureDef`, ~line 225, beside `glyph`)
- Modify: `assets/species/README.md`, `assets/structures/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `SpeciesDef::sprite: Option<String>`, `#[serde(default)]`
  - `StructureDef::sprite: Option<String>`, `#[serde(default)]`
  - `SpeciesDef::sprite_name(&self) -> &str`
  - `StructureDef::sprite_name(&self) -> &str`

Both methods return the override when set and the def's own id otherwise.
`SpeciesId` and `StructureId` are both `String` aliases, so `&str` is the
natural return and no allocation is needed.
**The fallback is written once per type and nowhere else** — a reader that
does `def.sprite.clone().unwrap_or(id)` itself is a second copy of the
convention, and the copy that drifts is the one nobody runs.

Author no `sprite:` value in any shipped `.ron` file. Every shipped def uses
the convention, and the field ships exercised only by tests — which is the
point: it is an escape hatch, not the normal path.

- [ ] **Step 1: Write the failing tests.** Their intent:
  - `sprite_name()` returns the id when the field is absent, and the
    override when present, for each of the two def types.
  - **The census the spec is built around:** every shipped def that authors
    a `sprite:` override resolves to a file that exists under
    `assets/sprites/`. A def with no override is skipped — a
    convention-named sprite that is absent is the normal case and must stay
    silent. Write it so it still passes with zero overrides shipped (it
    should, vacuously) but fails the moment one names a missing file; say in
    your report how you confirmed that second half, since a census nobody
    can make fail is not a census.
  - An existing `.ron` file with no `sprite:` key still parses.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-engine sprite`
- [ ] **Step 3: Add the fields, the two methods, and both README sections.**
- [ ] **Step 4: Run and watch pass**, then `cargo test -p feral-processes-engine assets`. `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 2: The name reaches the renderer

**Files:**
- Modify: `crates/engine/src/views.rs` (`EntityView`, beside `glyph`)
- Modify: `crates/engine/src/game/inspection.rs` (the three sites that build
  an `EntityView` — around lines 1026, 1477 and 1608; find them all rather
  than trusting those numbers)
- Test: alongside the existing view tests in `crates/engine/src/tests/`

**Interfaces:**
- Consumes: `sprite_name()` from Task 1.
- Produces: `views::EntityView::sprite: Option<String>` — the resolved name
  for a species or structure entity, `None` for anything that is neither.

`Option<String>` and not an interned handle, deliberately: views are rebuilt
every frame, entities on screen number in the dozens, and the principle is
no optimization ahead of evidence. Do not reach for `Arc<str>` here.

**There are three construction sites and they do not all have a def in
hand.** Resolve what each one actually knows; a site with no def resolves to
`None` rather than inventing a name.

- [ ] **Step 1: Write the failing tests.** Their intent: a wild creature's
  view carries its species' resolved name; a structure's view carries its
  structure's; an entity that is neither carries `None`; a def with an
  override carries the override rather than the id.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-engine sprite`
- [ ] **Step 3: Add the field and populate it at every construction site.**
- [ ] **Step 4: Run and watch pass**, then the whole `cargo test -p feral-processes-engine`. `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 3: The loader reads the directory

**Files:**
- Modify: `crates/gui/src/sprites.rs` (delete `SPRITES`, scan instead)
- Modify: `crates/gui/tests/sprites.rs`

**Interfaces:**
- Consumes: nothing from Tasks 1-2.
- Produces: a `SpriteTable` whose keys are the file stems of every loadable
  PNG in `assets/sprites/`.

**The asset root is already in hand and must not be re-derived.**
`asset_plugin` in `crates/gui/src/lib.rs` feeds `assets_dir()` to
`AssetPlugin::file_path`; the scan reads `assets_dir().join("sprites")`, and
bevy keeps loading by the same relative path it does today. Any other way of
finding that directory is a second site deciding a runtime path, which
`crates/launcher/src/paths.rs` exists to prevent.

**This inverts the failure mode, which is the point.** After the scan the
table holds exactly what is on disk, so a name with no file behind it is
unreachable and a def with no art is silent. Registration still gates on
`LoadState::Loaded` — a `TextureId` minted before the pixels exist draws an
unbacked quad for the first frames of a run.

**Delete `every_sprite_the_loader_asks_for_is_on_disk`.** It exists to catch
a `SPRITES` entry with no file, and that state no longer exists; kept, it
would assert the shipped directory against itself and prove nothing. The
no-cruft rule applies to tests.

- [ ] **Step 1: Write the failing tests.** Their intent: the table holds
  every PNG in the directory, keyed by file stem; a missing directory loads
  an empty table with no panic; a non-PNG file in the directory is ignored;
  a malformed image is skipped with a warning and the rest still load.
  Deleting `assets/sprites/` must restore the glyph map exactly — that is
  the property the whole seam rests on.
  - **`the_shipped_sprites_are_one_cell` stays, and now guards every file
    the scan finds** rather than every name a const listed. It is the only
    thing standing between a 24x24 PNG dropped into the directory and art
    that resamples into mush at some zoom, and the scan widens what it has
    to cover rather than narrowing it.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-gui sprite`
- [ ] **Step 3: Replace the const with the scan; delete the obsolete test.**
- [ ] **Step 4: Run and watch pass**, then `cargo test -p feral-processes-gui`. `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 4: Any entity can wear a sprite

**Files:**
- Modify: `crates/gui/src/render/base.rs` (the tile loop's sprite lookup)
- Test: `crates/gui/src/render/base.rs`'s existing test module

**Interfaces:**
- Consumes: `EntityView::sprite` (Task 2), the scanned table (Task 3).
- Produces: nothing later tasks read.

Today the lookup asks `is_player` for a `PlayerLook` name and hardcodes
`"anchor"`; it now asks the view for any entity. **The player's fallback is
three rungs and still outranks this** — drawn icon, then named sprite, then
glyph — while everything else is two, sprite then glyph. **The anchor keeps
its hardcoded name**: it has no def to carry a field, and that site already
documents why it is named in Rust.

- [ ] **Step 1: Write the failing tests.** Their intent: a creature whose
  species has art draws the sprite **and not its glyph** (assert the absent
  glyph — the overdraw rule, and the reason it matters is that art with one
  transparent pixel exposes a glyph still drawn underneath); a creature
  whose species has no art draws its glyph; a structure behaves the same
  both ways; the player's drawn icon still wins over everything; the anchor
  still draws `anchor`.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-gui`
- [ ] **Step 3: Ask the view for the name at that site.**
- [ ] **Step 4: Run and watch pass.** `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 5: Documentation and the full gate

**Files:**
- Modify: `assets/sprites/README.md` — what names a sprite is looked up
  under now, and that art for a species or structure is authored near-white
  because it is tinted by multiplication (the drawn player icon remains the
  one exception).
- Modify: `docs/seams.md` — the drawing-seam entry, if the loader's rule is
  now stated inaccurately there.
- Modify: `.claude/skills/seams/references/hud.md` — same test.
- Modify: `CLAUDE.md` if its "The drawing seam" paragraph is now inaccurate;
  it and `AGENTS.md` are **gitignored twins**, so edit `CLAUDE.md`, `cp` it
  to `AGENTS.md`, and commit neither.

No `CHANGELOG.md` entry and no version bump — this repo bumps, writes the
section and tags once, at the merge.

- [ ] **Step 1: Write the docs.**
- [ ] **Step 2: Run the full gate.** `cargo test --workspace`, `cargo clippy
      --workspace --all-targets`, `cargo fmt --check`. Report real numbers
      and any failure text; do not claim green without the output.
- [ ] **Step 3: Commit.**
- [ ] **Step 4: Stop.** Landing is the user's call.

---

## What this plan does not do

It draws no art, so nothing on screen changes and nobody can look at this and
see whether it worked. The first real proof is one PNG dropped into
`assets/sprites/` named after a species id — worth doing by hand once, with
the user watching, before anyone draws 47 of them.
