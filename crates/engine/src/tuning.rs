//! Every gameplay difficulty knob the engine hardcodes, in one place.
//!
//! Change a number here and the game gets easier or harder — that is the
//! whole reason this module exists. Nothing here is referenced by an
//! identifier the save format or the `.ron` schemas depend on, so a value
//! can be retuned freely; only the *shape* of a formula lives in the module
//! that uses it.
//!
//! What is deliberately **not** here:
//!
//! - **Anything already expressed as data.** Species stats, item and craft
//!   costs, structure economy, research costs and ability magnitudes are
//!   `.ron` files under `assets/`. Tune those by editing the files — see
//!   each directory's `README.md`. Pulling them into Rust would break
//!   moddability.
//! - **Type invariants.** `components::NEED_MAX`/`NEED_MIN` bound what
//!   `Needs` may hold; every reader assumes they hold. They live beside the
//!   type they constrain.
//! - **Infrastructure.** `world::CHUNK_SIZE`, `save::SAVE_FORMAT_VERSION`,
//!   `resources::MESSAGE_LOG_CAP`/`EFFECT_QUEUE_CAP`, `MAX_CUSTOM_NAME_LEN`
//!   and the `*_ID` string identifiers are sizing and plumbing, not
//!   difficulty.
//! - **Simulation-only values.** `balance_sim::TURN_CAP` and the guard
//!   constants beside it tune the offline projections, not the game.

use crate::components::Stats;

// ─────────────────────────────────────────────────────────────────────────
// Player baseline & progression
// ─────────────────────────────────────────────────────────────────────────

/// The player's stats at level 1, before any leveling or gear — the seed
/// value `Game::new` spawns the player with, and the baseline `balance_sim`'s
/// projections grow from, so both stay in lockstep.
pub const PLAYER_BASE_STATS: Stats = Stats {
    hp: 90,
    max_hp: 90,
    atk: 6,
    def: 2,
};

/// Flat stat growth per level-up, before `growth_multiplier` scales it —
/// see `progression::stats_after_levels`. With ATK and DEF both at 1, a
/// multiplier has to cross a rounding boundary (roughly +0.5) to change
/// them at all, so `HP_PER_LEVEL` carries most of the effective
/// granularity.
pub const HP_PER_LEVEL: i32 = 12;
pub const ATK_PER_LEVEL: i32 = 1;
pub const DEF_PER_LEVEL: i32 = 1;

/// Growth-rate multiplier for anything with no species-specific rate of
/// its own. The player (who has no species at all) always levels at this
/// rate; it's also `SpeciesDef::growth_multiplier`'s default, so a species
/// file written before that field existed keeps growing exactly as before.
pub const BASELINE_GROWTH_MULTIPLIER: f32 = 1.0;

/// Level ceiling for a *creature* (tamed or wild), regardless of XP
/// source. `add_xp` stops leveling — and stops accumulating XP at all —
/// once this is reached, so a maxed-out creature's `Experience::xp` just
/// stalls instead of piling up into a huge, meaningless number.
///
/// The player is deliberately **not** capped: they keep leveling forever,
/// so late-game progression stays open-ended and a long run always earns
/// something. Callers express that by passing `None` as `add_xp`'s
/// `level_cap` for the player and `Some(CREATURE_MAX_LEVEL)` for
/// creatures.
///
/// This is a live-gameplay cap only: it deliberately doesn't apply to
/// `crate::balance_sim`'s offline curve-shape projections, which search well
/// past any level actually reachable in play on purpose (see that
/// module's docs).
pub const CREATURE_MAX_LEVEL: u32 = 12;

/// Fraction of in-level XP knocked back by a "setback" penalty (a flatline,
/// a Forgiving-mode reboot, or a forced jack-out mid-battle) — see
/// `progression::apply_setback_xp_penalty`. Deliberately mild: it erodes
/// progress toward the next level, never the level or stats themselves.
pub const SETBACK_XP_PENALTY_FRACTION: f64 = 0.2;

/// How much the player's `Decompiler` skill grows per level gained.
pub const DECOMPILER_SKILL_PER_LEVEL: i32 = 1;

/// Perk Points (see `perks::Perk`) awarded per player level gained.
pub const PERK_POINTS_PER_LEVEL: u32 = 1;

/// Every party member (see `resources::Party`) gains `1 / PARTY_XP_DIVISOR`
/// of whatever XP the player just earned from a kill or successful
/// decompile — see `Game::award_party_xp`.
pub const PARTY_XP_DIVISOR: u32 = 2;

/// XP required to advance from level *N* to *N + 1* is `N` times this — a
/// linear per-level step, so cumulative XP to reach a level grows
/// quadratically. `balance_sim`'s companion-level projection leans on that
/// quadratic shape: half the XP rate lands a companion at roughly
/// `1 / sqrt(2)` of the player's level, not half of it.
pub const XP_PER_LEVEL_STEP: u32 = 20;

/// XP a tamed creature earns for each completed gather cycle.
pub const WORK_XP_PER_CYCLE: u32 = 5;

/// A cronjob worker stops earning XP from `task_progress_system` once it
/// reaches this level — structure work is meant to be a steady, low-effort
/// income, not a way to grind a pet's level without ever battling. Levels
/// above this only come from combat (`Game::award_player_xp` /
/// `award_party_xp`), up to the separate, higher ceiling every creature
/// shares — see `CREATURE_MAX_LEVEL`.
pub const WORK_XP_LEVEL_CAP: u32 = 10;

// ─────────────────────────────────────────────────────────────────────────
// Zone & distance scaling
// ─────────────────────────────────────────────────────────────────────────

/// Geometric base for `ZoneLevel::stat_multiplier`, the flat multiplier on
/// every wild program's stats in a zone: zone 1 is x1 and each level after
/// multiplies by this (1, 2, 4, 8, 16, ...). The single steepest difficulty
/// curve in the game — `GEAR_LEVEL_GROWTH` is deliberately matched to it so
/// neither gear nor zone depth outruns the other.
pub const ZONE_STAT_GROWTH: i32 = 2;

/// Tile distance per step of `DISTANCE_STAT_STEP_BONUS`, counted from
/// `Game::distance_from_danger_origin` — the base platform's edge once a
/// Home exists, `ZoneSpawnPoint` before then. See
/// `Game::distance_stat_multiplier`.
pub const DISTANCE_STAT_STEP_TILES: i32 = 15;

/// Stat growth added per `DISTANCE_STAT_STEP_TILES` step away from the
/// zone's spawn point, on top of `ZoneLevel::stat_multiplier` — a gentler,
/// linear (not doubling) knob than zone depth, since it's optional
/// distance covered within a zone you can always retreat from, not a
/// one-way commitment like breaching deeper.
pub const DISTANCE_STAT_STEP_BONUS: f32 = 0.25;

/// Cap on `distance_stat_multiplier`, so wandering far enough doesn't
/// scale stats forever within a single zone — unlike zone depth, which
/// really is unbounded.
pub const MAX_DISTANCE_STAT_MULTIPLIER: f32 = 3.0;

/// Tile distance per doubling of a wild group's size, counted from the same
/// origin as `DISTANCE_STAT_STEP_TILES` (the platform's edge once a Home
/// exists) — see `Game::max_group_size`. Deliberately equal to
/// `DISTANCE_STAT_STEP_TILES`: how many programs meet you and how hard each
/// one hits escalate on the same footing as you push out.
pub const GROUP_SIZE_STEP_TILES: i32 = DISTANCE_STAT_STEP_TILES;

