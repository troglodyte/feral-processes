# Tactical reactions: opportunity attacks

**Status:** approved design, not started. Todo #104, project A.
**Scope:** tactical board only. Engine (`tactical/`, `game/`) and gui
(`render/tactical.rs`). No save change — `TacticalBattle` is never saved.

## Why

On a battle map, a body next to an enemy can walk away or run a routine at
no cost. The only zone of control is "a body is a wall"
(`reach::movement_field`). Reactions make standing next to an enemy
matter, and they are the foundation for two later projects: overwatch
(B) and a riposte passive (C).

## Rules

1. **Budget.** Every body on the board, party, hostile and summon alike,
   has one reaction per round. It is refunded at the start of that body's
   own turn, inside `Game::hand_on_turn`. The spent set lives on
   `TacticalBattle`.
2. **Reach is melee.** A body can react only against a body within
   `TACTICAL_MELEE_RANGE` of it, whatever its `swing_range`. A ranged
   reaction would be overwatch, which is project B.
3. **Two triggers.**
   - **Leaving reach.** A `tactical_step` that starts inside a hostile's
     melee reach and ends outside it provokes that hostile. A step that
     stays adjacent does not.
   - **Invoking.** `run_tactical_routine` provokes every hostile adjacent
     to the invoker. `AbilityEffect::Decompile` is exempt.
4. **One door: `Game::provoke(mover, reactors)`.**
   - **Who reacts:** each reactor must be an enemy of the mover, have its
     reaction unspent, and have line of sight to the mover.
   - **Order:** the reactors act in initiative order.
   - **The swing:** each takes one basic swing through
     `resolve_and_apply_attack`, with **fumbles disabled** (the Opening
     riposte's rule, `battle.rs:270`). This stops reactions triggering
     ripostes that trigger more reactions.
   - **Stopping:** the loop ends as soon as the mover is dead or stunned
     (`is_stunned`).
   - **Cloak:** a cloaked mover provokes nobody, because the cloak rule
     filters every door that names a body. A reactor that swings loses its
     own cloak through the existing `break_cloak` in
     `resolve_and_apply_attack`.
5. **Timing and interrupts.**
   - **Step:** `provoke` runs before the move is written. If the mover
     ends up dead or stunned, the step does not happen and the rest of a
     hostile's planned walk is dropped.
   - **Routine:** the order is:
     1. Charge (cooldown and Power, as today).
     2. `provoke`.
     3. If the invoker is dead or stunned, the routine **fizzles** and logs
        "<name>'s <routine> is cut off." Nothing is refunded. It still
        reports success (`Ok`) rather than a refusal, following the rest
        interrupt (`game/turn.rs:1192`).
     4. Otherwise, the effect resolves.
     5. Reap and `hand_on_turn`, as today. These already handle a dead
        actor.
   - Every refusal still happens before the charge. The fizzle is not a
     refusal.
6. **Feedback.** A log line for each reaction, plus a `BoltCue` from the
   reactor to the mover, the same travel rule a swing uses.

## AI

- `Game::provocation(mover, from, to)` returns the reactors a step or an
  invocation would provoke, with the expected damage. The expected
  damage is a **call** to `battle::expected_damage` over the real
  profiles. The same function feeds the telegraph and the tests.
- **Walks:** hostile walk planning subtracts the expected reaction damage
  from a cell's merit, summed over the walk's steps. The "stay put unless
  a cell beats your own" rule is unchanged. The cost only makes a
  provoking cell worth less.
- **Routines:** a hostile adjacent to an enemy discounts a routine's
  value by its expected reaction damage and by the probability that the
  routine fizzles.
- Party auto-attack (`tactical_auto_beat`) uses the same planner, so it
  inherits both terms.
- At temperature zero this still spends no RNG draw. The new terms are
  expectations, not rolls.

## Telegraph (gui)

- The movement overlay tints each destination whose path provokes, using
  `provocation`.
- While the acting body is adjacent to a hostile, each routine row reads
  "provokes N". Decompile rows never do.

## Tuning

No damage multiplier on reaction swings to begin with. Before and after,
run the `dev-arenas/` tactical scenarios, and record the numbers in
`docs/measurements/`. `balance_sim` can't see this: it has no board.

## Tests (engine)

- A reaction is spent by a swing and refunded at the reactor's next turn,
  not before.
- Stepping out of reach provokes. A step that stays adjacent does not.
- A cloaked mover provokes nobody.
- A reaction swing never fumbles. Sweep seeds and assert that no
  `Fumble` comes back.
- A fatal or stunning reaction stops the walk, and the mover's cell is
  unchanged.
- Invoking next to a hostile provokes. Decompile does not.
- A fizzled routine has spent its Power and cooldown and applied no
  effect.
- With two otherwise-equal walks, the AI picks the one that provokes
  nothing.
- `provocation`'s prediction matches what `tactical_step` actually does.

Every test must fail with its fix removed.

## Not in scope

Overwatch (B), riposte passive (C), shove and guard (E, F), and the
non-tactical group-battle model.
