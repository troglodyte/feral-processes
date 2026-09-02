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
//! - **Type invariants.** `components::POWER_MAX`/`POWER_MIN` bound what
//!   `PowerReserve` may hold; every reader assumes they hold. They live beside the
//!   type they constrain.
//! - **Infrastructure.** `world::CHUNK_SIZE`, `save::SAVE_FORMAT_VERSION`,
//!   `resources::MESSAGE_LOG_CAP`/`EFFECT_QUEUE_CAP`, `MAX_CUSTOM_NAME_LEN`
//!   and the `*_ID` string identifiers are sizing and plumbing, not
//!   difficulty.
//! - **Simulation-only values.** `balance_sim::TURN_CAP` and the guard
//!   constants beside it tune the offline projections, not the game.

use crate::components::{Rarity, Stats};

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
    mitigation: 2,
};

/// Points offered on the character-creation stat screen, spent **on top
/// of** `PLAYER_BASE_STATS` rather than redistributing it — so `balance_sim`'s
/// modelled floor stays valid no matter how the pool is spent.
///
/// **The pool size is the all-Atk offense ceiling**, because Atk is priced
/// 1-for-1 and always will be (`CREATION_COST_ATK`'s reason). At 20 the
/// widest offensive build opens on 26 atk against the baseline 6, and the
/// widest defensive one on 210 `max_hp` against 90.
///
/// **20, and this is a design decision that overrides a balance argument.**
/// The number was 5, set against a rejected 10 on the grounds that 16 atk
/// was 2.7x the level-1 offense `balance_sim` treats as its floor. 20 is
/// 4.3x that floor, so the opening zone is meant to be soft for a player
/// who spends the whole pool on one axis — the screen is a build-defining
/// choice rather than a garnish, and `Game::level_cap` and the zone curve
/// are what the run is paced by. `balance_sim` models the *unspent*
/// baseline, so none of its curves move with this; what it cannot see is
/// exactly what this constant now buys.
///
/// It sits **exactly on** `MAX_CREATION_STAT_POINTS`, so raising it
/// further is a deliberate re-argument of that bound rather than a nudge —
/// the `const` assertion below fails otherwise.
pub const CREATION_STAT_POINTS: u32 = 20;

/// Pool points one point of Integrity costs. See `CREATION_GAIN_INTEGRITY`
/// for what a point buys.
pub const CREATION_COST_INTEGRITY: u32 = 1;

/// Pool points one point of Atk costs — priced 1-for-1, the same rate
/// `Reward::RandomMainStat` already grants it at.
pub const CREATION_COST_ATK: u32 = 1;

/// Pool points one point of Decompiler (`Decompiler::skill`) costs — priced
/// 1-for-1, like Atk.
pub const CREATION_COST_DECOMPILER: u32 = 1;

/// Pool points one point of Def (`Stats::mitigation`) costs — priced
/// 1-for-1 like the other three axes.
///
/// **Three was the argument, and the instrument refuted it.** The claim
/// was that mitigation is the one axis levelling never raises (see
/// `HP_PER_LEVEL`'s doc comment on why there is no mitigation-per-level
/// constant at all), so pricing it like the rest would make it dominant on
/// a screen where the player chooses rather than a roll. It was not
/// dominant at any price: a unit is **one percentage point** on a base of
/// 2, and `docs/measurements/2026-09-01-creation-stat-pool-exchange-rates.md`
/// measured the whole 5-point pool spent on Def as **byte-identical to the
/// control** over 200 fights — same win rate, same round count, same HP
/// left. Priced at three it was a trap row: the dearest axis and the only
/// one that moved nothing. At one, a full-Def build reaches 11%
/// mitigation, which is the first spend on this axis a fight can see.
pub const CREATION_COST_DEF: u32 = 1;

/// `Stats::max_hp` granted per point of Integrity bought on the creation
/// stat screen — and `Stats::hp` with it, unconditionally: a run must not
/// start damaged, the trap `MainStat::Integrity`'s own doc comment records.
pub const CREATION_GAIN_INTEGRITY: u32 = 6;

/// Ceiling on `CREATION_STAT_POINTS`, asserted rather than trusted —
/// `MAX_PROFILE_STAT_POINTS`'s reason: a permanent buff with no ceiling is a
/// shape this design has already closed off twice. The shipped pool now
/// sits exactly on it, so there is no headroom left and the next raise has
/// to move this constant on purpose — which is the point of it.
pub const MAX_CREATION_STAT_POINTS: u32 = 20;

const _: () = assert!(CREATION_STAT_POINTS <= MAX_CREATION_STAT_POINTS);

/// Perk Points the character-creation perk step hands out, on top of
/// whatever the cross-run profile grants.
///
/// **Four, which is two of the cheap perks or one of the dearest.** The
/// screen exists to be a decision on every run including a first one, and
/// a fresh profile grants nothing — perks are otherwise earned at
/// `PERK_POINTS_PER_LEVEL` a level, so this is two levels of perk income
/// as a head start. Shipped costs run 2..=4.
///
/// **Ungated by any instrument.** `balance_sim` models no perks at all —
/// each is a hook into a particular formula (see `perks.rs`) — so nothing
/// in the suite can tell you what raising this is worth.
///
/// **Spent at creation or lost**, exactly like the stat pool and the kit
/// allowance — so the step refuses to be left while anything on it is
/// still affordable, the same rule as the other two. The allowance is
/// never granted as points: `apply_creation_perks` grants only what the
/// basket costs and immediately spends it, because `Game::new` runs that
/// same path with an empty basket and must keep producing today's player.
pub const CREATION_PERK_POINTS: u32 = 4;

/// Credits the character-creation kit step gives the player to spend on
/// `items_db::creation_shelf`.
///
/// **25, because that is the band the shipped class kits already occupy.**
/// Priced through `items_db::value_of`, the five `assets/classes/` kits are
/// worth Leech 11, Striker 15, no-class fallback 21, Medic 23, Bastion 29
/// and Saboteur 35. A picked kit therefore trades *shape* against an
/// authored one rather than size — `roll_points_spread`'s principle on the
/// other pool. Set it well above that band and the step is a free upgrade
/// over every class; well below and picking is a punishment for engaging
/// with the screen.
///
/// Whatever the basket leaves unspent arrives as Credits, so this is also
/// the most a run can open holding.
pub const CREATION_CREDITS: u32 = 25;

/// The dearest item `items_db::creation_shelf` will offer, in the same
/// Credits `CREATION_CREDITS` is denominated in.
///
/// **This is a layout constraint as much as a design one.** The creation
/// wizard promises no scroll — `the_tallest_creation_step_fits_its_screen`
/// holds every step under `popup::popup_max_rows`, 28 at 1280x720 — and the
/// shelf is derived from a **moddable** item set, so its length is this
/// number's consequence. At 8 the shipped assets stock 23 rows; at 12 they
/// stock 26 and the census has no room left for the step's footer.
///
/// What 8 buys the player: every material, all three companion upgrades,
/// the rest charge, and six pieces of tier-1 gear. What it keeps off: the
/// zone-gated gear from `arc_lance` (12) up, which is a run's reward.
pub const CREATION_SHELF_MAX_VALUE: u32 = 8;

/// Hard cap on how many rows `items_db::creation_shelf` returns, after the
/// `CREATION_SHELF_MAX_VALUE` filter.
///
/// The *mod* safety net, not the shipped bound: a modded `assets/items/`
/// full of cheap items would otherwise push the step past the wizard's
/// no-scroll ceiling, and this truncates instead — `MAX_NEED_ROWS`
/// trimming before the WORK box's own cap, for that reason. The shipped
/// set is held well under it by `the_shipped_shelf_fits_the_wizard`, so
/// reaching this cap is a signal to retune the ceiling above, never to
/// raise this.
pub const CREATION_SHELF_ROWS: usize = 26;

/// Flat stat growth per level-up, before `growth_multiplier` scales it —
/// see `progression::stats_after_levels`.
///
/// These are `K = 2` times what they were, along with every other constant
/// denominated in entity level (`PERK_POINTS_PER_LEVEL`,
/// `DECOMPILER_SKILL_PER_LEVEL`, the two `ABILITY_*_SCALE_PER_LEVEL` rates)
/// and the reciprocal of every constant denominated in *levels per*
/// something (`PLAYER_ROUTINE_SLOT_PER_LEVEL`,
/// `COMPANION_ROUTINE_SLOT_PER_LEVEL`, `TALENT_START_LEVEL`,
/// `WORK_XP_LEVEL_CAP`). Half as many level-ups, each worth twice as much,
/// paid for by `XP_PER_LEVEL_STEP`'s matching `K^2` — so the *power* curve
/// is where it was and only its grain changed. A level-up is meant to be an
/// event rather than a tick.
///
/// ATK at 2 also buys back granularity that was silently missing: at 1, a
/// `growth_multiplier` had to cross a rounding boundary (roughly +0.5) to
/// move it at all, so `HP_PER_LEVEL` carried nearly all of a species' growth
/// rate on its own — see `progression::scaled_growth`.
///
/// There is deliberately no mitigation-per-level constant. Mitigation is
/// percentage points and a percentage that grows per level approaches
/// immunity, so levelling buys HP, attack, accuracy and evasion and never
/// mitigation — see `components::Stats::mitigation`.
pub const HP_PER_LEVEL: i32 = 24;
pub const ATK_PER_LEVEL: i32 = 2;

/// Growth-rate multiplier for anything with no species-specific rate of
/// its own. The player (who has no species at all) always levels at this
/// rate; it's also `SpeciesDef::growth_multiplier`'s default, so a species
/// file written before that field existed keeps growing exactly as before.
pub const BASELINE_GROWTH_MULTIPLIER: f32 = 1.0;

/// The level at which talent points begin: a companion earns one per level
/// above this, spent on its class tree in `assets/talents/`.
///
/// **It was `CREATURE_MAX_LEVEL` and it was a cap.** Under that name it was
/// the ceiling every creature stopped at, with a Kernel Ring buying levels
/// above it. `Game::level_cap` is the ceiling now — one number for the
/// player and every companion, read off the zone — and this constant kept
/// only its other job. The rename is not tidying: a constant whose meaning
/// changes under a name it keeps compiles perfectly and misleads every
/// reader after it, and nothing here would have failed to build.
///
/// It shares a value with `ZONE_LEVEL_CAP_FLOOR` and that is a coincidence;
/// see that constant.
///
/// Halved by `HP_PER_LEVEL`'s `K = 2`, so a level costs the same *power* it
/// always did.
pub const TALENT_START_LEVEL: u32 = 6;

// ---- Companion development: rings and talents -------------------------
//
// A Privilege Ring, dropped only by an underground lair guardian, opens a
// Kernel Ring on one program. A ring **no longer buys levels** — the zone
// caps everyone at the same number now — it buys the right to *spend* the
// levels already earned above `TALENT_START_LEVEL` on that program's class
// tree (`assets/talents/`). What makes a program individual is which nodes
// it took, not that it is allowed to be bigger than its roster-mates.

/// How many Kernel Rings a single program may have open — see
/// `components::KernelRing`. Each one costs more Privilege Rings than the
/// last (`Game::ring_cost`), so three is already a 1+2+3 = 6-guardian
/// investment in one companion.
///
/// It bounds two things at once: how deep into its tree one program may
/// spend (`Game::talent_points`), and the depth of every talent tree, since
/// `assets/talents/` ships `KERNEL_RING_MAX * LEVELS_PER_RING` tiers and a
/// census refuses a tree that does not. Raising it means authoring a tier for
/// every class, not just changing a number.
pub const KERNEL_RING_MAX: u32 = 3;

/// How many talent tiers one Kernel Ring opens — see `Game::talent_points`.
/// Carries `HP_PER_LEVEL`'s `K = 2` like every other per-level constant.
///
/// It used to buy that many *levels* of ceiling as well. It no longer does:
/// `Game::level_cap` is the one ceiling and a ring does not move it.
pub const LEVELS_PER_RING: u32 = 2;

/// Ceiling on a single `TalentNode::Stat` node's percentage — asserted over
/// the real `assets/talents/` by a census in `tests/assets.rs`.
///
/// A developed companion already carries four multiplicative axes (bought
/// Recompile Kernel tiers, five refactor slots at ~1.28x power, the levels a
/// ring buys, and now talents). Options compound far less dangerously than
/// numbers do, which is why the shipped trees weight toward `Ability`,
/// `Affinity` and `RoutineSlot` and this bound sits low.
pub const MAX_TALENT_STAT_PERCENT: f32 = 15.0;

/// The ceiling on a `TalentNode::Accuracy`'s points, `MAX_TALENT_STAT_PERCENT`'s
/// peer on the axis that has no `Stats` field to bound it.
///
/// Flat rather than a percentage, so it does not compound with the four
/// multiplicative axes a developed companion already carries — and bounded
/// all the same, because Accuracy feeds a ratio: unbounded, one node would
/// walk a companion to `HIT_CHANCE_MAX` on its own and take every later
/// decision in its tree with it.
pub const MAX_TALENT_ACCURACY_POINTS: i32 = 6;

/// The **floor** under the level `arena::set_level` will stage a companion
/// up to; the scenario's own zone cap is the other half, and the higher of
/// the two wins.
///
/// **It was `absolute_companion_level_cap` and it was the live ceiling.**
/// `Game::level_cap` is that now. It survived the rename as the arena's
/// whole ceiling, for a reason that still holds in one direction: five
/// shipped `dev-arenas/` scenarios author `level: 12` and most sit in zone
/// 1, whose cap is `ZONE_LEVEL_CAP_FLOOR` — staging those against the zone
/// cap alone silently clamps every one, which is a failure this repo has
/// already had, where old reports stopped being comparable and nothing said
/// so.
///
/// What inverted on 2026-08-28 is the other direction. Since the zone level
/// cap shipped, this figure sits *below* the cap from zone 2 on, so as the
/// sole ceiling it silently clamped **upward** authoring instead: a zone-3
/// scenario asking for the level-23 party that zone actually permits was
/// staged at 12, and the Stack's depth curve was measured against half a
/// party without anything saying so. See
/// `docs/measurements/2026-08-28-stack-depth-curve-after-danger-steps.md`.
/// Taking the higher of the two keeps the zone-1 property and ends the
/// clamp — a scenario still authors its own composition and has no
/// `KernelRing` to read.
pub const fn arena_level_ceiling() -> u32 {
    TALENT_START_LEVEL + KERNEL_RING_MAX * LEVELS_PER_RING
}