/// Geometric base for the group-size growth `GROUP_SIZE_STEP_TILES` steps
/// unlock, and the cap on how many of those steps count. The exponent has
/// to be clamped because the map is unbounded and a shift of 32 or more is
/// a panic in debug; `MAX_GROUP_SIZE_DISTANCE_STEPS` is set where
/// `GROUP_SIZE_DISTANCE_GROWTH.pow(steps)` already exceeds `MAX_GROUP_SIZE`,
/// so the clamp is exact rather than a fudge.
pub const GROUP_SIZE_DISTANCE_GROWTH: u32 = 2;
pub const MAX_GROUP_SIZE_DISTANCE_STEPS: u32 = 7;

/// Frames descended per escalation step in the Stack — the underground
/// counterpart to `GROUP_SIZE_STEP_TILES`, feeding the same curve through
/// `Game::danger_steps`.
///
/// One frame per step, against fifteen tiles per step on the surface,
/// because descending *is* the commitment that walking out is: a frame is
/// a maze to cross and a one-way door at the bottom of it, where fifteen
/// tiles is a few turns' walk you can turn around in. The party also
/// arrives at depth 1 already having chosen to be there, so the first frame
/// stays at step zero — the entrance is the Stack's own opening ring.
pub const GROUP_SIZE_STEP_FRAMES: u32 = 1;

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

/// Hard ceiling on a single species group. With `MAX_ENEMY_GROUPS` groups on
/// the field, one intrusion tops out at four hundred programs.
pub const MAX_GROUP_SIZE: u32 = 100;

/// How many distinct species groups can engage in one intrusion. A cluster
/// with more species than this engages its largest groups and leaves the
/// remainder standing on the map as ordinary hostiles — they're met on the
/// next bump rather than silently despawned.
pub const MAX_ENEMY_GROUPS: usize = 4;

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
/// upper bounds on a hostile program's power (see `Stats::power`) relative
/// to the player's own — anything at or under `DIFFICULTY_EASY_MAX` reads
/// Green, up through `DIFFICULTY_EVEN_MAX` reads Yellow, up through
/// `DIFFICULTY_TOUGH_MAX` reads Orange, and anything above that reads Red.
pub const DIFFICULTY_EASY_MAX: f64 = 0.7;
pub const DIFFICULTY_EVEN_MAX: f64 = 1.1;
pub const DIFFICULTY_TOUGH_MAX: f64 = 1.6;

// ─────────────────────────────────────────────────────────────────────────
// Combat
// ─────────────────────────────────────────────────────────────────────────

/// Relative weight each party member carries in a wild program's target
/// roll. Ranks are *soft*: everyone stays targetable, slot order only
/// changes the odds — a back-slot member is hit
/// `FRONT_SLOT_AGGRO_WEIGHT / BACK_SLOT_AGGRO_WEIGHT` times less often than
/// a front-slot one, never zero times. Bracing (see `Game::begin_defend`)
/// adds `DEFEND_AGGRO_WEIGHT` on top, which is what makes Defend a
/// party-level play rather than a selfish one.
pub const FRONT_SLOT_AGGRO_WEIGHT: u32 = 3;
pub const BACK_SLOT_AGGRO_WEIGHT: u32 = 1;
pub const DEFEND_AGGRO_WEIGHT: u32 = 4;

/// How many party slots count as the front line for `FRONT_SLOT_AGGRO_WEIGHT`
/// — the player plus the first two companions.
pub const FRONT_SLOTS: usize = 3;

/// Initiative baseline for a species whose `.ron` file omits `base_speed` —
/// the midpoint of the shipped roster's range, so an un-annotated mod
/// species is neither free initiative nor dead weight.
pub const DEFAULT_BASE_SPEED: i32 = 10;

/// The player's initiative baseline. A shade above `DEFAULT_BASE_SPEED`: the
/// player acts first against an average opponent, but loses the roll to
/// anything genuinely fast.
pub const PLAYER_BASE_SPEED: i32 = 11;

/// Each round every combatant rolls `base_speed + rng(0..=INITIATIVE_DIE)`
/// and acts in descending order. Sized so a 4-point speed gap still loses
/// the roll sometimes — order should be a tendency, not a lookup table.
pub const INITIATIVE_DIE: i32 = 10;

/// Move power behind the player's own basic strike. The player has no
/// `Creature` component and so no species moveset — this is their one move,
/// with `Stats::atk` and equipment carrying the rest of the scaling.
pub const PLAYER_STRIKE_POWER: i32 = 5;

/// How often a wild program reaches for the status effect on the move it
/// just used — see `Game::wild_retaliate`.
///
/// Wild programs used to attempt their move's effect every single turn, so
/// a species with a nasty stun was that stun on repeat. At 20% a moveset
/// reads as something a program *can* bring to bear rather than all it does.
///
/// Gates the effect only; the move still lands its full damage. That is
/// deliberate — it changes how a fight feels without touching the damage
/// curves `balance_sim` projects, so retuning it does not move them.
///
/// Composes with each move's own `effect.chance` (`.ron` data, 0.3-0.5 on
/// the shipped roster) rather than replacing it, so an effect actually
/// lands on roughly 6-10% of wild attacks.
pub const WILD_ABILITY_CHANCE: f64 = 0.2;

/// DEF granted for the round by the Defend action.
pub const DEFEND_DEF_BONUS: i32 = 6;

/// Fatigue the player spends each time they command a companion in battle
/// (see `BattleAction::Special`) — the rally/special-ability
/// bonus isn't free, whichever kind the companion has.
pub const COMPANION_COMMAND_FATIGUE_COST: f32 = 5.0;

/// Below this Power ("Power" is the player-facing label for `Needs.hunger`)
/// threshold, the player's own attacks start losing effectiveness — see
/// `battle::power_attack_multiplier`.
pub const LOW_POWER_ATTACK_THRESHOLD: f32 = 50.0;

/// Floor under a single attack's damage, so battles can't stall out on a
/// high-defense matchup where `move_power + atk - def` goes non-positive.
pub const MIN_DAMAGE: i32 = 1;

/// What the player's attack total falls to at zero Power. Between zero and
/// `LOW_POWER_ATTACK_THRESHOLD` the multiplier interpolates linearly from
/// this up to full strength — see `battle::power_attack_multiplier`.
pub const LOW_POWER_MIN_ATTACK_MULTIPLIER: f32 = 0.5;

/// Divisor on each party member's ATK and DEF when totalling the passive
/// bonus they lend the player outside their own actions (see
/// `Game::party_stat_bonus`), floored at 1 per member so every companion
/// contributes something. Companions already act in their own right; this
/// is the smaller standing bonus on top.
pub const PARTY_PASSIVE_STAT_DIVISOR: i32 = 10;

/// Chance that a *successful* jack-out still costs the player a parting
/// counter-strike. Whether the escape happens at all is a separate roll —
/// see `battle::jack_out_chance`.
pub const FLEE_COUNTERATTACK_CHANCE: f64 = 0.5;

/// Coefficients of `battle::jack_out_chance`. The base is the escape chance
/// at an even matchup — where your side's summed `Stats::power` equals
/// theirs — before the luck roll, so running from a fair fight usually
/// works. The chance scales linearly with that power ratio: outnumbered and
/// outgunned, you are much likelier to be pinned.
pub const JACK_OUT_BASE_CHANCE: f64 = 0.6;

