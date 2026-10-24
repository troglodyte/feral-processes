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
/// program used to drop directly.
///
/// **Zero readers now** — program extraction (2026-09-04) retired the
/// direct grant this priced (`Game::leave_downed_program` replaced it; see
/// `docs/superpowers/specs/2026-09-04-program-extraction-design.md` section
/// 5). Kept only as the expected-value target decision 8's drop-neutrality
/// fits the starter tool's yield against — Task 6's to spend, not to
/// delete for reading as unused.
///
/// Doubled from `1..=2` on 2026-08-02, when deleting the scan action left
/// kills as the only source of Core Fragments outside a built base. At 1..=2
/// the first Extractor — 12 fragments, on top of Home's 5 — was about eight
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

/// What one point of the Analysis attribute **either side of**
/// the Analysis attribute's catalogue base is worth on the mining roll. The shipped *non-boss*
/// roster spans 5 to 15, so the fourth term ranges about -0.10 to +0.10 —
/// enough that a Cipher and a Construct posted to the same Mk1 node visibly
/// disagree (0.58 against 0.40), and small enough that a node's own tier,
/// worth `MINING_SUCCESS_PER_LEVEL` a step, still outruns species choice
/// over a few upgrades. The two bosses run higher still (Overseer 16,
/// Wintermute 18), but neither can ever be tamed or posted to a job, so
/// their Analysis never reaches this roll.
pub const MINING_SUCCESS_PER_INT: f64 = 0.02;

/// What one point of `SpeciesDef::base_speed` **either side of**
/// `DEFAULT_BASE_SPEED` is worth on the length of a work cycle. The shipped
/// roster spans 6 (Construct) to 14 (Sprite), so a cycle ranges 1.2x to
/// 0.8x the machine's own rate — an Extractor's 10 ticks becomes 12 or 8,
/// and a Fabricator's 30 becomes 36 or 24.
///
/// Sized like `MINING_SUCCESS_PER_INT`: enough that swapping the posted
/// program is visible on one screen, small enough that upgrading the
/// machine still beats re-running the roster.
pub const WORK_TICKS_PER_SPEED: f64 = 0.05;

/// What one rung of `components::Rarity` adds to the build quality a program
/// leaves in the machine it was spent on — see
/// `game::base::building::build_quality`.
///
/// A fifth of `GRADE_PER_RARITY_RUNG` on purpose. Rarity is an axis the
/// player does not choose: a Prismatic program is what the spawner handed
/// them, so this insures a build rather than deciding one. A Prismatic with
/// a Poor roll still leaves a machine slower than shipped, which is the
/// property `rarity_lifts_a_build_by_exactly_one_rung_per_rung` pins.
pub const BUILD_QUALITY_PER_RARITY_RUNG: f32 = 0.03;

/// How hard a build quality either side of neutral pulls on a work cycle,
/// as a fraction of the deviation — see `systems::work_ticks_at_speed`.
///
/// Deliberately half of what a point of `WORK_TICKS_PER_SPEED` is worth over
/// the same range, because the two **multiply**: the builder's figure and
/// the posted worker's speed both scale one cycle. At the roll range's edges
/// (`MIN_INDIVIDUAL_ROLL`..`MAX_INDIVIDUAL_ROLL`, 0.8 to 1.2) a build lands
/// a cycle at 1.10x to 0.90x its shipped rate — visible on one screen next
/// to the machine's own tier, and small enough that upgrading still beats
/// re-rolling who built it.
pub const BUILD_QUALITY_TICK_WEIGHT: f64 = 0.5;

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
// do not: with `BASE_PET_CAPACITY` at 3 slots, every program at a machine is one
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
/// goods on the board as bulk errands; a Subroutine part is 20.
pub const CONTRACT_MAX_DELIVER_VALUE: u32 = 8;

/// How many points around the base's doorstep a rolled contract samples to
/// decide which programs live in this sector.
///
/// A sample rather than a walk because `Game::contract_board` is on a
/// per-frame path — the contracts screen and the command pages' row tests both
/// call it — while the ring it walks grows with a base that can reach
/// `MAX_BUILD_RADIUS_TILES`. Biome features run about 25 tiles across at
/// `WorldMap::classify`'s noise scale, so 32 points stay inside one feature
/// apiece out to the widest base the game allows.
pub const CONTRACT_HABITAT_SAMPLES: i32 = 32;
