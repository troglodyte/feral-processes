# Seams: Combat: tactical battles - reactions, fx, AI aim & squads

- **A reaction is one budget, two triggers and one door, and the refund is at
  a body's own turn.** Every body carries one reaction a round; leaving a
  hostile's melee reach provokes it and so does invoking beside one, and
  `Game::provoke` is where both land so the swing, the charge, the order and
  the stopping rule are written once. **The refund is `TacticalBattle::
  begin_turn` and not the hand-on**, which is the design's own wording and is
  wrong for the reason `hand_on_turn`'s doc already gives: the cursor moves in
  two places, and a body dying on the last rung starts the next round from
  inside `remove` with no `end_turn` reached — so a refund hung off the
  hand-on misses whoever the wrap landed on, and one body cannot react for a
  round for a reason nothing on screen explains. It cost one reordering,
  `remove` wrapping before it begins the turn, because `begin_turn` reads
  `actor()` to know whose budget to hand back. **Reach is
  `TACTICAL_MELEE_RANGE` and deliberately not `Game::swing_range`** — this is
  the one place in `tactical/` that reads the constant rather than asking how
  far a body swings, because a reaction at weapon range is overwatch, a
  different feature, and a reach weapon must not make its holder a three-cell
  tripwire. **The swing is free and cannot fumble**, the Opening rung's
  non-recursion rule finding a second caller in
  `battle::resolve_free_attack`; `Swing::free`'s polarity is the Default-safe
  one, so a `Swing::default()` written later cannot switch the ladder off. **A
  cut-off routine fizzles rather than being refused**: the provocation is
  below the charge, so the Power and the cooldown are gone and nothing lands,
  which is the rest interrupt's shape — and `Decompile` provokes nobody,
  because a capture's whole cost is already its catalyst. The trap found three
  times in the building is that **a reaction can close the fight under the
  action that provoked it**: `provoke` answers *still on the board* rather
  than `creature_alive`, because a Forgiving player is rebooted by the tick
  the reap rides and reads as alive with no cell, and the caller then walks
  into a `TacticalBattle` that is gone. `step_along_walk`'s refusal arm is the
  same trap mirrored — it commits an empty walk, which after a fatal reaction
  would commit it to *the next body*. The AI term rides **merit** and not just
  the score (`walk_risk` inside `scored_cells`), because the filter is what
  makes staying put the default and a cell that costs three Integrity on the
  way out is not a reason to leave the one you are on; the damage is a *call*
  to `battle::expected_damage` and an expectation rather than a roll, so
  temperature zero stays stream-neutral. **What the design asks for and is not
  built**: discounting a *routine's* value by its expected reaction damage,
  because `tactical_intent` has no value scale to discount — it answers Some
  or None — so it would mean inventing a routine-versus-swing comparison the
  model does not have. And `docs/measurements/2026-09-17-tactical-reactions-arena-blind.md`
  is the instrument's own blind spot: **zero reactions fire in fifty reps** of
  the only tactical arena scenario, because the planner holds its ground once
  in reach and nothing there invokes in melee. See
  `seam:a-reaction-is-one-budget-two-triggers-and-one-door`.

- **A `BoltCue` lives in `TacticalBattle` cells and is its own queue, never a
  fifth `EffectKind`.** `VisualEffect`'s whole shape is a *world* tile, and a
  board cell pushed into `EffectQueue` pins a flash to an unrelated tile out
  in the zone — the `Position` seam's convenience in a new place, where the
  grid-of-cells shape makes the wrong thing compile *and draw*. `TransitCue`
  refused the same fold one space over and is the precedent. **No `kind`
  field**: the streak travels `from` → `to` by one rule at every distance,
  and at one cell that is a short flick across a single square, which *is*
  the melee feedback — a kind field would make a renderer restate the melee
  threshold to pick between two draws, the third copy the door above exists
  to prevent. What the cue gained is an **`fx` id** (`ItemDef.fx` of the
  wielded weapon, `AbilityDef.fx` for a routine): data names the draw, the
  engine copies it and never reads it, and an unknown id falls back to
  `streak`. A routine's blast is a second list in `BoltQueue`
  (`RoutineCue`, cells from `reach::shape_cells`), not a new Resource.
  **Every travel primitive finishes inside `BOLT_SECONDS`**, or a richer
  effect is still in flight when the next body acts. `EffectLibrary` lives in
  `Fx.library`, and `Impact::Sparks` draws nothing because the engine's `Hit`
  cue already bursts. **The colour is carried and not looked up**, because a fumble's
  Recoil rung can kill the body that swung and a lookup would have nothing to
  ask. Pushed **before** `resolve_and_apply_attack` and inside the recipients
  loop, so a body that dies to the blow still gets its streak and a sweep
  fires one per body it caught. gui's `BOLT_SECONDS` is **derived** from
  `TACTICAL_TURNS_PER_SECOND` rather than restated, or a retune of the pace
  leaves a streak in flight when the next body acts and reads as two attacks
  at once; and it takes **no stagger** where `Walker` has one — a squad files
  out one behind the other, but a sweep's streaks all leave the same swinger
  in the same instant. See
  `seam:a-boltcue-lives-in-tacticalbattle-cells-and-is-its-own-queue`.