/// Uniform random multiplier drawn fresh on every jack-out attempt, so the
/// odds are never a lookup: a hopeless-looking escape sometimes works and a
/// favourable one sometimes doesn't. Same spread as the per-creature stat
/// roll below, and for the same reason — enough wobble to matter, not
/// enough to overturn the matchup.
pub const JACK_OUT_LUCK_MIN: f64 = 0.8;
pub const JACK_OUT_LUCK_MAX: f64 = 1.2;

/// Hard bounds on the final jack-out chance, applied after the ratio and
/// the luck roll. Mirrors `CAPTURE_CHANCE_MIN`/`MAX`: no escape is ever
/// hopeless and none is ever certain. The floor is what keeps an ambush by
/// an overwhelming pack survivable — expensive, since every failed attempt
/// costs a full enemy volley, but never a guaranteed death.
pub const JACK_OUT_CHANCE_MIN: f64 = 0.10;
pub const JACK_OUT_CHANCE_MAX: f64 = 0.95;

/// Uniform random-roll range applied independently to each of a newly
/// created creature's stats (baked into `Stats` at spawn) and to its
/// growth rate (`Potential::growth_roll`) — see `Game::roll_potential`.
/// The "same species, different stats" mechanic; doesn't apply to the
/// player, who has no species.
pub const MIN_INDIVIDUAL_ROLL: f32 = 0.8;
pub const MAX_INDIVIDUAL_ROLL: f32 = 1.2;

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
/// cannot exceed 0.33 — the ICE Breaker's `taming_potency` of 0.4 is the
/// strongest catalyst that ships. As a flat addend, skill 40 was worth +0.80
/// and pinned every attempt to `CAPTURE_CHANCE_MAX` regardless of species,
/// leaving `taming_difficulty` and weakening the target both meaningless.
/// Scaling the base instead keeps them inside what's being multiplied, so
/// they matter at every skill level.
pub const DECOMPILER_SKILL_BONUS: f32 = 0.02;

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

/// Hard bounds on the final decompile chance, applied after skill bonuses.
/// No attempt is ever hopeless and none is ever certain.
///
/// These are safety rails, not balance levers — in particular the maximum is
/// unreachable with the content that ships and shouldn't be reached for when
/// tuning. The only catalyst is the ICE Breaker at `taming_potency` 0.4, so
/// the best base the formula can produce is a fully-drained Drone at
/// `0.4 * 0.9 * 0.91 = 0.328`; clearing 0.95 would need a 2.9x skill
/// multiplier, i.e. `Decompiler` skill 95, against a realistic ceiling near
/// 38 (level 30 plus the best 8 points of gear). See
/// `high_skill_does_not_flatten_the_gap_between_easy_and_boss_species`.
pub const CAPTURE_CHANCE_MIN: f32 = 0.05;
pub const CAPTURE_CHANCE_MAX: f32 = 0.95;

/// `SpeciesDef::taming_difficulty` assumed for a species that has gone
/// missing from the db mid-battle — dead centre of the 0..=1 range, so a
/// lookup failure neither gifts nor denies the capture.
pub const DEFAULT_TAMING_DIFFICULTY: f32 = 0.5;

// ─────────────────────────────────────────────────────────────────────────
// Spawning & encounters
// ─────────────────────────────────────────────────────────────────────────

/// Chance per tick that a wild spawn roll fires at all (see
/// `Game::maybe_spawn_wild_creature`), and the box radius around the player
/// the roll places into. The radius is wide enough that a spawn lands
/// off-screen and is walked into rather than appearing on top of you.
pub const WILD_SPAWN_CHANCE: f64 = 0.05;
pub const WILD_SPAWN_RADIUS_TILES: i32 = 12;

/// Chance per walked step that the player is ambushed — a biome-appropriate
/// pack spawns adjacent and engages immediately, with no chance to route
/// around it (see `Game::maybe_ambush`). Deliberately an order of magnitude
/// rarer than an ordinary encounter feels like it should be: the map is
/// already full of programs you can choose to fight, so this is the tax on
/// crossing open ground rather than the main source of battles. Not rolled
/// on base platform tiles, and never produces a boss or a nest.
pub const RANDOM_ENCOUNTER_CHANCE: f64 = 0.02;

/// How many wild programs a zone is seeded with on entry, before any
/// per-tick spawn rolls — see `Game::spawn_initial_creatures`. Applies both
/// to a new run and to every zone breached afterwards, so a fresh sector is
/// never empty while the spawn rolls warm up.
pub const INITIAL_WILD_POPULATION: usize = 14;

/// How far from the player a zone's opening wild programs scatter (see
/// `Game::spawn_initial_creatures`). Widened by the platform radius when
/// the player has a base, since nothing can spawn on platform floor.
pub const INITIAL_SPAWN_SCATTER_TILES: i32 = 15;

/// How many Stack links a zone is seeded with — see
/// `Game::spawn_surface_links`. Deliberately few: a link is
/// something you go looking for, and one on every corner would make the
/// zone map a lobby rather than a place.
pub const STACK_LINKS_PER_ZONE: usize = 3;

/// How far from the player's arrival point a zone's Stack links
/// scatter. Wider than `INITIAL_SPAWN_SCATTER_TILES` so finding one is a
/// trip rather than a glance.
///
/// It does *not* keep links off the base platform, though it used to claim
/// so: `STACK_NEAREST_LINK_TILES` places the first link of every zone 5-8
/// tiles out, against a slab of `MAX_BUILD_DISTANCE_FROM_HOME` 7. What
/// actually holds that line is `Game::stamp_platform`, which despawns any
/// link inside the slab, plus `spawn_surface_links` skipping
/// `Biome::Platform` when the platform already exists.
pub const STACK_LINK_SCATTER_TILES: i32 = 40;

/// How close the *first* link of a zone is placed.
///
/// Measured against the map viewport rather than picked for feel: at the
/// default zoom the pane shows roughly ±16 by ±9 tiles, so anything past
/// this is off screen when the player materializes. With all three links
/// scattered to `STACK_LINK_SCATTER_TILES`, most seeds put every one
/// of them out of sight, and a player with no reason to think links exist
/// has no reason to go looking. One always within the opening view is the
/// on-ramp; the other two are still a trip.
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
/// `BASE_PET_CAPACITY`, which is **3**. Six frames of a full stack therefore
/// offer four or five programs to a roster that holds three, so a thorough
/// descent still refuses at the end. Whether that reads as pressure toward
/// capacity-granting structures or as a dead mechanic is the one question
/// phase 4 shipped without an answer to — `balance_sim` models no roster and
/// cannot gate it.
pub const STACK_ORPHANS_PER_FRAME: usize = 1;

/// What one step onto corrupted substrate costs, as a fraction of the
/// player's maximum HP, with `STACK_CORRUPTION_MIN_DAMAGE` as a floor.
///
/// A fraction rather than a flat figure, and rather than the depth scaling
/// `STACK_CACHE_DEPTH_GROWTH` uses, because Stack depth is uncorrelated with
/// player level: the party is 90 HP at level 1 (`PLAYER_BASE_STATS`) and
/// around 510 by mid-run, so any flat number is lethal at one end and free at
/// the other. At 3%, a three-cell patch costs about a tenth of the bar
/// wherever the party is in the run.
pub const STACK_CORRUPTION_HP_PERCENT: f32 = 0.03;
pub const STACK_CORRUPTION_MIN_DAMAGE: i32 = 2;

/// Credits a cache holds at depth 1, before `STACK_CACHE_DEPTH_GROWTH`.
///
/// Credits rather than Core Fragments because a Stack run should pay for
/// itself in the one currency that survives a breach — see
/// `EconomyRole::TradeCurrency`. Fragments are what the surface is for.
pub const STACK_CACHE_CREDITS: std::ops::RangeInclusive<u32> = 12..=30;

