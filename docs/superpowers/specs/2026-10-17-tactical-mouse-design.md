# Battle maps: the mouse walks, aims and picks a target

**Date:** 2026-10-17
**Status:** approved in chat; see `INDEX.md` for whether it shipped.

## Intent

A fight on a battle map is played one arrow key per cell.
Crossing the board to reach a hostile is six or seven presses, each one a
turn beat the player has to count against the blue wash, and aiming a
routine is the same walk again with a cursor.
The surface map already answers a click with a travel, so the hand is on
the mouse until the fight opens and then has to leave it.

After this change the mouse does on the board what it already does on the
surface map: a click on a reachable cell walks the acting body there, a
click on a hostile picks it and a second click swings, and while a routine
or swing is being aimed the cursor rides under the pointer and a click is
Enter.
Every key keeps working exactly as it does today.

## Goals

- A primary click on a reachable cell walks the acting body there along
  the engine's own shortest path, paced one cell a beat like a hostile's
  walk, and spending steps through the same door an arrow key spends them.
- Hovering a reachable cell draws the route that click would take, so a
  crossing through a red cell is visible before it is committed.
- A primary click on a hostile marks it; a second click on the marked
  hostile swings at it.
  A click anywhere else clears the mark.
- In `Mode::TacticalAim`, the cell cursor follows the pointer over the
  board and a primary click commits the aim where the cursor is.
- All of the above live in app-core as rules over a cell.
  The renderer resolves a pixel to a cell and nothing more.

## Non-goals

- Mouse wheel zoom on the board, dragging the camera, or any gesture on
  the turn strip, the wash legend or the side panels.
- Clicks on the Stack corridor, the base grid, or the intrusion screen.
- A click on a hostile that walks up to it and swings in one gesture.
  The player asked for the mark-then-swing form; a walk stays a separate
  click on a cell.
- Hover labels on the board.
  `App::hover_lines` reads the surface map and refuses here; that stays.
- Touch, double-click, or any button but primary and secondary.

## Anti-goals

- A click that acts for a body that is not the player's.
  The wild side's turn ignores the pointer as it ignores the keyboard.
- A second spelling of a step, a swing or an aim.
  Every pointer gesture ends in a call the keyboard already makes:
  `Game::tactical_step` for a cell of the walk, `Game::tactical_attack` for
  the swing, `App::commit_tactical_aim` for the aim.
- A renderer that knows what a click means.
  `crates/gui` resolves pixel to cell through the layout the draw pass
  published and hands the cell to `App`; it never reads the view to decide.
- A walk the player cannot stop.
  A key or a click during the walk ends it, and that press is swallowed,
  the rule auto-attack already has.

## Constraints

- `Game.world` stays private; nothing here needs an accessor.
- The drawing seam holds: the path highlight and the target outline are
  drawn through `Painter` in `render/tactical.rs`.
- The seams in `.claude/rules/seams-tactical.md` that this touches, each
  kept rather than worked around:
  a hostile's walk is a run of real `tactical_step`s one cell a beat, and
  the player's walk is the same run through the same door;
  `TacticalBattle::walk` is an `Option` and `Some(vec![])` is "has
  arrived";
  the pacing carry is in seconds and the step rate is derived off
  `Game::tactical_walking`;
  walking into a hostile is a swing gated on `Hostile` and
  `tactical_awaits_input`;
  the camera is held on the body that acted;
  a refusal is one sentence on two surfaces through `App::refuse`.
- No save-format change.
  Nothing new is serialised: the target mark and the hover path are
  frame-to-frame app-core state and die with the fight.
- The help page `assets/help/28-battle-maps.md` gains the mouse gestures
  under "On your turn".
  `CHANGELOG.md` gets its line at landing.

## Behaviour, as decided

### Walking

On a party body's turn in `Mode::TacticalBattle`, a primary click on a
cell in `TacticalView::reachable` commits a walk to it.
App-core cannot plan the path itself: `Game.world` is private and the view
carries cells without costs, so the engine gets one planning door,
`Game::tactical_walk_to(cell) -> bool`, which builds the acting body's
movement field, takes `reach::path_to` over it (the path a hostile would
plan) and commits it through `TacticalBattle::commit_walk`; it answers
`false` and commits nothing for a cell the field never reached.
A second door, `Game::tactical_path_to(cell) -> Vec<(i32, i32)>`, answers
the same path without committing it, for the hover preview below.
The pacing loop then spends it one cell a beat at `TACTICAL_STEPS_PER_SECOND`,
each cell a real `Game::tactical_step`, so a provoking cell draws its
reaction, a body put down on the way stops where it fell, and a step the
board refuses drops the rest of the walk.
Arriving spends no action: the body stands on the cell with whatever steps
and its action still in hand, exactly as if the keys had walked it.

A click on a cell that is not reachable refuses with "Out of reach." through
`App::refuse`.
A click on the acting body's own cell does nothing.