- **`TacticalFxQueue` is `BoltCue`'s pattern applied to a body's own hit or
  heal, and it is cued inside `apply_damage`/`restore_hp` themselves.**
  `Game::apply_damage` and `Game::restore_hp` (`game/combat_damage.rs`) are
  already the two doors CLAUDE.md names for damaging and healing a
  creature, so hooking `cue_tactical_fx` there — rather than at
  `resolve_and_apply_attack`, `apply_fumble_rung`, the Heal/Drain arms of
  `use_ability`, a Repair Bay tick, a sortie heal, and every other caller —
  is what keeps the zero-gate (`dealt > 0` / `restored > 0`) a single
  assertion instead of one a new call site can forget. `cue_tactical_fx`
  itself is a no-op wherever `TacticalBattle::cell_of(target)` answers
  `None`, which is every one of those other callers. **A `kind` field,
  where `BoltCue` has none**: a hit and a heal draw nothing alike — a red
  wash with a spark burst against a green mark that bounces — so
  `TacticalFxKind::{Hit, Heal}` is the branch a shapeless cue would only
  hand back to gui. **The gui side reuses rather than copies**:
  `fx.rs`'s `tile_flash`/`draw_bursts` are each split into a private core
  (`flash_in`/`draw_bursts_in`, taking a slice) plus two thin wrappers, so
  a Hit cue drives `EffectKind::Hit`'s existing wash and spark burst
  through a second list (`Fx::tactical_flashes`, board cells) rather than
  a second implementation. A Heal has no match in `EffectKind`'s table at
  all, so it gets its own list (`Fx::cell_marks`) and its own draw call
  (`draw_cell_marks`), tinted `palette::HEALTHY` — no new role needed,
  since a body's Integrity coming back is exactly what that role already
  means — and lifted by `(PI * HEAL_MARK_BOUNCES * t).sin().abs()`, whose
  `abs()` is what keeps the curve touching its rest position at the start,
  the middle and the end rather than dipping below it. **A reaction is the
  third kind** (`TacticalFxKind::Reaction`, pushed by `Game::provoke` at the
  *reactor's* cell) and the second mark: `CellMark`'s `MarkKind` owns the
  glyph, hue, life and height curve, so a `!` in `ATTENTION` that pops and
  a `+` that bounces are one list and one draw rather than two copies of
  the centring. gui's `frame_cues` also hears it, `SoundEvent::Reaction`,
  off the same queue — not gated on base space. Both lists are
  cleared, not merely left to expire, the moment `begin_frame`'s
  `in_battle` goes false — board cell indices are reused between fights,
  `clear_bars()`'s own reason. See
  `seam:tacticalfxqueue-is-boltcues-pattern-for-a-bodys-own-hit-or`.

- **A battle map's round wrap is marked in two places and queued in
  neither.** The engine half is one `MessageKind::Round` divider at the top
  of `Game::tactical_round_upkeep`, which both wraps reach (`hand_on_turn`
  and a siege body leaving the board in `siege/raiders.rs`), so a third
  wrap path marks itself by calling it. The gui half is derived, not cued:
  `Fx::observe_round` diffs `Game::tactical_round` once a frame, and a wrap
  is a round *rising* while one fight stays open — `None` between fights is
  what stops a new fight's round 1 from reading as a wrap. The sound is
  played off `observe_round`'s answer whether or not effects are on.

- **`Fx`'s fight-scoped state is kept by `gui::fx_in_fight`, never by
  `Mode::is_battle` alone.** A battle map is deliberately not a battle
  mode, and `begin_frame` clears the bars, `tactical_flashes` and
  `cell_marks` whenever its flag is false — so with `is_battle` alone every
  hit flash, heal mark and reaction mark on a battle map was cleared in the
  frame that pushed it, shipped that way, and no test saw it because every
  renderer test passes `true` by hand. The flag asks `in_tactical_battle`
  rather than listing modes, since the clear exists so a mark cannot
  outlive the board its cell indexes. `draw_a_frame` calls the same
  function, which is what makes the regression test a test of the frame.

- **A battle map's blows are heard off `SwingCueQueue`, never off the
  reveal.** A tactical fight never calls `MessageLog::open_round`, so
  `Game::battle_log` is the whole fight, and `App::advance_reveal` — which
  steps through it *by position* — went silent mid-fight once
  `MESSAGE_LOG_CAP` dropped lines off the front: the range's length stopped
  moving at 100, `revealed` caught up, and nothing after was ever heard.
  Before that it was also late, pacing blows at `REVEAL_LINES_PER_SECOND`
  against a board paced by `TACTICAL_TURNS_PER_SECOND`. So `log_swing` — the
  one door every swing's band goes through, both models' `use_ability`
  included — pushes the band onto its own queue, **gated on the
  `TacticalBattle` resource**, or the group model is heard twice.
  `App::take_sounds` drains it (app-core owns `swing_sound`, so the engine
  queues a `SwingOutcome` rather than naming a sound) and must be called
  every frame, `take_bolts`' reason. The reveal's own push is gated on
  `Mode::is_battle` for the mirror reason; the tactical modes are
  deliberately not battle modes. **No cell** where `TacticalFxCue` has one:
  a cue is heard, not drawn. Held by
  `a_swing_is_heard_however_full_the_log_is` (mutation-checked) and
  `the_reveal_is_silent_on_a_battle_map`. See
  `seam:a-battle-maps-blows-are-heard-off-swingcuequeue-never-off-the`.

- **The battle camera is held on the body that acted, and the dwell is
  derived from the turn beat.** `Game::hand_on_turn` fires inside the same
  call that resolves an attack, so `TacticalView::active` already names the
  *next* body the instant the swing lands — and the camera was aimed at
  `acting_cell(view)`, the live answer, which put every blow off screen with
  its streak still in flight. Aiming it there again is the "simplification"
  to refuse: it reads as deleting pointless indirection. `Fx::battle_center`
  holds a `CameraHold` instead, and its `release` is pushed forward on every
  frame the same body is still acting, so the dwell is measured from the
  hand-over without anything having to notice one — which also keeps a
  **walk** centred, since its steps are six a second and a hold would fall a
  cell behind. That collapses to **two match arms**: a body still acting and
  a body being latched onto fresh want the identical write.
  `CAMERA_DWELL_SECONDS` is derived on `BOLT_SECONDS`' rule, but **from
  `TACTICAL_HANDOVER_SECONDS` and not from the turn beat** — and that
  correction is the seam's second half. Tied to the beat the hold could only
  reach 0.344s, of which `HIT_FLASH_SECONDS` spent 0.30: 44ms of stillness,
  which is not a pause anybody sees. Raising the fraction alone is the wrong
  fix and the arithmetic says so — a 0.55s hold puts the pan's landing at
  0.80s against a body that acts at 0.625s, so the fix that put a blow on
  screen puts the *following* one off it. Stretching
  `TACTICAL_TURNS_PER_SECOND` buys the room and charges twice, since every
  wild turn spends a hand-over **and** a beat between arriving and striking.
  So the hand-over is its own figure (0.9s, hold 0.558s, 0.258s of stillness,
  pan landing by 0.81s) and **the third wait is derived, not remembered**,
  which is what keeps `advance_tactical`'s "no second piece of pacing state"
  claim true: `Game::tactical_turn_opening` is `!walk_planned() && spent()
  == 0 && !acted()`, exactly the state between one body's action and the
  next body's first beat — `walk_planned` and not `walking`, because
  `Some(vec![])` is a body that has arrived. Two of the three invariants are
  **`const _: () = assert!(…)`** beside their constants rather than tests,
  clippy's `assertions_on_constants` being right and a build failure being
  stronger; the pan fitting cannot be, because its length falls out of
  `CAMERA_DECAY`, so that one eases the real `camera_step` across the widest
  board in the time the hold leaves over. The `seen`
  field is what tells a board coming back on screen from one that never
  left it, without which the second fight of a session pans in from wherever
  the surface map left the camera. **The pan cost two bounds checks**:
  `camera_step` takes its clamp as a parameter because `CAMERA_MAX_LAG`'s
  argument is the surface map's one extra ring of tiles and applies to
  nothing else, and at one tile every hand-over was a cut — but the turn
  arrow and the aim cursor were both drawn *unchecked*, on the grounds that
  the acting body was within one tile of pane centre by construction, and
  the map pane is a region of a shared screen. The dwell's own test trap:
  read off a third "witness" body it is **vacuous**, because a lone-hostile
  fight has two or three bodies and the player is usually one of the two in
  the order, so the glyph box is `None` in both frames and `assert_eq!` of
  two `None`s passes. See
  `seam:the-battle-camera-is-held-on-the-body-that-acted`.

- **Walking into a hostile is a swing, and both of its gates are doors that
  already existed.** `move_player`'s bump ladder one space over: an occupied
  cell answered `Refused`, so an arrow key pressed at the body a whole turn
  had been spent closing on did nothing. The swing is `tactical_attack` and
  not a second spelling of one, so the range, the sight check, a reach
  weapon's sweep, the cloak refusal, the reap and the hand-on all come from
  there — and two things fall out rather than being written: a bump **ends
  the turn**, because the action is what a swing costs, and a bump into a
  cloaked body is refused and then falls through to the occupied-cell
  refusal, so it leaks nothing that seam has not already conceded. It is
  read **above** the movement gates, because a swing is priced in the action
  and a body that has walked its whole allowance can still finish the
  approach it spent it on. The gates are `Hostile` — friendly fire through
  the aim cursor is full and legal, but an arrow key is not an aim, and a
  bump that hit whatever was in the way makes crossing your own line a coin
  flip — and `tactical_awaits_input`, which is false for exactly the bodies
  the beat loop drives: `tactical_step` is the door that loop's walk goes
  through, so without it a hostile spends its action part-way along a path
  it planned, on its own side. Asked inside `tactical_step` it is
  caller-independent, since the acting body *is* the stepping body, which is
  what keeps it from becoming the third predicate `TacticalView::player_turn`
  already was. `StepOutcome::Struck` is a fourth variant rather than a
  `Moved` that lies; its arm in `step_along_walk` is grouped with the
  refusals, because the gate makes it unreachable and a fired one has spent
  the action anyway. **The test trap, found by mutation**: a hostile stepping
  into the *player* passes with the second gate deleted, because the player
  is not `Hostile` and the first gate already refuses it — it has to field
  two hostiles. And a landing bump cannot be forced (the move is rolled
  first) or cloned (`Game` is not `Clone`), so it is found by rebuilding the
  fight per stream. See
  `seam:walking-into-a-hostile-is-a-swing-and-both-gates-are`.

- **A summon's containment is omission, not a check — it never passes through
  `roster_parts()`.** `Game::fork_programs` spawns through
  `spawn_wild_creature_scaled` and strips `Hostile`/`WanderAi`,
  `adopt_program`'s two removals, and stops there. No `Tamed` is what makes
  it invisible to `pet_count`, `base_staff`, the labour scheduler, `rest`,
  memories, needs and the save; no `Experience` is what makes
  `award_companion_xp` skip it with no exclusion written; no `ProgramId` is
  what leaves no orphaned memories to reconcile on load. The rejected
  alternative — a `Tamed` program carrying a `temporary` flag — inverts
  that: every one of those eight subsystems needs a new exclusion, and each
  one compiles fine when forgotten. It gains exactly two things,
  `components::Summoned` (battle-scoped on `Cloaked`'s precedent, so no
  `SAVE_FORMAT_VERSION` bump) and a `PowerReserve`, because
  `ability_unavailable` reads the reserve off the entity in question and a
  body without one could never run the moves it was spawned to run. Two
  places genuinely are checks, both because the code they sit in never asks
  about `Tamed`: `finish_fight`'s `With<Summoned>` sweep — unconditional,
  living or not, `resolve_sortie_battle`'s rule, and enough on its own
  because all five endings funnel through `finish_fight` — and the
  `bench_or_dissolve` skip in the same function's dead loop, without which
  a dead fork announces a downed program the player never had and
  `detach_from_play` `retain`s it out of `Party` mid-teardown, which is the
  removal the slot seam forbids. See
  `seam:a-summons-containment-is-omission-not-a-check`.

- **A reinforcement's lost turn is `Reinforcement::reorienting`, never a
  `Stun`, and `finish_fight` sends every one home.** A `Stun` costs no turn on
  a battle map — it only stops a walk — so `pass_reorienting_turn` passes it
  from `hand_on_turn`. The landing is `Summon`'s (whole-board nearest free
  cell, pushed to `Party` uncapped), `call_reinforcement` strips `Task`/
  `Carrying` itself because the scheduler does not run mid-fight, and the
  teardown after the dead-party loop is what returns a survivor to `Staff` by
  omission. No save field: a non-siege battle map is never saved.

