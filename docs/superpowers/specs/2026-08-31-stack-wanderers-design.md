# Stack wanderers: something to see coming

**Status:** Approved 2026-08-31. Not implemented. (Status headers in this
directory are written at approval time and go stale — see `INDEX.md`; answer
"did this ship" from `CHANGELOG.md`, never from here.)

`feral-TODO.md` #27, "add visual indicator of entity in the stack".

## The problem

**Nothing lives in a Stack frame.** Every fight underground is
`maybe_stack_encounter` (`game/stack.rs`) rolling `STACK_ENCOUNTER_CHANCE`
inside `Game::arrive`, spawning a pack straight into `start_battle` and
logging *"Something moves in the dark ahead."* There is no "ahead". The pack
did not exist a moment earlier and stood nowhere.

The corridor view already marks **every static thing** a frame holds —
`render/stack.rs`'s `cell_mark` is exhaustive over `StackCellView` and carries
`<` `>` `!` `&` `*` `v` `~` `o` `$` `+`. The frame map has exactly two marks,
`FrameMapMark::Party` and `FrameMapMark::Fight`. So the one thing the player
most needs to see is the one thing neither view can draw.

The consequence is that Trace — four bands, a stat multiplier, a group
multiplier and an encounter multiplier — is a difficulty curve whose entire
observable output is *how often you are jumped by nothing you can point at*.
The meter is visible; what it buys is not.

## What this builds

Hostile programs that **live in a frame**: placed by the frame seed, still
until they notice you, then closing on you a step at a time. You can see one
down a corridor, read what it is, and decide whether to walk around it.

Four decisions were settled before this was written and are not open here:

1. **Dormant, then hunt** — not static lurkers, not always-roaming.
2. **The blind ambush survives, gated on Trace** — a quiet party fights only
   what it can see; a loud one still gets jumped.
3. **Contact starts the fight, either direction** — no prompt, no engage key.
4. **A wanderer is a record, not an ECS entity.**

That fourth is the load-bearing one. The rejected alternative was spawning
real `Creature` entities into the frame with a `StackWanderer { home }`
component. It buys persistent HP between encounters and `creature_label` for
free, and it costs the seam that keeps the Stack comprehensible: **the
player's `Position` stays on the surface while underground**, and CLAUDE.md
already flags `DigSite` as the second non-`Structure` entity carrying a
base-space `Position` — *"so `Structure` being the space tag no longer answers
'which space is this?' on its own."* A third coordinate space makes that
worse. `CurrentStack` is not serialized, so every wanderer would need
despawning on ascend and respawning on load. Nothing here asked for persistent
wounds.

The pattern this follows instead is already shipped, twice: `CellKind::Orphan`
— *"There is no creature here until it is adopted. What species this one would
be is a function of the frame spec."*

## Not building

Named and refused, so nobody adds them back as "obvious":

- Persistent wanderer HP, or a wanderer that flees when hurt.
- A "back off" prompt on contact. Contact is contact.
- Sprites. The glyph is the deliverable; `assets/sprites/` can claim it later
  through the existing `Painter::sprite` seam without any change here.
- Wanderers on the surface. The surface already has a wild population.
- Any change to what a wanderer drops. Contact hands off to the ambush path,
  so loot, XP and Trace are whatever that path already gives.

---

## 1. Placement — pure, in `crates/engine/src/stack.rs`

`Frame` gains one field:

```rust
pub wanderers: Vec<(i32, i32)>,   // home cells
```

`Frame` is regenerated rather than saved (`resources::CurrentStack` is
deliberately not serialized), so this is derived data on a derived struct and
adds nothing to the save.

A new `place_wanderers(level, rng)` runs **last** in `stack::generate`. Last
matters: it consumes no draw that any existing placement depends on, so no
shipped frame layout changes and no seeded test moves. It uses the existing
`shuffled_floor(level, rng, pred)` — the same helper `place_orphan` uses —
with a predicate of:

- `CellKind::Floor` exactly. Not a link, not a cache, not the lair, not a
  door, not corruption.
