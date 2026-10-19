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
///
/// **Slots, not a limit.** A roster past this total still captures; what
/// it pays is the `unslotted` memory on each program past it
/// (`Game::note_unslotted`), which walks them down the morale ladder into
/// tantrums. `ROSTER_HARD_CAP` is the only door.
pub const BASE_PET_CAPACITY: usize = 3;

/// The one hard ceiling on how many tamed programs the player may own,
/// counted the way `Game::pet_count` counts — party, sortie and base staff
/// alike. **Hidden on purpose**: no screen shows it, because the ceiling
/// the player is meant to feel is the base coming apart past its slots.
/// It exists to bound the per-tick cost of a base, not to shape play.
pub const ROSTER_HARD_CAP: usize = 200;

/// How much pressure one tick of a sector adds to `resources::RaidPressure`,
/// per sector level — `Game::raid_check` accrues `RAID_PRESSURE_PER_ZONE *
/// zone` every tick and sweeps when the total crosses its threshold.
///
/// **Scaling the accrual rather than the threshold is what makes a deeper
/// sector more dangerous on one axis instead of two.** The threshold is a
/// fixed budget plus jitter; how fast you spend it is the only thing depth
/// changes, so the warning window shortens by exactly the same factor the
/// interval does and there is one number to reason about rather than two.
///
/// `1` because the threshold is authored in these units: see
/// `RAID_PRESSURE_THRESHOLD` for the arithmetic that turns the pair into
/// minutes of play.
pub const RAID_PRESSURE_PER_ZONE: u32 = 1;

/// The pressure a base spends between sweeps, before jitter.
///
/// **Authored in minutes of play and written here in ticks.** The world runs
/// at `app_core::WORLD_SPEED_MULTIPLIER` = 2 ticks a second, and
/// `RAID_PRESSURE_PER_ZONE * zone` accrues per tick, so the interval in
/// minutes is `RAID_PRESSURE_THRESHOLD / (RAID_PRESSURE_PER_ZONE * zone) /
/// 2 / 60`. At `4800` that is **20 minutes in sector 2**, 13 in sector 3, 10
/// in sector 4 and under 7 by sector 6.
///
/// The figure it replaced was a `0.012` chance per tick, which is a sweep
/// every **42 seconds** — frequent enough that no single one could carry any
/// weight, and evenly enough distributed that no stretch of play could feel
/// safe or threatened. Nearly all of this feature is that one ~28x cut; the
/// warning and the jitter only make the new spacing legible.
pub const RAID_PRESSURE_THRESHOLD: u32 = 4800;

/// How far either side of `RAID_PRESSURE_THRESHOLD` a drawn interval may
/// land, in percent — so sector 2 sweeps every 15 to 25 minutes rather than
/// every 20 exactly.
///
/// **Drawn once per interval, never per tick.** A per-tick jitter is
/// invisible: summed over the two thousand-odd ticks an interval takes, the
/// law of large numbers flattens it to within a percent of its own mean, and
/// the clock is a metronome again. Jittering the target instead is one draw
/// and the only one that moves anything a player could feel.
pub const RAID_PRESSURE_JITTER_PERCENT: u32 = 25;

/// How far through an interval the approach warning fires, in percent of
/// the drawn target.
///
/// A percentage of the *drawn* target rather than a fixed pressure, so the
/// warning window is the same share of every interval however the jitter
/// fell — a warning that arrived a fixed distance out would be most of a
/// short interval and a sliver of a long one.
///
/// `80` buys about four minutes in sector 2 and shortens with depth exactly
/// as the interval does. That is enough to walk home from a shallow dive,
/// post a guard or run a repair, and not enough to treat the warning as the
/// real start of the clock.
pub const RAID_PRESSURE_WARN_PERCENT: u32 = 80;

