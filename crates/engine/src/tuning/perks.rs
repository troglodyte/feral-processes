// ─────────────────────────────────────────────────────────────────────────
// Implants
// ─────────────────────────────────────────────────────────────────────────

/// The Neural Load a level-1 player can carry before an implant overloads
/// them — `implants::load_cap`'s constant term. Two cheap implants fit from
/// the start; nothing heavier does.
pub const IMPLANT_LOAD_BASE: u32 = 4;

/// Levels per extra point of Neural Load cap.
///
/// Chosen so the cap reaches 16 — the Load of the whole first set, six
/// implants at once — in the middle of the level band `balance_sim` models.
/// That band is zones 1 to 10 (`MAX_GEARED_ZONE_SWEPT`), whose level caps run
/// 6, 12, 23 ... 100 (`zone_level_cap`), so its middle is zone 5-6 at levels
/// 45-56. `IMPLANT_LOAD_BASE + 48 / 4 = 16` lands inside it: before then
/// a player chooses which implants to run, after it they can run them all.
pub const IMPLANT_LOAD_LEVELS_PER_POINT: u32 = 4;

/// Extra Power upkeep per point of installed Load, as a fraction of the base
/// drain: `implants::drain_factor` is `1 + load * this`. The full first set
/// (16 Load) therefore drains 1.8 times as fast as a bare player.
pub const IMPLANT_DRAIN_PER_LOAD: f32 = 0.05;

/// Chance, per point of overload, that a battle start arms a rejection
/// status on the player.
pub const REJECTION_CHANCE_PER_LOAD: f64 = 0.10;

/// Ceiling on that chance, so heavy overload is a bad bet and never a
/// certainty.
pub const REJECTION_CHANCE_MAX: f64 = 0.5;

/// The statuses a rejection picks from, uniformly. Ids in
/// `assets/statuses/`; one missing from the loaded set is skipped.
pub const REJECTION_STATUSES: [&str; 3] = ["throttled", "exposed", "stun"];

/// Rounds a battle-start status (a rejection, or an implant's own
/// `BattleStartStatus`) lasts. Two, so even a stun is a bad opening and not a
/// lost fight.
pub const REJECTION_STATUS_DURATION: u32 = 2;

/// `core_fragment`s charged per point of Load to take an implant back out.
/// A cheap 2-Load implant costs 12
/// to remove, about what two kinetic edges cost to build.
pub const IMPLANT_REMOVAL_FRAGMENTS_PER_LOAD: u32 = 6;

/// Power the Dead Man's Switch spends each time it saves the player.
pub const DEAD_MANS_SWITCH_POWER: f32 = 30.0;

// ─────────────────────────────────────────────────────────────────────────
// Phase Keys
// ─────────────────────────────────────────────────────────────────────────

/// Phase Key slots, one per zone 1..=`PHASE_KEY_COUNT`. Fixed in code so a
/// mod can change what a key is but never whether the story can be finished.
pub const PHASE_KEY_COUNT: u32 = 10;

/// Chance an eligible Stack guardian kill drops its zone's Phase Key.
pub const PHASE_KEY_DROP_CHANCE: f64 = 0.33;

/// The eligible guardian kill (counting misses) on which the key drops for
/// certain, so bad luck costs a few kills and never the run.
pub const PHASE_KEY_GUARANTEE_KILLS: u32 = 3;

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

/// `Perk::LowPowerMode`'s cut to the base Power drain, per level (the
/// multiplier is `1.0 - this * level`, floored at 0.0), so rank 4 stops the
/// base drain. Implant upkeep is outside it — see `implants::power_multiplier`.
pub const LOW_POWER_MODE_REDUCTION_PER_LEVEL: f32 = 0.25;

/// Flat Power a tick each `Perk::LowPowerMode` level past zero drain adds.
/// One level cancels exactly 4 Load of implant upkeep
/// (`HUNGER_DECAY_PER_TICK * IMPLANT_DRAIN_PER_LOAD * 4`), so the levels pay
/// for implants before they refill anything.
pub const LOW_POWER_MODE_TRICKLE_PER_LEVEL: f32 = 0.03;

/// How much `Perk::ExploitFocus` shaves off `CAPTURE_HP_PENALTY` per level,
/// floored at a penalty of 0.
///
/// Deliberately *not* effective Decompiler skill, which is what this perk
/// used to grant. That stat already grew 2 per player level for free (now it
/// comes from Analysis, at 1 a point), so the perk was buying one level's worth of
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

/// What `Perk::Teardown` adds per level. Used to sit on top of the kill's
/// own `WORK_RESOURCE_DROP` roll (2..=4); that roll is gone
/// (`Game::leave_downed_program` replaced the direct grant it priced), and
/// `Game::extraction_yield` is now where `perks::salvage_bonus` reads this —
/// see the `Perk::Teardown` variant's own doc in `perks.rs`. Added to the
/// unit count as a flat addend, never a second `GameRng` draw, the same
/// discipline the retired roll followed.
///
/// This is a permanent income *rate*, not a loop — nothing here mints value
/// out of nothing, and it is bounded by how many bodies the player strips,
/// which is what makes it the salvage perk rather than a trader one. It is
/// still one of the two steepest things in this section relative to what it
/// modifies (+1 on `TOOL_BASE_UNITS`' 3, so a third again on a fresh common
/// body, decaying as tool tier, bench tier and grade raise the base), so it
/// carries the highest price in the catalogue.
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

/// Slots the player and every companion have at level 1. Two, and
/// `decompile` occupies one of the player's — a new game pre-installs that
/// ability, so the player starts with one free slot.
pub const ROUTINE_SLOT_BASE: u32 = 2;

/// Levels per grant of `ROUTINE_SLOTS_PER_STEP` slots, the player and
/// companions alike. Was 5 for the player and 1 for a companion; one curve
/// between the two means a program's kit and the player's grow at the same
/// pace, and what sets them apart is class, rarity and quality instead.
pub const ROUTINE_SLOT_PER_LEVEL: u32 = 3;

/// Most routines the level curve grants, reached at level 15. The player
/// has no level ceiling (`progression::add_xp` takes `None`), so this clamp
/// is what bounds the curve. Class, rarity, quality, talents and implants
/// are added past it, so it bounds the curve and not the total.
pub const ROUTINE_SLOT_CAP: u32 = 12;

/// Slots a body of each combat class adds to the level curve — a species'
/// derived class for a companion, the chosen class for the player, through
/// `classes::affinity_routine_slot_bonus`. The kit-driven roles carry more;
/// a Bastion, whose job is standing in the way, carries one fewer.
pub const STRIKER_ROUTINE_SLOT_BONUS: i32 = 0;
pub const BASTION_ROUTINE_SLOT_BONUS: i32 = -1;
pub const MEDIC_ROUTINE_SLOT_BONUS: i32 = 1;
pub const SABOTEUR_ROUTINE_SLOT_BONUS: i32 = 2;
pub const LEECH_ROUTINE_SLOT_BONUS: i32 = 1;

/// Slots a companion gains per `Rarity` tier above Ordinary, so a
/// Prismatic program carries four more than an ordinary one.
pub const RARITY_ROUTINE_SLOTS_PER_TIER: u32 = 1;

/// Slots a companion gains at each rung of `Potential::quality_rung`,
/// Poor first — Above Average one, Excellent two, nothing below.
pub const QUALITY_ROUTINE_SLOT_BONUS: [u32; 5] = [0, 0, 0, 1, 2];

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