- **A summon is pushed to `Party` and `planned` together, and the failure
  mode is silence.** `planned` is sized once at `begin_battle` as
  `Party.len() + 1` while `roll_initiative` reads `Party.0.len()` **live**,
  so a body pushed to `Party` alone draws an initiative rung and then never
  acts (`plan.get(slot)` is `None`) while `living_party`, `battle_rows`,
  `battle_active_slot` and hostile targeting all iterate `planned.len()` and
  cannot see it — invisible, inert, and nothing fails. A test asserting only
  that the body exists passes against it, so assert on it *acting*.
  Appending is safe where removal is not: the seam forbids removal, which
  shifts every slot behind it, which is why `dissolve_summons` **kills**
  rather than removes and is model-blind — a dead body left in its slot is
  what already happens to a companion that dies mid-fight, and each model's
  reap carries it from there. The other half is that a fork acts but is not
  the player's to command: `slot_is_commanded` (one predicate at four
  planning gates, not the check repeated) keeps the cursor off it and
  `plan_summons` fills the gap at the top of `battle_resolve_round`, above
  the `planned` clone. `choose_summon_action` is deliberately not
  `choose_wild_action` — that is the trained policy, whose features speak
  group indices and aggro slots from the hostile side, and whose weights are
  pinned against a design boundary this would cross. See
  `seam:a-summon-is-pushed-to-party-and-planned-together`.