/// The level cap in zone 1, and the flat floor under
/// `Game::level_cap`'s line for every zone the line would put lower.
///
/// It shares a value with `TALENT_START_LEVEL` today and **that is a
/// coincidence**: one answers "how far may anyone develop in the opening
/// zone", the other "at what level do talents begin". Either may be retuned
/// without the other, so neither is expressed in terms of the other.
///
/// Zone 1 is clearable at level 1 by the sim's own measurement, so the floor
/// is not a bound the opening zone ever meets — it is room to develop in
/// before the first breach.
pub const ZONE_LEVEL_CAP_FLOOR: u32 = 6;

/// Levels the cap rises per zone breached.
///
/// **Derived, not chosen.** It is the smallest integer slope that keeps
/// `balance_sim::min_level_to_clear_zone`'s *geared* requirement reachable
/// at every zone measured out to 16 — a cap below that requirement is not
/// difficulty, it is a run that cannot continue. Zone 11 (needs 100, capped
/// at 111) and zone 12 (needs 113, capped at 122) are the binding zones; a
/// slope of 10 leaves zone 12 unclearable.
///
/// The consequence, recorded rather than hidden: the cap sits *above* the
/// gear-free requirement in zones 2-6, so those can still be cleared by
/// levelling alone. The two clear curves both pass near the origin and then
/// diverge, so no single line can sit inside the band at both ends of the
/// range. See `docs/measurements/2026-08-27-zone-level-cap.md`.
pub const ZONE_LEVEL_CAP_STEP: u32 = 11;

/// XP for the first Perk Point bought with overflow — what a player at the
/// level cap pays before they hold any perks.
///
/// See `OVERFLOW_XP_STEP` for why the price is not flat.
pub const OVERFLOW_XP_BASE: u32 = 400;

/// How much more each Perk Point costs per perk already held, so
/// `xp_per_point = OVERFLOW_XP_BASE + OVERFLOW_XP_STEP * perks_held`.
///
/// **The rise is the whole mechanism and zero is not a safe value here.**
/// Perks are uncapped and repeatable at a flat Perk-Point price, and
/// `Perk::Attacker` writes straight into `Stats`, so a flat exchange rate
/// makes overflow XP a linear unbounded power source and the grind this cap
/// exists to end comes back wearing a perk's hat. A linear *cost* makes
/// points earned grow like the square root of XP spent, which loses the race
/// against a linear zone curve forever — which is the point.
///
/// `perks_held` is `Perks::unlocked.len()`, derived and never stored.
pub const OVERFLOW_XP_STEP: u32 = 120;

/// The level cap in `zone` — the one expression of the formula.
///
/// A free function rather than `Game::level_cap`'s body, because
/// `balance_sim` and any bevy system holding a `ZoneLevel` need the same
/// answer and a second copy of a curve is what this repo has been bitten by
/// four times. `Game::level_cap` is a call to it.
pub const fn zone_level_cap(zone: u32) -> u32 {
    let line = 1 + ZONE_LEVEL_CAP_STEP * zone.saturating_sub(1);
    if line > ZONE_LEVEL_CAP_FLOOR {
        line
    } else {
        ZONE_LEVEL_CAP_FLOOR
    }
}

/// Fraction of in-level XP knocked back by a "setback" penalty (a flatline,
/// a Forgiving-mode reboot, or a forced jack-out mid-battle) — see
/// `progression::apply_setback_xp_penalty`. Deliberately mild: it erodes
/// progress toward the next level, never the level or stats themselves.
pub const SETBACK_XP_PENALTY_FRACTION: f64 = 0.2;

/// How much the player's `Decompiler` skill grows per level gained.
/// Carries `HP_PER_LEVEL`'s `K = 2`, so skill still tracks total power
/// rather than level count.
pub const DECOMPILER_SKILL_PER_LEVEL: i32 = 2;

/// Perk Points (see `perks::Perk`) awarded per player level gained.
/// Carries `HP_PER_LEVEL`'s `K = 2`: perks are bought out of total progress,
/// so halving the level count without this would halve the perk budget of a
/// whole run as a side effect of a legibility change.
pub const PERK_POINTS_PER_LEVEL: u32 = 2;

/// Every party member (see `resources::Party`) gains `1 / PARTY_XP_DIVISOR`
/// of whatever XP the player just earned from a kill or successful
/// decompile — see `Game::award_party_xp`.
pub const PARTY_XP_DIVISOR: u32 = 2;

/// XP required to advance from level *N* to *N + 1* is `N` times this — a
/// linear per-level step, so cumulative XP to reach a level grows
/// quadratically. `balance_sim`'s companion-level projection leans on that
/// quadratic shape: half the XP rate lands a companion at roughly
/// `1 / sqrt(2)` of the player's level, not half of it.
///
/// It is `20 * K^2` for the `K = 2` level-coarsening below, and that square
/// is what makes the coarsening cost-neutral rather than a difficulty
/// change: cumulative XP to a level is `(STEP / 2) * L^2`, so doubling the
/// stats a level grants (halving the levels needed for a given power) has to
/// be paid for by four times the step, or the same power would arrive for a
/// quarter of the XP. **None** of this run's slowdown lives here — all of it
/// is `xp_challenge_factor` below. Retuning the two independently is the
/// point: this one is legibility, that one is difficulty.
pub const XP_PER_LEVEL_STEP: u32 = 80;

/// A kill's XP is the victim's max HP scaled by how hard it was — see
/// `progression::kill_xp`. The scale is `power_ratio` (the very number
/// `difficulty_color` buckets into the map's con-colours) over
/// `DIFFICULTY_EASY_MAX`, so full XP lands exactly at the green/yellow
/// boundary and the rule a player can state is "green pays less, yellow and
/// up pays full".
///
/// Sharing `DIFFICULTY_EASY_MAX` rather than carrying a par of its own is
/// deliberate: the colour on the map is the only advance notice a fight's
/// XP value gets, and a second threshold would let the two drift until the
/// glyph lied about the reward.
///
/// Both clamps are load-bearing in opposite directions. Without the
/// **floor**, an over-levelled party earns literally nothing in the opening
/// ring, which is the one place the game deliberately keeps fights trivial
/// (`Game::in_opening_ring`) — 0.25 leaves farming viable but pointless
/// rather than broken. Without the **ceiling**, a Stack guardian pays a
/// multiplier on top of HP that depth has already inflated
/// (`STACK_DEPTH_STAT_STEP`), which is the double-count that made four
/// depth-3 fights worth five levels.
pub const XP_CHALLENGE_FLOOR: f64 = 0.25;
pub const XP_CHALLENGE_CEIL: f64 = 2.0;

/// XP a tamed creature earns for each completed gather cycle.
pub const WORK_XP_PER_CYCLE: u32 = 5;

/// A cronjob worker stops earning XP from `task_progress_system` once it
/// reaches this level — structure work is meant to be a steady, low-effort
/// income, not a way to grind a pet's level without ever battling. Levels
/// above this only come from combat (`Game::award_player_xp` /
/// `award_party_xp`), up to the separate, higher ceiling every creature
/// shares — see `TALENT_START_LEVEL`.
/// Halved by `HP_PER_LEVEL`'s `K = 2`, like the ceiling above it.
pub const WORK_XP_LEVEL_CAP: u32 = 5;

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
/// 128 is four `world::CHUNK_SIZE` chunks: far enough to be a decision the
/// player makes over a session rather than a step they take by accident,
/// close enough that the frontier is reachable without a Portal. This is the
/// knob to move if the field reads too flat or too steep; every other part
/// of the ramp is derived from the zone curve and is not a number to tune.
pub const DANGER_RAMP_TILES: i32 = 128;

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
/// Raised from 4 on 2026-08-09, and the reason is the whole argument for
/// why this number is not free to drift back down.
///
/// These weights enter `Game::choose_wild_action` as `ln(weight)`, so what
/// Defend actually buys is `ln(7/3) = +0.85` of score — and the trained
/// policy's `est_damage_frac` term is worth about `-1.0` against a bracing
/// target, because *reducing incoming damage is what bracing is*. At 4 the
/// two cancelled and came out the wrong way round: bracing drew 40% of the
/// fire against 44% not bracing, so Defend was quietly counterproductive.
///
/// This is not fixable by pinning a feature — a damage-aware policy will
/// always have a reason to walk past the tank — nor by
/// `ENEMY_POLICY_TEMPERATURE`, which divides the prior and the learned term
/// alike and so cannot flip the sign. The prior has to be big enough to
/// win, and 7 is the smallest value that clears
/// `bracing_still_draws_more_fire_under_the_shipped_weights` with margin;
/// 6 flips the sign back but only by 0.02, which is inside the noise.
///
/// Bracing is therefore a stronger taunt than it was against the old
/// random-rolling enemy. That is the intended reading: Defend's old value
/// was an artifact of nobody on the other side thinking about it.
pub const DEFEND_AGGRO_WEIGHT: u32 = 7;

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

/// Extraction-aptitude baseline for a species whose `.ron` file omits
/// `base_int`, **and** the value the player themselves works a node at.
///
/// Deliberately one constant rather than the `DEFAULT_BASE_SPEED` /
/// `PLAYER_BASE_SPEED` pair above. Those two differ and so earn separate
/// names; here the player sitting at exactly the midpoint of the non-boss
/// roster's range (5..15, same as `DEFAULT_BASE_SPEED`'s own midpoint) is
/// the design, not a coincidence — it is what makes posting a sharp program
/// better than doing the job yourself and posting a dull one worse. (The
/// non-boss *mean* runs a touch above it, 10.27, which is fine — it's the
/// baseline both sides are judged against that matters, not a perfect
/// average.) A second constant would let the two drift and quietly delete
/// one side of that pressure.
///
/// It is also the zero point of `systems::mining_success_chance`'s fourth
/// term, so an un-annotated mod species extracts at exactly the rate every
/// species did before `base_int` existed.
pub const DEFAULT_BASE_INT: i32 = 10;

/// Each round every combatant rolls `base_speed + rng(0..=INITIATIVE_DIE)`
/// and acts in descending order. Sized so a 4-point speed gap still loses
/// the roll sometimes — order should be a tendency, not a lookup table.
pub const INITIATIVE_DIE: i32 = 10;

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

/// How sharply a wild program acts on the trained battle policy — the
/// softmax temperature `Game::choose_wild_action` samples its
/// `(move, target)` pair at.
///
/// 1.0 is the trained distribution as-is. Below 1.0 sharpens it, and 0 (or
/// less) is argmax — always the best-scoring pair, which reads as an enemy
/// that never makes a mistake. A large value flattens it back towards the
/// uniform move roll and slot-weighted target roll the game had before
/// policies existed, so raising this is how a policy that trained too well
/// gets dialled back without retraining.
///
/// This is the **only** shipping control over how hard the learned enemy
/// plays, and it is deliberately not backed by `balance_sim`: that gate is
/// RNG-free and models no abilities, so a policy that makes real fights
/// substantially harder moves none of its curves. The arena
/// (`dev-arenas/`, `FERAL_DEV_ARENA=1`) is the instrument here.
///
/// Inert with no `assets/policies/enemy_battle.ron` installed: with no
/// weights there is nothing to sample and the baseline rolls run instead.
pub const ENEMY_POLICY_TEMPERATURE: f32 = 1.0;

/// Mitigation granted for the round by the Defend action, in **percentage
/// points** — the unit `Stats::mitigation` carries. Re-authored from the flat
/// 6 points of subtractive absorption a brace used to add: the value had to
/// move with the unit, or a brace would have silently become a rounding
/// error. Well under `MAX_MITIGATION_PERCENT` so a brace is a real cut
/// without approaching immunity on its own.
pub const DEFEND_MITIGATION_BONUS: i32 = 20;

/// Scales every `AbilityDef::power_cost` in the game, applied wherever one is
/// read — so `Game::ability_unavailable`'s refusal and `Game::spend_power`'s
/// charge move together and cannot disagree. `Phase` and `Jump` are covered
/// too; one knob, no exemptions.
///
/// **This is the lever for the whole routine-cost curve, and the reason it exists
/// is that the shipped numbers' *scale* is inherited while their *ordering*
/// is trusted.** The 55 files carrying a cost were priced against a Fatigue
/// pool that refilled at 0.08 a tick — cheap and renewable. They are now
/// spent out of an irreplaceable underground reserve. The relative ordering
/// between abilities is worth keeping; the absolute scale almost certainly is
/// not, and 1.0 is the starting point rather than a measured answer.
///
/// Ungated by `balance_sim`, which models no abilities at all. Tuning happens
/// in play; the instruments that can see this are `dev-arenas/` and a
/// session. Two levels with different costs: the whole curve moves by editing
/// this and rebuilding, a single ability by editing its `.ron` and
/// restarting — no rebuild, which is the loop that matters mid-session.
pub const ROUTINE_POWER_COST_MULTIPLIER: f32 = 1.0;

/// Below this Power ("Power" is the player-facing label for `PowerReserve.hunger`)
/// threshold, the player's own attacks start losing effectiveness — see
/// `battle::power_attack_multiplier`.
pub const LOW_POWER_ATTACK_THRESHOLD: f32 = 50.0;

/// The Integrity fraction a party member has to cross *downward* in one
/// round to count as wounded — what fires `PassiveTrigger::AllyWounded`.
///
/// A third rather than a half, because half is where an ordinary exchange
/// puts somebody most fights: a threshold that common makes the trigger
/// indistinguishable from `RoundStart` on anything with a cooldown, and
/// stops the moment reading as a crisis at all. Low enough to mean trouble,
/// high enough that the answer still has a round or two to matter — under a
/// quarter, most of what could fire has already stopped helping.
pub const WOUNDED_INTEGRITY_FRACTION: f32 = 0.33;

/// What the player's attack total falls to at zero Power. Between zero and
/// `LOW_POWER_ATTACK_THRESHOLD` the multiplier interpolates linearly from
/// this up to full strength — see `battle::power_attack_multiplier`.
pub const LOW_POWER_MIN_ATTACK_MULTIPLIER: f32 = 0.5;

/// Divisor on the wielded program's ATK and DEF when totalling the bonus it
/// lends the player (see `Game::wielded_stat_bonus`), floored at 1 per stat.
///
/// Was deliberately kept independent of the party's own passive divisor
/// rather than expressed in terms of it, on the grounds that the party buff
/// was a candidate for removal. It was removed on 2026-08-19 and this
/// survived unchanged, which is the whole of what that independence bought.
pub const WIELDED_PROGRAM_STAT_DIVISOR: i32 = 10;

/// Chance that a player strike also fires one of the wielded program's
/// installed routines (see `Game::proc_wielded_routine`). The proc costs
/// nothing — no Power, no cooldown — so this rate is the whole of its price,
/// and it is the one carve-out from `ROUTINE_POWER_COST_MULTIPLIER` reaching
/// every routine. Unguarded by any test: `balance_sim` models no abilities at all,
/// so neither this nor the magnitudes it fires can move a curve.
pub const WIELDED_ROUTINE_PROC_CHANCE: f64 = 0.25;

