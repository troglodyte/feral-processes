---
paths:
  - "**/combat*"
  - "**/battle*"
  - "**/progression*"
  - "**/balance_sim*"
  - "**/abilities*"
  - "**/level_up*"
  - "**/talents*"
  - "**/taming*"
  - "**/spawning*"
  - "**/party*"
  - "**/kit*"
  - "**/arena*"
  - "**/arena/**"
  - "**/difficulty*"
  - "**/perks*"
  - "**/tuning*"
  - "**/species*"
  - "**/pursuit*"
  - "**/routines*"
  - "**/policy*"
  - "**/field*"
  - "**/contracts*"
  - "assets/abilities/**"
  - "assets/species/**"
---

# Load-bearing seams: Combat, progression and balance

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **`Game::kit_of` is the one answer to where a body's kit comes from**, and
  every reader of a kit figure is an exhaustive match on `Kit`.
- **Only the player emulates, and `ability_unavailable` is the gate for every
  chooser that offers Emulate** — a wielded program's proc and a hostile's
  retaliation call `use_ability` directly and keep their own filter.
- **`Game::apply_damage` (`game/combat_damage.rs`) is the only code path
  that *damages* a creature**, every rung of the fumble ladder included.
- **`PowerReserve`'s float is private, and the clamp is the type's.** Seven
  operations, matching the call sites exactly — an eighth is a signal to
  re-read the call site, not to widen the type.
- **`ability_unavailable` is the one gate, `spend_power` the one charge**,
  both priced through `abilities::routine_power_cost` so a refusal and a
  charge cannot quote different numbers.
- **Every routine that can be *run* is priced in Power; only a passive is
  exempt**, because a passive is never invoked.
- **A field buff's lifetime is decided by its kind *and* its source**, and
  `ActiveFieldBuff::runs_until_rest` is the one predicate.
- **`Trickle` is the one restore kind that does not scale with its
  invoker.** `Regen`'s ceiling is `max_hp` and grows with level; Power's is
  a fixed `POWER_MAX` forever, so a scaled `power: 1` is 7 a turn at the
  level cap and the authored number stops meaning anything.
- **`balance_sim` gates none of the Power economy** — it models no
  abilities, so the 66 costs, the multiplier and `trickle_charge`'s retune
  are all ungated.
- **A player's class grants affinities and nothing else, and
  `ability_affinity`'s player arm is where it lands.**
- **A difficulty band is a fractional zone step, never a flat multiplier on
  stats**, and `Game::zone_curve_ratio` is the one derivation it shares with
  the distance ramp.
- **Every difficulty curve in the game is linear.** A geometric enemy curve
  racing a linear player curve outruns it wherever you put the coefficients.
- **One draw, four bands: `battle::resolve_attack` is how every
  creature-versus-creature attack resolves.**
- **Flat Accuracy has one door per axis, and the two axes are not the same
  one.**
- **Mitigation is percentage points, and `Game::effective_mitigation` is the
  one door.** It caps at `MAX_MITIGATION_PERCENT` itself, so no reader has
  to.
- **A kill's XP is priced by challenge, sharing its thresholds with the con
  colour.** `progression::kill_xp`, clamped to `XP_CHALLENGE_FLOOR`..`CEIL`.
- **How outmatched you are is one duel formula on two sides, `Game::threat_to`
  — the party for the con and the capture odds, the player alone for kill
  XP — and `Stats::power` is not it.**
- **Levels come at half the count and twice the size, and that is
  power-neutral by construction.** Every per-level constant carries `K = 2`,
  every levels-per constant its reciprocal, and `XP_PER_LEVEL_STEP` carries
  `K^2`.
- **The ring buys room; the fights buy the points.** A Privilege Ring (a
  lair guardian's drop, and nothing else's) opens a Kernel Ring on one
  companion, and `open_kernel_ring` grants no stats, level or XP.
- **A Kernel Ring buys talent tiers, not levels.** `talent_points`' `earned`
  is `min(level - TALENT_START_LEVEL, rings * LEVELS_PER_RING)` — both gates
  live, `saturating_sub` because a companion below the start level is the
  common case.
- **Talent points are derived, never stored.** Level minus
  `TALENT_START_LEVEL`, minus the length of `components::Talents`; no count
  on the component, none in the save.
- **A `Stat` talent bakes into `Stats` at purchase and load must not
  re-apply it** — `CreatureSave` already writes the raised numbers, so
  `Talents` is a receipt exactly as `Refactors` is.
