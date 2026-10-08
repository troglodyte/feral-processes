# Seams: Combat: battle flow, rewards, arena, rest & pursuit

- **A fight is bounded by bodies, not just by groups.** `MAX_PACK_BODIES`
  is the ceiling on the whole pack, trimmed off the largest group each pass
  in `group_pack` — the two ceilings before it bounded a fight per group and
  per group count and never their **product**. **Two things hid it.** The
  surface never reaches the product, while `stack_encounter_pack` **fills**
  it by construction — one species pick and one full group roll per group
  slot the curve allows. And `balance_sim` has no Stack term and projects
  **one** group for surface clearability, so neither side is gated. The
  turned-away bodies **stay on the map** and are met on the next bump,
  `MAX_ENEMY_GROUPS`' rule. **The trap is that the species window is a
  threshold, not a gradient**: one step back or two changes nothing and three
  is a cliff, so this is not tuned by nudging `TIER_ENTRY_STEPS`.

- **`start_battle` is the only path that caps a pack; `begin_battle` opens
  one.** The split exists for `arena`, which authors its own composition. The
  two ceiling helpers are `pub(crate)` solely so the arena can *warn*.

- **There are two battle rosters, and which one a caller wants depends on
  whether it *draws* or *acts*.** `battle_view` is live truth;
  `battle_view_at(revealed)` replays `BattleTimeline`. Anything mapping a
  typed group letter onto `BattleState::groups` takes the **live** one. The
  timeline stores **rendered rows**, frames carry a *line count* rather than
  an index, and a frame is taken at **zero** lines.

- **A won fight says so, and it is the only ending that needed telling.**
  `settle_rewards` heads the results with "You won!", read off
  `BattleState::groups` being empty — telemetry's own definition. A jack-out
  and a flatline are left alone: both already declare themselves one line
  higher, at their own sites. The XP lines take an `Experience:` header and
  `Salvage:`'s indent, and are **built before the header is written** so it
  cannot stand over nothing.

- **A finished fight keeps the battle screen; it does not hand off to a
  summary page.** It reads: the final round's blows, the outcome, the
  salvage, the XP. `BattleTimeline::closing` is captured at the **top** of
  `end_battle`, the only moment a companion that died winning still exists.
  Its hostile half is empty on a win and populated on a jack-out, both
  deliberately. **A tactical fight is the exception, and it is its own
  mode rather than a branch in `draw_battle`.** Handed to
  `Mode::BattleResult` it drew the group model's screen over a fight fought
  on a map, and reusing that mode with a board check
  would still be `is_battle` — the reveal and a swallowed key, for lines the
  player already watched land. `Mode::TacticalResult` lists
  `Game::battle_outcomes` (the prune's own predicate,
  `MessageKind::survives_battle_prune`, read without pruning) over
  `Game::tactical_result_view`. Three things hold it up: the board is
  `TacticalView::frozen` — no actor, no reach — or it draws a turn arrow and
  a reach wash for a fight that is over; `Fx::battle_center`'s nobody-acting
  arm **refreshes** the hold's `seen`, or the hold lapses after
  `CAMERA_DWELL_SECONDS` and the camera swings to mid-board under the popup;
  and the gui test asserts the battle ground's fill, never a body's glyph,
  because the surface map draws the player's `@` too and a glyph check
  passed with no board at all.

- **A battle does not end when the player's HP hits zero**, and three things
  heal them before anyone outside can look. "Did the player win" is read off
  the *opponents*. A level-up full-heals, so an HP fraction sampled after the
  fight reports a hard-won win as free.

- **A fight's rewards are granted per kill and announced once.** Moving the
  *award* to the flush is the change to refuse — a level-up full-heals inside
  `add_xp`. The buffer is a field on `BattleState`, not a `Resource`. The
  flush sits above `dissolve_tamed_program` and ahead of
  `retain_outcomes_since_battle`.

- **`retain_outcomes_since_battle` runs when the player *leaves* the results
  screen, not when the fight ends.** `Game::prune_battle_narration` is the
  door and `App::leave_battle_result` the one caller; run inside `end_battle`
  it deleted the decisive round before anything could reveal it.
  `Mode::BattleResult` and `Mode::TacticalResult` each have one key handler
  and nothing ticks on either, so there are three exits to get right — the
  two keys and `check_game_over`, which names both modes. `keep_battle_narration` is unaffected,
  still `arena`'s alone.

