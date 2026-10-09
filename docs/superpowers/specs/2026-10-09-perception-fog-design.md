# Perception: fog of war on the surface map

**Date:** 2026-10-09

## Goal

A new derived stat, **Perception**, sets how far the player sees on the
surface tile map. Inside that radius the map is live. Outside it the player
sees only what they remember: dimmed terrain and the landmarks they have
already seen. Ground they have never seen is black. The more Perception, the
wider the circle.

## What the player said (decisions)

- **Derived, not a seventh attribute.** Perception is a `DerivedStat` fed by
  the existing attributes, so it spends no new points.
- **Analysis supplies 75% of it and Entropy 25%**, measured over each
  attribute's typical swing, its `spread`. Analysis is the stat you buy for
  sight, and Entropy is a small luck-flavoured nudge.
- **Baseline sight is slightly wider than the starting base**:
  `MAX_BUILD_DISTANCE_FROM_HOME` is 4, so baseline Perception is 5 tiles.
- **No cap.** More Perception keeps widening the circle.
- **The player's own Perception only.** Companions don't scout.
- **A plain circle**, with no line of sight. Walls don't block it.
- **Outside the circle, everything is remembered and dimmed.** Live things
  (creatures) show only in sight. Settlements you have seen stay drawn dim,
  and your own outposts always show.
- **Per-tile memory**, saved. Never-seen ground is black.
- **Surface tile map only.** Base space, zones, Stack frames and the
  chunk-scale world map overview are unchanged.
- **Battles are unchanged.** Enemy rows keep their exact numbers. (Hiding
  battle numbers behind Perception was considered and dropped.)

## Non-goals

- Line of sight, and terrain that blocks sight.
- Perception from companions or the party as a whole.
- Fog in base space, zones, Stack frames or the world map overview. The
  overview keeps its own chunk fog, `ExploredChunks`, which is unaffected.
- Any change to what the simulation does. Unseen creatures still hunt, aggro,
  path and start battles. The fog changes only what the player is shown and
  can address.

## Design

### 1. The stat

- `DerivedStat::Perception` is appended to the enum in `attributes.rs`, with
  the label `"Perception"`. Like `Extraction`, it is an `f32` carried
  through `DerivedBase`, `DerivedStats` and the `Derived` component. Its
  unit is **tiles of sight radius**.
- New `tuning.rs` constants:
  - `PERCEPTION_BASE_RADIUS: f32 = 5.0`. Documented against
    `MAX_BUILD_DISTANCE_FROM_HOME`, but **not computed from it**: the
    starting base may grow, and sight shouldn't grow with it.
  - `PERCEPTION_MIN_RADIUS: f32 = 2.0`. A floor, so a character rolled low
    in both attributes still sees the tiles beside them. `derive` clamps to
    it, as other derived stats are clamped.
- `DerivedBase::player()` and the program base both start at
  `PERCEPTION_BASE_RADIUS`. Only the player's value is read.
- Effects, data only:
  - `analysis.ron` gains `(stat: Perception, per_point: 0.5)`.
  - `entropy.ron` gains `(stat: Perception, per_point: 0.033)`.
  - Each file's `does:` sentence is updated to match.

  The fit: one `spread` of each attribute should move sight by 2 tiles,
  split 75/25. Analysis has a spread of 3, so 0.75 × 2 / 3 = 0.5 per
  point. Entropy has a spread of 15, so 0.25 × 2 / 15 ≈ 0.033 per point.
  A level's `STAT_POINTS_PER_LEVEL = 6`, all spent on Analysis, buys +3
  tiles.
- `assets/attributes/README.md` lists `Perception` among the `stat:`
  values.
- **Points screen:** `render/points.rs::effect_text` gets an explicit
  `Perception` arm reading `Perception 5.0 → 5.5 tiles`, to one decimal.
  The `_` arm would round 5.0 → 5.5 to "5 → 6".

### 2. Sight: one engine predicate

There is one engine answer to "what can the player see here", and every
player-facing reader uses it:

```rust
pub enum Sight { Unseen, Remembered, InSight }

impl Game {
    pub fn perception_radius(&self) -> f32;          // the player's Derived.perception
    pub fn sight_at(&self, pos: (i32, i32)) -> Sight; // surface only
}
```

- `InSight` means `dx² + dy² <= r²` from the player's current `Position`.
  It is computed live, never read from memory, so a teleport or travel
  never leaves a gap.
- `Remembered` means not in sight, but marked in `SeenTiles`.
- `Unseen` means everything else.
- **Off the surface, `sight_at` returns `InSight` everywhere.** Base space,
  zones and the Stack never fog. Answering in the predicate keeps every
  caller free of locale checks. This mirrors `require_surface`'s split.
- `FERAL_DEV_REVEAL`, already read in `stack_view.rs`, also makes
  `sight_at` return `InSight`. One dev switch reveals everything.

**What each sight state shows.** This is decided by one engine function,
`views::shown_at(sight, &EntityView) -> bool`, beside `drawn_on_surface_map`,
which both the gui and app-core call:

| Thing | InSight | Remembered | Unseen |
|---|---|---|---|
| Terrain | live | dim | black |
| Settlement / surface structure (`is_structure`) | live | dim | hidden |
| Player, party, wielded (`is_player` / `is_companion`) | live | live | live |
| Outposts (`outpost_marks`) | live | live | live |
| Every other entity (wild creatures, bosses, nemesis) | live | hidden | hidden |

The party is always shown because it is always within sight by
construction. The rule is still listed explicitly, so a companion trailing
past a small radius can't vanish.

**Readers that must use `shown_at` / `sight_at`.** Each one leaks a hidden
creature today:

- `gui/src/render/base.rs::draw_surface_map`. It filters `entities`, so the
  Threat readout, con colour, nemesis mark and target outline all follow,
  because they derive from the same `Vec`.
- `Game::find_target_in_direction` (examine, `inspection.rs`). Examine names
  only what is shown. Its own 12-tile `EXAMINE_RANGE_TILES` is unchanged, so
  with high Perception you can see further than you can examine.
- `app-core/src/app/hover.rs`: the hover label.
- `app-core/src/app/travel.rs`: click-travel chooses a `TravelGoal::Creature`
  only for a shown hostile. Chase's goal lookup in `base.rs` likewise.

Tactical and battle renderers (`view_entities(12, 12)`) are inside fights,
so they are unaffected.

### 3. Memory: `SeenTiles`

- `resources::SeenTiles`: a `BTreeMap<(i32, i32), ChunkSeen>` keyed by chunk
  coordinate (`CHUNK_SIZE = 32`, via `chunk_of`). `ChunkSeen` is one bit per
  tile, `[u64; 16]`, 1024 bits. Sorted keys keep the save deterministic.
- `Game::mark_seen_tiles()` marks every tile within the current radius. It
  is called:
  - beside `mark_explored_chunks` in the turn (`turn.rs`), surface only;
  - at the end of `Game::new` and `Game::load`, so the first frame isn't
    black outside a 5-tile hole that the first tick would have filled.
- **Saved** as `SaveData::seen_tiles` behind `#[serde(default)]`. The save is
  field-named RON, so this is additive and costs **no
  `SAVE_FORMAT_VERSION` bump**. An older save loads with no memory. The
  surface is black except around the player and fills in as they walk.
  Seeding it from `ExploredChunks` was rejected: that marks whole 96×96
  blocks the player never saw.
- Both `lifecycle.rs` init points and the literal `SaveData` constructions
  (`save.rs`, `tests/spawning.rs`) gain the field.
- Remembered terrain is drawn from the current `view_tiles_at`, not from a
  snapshot. Surface terrain changes only at outposts (`set_override`), and
  outposts are always shown. Snapshotting terrain would double the memory
  for a case that's next to invisible.

### 4. Rendering

- `draw_surface_map` gets each tile's `Sight` from one engine call over the
  view box. That is a new `Game::sight_view_at(center, hw, hh) ->
  Vec<Vec<Sight>>` mirroring `view_tiles_at`, so the renderer never loops
  over `sight_at` per tile.