/// Damage a consumable does when thrown at the wild group rather than used
/// (`Game::throw_item`).
///
/// One point, and meant to stay a ruinous rate. The throw costs no round —
/// it cannot, since an `ActionKind` would have to appear in
/// `battle_action_options`, which both renderers build the prompt from —
/// so this is free damage, and the only thing keeping it from being worth
/// doing is how bad the exchange is. The consumables are finite and cost
/// Credits; a point apiece is a joke, not a strategy.
///
/// Never lethal regardless of this number: `throw_item` clamps so the
/// target keeps at least 1 HP. See its doc comment for why.
pub const THROWN_ITEM_DAMAGE: i32 = 1;

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

/// How the gap between the two sides' `Stats::power` bends a decompile. The
/// ratio is `inspection::power_ratio`'s — the same number `difficulty_color`
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
/// is "you are wearing this program's ICE down", not a run-wide pity meter —
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
/// tuning. The only catalyst is the ICE Breaker at `taming_potency` 0.4, so
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
pub const WILD_LOCAL_DENSITY_TARGET: usize = 12;

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
/// `BASE_PET_CAPACITY`, which is **3**. Six frames of a full stack therefore
/// offer four or five programs to a roster that holds three, so a descent
/// with no base standing behind it still refuses at the end.
///
/// Phase 4 shipped without deciding whether that read as pressure toward
/// capacity-granting structures or as a dead mechanic. It is pressure: the
/// Data Cache's `pet_slot_bonus` was raised to **5** so that one of them —
/// ten Core Fragments, buildable before the first descent — absorbs a whole
/// stack's worth of orphans. The refusal is what a player who skipped the
/// cache meets, not what a thorough descent meets. `balance_sim` models no
/// roster and cannot gate any of that.
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
/// the surface one would make hauling a Mining Node's output down a hole
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

// ---------------------------------------------------------------------------
// Item quality
// ---------------------------------------------------------------------------

/// What a copy compiled exactly to its authored spec is worth, as a
/// percentage of `ItemDef::equipment`'s numbers — and what every copy in
/// every save written before the field existed loads as.
///
/// It is the identity element of `EquipmentStats::for_quality`, which is
/// why the whole band is expressed as a percentage of it rather than as a
/// multiplier: an authored item is the reference, not a floor.
pub const QUALITY_DEFAULT: u8 = 100;

/// The clamp on a rolled quality. Both ends are reachable — a fresh
/// player's craft can hit the floor and a developed base's can hit the
/// ceiling — so they are the band, not guard rails.
pub const QUALITY_MIN: u8 = 70;
pub const QUALITY_MAX: u8 = 130;

/// The three cuts in `items::quality_band`'s four-rung ladder: at or below
/// the first reads as under spec, the middle band as designed, the third as
/// above spec, and anything higher as exceptional.
///
/// The middle band is centred on `QUALITY_DEFAULT` on purpose: every copy
/// in every existing save sits there, so the ladder repaints nothing that
/// is already on screen.
pub const QUALITY_UNDER_MAX: u8 = 90;
pub const QUALITY_SPEC_MAX: u8 = 105;
pub const QUALITY_ABOVE_MAX: u8 = 120;

/// The granularity of a rolled quality. Every term in the roll is a
/// multiple of this and the spread is drawn **in steps** of it, so the sum
/// is on-step by construction and the clamp cannot produce an off-step
/// value.
///
/// Drawn in steps rather than drawn fine and rounded: rounding a uniform
/// draw onto a lattice gives the two end buckets half the width of the
/// others, which biases exactly the ends of the band the player reads for.
pub const QUALITY_STEP: u8 = 5;

/// The luck term — how far above its floor any one compile can roll, drawn
/// as `0..=QUALITY_SPREAD` in `QUALITY_STEP`s.
///
/// It is the same width at every floor, so improving a bench or taking the
/// perk moves the whole band up rather than narrowing it. That is what
/// keeps compiling a batch and keeping the best worth doing at every stage
/// of a run rather than only at the start of one.
pub const QUALITY_SPREAD: u8 = 20;

/// The floor a **found** copy rolls off, giving drops a 70–90 band against
/// a crafted piece's 80–100 at a bare bench.
///
/// Deliberately below `QUALITY_BASE`. Leaving drops at a flat
/// `QUALITY_DEFAULT` was rejected because an average find would then beat a
/// bad craft, which cuts against the whole intent; giving them the crafting
/// band was rejected because the base would then confer no reliability
/// advantage. A lucky find can still beat an unlucky craft, which is what
/// keeps a drop a lottery ticket rather than a disappointment.
pub const QUALITY_DROP_BASE: u8 = 70;

/// The floor a **compiled** copy rolls off before any of its terms: a bare
/// tier-1 bench, no perk, not careful, giving 80–100.
///
/// Deliberately *below* `QUALITY_DEFAULT`, which is what every craft was
/// worth before this axis existed — so a fresh player's gear is a little
/// worse than it used to be and the terms below are what buy it back. The
/// design intent is that a developed base out-produces the world, and a
/// base cannot do that if a bare bench already sits at the authored spec.
pub const QUALITY_BASE: u8 = 80;

/// What one tier of the bench a recipe names is worth, per tier **above
/// the first** — see `Game::best_structure_tier`, which reads a structure
/// with no `StructureTier` as tier 1 so the term is 0 for it.
///
/// This is the whole of why a compile bench is worth upgrading: tier is
/// read nowhere else for a structure without a `ResourceNode`, so the
/// fragments spent on a Fabricator buy better gear and nothing else.
pub const QUALITY_BENCH_PER_TIER: u8 = 5;

/// What one level of `Perk::TightenTolerances` adds to a compiled copy's
/// floor — the player-agency half of the bench term.
///
/// Priced at one `QUALITY_STEP`, the same as a bench tier, so the two read
/// as the same size of investment from opposite directions: one is spent in
/// fragments on the base, the other in Perk Points on the player. Player
/// *level* is deliberately not a term of its own — `scaled_for_level`
/// already scales gear to its wearer, so a level term here would compound
/// against itself late in a run.
pub const QUALITY_PERK_PER_LEVEL: u8 = 5;

/// What the careful-compile toggle adds to the floor.
///
/// One constant and one `bool` rather than a graduated dial: the player is
/// choosing to spend materials for quality, and a slider would ask them to
/// price that choice on every compile.
pub const QUALITY_CAREFUL_BONUS: u8 = 10;

/// What careful compiling costs, as a percentage added to every ingredient
/// line, rounded **up** — so a one-unit line costs two and the toggle is
/// never free.
///
/// Charged on the discounted cost rather than the authored one (see
/// `Game::craft_cost`), so `Perk::LeanCompiler` and this compose in the
/// order the player would expect: the perk makes a recipe cheaper, and
/// being careful about it costs half again of what they actually pay.
pub const QUALITY_CAREFUL_COST_PERCENT: u32 = 50;

/// Range of Portal Fragments a defeated boss guarantees **underground**,
/// multiplied by the frame's depth. The one and only source of the
/// breaching currency: ordinary kills, surface bosses, nests and Stack
/// caches all pay something else, so a zone is breached by going down and
/// killing the thing at the bottom of a stack, or not at all.
///
/// Underground a boss can only ever be a lair guardian —
/// `Game::stack_encounter_pack` draws with `boss` false and
/// `pick_habitat_species` honours it — so this is a lair payout even
/// though it is spelled as a boss one. A lair's escort, past zone 1, is
/// ordinary species and pays nothing.
///
/// Depth is the lever rather than the base range, per
/// `STACK_CACHE_DEPTH_GROWTH`'s argument that the bottom of a stack has to
/// pay better than it costs to reach.
///
/// Linear rather than compounding, and **not** because a zone's total is
/// bounded — it isn't. An individual lair is one-shot (`StackMemory`
/// remembers a cleared one), but `Game::collapse_stack` trades the beaten
/// stack for a fresh link elsewhere in the sector, and the replacement
/// stands on a new tile and so keys a new `FrameSpec` with an uncleared
/// lair of its own. A zone's supply of the breaching currency is therefore
/// renewable, which is what stops a player who spent a payout at a bench
/// from running the zone dry and stranding the run — fragments are an
/// ordinary crafting ingredient besides, and nine recipes and six research
/// projects want them on top of the portal itself. The test holding that is
/// `the_stack_replacing_a_beaten_one_pays_a_lair_of_its_own`.
///
/// Renewable is exactly why the curve stays linear. A compounding one over
/// a source that refills would let a party parked on deep ground print
/// fragments faster than the descent costs them; linear keeps one clear
/// worth about what the trip down to it was.
pub const STACK_BOSS_PORTAL_FRAGMENT_DROP: std::ops::RangeInclusive<u32> = 4..=8;

/// How many Privilege Rings one lair guardian pays — see
/// `components::KernelRing` and `Game::ring_cost`.
///
/// Flat rather than a range, and deliberately not a chance: a ring is the
/// unit the cost ladder is quoted in (1 + 2 + 3 = six guardians for one
/// fully developed program), so a roll here would only add variance to a
/// number the player is already counting. Depth does **not** multiply it the
/// way it multiplies fragments — a deep lair is harder, but a ring is a
/// permission slip rather than a payout, and doubling the supply at depth 3
/// would make the whole ladder a single afternoon.
///
/// Slowly renewable by the same bound as the fragments beside it:
/// `Game::collapse_stack` re-seeds a zone's lairs on a fresh tile, so another
/// ring always costs another run.
pub const STACK_BOSS_PRIVILEGE_RING_DROP: u32 = 1;

/// Upper bound, per zone level, on the `ItemDef::value` of gear a defeated
/// **surface** boss drops — see `Game::surface_boss_loot`. A surface boss
/// pays in power rather than progression: the band walks up the shipped
/// value ladder (scavenged 3-8 → standard 12-16 → researched 20-60 →
/// premium 80-120, documented in `assets/items/README.md`) as zones go by,
/// so "high-end for where you are" is literally what the pool means.
pub const SURFACE_BOSS_LOOT_VALUE_PER_ZONE: u32 = 30;

/// The bottom of that band, as a percentage of its ceiling. Keeps a boss
/// from paying out the cheap tier it long outgrew, without pinning the
/// pool so tightly that a gap in the value ladder empties it.
pub const SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT: u32 = 30;

/// How many items a defeated surface boss draws from that band. Drawn with
/// replacement — a thin band repeats rather than paying less.
pub const SURFACE_BOSS_LOOT_DROPS: u32 = 2;

/// The worst rare tier a **surface boss** may pay — see
/// `Game::pay_surface_boss_gear`. Ordinary drops have no floor and roll the
/// bare `rarity_for_roll` ladder, where a rare copy is a lucky accident.
///
/// A floor here rather than a bigger pile of items, because
/// `NEST_ORPHAN_CHANCE`'s doc already fixes what each of the three faucets
/// pays: a Stack lair boss pays progression, a surface boss pays *power*,
/// and a nest pays roster. Paying power means the gear a surface boss drops
/// has to be better than what the player could have picked up on the way
/// there, and "two more of the same items" is not that — the zone band
/// already decides which items, so the tier is the only axis left that says
/// *this fight was worth it* rather than *this fight took longer*.
pub const SURFACE_BOSS_LOOT_RARITY_FLOOR: Rarity = Rarity::Silver;

/// How often a dropped piece of gear carries an affix — see
/// `Game::roll_affix` and `affixes::AffixDef`.
///
/// Much commoner than a rare tier, and independently rolled, because the two
/// answer different halves of the same complaint. A rare tier is the *chase*:
/// rare enough that seeing one is an event, which at about 3.5% across the
/// whole ladder means most drops never have one. An affix is the *variety*:
/// at roughly one drop in five, it is what stops the other 96.5% being the
/// same item you already have four of. Set this as rare as a tier and
/// ordinary drops stay exactly as featureless as they were.
pub const GEAR_AFFIX_CHANCE: f64 = 0.20;

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

/// How long a wild program holds a tile before `systems::wander_ai_system`
/// offers it another — a half-open range, so the draw is
/// `MIN..MAX` and the longest wait is `MAX - 1`.
///
/// **These two are pinned to app-core's `WORLD_SPEED_MULTIPLIER`**, and
/// nothing makes them fail to compile when one moves without the other.
/// The world runs at that many ticks a real second; these hold a wanderer
/// at the wall-clock pace it had when the world ran at one. They were
/// `2..6` — mean 3.5 — while the clock ticked once a second. `4..11` is
/// the contiguous range whose mean is exactly twice that, 7.0; `4..12`
/// reads like the more obvious doubling and lands at 7.5, which is 7%
/// slower than the pace it claims to hold.
///
/// Deliberately *not* scaled from the multiplier at runtime: wall-clock is
/// the frontend's business and this file is pure tick-space, so the pairing
/// is prose on both ends rather than an argument threaded through the
/// engine.
pub const WANDER_COOLDOWN_MIN_TICKS: u32 = 4;
pub const WANDER_COOLDOWN_MAX_TICKS: u32 = 11;

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

/// Trade currency a destroyed nest pays (see `Game::grant_nest_cache`),
/// before `NEST_CACHE_CREDIT_ZONE_BONUS`.
///
/// Credits rather than the craft currency it used to pay: fragments are
/// `STACK_BOSS_PORTAL_FRAGMENT_DROP`'s alone now, and a nest is the one
/// piece of sustained surface work that a player might do instead of
/// descending. Paying it in the currency that survives a breach keeps it
/// worth clearing without reopening a surface route to the next zone.
///
/// This is the floor under a nest, not its point — see
/// `NEST_ORPHAN_CHANCE` for what a player actually clears one for.
pub const NEST_CACHE_CREDITS: std::ops::RangeInclusive<u32> = 20..=40;

/// Added to `NEST_CACHE_CREDITS` per zone below the current one, so a
/// deeper nest — whose guardians already scale — stays worth clearing.
/// Additive rather than multiplicative, matching `NODE_PAYOUT_ZONE_BONUS`;
/// see that constant for why compounding broke the economy.
pub const NEST_CACHE_CREDIT_ZONE_BONUS: u32 = 10;

