# The Stack

Renames the dungeon layer to fit the game's register, then gives it the
three things it currently lacks: a reason not to clear it exhaustively,
more than one kind of floor, and inhabitants that aren't fights.

Five phases, each shippable alone. Phase 1 is inert; phases 2–4 each add
mechanics; phase 5 is renderer-only.

## Status

Updated 2026-08-02. Each phase gets its own plan, written when its
predecessor lands — not up front, since a plan written against vocabulary
that does not exist yet goes stale.

| # | Phase | State | Save bump | Crates |
| --- | --- | --- | --- | --- |
| 1 | **The rename** — Stack, frames, links | ✅ **done**, merged `1ffa7ca` | no | engine, app-core, gui |
| 2 | **Trace** — greed-driven pressure, escalating ambushes | ✅ **merged** `b4a2e07`; band 1 retuned from the crawl, bands 2–3 unplayed | **yes** (15 → 16) | engine, gui |
| 3 | **Cell kinds** — breakpoint, fault, corruption | ✅ **merged** `97fe0ce`; the gating crawl met none of the three | **yes** (16 → 17) | engine, gui |
| 4 | **Inhabitants** — orphaned process, derelict trader, crash log | not started | **yes** | engine, gui, assets |
| 5 | **Corner map inset** | not started | no | gui only |

Phase 1's plan is at
`docs/superpowers/plans/2026-07-31-the-stack-phase-1-rename.md`. It was
amended six times mid-execution and those amendments are the useful part —
they record where the plan was wrong, which is the same shape of error the
later phases will make.

### Open question carried into phase 2 — resolved

**Should phase 5 (the corner map) be pulled forward, ahead of Trace?**
**No** — decided 2026-08-01. It stays last, so the shared glyph function is
written once against all three new cell kinds instead of being touched again
in phase 3. The cost accepted in exchange is that every phase-2 playtest
walks without a persistent map, hitting `g` for the full screen.

### What phase 1 did *not* change

Anything visible, beyond five strings. The Stack looks and plays exactly as
the dungeon did. The frame map is still a full-screen mode on `g`
(`app/playing.rs:192`); there is no corner inset, and there will not be one
before phase 5.

### Before building on phase 2

The spec's closing note stands and is the most important line in this
document, with one qualification earned on 2026-08-01. The per-source
*ratios* are now grounded in a measured frame — a kill has to be worth far
less than a cache, or the meter stops being about greed — but the **band
thresholds are still arithmetic and nothing more**. Where the lines go is
exactly the part no measurement can settle.

Capture a `dev-saves/` template and actually play phase 2 before phases 3
and 4 are built on top of it.

**Amended 2026-08-01, when phase 3 was designed.** The gate moved rather than
being dropped: phase 3 is designed and built against the unplayed bands, and
the playtest happens **before it merges**, covering Trace and the three new
cell kinds in one crawl. What that buys is a single session answering both
sets of questions instead of two; what it costs is that if 40/100/180 turn
out wrong, `TRACE_PER_BREAKPOINT` was priced against them and moves too.
Deferring the evidence, not skipping it — the branch does not land until the
crawl has happened.

**The crawl happened, and that is exactly what it cost.** Played
2026-08-01 on `dev-saves/stack.ron`: a cache, four fights, about a third of
a frame, and the meter never left **Quiet**. Working back from that found an
arithmetic fault rather than a matter of taste — three caches at 10 is 30
against a first band at 40, so stripping a whole floor of everything in it
still read Quiet. Thresholds are now **25/70/140**, and
`stripping_a_frames_caches_is_enough_to_be_noticed` pins the relationship so
it cannot silently go inert again. `TRACE_PER_BREAKPOINT` stayed at 25 and
its argument changed underneath, as predicted above. Only the first band is
evidence-backed; the session came nowhere near the other two.

## Why

The descent is currently a checklist. You walk a braided maze, crack every
cache, burn every seal, kill the thing at the bottom, and climb out. Nothing
punishes doing that slowly and completely — you can retreat, rest at Home,
and return to a map you keep and a lair that stays dead. There is no moment
where the shaft asks you a question.

It is also thin. One walkable cell kind, one hostile interaction (a rolled
encounter), one point of interest (a cache), and one boss.