- **Unseen:** the tile is filled black. No biome, edges or glyph.
- **Remembered:** the ground and any landmark glyph or sprite are drawn at a
  dim factor, `FOG_REMEMBERED_DIM` in the gui's own tuning beside the
  vignette constants. It is folded into the existing `vig` multiplier, so
  ground, glyph ink and sprite tint all dim through the one path. No cloud
  shade or per-tile jitter in memory: the world out there isn't moving
  for you.
- **The edge:** the outermost tile of the circle blends from live to
  remembered over one tile, using the distance from the player and
  `perception_radius()`. It's a cosmetic fade only. The `Sight` state
  stays binary.
- The existing Power-driven vignette still applies, stacked with the fog.
  If low Power plus fog reads as mud, the vignette's floor is the thing to
  retune. That is judged on screen during implementation, not specified
  here.

## Testing

**Engine (new, `tests/perception.rs`):**

- Perception from attributes:
  - baseline attributes give exactly `PERCEPTION_BASE_RADIUS`;
  - +1 Analysis gives +0.5;
  - +1 Entropy gives +0.033;
  - attributes far below base clamp at `PERCEPTION_MIN_RADIUS`.
- `sight_at`:
  - a tile at distance `r` is `InSight`, one at `r + 1` is not;
  - a walked-past tile is `Remembered`;
  - an untouched one is `Unseen`;
  - off the surface every tile is `InSight`.
- `mark_seen_tiles` marks across a chunk boundary, including negative
  coordinates.
- **Save → load keeps `SeenTiles`.** This must be a real save and load, not
  a RON round trip, per memory `ron-round-trip-cannot-catch-a-skipped-field`.
  An old save without the field loads with empty memory.
- `shown_at` covers every row of the table, one case each.
- `find_target_in_direction` skips a hostile outside sight but inside the
  12-tile examine range.

**app-core:** hover and click-travel ignore a hidden hostile.

**gui:**

- `draw_surface_map` draws no glyph for an unseen or remembered creature,
  and draws a remembered settlement.
- The Threat count excludes hidden hostiles.
- `effect_text` renders Perception to one decimal with "tiles".

**Existing tests that will break.** Many gui `base.rs` tests (~2160-6440),
`tests/base_space.rs::view_tiles_is_unchanged_on_the_surface`, and app-core
`menus.rs` examine tests place a creature further than 5 tiles from the
player. Fix them by **moving the creature within sight**. Where a test
needs distance (the examine-range tests at 12), raise the player's Analysis
through the attribute setter. That also exercises the real derivation path.
**No test-only bypass of the fog**, and no reliance on `FERAL_DEV_REVEAL`
inside tests, because an env var leaks between parallel tests.

**Gates:**

- `cargo test --workspace`.
- `cargo clippy --workspace --all-targets`.
- `balance_sim` must not move. Perception feeds no combat number, and a
  moved curve means something leaked.
- A screenshot of the fog: `--template stack` won't do, because it's
  surface play. Use a surface template and `--screenshot`.

## Release

Additive save field, so **not breaking**: a patch bump. `CHANGELOG.md` gets
the section. The `analysis.ron` and `entropy.ron` `does:` lines and the
attributes README change in the same commit as the stat.

## Risks

- **Tight baseline.** 5 tiles is an 11-tile circle. Creatures beyond it
  vanish from a viewport that today shows everything, so early surface play
  gets noticeably more dangerous. That is the intent. It's tuned by
  `PERCEPTION_BASE_RADIUS` alone.
- **Analysis gets stronger.** It already feeds ATK, decompiler skill and
  Extraction. Perception makes it the obvious stat to buy, and the per-point
  values are kept small for that reason.
- **Missed readers.** A fifth way to address a creature by position,
  added later, could leak through the fog. `shown_at` being the one
  predicate is the guard. The plan should grep for `view_entities_at`
  callers as a gate.
