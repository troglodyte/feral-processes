# Charge routines

**Date:** 2026-10-08

## Goal

Routines that wind up a large hit over several rounds, melee or ranged. The
charger can release early for a smaller hit. While charging, the charger cannot
move and spends its whole turn on the charge. A visible indicator shows who is
charging, how far along they are, and on the battle map, where the hit will land.

A hostile's wind-up is meant to be read and answered: step out of the aim, stun
the charger, or kill it first.

## What the player said (decisions)

- **Both combat models.** Charges work in group battles and on the tactical
  battle map. The root only matters on the map, because group battles have no
  movement.
- **Everyone can use them.** That covers the player (learned through research
  and installed like any other routine), companions and hostiles, the last two
  granted by their species files.
- **A charging round costs the whole turn.** While charging, the only choices are
  HOLD or RELEASE. There is no moving and no other routines.
- **The aim locks at the start, and a stun breaks the charge.** On the battle map
  the target cells are fixed when charging begins, so a victim can step out of
  them. A stun cancels the charge.

## Data

`AbilityDef` gains:

```rust
#[serde(default)]
pub charge: Option<ChargeSpec>,

pub struct ChargeSpec {
    /// Turns of charging that reach full power. At least 2.
    pub rounds: u32,
}
```

- `charge` is valid only on a `Damage` effect. A charge on any other effect, or
  `rounds < 2`, fails validation, and the file is skipped with a logged warning
  using the `*Db::load_dir` pattern.
- The authored `power` is the **full-charge** hit. A charged routine carries
  no other new fields. Shape, range, `ranged`, `status`, `accuracy`, `fx`,
  `cooldown` and `power_cost` all mean what they already mean.
- `assets/abilities/README.md` documents the field in the same change.
- A charge routine is not a passive, so it is priced in Power like any other
  routine.

## Timing

Let N be `charge.rounds` and k the number of turns spent charging so far.

1. **Start (k = 1).** The charger uses the routine. It passes through
   `ability_unavailable` and pays through `spend_power`, which are the existing
   gate and charge. The aim is locked, a `Charging` component is attached, and
   the charger's turn ends.
2. **Each later turn of the charger:**
   - If k = N, the routine fires automatically at full power before anything
     else happens that turn, and the turn ends.
   - Otherwise the charger chooses **HOLD** (k += 1, and the turn ends) or
     **RELEASE** (the routine fires at power × k/N, and the turn ends).
3. A charge therefore always lands on a later turn than the one it started on.
   That guarantees every target at least one turn to react.
4. **Firing** uses the ordinary path. The hit roll is `battle::resolve_attack`
   and damage goes through `Game::apply_damage`, using the scaled power. The
   effect's status rider rolls as authored and is not scaled.
5. **The cooldown** starts when the charge fires or is cancelled. It never
   starts at the beginning of a charge.

## State

`Charging { ability: AbilityId, rounds: u32, aim: ChargeAim }`, a component
that exists only for the length of the fight.

- `ChargeAim::Group(target)` in group battles. This is the target group locked
  at the start. If that group is gone at release, the routine retargets through
  the ordinary target choice for its `target` kind.
- `ChargeAim::Cells(Vec<(i32, i32)>)` on the battle map. These are the absolute
  board cells covered by the routine's shape when the charge starts, read
  through `routine_tactical_*`. At release, whoever stands in those cells is hit.
  There is no recheck of range, since the charger has not moved.
- **Cancelled by:**
  - a stun landing on the charger, checked in the same places that already test
    `is_stunned`;
  - the charger dying or leaving the fight;
  - the charger fleeing;
  - the fight ending.

  A cancelled charge keeps its Power spent and starts its cooldown.
- `combat_teardown` clears `Charging` along with the other state that lasts
  only for the fight.
- **Sieges.** A siege save does not keep statuses, buffs or cooldowns, and
  `Charging` follows that precedent: it is dropped across a save and load, and
  the Power stays spent. **There is no save-format change.**

## Root (battle map)

- `movement_allowance` returns 0 for a body that has `Charging`. This binds
  `tactical_step` and the AI's `movement_field`/`cell_merit` through the one
  allowance, so the AI never plans a walk for a charger.
- A charger's turn hands on exactly once, from the HOLD/RELEASE/auto-fire path.
  The AI turn must not hand on a second time.
- If a charger is displaced by someone else's routine (Teleport, Jump), its aim
  cells stay where they are. The charge is not cancelled.

## AI (hostiles and companions)

A charge routine is chosen through the existing routine choice, like any other
damage routine. On each later turn the charger:

- releases if a target in the aim would be killed at the current k/N power;
- cancels if no hostile-to-it body is left in the aim cells or the aimed group;
- otherwise holds until full.

A cancel by the AI counts as a cancel: Power stays spent and the cooldown
starts. The charger may act normally from its next turn.

## Player controls

While the player's lead is charging, the battle UI offers only **HOLD** and
**RELEASE**, as uppercase actions (see the row-selector rule), plus fleeing,
which cancels the charge. A charging companion acts on its own through the AI.

## Visual indicator

All drawing goes through `Painter`, per the drawing seam.

- **The charger's body** shows a `CHG k/N` tag beside its status tags and a
  pulsing outline. `TacticalBody` gains `charge: Option<(u32, u32)>`; the GUI
  reads only views.
- **Aim cells** get a danger wash on the battle map, with a legend entry. Aims
  of the player's side and of hostiles use distinct tints. The cells reach the
  GUI through the tactical view, never through `Charging` directly.
- **Group battles.** The charger's row reads `charging k/N → <group>`, where
  `<group>` is the aimed group's label.
- **Release** plays the routine's `fx` if it names one.

## Content

- **Overclock Strike.** Melee, `OneEnemyGroupFront`, Single shape, `rounds: 3`.
- **Long Compile.** Ranged, Line shape, `rounds: 4`.
- One hostile species gains one of the two, so the wind-up appears in ordinary
  fights.

Both join the research tree under the existing rules. The arena decides their
`power`, `power_cost` and `cooldown`. `balance_sim` models no routines, so it
is not the gate.

## Testing

Engine unit tests:

- Releasing at k fires at k/N of full power, and the status rider is unscaled.
- At k = N the routine fires automatically on the charger's next turn.
- A charge never lands on the turn it started.
- Battle map: a charger cannot step, and the AI charger stays on its cell.
- Battle map: a victim who steps out of the aim cells is not hit, and one who
  steps in is.
- A stun on the charger cancels the charge, Power stays spent, and the cooldown
  starts.
- The charger's death mid-charge does not break the initiative cursor.
- Group battle: the aimed group is gone at release, and the hit retargets.
- AI: releases early on a kill, cancels on an empty aim, and otherwise holds.
- Teardown clears `Charging`.
- Asset census: shipped charge routines are `Damage` with `rounds >= 2`, and a
  malformed `charge` is skipped with a warning.

Also:

- A dev arena, `dev-arenas/charge-routines.ron`, pits a player with each
  routine against the hostile that carries one.
- A screenshot of the battle map mid-charge, through the arena (per the
  battle-screenshot rule), to check the tag and the wash.

## Out of scope

- Scaling accuracy, range or area with the charge.
- Charges on non-damage effects (heals, buffs).
- Keeping a charge across a siege save.
- Interrupts other than a stun, such as damage taken.