- **Not a dead end.** Dead ends belong to caches and orphans; a wanderer
  parked in one is a wanderer nobody meets.
- At least `STACK_WANDERER_ENTRY_CLEARANCE` steps from `level.entry`,
  measured with the existing `distances_from`. The party must never arrive in
  a frame already in contact.

Count is `STACK_WANDERERS_PER_FRAME` scaled by depth, in `tuning.rs`. It
degrades through `.take()` exactly as `place_caches` and `place_orphan` do: a
frame short of eligible floor places fewer, which is a quiet frame and not a
bug worth a panic.

**World generation must not draw from `resources::GameRng`** — `generate`
already takes its own `StdRng` seeded from `FrameSpec::rng_seed()`, and this
draws from that one. A `GameRng` draw would not survive a save and would shift
every later roll in the run.

## 2. Identity — `crates/engine/src/game/stack_features.rs`

```rust
pub(crate) fn wanderer_species(&mut self, pos: StackPos, home: (i32, i32)) -> Option<String>
```

The `orphan_species` recipe verbatim: resolve `frame_spec(depth, frames,
entrance)`, take `habitat_pools(ex, ey, Some(depth), 0)`, and pick with
`StdRng::seed_from_u64(spec.rng_seed() ^ WANDERER_SALT ^ hash(home))`.

The `home` term is the one departure from `orphan_species`, and it is
required: a frame holds several wanderers and salting on the spec alone would
make them all the same program.

**Species is pinned to the home cell; stats are not.** That is the orphan
seam — what it is, is a property of the place and must survive a reload; what
it is worth is a property of the moment you met it, and comes from
`stack_escalation` at contact.

`habitat_pools` is `&mut self` — it reads `WorldMap::tile`, which generates a
chunk on demand — so `wanderer_species` is `&mut self` too, and **a view
cannot call it**. `Game::stack_view` and `Game::frame_map_revealed` are both
`&self`. That constraint is what decides section 3.

## 3. Live state — a rebuilt resource, plus one saved field

Split by lifetime, and the split is the whole of it.

**What is saved: one additive field on `resources::FrameMemory`.**

```rust
#[serde(default)]
pub slain: BTreeSet<(i32, i32)>,   // keyed by home cell
```

Home is the stable identity — it is what the frame seed hands back, and the
only thing `slain` can key on that a moving wanderer does not invalidate.
`BTreeSet` rather than `HashSet` for the reason `Stock` keys by `BTreeMap`:
this is serialized, and a hash container would make the save encoding differ
between two runs holding identical state.

**A Stack cell that can be used up needs both halves** — a placement *and* a
`FrameMemory` record. `slain` is that second half. Without it a wanderer
regenerates every time the party steps out of the frame and back in.

`StackMemory` is zone-local and already wiped **by name** in
`enter_next_zone`; this field rides inside `FrameMemory` and inherits that.

**No `SAVE_FORMAT_VERSION` bump.** The save has been field-named RON since
the format change that retired migrations, and the field is additive and
defaulted.

**What is not saved: `resources::FrameWanderers`.**

```rust
#[derive(Resource, Default)]
pub struct FrameWanderers(pub Vec<LiveWanderer>);

pub struct LiveWanderer {
    pub home: (i32, i32),
    pub at: (i32, i32),
    pub awake: bool,
    pub species: String,
}
```

Deliberately not serialized, on `CurrentStack`'s own argument: *a frame is
regenerated, not saved.* The live layer over that frame is regenerated with
it.

So **leaving a frame and coming back puts the wanderers home and asleep**, and
so does a save and load. That is a consequence, not an accident: they went
back to their patrol. It needs no explaining to a player, it makes the saved
surface one field instead of three, and it is what lets `species` be resolved
**once**, at `&mut self`, where the views can then read it off `&self`.

