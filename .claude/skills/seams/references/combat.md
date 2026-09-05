# Combat, progression and balance

- **`Game::apply_damage` (`game/combat_damage.rs`) is the only code path that
  *damages* a creature**, every rung of the fumble ladder included. Put a check
  that must see all damage here. `Game::kill_outright` is the one other thing
  that lowers HP — materialising inside rock, which no armour answers — and it
  is spelled as its own verb precisely so it cannot become a general mitigation
  bypass: there is no amount to pass. Both funnel through one private
  `lower_hp`, so the death check cannot be missed by either. `apply_damage`
  **returns what actually landed**, not what was asked for, for the reason
  `restore_hp` does: a log line printing the requested figure claims damage the
  target never took.
- **`PowerReserve`'s float is private, and the clamp is the type's.** Seven
  operations, matching the call sites exactly — an eighth is a signal to
  re-read the call site, not to widen the type. `POWER_MIN`/`POWER_MAX` stay
  in `components.rs`; `ROUTINE_POWER_COST_MULTIPLIER` is the knob and lives
  in `tuning.rs`. `views::PlayerStatus` calls the `Stats::power()` scalar
  `strength`, because it is the one struct carrying both.
- **`ability_unavailable` is the one gate, `spend_power` the one charge**,
  both priced through `abilities::routine_power_cost` so a refusal and a
  charge cannot quote different numbers. Both read the reserve off **the
  entity in question**, which is the whole of how a companion pays for its
  own Special. The two ends are deliberately asymmetric: a missing reserve
  **refuses** at the gate and is a **no-op** at the charge, which is what
  makes hostiles safe without a branch. The charge sits at the
  `BattleAction::Special` site, **not** in `use_ability` — the wielded proc
  and hostile invocations share that function and stay free.
- **Every routine was already priced; the field just reached nothing.** The
  2026-08-17 flip from `fatigue_cost` to `power_cost` renamed 55 keys and
  hoisted 10, with no value authored. The default is **0.0, not 5.0**:
  free-by-default is the only safe default once a field's audience widens to
  every ability.
- **A field buff's lifetime is decided by its kind *and* its source**, and
  `ActiveFieldBuff::runs_until_rest` is the one predicate. A routine-armed
  buff of a read-on-demand kind has no turn count: rest, a Forgiving reboot
  and same-kind displacement are the only things that end it. **The `source`
  half is the load-bearing one** — `ItemEffect::prebattle_buff` arms the same
  struct from a one-shot item, and `field_buff_power_of` *sums* a
  `Consumable` and a `Routine` entry of one kind, so a kind-only rule would
  make `patch_routine` permanent and stack it under Ablative Layer forever.
  `Regen`/`Trickle` are excluded because they are the only kinds with a
  per-tick effect and the only two that use `interval`, whose cadence is
  phased off the very counter an until-rest buff lacks. The drop is a free
  function (`components::drop_until_rest_buffs`) because
  `death_handling_system` has no `Game`, and in `rest` it sits **with the
  heal, not with the gates** — a rest that never happened clears nothing.
  The tag is `"rest"`: the map's status column is unmeasured and unclipped
  horizontally, so `"until rest"` runs off the panel.
- **`Trickle` is the one restore kind that does not scale with its invoker.**
  `Regen`'s ceiling is `max_hp` and grows with level; Power's is a fixed
  `POWER_MAX` forever, so a scaled `power: 1` is 7 a turn at the level cap
  and the authored number stops meaning anything.
- **`balance_sim` gates none of the Power economy** — it models no abilities,
  so the 66 costs, the multiplier and `trickle_charge`'s retune are all
  ungated. The suite proves the mechanism, not the numbers; `dev-arenas/` and
  a session are the instruments.
