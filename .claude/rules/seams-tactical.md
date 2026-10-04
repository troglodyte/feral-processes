---
paths:
  - "**/tactical*"
  - "**/tactical/**"
  - "**/squads*"
  - "**/reinforcement*"
  - "**/tamper*"
  - "**/taunt*"
  - "**/throw*"
  - "**/combat*"
  - "**/battle*"
---

# Load-bearing seams: Tactical battles

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **A battle map's coordinates live in `TacticalBattle`; `Position` is never
  written** — the third space after the Stack's `Locale` and base space's
  own, and `tactical/` not importing `Position` is the whole enforcement.
- **A body is a wall in `reach::movement_field`, and the allowance is both
  the budget and `walk_field`'s search box** — safe only because no step
  costs less than one, which is what makes `TACTICAL_MOVE_MIN`/`MAX`
  correctness bounds.
- **A fight ends through `Game::finish_fight`, and a `FightVerdict` is what
  each model answers it with** — `won` is the roster *emptied*, never
  "nothing is alive".
- **A fight's payout is reached through `Game::fight_rewards_mut`, and what
  a hostile's death pays is `Game::finish_hostile`** — both model-blind, and
  `finish_member` keeps only what is group-shaped.
- **A tactical fight's initiative is rolled once and kept in step by
  deletion, and the cursor names a body rather than a position.**
- **A step off the board edge is a departure and not a refusal, and the
  player's own is the jack-out.**
- **A routine's `shape:` and `range:` are read in tactical fights alone, and
  `AbilityDef::tactical_shape`/`tactical_range` is the one place an authored
  figure and the one derived from `AbilityTarget` are reconciled.**
- **`AbilityDef::tactical_*` reconciles authored and derived;
  `Game::routine_tactical_*` is the one door that layers research on top, and
  every reader that can see a decompile calls it.**
- **`use_ability` is the door the two combat models share; each converts its
  own aim, and full friendly fire is `reach::recipients` never reading
  `Hostile`.**
- **`Game::decompile_body` is the capture, and taking the captured body out
  of the fight is each model's own half.**
- **A routine's effect is shared; its refusals are not** —
  `Game::run_tactical_routine`, taking a def rather than an index, and
  `cooldown_floor` the whole of the difference between the two doors.
- **A hostile decides what it will do before it decides where to stand, and
  the closing term is a shortfall to the *band*, never a distance to the
  target.**
- **Staying put is the default: a body walks only to a cell that beats its
  own on `cell_merit` (reach and closing), and crowding chooses among those
  cells but is never itself a reason to move.**
- **One draw an AI turn, spent on the cell, and none at temperature zero** —
  the aim and the swing target are argmaxes, and the candidates are sorted
  before they are scored.
- **A hostile's walk is a run of real `Game::tactical_step`s spent one cell a
  beat, and `tactical_ai_turn` is that beat loop rather than a second
  spelling of a turn.**
- **`TacticalBattle::walk` is an `Option` because `None` is "has not chosen
  yet" and `Some(vec![])` is "has arrived"** — read as one, a spent walk is
  re-planned every beat and the draw above becomes one a cell.
- **The pacing carry is in seconds because a battle map has two rates**, a
  cell of a walk against `TACTICAL_STEPS_PER_SECOND` and everything else
  against `TACTICAL_TURNS_PER_SECOND`, derived off `Game::tactical_walking`
  rather than held beside the carry.
- **`Game::start_battle` is where the model is chosen, by inspecting the
  pack** — the pursuit path cannot know whether it is a guardian or a
  patrol, and the arena never passes through it.
- **The arena is the second chooser, and `arena::stage` takes the model its
  *caller* can drive** rather than the one the file asks for.
- **A headless tactical rep drives both sides through `tactical_drive_turn`,
  and a party body swings without invoking** — `PartyPlan::AllAttack`'s
  parity, not a policy invented for the tester.
- **An AI turn hands the turn on once, because the action already did it.**
- **A tactical fight is drawn in the map pane, and its turn strip takes the
  compass block's slot** — a block inside the pane, never a border strip.
- **The tactical modes are deliberately not `is_battle`**, which gates the
  reveal and routes `Fx`; `App::advance_tactical` paces the wild side
  against `dt` instead.
- **A turn ends in one place, `Game::hand_on_turn`, and it hands on only if
  the body that acted is still the one acting** — a body killed by its own
  fumble or its own blast has already left the order, and `remove` handed
  the turn on as it went.
- **A round on a battle map spends the upkeep an abstract round spends, in
  that order** — `tick_combatant_upkeep`, then the reap, then the tick,
  because the upkeep can kill and `death_handling_system` rides the tick.
- **The order wraps in two places**, `end_turn` and `TacticalBattle::
  remove`, so `hand_on_turn` compares against the round its caller read
  before it acted, and a fight that ends mid-round is `settle_tactical`'s
  tick.
- **The results page has two producers, `Game::closing_rows`, and one row
  builder per half** — `planned` is the only field of fourteen the two
  models disagree about.
- **A capture is aimed at something hostile, refused at the player's door**,
  where a swing at your own is friendly fire and stays legal.
- **A cloak is a targeting rule: the filter sits at the five doors that
  *name* a body and at none of the doors that *resolve* against one**, and
  `Game::break_cloak` is the one door an action removes it through.
- **A brace on a board is `Game::begin_defend` with only the mitigation
  crossing over, and it is worth what the turn order says it is worth** —
  `DEFEND_AGGRO_WEIGHT` weights a slot a battle map has none of, and the
  round-cadence buff leaves a body on the last rung bracing against nobody.