/// Chance a destroyed nest leaves an orphaned program of its own species,
/// adopted free — see `Game::grant_nest_cache`.
///
/// This is what a nest is *for*. Each of the three faucets that survive
/// this game's loot pass pays exactly one thing: a Stack lair boss pays
/// progression (`STACK_BOSS_PORTAL_FRAGMENT_DROP`), a surface boss pays
/// power (`SURFACE_BOSS_LOOT_VALUE_PER_ZONE`), and a nest pays roster.
///
/// Free where `Game::adopt_orphan` charges a taming catalyst: the Stack's
/// orphan is an opportunity walked past, a nest is a fight already paid
/// for in durability and respawning guardians. A full roster loses it.
pub const NEST_ORPHAN_CHANCE: f64 = 0.5;

/// Passes over the nest species' equipment drop table
/// (`Game::equipment_drops_for`), each entry rolled at its own chance on
/// each pass. Not a guarantee: a species whose table is empty, or whose
/// chances are low, can still pay no gear at all.
pub const NEST_CACHE_EQUIPMENT_ROLLS: u32 = 3;

// ─────────────────────────────────────────────────────────────────────────
// PowerReserve & rest
// ─────────────────────────────────────────────────────────────────────────

/// Per-tick drain of Power — the one need, see `systems::power_drain_per_tick`.
/// (`PowerReserve::hunger`; "Power" is the player-facing label.) It is the only
/// thing that can starve you, so it alone paces a session.
///
/// It is now also the budget every routine call is priced in, which changes
/// what a cost is *worth* without changing this number. Under the Fatigue
/// meter this replaced — which refilled at 0.08 a tick — `wild_jump`'s 20.0
/// was about 250 ticks of walking. Denominated in Power, underground, with
/// the regen hole in `power_regen_system` closed, it is a fifth of a reserve
/// with no supply. The costs did not move; the denominator did.
///
/// The one lever over the whole routine-cost curve is
/// `ROUTINE_POWER_COST_MULTIPLIER`. This constant still only paces
/// starvation.
pub const HUNGER_DECAY_PER_TICK: f32 = 0.15;

/// What a `DifficultyMode::Forgiving` reboot leaves the player with: max HP
/// divided by this (never below 1), and Power topped up to at least the
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
/// player's `Perk::KeenScavenger` adds a third term, and whoever is working
/// the node adds a fourth; see `systems::mining_success_chance`.
pub const MINING_SUCCESS_BASE: f64 = 0.4;
pub const MINING_SUCCESS_PER_LEVEL: f64 = 0.1;

/// What one point of `SpeciesDef::base_int` **either side of**
/// `DEFAULT_BASE_INT` is worth on the mining roll. The shipped *non-boss*
/// roster spans 5 to 15, so the fourth term ranges about -0.10 to +0.10 —
/// enough that a Cipher and a Construct posted to the same Mk1 node visibly
/// disagree (0.58 against 0.40), and small enough that a node's own tier,
/// worth `MINING_SUCCESS_PER_LEVEL` a step, still outruns species choice
/// over a few upgrades. The two bosses run higher still (Overseer 16,
/// Wintermute 18), but neither can ever be tamed or posted to a job, so
/// their `base_int` never reaches this roll.
pub const MINING_SUCCESS_PER_INT: f64 = 0.02;

/// What one point of `SpeciesDef::base_speed` **either side of**
/// `DEFAULT_BASE_SPEED` is worth on the length of a work cycle. The shipped
/// roster spans 6 (Construct) to 14 (Sprite), so a cycle ranges 1.2x to
/// 0.8x the machine's own rate — a Mining Node's 10 ticks becomes 12 or 8,
/// and a Fabricator's 30 becomes 36 or 24.
///
/// Sized like `MINING_SUCCESS_PER_INT`: enough that swapping the posted
/// program is visible on one screen, small enough that upgrading the
/// machine still beats re-running the roster.
pub const WORK_TICKS_PER_SPEED: f64 = 0.05;

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

// --- Class base jobs -------------------------------------------------------
//
// What a posted program's `SpeciesDef::affinity_class` is worth at a
// structure. Three of the five classes have a base job and two deliberately
// do not: with `BASE_PET_CAPACITY` at 3, every program at a machine is one
// absent from the party, so a Striker or a Saboteur being a waste at a post
// is what makes roster composition a cost rather than a formality.

/// Extra units a **Leech** draws from each successful gather cycle, on top of
/// `systems::node_payout`.
///
/// Flat rather than proportional, which makes it worth most at the shallow
/// end where the tap itself pays 1-2 a cycle — a class is meant to be worth
/// posting before the base is established, not after. It rides the same
/// branch `node_payout` does, so a `flat_payout` or banked node is untouched:
/// `research_data` is the game's only banked item and its flat 1 is the whole
/// of what keeps an uncapped bank honest against a fixed research ladder.
pub const LEECH_YIELD_BONUS: u32 = 1;

/// How many times a **Bastion**'s Defense counts when it is the program
/// guarding a structure a GC Entropy Sweep lands on.
///
/// The job is smaller than it reads: `Game::run_raid` finds its defender by
/// `Task::target` alone, so *every* posted program has always mitigated by
/// its DEF and this is a multiplier on behaviour that already existed. It is
/// stated as a multiplier rather than a flat bonus for exactly that reason —
/// a Bastion is the class whose stat shape spends most of its budget on DEF,
/// so doubling what that shape is already good at is what makes the post a
/// use of the *species* rather than a use of a program.
pub const BASTION_DEF_MULTIPLIER: i32 = 2;

/// Durability a posted **Medic** restores to the structure it guards, per
/// `STRUCTURE_REGEN_INTERVAL` — on top of whatever the base's Patch Nodes
/// restore to everything.
///
/// Per structure rather than a contribution to `Game::total_repair_rate`,
/// which is what makes *where* it stands a decision about what to protect.
/// Flat rather than scaled by the program's level, because the figure wants
/// a base played with one posted rather than a formula: 2 beats a Mk1 Patch
/// Node on that one structure and loses to a Mk3 across a whole base, which
/// is the trade it should be.
pub const MEDIC_REPAIR_PER_INTERVAL: u32 = 2;

/// Divisor applied to the *lesser* of two fused programs' stats: a fusion
/// keeps the better parent's stat outright and adds this fraction of the
/// weaker one (see `Game::fuse_companions`). Bounded further by
/// `MAX_FUSIONS`, so the compounding can't run away.
pub const FUSION_LESSER_STAT_DIVISOR: i32 = 2;

// ---------------------------------------------------------------------------
// Contracts
// ---------------------------------------------------------------------------

/// How many contracts a run may hold at once. Exceeding it is refused with
/// `ContractRefusal::TooMany` rather than silently capped — the "no silent
/// caps" rule.
///
/// 3 so a session has a shape without becoming a checklist: the point is to
/// answer "and now?", not to hand the player a queue to work through.
pub const MAX_ACTIVE_CONTRACTS: usize = 3;

/// How many offers a Broker shows at once.
pub const CONTRACT_BOARD_SLOTS: usize = 3;

/// How many cycles a board's offers stand before the epoch advances and the
/// board re-derives — see `Game::contract_board`.
///
/// An opening guess, and the one figure in this feature worth watching in
/// play: with no deadlines and a long refresh, a player who takes nothing
/// sees the same three offers for a long stretch and the board reads as
/// static. Nothing instruments this — `balance_sim` cannot see a contract.
pub const CONTRACT_REFRESH_CYCLES: u32 = 400;

/// The dearest item a rolled `Deliver` may ask for, in `ItemDef::value`.
///
/// A delivery is asked for by the score, and `ItemDef::value`'s ladder runs
/// printable 1 → scavenged 3-8 → standard 12-16 → researched 20-60 → premium
/// 80-120. Only the bottom rungs are things a base accumulates twenty of, so
/// this sits at the top of the scavenged band. Raising it puts researched
/// goods on the board as bulk errands; an etched Routine Disk is 20.
pub const CONTRACT_MAX_DELIVER_VALUE: u32 = 8;

/// How many points around the base's doorstep a rolled contract samples to
/// decide which programs live in this sector.
///
/// A sample rather than a walk because `Game::contract_board` is on a
/// per-frame path — the contracts screen and the base menu's row test both
/// call it — while the ring it walks grows with a base that can reach
/// `MAX_BUILD_RADIUS_TILES`. Biome features run about 25 tiles across at
/// `WorldMap::classify`'s noise scale, so 32 points stay inside one feature
/// apiece out to the widest base the game allows.
pub const CONTRACT_HABITAT_SAMPLES: i32 = 32;

// ---------------------------------------------------------------------------
// Production chains
// ---------------------------------------------------------------------------

/// How many units a structure's output buffer holds when its `.ron` file
/// sets no `capacity` — see `components::Stock`. This is what paces an
/// extractor now that a node has no deposit pool: it produces until the
/// buffer is full and then clogs until someone collects, so this number is
/// how long a base runs unattended.
pub const DEFAULT_OUTPUT_CAPACITY: u32 = 20;

/// How many units a posted program carries to a depot in one trip.
///
/// The cap is what makes `components::Carrying` a single `(item, qty)` pair
/// rather than a map — an uncapped drain of a `BTreeMap` output would have to
/// carry every id in it. It is also what leaves a buffer behind for a
/// downstream neighbour to keep pulling from across the round trip: at 5
/// against a default buffer of 20, a clog sheds a quarter and leaves the rest.
pub const HAUL_CARRY_CAPACITY: u32 = 5;

/// Chebyshev radius of the ring a program *arrives* in base space on,
/// around the Home.
///
/// Not where idle staff live — they wander, `work_orders::wander_step`.
/// This is the tile `work_orders::entry_tile` hands a body that has no
/// base-space cell yet, which in an established base is a program that has
/// just been beaten and still carries the surface tile it was taken on.
/// Outside 1 so an arrival never lands on the Home's own tile, and inside
/// the starting pocket so a base at its opening size has somewhere to put
/// one.
pub const IDLE_STAFF_RING_TILES: i32 = 3;

/// How many ticks an idle program holds a tile before drifting to a
/// neighbouring one.
///
/// Purely cosmetic — it is what makes a waiting pool read as *waiting*
/// rather than as frozen or as jittering. Deliberately not a random walk:
/// the direction is a function of `(staff index, step)` and draws no RNG at
/// all. A milling draw taken every tick for every idle program would shift
/// the shared stream harder than anything else in the game, and `CLAUDE.md`
/// records three separate occasions where a shifted stream silently rewrote
/// a seeded test in an unrelated file.
pub const IDLE_STAFF_STEP_TICKS: u64 = 6;

/// Integrity fraction below which a base-staff program takes itself off the
/// line and goes to a Repair Bay — see `Game::admit_the_badly_hurt`.
///
/// **The admission line, and there is deliberately no matching release
/// line.** The obvious second constant — leave the Bay at some higher
/// fraction — is the shape `MORALE_RECOVERED_AT` has, and it is the wrong
/// shape here: a Bay already had an exit condition before this threshold
/// existed, `run_repair_bays` dropping `components::Downed` at *full*
/// Integrity, and two ways out of one state is how they come to disagree.
/// Full is the release, so the hysteresis gap is this number to 1.0 — far
/// wider than any flicker could cross, and held by
/// `an_admitted_program_is_not_released_until_it_is_whole`.
///
/// Low on purpose. This pulls a working body off a machine, so it has to
/// mean *this program is about to be destroyed*, not *this program has been
/// in a fight*: set anywhere near half, a base that survives a sweep sends
/// its whole staff to queue at the Bay and stops producing for as long as
/// they take to mend.
pub const BAY_ADMISSION_HP_FRACTION: f32 = 0.20;

/// How far a hauling program's cost field reaches, centred on the tile it is
/// walking to — twice the *live* build radius, because two structures in one
/// base can sit at opposite corners of the slab and a worker may be standing
/// just outside it. A constant is what this used to be, and it cannot be one
/// any more: a base grown past the starting radius would refuse postings
/// across its own width, and `hauling::post_reach` is the single predicate
/// the cronjob menu and the assignment share, so a posting the menu accepted
/// would be one that never arrived.
///
/// Bounding the search at all is what stops a walk toward something
/// unreachable generating chunks forever on a lazily-generated infinite map
/// — the same reason `pursuit_field` bounds its successors.
///
/// `HAUL_WALK_MAX_TILES` is the second bound, and it is a performance floor
/// rather than a design one. The field is a Dijkstra search over a disc, so
/// its cost is quadratic in the reach, and the reach is twice the radius —
/// which makes it quartic in base size. Measured in a debug build, one
/// posted worker walking, per tick: 8 ms at radius 10, 28 ms at 20, 65 ms
/// at 30, and **764 ms at the ceiling of 100**. A tick is a keypress, and
/// several workers walk at once, so the far end of that is not a game.
///
/// Capping the reach keeps `post_reach` and `haul_step_system` in agreement,
/// because both read this one function: on a base wider than the cap allows,
/// the cronjob menu refuses a post the walker could not have completed and
/// says to get closer, rather than accepting one that never arrives.
pub fn haul_walk_radius(build_radius: i32) -> i32 {
    (build_radius * 2).min(HAUL_WALK_MAX_TILES)
}

/// The furthest a hauling program's walk field ever reaches, however wide
/// the base — see `haul_walk_radius` for the measurements behind it. Set so
/// a base up to radius 30 is crossable end to end, which is three times the
/// size the Heap Pillar's cost makes routine.
pub const HAUL_WALK_MAX_TILES: i32 = 60;

/// How many full batches of each ingredient a machine will pull into its
/// input before refusing more. Two, so a machine always has the next batch
/// staged while working the current one, but a greedy machine still cannot
/// drain a feeder that several machines share.
pub const INPUT_STOCK_BATCHES: u32 = 2;
pub const DEFAULT_STRUCTURE_DURABILITY: u32 = 30;

/// Step added to an item's base `EquipmentStats` per gear level above 1 —
/// level *N* = base * `(1 + GEAR_LEVEL_STEP * (N - 1))`, matching
/// `ZoneLevel::stat_multiplier`'s own per-zone step so neither levelling nor
/// gear dominates the other outright. Gear level is capped by
/// `resources::ZoneLevel`: reaching zone *N* is what "unlocks" level *N*
/// gear — see `Game::equip`.
///
/// Linear because the zone curve it is matched to is (see `ZONE_STAT_STEP`).
/// Keeping this geometric against a linear enemy curve would invert the old
/// bug rather than fix it — gear would outrun the zones instead of falling
/// behind them, which is what `balance_sim::best_case_gear_bonus`'s tests
/// caught the last time these two disagreed.
/// **Known cost, accepted rather than overlooked.** An equipped item's bonus
/// is written into `Stats` by `apply_equipment_delta` and a save restores
/// those numbers verbatim, so a save written before this became linear
/// carries the old geometric bonus baked in. Taking that item off subtracts
/// the *new*, smaller figure and welds the difference into the player's base
/// stats — the `EquippedItem::fusion_tier` trap, one rung along. It affects
/// gear equipped at zone 2 or deeper only (level 1 is unscaled under both
/// curves, so it is exactly zero there), it is a one-off per worn item, and
/// it errs in the player's favour. It is *correctable* — since
/// `SAVE_FORMAT_VERSION` 29 the payload is field-named RON, so a
/// `#[serde(default)]` flag saying "this save's worn bonus is already
/// linear" would load out of a file written before it existed and needs no
/// bump at all. It is not done because the error is one-off, bounded by the
/// item's base stats, and generous; a migration that rewrites a saved
/// `Stats` to claw a few points back is more ways to be wrong than the bug
/// is worth.
/// `an_equip_and_its_unequip_cancel_exactly_at_every_zone` holds the
/// invariant for every save written from here on.
pub const GEAR_LEVEL_STEP: f64 = 1.0;