- **A player's class grants affinities and nothing else, and
  `ability_affinity`'s player arm is where it lands.** No stat block
  (`ClassShape` is a *species'*) and no talent tree (talents are the
  companion's axis, in the creature arm only) — a spread of multipliers over
  the perk term, under the same `AFFINITY_MAX` clamp, plus `ClassDef::kit`.
  Every shipped class damps an axis and there is no Unaligned option, which
  is affordable only because `battle::expected_damage` has **no affinity
  term** and the player's ordinary swing never touches `ability_affinity` at
  all — it is `attack_range` into `resolve_attack`, and the three affinity
  readers are `combat_round.rs`, `game/field.rs` and `game/routines.rs`. So
  classes are ungated by `balance_sim` by construction, and a curve that
  moves after a class change is a bug, not a retune. The instrument is the
  **played** arena; the headless bin runs `PartyPlan::AllAttack` and invokes
  nothing, so it sees the stat pool and not the class. The class is stored
  and the spread re-resolved through `ClassDb` every read, so a retuned class
  file reaches a run in progress, and an empty `assets/classes/` is a
  supported install.
- **Every difficulty curve in the game is linear.** A geometric enemy curve
  racing a linear player curve outruns it wherever you put the coefficients.
  A linear tier step is a *ratio*, so `ZoneLevel::raised_a_tier` applies it
  rather than truncating to 1. `balance_sim` bounds per-zone *steps*, not
  ratios — a ratio bound passes any compounding curve with a small enough
  base.
- **One draw, four bands: `battle::resolve_attack` is how every
  creature-versus-creature attack resolves.** A single `r` decides crit (capped
  at the hit chance), hit, fumble (capped at `1 - hit chance`), miss — one draw
  rather than three, which bounds the RNG-stream shift and makes crit and
  fumble mutually exclusive *by construction*. `hit_chance` is the **ratio**
  form `k*acc / (k*acc + eva)`: scale-free, so a zone that multiplies
  everything changes no hit rate. A difference form is forbidden — it makes
  hit rate depend on absolute scale and deep zones drift to always-hit, and
  so is a flat `+n` on accuracy, which washes out as levels grow. `k` is
  `ATTACKER_ACCURACY_ADVANTAGE`, and it is the **only** thing that moves the
  parity baseline off 0.5 — necessarily symmetric, because this is a pure
  function of two numbers and cannot know which side is the player. Draw
  counts are pinned per outcome; the Opening rung's free swing **must not
  itself fumble**, or one bad roll chains into an unbounded exchange.
  `Game::attack_nest` is deliberately outside all of it: a structure has no
  speed and cannot dodge.
- **Flat Accuracy has one door per axis, and the two axes are not the same
  one.** `Game::accuracy_bonus` is what an *entity* brings to every swing it
  makes — gear, plus `Perk::TargetLock` for the player, plus
  `TalentNode::Accuracy` for a companion, split on identity against
  `player_entity()` so perks and talents can never stack. `battle::Swing`
  carries what an *invocation* brings, which is `AbilityDef::accuracy` and
  nothing else: `resolve_and_apply_attack` builds the defender's profile from
  `Swing::plain`, or an Opening rung's free counter would be aimed by the
  routine it is countering. Accuracy is **derived, never stored** and has no
  `Stats` field, so a fourth source must be an addend here rather than a
  bake. **The trap is enumerating `EquipmentStats`' fields by hand**: both
  emptiness arms in `AffixDef::fault` named three of six, so an affix paying
  only accuracy, evasion or damage was refused at load as granting nothing —
  `is_empty` and `has_upside` now **destructure**, on `cell_mark`'s rule.
- **Mitigation is percentage points, and `Game::effective_mitigation` is the
  one door.** It caps at `MAX_MITIGATION_PERCENT` itself, so no reader has to.
  **The trap is that `Stats::mitigation` already carries gear** —
  `apply_equipment_delta` bakes it in — so adding `gear_bonus` there
  double-counts every worn piece. It is **never scaled by level or zone**: a
  percentage that grows per level approaches immunity, which is why
  `stats_after_levels`, the wild spawner, `ZoneLevel::raised_a_tier` and
  `refactor::refactored` all leave it alone. Levelling's defensive growth is
  **evasion** instead. `Stats::power` prices it as the effective HP it buys,
  `max_hp / (1 - mitigation/100)`, so the cap is load-bearing there too —
  it is what keeps the denominator off zero.
- **A kill's XP is priced by challenge, sharing its thresholds with the con
  colour.** `progression::kill_xp`, clamped to `XP_CHALLENGE_FLOOR`..`CEIL`.
  The denominator is the player's power **alone** — counting the party would
  dock XP for recruiting. Both clamps are load-bearing in opposite directions.
- **Levels come at half the count and twice the size, and that is
  power-neutral by construction.** Every per-level constant carries `K = 2`,
  every levels-per constant its reciprocal, and `XP_PER_LEVEL_STEP` carries
  `K^2`. Species ability unlock levels are in the same currency and live in
  the assets. `PLAYER_BASE_STATS` is an offset, not a rate — do not sweep it in.
- **The ring buys room; the fights buy the points.** A Privilege Ring (a lair
  guardian's drop, and nothing else's) opens a Kernel Ring on one companion,
  and `open_kernel_ring` grants no stats, level or XP.
- **A Kernel Ring buys talent tiers, not levels.** `talent_points`' `earned`
  is `min(level - TALENT_START_LEVEL, rings * LEVELS_PER_RING)` — both gates
  live, `saturating_sub` because a companion below the start level is the
  common case. Its log line no longer promises a level ceiling — an unpinned
  player-facing claim that would have read as correct forever, so it has a
  test.
- **Talent points are derived, never stored.** Level minus
  `TALENT_START_LEVEL`, minus the length of `components::Talents`; no count on
  the component, none in the save. Which tier is next is that same length, so
  there is no cursor to keep in step. **`take_talent` writes the receipt
  *before* applying the node**, because `install_unlocked_routines` reads the
  list back — every refusal is already behind that line.
- **A `Stat` talent bakes into `Stats` at purchase and load must not re-apply
  it** — `CreatureSave` already writes the raised numbers, so `Talents` is a
  receipt exactly as `Refactors` is. It goes through `refactor::raised` for its
  whole-point floor, with gear lifted around the write. The other three kinds
  are read on demand at one seam each: `RoutineSlot` in `routine_slots`'
  companion arm, `Affinity` in `ability_affinity`'s creature arm (clamped), and
  `Ability` folded into the `declared`/`reached` lists both install paths
  already build — never a second install path.
- **A stat a purchase baked in needs a receipt, and `components::BoughtStats`
  is it** — `Perk::Buffer` and `TalentNode::Stat` both read the value at
  purchase and both floor at a whole point, so the mapping is many-to-one and
  a respec cannot invert it; inverting would also read *today's* constants, so
  a retune would change what an old save's respec hands back. The grant is
  recorded in the same branch that writes `Stats`, from the same value —
  `purchase_stat_gain` computed twice is `balance_sim.rs`'s drift again.
  `unbake_bought_stats` is the one subtractor and lifts gear around the write.
  **The trap is a fifth writer**: a new stat-granting perk or `TalentNode`
  kind that forgets the receipt compiles clean, works, and makes refunds
  quietly under-pay, surfacing as "respec is buggy" nowhere near the cause.
- **`ever_bought` is the half a respec must not reset** — `convert_overflow_xp`
  prices a minted Perk Point off it, and that escalator is the only bound on
  banked cap XP (`OVERFLOW_XP_STEP`: zero is not safe). Read off
  `Perks::unlocked`, a wipe empties the list and resets the price to the
  opening rate. Counted for **every** perk, not just the three that move a
  stat; the load seed is `max(saved, unlocked_perks.len())`, because the bare
  length is wrong for any save written *after* a respec.
- **Fusion keeps the dominant parent's ring and talents**, and
  `fuse_companions` is the door that silently drops a new component: it
  hand-writes its own list, so nothing fails to compile and the symptom reads
  as fusion being bad. Both parents' would launder two developed programs into
  one.
- **`Experience::xp_to_next` is derived on load and never read back from the
  save**; both load paths call `xp_for_level`. The field stays *written*,
  because removing one is what earns a `SAVE_FORMAT_VERSION` bump.
- **Distance from home is a difficulty axis again, capped at one zone step.**
  `Game::distance_from_danger_origin` feeds `in_opening_ring` and
  `Game::field_stat_mult`, the field ramp: 1.0 out to `OPENING_RING_TILES`,
  then linear to exactly the next zone's doorstep `DANGER_RAMP_TILES` beyond
  it. It fed a 3x stat multiplier and the group curves until 2026-08-05, and
  **the two bugs that removed it are closed by the shape rather than by a
  check** — restore either and the shape is what you have broken. *The
  underground leak*: every Stack spawn is placed at the surface **entrance
  tile**, so a distance term read inside the spawner scales a whole frame by
  how far out its link sits. The ramp is therefore computed by the *caller*
  and handed in through `SpawnEscalation::stat_mult` — the field
  `stack_depth_multiplier` already fills underground — and `stack_escalation`
  builds its own struct and never reaches the ramp. There is no `(x, y)`-
  derived term inside `spawn_wild_creature_scaled`, and that is the invariant
  `a_spawns_stats_come_from_its_escalation_and_never_from_its_tile` pins. *A
  zone with no difficulty of its own*: the cap is exactly `ZONE_STAT_STEP`,
  so a zone spans `[N, N+1]` and its floor is the previous zone's ceiling.
  Expressing it as a **ratio on the zone curve** rather than a curve of its
  own is what leaves `balance_sim` gating it for free — the far field of zone
  N *is* the zone N+1 fixture it already sweeps — which is the answer to the
  standing objection that the old multiplier was ungated. The ramp's floor is
  the ring, not zero: `beatable_by_a_fresh_player` is computed against the
  unscaled species. **`danger_steps` deliberately gained no distance term** —
  it is the one input both group curves and the species window read, and
  `TIER_ENTRY_STEPS` is 2, so distance would need two full steps to change
  what you meet and four to open apex bosses, which is the shape that
  collapses the zone ladder. Distance moves how hard a spawn is; zone and
  depth still decide what it is and how many. **Who gets the ramp is a census
  of `Game::field_escalation`'s callers**, which is why it is a second
  constructor and `SpawnEscalation::surface()` still means no escalation at
  all: `arena::encounter` must not move with a map coordinate, and
  `game::sortie` already prices its own risk through `habitat_pools`'
  `step_bonus`.
- **A basic attack is an `AbilityDef`, and combat names `MoveDef` nowhere.**
  `species::basic_attack_ability` is the one conversion; `moves:` stays the
  authored shape so no species file or mod needed editing, and
  `SpeciesDef::basic_attacks()` is what the readers take. Converted attacks
  are `power_cost: 0`, `cooldown: 0`, `wild_weight: 0` — a fallback that can
  be on cooldown leaves a hostile with no action, and filler attacks must
  stay out of the Routine Disk pool. **Both arms roll through
  `resolve_attack`, and what still differs is where the band comes from**: a
  basic attack takes the wielder's weapon band through `Game::attack_range`
  (a weapon **overrides** a natural attack rather than adding to it, keyed on
  the weapon carrying a band and not on the slot being filled), while a
  Special takes the ability's own, put through `abilities::scaled_range`.
  Both ends of a band scale, or a high-level ability collapses to a point.
  `ranged` sits on `AbilityDef` but is read **only** by the basic-attack
  path; honouring it in `use_ability` stops back-row hostiles running what
  they run today.

- **Every routine that moves Integrity rolls a band, and the census is what
  keeps it that way.** `spread` on `Damage`/`Drain`/`Heal`, rolled through
  `battle::DamageRange` — one draw whatever the width, so authoring a spread
  cannot shift a seeded stream. The serde default has to stay 0 for a mod's
  file to parse untouched, which is exactly why the shipped roster needs
  `every_shipped_integrity_routine_rolls_a_band`. The band is **centred**, so
  `DamageRange::mean` — and every `balance_sim` curve — does not move.
- **`Game::choose_wild_action` is the one place a wild program's swing is
  decided** — move and target as a single joint choice. The score is
  `ln(slot_aggro_weight) + w·features`; the `ln` makes an all-zero policy
  reproduce today's distribution *exactly*. The aggro table enters with a
  pinned coefficient of 1.0 and is **never** a learned feature.
- **Three policy features are pinned to zero in the shipped weights**, and
  that is a design boundary. Left free, training learns to kill the player and
  ignore the party, then to dodge the brace by reading its DEF.
  `bracing_still_draws_more_fire_under_the_shipped_weights` is what a retrain
  fails rather than shipping past. Deleting
  `assets/policies/enemy_battle.ron` restores the pre-policy game exactly.
- **`is_boss` marks an *apex* species — always a boss, never engine-scaled —
  while any species can be *rolled* into one** and takes `BOSS_STAT_MULT`
  instead. `Game::is_boss_creature` is the one door; `components::Boss` is the
  per-entity half, written at every boss spawn and saved. A boss spawns as its
  own group, and past zone 1 brings an escort from its own tile's
  `habitat_pools`. Neither kind rolls a rare tier.
- **A species' danger band is derived and gates where it may spawn.**
  `SpeciesDef::danger_band` off `growth_multiplier`, against a window in
  `tuning.rs` (`TIER_ENTRY_STEPS`, `TIER_WINDOW_STEPS`, `APEX_ENTRY_STEP`)
  read at `Game::danger_steps` — the zone step on the surface, and the zone
  step **plus** the depth step underground. `habitat_pools` takes `depth` as a
  **parameter** for the same reason `SpawnEscalation` does: ambient surface
  spawns keep rolling while the party is underground. The top band and apex
  **never exit**, or an unbounded step empties the world. The per-biome
  fallback (nearest band, ties upward) is load-bearing at both ends of the
  shipped roster — StaticField ships no band-0 species and OpenGrid no band-2
  — and never reaches for an apex.
- **Which side of the ground a boss dies on decides what it pays**, and
  underground is the game's **only** source of Portal Fragments. The gate is
  `Game::is_boss_creature` **and** underground. `surface_boss_loot` derives
  its band from `ItemDef::value`, giving that field a second meaning.
  `pick_lair_species` draws from the biome's apex pool **ungated by
  `APEX_ENTRY_STEP`** — stacks run 2-6 frames, so a windowed draw put a
  hand-authored apex out of reach of every lair shallower than depth 5. The
  step gate still holds for ordinary and ambush boss rolls, and the ordinary
  window stays the fallback, marked a boss too, so a biome shipping no apex
  does not strand a stack.
- **Trace's group-size lever is a `spawn_pack` parameter, never a resource
  read inside it** — surface spawns keep rolling while the party is
  underground. It is clamped back under `zone_group_cap`.
- **`Game::adopt_program` is the one way a program joins the roster without
  being beaten in a fight.** Two callers with opposite premises agree on what
  *becoming* a companion means. It deliberately omits `StackSpawn`, XP and the
  `Party` push, and neither caller checks `pet_capacity` inside it.
- **There are four doors into the roster and `Game::roster_parts()` is the
  only barrier** — `grant_starting_program`, a capture, `adopt_program`, and
  `fuse_companions`, which assembles its own component list. Nothing fails to
  compile when a component is missing from one of four hand-written tuples,
  so a fused companion unable to run reads as fusion being bad. Test
  fixtures go through it too.
- **Destroying a tamed program has two paths.** `dissolve_tamed_program`
  handles four cases; `fuse_companions` does its own `retain`/`despawn` and
  skips the detachment logging. Know which you are extending before adding a
  third.
- **No stats operation may run while a gear bonus is sitting in `Stats`.**
  Three operations would scale or bank it, welding the difference permanently
  into base stats. `Game::gear_bonus` and `Game::strip_gear` are the shared
  definitions; the four sites each take a different shape for a stated reason,
  and `fuse_companions` strips **before** the snapshot.
- **The wielded program's bonus is computed live**, so destroying the program
  ends the wield by omission — the regression to head off is a later "fix"
  adding an explicit clear to both destruction paths.
- **The wielded program's proc runs as the *program*, not the player**, so
  which program you wield is what the feature is worth.
  `wieldable_routines` excludes `field_only` and `Decompile`. The `W` key is
  an easter egg and a test holds the help text to never naming it.
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
  deliberately.
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
  `Mode::BattleResult` has one key handler and nothing ticks there, so there
  are exactly two exits to get right. `keep_battle_narration` is unaffected,
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
- **`Pursuing` must only ever be inserted alongside `NestGuardian`** — an
  untethered `Pursuing` has no leash and is never cleared.
- **`walkable()` alone does not decide where a `Pursuing` guardian may step**
  — `pursuit_field` excludes `Biome::Platform` separately.
- **There is one Dijkstra walk on the surface, and the step rule is a
  parameter.** `walk_field`, with `pursuit_field` a one-line wrapper. The
  predicate takes **the coordinate as well as the tile**, because refusing a
  tile a `Structure` stands on is entity state. You may step off an occupied
  tile, never onto one.
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
- **A profile pays at `Game::new` and never at `Game::load`, and the
  enforcement is an omission.** `install_profile` says what has been earned
  and both paths call it; `grant_profile_rewards` pays, and only the new-game
  path calls it. Paying on load doubles the bonuses invisibly on every reload.
- **`resources::RunFeats` is a per-tick drain queue and is not saved**, with
  two fields and two drainers, one each. The systems are registered
  **unchained** on the grounds that they share no mutable state; one shared
  queue would silently make that false.
- **`ActiveContract` stores the whole resolved `ContractDef`, not an id**, so
  a contract file edited or deleted mid-run cannot strand or rewrite one
  already accepted.
- **Contracts deliberately amend "progression is earned by fighting."** XP is
  a legal reward on *any* objective; anyone "restoring" the old invariant by
  gating XP behind combat is undoing the feature. **Portal Fragments are still
  earned only by fighting and descending** — `Reward::PortalFragments` is
  absent rather than unused, and a census refuses the back door.

- **`Game::level_cap` is the only ceiling in the game and it takes no
  entity** — player and every companion stop at the same zone-derived number,
  `max(ZONE_LEVEL_CAP_FLOOR, 1 + ZONE_LEVEL_CAP_STEP * (zone - 1))`.
  `tuning::zone_level_cap` is the formula and `Game::level_cap` a call to it,
  since a bevy system holding a `ZoneLevel` needs the same answer. It reads
  `ZoneLevel` and **nothing else**: a depth term added to "help" a deep stack
  is what `depth_does_not_lift_the_zone_level_cap` exists to refuse.
- **The cap's constants are fitted against `balance_sim` and the lower bound
  is a correctness bound.** The cap must sit at or above the *geared* clear
  requirement — below it a fully-equipped party cannot clear the zone at any
  level it may reach, which is a dead run, not difficulty. `STEP = 11` is the
  smallest slope with that property out to zone 16. **No line satisfies the
  upper bound too** — zones 2-6 stay grind-clearable by at most 6 levels,
  which is what `GRIND_TOLERANCE_LEVELS` measures and never a slack to widen.
- **Two renames are load-bearing and neither would fail to compile.**
  `TALENT_START_LEVEL` (was `CREATURE_MAX_LEVEL`) is no longer a cap, only the
  level talents begin at; `arena_level_ceiling()` (was
  `absolute_companion_level_cap()`) is the **arena's** ceiling alone, and
  pointing `arena::set_level` at the zone cap silently clamps the five shipped
  scenarios authoring `level: 12`. `WORK_XP_LEVEL_CAP` is a third, lower gate
  beside the cap — it is what stops a developed program being ground up at a
  Mining Node, and unifying it with the cap deletes that property.
- **XP at the cap is banked, not discarded, and banking and taxing share the
  one accumulator.** `add_xp` accumulates into `Experience::xp` and reports
  `LevelGain::overflow`, staying pure — it reports, the caller spends.
  `convert_overflow_xp` drains it to Perk Points at
  `OVERFLOW_XP_BASE + OVERFLOW_XP_STEP * perks_held`, re-read per point.
  **A flat price is not a safe default**: perks are uncapped and repeatable
  and write into `Stats`, so flat makes overflow an unbounded linear power
  source. Only the player converts — a companion has no `Perks`, so it is an
  omission rather than a check. Whatever is unconverted becomes real levels on
  the next breach, which is why this needed no save field.