- **`Game::swing_range` is the one door for how far a body swings, and its
  three readers are calls rather than copies** — holding a weapon at all
  replaces the species figure, and the range is a property of the body rather
  than of the move it rolls.
- **A swing needs line of sight, checked unconditionally** because
  `line_of_sight` excludes its endpoints and so is already a no-op for
  neighbours.
- **A routine needs it too, and `reach::aim_in_sight` is the census of which
  shapes read terrain at their aim** — a `Single` and a `Radius` are thrown at
  a cell and refused when it cannot be seen, a `Line` and a `Cone` are aimed
  as a direction and truncate themselves, and a `Radius`' splash is clipped
  from the aim inside `shape_cells`.
- **A `BoltCue` lives in `TacticalBattle` cells and is its own queue, never a
  fifth `EffectKind`**, with no `kind` field because one travel rule at every
  distance is also the melee feedback.
- **`TacticalFxQueue` is `BoltCue`'s pattern for a body's own hit or heal,
  cued once inside `apply_damage`/`restore_hp` rather than at each call
  site** — a hit reuses `EffectKind::Hit`'s wash and burst in gui, a heal
  and a reaction draw through `Fx::cell_marks`, one list keyed by `MarkKind`.
- **A battle map's blows are heard off `SwingCueQueue`, never off the
  reveal** — `Game::log_swing` fills it only while a `TacticalBattle` is
  open, `App::take_sounds` drains it, and `advance_reveal`'s cue is gated on
  `Mode::is_battle`.
- **A summon's containment is omission and not a check** — it never passes
  through `roster_parts()`, and the two places that *are* checks
  (`finish_fight`'s sweep and its `bench_or_dissolve` skip) are both code
  that never asks about `Tamed`.
- **A summon is pushed to `Party` and `planned` together**, and `plan_summons`
  fills the turn `slot_is_commanded` keeps the player's cursor off.
- **A reinforcement's lost turn is `Reinforcement::reorienting`, never a
  `Stun`, and `finish_fight` sends every reinforcement home.**
- **A body is spliced into tactical initiative *behind* the cursor, never
  ahead of it** — `TacticalBattle::insert_after_cursor` takes no index
  because `turn + 1` is the only safe one.
- **The battle camera is held on the body that acted**, because the turn is
  handed on inside the call that resolves the blow.
- **A battle map has three pacing waits, and the hand-over is the camera's**
  — `TACTICAL_HANDOVER_SECONDS`, which `CAMERA_DWELL_SECONDS` is a fraction
  of and `Game::tactical_turn_opening` is the derivation of.
- **The battle map passes no lag clamp, and the turn arrow and aim cursor are
  bounds-checked because of it.**
- **Walking into a hostile is a swing, and its two gates are `Hostile` and
  `Game::tactical_awaits_input`** — neither a new predicate.
- **Auto-attack and `[R]` invoke through `tactical_intent`'s party arm,
  gated on `Game::in_party` and `ability_unavailable`** — the arena passes
  `PartyTurns::SwingOnly` instead, so its numbers stay comparable, and
  `App::tactical_auto` is cleared by any key and by the fight ending.
- **`Game::decision_temperature` is the one door every tactical AI entry
  point reads**, and `tactical_ai_turn_at` is a test hook, not a fifth door.
- **A tamper ages on the tampered body's own hand-on, never in
  `tick_one_combatant`.**
- **A profiled hostile's forecast is a call into the planner its turn
  runs**: `tactical_intent`, `scored_cells` and `chosen_target`, with
  `argmax_scored` shared with `sample_scored`.
- **`AbilityEffect::tactical_only` is the group model's filter, applied at
  every chooser that is not a battle map**, and `use_ability`'s `Tamper` arm
  is `unreachable!` because of it.
- **A reaction is one budget, two triggers and one door** — `Game::provoke`,
  refunded in `TacticalBattle::begin_turn` because the cursor moves in two
  places and only one of them is `end_turn`.
- **A reaction reaches `TACTICAL_MELEE_RANGE` and never `Game::swing_range`**,
  because a reaction at weapon range is overwatch.
- **A reaction's swing is free and cannot fumble**, `battle::
  resolve_free_attack` — the Opening rung's non-recursion rule with a second
  caller.
- **A routine cut off by a reaction fizzles and is not refused**, keeping the
  Power and the cooldown the charge already took; a capture provokes nobody.
- **A reaction can close the fight under the action that provoked it**, so
  `provoke` answers *still on the board* rather than *still alive*.
- **Partial cover is one predicate, `reach::cover_between`**, whose arc test
  is a strictly-positive dot product and whose evasion enters at
  `Game::defender_profile_against`, the wrapper around `combatant_profile`
  that the AI, both telegraph marks and the roll all read through.
- **`tuning::FORMATIONS` is the whole of what a squad is, and an absent
  `components::Squad` is footprint 1, actions 1, swing share 1 at every
  reader.**
- **A body's cells come from `TacticalBattle::cells_of` and its reach from
  `reach::gap`, never a bare anchor**, and a footprint rule is only tested by
  a test with a real squad in it.
- **A body is shaped before it is seated**, `set_shape`'s assert, because
  `place` can only check the footprint it already knows about.
- **A turn ends when `actions_left` reaches zero, not when a body acts**, and
  `hand_on_turn`'s "still the one acting" guard sits beside that rather than
  under it.
- **A squad is never saved, and folding happens after the opening bearing is
  taken** — no world `Position` and no `Tamed`, so a pack folded too early
  degrades silently rather than failing.
- **A squad's death pays each remaining member, its capture is priced as the
  lead, and its disbanding is not damage** — so only the capture's cost goes
  through `Game::apply_damage`.