- **A body is spliced into tactical initiative behind the cursor, never
  ahead of it.** `TacticalBattle::insert_after_cursor` always inserts at
  `turn + 1` and takes no index, because there is exactly one correct answer
  and a caller choosing would be a caller getting it wrong. The cursor names
  a body rather than a position — `remove`'s three cases above are this trap
  in the other direction — so inserting ahead of it shifts every later entry
  down one and somebody acts twice with nothing on screen saying why. The
  test has to compare the order **by identity** across a mid-round splice:
  *a count of turns taken is conserved under a cursor shift* and passes
  against the bug. Placement is `deploy::nearest_free`, already exactly the
  search wanted, with `taken` built from `TacticalBattle::bodies()`; the
  seventh refusal in `tactical_use_routine` is what keeps a board with no
  room from spending anything. `tactical_ai_actor`'s gate widened to
  `Hostile` **or** `Summoned` — the first exception to "every party body is
  the player's to command", stated in its doc or the next reader reads the
  gate as a bug — and sidedness needed nothing, since `tactical_sides` is
  already relative to the actor. One asymmetry left standing: a fork on a
  battle map swings rather than invoking, because `run_tactical_beat`'s
  routine intent is still gated on `Hostile`. See
  `seam:a-body-is-spliced-into-initiative-behind-the-cursor-never-ahead`.

