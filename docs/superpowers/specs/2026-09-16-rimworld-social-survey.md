# RimWorld social behaviours: what is worth importing

**Status:** survey, not a design. The companion to
`2026-09-16-dwarf-fortress-social-survey.md`; what was selected out of both is
`2026-09-16-base-social-roadmap.md`.

Written 2026-09-16 from knowledge of RimWorld 1.5 and its expansions, not
checked against a source. Where play experience disagrees, play wins.

## What RimWorld's social sim actually is

1. **Three kinds of thought.**
   - *Memory* thoughts are stored, last a fixed duration, and mostly hold
     full strength until they expire. Repeats stack to `stackLimit`, each
     extra stack worth less (`stackedEffectMultiplier`, about 0.75).
   - *Situational* thoughts are never stored: re-evaluated from world state
     ("in darkness", "sharing a bedroom", "rival nearby") and gone when the
     condition is.
   - *Social* thoughts target one pawn and carry **two** numbers, an opinion
     offset and a separate mood offset. Most move opinion only.
2. **Mood and opinion are separate folds.** Mood is memory plus situational
   thoughts, and the displayed value drifts toward it. Opinion is a per-pair,
   asymmetric −100..+100: social thoughts plus a derived term from the other
   pawn's traits ("annoying voice").
3. **Interactions drive opinion.** Nearby awake pawns start one on a timer —
   chitchat, deep talk, slight, insult, kind words, romance — weighted by
   traits and current opinion. Each writes a social thought, draws a speech
   bubble and appends a generated line to a per-pawn social log. An insult
   can escalate into a social fight.
4. **Friend and rival are thresholds** (±20), and the label changes other
   thoughts: "my friend died" hurts, "my rival died" pleases.
5. **Major events are colony-wide and instant.** No gossip: everyone knows a
   colonist died, scaled by relationship and traits. Witnessing is a
   separate, stronger thought.
6. **Breakdowns.** Mood thresholds, shifted by traits, trigger breaks
   including an *insulting spree*; a break is followed by Catharsis. High
   mood triggers Inspirations.

## Against this game

| RimWorld | Here |
|---|---|
| Memory thought | `MemoryDef`; richer (a real decay), but strikes compound linearly |
| Situational thought | Absent — and derived-never-stored is this repo's own idiom |
| Separate opinion and mood offsets | Absent: `Game::morale` folds every memory, program-subject grudges included |
| Interactions | Absent; `idled_with` is the only social edge |
| Friend/rival thresholds | The band in the rumours design |
| Colony-wide events | Absent; the alternative to proximity gossip |
| Social fight | `tantrum.rs`, triggered by grievance rather than by an insult |
| Insulting spree | Absent |
| Catharsis | `vented.ron` |
| Trait-derived opinion | Absent; close to DF's preferences |
| "My friend died" | Absent |

## The two structural differences from DF

- **RimWorld keeps opinion out of mood.** DF, and this game today, let a feud
  drive both parties toward breaking. Resolved as a per-def `mood:` dial
  defaulting to today's behaviour.
- **RimWorld reaches everyone instantly; DF spreads by proximity.** Resolved
  as both: big events broadcast to witnesses, and routine spread rides a
  gossip interaction.

## Left in RimWorld deliberately

- **Romance, kinship and breeding.** Programs have no such relations.
- **Inspirations.** A high-mood reward pays out in production or combat, and
  would need its own balance argument.
- **Ideology's precepts and roles.** DF's values problem again: unreadable in
  a 38.5-cell column.
- **The social skill.** A progression axis not earned by fighting.