/// What each frame of depth multiplies a cache's credit payout by,
/// compounding. Deliberately steeper than `STACK_DEPTH_STAT_GROWTH`: going
/// deeper has to pay better than it costs, or the bottom of a stack is a
/// place with no reason to visit it.
pub const STACK_CACHE_DEPTH_GROWTH: f32 = 1.5;

/// Chance a cache also holds a portal fragment, per frame of depth (so
/// depth 3 rolls at three times this, capped at certainty).
///
/// The one route besides the Market listing and boss kills that pays the
/// breaching currency — see `market-portal-fragment-listing-is-load-bearing`.
pub const STACK_CACHE_FRAGMENT_CHANCE: f64 = 0.12;

/// Chance per step that walking a Stack corridor draws an encounter.
///
/// Much higher than `RANDOM_ENCOUNTER_CHANCE` on purpose: crossing open
/// ground is travel that fighting interrupts, but the Stack is somewhere you
/// go *to* fight. It is also what makes mapping one tense — every corridor
/// you walk to find the way down is a corridor that can cost you.
///
/// Arithmetic-plausible only, never playtested. Measured 2026-08-01: a frame
/// is 21x21 with **~206 walkable cells**, so a direct route to the way down
/// is 40-80 steps — three to six fights — while an exhaustive crawl with
/// backtracking is nearer 300, or **~24 fights against the frame's 3
/// caches**.
///
/// That second number is what the Trace gains below are shaped around. A
/// meter that paid per kill anything like what it pays per cache would be
/// driven almost entirely by combat, and would feed itself: more Trace,
/// more encounters, more kills. See `TRACE_PER_KILL`.
pub const STACK_ENCOUNTER_CHANCE: f64 = 0.08;

/// What each frame of Stack depth multiplies wild program stats by,
/// compounding, on top of `ZoneLevel::stat_multiplier` and
/// `Game::distance_stat_multiplier`.
///
/// A float and much gentler than `ZONE_STAT_GROWTH`'s flat doubling, because
/// descending is cheap — a link down, not a Portal you had to fund — so the
/// curve has to be walkable rather than a wall. XP follows for free:
/// a kill pays the defeated program's `max_hp`, so scaling stats scales the
/// reward with the risk.
pub const STACK_DEPTH_STAT_GROWTH: f32 = 1.35;

// ---- The Stack: Trace ------------------------------------------------
//
// Trace rises with what the party *takes* from a stack and escalates what
// comes for them. The design argument for every number below is in
// `docs/superpowers/specs/2026-07-31-the-stack-design.md`, "Phase 2".
//
// Walking is free, deliberately. A time-driven meter would tax exploration
// and map-making, rewarding the beeline and punishing the careful player —
// backwards for a maze whose per-frame map memory exists to reward
// learning it.

/// Trace for cracking a cache. The dominant source, and meant to be: a
/// frame holds exactly three, and the payout scales with depth, so the
/// meter and the reward rise together.
pub const TRACE_PER_CACHE: u32 = 10;

/// Trace for burning a seal. Near-negligible in practice — a stack holds
/// two, both on the bottom frame walling off the lair — but burning one
/// costs an access shard, and a cost the player paid should register.
pub const TRACE_PER_SEAL: u32 = 5;

/// Trace per hostile killed. **A fifth of a cache, and that ratio is the
/// load-bearing part**, not the absolute value. Kills are the
/// high-frequency source by an order of magnitude (see
/// `STACK_ENCOUNTER_CHANCE`), so paying them near cache rates would make
/// Trace a combat meter that feeds its own input.
pub const TRACE_PER_KILL: u32 = 2;

/// Trace for jacking into a breakpoint. **The single loudest thing the party
/// can do**, at two and a half caches: a breakpoint hands over the whole
/// frame's map at a stroke, and announcing yourself to the substrate is what
/// it costs.
///
/// Held at 25 through the 2026-08-01 retune, which changed what it *means*:
/// against `TRACE_NOTICED` at 40 this was one breakpoint plus two caches to
/// cross the first band, and against 25 a breakpoint alone crosses it on the
/// spot. That is the better reading of "the loudest thing you can do" — the
/// map is free and being seen taking it is immediate — so the number stayed
/// where it was and the argument for it changed underneath.
///
/// Still unplayed as a *decision*: the one crawl on record never used a
/// breakpoint at all, so whether anyone pays this is unmeasured.
pub const TRACE_PER_BREAKPOINT: u32 = 25;

/// Where each band begins. Half-open: a value sitting exactly on a
/// threshold is in the band it names.
///
/// Sized so that a thorough player — ~60 Trace per frame, from three
/// caches and the fights a 120-step crawl draws — arrives at the lair
/// **Hunted**, while a beeliner arrives around **Noticed**. That
/// difference is the question the descent is supposed to ask.
///
/// **Retuned 2026-08-01 from 40/100/180, on the first crawl anyone has
/// played.** The session cracked a cache and took four fights across about
/// a third of a frame and never left **Quiet** — and working back from that
/// showed the real fault, which is arithmetic rather than a matter of
/// taste: a frame holds `STACK_CACHES_PER_FRAME` caches at
/// `TRACE_PER_CACHE` each, so **stripping a whole floor of every cache in
/// it came to 30 against a first band at 40**. Maximal greed on an entire
/// frame produced no feedback at all, and a meter nobody can make move
/// cannot teach what it is for. `stripping_a_frames_caches_is_enough_to_be
/// _noticed` now pins that, so the relationship survives a future change to
/// either constant.
///
/// Only `TRACE_NOTICED` is evidence-backed. The upper two are re-derived
/// from the same ~60-per-frame model that produced the originals — one
/// thorough frame lands solidly in Noticed, the second reaches Traced, the
/// third Hunted — and are still unplayed, because that session never came
/// close to either.
pub const TRACE_NOTICED: u32 = 25;
pub const TRACE_TRACED: u32 = 70;
pub const TRACE_HUNTED: u32 = 140;

/// Per-band multiplier on `STACK_ENCOUNTER_CHANCE`, indexed by
/// `TraceBand::index`.
///
/// The gentlest of the three deliberately: it is the only lever that feeds
/// back into its own input, since more encounters mean more kills mean
/// more Trace. The teeth go in `TRACE_STAT_MULT`, which feeds back into
/// nothing.
pub const TRACE_ENCOUNTER_MULT: [f64; 4] = [1.0, 1.25, 1.6, 2.0];

/// Per-band multiplier on enemy stats, folded into
/// `Game::stack_depth_multiplier` and therefore applying to the lair
/// guardian as well as to ambushes — a party that looted its way to Hunted
/// meets a harder boss, having chosen to.
pub const TRACE_STAT_MULT: [f32; 4] = [1.0, 1.10, 1.25, 1.45];

/// Per-band multiplier on the group-size ceiling handed to `spawn_pack`.
///
/// **Inert in zone 1**, where `zone_group_cap(1)` pins every group to a
/// single member whatever this says. That is a consequence of the group
/// curve, not a defect in this constant.
pub const TRACE_GROUP_MULT: [u32; 4] = [1, 1, 2, 3];

/// Floor under `swarm_radius`, the radius that actually governs how
/// tightly a pack's members cluster around the tile a spawn roll picked
/// (`Game::try_spawn_habitat_creature`) and how far `gather_pack` searches
/// from whichever member the player bumped into: nothing gets tighter than
/// this, however small the group. `swarm_radius` grows past this floor
/// with group size, and its own doc explains why that still doesn't
/// guarantee a whole spawned cluster gathers into one fight.
pub const PACK_GATHER_RADIUS: i32 = 3;

