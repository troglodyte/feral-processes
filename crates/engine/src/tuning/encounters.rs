// ─────────────────────────────────────────────────────────────────────────
// Zone & distance scaling
// ─────────────────────────────────────────────────────────────────────────

/// Linear step for `ZoneLevel::stat_multiplier`, the flat multiplier on
/// every wild program's stats in a zone: zone 1 is x1 and each level after
/// *adds* this (1, 2, 3, 4, 5, ...).
///
/// **Linear rather than geometric, and the distinction is the difference
/// between a hard game and an unfinishable one.** It was a geometric base of
/// 2. Everything on the player's side of the fight grows linearly —
/// `ATK_PER_LEVEL` is 1, an item is worth a flat point
/// or four — so a doubling enemy curve is a geometric quantity racing a
/// linear one, and the geometric side always wins in the end whatever the
/// coefficients are. Under the subtractive damage rule of the time that did
/// not read as "hard": once enemy DEF passed your ATK every swing landed on
/// the one-point floor and the fight stopped responding to your stats at all.
/// That rule is gone — mitigation is a percentage cut capped below immunity
/// — but the geometric-versus-linear race is why the curves stay linear.
///
/// Measured on the real roster before the change: the level needed to keep
/// pace with a Stack guardian ran ~50, ~105, ~215, ~440, ~880 for zones 1-5,
/// and a zone-3 depth-5 lair was unbeatable at level 90 in the best gear the
/// game ships. Linear turns that into a roughly constant number of levels per
/// zone — a curve a player can fund forever.
///
/// `GEAR_LEVEL_STEP` is matched to this exactly as it was matched to the old
/// base, so neither gear nor zone outruns the other.
pub const ZONE_STAT_STEP: i32 = 1;

/// How many zone steps one rung of `resources::EnemyStrength` is worth.
///
/// The band is a **fractional zone step**, not a flat multiplier on stats,
/// and that is the whole of why it is affordable. `ZONE_STAT_STEP` above is
/// a linear addend, so a flat multiplier would be a geometric quantity
/// riding a linear curve — the race that doc spends its length explaining
/// the game does not run — and it would mean a different thing at every
/// zone: x1.5 is half a zone at zone 1 and three zones at zone 6. As a step
/// it means one thing everywhere, "the ground fights like it is N zones
/// deeper", and `balance_sim` gates it for free, because zone N at one band
/// up is arithmetically the zone N+1 fixture the sweeps already cover.
/// `Game::field_stat_mult`'s cap makes the same argument for distance.
pub const ENEMY_STRENGTH_BAND_STEP: f32 = 0.5;

/// Radius of the zone-1 newbie ring, in tiles from
/// `Game::distance_from_danger_origin` — the base platform's edge once a
/// Home exists, `ZoneSpawnPoint` before then. Inside it, only species a
/// bare level-1 player can beat are *born* (see `Game::in_opening_ring`).
///
/// The ring is the flat *floor* under the field ramp: `DANGER_RAMP_TILES`
/// measures from its edge, so inside it `Game::field_stat_mult` is exactly
/// 1.0. That is not politeness — `balance_sim::beatable_by_a_fresh_player`
/// is computed against the unscaled species, so any ramp in here falsifies
/// it. What the ring does that the ramp does not is gate the *pool*: it
/// decides which species are born, where the ramp only decides how hard
/// one already born is.
///
/// Its own literal, and deliberately *not* `MAX_BUILD_DISTANCE_FROM_HOME`,
/// which is what it used to be. That spelling made the ring exactly your
/// base and its doorstep, travelling with the base for free — an argument
/// that only held while the base was one fixed size. It is now a starting
/// size a Heap Pillar grows, so a derived ring would shrink the nursery to
/// 4 for the opening minutes and then *widen* it every time the player
/// builds, which is a difficulty knob keyed to base geometry: precisely the
/// thing removed on 2026-08-05 when distance stopped scaling anything.
/// 7 keeps the ring the size it has always been and stops it moving.
///
/// An explicit radius rather than "wherever the curves say a fight is one
/// program", which is how the ring used to be spelled before that: with
/// fixed zone scaling that condition is true across the whole of zone 1, so
/// the old spelling would have silently made the entire zone a nursery.
pub const OPENING_RING_TILES: i32 = 7;

/// How far past `OPENING_RING_TILES` a surface spawn has to be for the field
/// ramp to reach its cap — see `Game::field_stat_mult`.
///
/// **The cap is one zone step, not a free multiplier.** At the cap a spawn's
/// stats are arithmetically `ZoneLevel(zone + 1).stat_multiplier()`: the far
/// field of a zone is the doorstep of the next one. Two things follow, and
/// both are the reason distance is affordable again after being removed on
/// 2026-08-05. `balance_sim` needs no new bound, because the far field of
/// zone N *is* the zone N+1 fixture it already sweeps. And the zone number
/// still means something — a zone spans `[N, N+1]` and its floor is exactly
/// the previous zone's ceiling — which is the answer to the first of that
/// removal's two bugs, a zone having no consistent difficulty of its own.
///
/// 32 is one `world::CHUNK_SIZE`, and it is scaled against what the rest of
/// the map already measures in rather than against the world being infinite.
/// Everything the player reaches for sits inside a couple of dozen tiles —
/// `EXAMINE_RANGE_TILES` and `WILD_SPAWN_RADIUS_TILES` are both 12, the
/// nearest Stack link is `STACK_NEAREST_LINK_TILES` at 8 — and the furthest
/// the game scatters its own content is `STACK_LINK_SCATTER_TILES` at 40.
/// A ramp topping out at `OPENING_RING_TILES + 32 = 39` therefore finishes
/// exactly where the outermost link already is: walk to the far links and
/// you have seen the whole gradient. It opened at 128, four chunks, which
/// was picked off the world being unbounded and was three times further out
/// than anything else on the map — the field read flat because the player
/// never walked far enough to leave the first tenth of the ramp.
///
/// The trade is deliberate and worth stating: past 39 tiles a zone sits
/// uniformly at the next zone's doorstep, so the gradient is an *opening*
/// rather than a property of the whole zone. That is the right way round —
/// the ramp exists so a strong creation build has somewhere to walk early,
/// not so the zone has texture forever.
///
/// This is the knob to move if the field reads too flat or too steep; every
/// other part of the ramp is derived from the zone curve and is not a number
/// to tune.
pub const DANGER_RAMP_TILES: i32 = 32;

