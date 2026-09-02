# Shiny variants: Optimized and Overclocked programs — design

**Date:** 2026-08-10
**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header.
**Scope:** `crates/engine`, `crates/gui` — two crates, so this earns the full
spec-and-plan pipeline per `CLAUDE.md`'s process-weight rule.
**Save format:** **breaks.** `CreatureSave` gains a field, so
`SAVE_FORMAT_VERSION` goes 25 → 26 and every existing save stops loading.

## The gap this exists to fill

A wild program varies within its species along exactly one axis today:
`Potential`, four independent ±20% rolls baked into `Stats` at spawn
(`tuning.rs:418-419`, `components.rs:785-798`). That axis is doing real work
— `growth_roll` scales every subsequent level-up — but it has two properties
that make it invisible as a *find*:

- It is **continuous**. There is no threshold, so there is no moment where
  something crosses from ordinary into special.
- It is **unannounced**. The only surface is `quality_label` ("Poor" through
  "Excellent") on the manifest and party screens — a number you look up
  after you have already caught the thing.

So the roster fills with programs the player never chose. Nothing on the map
ever says *that one, go get that one*.

This adds a second axis over the same territory, deliberately coarse and
deliberately loud: a rare spawn-time tier that multiplies stats outright and
is visible on the grid before you engage.

| Tier | Enum | Player-facing | Stat multiplier | Spawn chance |
|---|---|---|---|---|
| — | `Rarity::Ordinary` | (nothing) | 1.0x | — |
| Uncommon | `Rarity::Silver` | **Optimized** | 1.5x | 3.0% |
| Rare | `Rarity::Gold` | **Overclocked** | 1.8x | 0.5% |

Naming follows the **Raid / "GC Entropy Sweep" precedent**: the enum, the
constants and the save field say `Silver`/`Gold`, which is what the *colours*
are; everything a player reads says Optimized/Overclocked.

### Why not just widen `Potential`?

The obvious objection, so it gets an answer up front. `Potential`'s own doc
calls itself "the same species, different stats" mechanic, and a rarity tier
is a second thing over that ground. Widening the band to ±60% would not
produce this feature: it would move the whole distribution, make every spawn
noisier, and still have no threshold to draw a bar for. What is wanted is a
*discrete, rare, visible* event on top of an unchanged continuous band — and
the two multiply cleanly, so a gold lands 1.44x–2.16x an ordinary spawn of
the same species.

## Two consequences that are free, and must not be "fixed"

**XP already scales.** A kill pays the victim's `max_hp`
(`game/combat_round.rs:632-635`, and the same on the decompile path at
`combat_rewards.rs:415`). Since rarity is baked into `max_hp` at spawn, an
Overclocked kill pays 1.8x XP with no code. `STACK_DEPTH_STAT_GROWTH`'s doc
already states this as intended design for depth scaling; it holds here.

**Taming already preserves it.** `attempt_decompile` mutates the wild entity
in place — it never despawns, never respawns, never re-rolls
(`combat_rewards.rs:382-410`). So an Overclocked program you catch stays
Overclocked forever. That is the whole collectible hook, and it costs nothing.

**Loot is deliberately untouched.** `award_loot` is keyed off species only and
stays that way. The reward is the creature plus the free XP; a drop
multiplier is a separate knob for a separate day.

## Architecture

### One chokepoint does all the work

Every wild creature in the game is assembled by exactly one `world.spawn()`:
`Game::spawn_wild_creature_scaled`, `game/spawning.rs:165-210`. Its entire
stat derivation is one closure:

```rust
let scale = |base: i32, roll: f32| ((base as f32) * mult * depth_mult * roll).round() as i32;
```

Rarity becomes a fourth factor there, and `Rarity` joins the bundle. **No
caller changes at all.** `spawn_pack`, `spawn_nest_guardian`,
`stack_encounter_pack`, `rouse_lair`, `adopt_program`,
`spawn_initial_creatures` and the arena's `build_opponents` all inherit it,
because they all funnel through this one function.

The eligibility rule lives at the chokepoint too, because both inputs are
already in scope: `species.is_boss`, and `x, y` for `in_opening_ring`.

### Eligibility: not bosses, not the opening ring

**Bosses are excluded** because a boss has no engine-side multiplier by
design — `assets/species/README.md` says so explicitly, its stats are
hand-authored per file. Multiplying a hand-tuned number by 1.8 discards the
authoring.

**The opening ring is excluded** because
`balance_sim::beatable_by_a_fresh_player` asserts that a fresh player can
beat one program there, and it computes the pessimistic case using
`MAX_INDIVIDUAL_ROLL`. An Overclocked drone in the ring falsifies that
guarantee. `Game::in_opening_ring` (`spawning.rs:450`) is the existing
predicate and the only one that should be consulted.

