# Staff mood at a glance — design

**Date:** 2026-10-29

## Intent

Base staff turn on each other at the end of a long slide in morale, and today
the first thing the player sees of that slide is the fight. The only place
mood shows is the staff report panel, a menu away. The player wants to see,
on the main base screen and without opening anything, which programs are
getting agitated, early enough to act before anyone downs tools or lashes
out.

**Statuses only.** The screen says *how bad*, never *why*. The cause stays in
the base menu's staff report, which the player opens when a status shows.

## Goals

- Every base staff program whose mood is off normal wears a mark on its map
  tile, graded by how bad it is.
- An always-visible HUD readout counts staff by mood step.
- The warning starts **before** behaviour changes — a "slipping" step above
  the sulking threshold.

## Non-goals

- Showing the cause (which machine, which memory, missing amenity).
- Any change to how morale, grievances or tantrums work, or to their
  thresholds.
- Reworking the staff report's mood words (`views::morale_band`).
- Party members out on the surface. This is about base staff.

## Anti-goals

- **Recolouring the program's glyph.** Glyph hue is identity
  (`hud::palette::glyph`, see `.claude/rules/seams-hud.md`); a mood tint
  would make one program read as two.
- **Steps that disagree with behaviour.** A program shown "ok" must never be
  sulking, and one shown "downed tools" must have actually stopped working.
- **Reusing `morale_band`'s scale.** Its words ("uneasy" at −10, "bitter" at
  −20) do not line up with the behaviour thresholds (−8 / −50 / −75); see the
  memory `morale-band-scales-do-not-line-up`.

## The steps

| Step | Shown when | What the program is doing |
|---|---|---|
| ok | otherwise | — |
| slipping | no `Disgruntled`, and `Game::morale` < `MORALE_SLIPPING_AT` | nothing different yet |
| sulking | `Disgruntled` at `Grievance::Sulking` | works, avoids machines it resents |
| downed tools | `Grievance::DownedTools` | stops taking postings |
| lashing out | `Grievance::LashingOut` | hits whoever is in reach |

- **Sulking and worse are read off the stored `Disgruntled` marker, not off
  the raw morale number.** The marker carries the hysteresis (in at −8, out
  at −6) and the ratchet, so reading it is what keeps the display in step
  with what the program is actually doing. Deriving these from morale would
  break the second anti-goal at the boundaries.
- **`MORALE_SLIPPING_AT = -4.0`** *(my call: halfway from neutral to
  sulking)*, a new constant in `tuning/staff.rs` beside the other `MORALE_*`
  constants. It must sit above `MORALE_RECOVERED_AT` (−6), so a test
  asserts `MORALE_SLIPPING_AT > MORALE_RECOVERED_AT > MORALE_SULKS_AT`.
- *(My call)* The slipping step has no hysteresis. It's display-only and
  morale moves smoothly, so the worst case is a mark blinking at the edge.
  The builder adds hysteresis only if that shows up in play.

## Where it shows

### On the map

A base staff program at any step other than ok wears a small mark on its own
tile.

- The mark sits in a tile corner or edge not already used by the con-read
  earmark, the staffed mark, the rarity bar or the progress bar. The
  builder picks the spot and records why in the doc comment, in the style of
  `render/marks.rs`.
- It's coloured by step from existing palette roles *(my call)*: slipping
  `WARN`, sulking `ATTENTION`, downed tools and lashing out `THREAT`, with
  downed tools and lashing out told apart by shape or weight. Shape is the
  builder's call, but it must read at map zoom without colour (two
  different glyphs, or a pip that gets bigger).
- It lives alongside `rising_mark` (the floating `+`/`^`) and doesn't replace
  it. A body can be sulking and being serviced at once.
- The step reaches the renderer as a new field on `EntityView`, derived in
  the engine. The renderer never computes it.

### On the HUD

- **The CREW pane's collapsed bar** shows the non-ok counts when any staff
  member is off ok, worst step first. For example: `1 lashing · 2 sulking ·
  1 slipping`. When all staff are ok it keeps today's summary unchanged.
  It must fit the bar. Text never clips or wraps (memory
  `gui-text-never-clips-or-wraps`), so the builder abbreviates or drops
  the mildest steps to fit, counting what was dropped rather than hiding it.
- **The open CREW pane** gets one mood row carrying the same counts.
- *(My call)* **Downed tools and lashing out also go on `Game::attention`**,
  as a new `AttentionKind`. Those two need the player now, and attention is
  the one derivation of "needs the player" that three surfaces already read.
  As the HUD rules already work, an attention row outranks the collapsed
  bar's calm summary, so the bar shows the alarm rather than the counts at
  those steps. Slipping and sulking do **not** raise attention, so they
  don't nag.
- *(My call)* The attention row is **aggregated, one row per step**
  ("2 downed tools", "1 lashing out"), never one per program, so it can't
  crowd out other rows. It maps to `InfoTab::Crew` in
  `hud/column.rs`'s exhaustive match. Its key opens the base menu's staff
  report if an existing attention kind already opens it. Otherwise it uses
  whatever an existing staff-related kind uses.
- The HUD counts show whether the player is in the base or on the surface.
  The staff still exist and can still slide.

### Who counts

*(My call)* "Base staff" is exactly `Game::base_staff()`. That includes
`Downed` and off-shift bodies, wherever they stand. Counts cover all of
them, not just the 12 the CREW pane has room to list. The map mark appears on
any of them that is drawn. A staff body with no memories reads morale 0, so
it's ok. That's correct, not a gap.

**Cost:** call `Game::morale` per staff body when building the view, with
no caching. Per CLAUDE.md, no optimisation without evidence.

## Constraints

- The engine derives the step (one pure function from `Option<Grievance>` and
  the morale sum to the step, unit-tested at each boundary). app-core and
  gui only read it. `Game.world` stays private.
- The engine provides one call for the per-step counts. The CREW summary,
  the CREW row and any test all read that same call, never a second fold
  over `EntityView`s.
- No save-format change: everything is derived from state that is already
  saved.
- No asset or schema change, so no ADR.
- `render/` draws only through `Painter` (drawing seam).
- No player-facing tick vocabulary in any label.

## Left to the builder

- The step enum's name and the exact player-facing words, as long as they
  keep the meanings in the table and stay plain.
- The mark's position, glyph and size within the constraints above.
- The abbreviation scheme for the collapsed bar.
- Where the counts call lives in `views/`, and the `AttentionKind` row's wording.

## Verifying

- Engine unit tests for the step function at each boundary, including a
  program that is `Disgruntled` at −7 (still sulking, because of the
  hysteresis) and one that isn't, at −7 (slipping).
- A test that the counts call agrees with the steps on `EntityView`.
- `panes` census: the collapsed summary fits at the narrowest supported
  width with all five steps present.
- A screenshot from a dev-save with disgruntled staff (or a capture of one),
  read before calling it done.