/// The fewest base-staff programs that could actually *defend*
/// (`Game::defending_base_staff_count` — `Game::base_staff`, minus
/// `components::Downed` and minus anything that has downed tools) the base
/// must have before a raid may fire at all. An opening base has too few
/// bodies to absorb attrition — a raid or two back to back can down the
/// whole staff, and once no program is left standing there is nobody to
/// defend the next one, so the base never recovers. Checked in `Game::raid_check` **after** the chance roll, so a
/// raid that would have missed anyway still consumes the same draw from
/// `GameRng` and this gate cannot shift any other test's RNG stream.
///
/// **Above `BASE_PET_CAPACITY` (3) on purpose.** A floor at or below the
/// opening slots meant a base being swept while its whole roster was still
/// the three programs it started with, which is the attrition this
/// constant exists to hold off. It is a count of bodies and no longer a
/// proxy for a Data Cache: slots stopped being a door, so five staff can
/// stand in a base with three slots — and two of them are then earning
/// `unslotted`, a base with its own trouble before a raid adds any. Pinned against the
/// literal by `four_undowned_staff_is_still_below_the_raid_floor`, because
/// every other raid test reads this constant and so moves with it.
pub const RAID_MIN_BASE_STAFF: usize = 5;

/// The first sector a GC Entropy Sweep may reach. The staff floor's twin on
/// the other axis, and it answers a different failure: `RAID_MIN_BASE_STAFF`
/// is about a base too thin to absorb attrition, this is about a *player*
/// who has not yet learned what a base is for. Zone 1 is where the first
/// Home goes up, and a sweep landing on it teaches that building is
/// punished rather than that it needs defending.
///
/// Checked in `Game::raid_check` **after** the chance roll, for
/// `RAID_MIN_BASE_STAFF`'s reason: an exempt sector still consumes the same
/// draw from `GameRng`, so this gate cannot shift any other test's RNG
/// stream. Pinned against the literal by
/// `the_second_sector_is_where_sweeps_begin`, because every other raid test
/// reads the constant and so moves with it.
///
/// This is the second thing zone 1 is exempt from, after
/// `Game::environment_biome_at`'s neutral terrain — deliberately its own
/// constant rather than a shared "is the opening sector" predicate, since
/// the two answer to different tuning and only coincide today.
///
/// `Game::enter_next_zone` reads it too, to raise
/// `NotificationKind::SweepsBegin` on the breach that lands here — so
/// retuning this moves the warning with the gate rather than leaving a
/// screen announcing sweeps in a sector that has none.
pub const RAID_MIN_ZONE: u32 = 2;

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

/// The radius the surface slab had, a 9x9 box, before the base moved into
/// its own pocket dimension. `STARTING_POCKET_HALF_WIDTH` and
/// `STARTING_POCKET_HALF_HEIGHT` size the base now;
/// this survives only as surface geometry around the anchor, which the
/// Stack's link placement keeps clear of.
pub const MAX_BUILD_DISTANCE_FROM_HOME: i32 = 4;

/// Tiles of surface sight a character at baseline attributes has. Slightly
/// wider than `MAX_BUILD_DISTANCE_FROM_HOME`, so the opening base sits inside
/// it, but a literal rather than computed from it: the starting base may grow,
/// and sight shouldn't grow with it.
pub const PERCEPTION_BASE_RADIUS: f32 = 5.0;

/// The floor `progression::derive` clamps Perception to, so a character
/// rolled low in Analysis and Entropy still sees the tiles beside them.
pub const PERCEPTION_MIN_RADIUS: f32 = 2.0;

/// How far the pre-cleared pocket reaches east and west of base space's own
/// origin when the first Home is deployed — `Game::lay_starting_pocket`,
/// which lays `BaseCell::Floor` over the chamfered box this,
/// `STARTING_POCKET_HALF_HEIGHT` and `PLATFORM_CORNER_CUT` describe and
/// writes nothing into `WorldMap` at all.
///
/// 9 gives a 19-cell-wide pocket, double the 9 it opened at when it matched
/// `MAX_BUILD_DISTANCE_FROM_HOME`'s old slab: at 9 wide the beginner
/// structures did not all fit, and digging out room for the first handful
/// was a chore rather than growth.
///
/// This is the *starting* size, the way the old constant was the starting
/// radius: slice 2's mining is what makes floor space something a player
/// buys, and this is only the ground the run opens with.
pub const STARTING_POCKET_HALF_WIDTH: i32 = 9;

