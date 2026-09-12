# Weapon range and travelling bolts on the battle map

**Date:** 2026-09-12

A basic attack on the tactical screen is adjacent-only and lands with no
visible travel. This gives a weapon a **distance** it can be swung from,
gives a species' `ranged` move the same on a board, and draws the blow as a
streak travelling from the swinger to whoever it lands on.

## What exists already

- `Game::tactical_attack` refuses anything but adjacency, hard-coded as
  `(from.0 - at.0).abs() > 1 || (from.1 - at.1).abs() > 1`.
- `ItemDef::reach` (`items::WeaponReach`) is **breadth and never distance** —
  the multi-target sweep Scatter Lance and Broadcast Storm author.
- `abilities::AbilityRange`, `tactical::reach::in_range`,
  `reach::distance` and `reach::line_of_sight` all exist and are read by
  routines. Nothing reads them for a basic attack.
- `SpeciesDef`'s `MoveDef::ranged` is the **group** model's front-line flag.
  It survives the conversion to an `AbilityDef`
  (`species::basic_attack_ability`) and nothing on a board reads it.
- The aim cursor (`a` → `Mode::TacticalAim`) already points anywhere on the
  board, so no new targeting UI is needed.
- `crate::fx::Fx` already holds `Walker` — a path drawn over wall-clock time
  — and is already threaded into `render::tactical::draw_tactical_map`.
- `Painter::line` is one of the fifteen operations. No new `Painter` op.

## Decisions

### A weapon's range is its own `ItemDef` field

```ron
range: Some(3),
```

`Option<u32>`, `#[serde(default)]`, absent meaning melee.

**Not inside `WeaponReach`.** That type is breadth, and folding distance into
it would force a single-target weapon that reaches three cells to author a
`target` and a `recharge` it does not want, and would make the two shipped
sweep weapons the only ones that could reach at all.

**Not on `EquipmentStats`.** The existing seam: those six fields are scaled
four ways by `Game::copy_bonus` and a range must not scale, and
`EquipmentStats::is_empty`/`has_upside` **destructure** on `cell_mark`'s rule
— a non-numeric field has no answer for either.

Refused at load outside `1..=tuning::TACTICAL_WEAPON_RANGE_MAX`, following
`ItemDb::load_dir`'s existing skip-with-a-warning pattern. A census in
`tests/assets.rs` holds every shipped authored range inside that window.

### A `ranged` move reaches two cells, and that number is load-bearing

`tuning::TACTICAL_RANGED_MOVE_RANGE = 2`.

Every shipped species has exactly two moves, and **14 of 17 have exactly one
`ranged: true`** — only Construct, Scrapper and Sentinel are pure melee. At
three cells almost every wild body on the board becomes a shooter and the
closing mechanic the model is built on evaporates. Two is one cell of
standoff: real, and closed by a single step. Player weapons author 2-3 and
keep the edge.

### A body's swing range is a property of the body, and `Game::swing_range` is the one door

In `game/combat.rs` beside `swing_reach`:

```rust
pub(crate) fn swing_range(&self, actor: Entity) -> u32
```

A worn weapon's authored `range`, else the largest range among the body's own
`SpeciesDef::basic_attacks()` (a `ranged` ability reaching
`TACTICAL_RANGED_MOVE_RANGE`), else `TACTICAL_MELEE_RANGE`.

**Three readers, and they are calls rather than copies**: the attack gate in
`tactical_attack`, `Intent::Swing`'s band in `tactical/ai.rs`, and
`swing_at_best_neighbour`'s target filter. Two of those hard-code
`TACTICAL_MELEE_RANGE` today, which is exactly the drift a shared doc comment
cannot hold — a body whose band says three and whose filter says one walks
into position and then passes.

A worn weapon **replaces** the species figure rather than being maxed against
it, which is `Game::attack_range`'s existing precedent: worn `damage` replaces
a species' natural range outright rather than adding to it. So a companion of
a ranged species holding a melee blade swings at arm's length — the weapon is
what it is swinging.

It is a property of the **body** and not of the move it happens to roll: the
AI decides its intent before it walks, and a range read off a move rolled at
swing time would let a body plan a standoff and then draw the melee half of
its pair.

**Read in tactical fights alone.** The group model has no geometry to spend
it on — `AbilityDef::tactical_shape`'s own rule, and `ranged` keeps its
separate group-model meaning untouched.

### The rolled move is constrained by the distance, and the group model is not touched

`Game::swing_move` gains `swing_move_at(entity, Option<u32> distance)`;
`swing_move(entity)` is a call to it with `None`. The filter narrows
`roll_species_move`'s candidate list to moves whose own range covers the
actual distance. So a body at two cells fires its ranged move and a body
standing adjacent rolls either, which is the variety `swing_move` exists for.

`None` is what the group model passes, so nothing about an abstract fight
changes. One `random_range` draw either way, whatever the candidate count, so
the seeded stream's **position** is unchanged — the drawn value may differ,
which is an ordinary retune signal and not a stream shift.

### Line of sight is required, with no melee branch

