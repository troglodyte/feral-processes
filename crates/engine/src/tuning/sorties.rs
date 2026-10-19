use super::*;

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

/// How long a parent rests after a breeding. A reference to the sortie
/// board's rotation rather than its own number: both are "about one trip".
pub const BREEDING_COOLDOWN_TICKS: u64 = SORTIE_BOARD_ROTATION_TICKS;
/// How long a child incubates, for the same reason.
pub const INCUBATION_TICKS: u64 = SORTIE_BOARD_ROTATION_TICKS;

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

/// Below 1.0 is the whole point of the lever, so a retune that took it to
/// 1.0 or above must fail the build rather than one test in a suite somebody
/// could mark ignored — `CREATION_STAT_POINTS`' precedent. What a sortie
/// kill actually pays over the real assets is still
/// `a_sortie_kill_pays_less_than_fighting_it_yourself`, which this only
/// takes the constant half off.
const _: () = assert!(SORTIE_XP_MULTIPLIER < 1.0);

/// Rounds one off-screen battle may run before it is called a draw.
///
/// A backstop and not a mechanic: two sides that cannot finish each other
/// would otherwise loop forever inside a single tick, which is a hang rather
/// than a long fight. Generous enough that no shipped matchup reaches it.
pub const SORTIE_MAX_BATTLE_ROUNDS: u32 = 60;

// ---------------------------------------------------------------------------
// Caravan routes
// ---------------------------------------------------------------------------

/// Ticks of travel every route pays regardless of destination —
/// `SORTIE_TRAVEL_BASE_TICKS`'s shape, its own constant rather than a shared
/// one because a route's clock counts tiles to one town and a sortie's
/// counts a step of risk; the two have no reason to move together.
///
/// Unmeasured: no arena or balance instrument prices trade throughput, so
/// this is a starting value to be corrected once the feature is played.
pub const ROUTE_TICKS_BASE: u64 = 100;

/// Extra ticks per tile of Chebyshev distance from the base anchor to the
/// destination — `sortie_duration`'s rule that a trip's length is a property
/// of the place, quoted once and run by the countdown. Unmeasured.
pub const ROUTE_TICKS_PER_TILE: u64 = 2;

/// How close a known settlement has to stand to the base→destination segment
/// to be able to prey on the trip — point-to-**segment** distance, in tiles,
/// so a town beside the middle of a long route is caught exactly as one at
/// either end.
///
/// **A quarter of a region**, `SETTLEMENT_GARRISON_RADIUS`'s reason: a flat
/// number is measured against nothing, and at the 15 this shipped with no
/// sampled world in 2,000 had a town near any lane. A quarter-region is the
/// same reach `SETTLEMENT_NOTICE_RADIUS` gives a town over its own ground,
/// which is the right size for "a lane this town can see" — they are kept
/// as two constants because one is about a deed and one about a corridor,
/// and nothing says they must move together.
///
/// **A route to the *nearest* town stays unpreyable at any radius, and that
/// is geometry rather than tuning.** Its corridor is short and points away
/// from everywhere else; the derivation insets each town from its region's
/// border, so the next town is a median 216 tiles off that segment. Trips to
/// a farther market are what carry risk — 8% of second-nearest lanes and
/// 18% of third-nearest have a town in the band, before `preys_on_routes`
/// narrows those to `Hostile` towns the party has actually found.
///
/// Measured — `docs/measurements/2026-09-05-settlement-aid-reach.md`.
pub const ROUTE_PREDATION_RADIUS: i32 = crate::settlements::placement::REGION_TILES / 4;

/// Chance, per predator, per leg completion, that a hit actually lands.
/// Unmeasured.
pub const ROUTE_PREDATION_CHANCE: f32 = 0.25;

/// Fraction of the cargo (outbound) or the proceeds (inbound) a successful
/// hit takes. Unmeasured.
pub const ROUTE_PREDATION_LOSS: f32 = 0.3;

/// Trips a base may run at once, dispatched and standing combined.
/// Unmeasured — the cap exists so a maxed-out Relay cannot queue unbounded
/// `TransitCue` traffic, not because a throughput number was derived for it.
pub const ROUTE_MAX_ACTIVE: usize = 4;

/// The most an outpost route's outbound leg loads off an outpost's stock in
/// one arrival, summed across every item — `routes::RouteEnd::Outpost`'s own
/// leg, `OUTPOST_STOCK_CAP`'s sibling rather than a share of it, since a
/// route's haul and an outpost's storage are two different questions.
/// Unmeasured — see design spec §11.
pub const ROUTE_OUTPOST_CARRY: u32 = 20;

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