/// How far the starting pocket reaches north and south of base space's
/// origin: 9 cells tall, the height it opened at. Separate from
/// `STARTING_POCKET_HALF_WIDTH` because the base was asked to grow wider,
/// not bigger — one radius for both made it twice as tall too.
pub const STARTING_POCKET_HALF_HEIGHT: i32 = 4;

/// The widest a base was ever meant to get, from when it was a slab stamped
/// onto the zone surface: `Game::build_radius` clamped here no matter how
/// many `build_radius_bonus` structures (the Heap Pillar's own) were
/// deployed, and `Game::clear_platform` swept this box when a Home came
/// down, since it had to cover the largest slab that could ever have
/// existed rather than the current shape.
///
/// **All three — `build_radius`, `clear_platform` and the Heap Pillar —
/// are deleted along with `resources::Platform`.** Base space grows by
/// laying floor on `base_grid::BaseGrid` now (`Game::lay_starting_pocket`,
/// slice 2's mining), not by widening a clamped radius, and nothing reads
/// this constant as an active ceiling any more.
///
/// Kept rather than deleted: two of this file's own doc comments
/// (`STACK_NEAREST_LINK_TILES`, `CONTRACT_HABITAT_SAMPLES`) still reason
/// about "the widest a base can ever get" in these terms, and several tests
/// use it to construct a base at the old worst case. A future slice giving
/// `BaseGrid` a real bound should retune or delete this against whatever
/// that bound turns out to be — it is a placeholder for "very large," not a
/// measured figure, and no longer the backstop it was written as.
pub const MAX_BUILD_RADIUS_TILES: i32 = 100;

/// How deep each of the base slab's four corners is chamfered, in diagonal
/// steps — the slab is the box above with `Platform::covers` trimming a
/// triangle off each corner, so at 2 the corner tile and the two beside it
/// are natural terrain and the base reads as rounded rather than as a
/// stamped square.
///
/// This is footprint, not decoration: `place_structure` measures against
/// the same predicate, so a cut tile is unbuildable and a raised value
/// takes buildable ground away. 0 restores the square.
pub const PLATFORM_CORNER_CUT: i32 = 2;

// ---- Growing the base: rock you cut ----------------------------------
//
// None of these is measured. They are starting values to play and then
// record under `docs/measurements/` — see the slice 2 section of
// `docs/superpowers/archive/specs/2026-08-19-base-out-of-phase-design.md`.

/// How much damage one cell of base-space rock absorbs before it opens.
///
/// **Never scaled by zone, depth or level.** The rock is the same rock all
/// run, so the thing that changes is the player: a wall that takes about
/// three swings at level 1 takes fewer late, and that is the reward for
/// levelling rather than a curve to tune. A scaled wall would make digging
/// cost the same forever, which is the one thing it must not do.
///
/// It used to take *one* late, and that was written down here as the reward
/// working. It is the reported bug instead: a wall that falls to an
/// accidental keypress is not terrain, and a developed player navigating
/// their own base demolished it a corner at a time.
/// `BASE_ROCK_MIN_SWINGS` is the floor that closes it without touching the
/// no-scaling rule above.
///
/// **This is now only the *fallback* kind's number.** Every kind in
/// `assets/rock/` authors its own, and `Game::wall_at` is the one door from
/// a coordinate to it — reading this constant where the question is about a
/// particular wall is the mistake to watch for, and it silently caps every
/// dense wall in the base at 24.
///
/// Priced against a level-1 player's swing — `PLAYER_UNARMED_DAMAGE`'s mean
/// of 5 plus `PLAYER_BASE_STATS`' atk of 6, so about 11 a hit.
pub const BASE_ROCK_DURABILITY: u32 = 24;