`tactical_attack` gains `reach::line_of_sight(&board, from, at)`
**unconditionally**. `line_of_sight` excludes its endpoints, so for adjacent
cells its loop is empty and the check is already a no-op — one rule, and no
second place `TACTICAL_MELEE_RANGE` has to be restated. `Cover` cells earn a
second job for free.

A reach weapon's swept shape is still cast from the actor toward the aim,
which is `reach::recipients`' existing rule: a `Line` fired from three cells
out simply runs further. Nothing about the sweep changes.

### The cue is engine-side, on its own queue, with no kind

```rust
pub struct BoltCue {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub color: components::GlyphColor,
}
```

Queued by `resources::BoltQueue`, drained by `Game::take_bolts()` —
`TransitQueue`'s rules exactly: capped at `EFFECT_QUEUE_CAP`, and
**deliberately not serialized**, because a bolt in flight has nothing to say
to a reloaded save.

**Engine-side, not inferred by gui.** A hostile's swing resolves inside a
single `AiBeat`, and by the time gui draws the next frame there is nothing
left on the board saying who shot whom.

**Its own queue, not `EffectQueue`.** These are `TacticalBattle` cells;
`VisualEffect::pos` is world coordinates. Pushing board cells into that queue
would pin a flash to an unrelated tile out in the zone — the same convenience
the `Position` seam refuses.

Pushed from `tactical_attack`, **one cue per body actually swung at**, so a
reach weapon fires a streak at each body its shape caught.

**No `kind` field.** The streak travels `from` → `to` by one rule, and at
distance one that is a short flick across a single cell — which is the melee
feedback, for free, and keeps gui from restating the melee threshold to
choose between two draws.

### gui draws it in the tactical pane, and its lifetime is derived

`Fx` grows `bolts: Vec<Bolt>`, fed in `begin_frame` beside `walkers` and
retained by age exactly as they are. Drawn in `render/tactical.rs` against
the pane's own `tile_origin_px` and `map_cell`, through `Painter::line`, in
the swinger's own `palette::glyph` hue.

`BOLT_SECONDS` is **derived from `TACTICAL_TURNS_PER_SECOND`** rather than
restated, so a bolt always lands before the next body acts: one beat is
0.625s at the shipped 1.6 turns per second, and a bolt is a fraction of it.
A restated constant is the copy that drifts when the pace is retuned.

The drain happens every frame in `crates/gui/src/lib.rs` whether or not it
will be drawn, matching the existing rule for effects and transits — a
disabled `Fx` must not leave the engine's queue sitting at its cap.

## Content

Seven weapons author a range; the other eight stay melee and pay for it in
damage.

| weapon | range |
|---|---|
| Arc Lance | 2 |
| Scatter Lance | 2 |
| Interrupt Coil | 2 |
| Monofilament Whip | 2 |
| Plasma Router | 3 |
| Siege Compiler | 3 |
| Broadcast Storm | 3 |

The damage rebalance between the reaching and the melee halves is settled
with an arena pass, not guessed in this document.

`assets/items/README.md` documents the new field in the same change.

## Testing

**Engine**

- `swing_range` answers the weapon's authored range, a `ranged` species'
  `TACTICAL_RANGED_MOVE_RANGE`, and 1 for a melee body with no weapon.
- `tactical_attack` lands at exactly the range and is refused one cell past
  it.
- A swing across a `Blocked` cell is refused; the same swing with the cover
  removed lands. **Delete the LOS line and this must fail** — the fix is
  removable, per this repo's vacuous-test rule.
- An adjacent swing is unaffected by the LOS check.
- A ranged hostile holds at its band instead of closing to adjacent.
- `swing_at_best_neighbour` picks a target at range, and skips one behind
  cover.
- A body at two cells rolls its ranged move, never the melee one.
- `roll_species_move` through the group model's `None` path is unchanged.
- `tactical_attack` queues one `BoltCue` per body swung at, `from` the
  swinger's cell and `to` each recipient's.
- A save round trip carries no bolt.
- `tests/assets.rs`: every authored `range` inside
  `1..=TACTICAL_WEAPON_RANGE_MAX`; a malformed one is skipped, not a panic.

**app-core**

- The aim cursor committed at distance lands rather than refusing with
  "Not from here."

**gui**

- A bolt expires inside one `TACTICAL_TURNS_PER_SECOND` beat.
- `draw_tactical_map` draws a bolt on the pane, and draws none once expired.

**Gates**

`cargo test --workspace`, `cargo clippy --workspace --all-targets`,
`cargo fmt`.

`cargo test -p feral-processes-engine balance_sim` is expected to report **no
change** — the sim models no geometry and cannot see a range. That is the
correct signal and not a coverage gap; the instrument for this feature is
`cargo run --bin arena`.

## Out of scope

- Routines firing bolts. They already draw their shape in the aim preview,
  and a basic attack is what was asked for.
- Any change to the group model's reading of `ranged`.
- A minimum range on a weapon. `AbilityRange` carries `min` and no shipped
  weapon wants one; a weapon that cannot be fired point blank is a second
  refusal the player has to learn from a message that does not exist yet.
- An accuracy falloff per cell. A formula nobody can read from the keyboard;
  reach is priced in authored damage instead.