- **Auto-attack and `[R]` invoke through `tactical_intent`'s party arm,
  gated on `ability_unavailable`.** Before todo #103, a battle-map party
  turn under either driver always swung — `run_tactical_beat`'s
  routine-intent branch was `Hostile`-only — so a party body driven by the
  player's own request never got a routine even though nothing else about
  the fight changed shape, which made auto-attack strictly weaker than
  playing by hand. The fix threads a `PartyTurns` enum (`Invoke` |
  `SwingOnly`) from each driver down through `run_tactical_turn` →
  `run_tactical_beat` → `tactical_intent`: `tactical_auto_beat` (`[A]`) and
  `auto_resolve_battle_with` (`[R]`) pass `Invoke`; `tactical_ai_beat` (the
  ordinary hostile/summoned/taken-over AI door) and `arena::run` pass
  `SwingOnly` — permanently, not as a stopgap, since `PartyPlan::AllAttack`
  invokes no routine either and the two models' numbers stay comparable
  only if neither auto-invokes headless. The party arm opens only for
  `Invoke` and only when the actor is `Game::in_party` — the player or a
  member of `Party` — and neither `Summoned` nor taken over, so a fork
  reached through `tactical_drive_turn` (which drives whichever body is
  acting with no gate of its own) still falls through to a swing — a body
  with a `PowerReserve` the player never funded invoking a priced routine is
  a separate decision this feature does not make. **`in_party` shipped a
  round after the rest** (an opus review of the merged feature caught it):
  the original gate excluded only `Hostile`/`Summoned`/taken-over, so base
  staff mid-siege — `Tamed`, never in `Party`, and `tactical_skippable` —
  fell through to it too and tried to invoke or drink a cell
  `Game::tactical_use_item` then refused, wasting the beat every turn.
  `in_party` is now the one membership check both doors share. The party
  arm's own choice is the first candidate — from `Game::ready_party_routines`,
  which asks `Game::actor_abilities` (the same kit the hand chooser offers,
  so an emulating player is not offered whatever is installed underneath
  the emulation) rather than a body's raw `Routines` — `ability_unavailable`
  passes, else — if the *sole* refusal for some candidate is
  `RoutineRefusal::Power` — a Power cell from the pack via
  `power_cell_for`, else a swing: `ability_unavailable`'s Power check runs
  *last*, after cooldown, tactical-only and the Decompile/Emulate/Teleport
  arms, precisely so `Power` alone means every other gate already passed.
  `power_cell_for` refuses any item whose `consume` carries a
  `prebattle_buff` — a sustain/backfeed cell primes a long-running Trickle
  buff meant to be spent deliberately, not drunk automatically because it
  also restores some Power. `Game::tactical_use_item` is the one door both
  the party arm and `[U]` on the battle map spend a cell through, every
  refusal landing before anything is spent. An invoked routine's cooldown
  floors at the hand's own (0, `tactical_use_routine`'s), not the hostile
  AI's (`ENEMY_ROUTINE_MIN_COOLDOWN`) — `run_tactical_intent` switches on
  the actor's `Hostile` marker, the one honest split between the two doors
  that call it. See
  `seam:auto-attack-invokes-through-tactical-intents-party-arm`.

