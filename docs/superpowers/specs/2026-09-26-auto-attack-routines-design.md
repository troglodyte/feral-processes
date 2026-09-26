# Auto-attack invokes routines, and drinks a Power cell to afford one

**Status:** designed 2026-09-26, not implemented. Todo #103.

## Intent

On a battle map, auto-attack (`[A]`) and auto-resolve (`[R]`) hand the
party's turns to the planner. Today that planner only ever swings for a party
body, so a player who wants their routines used has to play every turn by
hand. After this change a party body on auto invokes its routines the way a
hostile does, and when the routine it wants is refused *only* for lack of
Power it spends its turn on a Power cell from the pack.

Success: with `[A]` armed, the player and companions run ready, affordable
routines; a body blocked solely by Power and holding a plain Power cell drinks
one; nothing else about auto-attack changes, and the arena's headless numbers
do not move.

## Decisions (agreed in chat)

| Question | Decision |
|---|---|
| Who invokes on auto | Player **and** companions, each paying its own Power |
| How a routine is chosen | The hostile planner (`tactical_intent`), plus the player's gate |
| Manual item use on the map | Yes — a new `[U]` action, the door auto also uses |
| When a cell is spent | Only when Power is the chosen routine's sole refusal |
| `[R]` auto-resolve | Behaves as `[A]` does |
| Headless arena rep | Stays swing-only (`PartyPlan::AllAttack` parity) |

Scope is the battle map only. The group model's `[A]ll attack` and its
auto-resolve branch are unchanged.

## Design

### 1. Choosing: one planner, one extra gate

`tactical_intent` (`tactical/ai.rs`) gains a party arm, reached only when the
beat has opted in. A party body's candidate is the first routine that passes
`wild_routine_ready`'s filter **and** `Game::ability_unavailable`. The second
gate is the load-bearing half: `run_tactical_routine` charges Power through a
door that never asks `ability_unavailable` (which is why auto was swing-only),
so the planner must ask it before choosing. No candidate → `Intent::Swing`,
exactly as today. Aiming is unchanged: `best_aim` already handles a helpful
routine via `Intent::helpful` and `acts_for_hostiles`.

`wild_routine_ready` returns the *first* uncooled routine, not the first
affordable one, so the party arm cannot simply call it and gate the result —
factor its filter into a candidate iterator both arms walk, with the party arm
adding the `ability_unavailable` test.

The opt-in is a parameter (a two-variant enum, say `PartyTurns::Invoke` /
`SwingOnly`) on the beat entry points:

- `tactical_auto_beat` — `Invoke` (`[A]`)
- `tactical_drive_turn` — takes the parameter; `auto_resolve_battle_with`
  passes `Invoke` (`[R]`), `arena/run.rs` passes `SwingOnly`
- `tactical_ai_beat` — hostiles only, unaffected

The seam line "`tactical_auto_beat` lifts `tactical_ai_beat`'s `Hostile` gate
and changes nothing else" and the doc comments at `ai.rs` `tactical_auto_beat`
/ `tactical_intent` become false and are rewritten, in CLAUDE.md, the `seams`
skill and the graph.

### 2. `[U]` on the battle map

A new tactical action that uses one consumable through `Game::consume_item(actor,
id)` — the door `BattleAction::UseItem` already shares with the map. It costs
one action and ends through `Game::hand_on_turn`, like any other. `[U]` opens a
picker of the consumables in the pack (the group model's `u` rows); every
refusal lands before the item is spent. Uppercase, per the row-selector rule.
A new `Mode` joins `ALL_MODES` and the gui draw match.

### 3. When auto drinks a cell

`ability_unavailable` currently returns `Option<String>`. It becomes a
structured refusal (`RoutineRefusal`, with a `Power { shortfall }` variant)
rendered to the same strings at its existing call sites, so "refused only for
Power" is a match, never a string comparison.

When the party arm's preferred routine — the first candidate that passes every
gate *except* Power — is refused with `Power`, and no other candidate is
affordable, the body's intent is `Intent::UseItem(id)` for a cell. The next
beat re-asks the planner; nothing is reserved. A new `Intent` variant reaches
every match on `Intent` in `ai.rs` (range, `helpful`, the forecast's
`ForecastAction`); a cell is drunk where the body stands, so it plans no walk.

Which cell is **data, not an id**: a consumable whose `consume` restores Power
and which carries no `prebattle_buff`. Take the smallest restore that covers
the shortfall, else the largest. In shipped content that is `power_cell`
alone. The pack is shared, so a companion may drink one — the group model's
`UseItem` is per member too.

## Testing (engine first, TDD)

- auto invokes a ready, affordable routine
- a cooling or unaffordable routine falls back to a swing
- a cell is drunk only when Power is the sole refusal; never a
  sustain/backfeed-style cell; never with none in the pack
- `SwingOnly` (the arena path) never invokes and never drinks
- `[R]` invokes (`tests/auto_resolve.rs`)
- `RoutineRefusal` renders the strings the existing tests already assert
- app-core: `[U]` spends one action; a refused item spends nothing
- gui: the new mode draws (the `ALL_MODES` census)

## Not changing

Save format, schema, `.ron` files, the group model, hostile behaviour, and
`balance_sim` (which models no abilities).