The link is load-bearing in both directions: **the exclusion is what lets
`balance_sim` stay ignorant of rarity.** If the curve tests ever move as a
result of this feature, the exclusion is wrong, not the test.

### Gate before drawing, not after

`Game::roll_rarity(&SpeciesDef, x, y) -> Rarity` must return `Ordinary`
**without touching `GameRng`** when the spawn is a boss or in the opening
ring.

This is the deliberate mirror image of the density gate in
`maybe_spawn_wild_creature`, which rolls *first* precisely so that a miss
leaves the RNG stream untouched. Here the reasoning inverts: gating first
means boss spawns and every zone-1 opening-ring spawn keep their exact
current sequence, so the seeded tests covering those paths do not move.

Eligible spawns *will* consume one extra `random_range` each, shifting the
shared stream for everything drawn after them. **Some seeded spawn and combat
tests will need re-baselining. That is expected churn, not a regression** —
re-read each failure rather than assuming a bug.

### Rarity is a record, not a live multiplier

The component is a **receipt for a multiplier already spent**, the same shape
as `EquippedItem::fusion_tier`. `Game::load` restores `Stats` verbatim and
must **not** re-apply `stat_mult`; neither may `fuse_companions`. Getting
this wrong compounds the bonus on every single reload, invisibly, because a
stat carries no record of where it came from. This belongs in the component's
doc comment, since the regression to head off is a later reader "finishing
the job" by applying a multiplier they can see sitting unused.

## Display: four channels, and why none of them can merge

**Map — a coloured bar along the top edge of the tile.** The primary
indicator: an Overclocked program should be spottable on the grid without
inspecting it.

It must be a separate channel rather than a recolour, because the glyph's
colour is already spoken for. `game/inspection.rs:294-305` overrides a
hostile's authored species colour with `difficulty_color` — green → yellow →
orange → red by power ratio, magenta for a boss. That is the "can I win this
fight" read and rarity cannot have it. The bar contests nothing: the glyph
keeps saying how dangerous, the bar says how rare.

Drawn in `draw_surface_map` (`render/base.rs`) after the glyph
(`base.rs:676-682`) and before the spawn-point outline (`:688-692`). That
outline is the precedent, and its comment states the principle — an overlay
"rather than replacing the glyph, so whatever's actually standing there still
reads clearly on top of it."

Three details: the bar takes the **vignette but not the tile shade**,
matching the glyph's own rule at `base.rs:672-675` (depth applies to
everything on the map; per-tile jitter belongs to the ground); thickness
follows the spawn outline's `2.0`; and it draws for a tamed shiny too, on the
rare tiles where one is drawn at all (a companion is drawn only while out on
a hauling errand, `base.rs:519-531`).

**Name prefix** — "Overclocked Scrapper 2", via `Game::creature_label`
(`party.rs:127-132`) and **not** `zone_tagged_name`, because the latter also
feeds `EnemyGroupView::species_name` (`combat_round.rs:427`) and the battle
roster has its own channel. Two knock-ons, both accepted: `creature_label`
feeds the owned-pets sort key (`party.rs:246`), so shinies group by prefix in
the party menu; and a player-renamed program keeps its `CustomName` and gains
the prefix, which is correct — it *is* Overclocked.

**Menu row colour** — collides with `render/mod.rs::fusion_color`
(`mod.rs:134`), the one colour rule for anything fused. Rarity and fusion
depth are *both* permanent properties, so the existing "state to act on now
beats property to read at leisure" tiebreak gives no answer.

Resolution: **fusion outranks rarity**, extending the chain to
`CRITICAL > fusion > rarity > plain`. Same argument one level down — on the
fuse-picker screens cyan/magenta is the read for *can this still be an
input*, a question about an action available now; rarity is never actionable.
A fused Overclocked program keeps its name prefix and loses only the colour.

Mechanically: `rarity_color(Rarity) -> Option<Color>` beside `fusion_color` —
**the same function the map bar calls**, so a tile and a menu row cannot come
to mean different colours, which is exactly the invariant `fusion_color`'s
own doc argues for. Then `program_color(fusions, rarity) -> Option<Color>`
decides the precedence **once**; programs call it, gear keeps calling
`fusion_color` (gear has no rarity).

Two palette collisions to check on screen rather than in the source:
`YELLOW` is `(0.9, 0.8, 0.2)`, so GOLD needs pushing warmer (nearer
`(1.0, 0.72, 0.15)`); and `GRAY` is `(0.51, 0.51, 0.51)` with `WHITE` above
it, so SILVER must sit clear of both — nearer `(0.80, 0.84, 0.90)`, cool
rather than neutral, or it reads as a dimmed white glyph.