- **A stat a purchase baked in needs a receipt, and `components::BoughtStats`
  is it** — `Perk::Buffer` and `TalentNode::Stat` read the value at purchase
  and floor at a whole point, so a respec cannot invert them, and
  `ever_bought` is the half the wipe must not reset.
- **Fusion keeps the dominant parent's ring and talents**, and
  `fuse_companions` is the door that silently drops a new component: it
  hand-writes its own list, so nothing fails to compile and the symptom
  reads as fusion being bad.
- **`Experience::xp_to_next` is derived on load and never read back from the
  save**; both load paths call `xp_for_level`.
- **Distance from home is a difficulty axis again, capped at one zone
  step.** `Game::field_stat_mult` ramps from the opening ring's edge to
  exactly the next zone's doorstep, computed by the caller and never inside
  the spawner.
- **The party is the fifth thing that buys a difficulty step, each spawn
  rolls its own share of it, and it lands on `Game::group_steps` rather than
  `danger_steps`, whose third reader is the species window.**
- **A basic attack is an `AbilityDef`, and combat names `MoveDef` nowhere.**
  `species::basic_attack_ability` is the one conversion; `moves:` stays the
  authored shape so no species file or mod needed editing, and
  `SpeciesDef::basic_attacks()` is what the readers take.
- **`field_only` means never-in-battle and `field_runnable` means offered on
  the map, and a priced ally-facing `Heal` is the one effect that is both.**
- **Every routine that moves Integrity rolls a band, and the census is what
  keeps it that way.** `spread` on `Damage`/`Drain`/`Heal`, rolled through
  `battle::DamageRange` — one draw whatever the width, so authoring a spread
  cannot shift a seeded stream.
- **`Game::choose_wild_action` is the one place a wild program's swing is
  decided** — move and target as a single joint choice.
- **Three policy features are pinned to zero in the shipped weights**, and
  that is a design boundary.
- **`is_boss` marks an *apex* species — always a boss, never engine-scaled —
  while any species can be *rolled* into one** and takes `BOSS_STAT_MULT`
  instead.
- **A species' danger band is derived and gates where it may spawn.**
- **Which side of the ground a boss dies on decides what it pays**, and
  underground is the game's **only** source of Portal Fragments.
- **Trace's group-size lever is a `spawn_pack` parameter, never a resource
  read inside it** — surface spawns keep rolling while the party is
  underground.
- **`Game::adopt_program` is the one way a program joins the roster without
  being beaten in a fight.** Two callers with opposite premises agree on
  what *becoming* a companion means.
- **There are four doors into the roster and `Game::roster_parts()` is the
  only barrier** — `grant_starting_program`, a capture, `adopt_program`, and
  `fuse_companions`, which assembles its own component list.
- **Destroying a tamed program has two paths.** `dissolve_tamed_program`
  handles four cases; `fuse_companions` does its own `retain`/`despawn` and
  skips the detachment logging.
- **No stats operation may run while a gear bonus is sitting in `Stats`.**
  Three operations would scale or bank it, welding the difference
  permanently into base stats.
- **The wielded program's bonus is computed live**, so destroying the
  program ends the wield by omission — the regression to head off is a later
  "fix" adding an explicit clear to both destruction paths.
- **The wielded program's proc runs as the *program*, not the player**, so
  which program you wield is what the feature is worth.
- **A fight is bounded by bodies, not just by groups.** `MAX_PACK_BODIES` is
  the ceiling on the whole pack, trimmed off the largest group each pass in
  `group_pack` — the two ceilings before it bounded a fight per group and
  per group count and never their **product**.
- **`start_battle` is the only path that caps a pack; `begin_battle` opens
  one.** The split exists for `arena`, which authors its own composition.
- **There are two battle rosters, and which one a caller wants depends on
  whether it *draws* or *acts*.** `battle_view` is live truth;
  `battle_view_at(revealed)` replays `BattleTimeline`.
- **A won fight says so, and it is the only ending that needed telling.**
  `settle_rewards` heads the results with "You won!", read off
  `BattleState::groups` being empty — telemetry's own definition.
- **A finished group fight keeps the battle screen, and a finished tactical
  fight is a popup over its own board** — `Mode::TacticalResult` drawing
  `ClosingRoster::board`, the copy `finish_fight` takes because
  `TacticalBattle` goes with the fight.
- **A battle does not end when the player's HP hits zero**, and three things
  heal them before anyone outside can look.
- **A fight's rewards are granted per kill and announced once.** Moving the
  *award* to the flush is the change to refuse — a level-up full-heals
  inside `add_xp`.