/// How far `x` looks along the row or column the player is facing
/// (`Game::find_target_in_direction`).
///
/// **Engine tuning rather than the frontend's `MENU_SCAN_RADIUS`, which is
/// what this used to be.** That constant is a *menu window* — how much world
/// a picker lists — which is genuinely a frontend policy, and at 40 tiles it
/// is more than twice the map pane in either axis. Borrowing it made the
/// inspector name things the player could not see, which is a rule about the
/// game rather than about a list. Nothing keeps this in step with the pane
/// automatically: the renderer derives its half-width and half-height from
/// live pixels, so the visible area changes when the window is resized and
/// the engine cannot read it.
///
/// 12 sits inside the pane horizontally, a little past it vertically, and is
/// comfortably more than `MAX_BUILD_DISTANCE_FROM_HOME` so you can examine
/// right across your own base.
///
/// Deliberately not written as `WILD_SPAWN_RADIUS_TILES` even though both are
/// 12 today. They agree by coincidence and would have to be justified
/// together if they were one name — see `CLAUDE.md` on a shared formula
/// having to be a call rather than a claim.
pub const EXAMINE_RANGE_TILES: i32 = 12;

/// Geometric base for group-size growth per escalation step, and the cap on
/// how many steps count. The exponent has to be clamped because depth is
/// unbounded and a shift of 32 or more is a panic in debug;
/// `MAX_GROUP_SIZE_STEPS` is set where `GROUP_SIZE_DISTANCE_GROWTH.pow(steps)`
/// already exceeds `MAX_GROUP_SIZE`, so the clamp is exact rather than a fudge.
pub const GROUP_SIZE_DISTANCE_GROWTH: u32 = 2;
pub const MAX_GROUP_SIZE_STEPS: u32 = 7;

/// Frames descended per escalation step in the Stack, feeding
/// `Game::danger_steps`.
///
/// One frame per step, against `GROUP_SIZE_STEP_ZONES` for the surface,
/// because a frame is a maze to cross and a one-way door at the bottom of
/// it. The party also arrives at depth 1 already having chosen to be there,
/// so the first frame stays at step zero — the entrance is the Stack's own
/// opening ring.
pub const GROUP_SIZE_STEP_FRAMES: u32 = 1;

/// Zones breached per escalation step on the surface — the counterpart to
/// `GROUP_SIZE_STEP_FRAMES`, feeding the same curve through
/// `Game::danger_steps`.
///
/// One zone per step, so a zone's fights are the same shape wherever in it
/// you stand. This replaced a fifteen-tiles-per-step distance curve: the
/// escalation a player feels should come from the commitments they make —
/// funding a Portal, descending a link — not from which direction they
/// happened to wander.
pub const GROUP_SIZE_STEP_ZONES: u32 = 1;

/// Danger steps between one growth band entering the spawn pool and the
/// next, and how many steps a band stays in it once it has — the window
/// that decides which species a zone or a Stack depth may field.
///
/// Read against `Game::danger_steps`, the same scalar the two group-size
/// curves take, so there is no second difficulty axis to keep in step with
/// the first. Band `b` is live from `b * TIER_ENTRY_STEPS` through
/// `b * TIER_ENTRY_STEPS + TIER_WINDOW_STEPS` **inclusive**.
///
/// The top band never exits, whatever these say. Steps are unbounded
/// because zones and depth are, so a closed top empties the world.
pub const TIER_ENTRY_STEPS: u32 = 2;
pub const TIER_WINDOW_STEPS: u32 = 3;

/// The step a boss species (`SpeciesDef::is_boss`) becomes eligible at, and
/// it never exits either.
///
/// Apex is outside the growth ladder, so its entry is a constant rather than
/// a fourth rung of `TIER_ENTRY_STEPS`. Before this step a boss roll still
/// fires — it just draws an ordinary species and marks it, which is the
/// whole of "easy bosses early, hard bosses deep".
pub const APEX_ENTRY_STEP: u32 = 4;

