# The Stack

Renames the dungeon layer to fit the game's register, then gives it the
three things it currently lacks: a reason not to clear it exhaustively,
more than one kind of floor, and inhabitants that aren't fights.

Five phases, each shippable alone. Phase 1 is inert; phases 2–4 each add
mechanics; phase 5 is renderer-only.

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
contain it.

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

### Where it lives

A `trace: f32` field on the `Locale::Stack` variant — not a free-standing
resource.

Three reasons. It is genuinely part of where-you-are and how the place
regards you. `clear_dungeon` drops it for free by dropping the whole
variant, so there is exactly one place it can leak from. And it is saved,
which it must be: without persistence, saving mid-dive is a free Trace
reset — a far worse exploit than the retreat one below.

**Bumps `SAVE_FORMAT_VERSION`.**

### What raises it

Taking, not walking. Cracking a cache, burning a seal, killing a hostile.
Walking is nearly free.

This is the load-bearing choice. A time-driven meter would tax exploration
and map-making directly — rewarding the beeline and punishing the careful
player, which is backwards for a maze whose per-frame map memory exists
precisely to reward learning it. A greed meter makes risk proportional to
reward and leaves the crawl alone.

### Why resetting on exit is safe

Trace vanishes when you surface. That looks like it invites
retreat-and-return as a free reset, and it does not: caches, seals and lairs
are all one-shot per stack, recorded in `FrameMemory`. Climbing out to shed
Trace means returning to a stack with less left to take. The greed driver
and the existing spent-ness records make the reset self-limiting, which is
why no decay mechanic is needed.

### What it does

Four named bands — **Quiet / Noticed / Traced / Hunted** — shown in the
Stack HUD, each crossing logged as `MessageKind::Outcome` so it survives
`retain_outcomes_since_battle`.

Escalation reuses machinery that already exists: scale
`DUNGEON_ENCOUNTER_CHANCE`, then pack size through the existing
`depth_mult` path into `spawn_pack`, and at Hunted draw from the boss pool
the way `pick_lair_species` already does.

The readout and the threshold lines are not decoration. Escalating ambushes
with no visible cause are experienced as bad luck, not as consequence.
Without the HUD element this phase is a difficulty curve nobody can see.

No persistent Stack entities are introduced. There is no pursuing hunter.

### Tuning

Band thresholds, per-source Trace gain, and the per-band encounter and pack
multipliers are `pub const` in `tuning.rs`, in a labelled section, per the
difficulty-is-code rule.

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
  escalation actually changes the encounter roll and pack size. Seeded, no
  wall-clock, no unseeded RNG.
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

## Out of scope

- **Dark strata** — frames where the view cone shortens. Cut. It is the one
  addition that would need a walkable sight-blocker, inheriting the door trap
  in both cone consumers.
- **A pursuing hunter** — a Stack entity that occupies a cell and paths
  toward you. Cut in favour of escalating ambushes; no phase here introduces
  a persistent Stack entity, and adding one later is a real design decision,
  not an increment.
- **Trace decay over surface turns** — unnecessary given spent-ness makes the
  reset self-limiting.
- **Renaming zone travel.** "Breach" keeps its verb and is now unambiguous.

## Playtest note

None of this has been played. The Trace band thresholds and per-source gains
in particular are arithmetic-plausible and nothing more — whether the meter
asks an interesting question is not something the suite can answer. Phase 2
is the one to capture a `dev-saves/` template for and actually play before
building phases 3 and 4 on top of it.