**One writer.** Both existing sites that install a frame — `Game::enter_frame`
(`game/stack.rs:474`) and the load path (`game/stack.rs:1051`) — currently do
their own `insert_resource(CurrentStack(Some(level)))`. Extract
`Game::install_frame(level)`, which inserts `CurrentStack` **and** builds
`FrameWanderers` from `Frame::wanderers` minus that frame's `slain`, resolving
each species as it goes. With no other writer of `CurrentStack`, the compiler
holds the pairing: a frame cannot be installed without its wanderers.

A frame whose `wanderer_species` resolves to `None` (a habitat with an empty
pool) places no live wanderer for that home. A quiet frame, the way
`place_orphan` degrades.

## 4. The turn — `Game::run_wanderers`, one function

Slots into `Game::arrive` between `take_fault` and `maybe_stack_encounter`:

```
announce_passage
bleed_corruption
open_cache
rouse_lair
trip_breakpoint
take_fault
run_wanderers        <- new
maybe_stack_encounter
```

`arrive` is the one way to arrive on a cell, so a step, a phase through a
wall and a wild jump all advance wanderers, for free and identically. Placing
this after `take_fault` keeps that rule's reason intact: a party that fell
meets what lives in the frame they *landed* in.

`run_wanderers` does four things in this order, and the order is the whole of
it:

1. **Contact at the party's own cell.** You walked into it. Checked first
   because the party has already moved and no wanderer step should get to
   happen first.
2. **Wake.** Any sleeping wanderer whose `at` lies in `visible_rows(level, x,
   y, facing)` and no further than `notice_range(trace_band)` cells ahead.
3. **Step.** Every awake wanderer moves one cell toward the party.
4. **Contact again.** It walked into you.

### Waking

`visible_rows`, never `view_cone` — **symmetric sight**: if you can see it, it
can see you. That reuses the one sight walk rather than inventing a second,
which is the exact drift the seam file records (`describe_view_direction`
walked the raw cone on its own and could describe a cache through a shut
door).

`notice_range` is a `tuning.rs` array indexed by `TraceBand::index()`, running
1 at `Quiet` up to `STACK_VIEW_DEPTH` (4) at `Hunted`. So a quiet party can
spot one at the far end of a corridor and back away; a hunted party is seen by
everything it sees.

Waking logs **on the edge only** — the transition, not the state, which is
`set_machine_status`'s rule. `MessageKind::Outcome`, so it survives
`retain_outcomes_since_battle` the way the Trace band-crossing line does.

### Stepping

Step to the neighbouring walkable cell with the lowest `distances_from(level,
party)` — the BFS already in `stack.rs`, which `far_half_floor` and
`fault_landing` both use. Ties break on a fixed `(dx, dy)` order, so movement
is deterministic and testable.

**Tethered to home**, with `NestGuardian`'s rule and not the simpler one: a
step is refused only when it *both* leaves `STACK_WANDERER_TETHER` of home
**and** fails to close on home. The plain radius check froze a displaced
guardian for the rest of the run, and this inherits that fix rather than
rediscovering it. The tether is also what makes avoidance real — walk far
enough and it cannot follow.

Iteration is over `FrameWanderers` in **home order**, which `install_frame`
builds from `Frame::wanderers` and never re-sorts, so two wanderers contending
for one cell resolve the same way every run — `assembler_system`'s reason for
sorting machines by `(x, y)`.

A wanderer never steps onto another wanderer's cell, and never onto the
party's cell if step 1 already resolved contact this call.

### Contact

Contact **spends** the wanderer: drop it from `FrameWanderers`, insert its
home into `FrameMemory::slain`, then hand off to the path an ambush already
uses — its stored `species` → `stack_escalation(depth)` → `spawn_pack` → tag
`StackSpawn` → `remember_fight` → `start_battle`. Depth scaling, Trace stat
and group multipliers, and the `StackSpawn` sweep in `end_battle` are all
inherited rather than re-derived.

`spawn_pack` rather than a single body, so a wanderer fields the group its
species and the depth have earned — the glyph in the corridor is one program,
and what it brings is the frame's business.