While the walk is live, any key press or any click stops it and is
swallowed, in the manner of `handle_tactical_key`'s auto-attack stop: the
press does nothing else.
Stopping is a public `Game` door over `TacticalBattle::clear_walk`, which
is engine-private today.
The body keeps its place in the walk's last completed cell.
A cell the board refuses mid-walk (a body stepped into the path, a
reaction's refusal) ends the walk the same way; `StepOutcome::Struck`
cannot occur on a planned path, since a body is a wall in the field.

Hovering a reachable cell draws the path the click would take as a wash
over those cells, distinct from the four existing washes and named in the
legend.
The hover path is app-core state (`App::tactical_hover_path`, or the
pointer's cell from which the renderer asks for it; builder's choice), set
by the pointer handler each frame and cleared when the pointer leaves the
board or the turn passes.

### Picking a target

A primary click on a cell holding a hostile body marks that body as the
target: `App::tactical_target: Option<Entity>`.
The marked body is drawn with an outline in the emphasis colour, the same
weight as the acting-body outline.

A primary click on the marked hostile calls `Game::tactical_attack`.
When it refuses, the status line says "Not from here.", the sentence the
aim cursor's commit already uses, through `App::refuse`.
A landed swing clears the mark.

The mark is cleared by a primary click anywhere that is not the marked
body, by a secondary click, by `Esc`, by the turn passing to another body,
and by the fight ending.
A click that clears the mark also does what it would have done without
one: a click on a reachable cell both clears the mark and walks there.

A cloaked hostile is picked by the same rule, since `tactical_attack` is
the door that refuses it; the mark is not a second cloak check.
A decoy (`TacticalView::decoys`) has no entity and so cannot be marked: a
click on its cell is a click on a cell, and the cell rule above answers it.

A refused swing keeps the mark, so the player can walk and click it again.
The mark is read against the live view whenever it is used or drawn, and
dropped if its body is no longer on the board, so a hostile that died or
departed under a keyboard action never reaches `tactical_attack` and needs
no clearing event of its own.

### Aiming

In `Mode::TacticalAim`, the cell cursor moves to the pointer's cell when
the pointer **enters a different board cell**, not every frame: a pointer
parked over the board must not snap the cursor back after every arrow key.
The pointer handler reports the hovered cell; app-core keeps the last cell
it reported and moves the cursor only on a change.
When the pointer is off the board the cursor stays where it was.
A primary click calls `App::commit_tactical_aim` with the cursor's cell,
which is `Enter`.
A secondary click is `Esc`.
Everything the cursor already drives, the yellow aim wash, the white hits
preview and the teleport's two-cell re-open, follows it as it does from the
keys; the only new code is the cursor move itself.

`Mode::TacticalRoutine`, `Mode::TacticalItem` and `Mode::TacticalEmulate`
are lists, not boards, and the results popup is a frozen board; none of
them takes pointer input in this change.

### Turns that are not the player's

`Game::tactical_awaits_input` false, or `App::tactical_auto` set, means the
pointer handler does nothing on the board, the gate `handle_tactical_key`
already applies.
A click during auto-attack stops it, as a key does.

## How it gets made

### The board publishes its layout

`draw_tactical_map` computes the same camera centre, half-extent, sub-tile
offset and tile size that `draw_surface_map` stashes in
`Fx::set_map_click`.
It publishes a `MapClickLayout` the same way, so `render::tile_at_px`
inverts a pixel on the board to a board cell.
The clear at the top of the three-way branch in `render/base.rs` stays;
the tactical branch overwrites it as the surface branch does.

Publishing a layout means `handle_map_pointer` would start asking for a
hover label on the board.
`App::hover_lines` already answers empty there, and the label is not drawn
for an empty answer; the plan carries a test that pins this rather than
adding a second gate.

### One pointer handler, dispatched by mode

`lib.rs::handle_map_pointer` already finds the tile under a primary click
and the tile under a resting pointer.
It is split so that finding the tile stays where it is and what the tile
means moves into app-core: the handler reports the hovered cell every
frame and a click on release, and `App` decides what each means from
`self.mode`.
The surface map's `App::travel_to` becomes one arm of that decision.
The builder runs the design-patterns dialog on the exact shape; the
decision that is fixed here is that `crates/gui` has one map pointer path
and no `match app.mode` of its own.

The secondary button is read for the two gestures above (clear the mark,
cancel the aim) and nothing else.

### The player's walk is paced by the existing loop

`App::advance_tactical` returns early on the player's turn when auto-attack
is off.
It must instead keep spending beats while `Game::tactical_walking` is true,
through a party-side door that takes one cell off the committed walk with
`Game::tactical_step` and ends the walk on anything but `StepOutcome::Moved`.
The engine already has the walk, the step door and the step rate; what is
new is a beat the party side can owe.
The builder decides whether that door is a new `Game` method beside
`tactical_ai_beat` or a mode of it; what is fixed is that it calls
`tactical_step` per cell and never writes a position.

### Drawing

Two additions to `render/tactical.rs`, both through `Painter`:
a wash for the hover path, added to `Wash` and `board_washes` with a legend
entry, and an outline on the marked target drawn where the acting-body
outline is drawn.
Both follow the camera through `tile_origin_px` and are bounds-checked
against the pane, the rule the turn arrow and aim cursor already follow.

## Left to the builder

- The exact shape of the app-core pointer entry point and how the surface
  map's travel joins it.
- Whether the hover path is stored in `App` or derived per frame from the
  pointer's cell.
- The name and colour of the path wash, and whether it reuses `palette::PLAN`
  at a stronger alpha or takes a new entry.
- Where the party-side walk beat lives in the engine, and whether the
  stop door is a new method or an arm of an existing one.
- The tests: a cell click walks and spends steps; a click out of reach
  refuses with the sentence; a key mid-walk stops it and is swallowed;
  mark, swing, clear by each of the five routes; hover moves the cursor
  and click commits in aim mode; a click on the wild side's turn does
  nothing; and a gui test that the published tactical layout inverts a
  pixel to the cell that was drawn there.