/// How many `Hostile` creatures may exist across the whole map at once.
/// Wild creatures never despawn on their own, so without a bound the
/// world-wide population — and the per-tick AI cost of simulating it —
/// grows all session. Rather than blocking new spawns once the cap is
/// reached (which would let a population the player wandered away from
/// permanently starve the area they're actually in), reaching it culls
/// the `Hostile`s farthest from the player until the group about to spawn
/// fits — see `Game::maybe_spawn_wild_creature`. One roll can place up to
/// `MAX_GROUP_SIZE` creatures, so freeing a single slot would let the
/// population ratchet upward with every roll. Tamed programs never count
/// here at all; they shouldn't crowd out wild spawns just by existing.
pub const WILD_CREATURE_CAP: usize = 2000;

/// Chance a habitat spawn roll (see `Game::try_spawn_habitat_creature`)
/// picks a boss species instead of an ordinary one, when the tile's biome
/// has at least one boss defined for it.
pub const BOSS_SPAWN_CHANCE: f64 = 0.04;

/// Range of Portal Fragments a defeated boss guarantees, replacing the
/// flat `PORTAL_FRAGMENT_DROP_CHANCE` roll every other species gets.
pub const BOSS_PORTAL_FRAGMENT_DROP: std::ops::RangeInclusive<u32> = 3..=6;

/// Chance a habitat spawn roll (see `Game::try_spawn_habitat_creature`)
/// produces a Nest instead of an ordinary pack, for a species that has
/// `SpeciesDef::can_nest` set. Only rolled at all when `can_nest` is
/// true, mirroring how `BOSS_SPAWN_CHANCE` is only rolled when a boss
/// candidate exists — keeps the extra RNG draw out of the common
/// non-nesting path entirely.
pub const NEST_SPAWN_CHANCE: f64 = 0.06;

/// Chebyshev distance a `NestGuardian` may wander from its `Nest` — see
/// `systems::wander_ai_system`.
pub const NEST_TETHER_RADIUS: i32 = 5;

/// Inclusive range of guardians a freshly spawned `Nest` starts with —
/// see `Game::spawn_nest`.
pub const NEST_GUARDIAN_MIN: u32 = 2;
pub const NEST_GUARDIAN_MAX: u32 = 5;

/// Ticks between a guardian's death/taming and its replacement spawning
/// — see `Game::nest_respawn_tick`.
pub const NEST_RESPAWN_TICKS: u32 = 10;

/// A Nest's starting/max `Durability` — double the default structure
/// durability (`DEFAULT_STRUCTURE_DURABILITY`), since it's meant to
/// take real, sustained effort to clear, not a single lucky hit.
pub const NEST_DURABILITY: u32 = 60;

/// Tiles a pursuer covers per tick. `1` is player speed: you outrun a swarm
/// in a straight line but never shake it, and it catches you the moment you
/// stop to work, rest, or swing at the nest again. Above `1` they will reach
/// you.
pub const NEST_PURSUIT_STEPS_PER_TICK: u32 = 1;

/// Chebyshev distance **from the nest** past which a pursuer gives up.
/// Measured from the nest, not from where the chase began — so a nest near
/// the base can put pursuers on the doorstep.
pub const NEST_AGGRO_LEASH_RADIUS: i32 = 15;

/// Added to the leash radius to size the search box.
pub const NEST_PATH_SEARCH_MARGIN: i32 = 5;

/// Multiplier on an ordinary `WORK_RESOURCE_DROP` roll (see `Game::award_loot`)
/// applied to the resource a destroyed nest's species pays out (see
/// `Game::grant_nest_cache`) — the cache reads as several kills' worth of
/// `work_resource` at once, not a single kill's drop.
pub const NEST_CACHE_WORK_RESOURCE_MULT: u32 = 4;

/// Craft currency a destroyed nest pays (see `Game::grant_nest_cache`),
/// before `NEST_CACHE_FRAGMENT_ZONE_BONUS`. Deliberately under
/// `BOSS_PORTAL_FRAGMENT_DROP` (`3..=6`): a nest is sustained effort, a boss
/// is a wall.
pub const NEST_CACHE_FRAGMENTS: std::ops::RangeInclusive<u32> = 2..=5;

/// Added to `NEST_CACHE_FRAGMENTS` per zone below the current one, so a
/// deeper nest — whose guardians already scale — stays worth clearing.
/// Additive rather than multiplicative, matching `NODE_PAYOUT_ZONE_BONUS`;
/// see that constant for why compounding broke the economy.
pub const NEST_CACHE_FRAGMENT_ZONE_BONUS: u32 = 1;

/// Passes over the nest species' equipment drop table
/// (`Game::equipment_drops_for`), each entry rolled at its own chance on
/// each pass. Not a guarantee: a species whose table is empty, or whose
/// chances are low, can still pay no gear at all.
pub const NEST_CACHE_EQUIPMENT_ROLLS: u32 = 3;

// ─────────────────────────────────────────────────────────────────────────
// Needs & rest
// ─────────────────────────────────────────────────────────────────────────

/// How many ticks a full night's recharge cycle advances the clock by.
pub const REST_TICKS: u32 = 40;

/// Per-tick movement of the two needs — see `systems::tick_needs`. They run
/// in opposite directions and are not two clocks of the same kind: hunger
/// (surfaced as "Power") drains and is the only thing that can starve you,
/// so it alone paces a session.
///
/// Fatigue *refills*, because it is the pool battle abilities are paid for
/// out of (`AbilityDef::fatigue_cost`) rather than a survival need. It used
/// to drain too, which left it unrecoverable underground: the only full
/// restore is `Game::rest`, and that refuses anywhere but inside the base.
///
/// At this rate a 5.0-cost ability is worth ~62 ticks of walking. That
/// number is arithmetic, not playtested.
pub const HUNGER_DECAY_PER_TICK: f32 = 0.15;
pub const FATIGUE_REGEN_PER_TICK: f32 = 0.08;

/// What a `DifficultyMode::Forgiving` reboot leaves the player with: max HP
/// divided by this (never below 1), and both needs topped up to at least the
/// floor. Enough to keep going, not enough to make dying free — the XP
/// setback in `SETBACK_XP_PENALTY_FRACTION` applies on top either way.
pub const FORGIVING_RESPAWN_HP_DIVISOR: i32 = 2;
pub const FORGIVING_RESPAWN_NEED_FLOOR: f32 = 40.0;

// ─────────────────────────────────────────────────────────────────────────
// Loot, crafting & economy
// ─────────────────────────────────────────────────────────────────────────

/// Inclusive quantity range of its species' `work_resource` a defeated wild
/// program drops.
///
/// Doubled from `1..=2` on 2026-08-02, when deleting the scan action left
/// kills as the only source of Core Fragments outside a built base. At 1..=2
/// the first Mining Node — 12 fragments, on top of Home's 5 — was about eight
/// kills away, and a rest cost five of them.
///
/// The range is shared by every resource rather than split per item, and the
/// cost of that is borne on the Power Cell side: doubling for Fragments
/// doubled cells too. Rather than add a per-resource range, 2026-08-04 moved
/// the Scrapper off `power_cell`, leaving the Glitch as the one species that
/// drops cells — so the shared range now covers seven Core Fragment carriers
/// and one cell carrier. Count the `.ron` files before quoting a number here;
/// this comment claimed "four and three" for two days while the real split
/// was six and two.
pub const WORK_RESOURCE_DROP: std::ops::RangeInclusive<u32> = 2..=4;

