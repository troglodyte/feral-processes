// ---- The Stack: markets ----------------------------------------------
//
// Somebody is running a stall down there. What is on it is a function of
// the frame (`Game::market_offers`); what it costs is here.

/// The odds a Stack frame has a market on it at all — see
/// `stack::place_market`.
///
/// Well under half, deliberately: a stall you meet on every frame is a
/// shop, and a shop is somewhere you go back to. The whole of what makes
/// this one worth stopping at is that the next frame probably hasn't got
/// one, and neither has this one once you have bought the shelf out.
pub const STACK_MARKET_CHANCE: f64 = 0.35;

/// How many routines a market lists, before the program row it may or may
/// not also carry.
///
/// Two rather than one so the shelf poses a choice rather than a price, and
/// not more because each is listed at all three scopes below — four rows a
/// routine, and a screen that has to be scrolled is a screen that hides the
/// thing you came for.
pub const STACK_MARKET_ROUTINE_OFFERS: usize = 2;

/// The odds a market also has a program for sale.
pub const STACK_MARKET_PROGRAM_CHANCE: f64 = 0.4;

/// What a market charges for a bundle of etched disks: one, enough for a
/// fielded party, and enough for a roster.
///
/// Flat Credits rather than anything derived from the routine: the market
/// is selling the *writing*, not the knowledge (nothing here touches
/// `KnownRoutines`), and what a disk is worth does not depend on what is
/// burnt onto it. Against `STACK_CACHE_CREDITS` at 12-30 a cache and three
/// caches a frame, the cheapest rung is a frame or two of thorough looting
/// at depth 1 and much less deeper down.
///
/// The party rung is deliberately only twice the single rung while carrying
/// three times the disks, and the roster rung is priced above what a roster
/// is ever likely to need: buying breadth is the point of the ladder, and
/// the top rung is a run's savings rather than an errand.
pub const STACK_MARKET_ROUTINE_PRICE_ONE: u32 = 150;
pub const STACK_MARKET_ROUTINE_PRICE_PARTY: u32 = 300;
pub const STACK_MARKET_ROUTINE_PRICE_EVERYONE: u32 = 1000;

/// How many etched disks each price rung above hands over.
///
/// **Constants rather than the live party and roster sizes**, and that is
/// the whole reason they are here rather than read off `Party` and
/// `owned_pets`. A quantity derived from the party would change between the
/// player reading the shelf and paying for it — a program dismissed, a
/// companion left behind — which is exactly the objection
/// `Game::market_program_price` already makes about folding Trace into a
/// quote. What is on the shelf has to be what is bought.
///
/// `MAX_PARTY_SIZE` and a roster's realistic size are what these are
/// modelled on, but they are not *bound* to them: a retune of party size is
/// not automatically a retune of what a shelf sells.
pub const STACK_MARKET_ROUTINE_DISKS_ONE: u32 = 1;
pub const STACK_MARKET_ROUTINE_DISKS_PARTY: u32 = 3;
pub const STACK_MARKET_ROUTINE_DISKS_EVERYONE: u32 = 6;

/// The odds a market carries an exclusive routine's etched disk, as
/// `BASE + PER_DEPTH * depth`, clamped to `0.0..=1.0`.
///
/// Climbing with depth is what makes the deep Stack the place these are
/// actually shopped for, rather than something to farm by re-rolling depth-1
/// frames — which is cheap, since depth 1 is a short walk from the breach.
/// At depth 1 it is one market in twelve; by depth 8 it is better than one
/// in three.
///
/// A trader is the *expensive* half of the pair: the boss that drops the
/// same disk asks for a fight and nothing else, and
/// `STACK_MARKET_EXCLUSIVE_PRICE` is set above the roster rung so buying
/// one is never the casual option.
pub const STACK_MARKET_EXCLUSIVE_CHANCE_BASE: f64 = 0.04;
pub const STACK_MARKET_EXCLUSIVE_CHANCE_PER_DEPTH: f64 = 0.04;

/// What a market charges for a single exclusive routine's etched disk.
///
/// Above `STACK_MARKET_ROUTINE_PRICE_EVERYONE` deliberately: six disks of
/// something anyone can etch is a convenience, and one disk of something
/// nobody can etch is the run's prize. A player who can afford both should
/// feel the second one cost more.
pub const STACK_MARKET_EXCLUSIVE_PRICE: u32 = 1400;