- **`Game::decision_temperature` is the one door every tactical AI entry
  point reads, and `tactical_ai_turn_at` is a test hook, not a fifth
  door.** Before this, `tuning::TACTICAL_AI_TEMPERATURE` was passed
  literally at four call sites in `tactical/ai.rs`, each a place a fifth
  site or a later refactor could read the constant directly and silently
  miss a Cold Sample tamper. The mutation check,
  `every_tactical_ai_door_reads_the_temperature_door`, repeats the same
  Cold-tamper assertion through all four doors; reverting any one site to
  the bare constant fails exactly that case. `tactical_ai_turn_at` stays a
  test hook precisely because it is *not* one of the four — it lets a test
  force a temperature with no `Tampered` component, and must never become a
  production caller. `ENEMY_POLICY_TEMPERATURE` (the group model) is
  untouched: the group model has no board for a hostile to be tampered on.
  After the seam, `rg TACTICAL_AI_TEMPERATURE crates/engine/src/tactical`
  should show only the door's own definition and the log line's HOT/COLD
  comparison. See
  `seam:decision-temperature-is-the-one-door-every-tactical-ai-entry-point-reads`.

- **A tamper ages on the tampered body's own hand-on, never in
  `tick_one_combatant`.** Every other piece of in-fight state that ages per
  turn — statuses, buffs, cooldowns, the cloak — ages at round upkeep
  inside `tick_one_combatant`. A tamper does not, because a `duration: 1`
  Prompt Injection landed on a hostile that has *already acted this round*
  would, under the round rule, age to zero at that same upkeep before the
  target ever took a turn under it — full Power and cooldown spent for zero
  observable effect. `Tampered::age()` runs instead from
  `Game::hand_on_turn`, keyed to the tampered body's *own* turn being
  handed on, which guarantees exactly one turn taken under the entry
  regardless of when in the round it landed. The mutation check,
  `a_one_turn_injection_on_a_body_that_has_acted_is_live_on_its_next_turn`,
  fails the moment ageing moves back into `tick_one_combatant`. The one
  exception is `TamperEntry::fresh`: a self-applied entry (a companion's
  own radius tamper catching the invoker) skips its first ageing rather
  than decrementing, or it would lose a turn the target never had. Every
  path that ends a turn without going through `hand_on_turn` — a stunned
  body's skipped turn, `run_tactical_beat`'s empty-targets branch — had to
  be checked and routed through it, or an entry on that body freezes
  forever instead of expiring. See
  `seam:a-tamper-ages-on-the-tampered-bodys-own-hand-on-never-in-tick-one-combatant`.

- **A profiled hostile's forecast is a call into the planner its turn
  runs: `tactical_intent`, `scored_cells` and `chosen_target`, with
  `argmax_scored` shared with `sample_scored`.** CLAUDE.md's "a mirror must
  be a call, not a copy" rule applied here. Before this, the turn's
  planning lived inline — a local `Intent` built fresh in
  `run_tactical_beat`, the destination cell picked inside
  `walk_to_best_cell`'s own body — with no reusable function a forecast
  could call instead of restating. The extraction is a pure refactor
  (Task 7 Part A: the full tactical suite stays green with zero test
  edits) that pulls out `tactical_intent`, `scored_cells` and
  `chosen_target`, parameterised on a `from` cell rather than reading the
  actor's live position so they can be asked about a hypothetical
  destination. `Game::tactical_forecast` then calls the same three: the
  action always comes from `tactical_intent`; only at
  `decision_temperature(body) <= 0.0` does it also call `scored_cells` and
  take `argmax_scored` — never `sample_scored` — to name a destination and
  target. `argmax_scored` was extracted out of `sample_scored`'s own
  closure so the forecast and the real turn cannot disagree about
  tie-breaking. The mutation check,
  `a_cold_profiled_hostile_does_what_its_forecast_said`, sweeps 24 seeded
  boards comparing the forecast against what the turn then actually does;
  changing the forecast to call `sample_scored` at 0.5 with a cloned RNG —
  a plausible-looking "just sample like a real turn" bug — fails it. The
  forecast answers `None` for a body mid-turn (`!walk_planned() &&
  !acted`), which is what makes "exact at the moment the turn begins" a
  testable claim rather than a vague one. See
  `seam:a-profiled-hostiles-forecast-is-a-call-into-the-planner-its-turn-runs`.

