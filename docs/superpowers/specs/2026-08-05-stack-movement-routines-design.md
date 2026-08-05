# Stack movement routines — design

**Date:** 2026-08-05
**Status:** approved, not implemented

## The problem

A Stack frame is walked one cell at a time and there is nothing else you can
do about its shape. Every tool the player has for a frame — the map, the
view cone, a seal, a fault — either reads the maze or is inflicted by it.
Nothing lets the player *act on* it.

This adds two field routines that do: one steps through a single wall, one
jumps to a coordinate you point at and kills you if that coordinate is
solid. Both are manufactured and installed like any other routine, so the
cost is a research node, a Routine Disk, and one of six slots.

## The two routines

| | **Buffer Overrun** | **Wild Jump** |
|---|---|---|
| Effect | `AbilityEffect::Phase` | `AbilityEffect::Jump` |
| Does | Crosses exactly one solid cell along the current facing, landing on the open cell beyond | Moves the party to any cell of the current frame the player points at |
| Refused when | The cell beyond is also solid, or lies off the frame | Never for being unknown — that is the whole gamble |
| Risk | None; it works or it is refused | Landing cell is solid → the player dies |
| Costs | Fatigue, Trace | Fatigue, Trace |
| Second pick | None; commits from the routine list | A cursor over the frame map |

Wild Jump is a `goto` to an address nobody validated. Cells the party has
already seen are safe *because* they have been seen; the unmapped part of
the frame map is exactly the risk, and the map already draws unknown cells
as unknown, so the gamble reads on screen without a word of explanation.

Buffer Overrun is deliberately one wall thick. Any deeper and a cast from
the frame edge cuts a diagonal across the whole maze; at one wall it opens
the room next door and nothing further.

## How they are reached

Unchanged from every other researched routine — no new machinery:

1. A new research node teaches both ids into `resources::KnownRoutines`.
2. The player crafts a **Routine Disk** (2× Blank Substrate, Disk Press).
3. `Game::install_routine` spends knowledge + disk + a free slot.

Carrying both costs two of the player's six slots, and popping one out
destroys nothing further because the disk is already gone. That is the
existing commitment model and this feature does not bend it.

## Engine

### The vocabulary

`AbilityEffect` gains two variants. Neither carries a magnitude:

- `Phase` — no fields. Its reach is fixed at one cell by the rules above,
  not authored per file, so a mod cannot widen it into a tunneller.
- `Jump` — no fields.

Both are **field-only**: `combat_round.rs`'s `use_ability` gets the same
`unreachable!` arm `FieldBuff` already has, and `combat.rs`'s
`battle_special_options` filter widens from "not `FieldBuff`" to "not
field-only", so neither can be commanded in a fight.

Cost comes off the existing top-level `AbilityDef::fatigue_cost` rather than
a new field. For a routine that never appears in battle, "what running this
costs the player in Fatigue" is already precisely what that field means.
`AbilityEffect::FieldBuff`'s own `power_cost` stays where it is — migrating
it is not this feature's business, and Fatigue is the right currency here
regardless: it is the ability-energy pool that regenerates per tick, whereas
Power is a need the Recharger Node deletes outright.

### The cast path

Both cast through the existing field-routine path in `game/field.rs`:

- `Game::field_routines` widens from "installed `FieldBuff` abilities" to
  "installed **field-castable** abilities". It stays the only place that
  list is built, so a filtered view and a cast-time index cannot disagree
  about what a position means.
- `FieldRoutineView` grows a field saying which second pick, if any, the row
  needs — replacing the current `needs_ally_target: bool`, which cannot
  express a third answer. Three shapes now exist: none, an ally, a cell.
- `Game::cast_field_routine` branches on the effect. Its existing contract
  holds for both new arms: refused during a battle and after game over,
  every check before the first write, the clock ticked only on success.

Both add one refusal the buff path does not have: **Stack only**. Not via
`require_surface` — that guards actions reaching zone-map state through a
`Position` pinned to the entrance tile, which is the opposite problem. These
two read and write `Locale::Stack`'s own coordinates, so what they need is
the presence of that locale, and `Game::stack_pos` returning `None` is the
refusal.

### The arrival seam

`Game::step`'s tail is currently inline: `bleed_corruption`, `open_cache`,
`rouse_lair`, `trip_breakpoint`, `take_fault`, `maybe_stack_encounter`, with
`remember_view` before all of them. Its ordering is load-bearing and already
documented in that function — corruption first because it is a property of
arriving, the fault before the encounter roll so a party that fell rolls in
the frame it landed in.

Three ways to arrive at a cell is fine; three copies of what arriving
*means* is not. That tail is extracted into one function which `step` and
both new routines call. Same argument `Game::enter_frame` already makes
about there being one way into a frame.

Consequence, decided rather than discovered: **arrival hazards fire on a
jump exactly as on a step.** Jump onto a fault and you fall through it; jump
onto an uncleared lair and you have roused it.

Facing is unchanged by both routines.

### Death by rock

Wild Jump onto a solid cell kills the player through `Game::apply_damage` —
the single documented path that lowers a creature's HP — and **`Locale` is
never written**. The party does not briefly stand in the rock and get
rescued; they never move at all.

That matters beyond tidiness: rock is the one `CellKind` that is both
unwalkable and sight-blocking, so a party inside one is exactly the
occluder trap doors sprang, filling the first-person view with wall and
truncating the map to the party's own row. Not writing `Locale` means no
state exists in which that is reachable, so neither cone consumer needs a
new exception.