/// The fewest swings a cell of the fallback rock kind can ever be opened in.
///
/// `Game::strike_rock` caps one swing at `durability.div_ceil(min_swings)`,
/// so this is what stops a developed player demolishing a wall by clipping
/// a corner of their own base. **Level-independent on purpose**: scaling
/// durability with the player instead would make digging cost the same
/// forever, which is the one thing it must not do. Levelling still speeds
/// digging — three swings at level 1 down to this floor — it simply cannot
/// reach one.
///
/// At `BASE_ROCK_DURABILITY` of 24 the cap is 12, and a level-1 player's
/// ~11 a swing is under it, so the opening game's dig rate does not move at
/// all: the floor bites exactly where the bug was reported and nowhere else.
///
/// Every kind in `assets/rock/` authors its own; this is the one the
/// built-in fallback carries, which is why an empty `assets/rock/` restores
/// *uniform* rock and not the one-shot.
pub const BASE_ROCK_MIN_SWINGS: u32 = 2;

/// Chance, 0.0-1.0, that opening a cell of rock yields one Core Fragment.
///
/// **Bounded above by the Extractor's rate**, and that bound is the whole
/// rule: a dug cell has to return a trickle or the wall becomes a fragment
/// tap that undercuts the machine built to be one.
/// `mining_a_wall_never_undercuts_a_mining_node` holds it against the real
/// assets rather than against a number written here — and holds it **per
/// tick**, because this constant is a probability the strike clamps to
/// `0.0..=1.0`, so any per-cell comparison against a Blank Substrate's four
/// fragments passes for every legal value and asserts nothing. The rate
/// takes `BASE_ROCK_DURABILITY` and `BASE_DIG_TICKS_PER_SWING` in with it,
/// which is what makes softening the rock or quickening the swing show up
/// here too.
pub const BASE_MINE_FRAGMENT_CHANCE: f32 = 0.25;

/// How many ticks a posted digger spends per swing at a marked cell — one
/// cycle of `TaskKind::Excavate`, the crew's counterpart to a cronjob's
/// `work_ticks_for`.
///
/// The **damage** a swing lands is not here: it is the worker's own
/// `Game::swing_damage`, exactly as it is for the player, so a stronger
/// program digs a wall out in fewer swings rather than faster ones. This is
/// only the rate a crew works at against a player who swings once per
/// keypress, which is what keeps a marked wing something you leave running
/// rather than something faster than doing it yourself.
///
/// Unmeasured, like every other knob in this slice. Play it and record what
/// it said under `docs/measurements/`.
pub const BASE_DIG_TICKS_PER_SWING: u32 = 12;

// --- Farm labour ---

/// How many ticks a posted program spends dimpling one fallow farm tile —
/// one dig swing's cadence, since it is comparable hand-work. A crop's own
/// grow time lives in its `assets/crops/*.ron`, not here.
pub const FARM_DIMPLE_TICKS: u32 = 12;

/// How many ticks a posted program spends bringing in one ripe farm tile.
/// Half a dimple: the crop is already made, the body only has to carry it.
pub const FARM_HARVEST_TICKS: u32 = 6;

/// How many Core Fragments one VectorStasis Tile costs, laid by hand or by
/// the crew.
///
/// **Fragments, not Blank Substrate**, because the floor is what grows the
/// base and a Factory sitting between the mine and every tile made expansion
/// wait on a 12-tick press per cell. Half a substrate's four fragments, so
/// the plain floor is cheaper than it was as well as quicker; finishes stay
/// on substrate (`FLOOR_FINISH_COST`) as the Factory-fed decoration.
///
/// Above `BASE_MINE_FRAGMENT_CHANCE`'s quarter-fragment a cut cell returns,
/// so a dig plan never floors itself out of its own rubble.
pub const FLOOR_TILE_COST: u32 = 2;

