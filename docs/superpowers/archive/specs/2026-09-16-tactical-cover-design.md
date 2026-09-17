# Tactical cover

**Status:** approved design, not started. Todo #104, and it replaces the
separate flanking project (D).
**Scope:** tactical board only. Engine and gui. No save change and no new
`BattleCell` kind.

## Why

`BattleCell::Cover` blocks sight completely or does nothing. Either it
lies on the line between two bodies and no attack is possible, or it
doesn't and gives no protection: `line_of_sight` excludes the endpoints,
so standing next to a boulder does nothing. Partial cover, where you can
be seen but are harder to hit, is what makes positioning a decision. The
same rule also gives flanking for free: walk around the boulder and the
protection is gone.

## Rule

`reach::cover_between(board, attacker_cell, defender_cell) -> bool` is
the only function that decides cover, and it is pure. It returns true
when all three hold:

1. The attacker and defender are more than `TACTICAL_MELEE_RANGE` apart.
   Cover never protects in melee.
2. A `Cover` cell is one of the defender's eight neighbours, and it lies
   on the attacker's side: the vector from the defender to the cover and
   the vector from the defender to the attacker have a positive dot
   product. That is within 90°, so a boulder behind you does not count.
3. The attacker can see the defender. If it can't, there is no attack to
   modify.

The board keeps its four cell kinds, one for each combination of
crossable and see-through. There is one level of cover.

## Effect

The defender's evasion is raised by `tuning::COVER_EVASION_PERCENT`. This
has the same shape as the Exposed rung, which lowers evasion by a
percentage, so the hit chance still comes from `hit_chance` and no roll is
added.

**Where it enters.** Cover depends on both bodies, and
`combatant_profile` sees only one. So it goes where the two profiles
meet: a new `Game::defender_profile_against(attacker, defender, swing)`,
called by `resolve_and_apply_attack` and by every forecast (the reaction
`provocation` from project A, and the AI's scoring). This is a call, not a
copy. It does nothing when there is no `TacticalBattle` or either body is
off the board, so the group model is unchanged.

**Which attacks it applies to.** `battle::Swing` gains a flag saying
whether cover applies. It is set from the routine's
`AbilityDef::tactical_shape()` in `use_ability`, and the default is that
cover applies.

| Attack | Cover |
|---|---|
| Basic swings, reactions, `Single` routines | applies |
| `Line`, `Cone` | applies. The boulder still does not stop them any more than it does today; the defender is only harder to hit. |
| `Radius` | ignored. A blast flushes bodies out of cover. |
| Heals, buffs, anything that does not roll a hit | unaffected by construction |

## AI

`cell_merit` gains a cover term **for ranged bodies only**: those whose
band does not need to close in. Its value is the share of visible enemies
the cell has cover against. Melee bodies keep cover as a tie-breaker, as
crowding is today, so they don't stall behind boulders. The "stay put
unless a cell beats your own" rule is unchanged.

## Telegraph (gui)

- **Shield mark.** Drawn on a body that has cover against the hostile
  currently selected or aimed at. During a hostile's turn, it is drawn
  against the acting hostile. It takes a free tile corner and a palette
  role, following the corner-mark rules in CLAUDE.md's HUD section.
- **Movement overlay.** Marks destinations that have cover against at
  least one visible hostile.
- Both come from one engine query built on `cover_between`, so the screen
  and the roll can't disagree.

## Board generation

Measure first. Sample generated boards per biome and record how often a
walkable cell has a `Cover` neighbour. If the share is too low for cover
to matter, raising the density is a follow-up with its own measurement,
written to `docs/measurements/`.

## Tuning

Set `COVER_EVASION_PERCENT` so that a typical same-level swing loses a
noticeable share of its hit chance: start around the Exposed magnitude
and adjust with arena runs before and after.

## Tests (engine)

- `cover_between`: a boulder in the arc gives cover, a boulder behind the
  defender does not, melee range ignores cover, and a diagonal at exactly
  90° does not count (the dot product is zero).
- The evasion raise reaches the roll through `resolve_and_apply_attack`.
  Compare hit rates over seeded sweeps, or assert on the profile directly.
- `Radius` ignores cover. `Line`, `Cone` and `Single` are penalised.
- Moving the attacker around the boulder removes cover.
- The group model is unchanged: no `TacticalBattle`, no change.
- A ranged hostile moves to a covered cell over an equal uncovered one. A
  melee hostile does not leave a closing cell for cover.
- The telegraph query agrees with `defender_profile_against`.

## Not in scope

Destructible cover, low cover as a fifth cell kind, elevation, and a
flanking bonus beyond losing cover.

## Interactions

- **Project A (reactions):** reactions are melee, so cover never affects
  them. Rule 1 holds that by construction.
- **Squads (`2026-09-16-tactical-squads-design.md`):** a multi-cell body
  needs a rule for which of its cells counts. Take that up in whichever
  of the two lands second.