- **`AbilityEffect::tactical_only` is the group model's filter, applied at
  every chooser that is not a battle map, and `use_ability`'s `Tamper` arm
  is `unreachable!` because of it.** A tamper routine only means something
  against a hostile AI with a board, a cell and a walk to plan; the group
  model has none of that. Rather than teach every group-model path to
  special-case `Tamper`, `tactical_only()` is one exhaustive match (true
  for `Tamper` alone) applied at `wild_routine_ready`,
  `battle_special_options` (not `tactical_routine_options` — the filter
  sits at the caller that needs it, not inside the shared
  `special_options_for`), `choose_summon_action`, `wieldable_routines`,
  `field_runnable` and the sortie's off-screen resolution. Because every
  door in is closed, the branch inside `use_ability` that would actually
  run a `Tamper` effect is provably unreachable there, and marking it
  `unreachable!()` (beside `Decompile` and `Summon`) turns a hole in the
  filter into a compile-clean panic on first hit instead of a silent
  no-op — the effect resolves only through `run_tactical_routine`'s own
  `Tamper` branch, the one caller with the battle-map context (a
  `TacticalBattle`, a cell, a side) it needs. A sortie reaching
  `use_ability` with a `Tamper` effect would hit the `unreachable!`, which
  is why that test is the one that matters — everything upstream of it is
  defense in depth, and the panic is the backstop that proves it held. The
  same reasoning exempts the five tier-A tamper routines from
  `every_battle_ability_family_is_contiguous_from_single_upward`: that
  census exists for species-kit and hunt-pool content, a tamper routine is
  research-taught and enters neither, so `Heat Injection Group` and
  `Hallucination Group` may ship with no Single rung — one more `.filter`
  clause beside `Summon`'s existing exemption. See
  `seam:ability-effect-tactical-only-is-the-group-models-filter`.

- **`reach::cover_between` is the only thing that decides partial cover, and
  the arc test is a dot product that must be strictly positive.** A boulder
  beside the defender is only cover if it is on the *attacker's* side, which
  is the whole of what makes walking around it flanking; a `>= 0` comparison
  admits a boulder exactly abeam and hands out cover for standing next to a
  rock. That is deliberately the opposite choice from `shape_cells`' cone
  epsilon, which admits equality because an eight-way grid's diagonals sit
  exactly on the wedge — there is no grid artefact to rescue here. The rule
  reads `BattleCell::Cover` and not `blocks_sight()`, because the two are the
  same predicate only today and `blocks_sight` is the one a fifth kind would
  join. The evasion it buys enters at `Game::defender_profile_against`, which
  **wraps** `combatant_profile` rather than copying it: cover is a property
  of the pair and `combatant_profile` takes one entity by design, so the
  Exposed rung stays where it is. That door reads
  `world.get_resource::<TacticalBattle>()` and never `resource::<_>()` — it
  runs in the group model too, where a panic would be the whole group model.
  The flag that switches it off is `Swing::cover_ignored` and not
  `cover_applies`, for `Swing::free`'s reason: `Default` makes `false` the
  answer a later `..Default::default()` gives, and that has to be the answer
  that keeps the feature. The AI's term splits across two functions on
  purpose — `cell_merit` for a ranged band, because `scored_cells` uses merit
  as the candidate filter and a cover term anywhere else never opens a
  covered cell as a candidate; `cell_score` for a melee one, because a
  covered cell opened as a *candidate* is the sidestep pathology that filter
  was added to fix, and a melee body would stall behind boulders instead of
  closing. Both weights are read against the terms beside them
  (`TACTICAL_AI_REACH_SCORE`, `TACTICAL_AI_CLOSING_WEIGHT`,
  `TACTICAL_AI_CROWDING_WEIGHT`) and not against what cover is worth in the
  roll. The telegraph is two thin calls into the same rule —
  `TacticalBody::in_cover` for a body on a hostile's turn and
  `Game::body_in_cover` for the one the player is aiming at — and both, with
  `TacticalView::covered`, are cleared by `TacticalView::frozen`. See
  `seam:tactical-cover`.

## Tactical squads

`tuning::FORMATIONS` is the whole table — five of one species, footprint 2,
two actions, `swing_share` 0.5, the noun and the `^` mark — and it is in
`tuning.rs` rather than `assets/` because how hard the game is, is not
moddable. **A single body is not a formation row**: every reader treats an
absent `components::Squad` as footprint 1, actions 1, share 1, which is what
keeps the whole feature off the ordinary path.

The trap is that a squad is the first body that is **not one cell**, and
every reader that measured from a bare anchor was silently right until it
existed. `TacticalBattle::cells_of`/`footprint_of` and `reach::gap` are the
doors; a reader left on `distance` disagrees with the door beside it rather
than failing to compile. Two shipped that way and both had the same shape: a
squad's own four cells counted as walls in its *own* `movement_field`, so it
could not step right or down at all, and `ai.rs` measured anchor-to-anchor
while `tactical_attack` measured `gap`, so a squad adjacent along its bottom
edge declined to swing and tried to walk — which the first bug then
prevented. Neither failed a test, because every fixture that could see them
was hand-built at footprint 1. **A footprint rule needs a test with a real
squad in it.**