/// Extra members each zone level adds to the group-size cap: zone 1 is solo,
/// and every level after adds this against `MAX_GROUP_SIZE`
/// (1, 10, 19, 28, 37, ... saturating at zone 12). Only
/// `battle::attackers_in_group` of a group swing per round, so a deep swarm
/// is an attrition wall rather than a linear multiplier on incoming damage.
///
/// Additive rather than the geometric x3 this used to be, and the early
/// zones are the reason. Geometric growth from a base of 1 spends its whole
/// playable range in single digits — zone 2 capped every group at 3, which
/// held Stack packs and surface packs alike to three programs against a
/// party of five, whatever else the distance and depth curves had earned —
/// and then runs away past zone 4 into caps of 27 and 81 that no encounter
/// is designed around. A line opens the zones the game is actually played
/// in and keeps the tail legible.
///
/// This is the *ceiling*, not the roll: `spawn_pack` still draws uniformly
/// in `1..=ceiling`, so raising it widens the range of fights a zone can
/// produce rather than making every fight bigger.
pub const ZONE_GROUP_STEP: u32 = 9;

/// The floor under `zone_group_cap`, and so the size of a zone-1 group.
///
/// Zone 1 used to be solo, which fell out of the curve — `ZONE_GROUP_STEP *
/// (zone - 1) + 1` is 1 there — rather than being chosen. That made the
/// zone-1 fixture in `balance_sim` a five-against-*one* fight, which is not
/// the body ratio the rest of the curve is about, and it left a new player's
/// entire first zone unable to teach them that programs come in groups.
///
/// A floor rather than a change to the curve's base: zone 2 is already 10,
/// so this lifts zone 1 alone and every later zone's step is exactly what it
/// was. Note it also ends `TRACE_GROUP_MULT`'s zone-1 inertness, which was a
/// consequence of the old value and never an intent.
pub const ZONE_ONE_GROUP_CAP: u32 = 2;

/// Hard ceiling on a single species group.
///
/// It used to be quoted with `MAX_ENEMY_GROUPS` as "one intrusion tops out
/// at four hundred programs", and that product is exactly what nothing
/// bounded — see `MAX_PACK_BODIES`, which does, and well below either of
/// these. This is now the ceiling on how deep *one* species may stand in a
/// fight, and it binds only where the total does not.
pub const MAX_GROUP_SIZE: u32 = 100;

/// How many distinct species groups can engage in one intrusion. A cluster
/// with more species than this engages its largest groups and leaves the
/// remainder standing on the map as ordinary hostiles — they're met on the
/// next bump rather than silently despawned.
///
/// This is now a bound on a fight's *variety* rather than on its size:
/// `MAX_PACK_BODIES` decides how many bodies the engaging groups share
/// between them, and it is the smaller number.
pub const MAX_ENEMY_GROUPS: usize = 4;

/// Hard ceiling on the **whole** pack one fight may field, across every
/// group in it.
///
/// The two ceilings above bound a fight per group and per group count, and
/// nothing bounded their product — 4 x `MAX_GROUP_SIZE` in principle, and
/// 33 bodies against a party of four in practice at zone 3 depth 5, which
/// wins every rep. The surface never reached it (a surface fight is
/// whoever `gather_pack` found standing together, measuring 2.6 bodies at
/// zone 3), but `Game::stack_encounter_pack` fills it by construction: it
/// takes one species pick and one full group roll **per group slot the
/// curve allows**. So the same curve produced a shallow-Stack fight four
/// times the size of the surface fight beside it, and the Stack was
/// unwinnable from depth 3 down.
///
/// Fitted rather than chosen: 8 is what puts an ordinary Stack ambush at
/// 100 / 93.5 / 67 / 39.5 across depths 2-5 for a party at the zone level
/// cap, which is a curve that descends. 12 leaves depth 5 at 0%.
/// `docs/measurements/2026-08-28-stack-depth-curve-after-danger-steps.md`
/// carries the sweep, the isolation of the three terms that feed this
/// fight, and what the number is blind to.
///
/// A *ceiling on the fight*, not a cull: the bodies it turns away stay
/// standing on the map exactly as `MAX_ENEMY_GROUPS`' surplus does, and are
/// met on the next bump.
pub const MAX_PACK_BODIES: u32 = 8;

/// How many enemy groups are in melee range of the party. Groups past this
/// index can only act with a move flagged `ranged`, which is what keeps a
/// four-group pack from simply quadrupling incoming damage — and what makes
/// wiping the front group a real decision, since it promotes a back group
/// into reach.
pub const ENGAGED_GROUPS: usize = 2;

/// How much of a zone-portal structure's base `build_cost` is added to its
/// price per zone below the current one. Breaching deeper costs more, but
/// currency does not survive the trip (see `Game::enter_next_zone`), so
/// this is a ramp on a from-zero grind rather than a tax on a stockpile —
/// which is why it adds half the base rate per zone instead of doubling.
pub const ZONE_PORTAL_COST_GROWTH_PERCENT: u32 = 50;

/// Thresholds for `difficulty_color`'s old-school "con" coloring, as
/// upper bounds on `Game::party_threat` — how outmatched the player's side is
/// by one hostile program, `battle::threat_ratio`'s scale — so anything at or under `DIFFICULTY_EASY_MAX` reads
/// Green, up through `DIFFICULTY_EVEN_MAX` reads Yellow, up through
/// `DIFFICULTY_TOUGH_MAX` reads Orange, and anything above that reads Red.
pub const DIFFICULTY_EASY_MAX: f64 = 0.7;
pub const DIFFICULTY_EVEN_MAX: f64 = 1.1;
pub const DIFFICULTY_TOUGH_MAX: f64 = 1.6;