What happens next is already built and untouched by this change: on
Forgiving, `difficulty::death_handling_system` warps the player out through
`stack::surfaced`; on permadeath the run ends.

The frame map cursor is bounded to the grid, so an out-of-bounds coordinate
is unreachable rather than lethal. Inside the grid, solid is solid.

### The sealed-wing carve-out

Both routines refuse a landing that is **behind an unopened seal**, and
refuse landing **on** an unopened `SealedDoor`. The second is not
redundant: a sealed door is `walkable()`, so a jump could land on it and the
next ordinary step into the wing would never consult `pass_seal`, which only
fires when the *target* cell is sealed. Landing on the door is therefore the
bypass, not landing past it.

One helper serves both — a flood fill from the frame's entry over walkable
cells, treating unopened sealed doors as blocking, giving the set of cells
the party could not legitimately reach yet. It reads `FrameMemory::opened`,
so a seal the player has already burned stops excluding anything.

This is a **refusal, not a death**. An accident that costs nothing is better
feel than an accident that costs a run, and the point of the rule is to keep
the access-shard economy and "earn your way to the guardian" intact, not to
punish a misclick.

Everything else in the frame is fair game, including cutting past ordinary
doors and dead ends.

## app-core

`Mode::FieldCast` already routes a chosen routine either straight to the
cast or on to `Mode::FieldCastAlly` for a second pick, holding the pending
index in the field documented on `App`. Wild Jump adds a third destination:
a cell picker.

- New `Mode` for the cursor, dispatched from `App::handle_field_cast_key`
  when the chosen row asks for a cell.
- The cursor starts on the party's own cell, moves by the same keys the
  player already walks with, and is clamped to the frame's bounds.
- Esc backs out and spends nothing, matching every other second pick.

Buffer Overrun needs no second pick and commits from the routine list, the
same as a `WholeParty` field buff.

## gui

The cursor screen is a **third caller of `render/frame_map.rs`**. It widens
`layout`'s `fill` parameter and reuses `draw_grid` / `tile_color` /
`cell_glyph`, plus a cursor overlay. It does **not** get its own copy of the
glyph table: this is the one screen where drift silently misinforms rather
than merely looking wrong, and the player is about to bet their run on what
it says.

## Content

- `assets/abilities/buffer_overrun.ron`, `assets/abilities/wild_jump.ron`.
  Both `wild_weight: 0`, so neither spawns on a wild program — these are
  manufactured tools, not something you find pre-installed on a glitch.
- One research node, `assets/research/address_translation.ron`, requiring
  `deep_analysis` and priced with the late nodes.
- No new item. Routine Disks already exist and are already craftable.

## Tuning

New `pub const`s in `tuning.rs`, in the Stack/Trace section beside
`TRACE_PER_SEAL`: `TRACE_PER_PHASE` and `TRACE_PER_JUMP`. Fatigue costs are
authored per ability in the `.ron` files, since `fatigue_cost` is already an
ability-level field.

## Documentation

- `assets/abilities/README.md` — document both new effects and the fact that
  they are field-cast and Stack-only. The same edit fixes a stale paragraph
  already there: it still describes the pre-Routine-Disk model, claiming
  "every loaded ability automatically gets a `routine_<ability_id>` item
  minted for it". `ItemDb::synthesize_routines` no longer exists.
- `assets/research/README.md` if the new node needs anything said about it
  beyond the existing schema.
- `CHANGELOG.md`.
- `CLAUDE.md` load-bearing seams: the arrival-seam extraction earns an entry,
  in the same shape as the `enter_frame` one.
- Not `docs/manual.md`, not the root `README.md` — both are carved out.

## Testing

Engine:

- Phase crosses one wall and lands on the open cell beyond.
- Phase is refused by two-deep rock, and by the frame edge.
- Phase and Jump are both refused on the surface, in battle, and after game
  over.
- A refused cast spends no Fatigue and raises no Trace.
- Jump to a mapped floor cell arrives, and fires the arrival tail — asserted
  through a concrete hazard, e.g. jumping onto a fault drops the party.
- Jump onto rock kills the player and leaves `Locale` unchanged.
- Both refuse a landing behind an unopened seal, and on an unopened sealed
  door; both allow the same landing once the seal has been burned.
- Both raise Trace on success.
- The arrival tail is called by all three arrival paths — the regression
  that matters is a fourth caller skipping it, so the test asserts behaviour
  (a cache emptied by a jump) rather than a call.

app-core:

- The cell picker opens only for a routine that asks for one.
- The cursor clamps to the frame bounds.
- Esc backs out spending nothing.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
`balance_sim` is unaffected — it models no abilities at all — so it is a
regression check here, not evidence.

## Deliberately not in scope

- Surface teleport. Anything writing `Position` while `Locale::Stack` may be
  live is the hole `clear_stack` and `require_surface` exist to guard, and a
  zone-map jump needs its own reachability and build-radius rules. Stack
  only.
- Migrating `FieldBuff`'s `power_cost` onto `fatigue_cost`.
- Any second `Phase` depth, authored or perk-scaled.
- Save-format changes. Ability ids are strings in `Routines` and
  `KnownRoutines`; `AbilityEffect` is loaded from assets and never
  serialized into a save.