**Spent at contact, not at victory** — `open_cache`'s rule, not
`rouse_lair`'s. Walking onto a cache empties it whatever happens next. The
alternative needs a `BattleState` field carrying which wanderer this fight is
and a second `mark_lair_cleared` to write it at teardown, for one behavioural
difference: a Forgiving death would return the wanderer to its cell. That is
not worth a second field on `BattleState` and a second teardown path.

Contact refuses, leaving the wanderer where it is, when
`is_game_over().is_some()` or `has_active_battle()` — the two guards
`maybe_stack_encounter` already holds.

## 5. Ambushes — the Trace gate

`maybe_stack_encounter` returns early when `self.trace_band() ==
TraceBand::Quiet`.

One line. Above `Quiet` the existing `TRACE_ENCOUNTER_MULT` array applies
unchanged, so the loud end of the meter is untouched and only the quiet end
moves. This is what makes the four bands mean something a player can name:
quiet buys you *fighting only what you can see*.

## 6. Views — `crates/engine/src/views.rs`

`StackView` gains:

```rust
/// Same shape and order as `cells`. `None` on a cell nothing stands in.
pub occupants: Vec<Vec<Option<StackOccupant>>>,

pub struct StackOccupant {
    pub glyph: char,
    pub color: GlyphColor,
    pub name: String,
    pub awake: bool,
}
```

Filled from `resources::FrameWanderers` — which is why that resource carries
the resolved `species` and why the views can stay `&self`. Built in
`stack_view` over the **full `view_cone`**, matching `cells`, which is what
`stack_view` already builds over. Occlusion is not this function's
business: `draw_stack` paints back to front, so a near face covers whatever
stands behind it. That is the contract `cells` already lives under, and
occupants must live under the same one or the two grids stop being indexable
together.

Note the asymmetry, and that it is deliberate: **occupants ride the full cone;
waking and the map mark ride `visible_rows`.** The view may draw a wanderer
the renderer is about to paint a door over; nothing may *wake* or *map* one
behind that door.

`FrameMapMark` gains `Hostile`. Emitted only for wanderers whose `at` is in
`visible_rows` **right now** — a live sighting, never a memory. They move, so
a remembered position is a lie, and the seam already forbids the map marking
what the view never showed. `FERAL_DEV_REVEAL` reveals all of them, on the map
only, exactly as it does terrain.

`Game::describe_view_direction` names a wanderer ahead of the terrain it is
standing on: what is coming at you outranks what it is standing on.

## 7. Drawing — `crates/gui/src/render/`

**`render/stack.rs`.** `draw_cell` takes the occupant alongside the cell and
draws its glyph **instead of** `cell_mark`'s where both land — a hostile
standing on a cache is the thing you must react to. Same `MARK_FOG` curve a
mark fades on, not the geometry's `FOG`, and for the same reason: this is the
one thing in the pane that exists to be spotted from the far end of it.

Asleep is **dimmed**, awake is **full bright**. Brightness is this pane's only
spare channel — hue is the authored species colour and stays that, drawn
through `hud::palette::glyph`, because *a program reading as one colour on the
grid and another in the corridor is the failure to avoid*. Depth already
spends brightness through `MARK_FOG`; the sleep dim multiplies on top of it
rather than replacing it.

`cell_mark` stays exhaustive over `StackCellView` and gains no arm — a
wanderer is not a cell kind, which is precisely why it can move.

**`render/frame_map.rs`.** One arm for `FrameMapMark::Hostile` in the shared
glyph table. The frame map is drawn twice and defined once, so `draw_frame_map`
and `draw_map_inset` both get it from that single edit. The colour is a
palette role, not a literal.

---

## Testing

Engine, in `crates/engine/src/tests/` (fixtures live in `tests/support.rs` —
look there before writing a new one):

- **Placement is pure.** The same `FrameSpec` generates identical
  `Frame::wanderers` twice.
- **Placement draws no `GameRng`.** Generate a frame, assert the `GameRng`
  stream is where it was.