// ─────────────────────────────────────────────────────────────────────────
// Taming
// ─────────────────────────────────────────────────────────────────────────

/// Fraction by which each point of the player's `Decompiler` stat (see
/// `components::Decompiler`) *multiplies* their decompile odds — skill 40 is
/// a 1.8x on whatever the attempt was already worth.
///
/// Multiplicative rather than added on top, because `Decompiler` skill has no
/// ceiling (+1 per player level forever, +1..=4 from each of fifteen gear
/// items) while the base it applies to
/// cannot exceed 0.33 — the Breach Kit's `taming_potency` of 0.4 is the
/// strongest catalyst that ships. As a flat addend, skill 40 was worth +0.80
/// and pinned every attempt to `CAPTURE_CHANCE_MAX` regardless of species,
/// leaving `taming_difficulty` and weakening the target both meaningless.
/// Scaling the base instead keeps them inside what's being multiplied, so
/// they matter at every skill level.
pub const DECOMPILER_SKILL_BONUS: f32 = 0.02;

/// Percentage points a `PlayerClass::Decompiler` adds to every decompile
/// attempt, through `classes::capture_boost_pct`.
///
/// A whole-attempt multiplier rather than an addend to the `Decompiler`
/// stat, so it is worth as much at the level cap as at level 1 — see that
/// query's own doc comment. Argued from the shipped curve rather than
/// measured: `balance_sim` models no decompiling at all, so the arena is
/// the only instrument that can retune this.
pub const CLASS_DECOMPILE_BOOST_PCT: i32 = 15;

/// Routine slots a `PlayerClass::Invoker` carries on top of the level
/// curve, through `classes::routine_slot_bonus`.
///
/// Doubles a level-1 player's two slots and still stands at the cap, since
/// it is added past `PLAYER_ROUTINE_SLOT_CAP` the way `talent_routine_slots`
/// already adds past the companion cap.
pub const CLASS_ROUTINE_SLOT_BONUS: usize = 2;

/// What a `PlayerClass::Fabricator` scales every work cycle's length by,
/// through `classes::work_tick_scale`. Below 1.0 is faster.
///
/// One fifth off, which at the shipped `ticks_per_unit` rates is a cycle or
/// two an hour of play rather than a different game. `balance_sim` has no
/// base term, so this is unmeasured like the two above it.
pub const CLASS_WORK_TICK_SCALE: f64 = 0.8;

/// Coefficients of `taming::capture_chance`. Ceiling below a full 1.0 means
/// even a fully-weakened, zero-difficulty target isn't a sure thing on item
/// potency alone; the two penalties subtract the target's remaining HP
/// fraction and its species' `taming_difficulty` from that ceiling.
///
/// `CAPTURE_HP_PENALTY` is the one of the three a player can move directly,
/// via `Perk::ExploitFocus` — see
/// `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL`. It is also the largest
/// swing in the formula: at the ceiling of 0.9 it takes a full-HP target's
/// term down to 0.25, so draining one before attempting a decompile is worth
/// 3.6x.
pub const CAPTURE_POTENCY_CEILING: f32 = 0.9;
pub const CAPTURE_HP_PENALTY: f32 = 0.65;
pub const CAPTURE_DIFFICULTY_PENALTY: f32 = 0.6;

/// How the gap between the two sides bends a decompile. The ratio is
/// `Game::party_threat`'s — the same number `difficulty_color`
/// buckets into con colors — so the color painted on a program and the odds
/// rolled against it can never disagree about which of you is stronger. Both
/// ramps are bounded by the `DIFFICULTY_*` thresholds rather than by numbers
/// of their own, so moving a con boundary moves the taming math with it.
///
/// The two halves are deliberately not symmetric, because the problems
/// aren't. On the weaker-target side the ratio waives `CAPTURE_HP_PENALTY`
/// instead of scaling the chance: against a program you delete in one strike
/// there is no wearing-down for that penalty to reward, so a formula that
/// keeps demanding it is demanding something the fight cannot supply. Full
/// relief lands at `DIFFICULTY_EASY_MAX`, where the target stops reading as a
/// threat at all, and there is none left by `DIFFICULTY_EVEN_MAX`.
///
/// On the stronger-target side it multiplies the whole attempt down from
/// `DIFFICULTY_TOUGH_MAX`, for the reason `DECOMPILER_SKILL_BONUS` and
/// `DECOMPILE_ATTEMPT_BONUS_PCT` also multiply rather than adding into the
/// base: how outgunned you are must not become a route around a species' own
/// `taming_difficulty`, in either direction. The floor keeps a deep-zone
/// monster a long shot rather than a wall — 0.6x on top of a boss-grade
/// difficulty still clears `CAPTURE_CHANCE_MIN` comfortably.
pub const CAPTURE_OUTCLASSED_RATIO_FLOOR: f32 = 2.5;
pub const CAPTURE_OUTCLASSED_MULT_FLOOR: f32 = 0.6;

/// How much each decompile already attempted against a target raises the
/// odds of the next attempt on that *same* program, in percentage points,
/// and how many attempts stop counting.
///
/// The counter is battle-scoped (`BattleState::decompile_attempts`), so this
/// is "you are wearing this program's defences down", not a run-wide pity meter —
/// walking away and coming back meets it fresh. The cap is what keeps it
/// from becoming one: at 5 x 10 points the most persistence can buy is
/// 1.5x, so a stubborn species stays a gamble however many catalysts get
/// burned on it, and the catalyst cost is the real brake.
///
/// It multiplies alongside `skill_multiplier` and `capture_boost_pct` rather
/// than adding into the base, so — like those two — it can't be out-scaled
/// by a species' own resistance.
pub const DECOMPILE_ATTEMPT_BONUS_PCT: u32 = 10;
pub const DECOMPILE_ATTEMPT_BONUS_CAP: u32 = 5;

