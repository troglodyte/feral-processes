# Seams: Combat: kit, Power, routines & abilities

- **`Game::kit_of` is the one answer to where a body's kit comes from**, and
  every reader of a kit figure is an exhaustive `match` on `Kit` with no `_`
  arm. The trap is a reader that branches on `player_entity()` or on
  `Creature` directly for a kit figure — attack band, reach, affinity, basic
  attacks: it compiles clean and silently ignores the next kit source
  (emulation, todo #100), so the player emulating a ranged species still
  swings at arm's length in that one place. `Kit::Unarmed` is the player *and*
  a body whose species no longer resolves; a reader that must tell those two
  apart guards the `Unarmed` arm on the player, never the whole match.
  Who a body *is* — `routine_slots`, perks, talents — is not a kit figure and
  stays keyed on player or companion.

- **Only the player emulates, and `ability_unavailable` is where every other
  body is refused.** The trap is a *second* place that excludes `Emulate` for
  a non-player chooser instead of trusting that one gate — the final review
  found four of them (`choose_summon_action`, `swing_for_the_squad`,
  `wieldable_routines`, `wild_routine_ready`), and two were already dead:
  `choose_summon_action` and `swing_for_the_squad` both already filter on
  `self.ability_unavailable(actor, def).is_none()` as their last step, so a
  `!matches!(.., AbilityEffect::Emulate {..})` filter beside it never fires —
  it can be deleted with nothing lost. The other two are load-bearing and
  must stay: `wieldable_routines` feeds a wielded program's proc, which goes
  straight to `use_ability` and never calls `ability_unavailable` at all (the
  proc is free, priced at nothing). Today a proc Emulate would be a no-op —
  no `PendingEmulateImage` is set on that path, so the arm `continue`s — but
  that is an accident of the threading, not a guard, so the filter stays;
  `wild_routine_ready` feeds `wild_retaliate`, which arms the cooldown and
  runs the routine directly, also with no `ability_unavailable` call —
  hostiles hold no `PowerReserve` by design and are routed around that gate
  on purpose. **The test before deleting one of these filters**: does the
  chooser's own selection loop already call `self.ability_unavailable(actor,
  def)` before acting on the pick? If yes, a same-purpose `!matches!` filter
  beside it is dead. If no, it is the only thing stopping the choice and must
  stay, with a comment saying so. `install_disk` closes the write side too —
  the Emulate disk can't be installed onto anyone but the player — and
  `unemulate` clears every `Party` body at teardown, not only the player, as
  defence in depth against a mod or future bug handing one to a companion
  some other way. See `seam:only-the-player-emulates` for the argument.

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

- **Every routine that can be run is priced in Power; only a passive is
  exempt.** The default is **0.0, not 5.0** — free-by-default is the only safe
  default once a field's audience widens to every ability, and a mod that
  means to charge says so. The trap is what that costs *shipped* content: the
  2026-08-17 flip from `fatigue_cost` to `power_cost` renamed 55 keys and
  hoisted 10 with no value authored, and a rename cannot carry a value that
  was never there — five ladders had authored no cost at all, so their v1.0
  rungs sat at `0.0` for three weeks while every rung above them stayed
  priced. A `0.0` default makes "priced at nothing" and "never saw this
  field" the same state. Being free is worse than being cheap because a
  cooldown carries a battle and cannot carry the map: there are no rounds out
  there, so Power is a field invocation's *whole* price, which is why
  `AbilityDef::field_runnable` refuses an unpriced heal rather than offering
  one. `every_runnable_routine_is_priced_in_power` holds the shipped files to
  it with **no exceptions list** — the one carve-out is structural, a passive
  being skipped because it is never invoked. Price a new v1.0 rung off its
  own ladder's v2.0, and never reach for an exception: `decompile` pays a
  token 1.0 rather than being excepted, because it is the one routine with no
  cooldown and a reserve is all that ever refuses it.

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

- **`field_only` means never-in-battle; `field_runnable` means offered on the
  map.** One predicate answered both until a `Heal` became the first effect
  that runs in both places. **The trap is widening `field_only` instead**: it
  is read by four battle-side filters (`battle_special_options`,
  `wild_routine_ready`, `wieldable_routines`, `sortie.rs`'s
  `swing_for_the_squad`) that all mean "never in a fight", so widening deletes
  heals from every Special menu — and it breaks the load, because
  `passive_field_mismatch` **refuses** a `triggers` on a field-only effect
  (`hot_spare`) and `field_only_dead_fields` **warns** on a `cooldown` (all
  nine shipped heals). `field_runnable` is on `AbilityDef` rather than
  `AbilityEffect` because two thirds of the rule are not in the effect —
  `is_passive` reads `triggers`, the price gate reads `power_cost` — and
  because `Game::routine_detail`'s "when" line reads it too, so the inspect
  page cannot promise a row the list will not show. **A `Heal` reaches the map
  only if it is priced and ally-facing.** A cooldown counts battle rounds and
  the map has none, so Power is the only throttle out there and a free heal
  has none — `hot_patch` is free and `scaled_range` grows its band with the
  invoker's level, so it would be unlimited repair at every level;
  `no_shipped_field_runnable_routine_runs_for_free` is the census. The
  ally-facing half is what keeps `Game::field_recipients`' `unreachable!`
  unreachable: a `FieldBuff` is held to two targets by
  `field_buff_target_mismatch` at load, a `Heal` is not, so
  `AbilityTarget::is_ally_facing` is the gate and is exhaustive on
  `cell_mark`'s rule. **What a field heal does is `use_ability`'s, not a copy**
  — same band, same invoker scaling, same "log what `restore_hp` returned"
  rule; `run_field_heal` owns the price, the refusals and the tick alone. Its
  one addition is that an invocation landing only on full bars is refused above
  the charge: in a fight a wasted turn is a real choice and the round advances
  anyway, but out here declining is free.

- **Every routine that moves Integrity rolls a band, and the census is what
  keeps it that way.** `spread` on `Damage`/`Drain`/`Heal`, rolled through
  `battle::DamageRange` — one draw whatever the width, so authoring a spread
  cannot shift a seeded stream. The serde default has to stay 0 for a mod's
  file to parse untouched, which is exactly why the shipped roster needs
  `every_shipped_integrity_routine_rolls_a_band`. The band is **centred**, so
  `DamageRange::mean` — and every `balance_sim` curve — does not move.

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

- **`arm_status` is the sole writer of a body's `StatusEffects`, and
  `restore_hp` the sole heal path.** Besides Cleanse and
  `clear_battle_status_effects`, nothing else touches the list: `arm_status`
  is where the id resolves, `status_resist` scales the duration and the def's
  stacking applies, and a second writer skips one of them or sets
  `landed_this_round` wrong. `HealBlock` is read only inside `restore_hp`, so
  a heal or drain that edits `Stats.hp` directly walks around Locked without
  a test failing. Full argument: graph `seam:status-effects-arm-status-is-the-sole-writer-and-restore-hp-the-heal-chokepoint`.