**Battle roster — a short tag, not the prefix.** `NAME_W = 18`
(`render/battle.rs:61`) and `cell()` clips with `…`, so "Overclocked Scrapper
2" (22 chars) truncates and "3 Overclocked Scrappers" is worse. The tag is
appended where `[BOSS]` is appended (`battle.rs:252`), *outside* the
fixed-width name cell, so it shifts no columns.

## Files

| File | Change |
|---|---|
| `engine/src/components.rs` | `Rarity` enum near `FusionCount` (`:848`) — the shape to copy. Absent reads as `Ordinary`. **Variant order is save format**; append, never reorder. |
| `engine/src/tuning.rs` | Chance + magnitude as one doc-commented block beside `BOSS_SPAWN_CHANCE` (`:909-912`), the existing "rare thing instead of an ordinary thing" knob. Cross-reference `MIN/MAX_INDIVIDUAL_ROLL` (`:413-419`). |
| `engine/src/game/spawning.rs` | `roll_rarity` beside `roll_potential` (`:350`); one factor in `scale`; `rarity` in the bundle at `:186`. |
| `engine/src/game/party.rs` | Prefix in `creature_label`; `fuse_companions` (`:617`) takes `max(parent_a, parent_b)` rarity **without re-multiplying**. |
| `engine/src/save.rs` | `CreatureSave.rarity` with `#[serde(default)]` (inert for bincode; keeps the field-named `dev-saves/` RON templates parsing). Bump to 26, add the `25 → 26` line **and backfill the missing `24 → 25` line** — that bump came from `b2975b4` and was never documented. |
| `engine/src/game/lifecycle.rs` | Write (`:707`-ish), read (`:425`-ish). The save query tuple at `:625` is at bevy's 15-element ceiling, so `Rarity` joins the trailing nested tuple, which has room. |
| `engine/src/views.rs`, `game/inspection.rs` | `rarity` on `EntityView` (`:312`, plus the hardcoded default at `:696` for `symlink_targets`), `PetInfo` (`party.rs:257`), `ProgramManifest`. Plain structs, so the compiler finds every site. |
| `gui/src/render/mod.rs` | `SILVER`, `GOLD`, `RARITY_BAR_PX`; `rarity_color`; `program_color`. |
| `gui/src/render/base.rs` | The map bar. |
| `gui/src/render/battle.rs` | The roster tag. |
| `gui/src/render/*.rs` | Route the program-side `fusion_row` callers through `program_color`. |

## Testing

TDD, failing test first, each named for what it pins.

**Engine**
- `rarity_multiplies_every_stat` — `stat_mult` arithmetic, direct.
- `a_boss_never_rolls_a_rarity`, `no_shiny_spawns_in_the_opening_ring` — and
  both must also assert `GameRng` is **unadvanced**, which is the half a
  naive test misses and the whole point of gating before drawing.
- `taming_a_shiny_keeps_it_shiny` — decompile preserves `Rarity` and `Stats`.
- `a_shiny_survives_a_save_round_trip` — rarity comes back *and stats are
  unchanged*, which is the re-multiplication trap.
- `fusing_two_shinies_keeps_the_higher_rarity`.
- `spawn_shiny_on_player_tile` fixture in `tests/support.rs` (spawn, then
  insert `Rarity`) so the tag-travel tests don't hunt for a seed.

**GUI**
- `fusion_outranks_rarity_in_a_menu_row` — pins the precedence decision.
- `a_shiny_battle_tag_does_not_shift_the_columns_after_it`, following
  `a_long_name_does_not_shift_the_columns_after_it` (`battle.rs:879`).
- `the_map_bar_does_not_replace_the_difficulty_colour` — an Overclocked
  hostile still reports its `difficulty_color` glyph. This is what stops a
  later tidy-up collapsing two channels into one recolour.

**Gates**
- `cargo test -p feral-processes-engine balance_sim` — **should be unmoved.**
  If it moves, the opening-ring exclusion is wrong.
- `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
- Playtest. Raise the chances to ~0.5 locally and look at one on the map, in
  a fight, and in the party menu. A green suite says nothing about whether
  silver reads as silver on screen.

## Migration

There is no migration path — `load_from_file` rejects a version mismatch
outright. A player who wants to keep a game must, **on the pre-bump binary**:

```sh
cargo run --bin savetool -- dump saves/save.bin s.ron   # before rebuilding
cargo run --bin savetool -- pack s.ron saves/save.bin   # after
```

RON is field-named, so the new `#[serde(default)]` field fills itself in.

## Docs

`CHANGELOG.md` gets its own section; this is a **save-format break**, which
is what "breaking" means in this repo's versioning policy — read the
preamble for which digit moves. `assets/species/README.md` needs a line
noting rarity is engine-rolled and not a species field, since that is the
first place a modder will look for it.
