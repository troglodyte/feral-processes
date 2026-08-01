# The Stack

Renames the dungeon layer to fit the game's register, then gives it the
three things it currently lacks: a reason not to clear it exhaustively,
more than one kind of floor, and inhabitants that aren't fights.

Five phases, each shippable alone. Phase 1 is inert; phases 2–4 each add
mechanics; phase 5 is renderer-only.

## Status

Updated 2026-08-01. Each phase gets its own plan, written when its
predecessor lands — not up front, since a plan written against vocabulary
that does not exist yet goes stale.

| # | Phase | State | Save bump | Crates |
| --- | --- | --- | --- | --- |
| 1 | **The rename** — Stack, frames, links | ✅ **done**, merged `1ffa7ca` | no | engine, app-core, gui |
| 2 | **Trace** — greed-driven pressure, escalating ambushes | specced 2026-08-01, not built | **yes** (15 → 16) | engine, gui |
| 3 | **Cell kinds** — breakpoint, fault, corruption | not started | **yes** | engine, gui |
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

- **Breakpoint** — jack in to reveal the current frame's map, or spend Power
  for a buff. The loudest thing you can do; spikes Trace hard. Needs a
  spent-ness record, so it is a new `FrameMemory` field.
- **Fault** — falls you one frame, landing away from that frame's up-link.
  Close to `descend` with a different landing cell.
- **Corruption** — bleeds HP per step. Creates routing decisions in a maze
  that currently has exactly one kind of walkable cell, and gives the map
  memory something worth having beyond "seen".

**Bumps `SAVE_FORMAT_VERSION`** for the breakpoint record.

### Constraints these inherit

- Every one of them is walkable and none blocks sight, so none inherits the
  door trap CLAUDE.md documents — the one where a cell that is both walkable
  and sight-blocking fills the first-person view and truncates the map. Any
  later variant that *does* block sight must handle the `ahead == 0`
  exception in both `remember_view` and `draws_as_face`.
- Generation must not draw from `resources::GameRng`. Placement salts off
  `LevelSpec::rng_seed`, like everything else in `dungeon::generate`.
- Corruption's HP loss goes through `Game::apply_damage`
  (`game/combat_damage.rs`), which is the only path that lowers a creature's
  HP. Not a direct `Stats::hp` write.
- Placement counts go in `tuning.rs`. The kinds themselves stay engine code —
  same argument as `Perk` variants: each is a hook into generation and step
  handling with no shared shape to express as data.

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
  identical for a given `LevelSpec` across two `generate` calls; corruption
  routes through `apply_damage`; a fault lands somewhere walkable and not on
  the up-link; a spent breakpoint stays spent across leaving and re-entering
  the frame.
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