/// Bonus `Game::fuse_item` adds to an item type's equipped stats, per
/// fusion tier — additive, not compounding (tier 2 is +20%, not +21%).
pub const ITEM_FUSION_BONUS_PER_TIER: f64 = 0.20;

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

/// How many percentage upgrades one program may carry (see
/// `components::Refactors`), in the same spirit as `MAX_FUSIONS` above.
///
/// The cap is what stands between a craftable buff and infinite stats: those
/// buffs come off a chain rooted in a Mining Node, which produces Core
/// Fragments forever, and turns are free — so nothing else bounds how many a
/// player can compile. The zone-bump track needs no such cap because it
/// bounds itself, refusing once the program has caught up with the player.
///
/// It is also what makes the choice interesting, since five slots across
/// three stats is a specialisation rather than a shopping list.
pub const MAX_COMPANION_REFACTORS: u32 = 5;

/// What an item is worth when its `.ron` file names no `value` — the flat
/// rate every item in the game traded at before the price ladder existed,
/// so a mod written against the older schema keeps its old behaviour.
///
/// Doubles as the ladder's floor. Anything a base can print on a timer is
/// pinned here by `every_base_produced_item_sits_at_the_floor_price`, for
/// the reason that test documents: a `work.produces` item's value is really
/// a Credit-per-tick rate, and the recipe ceiling below cannot see it.
pub const DEFAULT_ITEM_VALUE: u32 = 1;

/// What an etched Routine Disk is worth, and what an *exclusive* one is —
/// the `ItemDef::value` `ItemDb::synthesise_etched_disks` gives every disk
/// it derives.
///
/// An ordinary disk is worth a good deal more than the blank it was written
/// on (`routine_disk.ron` values a blank at 5), because the labour of
/// knowing the routine went into it. The exclusive figure is set well below
/// `STACK_MARKET_EXCLUSIVE_PRICE`: a trader pays `STACK_MARKET_SELL_RATE`
/// per point of value, so a disk that sold back for what it cost would make
/// the shelf a laundry — buy the row, walk two frames, sell it to the next
/// trader, repeat. Sixty against fourteen hundred is a loss the player takes
/// on purpose or not at all.
pub const ETCHED_DISK_VALUE: u32 = 20;
pub const ETCHED_DISK_EXCLUSIVE_VALUE: u32 = 60;

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
/// **Above `BASE_PET_CAPACITY` (3) on purpose**, so raids cannot begin
/// until the base has grown a roster structure — a Data Cache, the one
/// shipped `pet_slot_bonus` — or the player has taken the roster perk. A
/// floor at or below the opening capacity meant a base being swept while
/// its whole roster was still the three programs it started with, which is
/// the attrition this constant exists to hold off. Pinned against the
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

/// The radius the base starts at — a 9x9 pocket of ~69 buildable tiles,
/// halved from the 15x15 it was until the Heap Pillar shipped and never
/// grown by anything today: `Game::build_radius` and the Heap Pillar's
/// `build_radius_bonus` that widened it both retired with
/// `resources::Platform`, and nothing has replaced "how wide is a base" as
/// a *derived* question yet — `Game::lay_starting_pocket` is the only
/// reader, and it just lays this much floor once. The opening base is
/// deliberately cramped — growth is the feature, and a base that starts at
/// its final size can never read as a settlement that grew; slice 2's
/// mining is what makes it grow again.
pub const MAX_BUILD_DISTANCE_FROM_HOME: i32 = 4;

/// How far the pre-cleared pocket reaches from base space's own origin when
/// the first Home is deployed — `Game::lay_starting_pocket`, which lays
/// `BaseCell::Floor` over the chamfered box this radius and
/// `PLATFORM_CORNER_CUT` describe and writes nothing into `WorldMap` at all.
///
/// **Deliberately equal to `MAX_BUILD_DISTANCE_FROM_HOME` above, and
/// deliberately not defined as it.** The opening base has to play exactly as
/// it did when it was a slab stamped onto the zone surface — that is the
/// whole claim of the relocation, and the same 69 buildable cells is what
/// makes it checkable. But the slab constant belonged to `resources::Platform`,
/// which is deleted now, and a pocket that followed it by reference would
/// have gone wherever that deletion left it rather than staying where it was
/// measured.
///
/// This is the *starting* size, the way the old constant was the starting
/// radius: slice 2's mining is what makes floor space something a player
/// buys, and this is only the ground the run opens with.
pub const STARTING_POCKET_RADIUS: i32 = 4;

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
// `docs/superpowers/specs/2026-08-19-base-out-of-phase-design.md`.

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
/// **Bounded above by the Mining Node's rate**, and that bound is the whole
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

/// How long a cut cell survives unfloored before base space takes it back —
/// `game::base::entropy`, which reverts it to solid rock rather than to
/// chipped rock, so re-opening it costs the swings it cost the first time.
///
/// The pressure the rest of slice 2 is drawn against: cutting is cheap and
/// flooring is not, so without a clock on the frontier a player would open
/// the pocket outward and floor it at leisure. It wants to be long enough
/// that an ordinary dig-then-floor cycle never loses ground and short enough
/// that over-digging is felt, and **it has never been measured** — the spec's
/// own open question. Play it and record the answer under
/// `docs/measurements/`.
///
/// A laid `BaseCell::Floor` is untouched by it at any age, and so is any
/// cell with a body standing on it.
pub const BASE_ENTROPY_REFILL_TICKS: u64 = 300;

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
pub const ATTACKER_BONUS_PER_LEVEL: i32 = 2;

/// Permanent DEF `Perk::Defender` adds to the player's `Stats`, per level.
pub const DEFENDER_BONUS_PER_LEVEL: i32 = 2;

/// Percentage of current max Integrity `Perk::Buffer` adds to the
/// player's `Stats`, per level.
pub const BUFFER_BONUS_PERCENT_PER_LEVEL: f32 = 0.01;

/// Floor on `Perk::Buffer`'s per-level max Integrity bonus, so it's still
/// worth buying early when 1% of max Integrity would round to less than
/// this.
pub const BUFFER_MIN_BONUS_PER_LEVEL: i32 = 10;

/// Fraction of an incoming Trace rise `Perk::Obfuscation` cancels, per level.
///
/// A proportion rather than a flat subtraction because what raises Trace runs
/// from `TRACE_PER_KILL` (2) to `TRACE_PER_BREAKPOINT` (25): a flat term large
/// enough to be felt against a breakpoint would zero every kill in the game,
/// and one small enough to leave a kill alone would be invisible against the
/// rest. Five levels therefore halves the whole schedule rather than deleting
/// its cheap half.
///
/// The rate has to clear rounding to be worth anything, which is what sets
/// it: `perks::trace_after_obfuscation` rounds, so a level shows up only once the source
/// times the reduction reaches a half. At 0.10 that is one level against a
/// cache (10) or a breakpoint (25) and three against a kill (2). At 0.05 a
/// cache took two levels to move at all — the first purchase bought
/// literally nothing on every source but the breakpoint, which is not a
/// perk anyone would buy a second level of.
///
/// `perks::trace_after_obfuscation` floors the result at 1, so however many levels are
/// stacked a source still costs something — see `Perk::Obfuscation` for why
/// that floor is not the same call `LOW_POWER_MODE_REDUCTION_PER_LEVEL` makes.
/// Ten levels is where every source reaches that floor, which at
/// `assets/perks/obfuscation.ron`'s price is thirty levels' worth of Perk
/// Points spent on nothing else.
pub const OBFUSCATION_REDUCTION_PER_LEVEL: f32 = 0.10;

/// Roster slots `Perk::ProcessPool` adds per level, on top of
/// `BASE_PET_CAPACITY` (3) and any deployed `pet_slot_bonus`.
///
/// A whole slot per level against a base of 3 is the largest step any perk
/// takes, and it is priced accordingly in `assets/perks/process_pool.ron`: a
/// Data Cache is worth 5 slots for a building's materials, so the perk is the
/// expensive way to the same place, bought when there is nothing to build
/// with.
pub const PROCESS_POOL_SLOTS_PER_LEVEL: usize = 1;

/// Work resource `Perk::Teardown` adds to a kill's drop, per level, on top of
/// the `WORK_RESOURCE_DROP` roll (2..=4).
///
/// This is a permanent income *rate*, not a loop — nothing here mints value
/// out of nothing, and it is bounded by how many fights the player takes,
/// which is what makes it the salvage perk rather than a trader one. It is
/// still the steepest thing in this section relative to what it modifies
/// (+33% to +50% at a single level), so it carries the highest price in the
/// catalogue.
pub const TEARDOWN_SALVAGE_PER_LEVEL: u32 = 1;

/// Accuracy one level of `Perk::TargetLock` adds to every attack the player
/// makes. Peer to `ATTACKER_BONUS_PER_LEVEL`, which buys 2 points of ATK for
/// a comparable price.
///
/// Worth most early, which is deliberate: an unaimed player sits near the
/// `ATTACKER_ACCURACY_ADVANTAGE` baseline for the first ten levels and then
/// climbs on their own, because a hostile's Evasion grows with the zone
/// while theirs grows with their level.
pub const TARGET_LOCK_ACCURACY_PER_LEVEL: i32 = 2;

/// Durability `Perk::Failover` adds to `Game::total_repair_rate` per level,
/// restored to every damaged structure each `STRUCTURE_REGEN_INTERVAL`.
///
/// Sized against the Patch Node's `repair.per_tier` rather than against
/// `RAID_DAMAGE`: this is the same rate a building contributes, so a level is
/// deliberately a fraction of what deploying one is worth. What the perk
/// actually buys is that a base with no Patch Node standing stops taking
/// permanent damage at all.
pub const FAILOVER_REPAIR_PER_LEVEL: u32 = 1;

// ─────────────────────────────────────────────────────────────────────────
// Routine slots
// ─────────────────────────────────────────────────────────────────────────

/// Slots granted per step of level growth, for the player and companions
/// alike. Two: a routine kit is the one place the game asks you to displace
/// something you already have, and at one slot a step every install past the
/// first was a trade rather than a choice. Doubling the grant rather than the
/// step rate keeps every slot landing on the level it always did.
pub const ROUTINE_SLOTS_PER_STEP: u32 = 2;

/// Slots a companion has at level 1 before any per-level growth, then
/// `ROUTINE_SLOTS_PER_STEP` more for every `COMPANION_ROUTINE_SLOT_PER_LEVEL`
/// levels. The floor of 1 in `abilities::companion_routine_slots` is what
/// keeps a level-1 program from having nowhere to hold its innate kit.
pub const COMPANION_ROUTINE_SLOT_BASE: u32 = 0;

/// Levels a companion needs per grant of `ROUTINE_SLOTS_PER_STEP` slots.
/// Halved by `HP_PER_LEVEL`'s `K = 2` alongside `TALENT_START_LEVEL`, so a
/// companion still reaches its ceiling at the level it always did.
pub const COMPANION_ROUTINE_SLOT_PER_LEVEL: u32 = 1;

/// Most routines a companion can hold at once, still reached at level 6 —
/// `ROUTINE_SLOTS_PER_STEP` doubled the kit without moving the level it
/// tops out at.
pub const COMPANION_ROUTINE_SLOT_CAP: u32 = 12;

/// Slots the player has at level 1. Two, and `decompile` occupies one — a new
/// game pre-installs that ability, so the player starts with one free slot
/// rather than having to reach `PLAYER_ROUTINE_SLOT_PER_LEVEL` for it.
pub const PLAYER_ROUTINE_SLOT_BASE: u32 = 2;

/// Levels the player needs per additional routine slot. Deliberately far
/// slower than a companion's: researched routines are meant to be a choice
/// between programs, not a second kit the player accumulates for free.
/// Halved by `HP_PER_LEVEL`'s `K = 2`, so a slot still costs the same
/// progress it used to rather than twice as much.
pub const PLAYER_ROUTINE_SLOT_PER_LEVEL: u32 = 5;

/// Most routines the player can hold at once, still reached at level 25. The
/// player has no level ceiling (`progression::add_xp` takes `None`), so this
/// clamp is the only thing bounding their slots.
pub const PLAYER_ROUTINE_SLOT_CAP: u32 = 12;

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
/// gentle: `ATK_PER_LEVEL` is 2. A +3 attack buff
/// against a base ATK of 6 is already half again; scaling it on the HP curve
/// below would turn the same routine into a tripling.
///
/// Doubled by `HP_PER_LEVEL`'s `K = 2`. Both ability rates are per *level*
/// against stats that are now also per level, so leaving them alone would
/// have halved every routine's late-game magnitude as a side effect.
pub const ABILITY_STAT_SCALE_PER_LEVEL: f32 = 0.30;

/// How much each level adds to an ability magnitude measured in **HP** —
/// `Damage`, `Drain`, `Heal`, and the per-round bite of a `Debuff`. Steeper
/// than `ABILITY_STAT_SCALE_PER_LEVEL` by design, and for the same reason
/// that one is gentle: these are weighed against Integrity, which grows at
/// `HP_PER_LEVEL` (24) per level and doubles again per zone
/// (`ZONE_STAT_GROWTH`).
///
/// Ability damage used not to be level-scaled at all. The damage formula is
/// `power + ATK - DEF`, and ATK was held to carry the progression on its own
/// — but ATK grows at 2 per level against Integrity's 24, so an authored
/// power fell further behind its target every level. By the time a level-10
/// player with the affinity perk five deep faced a 400-Integrity program,
/// the heaviest shipped routine hit for 35. That is what this rate exists to
/// fix; `tests::combat_abilities` pins the resulting figure.
///
/// Doubled by `HP_PER_LEVEL`'s `K = 2` — see
/// `ABILITY_STAT_SCALE_PER_LEVEL`.
pub const ABILITY_HP_SCALE_PER_LEVEL: f32 = 0.80;