/// A mining node's per-cycle success chance is `MINING_SUCCESS_BASE` plus
/// `MINING_SUCCESS_PER_LEVEL` per tier, capped at 1.0 — so a basic level-1
/// node succeeds about half the time and upgrading buys reliability. The
/// player's `Perk::KeenScavenger` adds a third term; see
/// `systems::mining_success_chance`.
pub const MINING_SUCCESS_BASE: f64 = 0.4;
pub const MINING_SUCCESS_PER_LEVEL: f64 = 0.1;

/// Extra units a worked node pays per zone below the current one, on top of
/// its upgrade tier — see `systems::node_payout`.
///
/// Deliberately additive, and deliberately *not* `ZONE_STAT_GROWTH`. Node
/// payout used to be `tier * ZoneLevel::stat_multiplier()`, which borrowed
/// the enemy-difficulty curve as the economy curve: yield doubled with depth
/// *and* multiplied by tier, so a Mk5 node paid 5 a cycle in zone 1 and 80
/// in zone 5 while every sink — build costs, upgrades, market prices — stayed
/// flat. Depth and tier compounding each other is what made Core Fragments
/// stop being a constraint by zone 2. Adding instead of multiplying keeps
/// both levers meaningful without either running away, and leaves
/// `stat_multiplier` to mean enemy difficulty and nothing else.
pub const NODE_PAYOUT_ZONE_BONUS: u32 = 1;

/// Divisor applied to the *lesser* of two fused programs' stats: a fusion
/// keeps the better parent's stat outright and adds this fraction of the
/// weaker one (see `Game::fuse_companions`). Bounded further by
/// `MAX_FUSIONS`, so the compounding can't run away.
pub const FUSION_LESSER_STAT_DIVISOR: i32 = 2;

// ---------------------------------------------------------------------------
// Production chains
// ---------------------------------------------------------------------------

/// How many units a structure's output buffer holds when its `.ron` file
/// sets no `capacity` — see `components::Stock`. This is what paces an
/// extractor now that a node has no deposit pool: it produces until the
/// buffer is full and then clogs until someone collects, so this number is
/// how long a base runs unattended.
pub const DEFAULT_OUTPUT_CAPACITY: u32 = 20;

/// How many full batches of each ingredient a machine will pull into its
/// input before refusing more. Two, so a machine always has the next batch
/// staged while working the current one, but a greedy machine still cannot
/// drain a feeder that several machines share.
pub const INPUT_STOCK_BATCHES: u32 = 2;
pub const DEFAULT_STRUCTURE_DURABILITY: u32 = 30;

/// Chance a defeated wild program additionally drops a Portal Fragment,
/// independent of its species' own `work_resource`/`equipment_drop`.
/// Fragments are the raw material for deploying a zone-portal structure
/// (see `StructureDef::zone_portal`).
pub const PORTAL_FRAGMENT_DROP_CHANCE: f64 = 0.35;

/// Growth factor applied to an item's base `EquipmentStats` per gear level
/// above 1 — doubles each level (level *N* = base *
/// `GEAR_LEVEL_GROWTH.powi(N - 1)`), matching `ZoneLevel::stat_multiplier`'s
/// own per-zone doubling so neither leveling nor gear dominates the other
/// outright — see `balance_sim::best_case_gear_bonus`'s tests for the
/// simulation that surfaced the old 2.5x growth overtaking it. Gear level
/// is capped by `resources::ZoneLevel`: reaching zone *N* is what
/// "unlocks" level *N* gear — see `Game::equip`.
pub const GEAR_LEVEL_GROWTH: f64 = 2.0;

/// Bonus `Game::fuse_item` adds to an item type's equipped stats, per
/// fusion tier — additive, not compounding (tier 2 is +20%, not +21%).
pub const ITEM_FUSION_BONUS_PER_TIER: f64 = 0.10;

/// Floor on what one fusion tier is worth to a stat the item actually has,
/// in flat points. Shipped equipment sits in the 1..=4 range, where
/// `ITEM_FUSION_BONUS_PER_TIER` alone rounds away to nothing — 4 × 1.1 is
/// 4.4, which rounds back to 4 — so without this the mechanic is invisible
/// on every real item and a fusion reads as pure loss. Applies only to a
/// stat that is already non-zero: a floor makes an item better at what it
/// does, it does not hand it a stat it never had.
pub const ITEM_FUSION_MIN_BONUS_PER_TIER: i32 = 1;

/// Copies of an item `Game::fuse_item` consumes from inventory per fusion.
pub const ITEM_FUSION_COST: u32 = 2;

/// How many fusions deep a program's lineage may go before it's a
/// finished product (see `components::FusionCount`). A program at this
/// depth can't be fed into another fusion at all, so the stat-compounding
/// `fuse_stat` gives is bounded instead of being an endless duplicate
/// laundry.
pub const MAX_FUSIONS: u32 = 3;

/// What an item is worth when its `.ron` file names no `value` — the flat
/// rate every item in the game traded at before the price ladder existed,
/// so a mod written against the older schema keeps its old behaviour.
///
/// Doubles as the ladder's floor. Anything a base can print on a timer is
/// pinned here by `every_base_produced_item_sits_at_the_floor_price`, for
/// the reason that test documents: a `work.produces` item's value is really
/// a Credit-per-tick rate, and the recipe ceiling below cannot see it.
pub const DEFAULT_ITEM_VALUE: u32 = 1;

/// What a trading post charges to sell the player back something they sold
/// it, as a multiple of that trader's own `TradeDef::sell_rate` — see
/// `Game::buy_back`. At 2 every round trip is a net loss, which is what
/// keeps the shelf a safety net rather than a strategy.
///
/// Here rather than in `TradeDef` because it is an economy knob, and this
/// file is where difficulty lives even though the content it prices is data.
pub const BUYBACK_PRICE_MULTIPLIER: u32 = 2;

/// Fraction of a structure's current build cost refunded when it's removed
/// (see `Game::remove_structure`), rounded down per item. Applies uniformly
/// whether the structure is removed directly or swept up in a Home's
/// cascading removal.
pub const STRUCTURE_REMOVAL_REFUND_PERCENT: u32 = 30;

// ─────────────────────────────────────────────────────────────────────────
// Base & raids
// ─────────────────────────────────────────────────────────────────────────

/// The player's active battle party can hold at most this many tamed
/// programs at once. With soft ranks, the slots past `FRONT_SLOTS` draw
/// less enemy fire than the ones in front of them, so a full roster is
/// deeper as well as bigger.
pub const MAX_PARTY_SIZE: usize = 5;

/// How many tamed programs the player may own in total (across the active
/// party, cronjob workers, and idle pets) before any capacity-granting
/// structures — see `StructureDef::pet_slot_bonus` and `Game::pet_capacity`,
/// which add to this base. Distinct from `MAX_PARTY_SIZE`, which caps only
/// how many of those pets can fight at once.
pub const BASE_PET_CAPACITY: usize = 3;

/// Chance per tick (see `Game::raid_check`) that a random deployed
/// structure comes under raid, if any exist.
pub const RAID_CHANCE_PER_TICK: f64 = 0.012;

/// Damage a raid deals to a structure's `Durability` when it has no
/// assigned cronjob worker defending it. Deliberately small relative to
/// `DEFAULT_STRUCTURE_DURABILITY`: a raid is meant to be attrition the base
/// can recover from, not a three-hit countdown to losing the structure
/// outright.
pub const RAID_DAMAGE: u32 = 4;