- **There is one way into a staged arena fight, `arena::stage`**, and one
  reader of what one cost, `arena::Watch`. An app-core copy of the outcome
  logic is the copy nobody runs.

- **An arena session touches no disk, and all three of those are omissions**
  — save, profile, run history — each with its own test asserting on the
  *file*. The profile is the one that costs real money if it regresses.

- **Battle telemetry is the fourth thing an arena session touches, and it is
  allowed to write.** `flush_battle_telemetry` sits **above** `after_tick`'s
  `in_arena()` early return. `serde_json` is app-core's dependency and never
  the engine's; `Game::record` takes a closure, not a value; `arena::stage`
  takes the flag as a **parameter**.

- **`nest_aggro_tick` is the first code to call `start_battle` from inside
  `tick_inner`**, which is why `rest`'s tick loop needed a battle check.
  Anything else that starts a fight from a tick inherits the obligation.

- **`nest_aggro_tick` is a reader of the player's `Position` and needs the
  underground guard** even though it never went through `require_surface`. The
  distinction that matters is whether the code drags the player into
  something, not whether it reads `Position`.

- **Resting is priced by locale, never gated by it.** Free inside base space,
  one unit of an `ItemDef::enables_rest` item anywhere else — the open grid
  and the Stack alike — and **no rest advances the clock**, which is what
  makes the free half safe. A ticking free rest farms production and raid
  pressure; a ticking priced rest was the game's only bulk time source.
  `Game::wait` is the only way time passes without an action now. **The trap
  is that `rest` reads as an unguarded `Position` action**, so a
  `require_base` added "back" deletes the field half silently. **The mirror
  trap is app-core's**, and it shipped: `r` was bound on the surface and
  absent from `handle_stack_key`, whose match ends in `_ => {}`, so resting
  underground was a swallowed keypress with no refusal and nothing in the
  log. A key the engine supports everywhere has to be bound in *both*
  dispatches, and only the Stack arms whose behaviour differs are listed in
  the help page's "In the Stack" sublist.

- **A charged rest rolls `REST_AMBUSH_CHANCE` for an interrupt, and the roll
  rides the branch that takes the charge.** That placement — below the
  payment, above every restore — is the whole feature, and all three of its
  properties are consequences of it rather than checks. **The trap is adding
  the locale test it looks like it is missing**: base space is safe because a
  free rest never reaches the roll, so an `in_base` guard *inside* the roll is
  either a no-op or, written the other way round, the thing that makes the
  slab dangerous. The second trap is the refund a reader will want to add —
  the outlet is spent and nothing is restored *on purpose*, since a refund
  makes the risk free and the constant meaningless. The third is that a roll
  which hits but **fields no pack must lapse into an ordinary rest**, or a
  charge burns for no fight at all, which is the one outcome a player cannot
  read as anything but a bug. **This is the first roll site that cannot know
  its locale by construction** — every other spawn path is reached from one
  kind of movement, so `surface_ambush_pack` and `stack_encounter_pack` are
  named as a pair and each states its own placement rules once.

- **A rest repairs the programs standing with the player and nobody else** —
  `InParty` and `Wielded` yes, `Sortie` and `Staff` no. The walk used to be
  over every `Tamed` program the player owned, so a rest four frames down the
  Stack reached back and healed the base. Read off `Game::program_role` and
  never off `Party`: `Staff` is what `role_of` leaves *over*, so a
  hand-written party test also excludes `Wielded`, which is in the player's
  hands. **Exhaustive, `cell_mark`'s rule** — and briefly not: while `Staff`
  was the only exclusion this was `!= Staff`, on the argument that a fifth
  role should inherit the heal because being left out is what strands a
  program. `Sortie` joining retired it. The roles now split two and two on
  whether the program is *with you*, so there is no majority to default to,
  and the role the negative form defaulted *in* was the one that wanted
  defaulting out — a default that got its only real test wrong is an unasked
  question, not a safe side. Power is still refilled for every role: a Bay
  gives Integrity only, so withholding it invents a second dead end.
  **Two gaps follow and neither is this seam's to close.**
  `run_repair_bays` queries `With<Downed>`, which only `bench_or_dissolve`
  inserts and only under Forgiving, so a staff program damaged and still
  standing — every survived sweep's defender, every fresh capture, every
  party member stood down — has no route at all, and on Permadeath the Bays
  serve nobody. And a squad's only restore is now the 15% paid between
  battles, after which it is `Staff` again, so damage accumulates across
  trips until `SORTIE_MIN_HP_FRACTION` refuses the next dispatch with
  nothing at home able to lift the refusal.