/// How many Blank Substrate a finish costs, on top of the
/// `FLOOR_TILE_COST` fragments the tile itself already spent —
/// `floors::FloorDb`'s one price, since there is no per-finish `cost` field
/// (see `assets/floors/README.md`).
pub const FLOOR_FINISH_COST: u32 = 3;

/// How many ticks of construction each unit of material in a structure's
/// build cost is worth — see `components::BuildSite::required_ticks`.
///
/// **The build meter is derived from the cost, never stored beside it.** A
/// site already carries the resolved `build_cost` it was filed against, so
/// a second stored figure could only ever drift from it: a save written
/// under one rate would go on counting to the old total while the base
/// beside it counted to the new one. Deriving it makes retuning this
/// constant a change every site in every save agrees about on the next
/// tick, which is what a future per-structure build-time knob wants to
/// slot into.
///
/// It is deliberately a rate *per unit of material* rather than a flat
/// number of ticks: a Home-sized bill of materials should take longer to
/// stand up than a Depot's, and pricing it off the cost is the only figure
/// already in the site that says how big the thing is.
///
/// Unmeasured, like the rest of this slice. Play it and record what it said
/// under `docs/measurements/`.
pub const BUILD_TICKS_PER_MATERIAL: u32 = 2;

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
// Sieges
// ─────────────────────────────────────────────────────────────────────────

/// How much pressure one tick of a sector adds to `resources::SiegePressure`,
/// per sector level — `RAID_PRESSURE_PER_ZONE`'s shape, and `1` for the same
/// reason: the threshold is authored in these units.
pub const SIEGE_PRESSURE_PER_ZONE: u32 = 1;

/// The pressure a base spends between sieges, before jitter.
///
/// **Authored in minutes of play and written here in ticks**, `
/// RAID_PRESSURE_THRESHOLD`'s method: the world runs at
/// `app_core::WORLD_SPEED_MULTIPLIER` = 2 ticks a second, and
/// `SIEGE_PRESSURE_PER_ZONE * zone` accrues per tick, so the interval in
/// minutes is `SIEGE_PRESSURE_THRESHOLD / (SIEGE_PRESSURE_PER_ZONE * zone) /
/// 2 / 60`. At `14_400` that is **60 minutes in sector 2** and **20 minutes
/// in sector 6** — a siege is meant to be an event a run meets a handful of
/// times, not a chore met every session, so its interval is much longer
/// than the sweep's.
pub const SIEGE_PRESSURE_THRESHOLD: u32 = 14_400;

/// How far either side of `SIEGE_PRESSURE_THRESHOLD` a drawn interval may
/// land, in percent — `RAID_PRESSURE_JITTER_PERCENT`'s reason, drawn once
/// per interval and never per tick.
pub const SIEGE_PRESSURE_JITTER_PERCENT: u32 = 25;

/// How far through an interval the approach warning fires, in percent of
/// the drawn target — `RAID_PRESSURE_WARN_PERCENT`'s shape. Landed here in
/// Task 1 rather than Task 2 because `Game::dev_wind_siege_clock` needs it
/// to wind to a warn point before Task 2 gives the warning itself a body;
/// `SIEGE_WARN_FLOOR_TICKS`, the second half of the warning's own timing,
/// follows in Task 2.
pub const SIEGE_PRESSURE_WARN_PERCENT: u32 = 80;

/// The wall-clock floor under the approach warning, in ticks — the fewest
/// ticks of notice a siege gives however deep the drawn interval is.
///
/// A sweep's answer to its own warning is "post someone at the machine you
/// care about", so a percentage share of the interval is right for it: the
/// window shrinks with depth exactly as the interval does, and the answer
/// takes no longer in a deep sector than a shallow one. A siege's answer is
/// "get home", and the trip is longest in exactly the sectors where
/// `SIEGE_PRESSURE_WARN_PERCENT`'s share is shortest — so the siege warns at
/// whichever is earlier, the share or this floor, converted through the
/// sector's own accrual rate since the floor is stated in ticks and the
/// meter is not a clock.
///
/// **Unvalidated.** `960` is 8 minutes, picked as a defensible number and
/// not a measured one — nothing in this repo models the walk home from a
/// deep Stack frame. Play it before trusting it.
pub const SIEGE_WARN_FLOOR_TICKS: u32 = 960;