/// Level ceiling on both ability scales. The player has no level cap
/// (`progression::add_xp` takes `None`), so without this a long enough game
/// multiplies every routine without bound. A companion is capped far lower
/// by `TALENT_START_LEVEL` and never reaches this.
/// Halved by `HP_PER_LEVEL`'s `K = 2`, so the cap bites at the same power it
/// used to — and doubling the rates above without halving this would have
/// doubled the ceiling itself.
pub const ABILITY_SCALE_LEVEL_CAP: u32 = 20;

/// An ability magnitude's neutral affinity — no bonus, no penalty. The
/// value every `AffinityKind` defaults to, and what an invoker with neither
/// a species nor perks resolves to.
pub const AFFINITY_NEUTRAL: f32 = 1.0;

/// Bounds every affinity is clamped to when a species file is loaded.
/// Deliberately wider than `MIN_INDIVIDUAL_ROLL`..`MAX_INDIVIDUAL_ROLL`
/// (0.8-1.2): a damage affinity scales only an ability's *authored* power,
/// which is a minority of `power + ATK - DEF` at a high level, so a narrow
/// band would make damage affinities imperceptible.
///
/// These compound with whichever level scale the category uses. A companion
/// caps at `TALENT_START_LEVEL` (12), so a stat magnitude's ceiling is 2.8x
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
/// Damage/Drain ability except `broadcast_storm`: the damage formula gives
/// Attacker `ATTACKER_BONUS_PER_LEVEL` flat damage per level for 2 Perk
/// Points on *every* attack, while 0.05/level of authored `power` 10
/// (`packet_shred`, `siphon_cycles`) is only +0.5 damage per level for the
/// same 2 points, and only on that one category. 0.15 is what closed that
/// gap. It no longer closes it at level 1 — `ATTACKER_BONUS_PER_LEVEL` rose
/// to 3 on 2026-08-05, against 0.15 of an unscaled `power` 10's +1.5 — but
/// the comparison is not level-independent and never was: the affinity term
/// multiplies a magnitude that grows at `ABILITY_HP_SCALE_PER_LEVEL`, so it
/// overtakes the flat perk from around player level 3 and keeps widening,
/// while Attacker's 3 is 3 forever. The cost of the higher rate is reaching
/// `AFFINITY_MAX` sooner — 7 levels / 14 Perk Points, instead of 20 levels /
/// 40 — after which further levels buy nothing more from this perk
/// specifically. That tradeoff (worse than Attacker for the first few player
/// levels, better and unboundedly so after, against a ceiling Attacker
/// doesn't have) was the owner's call, not a balance-formula default.
pub const AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED: f32 = 0.15;

/// Floor on the cooldown a hostile arms after spending a routine.
///
/// `AbilityDef::cooldown` is `#[serde(default)]` 0, and a carrier fires
/// whenever its routine is off cooldown — so a mod ability declaring no
/// cooldown would fire every single round. The player side keeps the
/// authored value untouched, which is what leaves `decompile` spammable.
pub const ENEMY_ROUTINE_MIN_COOLDOWN: u32 = 1;

// ─────────────────────────────────────────────────────────────────────────
// Achievement profile ceilings
// ─────────────────────────────────────────────────────────────────────────

/// Most main-stat points a fully-cleared `assets/achievements/` ladder may
/// hand a new run, summed across every `Reward::RandomMainStat`.
///
/// `balance_sim` simulates a run's own curve and deliberately does not model
/// the cross-run profile, so this bound — asserted over the real assets by
/// `the_full_ladder_stays_under_its_ceiling` — is the only gate on how much
/// the profile is worth. A permanent buff with no ceiling is the shape this
/// design has already closed off twice (the scan action, the Market's
/// fragment listing).
///
/// The shipped ladder spends 7. The eighth is budget for a third boss
/// species, so adding one does not move the test.
pub const MAX_PROFILE_STAT_POINTS: u32 = 8;

/// Most Perk Points a fully-cleared profile may hand a new run. See
/// `MAX_PROFILE_STAT_POINTS` for why this is asserted rather than trusted.
pub const MAX_PROFILE_PERK_POINTS: u32 = 5;

/// Most `Reward::StartingProgram` rungs the ladder may carry. One: a second
/// free companion is half a starting party rather than a flavour of one.
pub const MAX_PROFILE_STARTING_PROGRAMS: u32 = 1;

// ---------------------------------------------------------------------------
// Nemesis
// ---------------------------------------------------------------------------

/// Ceiling on how many wild programs may carry `components::Nemesis` at
/// once, counted by querying live holders (`Game::mark_nemeses`) rather than
/// tracked in a resource — the entities already are the ledger, so there is
/// nothing to save and nothing that can drift out of step with them.
///
/// Set well above what a run realistically holds: a nemesis dies when you
/// kill it, and a breach wipes every hostile in the zone along with it. This
/// reads as a runaway backstop, not a difficulty knob.
pub const MAX_NEMESES: usize = 10;

// ---------------------------------------------------------------------------
// Ground conditions
// ---------------------------------------------------------------------------

/// Most of the player's maximum Integrity a single step onto ambient ground
/// may cost, as a fraction (see `environment::EnvironmentEffect`).
///
/// A playability bound, checked against the **folded** effect rather than
/// any one source: ground and a later weather layer both add into
/// `attrition_percent`, so a pair that each stay well inside this ceiling on
/// their own can still sum past it. Terrain is not a fight — it cannot be
/// fled, refused or out-levelled, and a step is the cheapest action in the
/// game — so a folded `0.5` would be death in two steps with no decision in
/// between. Set so a sector of the worst legal ground is a supply problem
/// rather than a countdown — crossing it wants planning, not luck.
pub const MAX_ENVIRONMENT_ATTRITION: f32 = 0.05;

/// Most extra ticks a single step onto ambient ground may cost, on top of
/// the one every step already costs.
///
/// Also a ceiling on the fold rather than on ground alone, for the same
/// reason `MAX_ENVIRONMENT_ATTRITION` is: a tick runs the whole schedule,
/// and a summed drag the player cannot tell from a hang is worse than a
/// single source authoring one. Three is already a step that costs four,
/// which is as slow as ground can be before walking stops being the way you
/// get anywhere.
pub const MAX_ENVIRONMENT_DRAG_TICKS: u32 = 3;

/// Most `EnvironmentEffect::min_damage` may reach, once folded.
///
/// Also a ceiling on the fold rather than on ground alone, for
/// `MAX_ENVIRONMENT_ATTRITION`'s own reason: the floor is what actually
/// decides the bite at low level, since `bite` takes the summed percentage
/// or the summed floor, whichever is larger, and the percentage does not
/// overtake a floor of 2 until `max_hp` reaches roughly 57. Set with
/// headroom over the highest shipped fold (Null Sector's `DanglingReads` +
/// `LeakingMemory`, 1 + 1 = 2) rather than equal to it —
/// `MAX_STATIC_AMBUSH_MULT`'s own reasoning: a ceiling that exactly
/// restates the content is not a guard.
pub const MAX_ENVIRONMENT_MIN_DAMAGE: i32 = 4;

/// Most `EnvironmentEffect::ambush_mult` may multiply
/// `RANDOM_ENCOUNTER_CHANCE` by, once folded.
///
/// Ground itself never sets this above `1.0` — the term exists so a later
/// weather layer has somewhere to land without a second field appearing on
/// the struct then. Set above `SignalNoise`'s own authored `x2.0` rather
/// than equal to it: a ceiling exactly at the one shipped value that reaches
/// it is a restatement of the content, not a guard, and would be
/// unreachable for every combination actually on the table (Null Sector's
/// `SignalNoise` folded with `LeakingMemory` still tops out at `x2.0`, since
/// only `SignalNoise` touches the term at all). Room enough that a second
/// ambush-multiplying source could fold in without instantly saturating.
pub const MAX_STATIC_AMBUSH_MULT: f32 = 2.5;

/// `GroundCondition::DanglingReads`'s attrition, folded with any live
/// weather claiming Null Sector.
pub const DANGLING_READS_ATTRITION: f32 = 0.02;
/// `DanglingReads`'s floor.
pub const DANGLING_READS_FLOOR: i32 = 1;

/// `GroundCondition::ThermalLoad`'s attrition.
pub const THERMAL_LOAD_ATTRITION: f32 = 0.03;
/// `ThermalLoad`'s floor.
pub const THERMAL_LOAD_FLOOR: i32 = 2;

/// `GroundCondition::LockContention`'s extra step cost, on top of the one
/// every step already costs.
pub const LOCK_CONTENTION_DRAG_TICKS: u32 = 1;

// ---------------------------------------------------------------------------
// Static weather
// ---------------------------------------------------------------------------

/// Ticks in one weather epoch — how long a `StaticEvent` (or clear ground)
/// stands before `static_at` re-derives it. Every biome in a zone turns over
/// at the same instant, which is invisible in play: the player is standing
/// in one biome at a time.
pub const STATIC_EPOCH_TICKS: u64 = 150;

/// The implicit "nothing is live" weight every biome's pool carries beside
/// its events' own weights, so most epochs in most biomes are clear.
pub const STATIC_CLEAR_WEIGHT: u32 = 3;

/// `StaticEvent::LeakingMemory`'s pool weight, against `STATIC_CLEAR_WEIGHT`
/// and whatever else claims Null Sector.
pub const LEAKING_MEMORY_WEIGHT: u32 = 1;
/// `LeakingMemory`'s extra attrition, added on top of Null Sector's own
/// `DanglingReads` — the shipped case for "no event is attrition-only except
/// on ground that is already attrition."
pub const LEAKING_MEMORY_ATTRITION: f32 = 0.015;
/// `LeakingMemory`'s extra floor, added to `DanglingReads`'s own.
pub const LEAKING_MEMORY_FLOOR: i32 = 1;

/// `StaticEvent::ThreadStorm`'s pool weight.
pub const THREAD_STORM_WEIGHT: u32 = 1;
/// `ThreadStorm`'s extra step cost, on top of the one every step costs.
pub const THREAD_STORM_DRAG_TICKS: u32 = 1;
/// `ThreadStorm`'s multiplier on `RANDOM_ENCOUNTER_CHANCE`.
pub const THREAD_STORM_AMBUSH_MULT: f32 = 1.5;

/// `StaticEvent::PacketFlood`'s pool weight.
pub const PACKET_FLOOD_WEIGHT: u32 = 1;
/// `PacketFlood`'s extra step cost. Open Grid otherwise carries no standing
/// condition at all — this is the one thing that ever taxes it.
pub const PACKET_FLOOD_DRAG_TICKS: u32 = 1;
/// `PacketFlood`'s multiplier on `RANDOM_ENCOUNTER_CHANCE`.
pub const PACKET_FLOOD_AMBUSH_MULT: f32 = 1.6;

/// `StaticEvent::SignalNoise`'s pool weight — claimed twice, once in
/// Deadlock's pool and once in Null Sector's, so this one number prices
/// both without a second field.
pub const SIGNAL_NOISE_WEIGHT: u32 = 1;
/// `SignalNoise`'s multiplier on `RANDOM_ENCOUNTER_CHANCE`. Carries no
/// damage term at all — the shipped case for an event felt entirely
/// through what it lets happen to you rather than through a bigger number.
pub const SIGNAL_NOISE_AMBUSH_MULT: f32 = 2.0;

// ---------------------------------------------------------------------------
// Sector traits
// ---------------------------------------------------------------------------

/// Least standable ground a sector may leave, as a fraction of the tiles
/// around the origin (see `sectors::walkable_fraction`).
///
/// A playability bound rather than content, which is why it is here and not
/// in the `.ron`: nothing about a threshold delta stops an authored sector
/// generating a map that is almost entirely Data Void and Black Ice, and
/// that is not merely ugly. `enter_next_zone` calls `find_walkable_start` on
/// the new map, every spawn, structure and Stack link refuses an unwalkable
/// tile, and `stamp_platform` needs somewhere to lay the base — a sector
/// with no ground is a stranded run.
///
/// Set well under the neutral shape's own figure so an authored sector has
/// real room to be hostile, and well over zero so it still refuses one that
/// strands the player. A sector under this floor is skipped at load with a
/// warning, like any other malformed file.
pub const MIN_SECTOR_WALKABLE_FRACTION: f64 = 0.45;

// ─────────────────────────────────────────────────────────────────────────
// Combat resolution: to-hit, crit, fumble, mitigation
// ─────────────────────────────────────────────────────────────────────────

/// Accuracy and Evasion are **derived, never stored** — see
/// `battle::accuracy_of`/`evasion_of`. Both come off `SpeciesDef::base_speed`
/// (range 6..=14 across the shipped roster) plus entity level plus gear, so a
/// fast program both hits and dodges well. `atk` is deliberately absent from
/// both: feeding it to-hit *and* damage compounds quadratically and is the
/// most likely thing to break `balance_sim`'s curves.
pub const ACCURACY_PER_SPEED: f64 = 1.0;
/// See `ACCURACY_PER_SPEED`. Levelling buys accuracy; it never buys mitigation.
pub const ACCURACY_PER_LEVEL: f64 = 0.5;
/// See `ACCURACY_PER_SPEED`.
pub const EVASION_PER_SPEED: f64 = 1.0;
/// See `ACCURACY_PER_LEVEL`.
pub const EVASION_PER_LEVEL: f64 = 0.5;

/// What the attacker's Accuracy is multiplied by inside
/// `battle::hit_chance`, before the ratio against the defender's Evasion.
///
/// **A multiplier and not an addend, because the ratio form is scale-free
/// and must stay that way.** `k*acc / (k*acc + eva)` survives doubling both
/// sides exactly as `acc / (acc + eva)` does, so a zone that scales
/// everything by its tier multiplier still changes no hit rate. A flat
/// `+n` on accuracy would wash out as levels grow, which is the same
/// scale-dependence the difference form was rejected for.
///
/// 1.4 puts an even matchup at 0.583 rather than 0.5. The old 0.5 baseline
/// was the whole of "routines miss too often": measured against the shipped
/// roster, a player with no accuracy gear sat at 0.44-0.64 for the first ten
/// levels and both apex species — the lair guardians — are the fastest
/// things in the game and so the hardest to hit at every level. A basic
/// attack shrugs a miss off; a routine has already spent its Power and armed
/// its cooldown by the time the roll happens, which is why the same rate
/// reads as a routine problem.
///
/// **Necessarily symmetric.** `hit_chance` is a pure function of two
/// numbers and cannot know which side is the player, so hostiles take the
/// same edge — notably lifting them off `HIT_CHANCE_MIN`, which a
/// high-level player had pinned them to. The player's *asymmetric* edge is
/// the flat accuracy sources instead, summed by `Game::accuracy_bonus`.
pub const ATTACKER_ACCURACY_ADVANTAGE: f64 = 1.4;