- **`power_regen_system` needs that same guard**, and is the third in the
  family. A Recharger within radius of the entrance tile otherwise refills
  the party four frames down, which is the whole of the Stack's Power
  scarcity. Its test asserts both halves in one function — the underground
  half alone passes against a bare `return`.

- **`Pursuing` must only ever be inserted alongside one of the two
  tethers** — `NestGuardian` or `TownPatrol`. An untethered `Pursuing` has no
  leash and is never cleared, so the program chases across the whole zone
  forever.

- **`pursuit_tick` drives both tethers off one field, and that field is
  sized off the *maximum* of the two leashes.** They are equal today
  (`NEST_AGGRO_LEASH_RADIUS`, `SETTLEMENT_PATROL_LEASH_RADIUS`, both 15);
  raising the patrol's past the nest's without the `max` produces patrols
  that read as absent from the field and give up where they stand — a
  mechanic that disappears with no error anywhere. **And dropping an arm
  from the two-arm collection does not stop that kind pursuing**, since the
  step loop queries `With<Pursuing>` alone: it stops one ever being
  *released*. The leash test is therefore the one that matters, and it must
  be built on a town at `Hostile` — against a Neutral one
  `patrol_aggro_tick`'s stand-down drops `Pursuing` for free and the test
  passes with the arm deleted.

- **`walkable()` alone does not decide where a `Pursuing` guardian may step**
  — `pursuit_field` excludes `Biome::Platform` separately.

- **There is one Dijkstra walk on the surface, and the step rule is a cost
  function, not a predicate.** `walk_field`, with `pursuit_field` a one-line
  wrapper. The rule takes **the coordinate as well as the tile**, because
  refusing a tile a `Structure` stands on is entity state. You may step off
  an occupied tile, never onto one. It answers `Option<u32>` — `None`
  refused, `Some(c)` what entering costs — because the tactical battle map's
  `Rough` ground costs two and a predicate cannot say "crossable, but
  dearly"; every surface and base-space caller answers `.then_some(1)` and
  gets the uniform field it always got. `radius` still bounds a Chebyshev
  **box** and not a budget, which is safe only because no step costs less
  than one: a caller spending a budget passes it as the radius and filters
  the result by cost, and an unbounded allowance would be an unbounded
  search.

- **A `NestGuardian`'s tether refuses a step only when it both leaves
  `NEST_TETHER_RADIUS` and fails to close on the nest.** The simpler check
  froze a displaced guardian for the rest of the run.

- **`BattleState::planned` indexes `Party` positionally.** Nothing may leave
  `Party` mid-battle; deferred removal is why `end_battle` exists.

- **An initiative order names the party by slot and the wild side by
  identity**, and that asymmetry is the point. `Party` cannot shrink
  mid-battle, so a slot is safe; the wild side *does* — `remove_member`
  drops a dead member and drops an emptied group — so `battle::Actor::Enemy`
  carries an `Entity`. Positional, it named whoever slid into its place: the
  group behind a fallen one swung **twice** and one whose index moved off
  the end **lost its round in silence**. **A count of swings sees neither**
  — the shift conserves the number of actors, not who they are — so the test
  counts by move name. The group index stayed a `wild_retaliate` parameter,
  but its two real callers read it live off `Game::group_of` at the moment
  the program swings.

- **A charge is cancelled where the stun lands, and its cooldown arms only in
  `end_charge`.** The battle map never reads stun at turn start
  (`tactical/turn.rs`), so a check at the charger's own turn lets a stunned
  charger fire; `arm_status` is the one hook that sees a stun in both models
  and calls `cancel_charge`. Arming the cooldown at the start instead would
  tick it down during the wind-up and leave the routine ready the instant it
  fired, so the start pays Power only and `end_charge` (the sole remover of
  `Charging`, for both firing and cancelling) arms it through the model's own
  helper. A cancelled charge keeps its Power spent and still starts the
  cooldown. A hostile only ever holds a charge routine through the wild roll,
  never its species kit.