/// Damage a defending cronjob worker takes fending off a raid on its
/// structure — win or lose, defending has a cost. The raid's damage to the
/// structure itself is reduced by the worker's Defense stat instead
/// (`RAID_DAMAGE.saturating_sub(worker_def)`).
pub const RAID_DEFENDER_DAMAGE: i32 = 6;

/// Every non-Home structure must be deployed within this many tiles (per
/// axis, same box-radius style as `StructureDef::power_regen`'s
/// `radius`) of the Home structure — a base clusters around its Home
/// rather than sprawling across the map.
pub const MAX_BUILD_DISTANCE_FROM_HOME: i32 = 7;

/// How often (in ticks) the base's repairers restore `Durability` to
/// damaged structures — see `Game::structure_regen`.
///
/// There is deliberately no companion "amount" constant. Structures do not
/// heal on their own at all: every point of repair comes from a deployed
/// structure declaring `StructureDef::repair` (the Patch Node), so raid
/// damage is permanent until the player builds something that undoes it.
/// Reintroducing a free trickle here would design that structure out from
/// under itself — `raid_damage_is_permanent_without_a_repairer` is the test
/// that says so.
pub const STRUCTURE_REGEN_INTERVAL: u64 = 20;

// ─────────────────────────────────────────────────────────────────────────
// Perk magnitudes
// ─────────────────────────────────────────────────────────────────────────

// What each perk *does* per level lives here; what each perk *costs* does
// not — that is authored alongside its name and description in
// `assets/perks/*.ron`, so retitling and re-pricing the catalogue is a file
// edit. The magnitudes below stay code for the reason at the top of this
// module: content is moddable, how hard the game is, is not.

/// How much `Perk::KeenScavenger` adds to `systems::mining_success_chance`
/// per level, on top of what the node's own level is worth (still clamped
/// at a certainty).
///
/// A node is the base's income and a level-1 one yields barely half the time,
/// so this is deliberately small: at `MINING_SUCCESS_PER_LEVEL`'s rate a
/// single upgrade tier is worth ten levels of the perk. The perk smooths the
/// early game, when there is nothing to spend Perk Points on and no
/// fragments to upgrade with; it is not a substitute for upgrading.
pub const KEEN_SCAVENGER_BONUS_PER_LEVEL: f64 = 0.01;

/// `Perk::LowPowerMode`'s hunger-decay reduction, per level (the decay
/// multiplier is `1.0 - this * level`, floored at 0.0).
pub const LOW_POWER_MODE_REDUCTION_PER_LEVEL: f32 = 0.01;

/// How much `Perk::ExploitFocus` shaves off `CAPTURE_HP_PENALTY` per level,
/// floored at a penalty of 0.
///
/// Deliberately *not* effective Decompiler skill, which is what this perk
/// used to grant. That stat already grows `DECOMPILER_SKILL_PER_LEVEL` per
/// player level for free, so the perk was buying one level's worth of
/// automatic growth for `PERK_COST_EXPLOIT_FOCUS` levels' worth of points —
/// strictly dominated, and invisible next to the free growth. The HP penalty
/// is a separate axis: it decides how far a target must be worn down before
/// decompiling it is realistic, so this perk buys attempts on *healthier*
/// programs rather than better odds across the board. At 0 HP it does
/// nothing, which is the point.
pub const EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL: f32 = 0.03;

/// Per-item discount `Perk::LeanCompiler` applies to `Game::craft` recipe
/// costs, per level (never below 1 each).
pub const LEAN_COMPILER_DISCOUNT_PER_LEVEL: u32 = 1;

/// Permanent ATK `Perk::Attacker` adds to the player's `Stats`, per level.
pub const ATTACKER_BONUS_PER_LEVEL: i32 = 1;

/// Permanent DEF `Perk::Defender` adds to the player's `Stats`, per level.
pub const DEFENDER_BONUS_PER_LEVEL: i32 = 1;

/// Percentage of current max Integrity `Perk::Buffer` adds to the
/// player's `Stats`, per level.
pub const BUFFER_BONUS_PERCENT_PER_LEVEL: f32 = 0.01;

/// Floor on `Perk::Buffer`'s per-level max Integrity bonus, so it's still
/// worth buying early when 1% of max Integrity would round to less than
/// this.
pub const BUFFER_MIN_BONUS_PER_LEVEL: i32 = 10;

// ─────────────────────────────────────────────────────────────────────────
// Routine slots
// ─────────────────────────────────────────────────────────────────────────

/// Slots a companion has at level 1 before any per-level growth, then one
/// more for every `COMPANION_ROUTINE_SLOT_PER_LEVEL` levels. The floor of 1
/// in `abilities::companion_routine_slots` is what keeps a level-1 program
/// from having nowhere to hold its innate kit.
pub const COMPANION_ROUTINE_SLOT_BASE: u32 = 0;

/// Levels a companion needs per additional routine slot.
pub const COMPANION_ROUTINE_SLOT_PER_LEVEL: u32 = 2;

/// Most routines a companion can hold at once, reached at level 12.
pub const COMPANION_ROUTINE_SLOT_CAP: u32 = 6;

/// Slots the player has at level 1. One, and `decompile` occupies it — a new
/// game pre-installs that ability, so the player's first *free* slot is the
/// one `PLAYER_ROUTINE_SLOT_PER_LEVEL` grants.
pub const PLAYER_ROUTINE_SLOT_BASE: u32 = 1;

/// Levels the player needs per additional routine slot. Deliberately far
/// slower than a companion's: researched routines are meant to be a choice
/// between programs, not a second kit the player accumulates for free.
pub const PLAYER_ROUTINE_SLOT_PER_LEVEL: u32 = 10;

/// Most routines the player can hold at once, reached at level 50. The
/// player has no level ceiling (`progression::add_xp` takes `None`), so this
/// clamp is the only thing bounding their slots.
pub const PLAYER_ROUTINE_SLOT_CAP: u32 = 6;

// ─────────────────────────────────────────────────────────────────────────
// Wild routines and ability scaling
// ─────────────────────────────────────────────────────────────────────────

/// Chance a freshly spawned wild program carries a routine its species
/// never grants — a "carrier". It uses that routine against you in battle,
/// and hands it over installed if you decompile it.
///
/// This decides *whether* a carrier appears; which routine it gets is the
/// per-ability `wild_weight` in `assets/abilities/*.ron`. Deliberately low:
/// a carrier should be a thing you go hunting for, not the default program
/// in the field.
///
/// Unrelated to `WILD_ABILITY_CHANCE`, which gates whether a wild program
/// reaches for its *move's* status effect on a given swing.
pub const WILD_ROUTINE_CHANCE: f64 = 0.06;

/// How much each level adds to an ability magnitude measured in **stat
/// points** — a `Buff` or `FieldBuff` power, which is added straight to ATK
/// or DEF or read as percentage points. The multiplier is
/// `1.0 + level * this`.
///
/// Deliberately gentle, because the curve it has to keep pace with is
/// gentle: `ATK_PER_LEVEL` and `DEF_PER_LEVEL` are both 1. A +3 attack buff
/// against a base ATK of 6 is already half again; scaling it on the HP curve
/// below would turn the same routine into a tripling.
pub const ABILITY_STAT_SCALE_PER_LEVEL: f32 = 0.15;