- **Placement respects entry clearance**, over many seeds: no home cell within
  `STACK_WANDERER_ENTRY_CLEARANCE` of `entry`.
- **Placement takes no dead end**, so caches and orphans keep theirs.
- **Adding `place_wanderers` moved no existing placement** — a frame's caches,
  orphan, market, links and lair are unchanged for a fixed spec. This is the
  test that pins "runs last".
- **Species is stable across a reload** — regenerated, not saved, so this
  asserts the derivation and not a field — and **differs between two wanderers
  in one frame**, which is what the `home` salt term buys.
- **A loaded frame has its wanderers home and asleep**, and a frame left and
  re-entered does too. This is the documented consequence of not saving them;
  a test pins it so nobody later reads it as a bug.
- **Wake threshold per band**: a wanderer at N cells sleeps at `Quiet` and
  wakes at `Hunted`.
- **A wanderer behind a shut door does not wake**, which is `visible_rows`
  doing its job.
- **Waking logs once**, not once per turn while awake.
- **The tether refuses the step that leaves it**, and **a displaced wanderer
  outside its tether still steps home** — the second is the regression the
  `NestGuardian` rule exists for and the one a naive radius check fails.
- **Contact both directions** starts a battle: party steps onto wanderer, and
  wanderer steps onto party.
- **Contact spends it**: leave the frame, come back, it is gone.
- **`slain` survives a real save → load**, not only the RON round trip — a
  `#[serde(skip)]` or a missed wiring leaves the round-trip test green.
- **`enter_next_zone` clears it** with the rest of `StackMemory`.
- **The ambush roll is refused at `Quiet`** and still fires above it. Written
  so that deleting the gate fails it.

GUI, in `crates/gui/src/render/stack.rs`'s own test module:

- **The occupant glyph is drawn and the terrain mark is not**, where both land
  on one cell. Assert the absent mark, not only the present glyph — this is
  the `sprite` seam's overdraw lesson: asserting only what appears passes
  against a draw that also emits what it was meant to replace.
- **A sleeping occupant is dimmer than a waking one at the same depth.**
- **An occupant still outshines the geometry at an empty reserve**, matching
  the existing `a_mark_still_outshines_the_geometry_at_an_empty_reserve`.
- **Drawing a view with occupants at every depth does not panic**, matching
  the existing degenerate-view tests.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.

## What no test can reach

**`balance_sim` has no Stack term at all**, so none of this is gated there —
and the arena, which is the only instrument for a Stack fight, stages an
authored composition and cannot see *frequency*. This change moves how often
the party fights underground in two directions at once: down at `Quiet`, where
the ambush stops firing, and up wherever wanderers are met. Neither figure is
measurable by anything in the repo.

So the tuning of `STACK_WANDERERS_PER_FRAME`, `notice_range` and
`STACK_WANDERER_TETHER` is a **playtest question**. `FERAL_DEV_REVEAL=1 cargo
run -- --template stack` opens on a frame with the map lit, which is the
instrument that exists.

A green suite is not evidence this reads right. Whether a wanderer at four
cells is legible against the fog, whether the sleep dim says "asleep" rather
than "far away", and whether the tether lets you actually walk around one are
all things a human has to look at.

## Seam paperwork

If this ships, one seam is added, in the order `.claude/skills/seams/SKILL.md`
sets out — the argument to `docs/seams.md`, the trap to
`.claude/skills/seams/references/stack.md`, the one-sentence rule to
`CLAUDE.md` under **The Stack**. The candidate sentence:

> **A wanderer is a record and never an entity, and only its death is
> saved.** Placement is pure in `FrameSpec`, `FrameWanderers` is rebuilt by
> `install_frame` exactly as `CurrentStack` is, and `FrameMemory::slain` is
> the one half that outlives the frame.

Two existing entries want a line each rather than a rewrite:
`maybe_stack_encounter` is now Trace-gated, and `StackView::occupants` rides
the full cone while waking rides `visible_rows`.

`CHANGELOG.md` and a version bump happen at the merge, not on the branch.