A squad's shape is **set before it is seated**, and `set_shape`'s
`debug_assert` is what holds that: `place` validates the footprint it knows
about, so a body seated while still reading as one cell has its block checked
against nothing, and widening it afterwards leaves two bodies overlapping
with no refusal anywhere. That ordering was correct for a while only because
`deploy::plan` reserved a clear block two files away — an invariant in
another module is not an invariant.

`actions_left` replaced `acted`, and the turn ends when it reaches zero
rather than when a body acts. `hand_on_turn`'s existing "the body that acted
is still the one acting" guard sits **beside** the new one, not under it: a
body killed by its own fumble or its own blast has already left the order and
`remove` handed the turn on as it went, so a second action must not
resurrect it.

Three omissions carry the rest. A squad has no world `Position` and no
`Tamed`, which is what keeps it out of `save.rs` — so folding must happen
**after** the opening bearing is taken off the original pack, or
`gather_pack` reads a pack of one and the bearing degenerates, both silently.
It does carry `Creature` (`group_pack` drops a member without one) and
`StatusEffects` (`arm_status` is a documented no-op without it, while
`use_ability` logs the condition landing regardless — so a squad read as
immune to Exposed while the log said otherwise). Death pays each **remaining
member's** own payout with the overkill shared as a *parameter*, never by
writing a member's `Stats`; a capture takes the lead and prices the roll as
the lead, not as the summed block; and disbanding hands each member back at
the squad's Integrity fraction, floored at 1, which is deliberately **not**
damage and so does not pass through `apply_damage`. See `seam:tactical-squads`.

- **A prop is not a body, and its reach, sight and cover are asked of `Board`
  methods, never `Board::cell`.** A prop holds no initiative slot, takes no
  hit roll, pays no XP and touches no morale: it is struck through
  `Board::damage_prop` in `tactical/prop_fight.rs` rather than seated beside
  the creatures, the spec's rejected 'inert body' design, which would have
  needed every `bodies()` reader taught to skip it and every payout path
  taught not to pay it. What a destroyed prop does to a *body* still goes
  through `Game::apply_damage`, the one damage door. Because a prop can hide,
  block or cover without being a cell kind, `Board::cell` no longer answers
  whether a cell blocks sight, is cover or can be walked: reach, sight and
  cover read `Board::blocks_sight`, `Board::is_cover`, `walkable` and
  `move_cost`, which fold props in. A reader that calls `Board::cell` and
  matches on `BattleCell` sees the terrain under the prop and silently misses
  it (the `reach.rs` cover doc still said `BattleCell::Cover` after the move).

- **Prop damage takes no RNG draw: a swing is the plain figure less armour, a
  routine's is the mean of its band.** `tactical_attack_prop` deals
  `swing_damage` less the prop's armour (at least 1) and `routine_prop_damage`
  the rounded mean of the scaled band plus attack, so a prop in a blast or
  beside a swing never moves the seeded stream. Rolling would shift every
  later draw in a seeded fight (see the RNG-stream-shift memories), and the
  AI's argmaxes over props must stay draw-free like its other aim choices. A
  blast's damage to bodies is likewise the blast's full figure and rolls
  nothing, which is what lets `blast_net_value` be exact.

- **A party-side body vetoes a detonation that would hit its own side, while a
  hostile only subtracts.** `blast_net_value` returns `None` when the actor
  fights for the party and any body of the party (the actor included) stands
  in the blast, matching `spares_own_side` in `ai.rs`' routine aim. Net
  scoring alone fired a blast through a companion whenever two hostiles stood
  beside it, which is a party AI trading its own side's Integrity for the
  other's. Hostiles keep the subtraction: friendly fire is full and a wild
  body that catches its own packmate in a profitable blast is the intended
  behaviour. Both sides compute the same `blast_reach` as the blast itself, so
  what is scored is what lands, and that reach skips bodies already at 0
  Integrity so a chain does not hit a fallen-but-unreaped body twice.

- **The prop chain cap counts blasts, so the last prop in a chain stays
  standing.** `TACTICAL_PROP_CHAIN_MAX` bounds how many blasts a chain may set
  off. `detonate` therefore does not strike a volatile prop once `depth + 1 >=
  TACTICAL_PROP_CHAIN_MAX`; non-volatile props are still broken. The earlier
  form returned early at the cap *after* the prop was destroyed, so the last
  prop in a line was removed, logged 'detonates' and queued a volatile
  destruction cue while dealing no damage: a dud, and a blast count one higher
  than the cap said. A line of six volatile props shows exactly the cap's
  worth of blasts and leaves the rest standing; the chain test asserts the cue
  count as well as what stands.