/// The first sector a siege may reach. `RAID_MIN_ZONE`'s twin, gating
/// **accrual** and never firing — see `Game::siege_check`'s first line for
/// why gating firing instead would ambush a player the instant they cross
/// into the sector that can spend the pressure the opening sector banked.
pub const SIEGE_MIN_ZONE: u32 = 2;

/// The base size of a besieging pack, before `SIEGE_PACK_PER_ZONE` scales it
/// with sector — see `siege::offscreen::pack_size`. The off-screen
/// resolution and the fought fight must field the same pack, so this is
/// read through that one function rather than restated at either call site.
pub const SIEGE_PACK_BASE: u32 = 4;

/// How much a besieging pack grows per sector, added to `SIEGE_PACK_BASE`
/// and capped at `SIEGE_PACK_MAX`.
pub const SIEGE_PACK_PER_ZONE: u32 = 1;

/// The largest a besieging pack may grow, whatever the sector.
pub const SIEGE_PACK_MAX: u32 = 12;

/// Each on-shift staff body's contribution to the off-screen defence a
/// siege is priced against, in `Game::resolve_siege_offscreen`'s shortfall
/// formula. A body actually fought beside in the tactical fight is worth
/// far more than this — the figure only prices the base's *passive*
/// resistance while its owner is away, `defending_base_staff_count`'s own
/// pool, counted regardless of what job each body happens to be posted to.
pub const SIEGE_STAFF_DEFENSE: u32 = 2;

/// Units lifted from base shelves per unanswered point of an off-screen
/// siege's shortfall.
pub const SIEGE_STEAL_PER_POINT: u32 = 4;

/// Structure `Durability` lost per unanswered point of an off-screen
/// siege's shortfall.
pub const SIEGE_DAMAGE_PER_POINT: u32 = 3;

/// How much unanswered shortfall it takes to bench one staff body in an
/// off-screen siege — `shortfall / SIEGE_POINTS_PER_CASUALTY` bodies,
/// through the same door a Forgiving death already uses
/// (`Game::bench_or_dissolve`).
pub const SIEGE_POINTS_PER_CASUALTY: u32 = 10;

/// How much of the besieging pack has to be down — killed or gone through
/// the door with its plunder, `Game::besieger_turn`'s own reading of "down"
/// — before the rest withdraw unconditionally. Half the pack breaking a
/// siege is the spec's own figure; there is no quota and no round limit
/// besides it.
pub const SIEGE_MORALE_BREAK_PERCENT: u32 = 50;

// ---------------------------------------------------------------------------
// Outposts
//
// **Correction 6: v1 applies no class-based yield bonus at an outpost.**
// `Game::run_outposts` calls `systems::mining_success_chance` directly for
// each crew member — base INT and the player's Keen Scavenger perk, with
// `morale`/`need_strain` at `0.0` because away crew have neither — and
// builds no `CycleModifiers` for the program, which is the door a Striker's
// second action or a Leech bonus would otherwise come through. Open per
// design spec §11: whether either should apply at an outpost is unresolved,
// not "the base's own rule already covers it."
// ---------------------------------------------------------------------------

/// How many outposts may stand at once — `ROUTE_MAX_ACTIVE`'s reason: a cap
/// so an unbounded number of fixtures cannot pile onto the raid roll and the
/// production tick, not a throughput number derived for it. Unmeasured; see
/// design spec §11.
pub const MAX_OUTPOSTS: usize = 4;

