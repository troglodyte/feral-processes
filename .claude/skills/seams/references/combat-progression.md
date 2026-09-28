# Seams: Combat: attack resolution, XP, levels, talents & the cap

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

- **How outmatched you are is one duel formula on two sides, `Game::threat_to`
  — the party for the con and the capture odds, the player alone for kill
  XP.** `Stats::power` is effective HP *plus* attack, and at HP ten times the
  size of attack it measured HP alone: a program hitting twice as hard as the
  player read Yellow beside one barely scratching them (todo #112). Handing
  any of the three readers `Stats::power` again compiles clean and brings
  that back; so does passing `party_threat` to `kill_xp`, which makes a
  companion cost XP. A fixture that sizes a program "to an even match" must
  pad until `party_threat` reaches the threshold, not add up a power total.

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

- **The level-up page's before column is a stored `Combatant`, never a
  re-derivation at the old level.** `Game::snapshot_player` calls the same
  `combatant_profile` a live fight calls, at the instant a level-up begins,
  and `resources::PendingLevelUp` holds the result until
  `Game::take_level_up_report` drains it. The trap is a second
  `combatant_profile`-shaped function parameterized on a level — Stats are
  mutated in place on level-up with no per-level history kept, so "what did
  the player's `Combatant` look like one level ago" has no live source of
  truth except a snapshot taken *before* the mutation; a re-derivation would
  have to either rewind `Stats` (nothing stores the old values) or duplicate
  `combatant_profile`'s whole formula with a level parameter threaded through
  it, which is exactly the "a doc comment that claims to mirror another
  module's formula must be a call, not a copy" trap CLAUDE.md's code
  principles already name four times against `balance_sim.rs`. The write is
  guarded on `PendingLevelUp` being `None`, which is what makes several
  levels gained in one award collapse into **one** page: `from_level` is the
  level held before the *first* of them, not a fresh snapshot per level.
  `PendingLevelUp` is not saved — `RunFeats`' own precedent — so quitting
  between the level and reading the page loses the page, never the levels.