- **`retain_outcomes_since_battle` runs when the player *leaves* the results
  screen, not when the fight ends.** `Game::prune_battle_narration` is the
  door and `App::leave_battle_result` the one caller; run inside
  `end_battle` it deleted the decisive round before anything could reveal
  it.
- **There is one way into a staged arena fight, `arena::stage`**, and one
  reader of what one cost, `arena::Watch`.
- **An arena session touches no disk, and all three of those are omissions**
  — save, profile, run history — each with its own test asserting on the
  *file*.
- **Battle telemetry is the fourth thing an arena session touches, and it is
  allowed to write.** `flush_battle_telemetry` sits **above** `after_tick`'s
  `in_arena()` early return.
- **`nest_aggro_tick` is the first code to call `start_battle` from inside
  `tick_inner`**, which is why `rest`'s tick loop needed a battle check.
- **`nest_aggro_tick` is a reader of the player's `Position` and needs the
  underground guard** even though it never went through `require_surface`.
- **Resting is priced by locale, never gated by it.** Free inside base
  space, one unit of an `ItemDef::enables_rest` item anywhere else — the
  open grid and the Stack alike — and **no rest advances the clock**, which
  is what makes the free half safe.
- **A charged rest rolls `REST_AMBUSH_CHANCE` for an interrupt, and the roll
  rides the branch that takes the charge** — so base space is safe by
  placement, not by a locale check, and there is no refund.
- **A rest repairs the programs standing with the player and nobody else** —
  `InParty` and `Wielded` yes, `Sortie` and `Staff` no, exhaustively matched
  on `ProgramRole` so a fifth role fails to compile rather than defaulting.
- **`power_regen_system` needs that same guard**, and is the third in the
  family.
- **`Pursuing` must only ever be inserted alongside `NestGuardian`** — an
  untethered `Pursuing` has no leash and is never cleared.
- **`walkable()` alone does not decide where a `Pursuing` guardian may
  step** — `pursuit_field` excludes `Biome::Platform` separately.
- **There is one Dijkstra walk on the surface, and the step rule is a
  *cost function*, not a predicate** — `walk_field`, with `pursuit_field` a
  one-line wrapper and every surface and base-space caller answering
  `.then_some(1)`.
- **A `NestGuardian`'s tether refuses a step only when it both leaves
  `NEST_TETHER_RADIUS` and fails to close on the nest.** The simpler check
  froze a displaced guardian for the rest of the run.
- **`BattleState::planned` indexes `Party` positionally.** Nothing may leave
  `Party` mid-battle; deferred removal is why `end_battle` exists.
- **An initiative order names the party by slot and the wild side by
  identity**, and that asymmetry is the point.
- **A profile pays at `Game::new` and never at `Game::load`, and the
  enforcement is an omission.** `install_profile` says what has been earned
  and both paths call it; `grant_profile_rewards` pays, and only the
  new-game path calls it.
- **`resources::RunFeats` is a per-tick drain queue and is not saved**, with
  two fields and two drainers, one each.
- **`ActiveContract` stores the whole resolved `ContractDef`, not an id**,
  so a contract file edited or deleted mid-run cannot strand or rewrite one
  already accepted.
- **Contracts deliberately amend "progression is earned by fighting."** XP
  is a legal reward on *any* objective; anyone "restoring" the old invariant
  by gating XP behind combat is undoing the feature.
- **`Game::level_cap` is the only ceiling in the game and it takes no
  entity** — player and every companion stop at the same zone-derived
  number, `max(ZONE_LEVEL_CAP_FLOOR, 1 + ZONE_LEVEL_CAP_STEP * (zone - 1))`.
- **The cap's constants are fitted against `balance_sim` and the lower bound
  is a correctness bound.** The cap must sit at or above the *geared* clear
  requirement — below it a fully-equipped party cannot clear the zone at any
  level it may reach, which is a dead run, not difficulty.
- **Two renames are load-bearing and neither would fail to compile.**
- **XP at the cap is banked, not discarded, and banking and taxing share the
  one accumulator.** `add_xp` accumulates into `Experience::xp` and reports
  `LevelGain::overflow`, staying pure — it reports, the caller spends.
- **The level-up page's before column is a stored `Combatant`, never a
  re-derivation at the old level.**
- **`arm_status` is the sole writer of `StatusEffects` (besides Cleanse and teardown) and every in-battle heal or drain goes through `restore_hp`, where `HealBlock` is read.**