/// Hard bounds on the final decompile chance, applied after skill bonuses.
/// No attempt is ever hopeless and none is ever certain.
///
/// These are safety rails, not balance levers — in particular the maximum is
/// unreachable with the content that ships and shouldn't be reached for when
/// tuning. The only catalyst is the Breach Kit at `taming_potency` 0.4, so
/// the best base the formula can produce is a fully-drained Drone at
/// `0.4 * 0.9 * 0.91 = 0.328`; clearing 0.95 would need a 2.9x multiplier
/// over it. The three that stack are skill (1.76x at a realistic `Decompiler`
/// ceiling near 38 — level 30 plus the best 8 points of gear), a capped
/// 1.5x from `DECOMPILE_ATTEMPT_BONUS_PCT`, and whatever `CaptureBoost` is
/// running; the first two together reach 0.866, so the margin is real but
/// thinner than it was. See
/// `high_skill_does_not_flatten_the_gap_between_easy_and_boss_species`.
pub const CAPTURE_CHANCE_MIN: f32 = 0.05;
pub const CAPTURE_CHANCE_MAX: f32 = 0.95;

/// `SpeciesDef::taming_difficulty` assumed for a species that has gone
/// missing from the db mid-battle — dead centre of the 0..=1 range, so a
/// lookup failure neither gifts nor denies the capture.
pub const DEFAULT_TAMING_DIFFICULTY: f32 = 0.5;

/// Decompile rolls a Holding Cell's prisoner gets before it breaks down.
pub const JAIL_MAX_ATTEMPTS: u32 = 5;

/// The `item_potency` of every decompile roll against a prisoner, so no
/// catalyst is spent. Above the Breach Kit's 0.4 on purpose: at 0.4 a
/// full-Integrity, even-match prisoner of a mid-difficulty species (0.5) is
/// taken on 11.6% of the first roll and only about 53% of the time across
/// `JAIL_MAX_ATTEMPTS` rolls (each failure adds 10% of the base, up to
/// `DECOMPILE_ATTEMPT_BONUS_CAP`), so nearly half the cells would end in a breakdown.
/// 0.55 gives 15.9% on the first roll, rising to 22.2% on the fifth, and
/// about 65% overall. `tests/prison.rs` holds that shape.
pub const JAIL_BASE_POTENCY: f32 = 0.55;

/// The share of an ordinary extraction roll a broken-down prisoner pays out
/// into its cell's `Stock::output`: a failed decompile still salvages
/// something, but well under what stripping the same record on a rig pays.
pub const JAIL_BREAKDOWN_SCALE: f32 = 0.4;

// ─────────────────────────────────────────────────────────────────────────
// Spawning & encounters
// ─────────────────────────────────────────────────────────────────────────

/// Chance per tick that a wild spawn roll fires at all (see
/// `Game::maybe_spawn_wild_creature`), and the box radius around the player
/// the roll places into. The radius is wide enough that a spawn lands
/// off-screen and is walked into rather than appearing on top of you.
///
/// **Pinned to app-core's `WORLD_SPEED_MULTIPLIER`**, the same way
/// `WANDER_COOLDOWN_MIN_TICKS` is and with nothing to make it fail to
/// compile when that moves: this was `0.05` while the world ran at one tick
/// a real second, and it is halved so that ambient encounters keep arriving
/// at the rate they did per real minute rather than at twice it. A *cap* is
/// not what paces this — `WILD_LOCAL_DENSITY_TARGET` bounds how many end up
/// standing around you, not how often one shows up.
pub const WILD_SPAWN_CHANCE: f64 = 0.025;
pub const WILD_SPAWN_RADIUS_TILES: i32 = 12;

/// Chance per walked step that the player is ambushed — a biome-appropriate
/// pack spawns adjacent and engages immediately, with no chance to route
/// around it (see `Game::maybe_ambush`). Deliberately an order of magnitude
/// rarer than an ordinary encounter feels like it should be: the map is
/// already full of programs you can choose to fight, so this is the tax on
/// crossing open ground rather than the main source of battles. Not rolled
/// on base platform tiles, and never produces a boss or a nest.
pub const RANDOM_ENCOUNTER_CHANCE: f64 = 0.02;

/// Chance that powering down in the field is interrupted — the pack engages
/// and nothing is restored, though the charge is spent all the same.
///
/// **Not comparable to the per-step rates above it.** Those are rates that
/// accumulate over a walk; this is the whole risk of one discrete event, so
/// a number that looks an order of magnitude larger is not an order of
/// magnitude more dangerous. Modest on purpose: an outlet that buys a fight
/// too often stops reading as a way to recover at all.
///
/// Only charged rests roll. A free rest inside base space never reaches this
/// — it rides the branch that takes the charge, so the slab stays the one
/// safe ground without a locale check of its own.
pub const REST_AMBUSH_CHANCE: f64 = 0.15;

