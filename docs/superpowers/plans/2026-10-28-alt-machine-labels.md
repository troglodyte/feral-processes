# Plan: Alt shows what each machine makes

Spec: [2026-10-28-alt-machine-labels-design.md](../specs/2026-10-28-alt-machine-labels-design.md).
Branch `worktree-alt-machine-labels` (this worktree; spec is its first commit).
Engine + gui. No save, schema, `.ron` or tuning change; no ADR.

Three phases, each one or more green commits. Phase 1 is small, Phase 2
carries the layout and needs no `base.rs` reading, Phase 3 wires it in. For
a subagent per phase, brief it with this file plus the spec, and forbid push,
checking out others' work, `git add -A` and stash.

## Phase 1: engine views (`crates/engine`)

- `views/map.rs`: `EntityView` gains `making: Option<String>`; `FarmTileView`
  gains `crop_name: Option<String>`. `crop` stays.
- `structures.rs`: add `Placed::product_name(self, items: &ItemDb) ->
  Option<String>` (item display name of `production()`, falling back to the
  id). Rewrite `Placed::label` to call it, so the lookup is written once.
- `game/inspection.rs` (~1505): `making: self.placed(entity).and_then(|p|
  p.product_name(&self.items))` — use whatever the `ItemDb` field is called.
- `game/base/farm.rs` (~352): fill `crop_name` from the sown recipe's display
  name.
- Fix every `EntityView {`/`FarmTileView {` literal that stops compiling
  (gui test helpers in `render/building.rs`, `render/base.rs`, and engine
  tests), with `making: None` / `crop_name: None`.

Tests (new or beside the nearest existing view test; `tests/farms.rs` for the
farm one), from a `dev-saves/` template where one has the structures:
set machine → recipe item name; fixed `work` extractor → its `produces`;
research node → "Research Data"; unset machine and Depot → `None`; sown plot →
crop name. Gate: `cargo test -p feral-processes-engine`, `balance_sim`
untouched by construction.

## Phase 2: pure layout (`crates/gui/src/render/alt_labels.rs`)

New module, declared in `render/mod.rs`. No `Painter`, no `Game`; tile units.

```rust
pub(crate) enum LabelKind<'a> { Room { cells: &'a [(i32, i32)], centre: (i32, i32) }, Machine { at: (i32, i32) } }
pub(crate) struct LabelRequest<'a> { pub kind: LabelKind<'a>, pub w: f32, pub h: f32 } // tiles
pub(crate) struct LayoutInput<'a> {
    pub labels: &'a [LabelRequest<'a>], // rooms first, then machines by (y, x)
    pub soft: &'a HashSet<(i32, i32)>,  // glyph tiles + footprint cells
    pub bounds: TileRect,               // visible pane, in tiles
}
pub(crate) enum Placement { Dropped, At { rect: Rect, leader: Option<((f32, f32), (f32, f32))> } }
pub(crate) fn layout(input: &LayoutInput) -> Vec<Placement>; // same order as labels
```

Shape is the builder's call; these are the contracts:

- Greedy in input order; placed boxes are hard obstacles; a candidate also
  inside `bounds`. Soft cells: prefer a candidate covering none; fall back to
  the first that covers some only when no clean one exists at that step.
- Room: candidate rows of `cells` ordered by distance from `centre`'s row
  (ties: above first), centred on the row's widest run nearest `centre`;
  must fit inside room cells (generalise `room_row_cols`/`room_label_fits`,
  which move here with their tests). Never a leader.
- Machine: right, left, above, below (no leader); then rings 2..=6
  (`tuning`-style const `LEADER_RADIUS = 6` at the top of the module — gui
  has no `tuning/`), ring order fixed (builder's call, documented). A leader
  runs from the box's nearest edge to the tile centre; reject one crossing
  any placed box (the spec's "never"), prefer one crossing no placed leader.
- Box sizes include the backing-box padding the draw will use, so the layout
  and the draw agree on what overlaps.

Tests: every bullet under the spec's "Layout, pure", plus: rooms are
unchanged when no machine is near (the move preserves today's placement);
a soft-only-free neighbourhood still places the label. Gate:
`cargo test -p feral-processes-gui alt_labels` (check the crate's package
name in `crates/gui/Cargo.toml`).

## Phase 3: draw (`crates/gui/src/render/base.rs`)

- Replace the room-label loop (~1791) with: build requests (room text via
  `measure_ui(.., ROOM_LABEL_SIZE)` ÷ `tile_px`; machine text from
  `EntityView::making` for in-view structures; one per farm plot from
  `crop_name`, anchored on the plot's first cell in (y, x)), soft set from
  structure glyph tiles plus the `structure_floor` grid already read at
  ~715, bounds from the pane. Only while `reveal` is held.
- Draw: leaders first (`Painter::line`, dimmed `TEXT`), then each machine
  label's dark backing rect, then all text in `TEXT` at `ROOM_LABEL_SIZE`.
  Through `Painter` only (drawing seam).
- Delete the old `room_row_cols`/`room_label_fits` and their tests from
  `base.rs` (moved in Phase 2). `base.rs` must not grow net.
- Draw tests beside `the_map_draws_the_alt_marker_only_while_reveal_is_held`:
  a base with a set machine paints its item name with reveal, not without.
- Screenshot: `FERAL_DEV_ALT=1 cargo run -- --template <base save with
  machines> --screenshot out.png`; Read the PNG, check no overlap.

Final gate: `cargo fmt`, `cargo clippy --workspace --all-targets`,
`cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`), then the opus
whole-branch review. CHANGELOG entry at deploy.
