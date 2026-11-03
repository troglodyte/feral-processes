# Haste routine — design

**Date:** 2026-11-03

## Intent

The user has a companion that can't reach the fight on the battle map in
time to contribute. Haste is a player-built routine that fixes that: cast it
on a companion and it moves far enough to matter for the rest of the
encounter. At v3.0 it also swings twice. Making companions fast is the
point, so the large numbers are deliberate. This is not a balance problem to
tune down.

## What the player sees

- A new routine **Core, Haste**, in v1.0, v2.0 and v3.0, built at the
  Workbench like any other Core.
- **Target:** one ally (the player or a companion). An `Area: Group` part
  extends it to the whole party. No Range, Trickle, Damage type or Status
  part fits it.
- **Duration:** the rest of the encounter. It is not a buff-slot buff, so it
  stacks with Boost Attack, Boost Mitigation and Defend, and none of them
  overwrite it.
- **Movement on the battle map:** v1 adds **+4 cells per turn**, and v2 and
  v3 add **+8**. The bonus goes on *after* the normal
  `TACTICAL_MOVE_MIN..=TACTICAL_MOVE_MAX` clamp, so a fast body hasted at v2
  moves 16 cells.
- **Swings (v3 only):**
  - **Classic battle screen:** doubles the target's attacks per round. One
    becomes 2, and a level 8+ Striker's 2 becomes 4.
  - **Battle map:** the body still has the same number of actions. When an
    action is an ordinary swing, it swings **twice**. A routine stays a
    single invocation. (The user chose this over doubling actions.)
- **Recasting** on a body that is already hasted keeps the higher version.
  Versions never add together.
- A hasted body shows a **Hasted** marker:
  - **Classic:** a row among the active buff rows.
  - **Map:** a mark on the body, like the cloak wash.
- Parts reach the player the way every Core's do: the trader shelf and
  synthesised research nodes. No authored files.

## Non-goals

- Wild programs never run Haste, and no species or `assets/abilities/` file
  references it. Its parts can't be extracted off a wild program.
- No change to the AI's general behaviour beyond reading the larger
  movement allowance and the extra swing, which it gets for free through the
  shared functions.
- No field (out-of-battle) use.

## Decided how

These were checked against the source by a research pass on 2026-11-03.

- **State:** a battle-scoped ECS component `Hasted { version }` on the
  combatant, modelled on `Cloaked` (`components/effects.rs`). It is armed
  like `arm_cloak` (`game/combat_status.rs`) and cleared in
  `game/combat_teardown.rs` for the player, party and hostiles, beside
  `uncloak`. Battle components are in no save, so this needs no
  `SAVE_FORMAT_VERSION` bump.
- **Save-format safety:** crafted routines are saved as parts (bincode, no
  field names). So **`Core::Haste` is appended at the end of `Core`**, and
  the new `AbilityEffect` variant at the end of its enum, so that no
  existing discriminant moves. The builder must confirm that an existing
  save still loads (a save→load test with a pre-existing crafted routine).
- **Movement:** `Game::movement_allowance` (`tactical/reach.rs`) adds the
  bonus after `allowance(...)`'s clamp. The `Charging → 0` early return
  keeps priority. The per-version figures are named constants in
  `tuning/routine_craft.rs` (or `tuning/tactical.rs`). `walk_field`'s
  search radius grows to 16. That is fine, but the builder should check
  nothing else assumes `TACTICAL_MOVE_MAX` is an upper bound on a body's
  movement.
- **Classic swings:**
  - `Game::attacks_for` (`game/combat.rs`), the single resolver, doubles
    for a `Hasted` v3 body.
  - The swing loops in `game/combat_round.rs` and `game/combat_enemy.rs`
    are capped by `MAX_ATTACKS_PER_ROUND`. That cap must stop truncating a
    hasted body's doubled count.
  - `inspection.rs`'s damage-per-round figure picks the change up through
    `attacks_for`.
  - `balance_sim` is untouched. It models no routines.
- **Map swings:** `tactical_attack` (`tactical/turn.rs`) resolves a second
  ordinary swing against the same target when the actor is `Hasted` v3, and
  still spends one action.
  - The second swing is skipped if the first one downed the target. The
    builder decides between that and retargeting, but must not swing at a
    corpse.
  - The "one swing a turn here" comment changes with it.
- **Effect plumbing:** a new `AbilityEffect::Haste { version }` (or one
  carrying the movement bonus and a doubled-swings flag; the builder
  decides). It has to hit every exhaustive match: `field_only`,
  `affinity_category`, the "names nobody" set, a `*_mismatch` validator
  restricting the target to `OneAlly`/`WholeParty` (like `cloak_mismatch`),
  the `validate` fault array, the `use_ability` arm, `decompose.rs` (effect
  → Core and Version) and `describe.rs`.
- **Catalogue:** `Core::Haste` is versioned, `Facing::Ally`,
  `areas() = [Group]`, takes neither Range nor Trickle, is not a field
  routine, and its material group is the builder's call, next to Cloak's.
  Update `Core::ALL`, the `MATRIX` in `routine_craft/tests.rs`, and
  `tests/assets/recipes_power.rs`.
- **Figures:** a `CRAFT_HASTE` in `tuning/routine_craft.rs`. Its Power cost
  and cooldown sit near Boost Attack's, rising per version through the
  shared per-version constants. The exact numbers are the builder's call.

## Left to the builder

- The exact Power cost and cooldown.
- The material group.
- The marker's exact look on each screen.
- The wording of the description string.
- Whether the effect variant carries the version or the derived figures.

## Verification

- Unit tests:
  - Movement bonus per version, including past the cap and with `Charging`
    still at 0.
  - `attacks_for` doubling (1→2, Striker 2→4).
  - A map second swing that is skipped on a downed target.
  - Recasting keeps the higher version.
  - Teardown clears it.
  - Catalogue fit rules.
- A save→load test with an existing crafted routine, proving the enum
  append moved nothing.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
- A screenshot through the arena of a hasted companion on the map (see the
  `battle-screenshot-goes-through-the-arena` memory).