/// What a market charges per point of a program's power (`Stats::power()`
/// as the species would spawn at this depth).
///
/// Must stay comfortably above the reciprocal of the surface trader's
/// `program_sell_divisor` (10, in `assets/structures/black_market.ron`), or
/// buying a program down here and selling it up there prints Credits. At 2
/// the round trip returns a twentieth of what it cost, and
/// `a_market_program_costs_more_than_a_trader_would_pay_for_it` is what
/// holds that against a retune of either number.
pub const STACK_MARKET_PROGRAM_PRICE_PER_POWER: u32 = 2;

/// What a market pays per unit of `ItemDef::value` for goods sold to it.
///
/// The same 1 the surface trader pays (`black_market.ron`'s `sell_rate`),
/// and that is the point: a Stack market is worth stopping at for *where*
/// it is and what is on the shelf, never for a better price. A rate above
/// the surface one would make hauling an Extractor's output down a hole
/// the best-paying thing in the game, on a curve nothing gates.
pub const STACK_MARKET_SELL_RATE: u32 = 1;

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

/// What each frame of Stack depth *adds* to the multiplier on wild program
/// stats, on top of `ZoneLevel::stat_multiplier` and
/// `Game::distance_stat_multiplier` (depth 1 = x1, depth 2 = x1.35, depth 3
/// = x1.70, ...).
///
/// Linear, for the reason `ZONE_STAT_STEP` gives at length: depth compounded
/// *on top of* the zone curve, so a geometric depth term made the deepest
/// frame of a deep zone the least reachable fight in the game. It stays
/// gentler than the zone step besides, because descending is cheap — a link
/// down, not a Portal you had to fund — so the curve has to be walkable
/// rather than a wall. The party does not choose a stack's depth either:
/// `frames_for` sets it from the link's distance to the spawn point, so a
/// player can be handed a 6-frame stack as their only remaining lair. XP follows for free:
/// a kill pays the defeated program's `max_hp`, so scaling stats scales the
/// reward with the risk.
pub const STACK_DEPTH_STAT_STEP: f32 = 0.35;

// ---- The Stack: Trace ------------------------------------------------
//
// Trace rises with what the party *takes* from a stack and escalates what
// comes for them. The design argument for every number below is in
// `docs/superpowers/archive/specs/2026-07-31-the-stack-design.md`, "Phase 2".
//
// Walking is free, deliberately. A time-driven meter would tax exploration
// and map-making, rewarding the beeline and punishing the careful player —
// backwards for a maze whose per-frame map memory exists to reward
// learning it.

/// Trace for cracking a cache. The dominant source, and meant to be: a
/// frame holds exactly three, and the payout scales with depth, so the
/// meter and the reward rise together.
pub const TRACE_PER_CACHE: u32 = 10;

/// Trace for forcing a seal. Near-negligible in practice — a stack holds
/// two, both on the bottom frame walling off the lair — but shouldering one
/// open is the loudest thing that happens on the way to the guardian, and
/// since nothing else is spent doing it, the noise is the whole cost.
pub const TRACE_PER_SEAL: u32 = 5;

/// Trace for stopping to listen (`Game::listen`). Under a seal's 5, since
/// listening takes nothing out of the frame, and well under a cache's 10 —
/// but never free, because a frame swept clean reporting silence is the
/// information the turn bought. Free-when-empty would make listening a
/// zero-risk sweep to run on every tile.
pub const TRACE_PER_LISTEN: u32 = 3;

/// Trace for stepping through a wall with `AbilityEffect::Phase`. Priced
/// beside a seal rather than a cache: it takes nothing out of the frame, but
/// it does route the party around a wall the substrate put there, and a
/// shortcut the maze did not offer should register the way a burned lock
/// does.
pub const TRACE_PER_PHASE: u32 = 5;

/// Trace for an `AbilityEffect::Jump`. Twice a phase, because it crosses the
/// whole frame rather than one wall — and still well under a cache, since
/// what Trace measures is what the party *takes* and a jump takes nothing.
/// The real cost of a wild jump is the gamble, not this.
pub const TRACE_PER_JUMP: u32 = 10;

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
/// It used to be **inert in zone 1**, where `zone_group_cap(1)` pinned every
/// group to a single member whatever this said. `ZONE_ONE_GROUP_CAP` lifted
/// that floor, so the lever now has room to move there too — the inertness
/// was always a consequence of the group curve rather than an intent of this
/// constant, which is why lifting the floor was allowed to end it.
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

