# Floor finishes

**Date:** 2026-09-16
**Status:** design approved, not implemented.
**Todo:** #95, "something synonymous to carving floors or carpeting with
different colors".

A decorative layer on laid base floor: a finish is a colour plus an optional
carved pattern, painted onto tiled cells by the dig crew, moddable as data,
drawn with the dev sprite editor, and worth a small morale lift through the
memory system.

## The sequence

Base space already has two steps, and this adds a third:

1. **Mine out** — a marked solid cell is cut to `BaseCell::Open`.
2. **Tile** — a marked `Open` cell becomes `BaseCell::Floor`, spending one
   `blank_substrate`. This is what holds the cell against entropy's
   reclamation.
3. **Finish** — a marked `Floor` cell takes a finish, spending
   `tuning::FLOOR_FINISH_COST` (**3**) more `blank_substrate`.

Each step is its own job. A finish never goes onto a cell that is not
already floor, and nothing about a finish affects step 2's tile:

- **Stripping** a finish removes the finish and leaves the floor tile. It
  costs nothing and refunds nothing.
- **Replacing** a finish overwrites it. The tile the old finish consumed is
  gone; nothing is refunded.

## Data: `assets/floors/`

One `.ron` per finish, loaded into `floors::FloorDb`:

| Field | Type | Meaning |
|---|---|---|
| `id` | `FloorId` (string newtype) | Save key and default sprite key. |
| `name` | `String` | Brush label and examine line. |
| `description` | `String` | One line of flavour. |
| `shade` | `FloorShade` | A named tint from a fixed, engine-side set. |
| `sprite` | `Option<String>`, `#[serde(default)]` | Sprite key override; falls back to `id`, `@`-prefixed ignored, per `SpeciesDef::sprite_name`. |
| `comfort` | `Option<MemoryId>`, `#[serde(default)]` | The memory a program writes by lingering on this finish. `None` is purely cosmetic. |

There is no `cost` field: every finish costs `FLOOR_FINISH_COST` tiles,
a tuning constant. A per-finish mix (a pigment item, say) is a later,
additive `#[serde(default)]` field if it is ever wanted.

**`FloorShade` is an enum, not an RGB triple**, resolved to a colour in
gui. A free RGB field lets a mod paint a carpet that reads as rock. An
unknown shade name fails that file's parse, which is the standard
skip-and-warn.