/// How close a candidate tile must be walked before it may be founded, in
/// Chebyshev tiles from the base anchor.
///
/// **Deliberately not `MAX_BUILD_DISTANCE_FROM_HOME`** (a base-space
/// constant, value 4) — an outpost stands on the *zone surface*, a
/// different coordinate space at a different scale, and reusing a base-space
/// literal there would make an unrelated base-building retune silently move
/// where an outpost may stand. **A fraction of a region**,
/// `ROUTE_PREDATION_RADIUS`'s reason: a flat number is measured against
/// nothing, and this is close enough to the anchor to reach on foot in one
/// session — a quarter of `SETTLEMENT_GARRISON_RADIUS`'s own half-region
/// reach, so an outpost this close to the base always sits well inside it,
/// not outside. Unmeasured — see design spec §11.
pub const OUTPOST_MIN_ANCHOR_DISTANCE: i32 = crate::settlements::placement::REGION_TILES / 8;

/// How far apart two outposts must stand, in Chebyshev tiles.
///
/// A fraction of a region, `OUTPOST_MIN_ANCHOR_DISTANCE`'s reason and
/// smaller than it: two outposts crowding each other is a lesser problem
/// than one crowding the base. Unmeasured — see design spec §11.
pub const OUTPOST_MIN_SPACING: i32 = crate::settlements::placement::REGION_TILES / 16;

/// The integrity a freshly founded outpost starts at, and the ceiling
/// `repair_outpost` restores it to (Phase 5). Unmeasured — see design
/// spec §11.
pub const OUTPOST_MAX_INTEGRITY: u32 = 100;

/// How many programs may be posted at one outpost at once.
///
/// `OUTPOST_TIER_CREW`'s own ceiling, so the top tier's crew requirement
/// must never exceed this — asserted by a test, `a-gated-consequence-can-
/// be-green-and-unreachable`'s rule. Unmeasured — see design spec §11.
pub const OUTPOST_CREW_CAP: usize = 6;

/// The stored `growth` an outpost needs to *hold* each tier, index 0 being
/// tier 1 (raw) — `outposts::OutpostTierDef`'s own indexing.
///
/// **Growth is never reset by a crew shortfall.** `outposts::tier` reads
/// this against the crew ladder below and takes the lower of the two, so an
/// outpost that grew to tier 3 and then lost crew reports a lower tier
/// without losing the banked number — re-crewing it recovers instantly
/// rather than re-growing from zero. Unmeasured — see design spec §11.
pub const OUTPOST_TIER_GROWTH: [u32; 3] = [0, 200, 500];

/// The crew an outpost needs *posted* to hold each tier, `OUTPOST_TIER_GROWTH`'s
/// index for index. The top entry must stay at or below `OUTPOST_CREW_CAP`,
/// or tier 3 is a gated consequence nobody can ever reach — asserted by a
/// test. Unmeasured — see design spec §11.
pub const OUTPOST_TIER_CREW: [usize; 3] = [1, 3, 5];

/// How much stored `growth` one `OUTPOST_CYCLE_TICKS` period of `Trend::Growing`
/// adds, per crew member counted from the tier-1 floor (`OUTPOST_TIER_CREW[0]`)
/// upward — so the minimum crew that avoids `Trend::Declining` still grows at
/// this base rate, and every program posted past it scales that rate up
/// rather than the floor crew growing nothing at all. Unmeasured — see
/// design spec §11.
pub const OUTPOST_GROWTH_PER_CREW: u32 = 2;

/// How much stored `growth` one tick of `Trend::Declining` costs. Unmeasured
/// — see design spec §11.
pub const OUTPOST_DECAY_PER_TICK: u32 = 1;

/// Ticks an outpost's stock may sit at `OUTPOST_STOCK_CAP` before
/// `Trend::Stale` degrades into `Trend::Declining`. Unmeasured — see design
/// spec §11.
pub const OUTPOST_STALE_GRACE_TICKS: u32 = 200;