/// Bounds on `battle::hit_chance`. The floor is what keeps
/// `balance_sim`'s `TURN_CAP` meaningful as stalemate detection rather than
/// as a fight-length cap: expected damage stays strictly positive, so a
/// timeout is a genuine stalemate.
pub const HIT_CHANCE_MIN: f64 = 0.25;
/// See `HIT_CHANCE_MIN`. Below 1.0 so no matchup is a guaranteed landing.
pub const HIT_CHANCE_MAX: f64 = 0.95;

/// Flat crit rate, symmetric between the player and hostiles. Clamped to at
/// most the hit chance inside `battle::resolve_attack`, so a crit is always a
/// hit. Gear crit is deferred — a `crit` field on `EquipmentStats` that
/// nothing authors is an unused feature flag.
pub const CRIT_CHANCE: f64 = 0.08;
/// What a crit multiplies. The **rolled portion only** — doubling the total
/// would scale crits with levelling and with every `atk` source in the game.
pub const CRIT_ROLL_MULTIPLIER: i32 = 2;

/// Flat fumble rate, symmetric between the player and hostiles, on its own
/// constant so it can be split per side later without touching resolution.
/// Clamped to at most `1 - hit_chance`, so a fumble is always a miss.
pub const FUMBLE_CHANCE: f64 = 0.05;

/// Where the four fumble rungs divide, against `d` — how deep into the
/// fumble band the roll fell, in `[0, 1)`. Weighted so the deep rungs are
/// rare: Exposed below the first, Recoil below the second, Opening below the
/// third, Crash above it. Rungs **replace** rather than stack; a cumulative
/// top rung is a run-ender.
pub const FUMBLE_RUNG_THRESHOLDS: [f64; 3] = [0.55, 0.85, 0.97];
/// Fraction of a fresh roll of the fumbler's own damage range that the
/// Recoil rung deals to the fumbler.
pub const FUMBLE_RECOIL_FRACTION: f32 = 0.5;
/// Percentage points of evasion the Exposed rung strips from the fumbler
/// until their next turn.
pub const EXPOSED_EVASION_PERCENT: i32 = 50;

/// How long the Exposed rung lasts. One round, because
/// `ActiveStatus::landed_this_round` already exempts the round a condition
/// lands in — so a duration of 1 is exactly "until the fumbler's next turn",
/// which is the rung's wording.
pub const EXPOSED_DURATION_ROUNDS: u32 = 1;
/// How long the Crash rung's stun lasts. One round, for the same reason
/// `EXPOSED_DURATION_ROUNDS` is: it costs the fumbler their next action and
/// nothing beyond it.
pub const CRASH_DURATION_ROUNDS: u32 = 1;

/// Ceiling on total mitigation, strictly below 100. Load-bearing twice: it
/// stops the damage path reaching immunity, and it is what keeps
/// `Stats::power`'s effective-HP denominator away from zero.
pub const MAX_MITIGATION_PERCENT: i32 = 75;

/// The player's damage range with no weapon equipped. Replaces
/// the flat `PLAYER_STRIKE_POWER` that used to be the player's one basic
/// strike; a weapon **overrides** this rather than adding to it.
pub const PLAYER_UNARMED_DAMAGE: crate::battle::DamageRange =
    crate::battle::DamageRange { min: 3, max: 7 };

// ---------------------------------------------------------------------------
// The gear power reference wearer
// ---------------------------------------------------------------------------
//
// One fixed wearer every gear copy in the game is rated against, so that a
// copy's power figure is **absolute** — the same number on the inventory
// list, the trader's shelf and the swap picker. `Game::copy_power` is the
// one door; nothing else may read this block.
//
// **Derived, not invented.** A reference far from where players actually
// stand makes every power figure in the game wrong in the same direction,
// which is hard to notice and easy to ship. The zone is the midpoint of the
// range `balance_sim` sweeps (1..=10), and the level is what its geared
// sweep reports as the minimum to clear that zone's toughest ordinary
// matchup — a player who is *there*, not one who has over-levelled. The
// three stat figures are then `progression::stats_after_levels` of
// `PLAYER_BASE_STATS` at that level, at `BASELINE_GROWTH_MULTIPLIER`, and
// `the_power_reference_wearer_is_a_levelled_player` asserts exactly that
// rather than trusting the arithmetic below.
//
// A retune moves the zone and the level; the three stat constants then
// follow from them and the census says so.

/// The zone the reference wearer stands in. Two jobs: it is the level a
/// candidate copy is scaled at (`Game::equip` caps gear level at the zone,
/// so gear level *is* the zone), and it is the zone the nominal hostile the
/// accuracy and evasion terms are priced against is drawn at.
pub const POWER_REFERENCE_ZONE: u32 = 5;

/// The reference wearer's character level — what `balance_sim`'s geared
/// sweep needs to clear `POWER_REFERENCE_ZONE`. Feeds the wearer's own
/// Accuracy and Evasion through `battle::accuracy_of` / `evasion_of`, which
/// is why it is a separate axis from the zone.
pub const POWER_REFERENCE_LEVEL: u32 = 34;

/// `stats_after_levels(PLAYER_BASE_STATS, POWER_REFERENCE_LEVEL - 1,
/// BASELINE_GROWTH_MULTIPLIER).max_hp` — 90 + 24 x 33.
pub const POWER_REFERENCE_MAX_HP: i32 = 882;

/// The same wearer's attack — 6 + 2 x 33.
pub const POWER_REFERENCE_ATK: i32 = 72;

/// The same wearer's mitigation. Levelling never raises it (a percentage
/// that grows per level approaches immunity), so this is
/// `PLAYER_BASE_STATS.mitigation` unchanged.
///
/// **It must stay strictly below `MAX_MITIGATION_PERCENT`**: `Stats::power`
/// divides by `1 - mitigation/100`, and keeping that denominator off zero
/// is the whole point of the cap.
pub const POWER_REFERENCE_MITIGATION: i32 = 2;

/// The band the reference wearer swings without a weapon, and so the band a
/// weapon's own is measured **against**. A weapon *overrides* the natural
/// attack rather than adding to it (`Game::attack_range`), so a weapon
/// whose band is worse than this one is worth negative offense — which is
/// the whole reason the term is a difference and not a sum.
///
/// `PLAYER_UNARMED_DAMAGE` unscaled, because that is what
/// `Game::natural_range_of` hands the player at every level.
pub const POWER_REFERENCE_DAMAGE: crate::battle::DamageRange = PLAYER_UNARMED_DAMAGE;

// ---------------------------------------------------------------------------
// Entity memories
// ---------------------------------------------------------------------------

/// The global stickiness dial: every def's authored `half_life` is multiplied
/// by this before the decay is taken. One number makes every grudge and every
/// fondness in the game longer or shorter.
///
/// Neutral at 1.0 so an authored `half_life` means the ticks it says. Per-def
/// half-lives stay in the `.ron`, because a scar outlasting a bad shift is a
/// content decision about those two memories; how sticky memory is *in
/// general* is a difficulty decision, and that split is the standing rule that
/// content is data and how hard the game is, is not.
pub const MEMORY_HALF_LIFE_MULTIPLIER: f32 = 1.0;

/// Most memories one program may hold at once; past it, `Game::remember`
/// evicts the weakest by magnitude.
///
/// **A layout constraint before it is a feel one.** `draw_popup` pages a
/// `Row::Item` span and a page with none drops any row past the bottom in
/// silence. A `PopupSize::Large` popup holds 23 rows at the tightest window
/// the font ramp allows — at 600px tall the font clamps at `MIN_UI_FONT` 16,
/// giving a `line_height` of 20 and an inset of 6.67 against a body of
/// `600 * 0.85` — and the memory page spends the rest of that on a title, a
/// Morale header and their spacing. Raising this past what fits means giving
/// that page a scroll first, not editing this number —
/// `the_tallest_memory_page_fits_its_popup` is what says so, and
/// `no_memory_row_overflows_its_popup` holds the other axis, which nothing
/// clamps at all.
pub const MEMORY_CAP_PER_PROGRAM: usize = 12;

/// Intensity **magnitude** below which an entry is dropped at the next
/// formation. Eviction is lazy — nothing sweeps — so this is the point at
/// which a memory stops being worth a row and a comparison rather than the
/// point at which it stops mattering.
///
/// At the shipped half-lives this is a little under four half-lives from a
/// single strike.
pub const MEMORY_FORGET_THRESHOLD: f32 = 0.5;

/// The share of a program's **maximum** HP a single landed hit has to take
/// before the program remembers what swung it.
///
/// Read against `max_hp` rather than against the HP it had left, so what
/// counts as a mauling is a property of the blow rather than of how worn down
/// the body already was — a scratch that finishes a program on one point is
/// not the thing `mauled_by` is about.
///
/// The figure compared is what actually *landed*, after mitigation: armour
/// that absorbed the blow is armour that stopped the scar.
pub const MEMORY_MAUL_FRACTION: f32 = 0.35;

/// Opinion of a base tile at or below which an idle program refuses to
/// drift onto it — `drift_idle_staff`'s last rejection, beside a tile a
/// `Structure` stands on, a tile that is not laid floor, the party's own
/// cell, and a tile another idle body holds.
///
/// **Signed, not a magnitude.** The comparison is `opinion_of(..) <` this,
/// so a fondness never triggers it and a grudge has to be a real one. It is
/// deliberately *not* pinned to `stranded_at`'s valence: the question the
/// hook asks is whether a program holds anything against that corner, not
/// whether one particular memory is in the store, and a second negative
/// `BaseTile` def must reach it without editing this line.
///
/// At the shipped `stranded_at` — valence -6.0, half-life 3000 — a single
/// stranding keeps a program off that tile for exactly one half-life, and a
/// second one inside that window roughly doubles the reach. A rejected
/// candidate costs nothing: the program holds its ground for that beat and
/// the ring offers it a different tile on the next step.
pub const MEMORY_AVOIDANCE_THRESHOLD: f32 = -3.0;

/// How far one point of `Game::morale` shifts a worker's extraction
/// reliability, in `systems::mining_success_chance`.
///
/// **Signed around a baseline of zero**, exactly like `base_int`'s term in
/// the same formula and `base_speed`'s in `work_ticks_at_speed`. A program
/// with no memories, the player working a node themselves, and an
/// `assets/memories/` that has been deleted all contribute precisely
/// nothing, so the shipped extraction rates mean what they have always
/// meant. Making it absolute would silently re-rate every node by wiring
/// alone.
///
/// Symmetric: a program that remembers good things extracts more reliably
/// and one that remembers bad things fizzles more. A catalogue where only
/// grudges bite makes every memory a liability and leaves the positive
/// kinds with nothing to do here.
pub const MEMORY_MORALE_PER_POINT: f64 = 0.005;

/// The most morale may shift extraction reliability in either direction,
/// as a fraction of the roll.
///
/// **A cap on the contribution, not on the result.**
/// `mining_success_chance` already clamps what it returns to `0.0..=1.0`,
/// because `GameRng::random_bool` panics outside it — that is a different
/// job. `Game::morale` is a signed sum of up to `MEMORY_CAP_PER_PROGRAM`
/// entries and is unbounded at both ends, so without this a bad run drives
/// a node's reliability to zero and the base stops producing, which reads
/// as the base being broken rather than as a program being unhappy.
///
/// This is what keeps morale a texture rather than a difficulty knob keyed
/// to run history. Like `MAX_MITIGATION_PERCENT`, it is **never** scaled by
/// level, zone or depth: a term that grows with the player approaches the
/// cap and stops meaning anything.
pub const MEMORY_MORALE_MAX_SHIFT: f64 = 0.10;

/// The most a program's need strain may shift extraction reliability.
///
/// **Its own cap, and the outer `clamp(0.0, 1.0)` is not it.** That clamp
/// exists because `GameRng::random_bool` panics outside `0..=1`, and it would
/// silently swallow an uncapped overshoot at the low end — so a test reading
/// the finished chance could not tell a working cap from a missing one. Read
/// `MEMORY_MORALE_MAX_SHIFT`'s argument, which is this one exactly.
///
/// One-directional in practice, because every shipped `morale_weight` is
/// negative: a program that has what it needs contributes zero rather than a
/// bonus, which is what keeps the shipped extraction rates meaning what they
/// have always meant.
pub const NEED_STRAIN_MAX_SHIFT: f64 = 0.15;

/// How much one point of need strain is worth to an extraction roll.
pub const NEED_STRAIN_PER_POINT: f64 = 0.01;

/// How often `Game::note_postings` writes what a posted program is doing —
/// every this many ticks, read straight off `GameClock`.
///
/// **The period is what stands in for an edge.** `Game::note_strandings` is
/// edge-triggered off `Stranded::since` and its doc comment says why a
/// per-tick write would be wrong: it "would saturate `strike_cap` in three
/// ticks and hold the grudge at full intensity for as long as the route
/// stayed broken, which makes `strikes` mean nothing". A posting has no such
/// edge — nothing distinguishes the first tick at a machine from the
/// thousandth — so the period is what makes `strikes` count stretches of
/// service instead of ticks.
///
/// It also keeps eviction lazy. `Game::remember` evicts at the tail of every
/// write, so a per-tick writer would make eviction effectively eager for any
/// program holding a posting and lazy for every idle one — a difference in
/// what a program remembers based on whether it happened to be working.
///
/// At the shipped work defs a body reaches `strike_cap` in well under a
/// half-life, so a full grudge or fondness is a real stretch of the run
/// rather than a moment of it.
pub const MEMORY_POSTING_PERIOD: u64 = 250;

// ---------------------------------------------------------------------------
// Sorties
// ---------------------------------------------------------------------------

/// Ticks of travel every sortie pays regardless of where it is going.
///
/// Travel deliberately dominates the trip: a fight is quick and getting
/// there is not. With the two terms below, the shipped middling site lands
/// near half a `CARAVAN_VISIT_INTERVAL_TICKS` — a reference point and not a
/// derivation; nothing computes one from the other.
pub const SORTIE_TRAVEL_BASE_TICKS: u64 = 150;

/// Extra travel ticks per step of a site's risk **offset** — never its
/// absolute danger band. Read against the absolute band, every trip in a
/// deep sector would take enormously longer for no reason the player could
/// name, and the feature would quietly stop being usable late in a run.
pub const SORTIE_TRAVEL_PER_RISK_TICKS: u64 = 75;