And "dungeon" is the wrong word for it. Every other place-noun in this game
is a computer word used as a physical place — Data Void, Static Field, Null
Sector, Black Ice, iso Market. "Dungeon" is the only fantasy import, and it
survives mostly in code and docs: exactly three player-visible strings
contain the word "dungeon".

That count was used during design to argue the rename is nearly free at the
surface, and it is too narrow — corrected during Task 2 of phase 1. Two more
player-visible strings say "Stairs" (`game/dungeon_view.rs:213,215`), which
the vocabulary change also moves. Five strings, not three. The conclusion
holds — this is still overwhelmingly a code-and-docs rename — but "three"
was measured against one word rather than against the vocabulary.

## Vocabulary

| now | becomes | why |
| --- | --- | --- |
| dungeon | **the Stack** | a stack is what a vertical run of frames *is* |
| dungeon level 3 of 5 | **frame 3 of 5** | frames are what a stack is made of |
| breach (`>`), stairs up, stairs down | **link** | one word for every connection you travel along |
| shaft (one entrance's run) | **a stack** | "each link opens its own stack" |
| breach (verb, zone travel) | unchanged | now unambiguous |
| lair | unchanged | the one place that should feel less like a machine |

Three new cell kinds, named to the same register:

- **breakpoint** — a place you stop and inspect state. Not "terminal":
  `assets/structures/terminal.ron` already ships a structure by that name.
- **fault** — a fault traps you into a deeper frame. Not "trap" or "jump":
  `use_symlink` already owns the longjmp.
- **corruption** — walkable ground that costs you to cross.

The surface link and the frame-to-frame links share a word deliberately. They
are the same thing at different scales, and the map and first-person view
already distinguish them by context rather than by name. The glyphs already
agree with this: the surface entrance and the down-stairs are both `>` today
(`render/dungeon_map.rs::cell_glyph`), so the naming is catching up to the
rendering rather than changing it.

### One collision, resolved

`crates/gui/src/render/dungeon.rs` already uses **frame** to mean a corridor
cross-section at one cell of distance — `fn frame`, the `SHRINK` constant,
the module doc. That is private, appears about five times, and is read by
nobody but the renderer. The player-facing word wins: the renderer's term
becomes **slice**, as part of phase 1.

## Phase 1 — the rename

Inert. Identifiers, module names, docs, three UI strings.

- `Locale::Dungeon` → `Locale::Stack`, `DungeonMemory` → `StackMemory`,
  `LevelMemory` → `FrameMemory`, `DungeonPos` → `StackPos`,
  `dungeon.rs` → `stack.rs` (engine, `game/`, `tests/`, gui `render/`),
  `CellKind::StairsUp`/`StairsDown` → `LinkUp`/`LinkDown`, the `DUNGEON_*`
  tuning consts, `DungeonMapView` and friends.
- `render/dungeon.rs`'s private `frame` → `slice`.
- Player-visible strings: `render/meta.rs:151`, and the two descend/ascend
  log lines in `game/dungeon.rs`.
- README's "Dungeons" section, `docs/manual.md`'s "In a dungeon:" section,
  and CLAUDE.md's load-bearing-seams entries, which name these types
  directly.

**No `SAVE_FORMAT_VERSION` bump.** The save is bincode with
`bincode::config::standard()`, which is positional — no field names, enum
variants by index. Verified in `crates/engine/src/save.rs:201-206`, which
documents this explicitly. Renaming moves no encoded byte. Variant *order*
must not change.

Lands as its own commit before any feature work, so everything after is
written in the new vocabulary and no reviewer has to separate "renamed"
from "changed".

## Phase 2 — Trace

A meter that rises with what you take, and escalates what comes for you.

Designed 2026-08-01 against a measured frame. Four claims in the original
sketch were wrong and are corrected below, each in place: where Trace
lives, what "pack size through `depth_mult`" means, whether Hunted draws a
boss, and how much a kill can be worth.

### The measurement everything else follows from

A frame is 21×21 with **~206 walkable cells, 3 caches, and seals only on
the bottom frame** (2 of them, walling off the lair). The encounter roll is
`STACK_ENCOUNTER_CHANCE` = 0.08 **per successful step, with no cooldown**
(`game/stack.rs`, in `step`).

So a thorough crawl of one frame is ~300 steps with backtracking, and
therefore **~24 encounters against 3 caches**. That ratio is the phase's
central design constraint:

- If a kill is worth anything close to a cache, Trace is a **combat meter,
  not a greed meter** — which inverts the load-bearing choice below.
- Worse, it is a **runaway loop**: more Trace → more encounters → more
  kills → more Trace.

Two rules fall out, and the tuning table obeys both. **A kill is worth a
fraction of a cache**, because it is the high-frequency source and so sets
the floor rather than the ceiling. And **encounter chance gets the gentlest
multiplier of the three**, because it is the only lever that feeds back
into its own input; the teeth go into the stat multiplier, which feeds back
into nothing.

A seal is close to a non-source — two per stack, both on the bottom frame,
so it will rarely fire before the lair. It is kept for completeness, at a
gain that reflects being a genuine cost the player paid a shard for.

### Where it lives

A **`Trace(u32)` resource**, cleared in `clear_stack`.

Not a `trace: f32` field on the `Locale::Stack` variant, which is what this
document originally specified on the reasoning that `clear_stack` would
drop it for free along with the variant. That reasoning is right about the
reset and wrong about the carry: **`descend_to` and `ascend_to` each
construct a fresh `Locale::Stack` wholesale**, so a field on the variant is
silently zeroed every time the party changes frame — precisely when Trace
should be accumulating. A field would need two carry-forward sites that a
future frame transition can forget; a resource needs zero, and resets in
`clear_stack`, which CLAUDE.md already establishes as the single exit door
that `use_symlink` goes *through* rather than around.

`u32` rather than `f32`: exact band comparison, exact save bytes,
deterministic tests, and no accumulated float error over a long dive.

**Bumps `SAVE_FORMAT_VERSION` 15 → 16** for the new `SaveData` field.
Persistence is not optional — without it, saving mid-dive is a free Trace
reset, which is a far worse exploit than the priced one below.

### What raises it

Taking, not walking. Cracking a cache, burning a seal, killing a hostile.
Walking is free.

This is the load-bearing choice. A time-driven meter would tax exploration
and map-making directly — rewarding the beeline and punishing the careful
player, which is backwards for a maze whose per-frame map memory exists
precisely to reward learning it. A greed meter makes risk proportional to
reward and leaves the crawl alone.

Hook sites, one per source:

| Source | Gain | Hook |
| --- | --- | --- |
| Cracking a cache | **10** | `open_cache` (`game/stack_features.rs`) |
| Burning a seal | **5** | `pass_seal` (same file) |
| Killing a hostile | **2** each | `award_loot` (`game/combat_rewards.rs`) |

`award_loot` rather than anything in the battle teardown because it is the
one place that knows a hostile actually *died* rather than being fled from
— the same reason `mark_lair_cleared` already lives there.

**Descending does not raise Trace.** It was considered and cut: it is
walking, not taking, and depth already has its own escalation curve in
`STACK_DEPTH_STAT_GROWTH`.

### Bands

Four names — **Quiet / Noticed / Traced / Hunted**. They are labels on one
continuous curve, not four discrete mechanics.

| Band | From | Encounter | Stats | Group |
| --- | --- | --- | --- | --- |
| Quiet | 0 | ×1.0 | ×1.0 | ×1 |
| Noticed | 40 | ×1.25 | ×1.10 | ×1 |
| Traced | 100 | ×1.6 | ×1.25 | ×2 |
| Hunted | 180 | ×2.0 | ×1.45 | ×3 |

Against a realistic ~120-step frame — ~10 encounters, ~15 hostiles, 3
caches ≈ 60 Trace per frame — a thorough player crosses into Noticed during
frame 1 and **arrives at the lair Hunted**; a beeliner arrives around
Noticed. That difference is the question the shaft is supposed to ask.

**Hunted does not draw from the boss pool.** The original sketch called for
it. It is cut, because `maybe_stack_encounter` documents the opposite rule
in place — *"a fight you never saw coming should not also be the hardest
fight available"* — and reversing a decision that carries its own reasoning
needs a better argument than escalation wanting a spike. The band's teeth
are the three multipliers.

### How escalation is applied

- **Encounter chance** — the band multiplier scales
  `STACK_ENCOUNTER_CHANCE` at the roll in `maybe_stack_encounter`.
- **Stats** — folded into `Game::stack_depth_multiplier`. Its only two
  callers are the ambush and the lair, so the guardian is buffed by the
  party's own greed for free, with no second code path to drift out of
  sync.
- **Group size** — a **parameter** threaded into `spawn_pack`, never read
  off the `Trace` resource inside it. That function's doc comment already
  documents this exact trap: a locale-derived multiplier read inside the
  spawn leaked into surface nest respawns, which keep rolling on every
  `tick` while the party is underground, and left 3× programs standing
  around the link mouth for the climb out. A Trace-derived multiplier would
  reproduce it precisely.

Threading a second scalar makes `spawn_pack` a six-parameter function with
two bare multipliers whose order is easy to swap. Whether that becomes a
small `PackScaling` value with a `SURFACE` constant — which would also let
the two surface call sites stop passing bare `1.0` — is a structural
choice, and goes through the `design-patterns` dialog at implementation
rather than being settled here.

Note that the group lever is **inert in zone 1**, where `zone_group_cap(1)`
pins every group to a single member. The tuning constants say so, so that
it is not later filed as a bug.

### Why resetting on exit is safe, and where it isn't

Trace vanishes when you surface. For caches and seals that is
self-limiting, and no decay mechanic is needed: both are one-shot per
stack, recorded in `FrameMemory`, so climbing out to shed Trace means
returning to a stack with less left to take.

**The lair is the exception, and it is a priced escape hatch rather than a
closed one.** The guardian stays un-spent until killed, and the party keeps
its map — so a player can loot a stack to Hunted, climb out, and walk the
known shortest path back down to meet the boss at Noticed. What that costs
is the walk itself: ~19 fights of attrition on the way down, at no reward,
since the caches are already empty. That is a real trade, and it is
recorded here as a known, accepted price rather than an oversight.

### The readout is not decoration

The band appears in the Stack HUD as a `TraceBand` on `StackView`, appended
to the existing `Facing N   Depth 1 / 3   (x, y)` heading in
`render/stack.rs`. Each crossing is logged as `MessageKind::Outcome` so it
survives `retain_outcomes_since_battle`.

Escalating ambushes with no visible cause are experienced as bad luck, not
as consequence. Without the HUD element this phase is a difficulty curve
nobody can see.

**Band only, never the raw number** — it is a threat readout, not a
progress bar, and a visible integer invites playing to the threshold
instead of to the risk. Crossings are monotonic within a dive (no decay,
reset only on surfacing), so only a rise is ever logged.

The full-screen `g` map does **not** get the band in this phase. It is a
one-line addition if playtesting says the decision to press on is being
made from that screen.

**No app-core change.** Trace adds no key, no mode and no screen, so the
input-and-flow state machine is untouched — the readout is engine state
that gui already asks for. This corrects the crate list in the status
table.

No persistent Stack entities are introduced. There is no pursuing hunter.

### Tuning

Band thresholds, per-source Trace gain, and the per-band encounter, stat
and group multipliers are `pub const` in `tuning.rs`, in a labelled
section, per the difficulty-is-code rule.

**Every number in the tables above is arithmetic against a measured frame
and nothing more.** The measurement makes the *ratios* defensible — a kill
must be worth far less than a cache, and that is now grounded rather than
guessed. It says nothing about whether 40 / 100 / 180 are the right places
to put the lines, which only playing can answer.

## Phase 3 — cell kinds

Designed 2026-08-01, against the source rather than against this document's
own sketch — which named `LevelSpec::rng_seed` and `dungeon::generate`, both
of them pre-rename spellings that phase 1 replaced with `FrameSpec::rng_seed`
and `stack::generate`. That is the third time this spec's forward-looking
prose has aged badly, and the reason each phase gets designed when its
predecessor lands rather than up front.

Three new `CellKind` variants — `Breakpoint`, `Fault`, `Corruption`. All
three are `walkable()`, none `blocks_sight()`.

**Bumps `SAVE_FORMAT_VERSION` 16 → 17** for the breakpoint record.

### Placement, and the order it happens in

Each kind gets its own site type, so no two compete for the same cells and
the existing caches keep every dead end to themselves:

```
carve_maze → braid → far cell (LinkDown|Lair) → LinkUp → seal_the_lair
  → place_doors        corridors: exactly 2 opposite exits    [existing]
  → place_breakpoint   junctions: 3+ exits                    [new]
  → place_faults       plain Floor, not dead ends             [new]
  → place_corruption   plain Floor, not dead ends, in patches [new]
  → place_caches       dead ends                              [existing]
```

Every new pass builds only on `CellKind::Floor`, the discipline
`place_caches` already follows, so none can pave over a link, the lair, a
door or a cache. Because all three are walkable, placing them before
`place_caches` leaves `is_dead_end` topology unchanged — the cache pass sees
exactly the dead ends it would have seen without them.

**Breakpoint sits on a junction, not a dead end.** A dead end is the natural
home for a reward earned by a walk, but caches already own those, and a hub
reads better for a thing that is infrastructure rather than loot.

**Faults are generated only on frames that have a way down**
(`!spec.is_bottom()`). A fault on the bottom frame has nowhere to drop you,
and one that is inert is a lie the player cannot see from the cell.

### Breakpoint

Fires from `Game::step`'s post-move block, beside `open_cache`. Inserts
every in-bounds cell of the frame into `FrameMemory::seen` — walls included,
so the `g` map draws as a complete frame rather than a floor plan floating in
nothing — then calls `raise_trace(TRACE_PER_BREAKPOINT)`.

One-shot, recorded in a new `FrameMemory::jacked: BTreeSet<(i32, i32)>`. A
set rather than the `bool` that `cleared` uses, so raising
`STACK_BREAKPOINTS_PER_FRAME` above 1 later cannot silently break it. Both
halves live in `game/stack_features.rs` beside the cache/seal/lair pairs,
with a `breakpoint_spent` reader for the two views.

`TRACE_PER_BREAKPOINT = 25`, against cache 10, seal 5 and kill 2. A free map
costs two and a half caches of noise: dear enough to be a decision, not so
dear that nobody ever pays it. Written against `TRACE_NOTICED = 40`, where
that was a breakpoint plus two caches to cross the first band; after the
retune to 25 it crosses on its own, which reads better for the loudest
action in the game. The number did not move, only what it buys.

**The Power-for-a-buff half of the original sketch is cut.** Two options
need a prompt, a prompt needs a `Mode`, and a `Mode` drags app-core into a
phase billed as engine-and-gui. Walking on reveals the map and nothing else.
If the reveal alone reads thin in playtest, the buff is a later phase with a
UI budget of its own.

### Fault

`descend_to` and `ascend_to` (`game/stack.rs`) are already one function
differing only in landing cell; the fault is the third caller. They collapse
into a single `enter_frame(depth, frames, entrance, landing)` rather than
gaining a third near-copy.

The landing cell is drawn from the frame below's plain `Floor` cells,
furthest from that frame's `entry`, from an RNG salted off its own
`rng_seed` — the scheme `pick_lair_species`'s `LAIR_SALT` established, not a
second one. `Floor` specifically, so a fall can land on neither the lair, a
cache, nor another fault.

Not one-shot, so no `FrameMemory` record: it is terrain, and climbing back to
fall again is allowed. **Raises no Trace** — falling is clumsy, not loud, and
Trace is a greed meter.

Inside `step` the fall happens after `open_cache` and `rouse_lair` (a fault
cell is neither) and **before** `maybe_stack_encounter`, so the ambush roll
happens in the frame the party landed in. Landing somewhere strange and being
jumped there is the right reading of the order.

### Corruption

Bleeds HP on each step onto a corrupted cell, through `Game::apply_damage`
and never a direct `Stats::hp` write. That routes it through
`mitigate_incoming_damage`, so a Mitigation field buff blunts it — accepted
as correct rather than as a leak: a mitigation field is exactly the thing
that ought to help here.

**The player only, not the party.** Hitting party members would pull in
`announce_program_death` and the permadeath path for something that is not a
fight.

**Placed in patches of `STACK_CORRUPTION_PATCH_CELLS`, not as single cells.**
A lone corrupted cell is a toll booth — one hit, walk on. A contiguous
stretch is something a player can decide to walk around, which is the
"routing decisions in a maze that has exactly one kind of walkable cell" this
phase exists to create. Each patch is a seed cell plus a short walk along
walkable neighbours.

**Damage is a percent of max HP** (`STACK_CORRUPTION_HP_PERCENT`) with a flat
floor (`STACK_CORRUPTION_MIN_DAMAGE`). The player is 90 HP at level 1
(`PLAYER_BASE_STATS`) and around 510 mid-run, and Stack depth is uncorrelated
with player level — so a flat figure, or one scaled by depth the way cache
payout is, would be lethal at level 1 and free at level 20. A percentage
costs the same fraction of the bar at either end.

Corruption can kill, and should. It reaches `is_game_over` through the
`tick()` that ends every step.

### Constraints these inherit

- Every one of them is walkable and none blocks sight, so none inherits the
  door trap CLAUDE.md documents — the one where a cell that is both walkable
  and sight-blocking fills the first-person view and truncates the map. Any
  later variant that *does* block sight must handle the `ahead == 0`
  exception in both `remember_view` and `draws_as_face`.
- Generation must not draw from `resources::GameRng`. Placement salts off
  `FrameSpec::rng_seed`, like everything else in `stack::generate`.
- Placement counts go in `tuning.rs`. The kinds themselves stay engine code —
  same argument as `Perk` variants: each is a hook into generation and step
  handling with no shared shape to express as data.
- The frame map's glyph table stays **one** definition. Phase 5 extracts it
  for the corner inset; phase 3 must not fork it in the meantime, which is
  precisely the drift CLAUDE.md's mirroring rule warns about.

### New tuning constants

`STACK_BREAKPOINTS_PER_FRAME`, `STACK_FAULTS_PER_FRAME`,
`STACK_CORRUPTION_PATCHES_PER_FRAME`, `STACK_CORRUPTION_PATCH_CELLS`,
`STACK_CORRUPTION_HP_PERCENT`, `STACK_CORRUPTION_MIN_DAMAGE`,
`TRACE_PER_BREAKPOINT`.

## Phase 4 — inhabitants

- **Orphaned process** — a fragmented program in a dead end that joins for
  an item rather than by winning a capture roll. A second route into the
  party, and a reason to descend with a slot free. Reuses the existing
  `Party` and tamed-program machinery.
- **Derelict trader** — buys and sells deep, at a markup. **This knowingly
  punches a hole in `require_surface`**, which currently guards trade in four
  places (`game/trade.rs:30,180,362,397`). Underground trade must not touch
  the zone map through `Position`, which is what that guard exists to
  prevent — so this is a narrower entry point, not a relaxed guard.
- **Crash log** — readable remains of the last party: a message, sometimes a
  hint about what is below.

Crash log text lives in `assets/` as data, not in Rust — same rule as item
and structure descriptions. A malformed file is skipped with a logged
warning, following `ItemDb::load_dir`.

Orphaned processes and crash logs each need a spent-ness record in
`FrameMemory`, so this phase bumps the save format too.

## Phase 5 — the corner map

The frame map, drawn as an inset in the top-left of the first-person pane,
always visible while in the Stack.

`Mode::DungeonMap` and its `g` binding stay. The two do different jobs: the
inset answers "which way am I facing and where have I been" at a glance; the
full screen answers "where is the unexplored wing" with roughly three times
the cell size and the legend that teaches the glyphs.

Renderer-only. No engine change — `Game::dungeon_map` already returns the
view this needs. No save change.

### Why last

By this point the map has three new cell kinds to draw and Trace state to
reflect. Building the shared glyph path once, at the end, beats touching it
in five consecutive phases.

### Shape

`draw_dungeon_map` currently draws at absolute coordinates from `(0,0)` and
adds a heading and legend, so this is not "call it with a smaller `w`/`h`".
The grid loop, `tile_color`, `cell_glyph` and `mark_glyph` are extracted into
one function taking an origin, which both callers use.

The two maps must be **a call, not a copy**. A duplicated glyph table is
precisely the drift CLAUDE.md's mirroring rule warns about, and the map is
the one screen where a stale glyph silently misinforms.

### Placement

The playing pane is 70% × 72% at the origin (`render/base.rs:72-73`). The
corridor's vanishing point is at that pane's centre, so its corners are
ceiling and floor wedges — the least information-dense pixels on screen.

Sizing is the real risk. At 1920×1080 the pane is 1344×778, so a 28%-wide
inset gives ~18px cells. At 1280×720 it is ~12px cells with ~9px glyphs:
legible for tiles, tight for the `!`/`&`/`+` markers. The inset fraction is a
presentation constant in the renderer, not a `tuning.rs` value.

## Testing

Per phase, failing test first.

- **Phase 1** — the existing suite is the test. It must pass unchanged in
  count. Additionally: a save written before the rename still loads after
  it, which is the claim the no-bump decision rests on.
- **Phase 2** — Trace rises on cache, seal and kill and not on a plain step;
  band thresholds map to the right band; surfacing clears it; a save/load
  mid-dive preserves it; each band crossing logs an `Outcome`-kind line;
  escalation actually changes the encounter roll, the stat multiplier and
  the group size. Seeded, no wall-clock, no unseeded RNG.

  Three of these are regression tests for corrections this design made, and
  are the ones worth writing first, since each guards a trap the original
  sketch walked into:

  - **Trace survives a descent and an ascent.** This is the whole reason it
    is a resource and not a variant field, and it fails against the
    original design.
  - **A Trace-escalated group multiplier does not touch a surface spawn.**
    `spawn_pack`'s doc comment records this leak happening once already
    with `depth_mult`; tick the surface while underground at Hunted and
    assert ambient spawns are unscaled.
  - **A frame's cache, seal and walkable counts** — the measurement the
    tuning rests on. Left unasserted, a later generator change moves the
    kill-to-cache ratio and silently turns the greed meter into a combat
    meter, with the suite still green.
- **Phase 3** — each kind generates within its tuning count; placement is
  identical for a given `FrameSpec` across two `generate` calls; corruption
  routes through `apply_damage`; a fault lands on `Floor` and not on the
  frame's `LinkUp`; a spent breakpoint stays spent across leaving and
  re-entering the frame.

  Four more, each guarding a placement rule the design above rests on and
  none of which a naive count test would catch: no new kind lands on a
  cache, a link, a door or the lair; the bottom frame generates no faults at
  all; the cache count is unchanged by the three new passes running before
  it, which is the claim "dead ends stay whole" makes; and a corruption
  patch is contiguous rather than three cells scattered across the frame.
- **Phase 4** — underground trade works and still cannot reach the zone map;
  a recovered orphan lands in `Party` and does not recur; a malformed crash
  log file is skipped, not panicked on.
- **Phase 5** — the inset fits its rect at both window sizes above and does
  not overlap the status panel or log; the glyph table has exactly one
  definition; drawing a degenerate view does not panic, matching the
  existing test in `render/dungeon_map.rs`.

## Gates

`cargo test --workspace` before each phase is called done, plus
`cargo clippy --workspace` and `cargo fmt`.

Phases 2 and 3 change spawn rates and add an HP drain, so both also run
`cargo test -p feral-processes-engine balance_sim`. A moved curve means
progression changed — that is the signal, not a broken test.

Phases 2–4 each change save format. Bump `SAVE_FORMAT_VERSION` once per
phase, and re-capture any `dev-saves/` template the change invalidates.

**Phase 2 invalidates none of them.** Templates are field-named RON, not
`.bin` — `crates/launcher/src/dev_template.rs` documents that a new
`#[serde(default)]` field still parses and that `generate` stamps the
current version on the way out. `dev-saves/extraction.ron` needs no
re-capture, and neither will phases 3 and 4 if they add fields the same
way.

## Out of scope

- **Dark strata** — frames where the view cone shortens. Cut. It is the one
  addition that would need a walkable sight-blocker, inheriting the door trap
  in both cone consumers.
- **A pursuing hunter** — a Stack entity that occupies a cell and paths
  toward you. Cut in favour of escalating ambushes; no phase here introduces
  a persistent Stack entity, and adding one later is a real design decision,
  not an increment.
- **Trace decay over surface turns** — unnecessary given spent-ness makes the
  reset self-limiting for caches and seals. It would not close the lair
  hatch either, which is priced by the walk back down rather than by the
  time spent up top; decay is the wrong tool for the one case that leaks.
- **Renaming zone travel.** "Breach" keeps its verb and is now unambiguous.

## Playtest note

None of this has been played. The Trace band thresholds and per-source gains
in particular are arithmetic-plausible and nothing more — whether the meter
asks an interesting question is not something the suite can answer. Phase 2
is the one to capture a `dev-saves/` template for and actually play before
building phases 3 and 4 on top of it.
