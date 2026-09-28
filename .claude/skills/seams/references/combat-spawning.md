# Seams: Combat: spawning, bosses, difficulty & the roster doors

- **A difficulty band is a fractional zone step, never a flat multiplier on
  stats**, and `Game::zone_curve_ratio` is the one derivation it shares with
  the distance ramp. `resources::EnemyStrength` has no rung below `Standard`
  and `Standard` is exactly zero steps, so every existing save and every
  `balance_sim` curve is untouched by the feature existing. **The trap is the
  obvious spelling**: a flat `stat_mult` band reads as the simpler design and
  is the bullet below's race in miniature — `ZONE_STAT_STEP` is 1, so a zone
  is a linear addend and a multiplicative band is a geometric quantity riding
  it, *and* it means a different thing at every zone (x1.5 is half a zone at
  zone 1 and three at zone 6). As a step it means one thing everywhere and
  `balance_sim` gates it for free, the distance ramp's own argument: zone N
  one band up **is** the zone N+1 fixture the sweeps already cover. The band
  and the ramp are therefore the same addend at the same seam and **add
  rather than compound** — `field_stat_mult` is
  `zone_curve_ratio(t + band.zone_steps())`, and underground the band arrives
  as a factor on `stack_depth_multiplier` where `trace_stat_mult` already
  composes. Both fold sites are **caller-side**, which is what leaves
  `a_spawns_stats_come_from_its_escalation_and_never_from_its_tile` true.
  Changing the band makes `enter_next_zone`'s three calls, because the term
  is baked into `Stats` at spawn and would otherwise only be felt on ground
  not yet reached — refused mid-fight, since `clear_local_wild` despawns
  bodies; and it misses a `NestGuardian` and a `Nemesis`, that function's own
  exclusions. The door is a dev-console row and **not** the Options screen,
  which writes cross-run `profile.ron` from a menu with no run behind it.

- **Every difficulty curve in the game is linear.** A geometric enemy curve
  racing a linear player curve outruns it wherever you put the coefficients.
  A linear tier step is a *ratio*, so `ZoneLevel::raised_a_tier` applies it
  rather than truncating to 1. `balance_sim` bounds per-zone *steps*, not
  ratios — a ratio bound passes any compounding curve with a small enough
  base.

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

- **The party is the fifth thing that buys a difficulty step, and it must
  stay out of `danger_steps`.** `tuning::zone_band_floor` pairs with
  `zone_level_cap` to give a zone a **level band**;
  `Game::party_band_progress` is the player's level as a 0..1 fraction
  across it, so a breach resets it and levelling is still worth something —
  a world that tracks the party exactly is a treadmill. `party_stat_steps`
  (progress times `PARTY_LEVEL_STAT_STEPS`) is one more addend inside
  `zone_curve_ratio`, at `field_stat_mult` and
  `stack_depth_multiplier_sharing`; it reaches the Stack where the distance
  ramp deliberately does not, because it is a property of the *party* and
  not of where the spawn was placed. **The trap is the count half's obvious
  home.** `danger_steps` is literally the scalar the two group curves read,
  but it has a *third* reader — the species danger-band window in
  `habitat_pools` and `pick_lair_species` — so a party term folded in there
  changes **what** you meet and eventually opens apex bosses because you
  levelled, the shape the distance bullet above already refuses. Hence
  `Game::group_steps`, a separate sum whose two callers are the census, with
  `MAX_GROUP_SIZE_STEPS` clamping the **sum**: bounded per half the total
  reaches twice it, and that ceiling is what stops
  `GROUP_SIZE_DISTANCE_GROWTH.pow` running away. **Each spawn rolls its own
  share.** `roll_party_share` is a uniform `0..=1`, so a developed party
  meets a spread from the ground's baseline up to their level rather than a
  field levelled in lockstep. It could not live inside `field_stat_mult` or
  `stack_depth_multiplier` because both have non-spawn readers —
  `stack_market` prices a **quote** off the second — so each has a pure door
  and a `&mut self` rolled one, and both fold sites stay caller-side, which
  is what leaves
  `a_spawns_stats_come_from_its_escalation_and_never_from_its_tile` true.
  **`roll_party_share` takes no draw at all when the term is zero**, and
  that omission is what keeps every seeded fixture still — `roll_affix`'s
  rule on an empty pool. There is deliberately **no clamp of its own**:
  `Game::level_cap` is the only bound a level-driven term can have, and
  `PARTY_LEVEL_STAT_STEPS` is the knob, held at or under one zone step by
  `the_party_stat_term_stays_inside_balance_sims_reach` — above that the far
  field of zone N is a fixture `balance_sim` does not sweep.

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