/// Ticks each battle adds to a trip.
///
/// There is deliberately no term for squad size, level or power anywhere in
/// the duration: a stronger squad shows up as better outcomes and never as a
/// faster cycle, or the feature becomes a throughput multiplier that scales
/// with itself. Duration is a property of the place, the way
/// `BASE_ROCK_DURABILITY` is never scaled by the player.
pub const SORTIE_TICKS_PER_BATTLE: u64 = 20;

/// How long one board of offers stands before it rotates.
///
/// **Longer than the longest trip** the shipped catalogue can quote, so a
/// board cannot rotate twice while the player is deliberating over it.
pub const SORTIE_BOARD_ROTATION_TICKS: u64 = 1200;

/// Offers on a board.
pub const SORTIE_BOARD_SLOTS: usize = 3;

/// The board's own salt. Its own constant and never a reused one, following
/// `CARAVAN_SALT`.
pub const SORTIE_SALT: u64 = 0xE7ED_1710_5EED_0003;

/// A program below this fraction of max Integrity is refused at dispatch.
///
/// Sending a hurt program on a twenty-fight trip is the mistake the
/// abort-on-first-casualty rule cannot save you from, because it fires on
/// the first battle.
pub const SORTIE_MIN_HP_FRACTION: f32 = 0.5;

/// Fraction of `max_hp` restored to each member between battles, paid for by
/// the provisioning charged at dispatch.
///
/// A **fraction** rather than flat Integrity, so provisioning keeps meaning
/// something at the level cap. This is the single dial that decides whether
/// a twenty-fight trip is survivable.
pub const SORTIE_PROVISION_HEAL_FRACTION: f32 = 0.15;

/// Units of the build currency the provisioning costs, per battle per body.
///
/// The "materials" leg of the four ways a sortie pays for itself. Priced per
/// battle *and* per body because both are what the provisions have to cover:
/// a long trip and a big squad each eat more of them. Charged from base
/// stock at dispatch through `stock::spend_from_base` — a base cost paid at
/// the Relay, not a build a body walks to.
pub const SORTIE_PROVISION_PER_BATTLE: u32 = 1;

/// What a sortie kill pays against what the same kill pays with the player
/// in the fight.
///
/// Below 1.0 deliberately: this is the one *tuned* lever on the yield, and
/// it exists so the cap can move without disturbing the two mechanisms that
/// earn it — Power not recovering in the field, and no rest out there.
pub const SORTIE_XP_MULTIPLIER: f32 = 0.6;

/// Rounds one off-screen battle may run before it is called a draw.
///
/// A backstop and not a mechanic: two sides that cannot finish each other
/// would otherwise loop forever inside a single tick, which is a hang rather
/// than a long fight. Generous enough that no shipped matchup reaches it.
pub const SORTIE_MAX_BATTLE_ROUNDS: u32 = 60;

// ---------------------------------------------------------------------------
// Caravan traders
// ---------------------------------------------------------------------------

/// How often a caravan is due — one visit per interval, exactly.
///
/// The rhythm the whole feature is felt as. Too short and a trader is
/// furniture the player stops walking over to; too long and a run never sees
/// one. Nothing instruments this: `balance_sim` models no base and no trade,
/// so this and `CARAVAN_MARKUP` are the two figures in the feature that only
/// a session can judge.
pub const CARAVAN_VISIT_INTERVAL_TICKS: u64 = 900;

/// How far into its interval a visit's arrival may slide.
///
/// Strictly below `CARAVAN_VISIT_INTERVAL_TICKS - CARAVAN_STAY_TICKS`, or two
/// consecutive visits could overlap and the "exactly one visit per interval"
/// property would stop being true. Its whole job is that a player cannot
/// count ticks to the next one.
pub const CARAVAN_ARRIVAL_JITTER_TICKS: u64 = 300;

/// How long a caravan stands beside the Market before it packs up.
///
/// Long enough to outlast a field trip: a player who sees the arrival line,
/// finishes what they were doing and walks home has to still find it there,
/// or the feature reads as a taunt. Counted from arrival, not from the
/// moment the caravan docks — a long walk in is the caravan's problem.
pub const CARAVAN_STAY_TICKS: u64 = 400;

/// How far from the anchor a caravan appears when its visit opens.
///
/// Chebyshev, on the derived bearing. Far enough that it is visibly walking
/// in rather than materialising on the doorstep, inside the walk radius its
/// `pursuit::walk_field` searches so the route can actually be found.
pub const CARAVAN_SPAWN_DISTANCE_TILES: i32 = 10;

/// What a caravan charges over an item's `ItemDef::value`.
///
/// Above 1.0 because convenience is the whole product: the same goods are
/// compilable at a bench or findable in the Stack, and a trader that
/// undercut either would make both pointless. It is *not* a second
/// difficulty axis — a caravan sells nothing that cannot be got another way,
/// which is why `Reward::PortalFragments`' exclusion is the one hard floor
/// under it.
pub const CARAVAN_MARKUP: f32 = 1.6;

/// Salts the caravan schedule apart from everything else derived off the
/// base's seed.
///
/// Its own named constant, per `FrameSpec::salted`'s rule — one salting
/// scheme, not a second seed source that could collide with the Stack's or
/// the Broker board's.
/// The most of one material a caravan carries in a single row.
///
/// A row's actual stack is drawn from `1..=` this off the shelf's own seed,
/// so a shelf reads as a wagon someone loaded rather than as a menu. Gear and
/// programs are never stacked — a copy is a copy.
pub const CARAVAN_MATERIAL_STACK: u32 = 12;

pub const CARAVAN_SALT: u64 = 0xCA57_A0A0_5EED_0002;

/// Chance a caravan's standout gear row lands above `Rarity::Ordinary`.
///
/// Spent by *narrowing the range the rarity roll is drawn from* rather than
/// by authoring a second rarity table, so the rungs keep the proportions
/// `rarity_spawn_chance` gives them and a new tier added there reaches a
/// wagon for free. A second table is the copy that drifts.
///
/// Deliberately short of 1.0: a standout row is a good find, not a
/// guaranteed one, and a shelf where every marked row is Silver-or-better
/// stops the rarity colour meaning anything.
pub const CARAVAN_BONUS_RARITY_CHANCE: f64 = 0.6;

/// The quality floor a caravan's standout gear row rolls off, against
/// `QUALITY_DROP_BASE` for every other row.
///
/// `QUALITY_DEFAULT` is the authored item's own figure, so this is the line
/// between "worse than spec" and "better than spec" — a plain drop rolls
/// `QUALITY_DROP_BASE..+QUALITY_SPREAD` and so is *always* below the number
/// the item was authored at, which is the whole reason a wagon of drop-rate
/// gear reads as a rack of junk.
pub const CARAVAN_BONUS_QUALITY_FLOOR: u8 = QUALITY_DEFAULT;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ZoneLevel;

    /// The reference wearer must stay a *derivation* of the two axes above
    /// it, not three hand-written numbers that drift the first time
    /// `HP_PER_LEVEL` or `PLAYER_BASE_STATS` moves. Its whole value is that
    /// it sits where players actually stand.
    #[test]
    fn the_power_reference_wearer_is_a_levelled_player() {
        let levelled = crate::progression::stats_after_levels(
            PLAYER_BASE_STATS,
            POWER_REFERENCE_LEVEL - 1,
            BASELINE_GROWTH_MULTIPLIER,
        );
        assert_eq!(levelled.max_hp, POWER_REFERENCE_MAX_HP);
        assert_eq!(levelled.atk, POWER_REFERENCE_ATK);
        assert_eq!(levelled.mitigation, POWER_REFERENCE_MITIGATION);
        // `Stats::power` divides by `1 - mitigation/100`.
        const { assert!(POWER_REFERENCE_MITIGATION < MAX_MITIGATION_PERCENT) };
    }

    /// Seeding and maintenance must agree about how crowded a zone should
    /// be. The derivation is what makes them agree by construction, so the
    /// property to pin is the round trip: scaling the seeded area back down
    /// to one spawn box has to land on the target the ambient roll enforces.
    ///
    /// Integer division loses a fraction of a creature per box, so this
    /// allows exactly that and no more — a derivation that drifted by a
    /// whole creature per box would be a real disagreement.
    #[test]
    fn a_chunk_is_stocked_at_the_density_it_is_maintained_at() {
        let chunk = crate::world::CHUNK_SIZE as f64;
        let spawn_box = (2 * WILD_SPAWN_RADIUS_TILES + 1) as f64;
        let boxes_covered = (chunk * chunk) / (spawn_box * spawn_box);
        let per_box = chunk_wild_population() as f64 / boxes_covered;

        assert!(
            (per_box - WILD_LOCAL_DENSITY_TARGET as f64).abs() < 1.0,
            "a chunk is stocked at {per_box:.2} per spawn box but the ambient \
             roll maintains {WILD_LOCAL_DENSITY_TARGET}"
        );
    }

    /// The zone curve was a bare `1 << (zone - 1)` before it was named, and
    /// geometric until it was measured. Pinning the sequence keeps a retune
    /// honest about what it costs, and pinning it as *linear* is the point:
    /// the player's side of the fight grows by a constant per level, so a
    /// geometric enemy curve is a race the player loses at some finite zone
    /// no matter how the coefficients are set. `balance_sim`'s level sweeps
    /// are projected against this.
    #[test]
    fn zone_stat_multiplier_rises_linearly_and_never_compounds() {
        let curve: Vec<i32> = (1..=8).map(|z| ZoneLevel(z).stat_multiplier()).collect();
        assert_eq!(curve, vec![1, 2, 3, 4, 5, 6, 7, 8]);

        // The property, stated as a property rather than as a table: every
        // step is the same size. A geometric curve passes the table above
        // for its first two entries and fails here at the third.
        let steps: Vec<i32> = curve.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            steps.iter().all(|&s| s == ZONE_STAT_STEP),
            "the zone curve compounds somewhere: {steps:?}"
        );
    }

    /// `Game::max_group_size` clamps its distance exponent to
    /// `MAX_GROUP_SIZE_STEPS` because the map is unbounded. That
    /// clamp is only lossless while the clamped growth already exceeds
    /// `MAX_GROUP_SIZE` — raise the cap without raising the step count and
    /// distance would silently stop mattering short of it.
    #[test]
    fn clamping_the_distance_exponent_cannot_cost_group_size() {
        assert!(
            GROUP_SIZE_DISTANCE_GROWTH.pow(MAX_GROUP_SIZE_STEPS) > MAX_GROUP_SIZE,
            "distance growth clamped at {MAX_GROUP_SIZE_STEPS} steps reaches only {}, \
             which no longer covers MAX_GROUP_SIZE ({MAX_GROUP_SIZE})",
            GROUP_SIZE_DISTANCE_GROWTH.pow(MAX_GROUP_SIZE_STEPS),
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

// ---------------------------------------------------------------------------
// Dispositions
// ---------------------------------------------------------------------------
// A program's hidden temperament — see `crate::disposition::Disposition`.
// Two constants and not a dozen, because the five dispositions are symmetric
// pole pairs on two axes: whatever one pole adds, its opposite subtracts.
// Giving a single pole its own number is what `the_poles_are_symmetric_about
// _neutral` fails on.

/// How far `Languid` and `Dogged` move `NeedDef::drain_per_tick`, as a
/// fraction either side of neutral.
///
/// **Must stay below 1.0**: at 1.0 a `Dogged` program's reserves stop
/// draining entirely and it never leaves a post for an amenity again, which
/// is not a personality but a broken need. Held by
/// `every_disposition_still_drains`.
pub const DISPOSITION_DRAIN_SWING: f32 = 0.30;

/// How far `Amiable` and `Abrasive` scale a memory's intensity, as a fraction
/// either side of neutral — amplifying one pole and damping the other.
///
/// **Must stay below 1.0**, and for a sharper reason than the drain swing: at
/// or above 1.0 the damped pole reaches zero or crosses it, and a memory that
/// changed sign would read as a program cheering up because it was hurt.
/// `Memory::intensity` states that rule for the decay curve; this is the same
/// rule one level up. Held by `felt_never_flips_a_memorys_sign`.
pub const DISPOSITION_MEMORY_SWING: f32 = 0.40;

// ---------------------------------------------------------------------------
// Acting out
// ---------------------------------------------------------------------------
// When a program's morale has run far enough below zero that it stops
// working — see `components::Disgruntled`. Two thresholds and not one,
// `NeedDef`'s `critical`/`content` pair: the gap between them is what stops a
// body downing tools and picking them up again every tick at the boundary.
//
// **Both figures are unmeasured.** Morale is a signed sum of decayed memory
// intensities with no natural scale, and nothing in `balance_sim` models base
// production, so these were chosen against the shipped valences (which run
// -8..+5 at strike caps of 3 and 4) and not against a run. They are the first
// thing to revisit after a base has been watched.

/// Morale at or below which a program will still work, but not at a machine
/// it holds a grudge against.
///
/// The mild rung. **Must sit above `MORALE_DOWNS_TOOLS_AT`** or the ladder
/// has no first step and a program goes straight from content to useless —
/// held by `the_ladder_climbs_in_order`.
pub const MORALE_SULKS_AT: f32 = -8.0;

/// Morale at or below which a program stops taking postings altogether.
///
/// **Below what any single memory can reach**, which is the whole of what
/// this number is for: downing tools takes a *pattern* of things going
/// wrong, never one of them. Held by
/// `no_single_memory_can_down_a_programs_tools` against the real
/// `assets/memories/`, with `two_bad_memories_can_still_down_a_programs_tools`
/// as the control — a rung nothing can reach is a deleted feature.
///
/// This was -18, "roughly two maxed grudges' worth", and that reading
/// counted a memory's `valence` alone. A grudge is not its valence: it is
/// valence x `strike_cap` x `DISPOSITION_MEMORY_SWING`, so the worst single
/// memory the game ships (`mauled_by`, -8 at a cap of 4, felt by an
/// `Abrasive` program) reaches **-44.8** and cleared the old line more than
/// twice over on its own. `frayed_here` did it too, on a base that simply
/// had nothing servicing a need — a standdown the player could neither
/// prevent nor answer, lasting longer than the run that earned it.
///
/// The mild rung is deliberately still reachable from one memory: a single
/// bad experience makes a program sulk, and that is what the ladder's first
/// step is for.
pub const MORALE_DOWNS_TOOLS_AT: f32 = -50.0;

/// Morale at which a disgruntled program goes back to work.
///
/// **Must stay strictly above `MORALE_SULKS_AT`** — equal, the
/// hysteresis gap closes and the marker flickers every tick, which is the
/// whole reason there are two numbers. Held by
/// `the_recovery_threshold_leaves_a_hysteresis_gap`. Still below zero: a
/// program does not have to be *happy* to work again, only no longer in the
/// hole.
pub const MORALE_RECOVERED_AT: f32 = -6.0;