/// The wild population one screenful of map should hold: `Hostile`s within
/// `WILD_SPAWN_RADIUS_TILES` of a tile, which is a 25x25 box and so almost
/// exactly the ~33x19 the map pane shows at default zoom. The target is
/// therefore legible on screen rather than being an abstract number.
///
/// A *target*, not a cap, and the one figure both halves of the population
/// model read: `Game::spawn_initial_creatures` seeds a zone up to it and
/// `Game::maybe_spawn_wild_creature` tops it back up without exceeding it.
///
/// Before it existed there was no target at all. Spawning is player-relative
/// and nothing ever removed a creature — `WILD_CREATURE_CAP` sits two orders
/// of magnitude above any real population and has never fired — so density
/// was simply the integral of where the player had stood. Measured on a real
/// save: 65 hostiles in one box around a base that had been worked at, 7 in
/// the entire map beyond 40 tiles. Both numbers are the same bug.
///
/// Halved from 12: the field read as crowded, groups overshoot the
/// target, and the ambient specks drawn by the renderer now carry the sense of
/// life that sheer headcount used to.
pub const WILD_LOCAL_DENSITY_TARGET: usize = 6;

/// The wild population one chunk of the world holds — see
/// `Game::populate_chunk`. Terrain arrives a `world::CHUNK_SIZE` square at a
/// time and so does what lives on it, which is what lets the sector be
/// populated everywhere rather than only where the player has been.
///
/// Derived rather than tuned, for the reason the old `initial_wild_population`
/// was: the density a patch of ground is *born* at and the density
/// `Game::maybe_spawn_wild_creature` *maintains* must not be able to drift
/// apart. It is `WILD_LOCAL_DENSITY_TARGET` scaled from one spawn box up to
/// one chunk.
///
/// It is an upper bound rather than an exact count, and safe to
/// over-estimate: `populate_chunk` applies the same density gate the ambient
/// roll does, so a placement that would overfill its own patch is skipped —
/// which matters because one placement puts down a *group* of up to
/// `max_group_size`, not one creature.
pub const fn chunk_wild_population() -> usize {
    let chunk = crate::world::CHUNK_SIZE as usize;
    let spawn_box = (2 * WILD_SPAWN_RADIUS_TILES + 1) as usize;
    (chunk * chunk * WILD_LOCAL_DENSITY_TARGET) / (spawn_box * spawn_box)
}

/// How many chunks out from the player's own get stocked, as a Chebyshev
/// radius — so 1 is the 3x3 neighbourhood around them.
///
/// One, not zero. Stocking only the chunk the player stepped into would pop
/// programs into view inside ground they can already see: a chunk is 32
/// tiles and the map pane shows roughly 33x19, so a chunk-edge arrival would
/// draw the spawn. One chunk of margin is at worst 32 tiles and at best 64,
/// both comfortably outside the pane, which preserves the property
/// `WILD_SPAWN_RADIUS_TILES` was chosen for: a spawn lands off-screen and is
/// walked into rather than appearing on top of you.
pub const POPULATION_CHUNK_MARGIN: i32 = 1;

/// How many chunks around the party the world map reveals as it walks. One
/// is the 3x3 block the party is standing in the middle of, which is as far
/// as `POPULATION_CHUNK_MARGIN` already stocks and about what the pane
/// shows.
pub const WORLD_MAP_REVEAL_RADIUS_CHUNKS: i32 = 1;

/// How close to its due date a Server must be for the world map to say it
/// grows "soon" rather than "in time". The map speaks in words, never in
/// ticks, so this is the one place the cut-off is a number.
pub const WORLD_MAP_GROWTH_SOON_TICKS: u64 = 3000;

/// How many Stack links a zone is seeded with — see
/// `Game::spawn_surface_links`. Deliberately few: a link is
/// something you go looking for, and one on every corner would make the
/// zone map a lobby rather than a place.
pub const STACK_LINKS_PER_ZONE: usize = 3;

/// How far from the player's arrival point a zone's Stack links
/// scatter — far enough that finding one is a trip rather than a glance.
///
/// It used to be pinned by a `const _: () = assert!` to sit inside the
/// distance a zone's wild programs were seeded to, or a link sent the
/// player onto ground that had been born empty. That relationship
/// dissolved when population became a property of place: ground stocks
/// itself when the player reaches it, however far out it is, so a link may
/// now scatter as far as it likes.
///
/// It does *not* keep links off the base platform by itself. That used to be
/// `Game::stamp_platform`'s job on top of it — despawning any link caught
/// inside the slab it stamped — and it retired with `resources::Platform`:
/// the base is out of phase now, so nothing a build does can catch a link
/// under it any more. `spawn_surface_links` skipping `Biome::Platform` is
/// dead for the same reason (nothing ever produces that biome on a
/// `WorldMap` again); a base-space replacement for both is slice 2/3's.
pub const STACK_LINK_SCATTER_TILES: i32 = 40;

/// How far past the base's own edge the *first* link of a zone is placed —
/// the width of the ring `Game::spawn_surface_links` draws its on-ramp from,
/// starting one tile outside the slab.
///
/// It means "on your doorstep", and it used to mean "on screen": the pane
/// shows roughly ±16 by ±9 tiles at the default zoom, and a link inside that
/// was how a player with no reason to think links exist found the first one.
/// That promise cannot survive a base that grows — at
/// `MAX_BUILD_RADIUS_TILES` the slab has eaten the viewport itself, so the
/// nearest ground a link may stand on is already past the bottom of the
/// pane. `Game::announce_surface_links` is what keeps the layer
/// discoverable now: the arrival scan reports how many links the sector has
/// and which way the nearest one lies, which does not depend on where the
/// pane happens to end. The other two links are still a trip
/// (`STACK_LINK_SCATTER_TILES`).
pub const STACK_NEAREST_LINK_TILES: i32 = 8;