/// How much each level adds to an ability magnitude measured in **HP** —
/// `Damage`, `Drain`, `Heal`, and the per-round bite of a `Debuff`. Steeper
/// than `ABILITY_STAT_SCALE_PER_LEVEL` by design, and for the same reason
/// that one is gentle: these are weighed against Integrity, which grows at
/// `HP_PER_LEVEL` (12) per level and doubles again per zone
/// (`ZONE_STAT_GROWTH`).
///
/// Ability damage used not to be level-scaled at all. `compute_damage` is
/// `power + ATK - DEF`, and ATK was held to carry the progression on its own
/// — but ATK grows at 1 per level against Integrity's 12, so an authored
/// power fell further behind its target every level. By the time a level-10
/// player with the affinity perk five deep faced a 400-Integrity program,
/// the heaviest shipped routine hit for 35. That is what this rate exists to
/// fix; `tests::combat_abilities` pins the resulting figure.
pub const ABILITY_HP_SCALE_PER_LEVEL: f32 = 0.40;

/// Level ceiling on both ability scales. The player has no level cap
/// (`progression::add_xp` takes `None`), so without this a long enough game
/// multiplies every routine without bound. A companion is capped far lower
/// by `CREATURE_MAX_LEVEL` and never reaches this.
pub const ABILITY_SCALE_LEVEL_CAP: u32 = 40;

/// An ability magnitude's neutral affinity — no bonus, no penalty. The
/// value every `AffinityKind` defaults to, and what a caster with neither
/// a species nor perks resolves to.
pub const AFFINITY_NEUTRAL: f32 = 1.0;

/// Bounds every affinity is clamped to when a species file is loaded.
/// Deliberately wider than `MIN_INDIVIDUAL_ROLL`..`MAX_INDIVIDUAL_ROLL`
/// (0.8-1.2): a damage affinity scales only an ability's *authored* power,
/// which is a minority of `power + ATK - DEF` at a high level, so a narrow
/// band would make damage affinities imperceptible.
///
/// These compound with whichever level scale the category uses. A companion
/// caps at `CREATURE_MAX_LEVEL` (12), so a stat magnitude's ceiling is 2.8x
/// from level times `AFFINITY_MAX` — 5.6x an authored power — and an HP
/// magnitude's is 5.8x times `AFFINITY_MAX`. That is the modder's choice to
/// make, which is the moddability contract.
pub const AFFINITY_MIN: f32 = 0.5;
pub const AFFINITY_MAX: f32 = 2.0;

/// Affinity a player affinity perk adds per level, for the three
/// *level-scaled* categories (`Heal`, `Buff`, `Debuff`): the perk's
/// multiplier is `AFFINITY_NEUTRAL + this * level`, clamped at
/// `AFFINITY_MAX` in `Game::ability_affinity` the same way a species'
/// affinity is clamped at load. One shared constant rather than three
/// identical ones, because those perks are the same shape — see
/// `Perk::affinity_kind` and `AffinityKind::perk_bonus_per_level`.
///
/// Higher than `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL` (0.03) on
/// purpose rather than matching it: a 3% nudge on a small `i32` power rounds
/// away to nothing for most shipped abilities. At 0.05 the same abilities
/// move by a visible +1.
///
/// `Damage` and `Drain` do **not** use this constant — see
/// `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED`, right below.
pub const AFFINITY_PERK_BONUS_PER_LEVEL: f32 = 0.05;

/// Affinity a player affinity perk adds per level, for `Damage` and `Drain`
/// only — deliberately a different rate from `AFFINITY_PERK_BONUS_PER_LEVEL`,
/// not a second copy of the same number.
///
/// The reason this rate was split off no longer holds: `Damage` and `Drain`
/// once skipped level scaling entirely, so the affinity term was their only
/// multiplier and had to be steep to be felt. They now scale on
/// `ABILITY_HP_SCALE_PER_LEVEL` like every other HP magnitude. The rate is
/// kept apart anyway, for the reason below, which never depended on that —
/// what these two perks compete against is not what the other three compete
/// against.
///
/// At the shared 0.05 rate, Payload Tuning and Siphon Protocol were
/// strictly worse value than the `Attacker` perk for every shipped
/// Damage/Drain ability except `broadcast_storm`: `compute_damage` gives
/// Attacker +1 flat damage per level for 2 Perk Points on *every* attack,
/// while 0.05/level of authored `power` 10 (`packet_shred`, `siphon_cycles`)
/// is only +0.5 damage per level for the same 2 points, and only on that
/// one category. 0.15 makes the level-for-level comparison favor the
/// affinity perk instead (1.5 damage per level against Attacker's 1), at
/// the cost of reaching `AFFINITY_MAX` sooner — 7 levels / 14 Perk Points,
/// instead of 20 levels / 40 — after which further levels buy nothing more
/// from this perk specifically. That tradeoff (better per-point value early,
/// a hard ceiling Attacker doesn't have) was the owner's call, not a
/// balance-formula default.
pub const AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED: f32 = 0.15;

/// Floor on the cooldown a hostile arms after spending a routine.
///
/// `AbilityDef::cooldown` is `#[serde(default)]` 0, and a carrier fires
/// whenever its routine is off cooldown — so a mod ability declaring no
/// cooldown would fire every single round. The player side keeps the
/// authored value untouched, which is what leaves `decompile` spammable.
pub const ENEMY_ROUTINE_MIN_COOLDOWN: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ZoneLevel;

    /// The zone curve was a bare `1 << (zone - 1)` before it was named.
    /// Pinning the sequence keeps a retune of `ZONE_STAT_GROWTH` honest
    /// about what it costs: this is the steepest curve in the game, and
    /// `balance_sim`'s level sweeps are projected against it.
    #[test]
    fn zone_stat_multiplier_doubles_per_level() {
        let curve: Vec<i32> = (1..=5).map(|z| ZoneLevel(z).stat_multiplier()).collect();
        assert_eq!(curve, vec![1, 2, 4, 8, 16]);
    }

    /// `Game::max_group_size` clamps its distance exponent to
    /// `MAX_GROUP_SIZE_DISTANCE_STEPS` because the map is unbounded. That
    /// clamp is only lossless while the clamped growth already exceeds
    /// `MAX_GROUP_SIZE` — raise the cap without raising the step count and
    /// distance would silently stop mattering short of it.
    #[test]
    fn clamping_the_distance_exponent_cannot_cost_group_size() {
        assert!(
            GROUP_SIZE_DISTANCE_GROWTH.pow(MAX_GROUP_SIZE_DISTANCE_STEPS) > MAX_GROUP_SIZE,
            "distance growth clamped at {MAX_GROUP_SIZE_DISTANCE_STEPS} steps reaches only {}, \
             which no longer covers MAX_GROUP_SIZE ({MAX_GROUP_SIZE})",
            GROUP_SIZE_DISTANCE_GROWTH.pow(MAX_GROUP_SIZE_DISTANCE_STEPS),
        );
    }

    /// The zone group cap has to reach `MAX_GROUP_SIZE` somewhere, or the
    /// hard ceiling is decoration. Under the old x3 growth that happened at
    /// zone 6, inside the range `balance_sim` sweeps; a line gets there
    /// later, which is the deliberate half of the trade — the early zones
    /// gained their range by the tail giving up its runaway.
    #[test]
    fn zone_group_step_saturates_the_group_cap_in_a_reachable_zone() {
        let zones_to_saturate = (1..=20)
            .find(|z| 1 + ZONE_GROUP_STEP * (z - 1) >= MAX_GROUP_SIZE)
            .expect("group growth should reach MAX_GROUP_SIZE within twenty zones");
        assert_eq!(zones_to_saturate, 12);
    }
}