The ten shades span the wheel, all dark enough that glyphs and
machine-status outlines stay legible over them (Platform's own argument):

| Shade | RGB |
|---|---|
| `Cobalt` | 0.10, 0.20, 0.55 |
| `Teal` | 0.05, 0.30, 0.32 |
| `Moss` | 0.14, 0.30, 0.12 |
| `Olive` | 0.28, 0.28, 0.08 |
| `Ochre` | 0.40, 0.26, 0.06 |
| `Umber` | 0.36, 0.30, 0.18 |
| `Wine` | 0.32, 0.06, 0.20 |
| `Plum` | 0.28, 0.10, 0.34 |
| `Violet` | 0.20, 0.12, 0.45 |
| `Slate` | 0.20, 0.23, 0.27 |

`Umber` was retuned from an original 0.24, 0.16, 0.10 during implementation:
the separation census below measured it 0.060 from `Entropy` brightened by
2.6 (real shipped rock reaches shade 3.0, where the gap was 0.075) —
functionally the same colour as an ordinary wall. The value above is picked
by the same measurement, maximizing Umber's own minimum distance across a
handful of brown/sand/earth candidates; its nearest neighbour is now
`Olive` at 0.130, and the census's system-wide floor turned out to be
`Wine` against brightened `Entropy` at 0.099 regardless — see
`FINISH_SHADE_MIN_SEPARATION`'s own comment in `terrain.rs`.

**This amends the hue rule for finished floor only.** The map's rule is
cool = walkable, hot = blocked (`terrain.rs`, `biome_tint`). Warm finishes
are admitted because a finish only ever sits on laid tile, so the rim
already separates it from every blocked cell, and each finished cell is
outlined (see Drawing). What a shade must still avoid is being *mistaken
for* something, so the rule becomes a separation census in gui: every
shade is at least a fixed distance from

- an exposed rock face — `Entropy`'s tint brightened by any shade in
  1.0..=4.0 (shipped rock tops out at 3.0; the rest is mod headroom);
- `Excavated` and plain `Platform`;
- the `THREAT` palette role and `Fx::structure_condition`'s damage wash;
- every other `FloorShade`.

A true rust or brick red fails the first and is why `Wine` leans magenta.

**Loading** follows `MemoryDb::load_dir`: an absent directory is an empty
database and a supported install (no finish brush is offered and the game
is exactly today's); a malformed file is skipped with a warning; `iter` is
sorted by id because the brush cycle walks it. `assets/floors/README.md` is
the schema reference.

**Shipped content:** three finishes, at least one with `comfort` set and at
least one without, each with 16x16 near-white art in `assets/sprites/`.

## Storage

`BaseGrid` gains `finishes: BTreeMap<(i32, i32), FloorId>`,
`#[serde(default)]`. `BaseCell` is untouched, so a finished cell is still
`Floor` to every existing reader — `is_floor`, `wander_step`'s leash,
structure placement, and the three arrival-tile checks — with no change to
any of them.

The field is additive under field-named RON, so **no
`SAVE_FORMAT_VERSION` bump**. An entry whose id no loaded `FloorDef`
resolves is dropped on load (logged once), and that cell draws as plain
floor. An entry on a cell that is not `Floor` is dropped the same way.
Nothing in play turns a `Floor` cell back today, but the check costs
nothing and makes "a finish implies floor" a property of the store rather
than of its callers.

`BaseGrid` methods: `finish_at`, `set_finish` (refuses a non-floor cell),
`clear_finish`.

## The verb

The mining tool: `m` in base space opens `Mode::Excavate`, unchanged. Inside
it, **`[F]` cycles the brush**: plain → each `FloorDb` entry in id order →
strip → plain. Uppercase per the row-selector rule; `F` is unbound in that
mode. The screen's header names the current brush. With an empty
`FloorDb` there is nothing to apply and nothing to strip, so the cycle is
just plain: `[F]` does nothing and the header does not mention it.

The box commit, anchor and clear-if-marked semantics of
`Game::toggle_mark_box` are unchanged; it takes the brush and passes it to
`set_mark`, which decides per cell:

| Brush | Solid | `Open` | `Floor`, no finish | `Floor`, finish X | `Floor`, finish Y |
|---|---|---|---|---|---|
| plain | mark: cut | mark: tile | skip | skip | skip |
| finish X | skip | skip | mark: finish X | skip | mark: finish X |
| strip | skip | skip | skip | mark: strip | mark: strip |

`DigSite` carries what the mark asks for: `finish: Option<FinishOrder>`
where `FinishOrder` is `Apply(FloorId)` or `Strip`, `#[serde(default)]`
on both the component and `DigSiteSave`. `None` is today's cut-or-tile
mark. The tile a finish mark spawns on needs no `Durability` damage, so it
is spawned with hp 0, as an `Open` mark already is.

**Clearing a mark** is unchanged: re-committing a box whose anchor is
marked clears every mark in it, of any kind.

## The crew

`dig_wants` and `run_dig_crew` take finish sites through the existing
walk-then-work path. On arrival:

- `Apply(id)`: spend `FLOOR_FINISH_COST` `blank_substrate` (base stores,
  then the pack — `spend_one_substrate` generalised to a count, refusing
  whole before anything moves), then `set_finish`. A site whose cell has
  meanwhile stopped being floor, or already wears `id`, is despawned with no
  charge.
- `Strip`: `clear_finish`, no charge.

An `Apply` site with fewer than `FLOOR_FINISH_COST` substrate available is
**not a want**, the rule
`Game::dig_wants` already applies to a dry tile job through
`build_is_workable`'s question, and it reuses `announce_dig_dry`'s
once-latched line. A `Strip` site is always workable.

Finish wants sit in `dig_wants`' output after cut and tile wants, so under
`truncate(staff.len())` a short-handed base keeps holding its floor before
it decorates it.

## The effect: a comfort memory

`Game::note_comforts`, called from `tick_inner` on the same period as
`Game::note_postings`. For every program standing in base space on a cell
whose finish resolves to a def with `comfort: Some(m)`, it calls
`Game::remember(who, m, MemorySubject::BaseTile(pos))`.

- Morale moves only through memories, so the lift shows on the memories
  page with no new row type, and `strike_cap` and decay already bound it.
  Decay means the lift fades unless the body keeps returning.
- Magnitude is the memory def's `valence`, so a mod's plush carpet names a
  stronger memory than a cheap one; the finish needs no number of its own.
- A positive `BaseTile` opinion cannot trip `drift_idle_staff`'s avoidance
  rejection, which reads only below `MEMORY_AVOIDANCE_THRESHOLD`.
- An unresolvable `comfort` id writes nothing, by `remember`'s existing
  resolve-first rule, so deleting `assets/memories/` still restores the
  pre-memory game.

Shipped: one new memory def, `at_ease_on` (`subject: BaseTile`, positive),
with its row in `MEMORY_TRIGGERS`.

Rejected: a live "room beauty" addend inside `Game::morale` (a second
morale source the memories page cannot explain) and a comfort need (needs
are serviced by amenity structures, not cells).

## Drawing

`render/base.rs` routes a `Floor` cell with a resolved finish to a new
`terrain::draw_finish` in place of `draw_slab`, in three layers:

1. The shade filled over the slab's inset rect.
2. An edge line around that rect in the shade scaled by
   `FINISH_EDGE_LEVEL` (0.5) — the same hue, darker — one sprite-pixel
   wide (`r.w / 16`), so each cell reads as a laid block rather than a
   continuous wash of colour. The edge is the renderer's, not the art's:
   a modded pattern never has to draw one.
3. `Painter::sprite(def.sprite_name(), tint = the shade)`, over the same
   rect. A missing sprite leaves layers 1 and 2, which still read as the
   finish.

The fill is always drawn — this is a pattern over ground, not a substitute
for a glyph — so the test asserts the fill, the edge and the sprite mesh.

The engine exposes the finish through the tile view `Game::view_tiles_at`
already builds (a `finish: Option<FinishView>` carrying shade and sprite
key), so gui never reads `FloorDb` or `BaseGrid`.

The examine line for a finished floor cell names the finish.

The `Mode::Excavate` overlay washes marked cells in `PLAN` as today; a
finish mark and a strip mark take the same wash.

## The sprite editor

`SpriteEditor`'s subject list gains the `FloorDb` entries, after structures
and before the two hardcoded names, keyed on `FloorDef::sprite_name()`.
`SpriteSubject` currently carries a `GlyphColor` for the preview tint; a
floor subject previews against its shade instead, so the subject's tint
becomes a small enum over the two sources rather than a second field.
Saving, `.png.off` and quantisation are unchanged.

## Files

- `crates/engine/src/floors.rs` (new) — `FloorId`, `FloorShade`,
  `FloorDef`, `FloorDb`.
- `crates/engine/src/base_grid.rs` — `finishes` and its three methods.
- `crates/engine/src/components.rs` — `DigSite::finish`, `FinishOrder`.
- `crates/engine/src/save.rs` — `DigSiteSave::finish`; load-time finish
  pruning.
- `crates/engine/src/game/base_space.rs` — brush through
  `toggle_mark_box`/`set_mark`; the crew's finish arm.
- `crates/engine/src/game/base/work_orders.rs` — finish wants in
  `dig_wants`.
- `crates/engine/src/game/memories.rs` — `note_comforts`.
- `crates/engine/src/game/inspection.rs` — `FinishView` on the tile view;
  examine line.
- `crates/app-core/src/app/excavate.rs` — `[F]`, brush state, header.
- `crates/app-core/src/app/sprite_forge.rs` — floor subjects.
- `crates/gui/src/render/base.rs`, `render/terrain.rs` — shade colours,
  `draw_finish`, `FINISH_EDGE_LEVEL`.
- `crates/engine/src/tuning.rs` — `FLOOR_FINISH_COST`.
- `assets/floors/` (new, with README), `assets/sprites/` (three PNGs),
  `assets/memories/at_ease_on.ron`, `assets/memories/README.md`.
- `CHANGELOG.md` at landing; CLAUDE.md seam lines, the `seams` skill and
  the graph, per the three-write rule.

## Tests

Engine:
- `FloorDb`: absent directory is empty; a malformed file is skipped with a
  warning; `iter` is id-sorted.
- `set_mark` per the table above, one assertion per cell of it.
- Crew `Apply`: floor gains the finish and `FLOOR_FINISH_COST` substrate
  is spent; with fewer than that in reach, nothing is spent.
- Replacing: the finish changes, `FLOOR_FINISH_COST` is spent, none
  returned.
- Strip: the finish is gone, the cell is still `Floor`, nothing spent or
  returned.
- A dry `Apply` site is not a want and announces once; a `Strip` site is.
- A cell stopped being floor under an `Apply` site despawns it uncharged.
- Save → load keeps `finishes` and `DigSite::finish` (a real save and load,
  not a RON round-trip, which cannot see a skipped field).
- Load drops a finish with an unknown id and one on a non-floor cell.
- `note_comforts`: a program on a comfort finish writes the memory; on
  plain floor or a comfort-less finish, nothing.
- Asset census: every shipped floor's `comfort` names a shipped memory def.

App-core:
- `[F]` cycles plain → finishes → strip → plain; with an empty `FloorDb`
  it is inert.
- The sprite editor lists floor subjects.

Gui:
- The separation census above, over every `FloorShade`.
- A finished cell draws the fill, the darker edge and the sprite; without
  a sprite, the fill and edge alone.

Gates: `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
`cargo fmt`. `balance_sim` is unaffected (it models no base).

## Known conflict

`feat/social-behaviours-survey` has uncommitted edits to `memories.rs`,
`components.rs` and `tests/assets.rs`. Whichever lands second rebases.