/// The total units an outpost's `stock` may hold across every item at once
/// — one combined figure, `Stock`'s per-item capacity deliberately not
/// reused here since an outpost's yield union can name several items at
/// once and the screen shows one "Stock N/cap" figure for all of them.
/// Unmeasured — see design spec §11.
pub const OUTPOST_STOCK_CAP: u32 = 60;

/// Integrity below this fraction of `OUTPOST_MAX_INTEGRITY` is `Trend::Declining`
/// regardless of crew or stock — a damaged outpost falls even at full
/// staffing, which is what makes `repair_outpost` (Phase 5) necessary rather
/// than optional. Unmeasured — see design spec §11.
pub const OUTPOST_DAMAGED_FRACTION: f32 = 0.5;

/// Ticks between production cycles — `Game::run_outposts` advances
/// `Outpost::cycle_progress` every tick and rolls the crew once it reaches
/// this. Unmeasured — see design spec §11.
pub const OUTPOST_CYCLE_TICKS: u32 = 50;

/// An outpost raid's base chance, rolled once per outpost inside
/// `raid_check`'s firing branch — design spec §8. Unmeasured — see design
/// spec §11.
pub const OUTPOST_RAID_BASE_CHANCE: f64 = 0.1;

/// How much each known Hostile town within `SETTLEMENT_RAID_RADIUS` of the
/// outpost's own tile — not the anchor — raises that chance, `Standing::
/// sends_raiders`'s own gate reused rather than restated. Unmeasured — see
/// design spec §11.
pub const OUTPOST_HOSTILE_TOWN_BONUS: f64 = 0.1;

/// How much each posted crew member lowers the chance. Unmeasured — see
/// design spec §11.
pub const OUTPOST_CREW_DEFENSE: f64 = 0.03;

/// The ceiling an outpost raid's rolled chance is clamped below —
/// `SETTLEMENT_GARRISON_MAX`'s reason one axis over: however many hostile
/// towns pile onto the bonus term, a raid must stay a possibility to defend
/// against rather than a certainty. Unmeasured — see design spec §11.
pub const OUTPOST_RAID_CHANCE_CAP: f64 = 0.9;

/// Integrity a raid hit takes off an outpost. Unmeasured — see design spec
/// §11.
pub const OUTPOST_RAID_DAMAGE: u32 = 25;

/// The share of an outpost's stock a raid hit steals, applied per item and
/// rounded down. Unmeasured — see design spec §11.
pub const OUTPOST_RAID_STEAL_FRACTION: f32 = 0.5;

/// Cells at which a room's area stops adding to its quality score.
pub const ROOM_AREA_FULL: f32 = 16.0;
/// Weight of the area term in `rooms::quality`.
pub const ROOM_AREA_WEIGHT: f32 = 0.5;
/// Weight of the finished-floor share in `rooms::quality`.
pub const ROOM_FINISH_WEIGHT: f32 = 0.5;
/// Weight of the crowding penalty in `rooms::quality`.
pub const ROOM_CROWD_WEIGHT: f32 = 1.0;
/// Structure anchors per cell a room holds before crowding starts to cost.
pub const ROOM_CROWD_FREE: f32 = 0.25;
/// Lower score bounds of the Plain, Fine and Superb bands; below the first
/// is Cramped.
pub const ROOM_BAND_PLAIN: f32 = 0.25;
pub const ROOM_BAND_FINE: f32 = 0.6;
pub const ROOM_BAND_SUPERB: f32 = 0.85;
/// Amenity refill multiplier by band, in `RoomBand` order (Cramped, Plain,
/// Fine, Superb). Applies only to an amenity standing in a room with a role.
pub const ROOM_SERVICE_SCALE: [f32; 4] = [0.85, 1.0, 1.15, 1.3];
/// Work-tick multiplier by band, in `RoomBand` order. Applies only to a
/// structure standing in the workshop room; fewer ticks is faster.
pub const ROOM_WORK_SCALE: [f64; 4] = [1.1, 1.0, 0.92, 0.85];