/// How close a link may get to where the player materializes.
///
/// Without a floor, `STACK_NEAREST_LINK_TILES` can put one on the
/// arrival tile itself — the player starts standing on a link — or one
/// step from it, so the first movement key of the run drops them into the
/// Stack they never chose to enter. It also keeps links off the tiles a
/// base's first few structures go on, which would otherwise be refused with
/// "there's a link here" for no reason the player could have foreseen.
pub const STACK_MIN_LINK_TILES: i32 = 5;

/// How far out `Game::collapse_stack` will look for somewhere to put the
/// link that replaces the one a beaten stack takes down with it.
///
/// The search walks Chebyshev rings outward from the collapsed tile and
/// takes the first legal one, so this is a *bound on the search* rather than
/// a distance the replacement is placed at: on ordinary ground it lands
/// within a ring or two, and the reading the collapse wants is that the
/// ground shifted right here rather than that a hole opened somewhere across
/// the sector.
///
/// The bound exists because failing has to be possible to reason about. A
/// zone with no link left is a run that can never breach again — `award_loot`
/// underground is the game's only source of Portal Fragments — so a search
/// that found nothing skips the collapse entirely and leaves the old link
/// standing. Twenty rings is thousands of candidate tiles, which makes that
/// branch a fail-safe rather than something a player will meet.
pub const STACK_COLLAPSE_RELINK_TILES: i32 = 20;

/// How many frames the shallowest stack runs before bottoming out.
///
/// Two rather than one so even the on-ramp link has a descent in it: a
/// single-frame stack is a room with a boss in it, and the thing being built
/// here is the Stack.
pub const STACK_FRAMES_MIN: u32 = 2;

/// The deepest a stack can run, however far out it sits.
///
/// A cap rather than an open curve because the frame count is what the
/// player commits to when they start down — six frames of walking back up is
/// already a long way from the surface with a hurt party.
pub const STACK_FRAMES_MAX: u32 = 6;

/// How many tiles from the zone's arrival point buys one more frame of
/// depth — see `frames_for`.
///
/// Depth rides on the same distance that already scales wild program stats
/// (`Game::distance_stat_multiplier`), so the two agree instead of pulling
/// against each other: a far link is deeper *and* fields harder programs,
/// and the player can read both off how long the walk there was. With
/// `STACK_LINK_SCATTER_TILES` at 40, this puts the outermost links
/// at the cap.
pub const STACK_TILES_PER_FRAME: i32 = 8;

/// How many caches a Stack frame hides — see `stack::place_caches`,
/// which puts them in the dead ends the braid pass left behind.
///
/// A reward rate, so it lives here rather than beside the generator with
/// `BRAID_PERCENT`: how *many* is balance, where *dead ends specifically* is
/// the shape of the content.
pub const STACK_CACHES_PER_FRAME: usize = 3;

/// How many plain doorways a Stack frame hangs — see
/// `stack::place_doors`.
///
/// Presentation as much as balance: a door blocks the view cone, so it turns
/// a corridor into a decision. Enough to break up a frame, few enough that
/// the maze doesn't read as a series of closed boxes.
pub const STACK_DOORS_PER_FRAME: usize = 4;

/// How many breakpoints a Stack frame exposes — see
/// `stack::place_breakpoint`, which puts them on junctions.
///
/// One. A breakpoint maps the entire frame, so the second one in a frame has
/// nothing left to show you; more than one would only ever be a shorter walk
/// to the same reward. `FrameMemory::jacked` is a set rather than a bool
/// anyway, so raising this is a one-line change if playtest disagrees.
pub const STACK_BREAKPOINTS_PER_FRAME: usize = 1;

/// The odds a jack-in resolves the whole frame — see `Game::trip_breakpoint`.
///
/// A port used to be a guaranteed map for a walk and 25 Trace, which made
/// the only decision it asked "is the walk shorter than mapping on foot".
/// At 0.6 it asks a better one: the Trace is spent the moment you jack in
/// and the map is what you might get for it. There is exactly one port per
/// frame (above) and a spent one stays spent whichever way the roll went, so
/// this is one try per frame and not a slot machine.
pub const STACK_BREAKPOINT_CHANCE: f64 = 0.6;

/// How far around the party a failed jack-in resolves, in cells, as a square
/// — see `Game::trip_breakpoint`.
///
/// The consolation, so it is deliberately much less than the walk it would
/// have saved: 7x7 of a 21x21 frame, and centred on the party rather than
/// aimed anywhere useful. It is worth taking anyway because it sees *through*
/// walls, which the view cone never does — a failed jack tells you which way
/// the junction you are standing on actually goes.
pub const STACK_BREAKPOINT_PARTIAL_RADIUS: i32 = 3;

/// How many faults a Stack frame drops through — see `stack::place_faults`.
///
/// Never generated on the bottom frame, which has nothing below it, so the
/// deepest frame of every stack has zero regardless of this number.
pub const STACK_FAULTS_PER_FRAME: usize = 1;