/// Multiplier on every stat of an **ordinary** species rolled into a boss.
/// An apex species (`SpeciesDef::is_boss`) never takes it — its stats are
/// hand-authored, and a blanket multiplier would discard the authoring, the
/// same reason it rolls no rare tier.
///
/// Calibrated against the ladder rather than picked: apex totals are 206 and
/// 236 against a band-2 median of 140, so ~1.5x is "one band up". 1.75 puts a
/// rolled boss above an Overclocked spawn (`GOLD_STAT_MULT`), which is what
/// makes it read as a wall rather than as a shiny — and a boss rolls no rare
/// tier on top, so this is the whole of its elevation.
///
/// **Ungated by `balance_sim`**, which models no bosses at all: see
/// `toughest_ordinary_species`, which excludes them. `dev-arenas/` is the
/// instrument for this number.
pub const BOSS_STAT_MULT: f32 = 1.75;

/// How often a wild spawn comes up rare, and what it's worth when it does —
/// see `Game::roll_rarity` and `components::Rarity`. The chances are checked
/// against **one** roll in tier order, rarest first, so they can't sum past
/// 1.0 and each tier is genuinely rarer than the one below rather than a
/// separate draw that lands on top of it.
///
/// This is a *discrete* axis deliberately laid over the continuous one:
/// `MIN_INDIVIDUAL_ROLL`/`MAX_INDIVIDUAL_ROLL` above already give every
/// creature a ±20% band, but a band has no threshold, so nothing about it
/// can be spotted on the map or chased. These multiply with that band
/// rather than replacing it, so a gold lands 1.44x-2.16x an ordinary spawn
/// of the same species.
///
/// None applies to a boss (its stats are hand-authored per `.ron`, so a
/// multiplier discards the authoring) nor inside `Game::in_opening_ring`
/// (where `balance_sim::beatable_by_a_fresh_player` guarantees a fresh
/// player can beat one program, computed against `MAX_INDIVIDUAL_ROLL`).
/// That second exclusion is what lets `balance_sim` stay ignorant of rarity
/// entirely — if its curves ever move because of this, the exclusion is
/// wrong, not the test.
///
/// **The top two rungs are deliberately gentler than the curve below them
/// suggests.** Silver→gold steps by 0.3, but platinum and prismatic step by
/// 0.2 and 0.15: a wild program's multiplier is applied to *every* stat at
/// once, so the top of this ladder is the hardest ordinary fight in the
/// game, and it can be rolled anywhere outside the opening ring at any
/// player level. Continuing at 0.3 a rung puts a 2.4x-3.6x program in front
/// of a zone-1 player who has no way to read the danger before the fight
/// opens. Fleeing exists, and that is the intended answer to one of these —
/// but only if the fight is survivable long enough to flee *from*.
pub const SILVER_SPAWN_CHANCE: f64 = 0.030;
pub const GOLD_SPAWN_CHANCE: f64 = 0.005;
pub const PLATINUM_SPAWN_CHANCE: f64 = 0.0015;
pub const PRISMATIC_SPAWN_CHANCE: f64 = 0.0003;
pub const SILVER_STAT_MULT: f32 = 1.5;
pub const GOLD_STAT_MULT: f32 = 1.8;
pub const PLATINUM_STAT_MULT: f32 = 2.0;
pub const PRISMATIC_STAT_MULT: f32 = 2.15;

/// The least a rare tier adds to a stat an item already has, per rung —
/// `EquipmentStats::for_rarity`'s floor, and the exact counterpart of
/// `ITEM_FUSION_MIN_BONUS_PER_TIER`.
///
/// Gear ships at 1..=4 points a stat, where `SILVER_STAT_MULT`'s 1.5x
/// rounds a 1 to 2 but leaves a 3 at 5 and a 2 at 3 — fine — while the
/// *percentage alone* would let a future retune of `stat_mult` quietly
/// produce a tier that changes nothing on the smallest items. The floor is
/// what makes every rung observable on every item that has the stat at all.
/// A stat sitting at zero stays at zero: a tier sharpens what an item does
/// and does not hand it a new stat, which is the affix's job.
pub const GEAR_RARITY_MIN_BONUS_PER_RUNG: i32 = 1;
