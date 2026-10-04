# Seams: Combat: tactical battles - turn flow, movement & targeting

- **A battle map's coordinates live in `TacticalBattle`; `Position` is never
  written.** The third space to settle this way, after the Stack's
  `resources::Locale` and base space's `Locale::Base`: a body in a fight keeps
  the `Position` it had when the fight opened, which is what lets teardown be
  a matter of dropping a resource rather than a restore pass on each of a
  fight's five endings. The trap is the *convenience* — the board is a grid of
  cells and so is the world map, so writing a battle cell into `Position`
  makes every existing drawing, targeting and pathing routine work at once,
  and moves the creature to a real tile out in the zone that a badly-ended
  fight then leaves it standing on. Nothing in the compiler holds this:
  `tactical/` does not import `crate::components::Position`, and that omission
  is the whole enforcement. See `seam:a-battle-maps-coordinates-live-in-tacticalbattle-position` for the argument.

- **A body is a wall in `reach::movement_field`, and a body's allowance is
  both its budget and `walk_field`'s search box.** One occupancy rule rather
  than a pass-through set and a destination set — an occupied cell is neither
  crossed nor stopped on, friend or foe — because bodies that can be walked
  through cannot hold a line, and this model has no other zone of control.
  The budget is spent in *cost*, so four points is four open cells or two
  rough ones, and it is handed to `walk_field` as the radius as well: that
  is sound only because no step costs less than one, so nothing outside a
  box of half-width `allowance` can be inside a budget of `allowance`. The
  trap is the corollary — `TACTICAL_MOVE_MIN` and `TACTICAL_MOVE_MAX` read
  as taste and are not. An unbounded allowance is an unbounded search, and a
  body that cannot move at all can neither close nor walk off the board, so
  an authored `SpeciesDef::movement` is clamped exactly as a derived figure
  is. See `seam:a-body-is-a-wall-and-the-allowance-is-both-the-budget-and` for the argument.