/// How many separate corrupted stretches a Stack frame grows, and how many
/// cells each runs to — see `stack::place_corruption`.
///
/// Patches rather than scattered cells, and this is the whole point of the
/// pair: a lone corrupted cell is a toll booth you pay and forget, where a
/// stretch is something you can decide to walk around. Two patches of three
/// against a frame of roughly a hundred walkable cells is sparse enough that
/// most routes miss them entirely, which is what makes hitting one a
/// decision rather than a tax.
pub const STACK_CORRUPTION_PATCHES_PER_FRAME: usize = 2;
pub const STACK_CORRUPTION_PATCH_CELLS: usize = 3;

/// How many orphaned processes a Stack frame leaves running — see
/// `stack::place_orphan`, which puts them in the dead ends the caches left.
///
/// A ceiling rather than a promise: the pass runs after `place_caches` and
/// wants the same site type, so a frame needs four plain-floor dead ends to
/// field one and about a quarter of them haven't got that many. The
/// measured rate is pinned by
/// `most_frames_place_an_orphan_and_none_places_two`.
///
/// One, and the supply is not what limits this. Each costs the player a
/// taming catalyst, which is cheap; what actually binds is
/// `BASE_PET_CAPACITY`, which is **3** slots. Six frames of a full stack
/// therefore offer four or five programs to a roster with three slots, so a
/// descent with no base standing behind it comes home with programs that
/// have nowhere to run — adopted, since slots are not a door, and earning
/// `unslotted` until the base makes room.
///
/// That is pressure toward capacity-granting structures, and it is meant
/// to be: the Data Cache's `pet_slot_bonus` was raised to **5** so that one
/// of them — ten Core Fragments, buildable before the first descent —
/// slots a whole stack's worth of orphans. `balance_sim` models no roster
/// and cannot gate any of that.
pub const STACK_ORPHANS_PER_FRAME: usize = 1;

/// What one step onto corrupted substrate costs, as a fraction of the
/// player's maximum HP, with `STACK_CORRUPTION_MIN_DAMAGE` as a floor.
///
/// A fraction rather than a flat figure, and rather than the depth scaling
/// `STACK_CACHE_DEPTH_GROWTH` uses, because Stack depth is uncorrelated with
/// player level: the party is 90 HP at level 1 (`PLAYER_BASE_STATS`) and
/// around 510 by mid-run, so any flat number is lethal at one end and free at
/// the other. At 10%, a three-cell patch costs about a third of the bar
/// wherever the party is in the run.
///
/// Raised from 3% on 2026-08-13, and the raise changes what the terrain *is*
/// rather than merely what it costs. At 3% a patch was a toll: crossing one
/// could not kill, so the only question was whether the detour was longer
/// than a tenth of the bar. At 10% a crossing cannot kill a party at full
/// health either — but a wounded one, or one that crosses back, dies, and
/// that death goes through `Game::apply_damage` into
/// `difficulty::death_handling_system` like any other. So the route around
/// is now a decision the party can get wrong, which is the point.
/// `balance_sim` has no Stack term at all and cannot gate any of this.
pub const STACK_CORRUPTION_HP_PERCENT: f32 = 0.10;
pub const STACK_CORRUPTION_MIN_DAMAGE: i32 = 2;

/// Credits a cache holds at depth 1, before `STACK_CACHE_DEPTH_GROWTH`.
///
/// Credits rather than Core Fragments because a Stack run should pay for
/// itself in the one currency that survives a breach — see
/// `EconomyRole::TradeCurrency`.
///
/// A cache used to also roll for a portal fragment. It no longer does: the
/// breaching currency is `STACK_BOSS_PORTAL_FRAGMENT_DROP`'s alone, so a
/// stack's progress toward the next zone is what the party fights the lair
/// for and not what they find in the walls on the way to it.
pub const STACK_CACHE_CREDITS: std::ops::RangeInclusive<u32> = 12..=30;

/// What each frame of depth multiplies a cache's credit payout by,
/// compounding. Deliberately steeper than `STACK_DEPTH_STAT_GROWTH`: going
/// deeper has to pay better than it costs, or the bottom of a stack is a
/// place with no reason to visit it.
pub const STACK_CACHE_DEPTH_GROWTH: f32 = 1.5;

/// Roughly one cell in this many is one the corridor speaks from as the
/// party walks through it — see `Game::narrates_passage`.
///
/// A cadence rather than a line per step. `Game::arrive` fires on every step
/// that covers ground, and the log pane draws a handful of rows, so
/// narrating unconditionally would bury the sighting line, the base's news
/// and the encounter roll under a wall of corridor. Narrating never was the
/// state this exists to leave: the `sighted` pools of `stack.floor` and
/// `stack.door` were authored with the rest of the bank and were unreachable
/// in play, because `Game::notability` — the discovery axis's gate — ranks
/// neither.
///
/// **Which cells speak is a property of the place**, derived from the
/// frame spec and the cell's own coordinates exactly as the words are, so
/// the same corridor speaks at the same corners after a save and reload and
/// on every later walk through it. That is what makes this a rhythm the
/// player can come to know rather than noise, and it is why the figure is a
/// divisor over a fold rather than a probability over `resources::GameRng`.
///
/// Three is a guess. It is a feel question — `balance_sim` has no Stack term
/// at all and models no log — so only play answers it.
pub const STACK_PASSAGE_NARRATION_ONE_IN: usize = 3;