- **A fight ends through `Game::finish_fight`, and a `FightVerdict` is what
  each model answers it with.** `end_battle` read `BattleState` six times,
  four of them asking "is anything hostile left" in different words, and
  `mark_nemeses`' own comment says the three must agree. The verdict is four
  fields; `finish_fight` is the sequence, and **the sequence is the thing
  being protected** — every step of it is ordered against another
  (`settle_rewards` before the closing capture, the capture before the reap,
  `mark_nemeses` in a narrow window above the resource removal, the lair
  collapse last). A second teardown beside it drifts a line at a time and
  nothing fails to compile. The trap is `won`: it means the roster was
  *emptied*, never "nothing is alive" — a jack-out taken in the round that
  flatlined the last hostile, before anything reaped it, reads as a win off
  the latter. Two smaller doors come with it: `Game::fight_rewards_mut` (the
  tally is a field on *each* model's resource, because a new `Resource`
  shifts bevy's query order under unrelated tests) and
  `Game::finish_hostile` (the kill line, XP, loot, nest respawn, patrol
  standing charge, despawn — a copy of it is a second place a patrol kill
  stops charging a town). See `seam:a-fight-ends-through-one-function-and-a-fightverdict-is` for the argument.

- **A tactical fight's initiative is rolled once and kept in step by
  deletion, and the cursor names a body rather than a position.** Rolled
  once because the turn-order strip is a planning instrument and a reshuffle
  between rounds makes any plan longer than a turn worthless;
  `Game::initiative_roll` is extracted so both models price a body the same
  way. The trap is the cursor's three removal cases, which are not alike: an
  entry **ahead** of it shifts everything down one and the cursor must
  follow or somebody silently loses a turn; an entry behind it changes
  nothing; and the **acting** body leaving means the cursor already names
  its successor, so the turn must be reset or the dead body's spent movement
  is charged to whoever is next. See `seam:the-turn-order-is-kept-in-step-by-deletion-and-the-cursor` for the argument.

- **A step off the board edge is a departure, not a refusal, and the
  player's own departure closes the fight.** Walking out is the only way to
  express disengaging on a grid, and it is safe only because of the
  `Position` seam above — a body that walks off is standing exactly where
  the fight opened, so there is nothing to restore. Three endings, one win:
  the board clear of hostiles is a win **even when they all broke off**; the
  player down is a loss; the player walking out is the jack-out. Omit the
  third and a fight stays open with nobody holding it. See `seam:the-board-edge-is-a-departure-not-a-wall`
  for the argument.

- **A routine's `shape:` and `range:` are read in tactical fights alone, and
  `AbilityDef::tactical_shape`/`tactical_range` is the one place authored and
  derived are reconciled.** Both are `#[serde(default)]` and **nothing
  shipped authors either**, so the derivation off `AbilityTarget` is what the
  whole roster runs on: one recipient is a `Single` at arm's length, a whole
  side is a blast, and `WholeParty` is the one centred on the invoker
  (derived range 0..0 — its own cell and nowhere else). The trap is reading
  `def.shape` directly: it is `None` for every routine in the game, so a
  reader that skips the door resolves the entire roster to nothing and the
  failure is silent — a routine that quietly becomes single-target reads as
  a nerf rather than a bug. Two censuses in `tests/assets.rs` close it, one
  of them a second `match` pinning the derivation table. Three constants and
  not one for the derived radii, because "one group" and "everything" are
  different sizes and the party's own is the widest. Not to be confused with
  `ranged`, a yes-or-no about the front line in the *group* model.
  See `seam:a-routines-geometry-is-authored-or-derived-and-one-door` for the argument.

- **`AbilityDef::tactical_*` reconciles authored and derived;
  `Game::routine_tactical_*` is the one door that layers research on top, and
  every reader that can see a decompile calls it.** Decompile's range and
  blast radius come from research (`Game::decompile_reach`), which is
  per-player state a def cannot see, so the def's own figure is stale for the
  one effect and still right for every other. A reader left on
  `def.tactical_range()` compiles, then offers an aim the door refuses or
  refuses one the outline draws. The readers are `tactical_use_routine`'s
  refusals, `run_tactical_routine`'s shape and `view.rs`'s two; `ai.rs` is
  left on the def because `ready_from_candidates` never lets a decompile
  reach an AI chooser, so admitting one there moves it. Who a blast captures
  is `capture_targets`, read by the refusal and the resolution alike.
  See `seam:game-routine-tactical-is-the-one-door-that-layers-research-on-a`
  for the argument.

- **`use_ability` is the door the two combat models share; each converts its
  own aim, and full friendly fire is `reach::recipients` never reading
  `Hostile`.** The design named `ability_recipients` as the shared door and
  the code already disagreed — `field_recipients` is a second converter —
  so the tactical one is a sibling rather than an arm, and the
  `SpecialTarget::Cell` variant that arm would need is an invented answer
  every abstract match in three crates would have to reject. The trap is the
  friendly-fire "fix": a side filter in `recipients` is one line, reads as an
  obvious bug fix, breaks nothing that compiles, and deletes the reason a
  shape is worth aiming — the test that holds it places bodies carrying **no
  components at all**. Terrain is read by two shapes of four: a line stops at
  the first cell that blocks sight and a cone drops what it cannot see, a
  blast is stopped by nothing, and `line_of_sight` excludes both endpoints,
  so standing in cover neither blinds a body nor hides it. The aim is a
  *bearing* for a line and a cone and a *destination* for a blast. The cone's
  epsilon is not slop: an eight-way grid's diagonals sit exactly 45 degrees
  off the facing, so a 90-degree wedge holds them only under a comparison
  that admits equality. See `seam:two-combat-models-one-applicator-and-friendly-fire-is-an` for the argument.

- **`Game::decompile_body` is the capture; taking the captured body out of
  the fight is each model's own half.** Everything down to the conversion —
  catalyst, roll, fraying count, XP, component strip, nest respawn — is the
  same act either way; a group index, a rank to promote and an `end_battle`
  are the group model's vocabulary and none of the three exists on a battle
  map. The fraying counter came with it: `Game::decompile_attempts`/`_mut`
  are `fight_rewards_mut`'s counterpart and follow its rule — a field on each
  model's resource, never a resource of its own — so `target_resistance`
  quotes the same count whichever model is holding the fight. A capture is
  aimed at one body and not resolved over an area, and it is the one effect
  in `tactical_use_routine` that does not go through `use_ability` — the same
  exception the group model's own Special site makes. See `seam:capturing-a-program-is-one-function-taking-it-out-of-the` for
  the argument.

- **A routine's effect is shared; its refusals are not — `Game::run_tactical_
  routine`, and `cooldown_floor` is the whole of the difference.** The trap is
  the tidy version: route the enemy AI through `tactical_use_routine`, the
  door the player already uses. `ability_unavailable` reads the reserve off
  the entity asked about and hostiles hold none by design, and every routine
  that can be *run* is priced in Power — so that door refuses a hostile every
  routine there is. Silently: it answers `false`, the AI falls through to a
  swing, the fight still finishes and the suite still passes. The arm ships
  correct-looking and never fires. It takes an `AbilityDef` and not an index,
  because `tactical_use_routine`'s index is into `actor_abilities`, which
  drops ids the `AbilityDb` cannot resolve and so is *not* a position in
  `Routines`. The floor is `abilities::armed_cooldown`'s own parameter: the
  player's routines cool at their authored rate, a hostile's are floored at
  `ENEMY_ROUTINE_MIN_COOLDOWN`. Nothing shipped can observe that floor —
  `field_only_dead_fields` warns about a cooldown on a field-only effect, so
  every shipped `cooldown: 0` routine is field-only and `wild_routine_ready`
  excludes it; the branch guards a mod, and a test that wants it must edit a
  shipped def rather than assert on one. See `seam:a-routines-effect-is-shared-its-refusals-are-not` for the argument.

- **A hostile decides what it will do before it decides where to stand, and
  the closing term is a shortfall to the *band* and never a distance to the
  target.** The obvious order — walk somewhere good, then pick an action —
  cannot express a standoff at all, because what "good ground" means depends
  on the range of the thing being run. And a distance term is monotone: it
  rewards every step toward the enemy, so a carrier standing inside its own
  minimum range is told to walk further in, and the routine it walked in to
  use is the one it can no longer fire. The three terms are read against each
  other, not tuned apart — the reach bonus must outrank closing across the
  whole width of `TACTICAL_BOARD_LARGE`, or a body walks past the swing it
  came for; crowding is the smallest because it is a tie-break between cells
  that both reach. Line of sight is asked only of a cell already in band, so
  cover ranks between "closed" and "can fire" rather than filtering a body
  out. None of it is `combat_policy.rs`: trained weights speak group indices
  and aggro slots, and what replaces a slot here is where a body stands —
  which is also why the swing takes the wounded neighbour rather than
  consulting `battle::slot_aggro_weight`. See `seam:a-hostile-decides-what-it-will-do-before-it-decides-where`.

- **Staying put is the default: a body walks only to a cell that beats its
  own on `cell_merit`, and crowding is never itself a reason to move.**
  `walk_to_best_cell` offered every cell in the movement field to the
  softmax, and a body already in reach tied with every other cell that
  reached — softmax over a tie is a uniform draw, and even the argmax
  (`max_by` keeps the *last* maximum) left for the last tied cell in reading
  order. So a hostile beside the player sidestepped nearly every turn, and
  the crowding term pushed it further, off a packmate. The fix is a
  candidate filter, not a margin on the score: `cell_merit` is the reach
  bonus and the closing term, `cell_score` is that minus crowding, and only
  cells strictly better than the body's own on merit are offered — its own
  cell alone when there are none. **A score margin cannot do it**: one
  closing step is worth `TACTICAL_AI_CLOSING_WEIGHT` and crowding is
  `TACTICAL_AI_CROWDING_WEIGHT` per ally, unbounded in pack size, so no
  margin both lets a body on rough ground close one cell and stops a crowded
  one shuffling. **The hold still spends its draw** (`sample_scored` over a
  one-cell list), so a turn costs one draw whichever way it goes. In band
  but blind is not in reach — `hits` asks sight — so a body behind cover
  still steps around. Neither "an area routine hitting more bodies" nor
  "out of danger" is priced by `cell_merit` (`hits` is `any`, and nothing
  models threat), so neither moves a body; adding either is a new term, not
  a loosening of the filter. Held by `a_hostile_already_in_reach_holds_its_cell`,
  with `a_hostile_out_of_reach_still_closes` and
  `a_hostile_in_range_but_blind_steps_into_sight` failing against an
  always-hold mutation. See `seam:staying-put-is-the-default`.

- **One draw a turn, spent on the cell, and none at temperature zero.** The
  aim and the swing target are argmaxes on purpose: a second draw lets a
  hostile fumble an aim it spent its whole walk earning, which reads as
  stupidity rather than variety. `sample_scored` returns the argmax before it
  touches the RNG at temperature zero, so `tactical_ai_turn_at(0.0)` is both
  pinnable and stream-neutral. The trap is the candidate list —
  `movement_field` answers a `HashMap`, iteration order over one is not stable
  between runs, and two equally-scored cells resolving differently in a seeded
  fight surfaces as an intermittent failure somewhere else entirely, so the
  cells are sorted before they are scored. See `seam:one-draw-a-turn-and-the-cell-is-where-it-is-spent`.

- **A hostile's walk is a run of real steps, one to a beat.** It used to be
  committed as a single placement, argued for on the grounds that nothing on
  the board reacts to a body mid-walk — so a path had no observable difference
  from its endpoint. The hole in that was the player: six cells crossed
  between two frames reads as a teleport, not an approach, in a model whose
  whole mechanic is positioning. Animating it in the renderer instead cannot
  work past the walk, because the action resolves in the same call as the
  placement and the blow would land while the glyph was still sliding. So a
  turn is a run of `AiBeat`s, and `tactical_ai_turn` is **written as that
  loop** rather than beside it, or the arena would measure a different fight
  from the one played. Three traps. The path is descended from the cost field
  (`reach::path_to`) — a predecessor is a neighbour whose cost is this cell's
  less what *entering this cell* cost, which `Rough` ground costing two is why
  it cannot be "minus one". `TacticalBattle::walk` is an `Option<Vec<_>>`
  because **`None` is "has not chosen yet" and `Some(vec![])` is "has
  arrived"**: read as one, a spent walk is re-planned every beat and the one
  draw a turn above becomes one a cell. And the steps go through
  `Game::tactical_step`, the player's own door, so a hostile's step is not a
  second implementation of what a step costs. See `seam:a-hostiles-walk-is-a-run-of-real-steps-one-to-a-beat`.

- **`Game::start_battle` is where the model is chosen, by inspecting the
  pack.** The toggle says a player wants tactical fights; it does not say
  *this* fight is one. **Deciding per call site cannot be made to work**:
  `game/turn.rs`'s pursuit path calls `start_battle` for a `Pursuing` body
  without knowing whether it is a nest guardian (out of scope) or a town
  patrol (in scope), and both arrive at the same line — so the decision has
  to look at what is in the pack. `fights_tactically` is that look: the
  profile toggle, `require_surface` **called** rather than restated, and no
  `NestGuardian` among the bodies. **The fourth gate is an omission** —
  `arena::stage` calls `begin_battle` directly, so it never passes through
  here and can never be routed *by this*; nests, lairs and raids open their
  fights by their own routes for the same reason. A reviewer hunting for the
  arena's exclusion will not find one; what to check instead is that
  `begin_battle` still has exactly two callers. The arena got the model back
  on its own terms a phase later without touching `fights_tactically`, which
  is the next entry. **Nothing above app-core re-derives the gates** —
  `App::opened_battle_mode` reads `Game::in_tactical_battle`, which model
  actually opened, because a second copy drifts on the day a fourth
  encounter kind lands and the symptom is a screen drawing one model over a
  fight fought in the other.

- **The arena chooses its own model, and `stage` takes the one its *caller*
  can drive.** A `stage` that simply honoured `Scenario::model` is wrong in a
  way only a screen shows: the bin resolves a fight by playing rounds, a
  battle map is played a body at a time, and `App::start_arena_fight` ends in
  `self.mode = Mode::Battle` unconditionally — so the played arena would draw
  a battle screen over a fight with no `BattleState` behind it, silently,
  nothing in the types disagreeing. app-core passes `CombatModel::Group`
  always and gets a refusal naming the bin through the `Err` arm it already
  had; `arena::run` passes the file's answer. A parameter rather than a guard
  at each call site because a guard is a thing to forget with nothing failing
  to compile. **Routing the arena through `fights_tactically` instead is the
  tidy version and is wrong**: that reads the *profile* toggle, and an arena
  session touches no profile — a measurement would depend on how the person
  running the bin likes to play. The model stays a **file field** and not a
  bin flag because the builder holds a `Scenario` and writes it whole, so the
  row survives a screen that cannot fight it; a flag lives outside the file
  and is lost by the round trip. **The board's biome is the player's own
  tile**, not an `encounter:` row's — overriding it is a third parameter on
  the door or a second copy of the `BattleSpec` construction, so a rolled
  marsh pack fights on whatever ground the player stands on and the README
  says so. See `seam:the-arena-chooses-its-own-model-and-stage-takes-the-one-its`.

- **A headless tactical rep drives both sides through one AI, and a party
  body swings without invoking.** `tactical_ai_actor`'s gate is `Hostile`
  because every party body is the player's to command, so a fight with nobody
  at the keyboard cannot be resolved through that door at all —
  `tactical_drive_turn` is the second door onto the same `run_tactical_turn`,
  and it has two callers, `arena::run` and `Game::auto_resolve_battle` — the
  second is `[R]`'s battle-map half, and it may drive a companion only
  because the player pressed that key and asked for it, the consent the
  `tactical_ai_actor` gate otherwise stands in for; called from a real fight
  unasked, it would walk a companion by itself. Two traps under it, both
  from asking what a party body does
  inside code written for hostiles. **`tactical_sides` had to become relative
  to the actor**: split by `Hostile` absolutely it hands a party body its own
  side to swing at, and the reproducer is the player alone against one
  hostile — a body that thinks the party is the enemy has nobody to close on,
  ends every turn where it stands, and the fight runs to `ROUND_CAP` for
  ever. For a hostile actor the two readings are the same list, which is why
  no seeded fight moved. And **the swing-only rule is `PartyPlan::AllAttack`
  parity**, not a policy invented for the tester — the group model's own
  arena plan invokes nothing either, so the two models' numbers stay
  comparable, and driving a party body through `wild_routine_ready` would
  have run its routines free besides (a hostile holds no `PowerReserve`,
  which is that picker's whole reason). A `PartyPlan` is **refused** for a
  tactical scenario rather than ignored. The rep loop's own trap:
  `MessageLog::open_round` has one caller and it is the group model's, so
  `since_round()` on a battle map is the whole fight and `Watch::observe`
  would re-record every line every round — it takes from where it stopped,
  keyed on the log generation, which the group model bumps per round so its
  own behaviour is unchanged. See `seam:a-headless-tactical-rep-drives-both-sides-and-a-party-body`.

- **An AI turn hands the turn on once, and the action already did it.** The
  action ends the turn, so `tactical_attack` and `tactical_use_routine` each
  end it themselves once they land — and `tactical_ai_turn_at` ended it
  again at its tail, so a hostile that swung spent **two** rungs of the
  order and skipped whoever came next. Against a lone hostile, the shape
  every wandering encounter has, the order is `[hostile, party]` and the
  hostile took every turn for ever. **The tail cannot simply be deleted**: a
  hostile with nothing in reach swings at nobody and ends no turn, so it is
  owed in some paths and not others — it asks whether the body is still up
  (`actor() == Some(actor)`), which also answers the case the old comment
  reached for, a fight that ended inside the action. **Phase 6's tests could
  not see it**: they assert a hostile closes and swings and that the AI
  declines a party body's turn, and neither asks who acts next — nothing
  headless waits on the answer, so it surfaced the first time a screen did.
  `tactical_ai_actor` is now the one definition of whose turn the AI drives
  and `Game::tactical_awaits_input` its complement, because **every party
  body is the player's to command** — the gate is `Hostile`, not `Player`,
  and two predicates would either hang the fight waiting for a key nobody
  may press or move a companion by itself. **A third one shipped anyway**:
  `TacticalView::player_turn` answered `actor == player`, so on a companion's
  turn the keybar said "the wild side is moving" and the reach wash went
  undrawn while app-core, reading the real door, sat waiting for a key the
  screen had named none of. It is a *call* to `tactical_awaits_input` now.
  The lesson generalises past this seam: a predicate consolidated in the
  engine is only consolidated as far as the view layer, and a view field
  that restates it in different words is the copy that drifts.

- **A turn ends in one place, `Game::hand_on_turn`, and it hands on only if
  the body that acted is still the one acting.** `TacticalBattle::remove`
  already begins the next body's turn when it takes the acting body out —
  the cursor names a body, not a position — so an action that ends the turn
  unconditionally on top of that skips whoever stood behind it. The trap is
  that a body killing itself with its own action looks impossible and is
  not: a fumble's `Recoil` or `Opening` rung damages the swinger, and a
  `Radius` routine catches its own invoker. In an order of `[companion,
  player, hostile]` a companion who fumbles fatally costs the player their
  turn, with nothing on screen to say why. Same question the phase-6 AI fix
  asks at its own level, one level down. See `seam:a-turn-ends-in-one-place-and-a-body-that-killed-itself-has`.

- **A round on a battle map spends the upkeep an abstract round spends, and
  `tick_combatant_upkeep` is the half both models share.** The trap is how
  quiet the omission is: with nothing ticking, every routine is once per
  fight (its refusal counting down "rounds" that never pass), `Stun` never
  wears off, `Bleed` never bites, every authored `duration` lasts the whole
  fight, `ENEMY_ROUTINE_MIN_COOLDOWN` is a floor under a clock that does not
  run, the world stands still for the length of the fight, and a defeat is
  deferred to the next idle tick — the player walks off the board at zero
  Integrity. Not a call to `tick_round_status_effects`, which ends in
  `reap_dead_members` and an `end_battle` that panics without a
  `BattleState`; what to do about what the upkeep killed is each model's own
  half. Two traps under that, both found reviewing the fix rather than the
  feature. The reap belongs **between** the upkeep and the tick, because a
  Bleed is damage and `death_handling_system` rides `Game::tick` gated on
  nothing but `hp <= 0` — tick first and a Forgiving player is rebooted
  inside an open fight, alive again with their world `Position` warped to
  the anchor while they stand on the board. And the order wraps in **two**
  places: `TacticalBattle::remove` calls `wrap()` itself, so a body dying on
  the last rung begins a round no `end_turn` was reached for, which is why
  `hand_on_turn` compares against a round its caller read before it acted.
  A fight that ends mid-round never wraps at all, so `settle_tactical`
  spends that round's tick as it closes. See `seam:a-round-on-a-battle-map-costs-what-a-round-costs`.

- **The results page has two producers — `Game::closing_rows` — and one row
  builder per half.** `BattleTimeline::closing` was filled from
  `battle_rows`, which opens on `BattleState`, so a tactical fight left it
  `None` and `draw_battle` returned before drawing anything: **a blank
  results screen at the end of every tactical fight**, with the win, the
  salvage and the XP written only to the log. The trap in fixing it is the
  obvious shape — a second producer that builds both halves itself — when
  `planned` is the only one of fourteen party fields the models disagree
  about, and a second copy of the other thirteen is a results page that
  disagrees with the fight it reports. `enemy_row` and `party_row` are one
  function each; a battle map passes a body and a count of one, all bodies
  `engaged`, because groups dissolve on a grid. See `seam:the-results-page-has-two-producers-and-the-rows-they-build`.

- **A capture is aimed at something hostile, and the refusal is at the
  player's door with the other five.** Aimed at your own companion,
  `decompile_body` succeeded on a good roll and handed it back through
  `roster_parts` — a **new `ProgramId`**, `Experience::default()`, no
  memories, every other program's memory of it orphaned, and kill XP paid to
  the player for its own companion. It kept its level-15 `Stats` and read as
  level 1. The gate is `Hostile` and nothing else: `tactical_attack` refuses
  only `actor == target`, and a swing at your own is real friendly fire and
  stays legal. Aimed at empty ground the same door refuses, which is what
  keeps a capture from spending its catalyst and the turn on nothing.

- **A cloak is a targeting rule, and the filter sits at the doors that *name*
  a body and at none of the doors that *resolve* against one.**
  `components::Cloaked` moves no stat; the five naming doors are
  `front_of_group`, `roll_enemy_target`, `living_targets`,
  `tactical_attack` and `tactical_sides`. **The split is the whole design** —
  `reach::recipients` never learns the word, so an area routine covering the
  cell still lands (collateral, not aim); the abstract model's
  `WholeEnemyGroup`/`AllEnemies` arms resolve a membership list rather than
  picking a front and keep hitting too; and `movement_field` still treats a
  cloaked body as a wall, so the cell it stands on is the tell. **Its own
  component and not a `BuffKind` or a `StatusKind`**: `CombatBuff` holds one
  wanted buff and bracing overwrites it, so a cloaked body that braced would
  uncloak itself and an ally's Rally would strip a cloak; `StatusEffects` is
  reserved for unwanted conditions, so `Cleanse` would strip your own. Being
  battle-scoped beside those two is what cost **no `SAVE_FORMAT_VERSION` bump
  and no save field**. **The never-empty rule**: where every candidate at a
  door is cloaked the filter is skipped for that pick, because
  `roll_enemy_target`'s `total == 0` fallback returns the *player*
  specifically, `front_of_group` answering `None` refuses the player's
  single-target attacks outright, and an empty `tactical_sides::targets` is
  what `run_tactical_beat` reads as "nothing to fight". `tactical_attack` is
  exempt: it is a pick of one body, not a pool. **`Game::break_cloak` is the
  one door an action removes it through**, three callers each naming a
  different body's — the attacker's in `resolve_and_apply_attack`, the
  actor's in `use_ability` gated on `AbilityEffect::breaks_cloak`, and the
  target's in `apply_damage`. Idempotent and logged on the transition, so a
  `Damage` routine hitting two of the three reveals once. **Everything not on
  that list is an omission and the omissions are the feature** — moving,
  bracing, `Heal`, `Buff`, `Cleanse`. **A miss is asymmetric on purpose**: it
  breaks the attacker's, because committing to a swing is the aggressive act,
  and leaves the defender's, because only landed damage reaches
  `apply_damage` — so a cloak cannot be flushed out by swinging at where you
  guess it is. `breaks_cloak` is exhaustive on `cell_mark`'s rule, with a
  second copy of the table in `tests/assets.rs` so flipping an arm is a
  decision taken twice. **The upkeep had to be extracted first**:
  `tick_combatant_upkeep` was three verbatim copies of the same block, so a
  fourth thing to age meant writing it three more times, and the drift that
  heads off is a cloak that expires for companions and never for the player.
  **Two mutations survived the first test pass and both are the trap for
  anyone re-testing this**: deleting the `tactical_sides` filter leaves
  everything green, because `tactical_attack` already refuses the target — it
  is isolated only by `swing_at_best_neighbour`'s sort, which picks the
  wounded cloaked body every turn and is refused every turn, so the exposed
  body beside it is never swung at; and deleting the attacker hook leaves
  everything green under `player_attacks`, because the round's retaliation
  breaks the player's cloak through `apply_damage` instead. See
  `seam:a-cloak-is-a-targeting-rule-and-the-filter-sits-at-the`.

- **A brace on a board is `Game::begin_defend`, and only the mitigation
  crosses over.** `Game::tactical_defend` is `tactical_attack`'s third
  sibling; the effect needed nothing new, because `effective_mitigation`
  reads `CombatBuff` from inside `Game::apply_damage`, the one door damage
  comes through. Two traps. **The duration is the group model's cadence and
  not the tabletop one**: `remaining: 1` against `hand_on_turn`'s wrap, so a
  brace covers everyone below the bracing body in the order and a body on
  the *last* rung braces against nobody — the wrap fires the moment it hands
  the turn on. "Until your next turn" is fairer and was rejected on what it
  costs a shared component: the buff would have to outlive one upkeep tick
  **and** be cleared at the body's own turn start, and `is_defending`
  identifies a brace by its *power* being exactly `DEFEND_MITIGATION_BONUS`
  and never reads `remaining`, so the two expiry rules would be invisible to
  each other. What the chosen rule costs is legible instead — the turn strip
  names the order and hangs an arrow over whoever is acting. **And the aggro
  half must not be ported**: `DEFEND_AGGRO_WEIGHT` weights an aggro *slot*
  and a board has none, so the port is a term in `swing_at_best_neighbour`'s
  sort — one line, reads as finishing the feature, and wrong. That sort is
  deliberately not `slot_aggro_weight`: reaching a body at all is a battle
  map's answer to who is exposed, and a brace that pulled the swing makes
  bracing a lure, which is a different feature and would have to be read
  against the AI's three cell-scoring terms, which are tuned against each
  other. **The stun gate is a third omission**, `battle_resolve_round`'s
  Defend loop skips a stunned body and this does not — nothing in
  `tactical/` reads stun at all, so gating the brace alone would make
  bracing the one thing a stunned body could not do, which reads as the
  brace being broken rather than as the missing stun handling it is. The
  fixture's own trap: `force_the_next_attack_to_land` cannot pin a tactical
  swing, because `swing_move` rolls the move first and eats the forced roll
  — reseed through `reseed_rng` immediately before the swing instead. See
  `seam:a-brace-on-a-board-is-the-group-models-and-only-the` for the argument.

- **`Game::swing_range` is the one door for how far a body swings, and its
  three readers are calls rather than copies.** All three hard-coded
  `TACTICAL_MELEE_RANGE` before it existed — `tactical_attack`'s gate,
  `Intent::Swing`'s band and `swing_at_best_neighbour`'s target filter — and
  **nothing fails to compile when one drifts**: a body whose band says two
  and whose filter says one walks into position and then passes its turn,
  which reads as the AI being stupid rather than as a bug. Three decisions
  under it. **Holding a weapon at all replaces the species figure**, and a
  weapon that authors no `range` swings at arm's length, so a ranged program
  holding a melee blade is melee — deliberately *not* `attack_range`'s rule,
  which falls through to the natural band, because a weapon with no *damage*
  would leave its wielder disarmed where a weapon with no range simply **is**
  a melee weapon. (The design spec's one-line formula paraphrases it the
  other way round; its prose and the shipped test do not.) It is a property
  of the **body and never of the move it rolls** — the AI picks its intent
  before it walks, so a range read at swing time lets a body plan a standoff
  and then draw the melee half of its pair, which is why `Intent::Swing`
  carries the range on the variant rather than reading it inside `band()`,
  which takes no actor. And it is **read in tactical fights alone**:
  `MoveDef::ranged` keeps its separate group-model meaning untouched.
  `TACTICAL_RANGED_MOVE_RANGE = 2` is load-bearing — fourteen of seventeen
  shipped species carry exactly one `ranged: true` move, so at three nearly
  every wild body becomes a shooter and the closing this model is built on
  stops mattering. `TACTICAL_WEAPON_RANGE_MAX = 3` is refused at load rather
  than clamped: a weapon quietly swinging shorter than its file says reads as
  a nerf. **The sight check is unconditional with no melee branch** —
  `line_of_sight` excludes its endpoints, so for neighbours its loop is empty
  and the check is already a no-op, which is what keeps
  `TACTICAL_MELEE_RANGE` from being restated a fourth time. See
  `seam:game-swing-range-is-the-one-door-for-how-far-a-body-swings`.
