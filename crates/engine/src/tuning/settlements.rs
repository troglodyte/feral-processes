// ---------------------------------------------------------------------------
// Travel
// ---------------------------------------------------------------------------

/// How far past the goal-to-player Chebyshev distance `Game::travel_step`
/// widens its search box, on top of `pursuit::walk_field`'s own radius-as-
/// box-not-budget rule (see that function's doc). A route that has to
/// detour around one obstacle costs more than the raw distance, and this is
/// the slack that keeps the detour inside the box rather than reading as
/// `NoRoute` — `CARAVAN_SPAWN_DISTANCE_TILES`'s neighbour in spirit, an
/// unmeasured deliberate number rather than a fitted one.
pub const TRAVEL_ROUTE_MARGIN: i32 = 8;

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

/// The side of one condition cell, in tiles — the grain at which a
/// `GroundCondition` claims ground or leaves it clear.
///
/// A cell rather than a tile because a patch has to be something the player
/// can *see* and route around: at tile granularity attrition reads as noise
/// that follows you everywhere, and there is nothing to learn. Sixteen puts
/// one or two blotches inside a typical biome crossing (straight-line runs
/// through Null Sector and Backplane measure median 9 tiles, p90 18), which
/// is big enough to recognise and small enough that a crossing is a route
/// choice rather than a wall.
pub const CONDITION_CELL_TILES: i32 = 16;

/// Weight of "nothing claims this cell" against `CONDITION_CLAIM_WEIGHT`,
/// in the roll `Game::condition_at` reduces.
///
/// **This is the constant that makes `GroundCondition::for_biome`'s promise
/// true.** That doc has always said unclaimed is the common case; it was a
/// statement about the catalogue, and nothing ever checked it against the
/// map worldgen actually produces. Null Sector and Backplane together are
/// about three quarters of walkable ground (`WorldMap::classify`, Perlin at
/// 0.03-0.05), so while a condition claimed its whole biome, three steps in
/// four attrited — and silently, since the bite pushed no line. At 3:1 the
/// claimed share of those biomes falls to a quarter, putting roughly a fifth
/// of walkable ground under a condition and the rest back to scenery.
/// `tests::environment::unclaimed_ground_is_the_common_case` is the census
/// that holds it.
pub const CONDITION_CLEAR_WEIGHT: u32 = 3;

/// Weight of "this cell carries its biome's condition", against
/// `CONDITION_CLEAR_WEIGHT`. Shared by all three conditions rather than
/// authored per-def like `StaticDef::weight`: at most one condition can ever
/// claim a biome (`for_biome` is a match), so the pool is always a single
/// entry and a per-condition weight would be a knob with nothing to balance
/// against.
pub const CONDITION_CLAIM_WEIGHT: u32 = 1;

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

// ─────────────────────────────────────────────────────────────────────────
// Settlements: where towns stand, what their shelves carry, how they price
// ─────────────────────────────────────────────────────────────────────────

/// How many chunks across a settlement region is.
///
/// The map is unbounded, so density is the only thing that can be tuned:
/// this and `SETTLEMENT_REGION_PERCENT` together say how far apart towns
/// are. At 4 chunks a region is 128 tiles across — the map viewport is
/// about 45 x 25 tiles, so the nearest town is a median three screens off
/// and a town is still never visible from the last one, which is the claim
/// the spacing exists to make.
///
/// **Halved from 8 (256 tiles) after play read as too far to walk.** The
/// measured median from the anchor to the nearest town went 147 tiles to
/// 71, and the 90th percentile 227 to 102 — the tail is what was actually
/// wrong, since a quarter of runs asked for a 200-tile hike before the
/// player had met anyone. Every settlement radius is a fraction of
/// `placement::REGION_TILES`, so this is close to a pure travel-time change:
/// the share of worlds with a town inside `SETTLEMENT_GARRISON_RADIUS` is
/// 39.6% at 4 chunks against 39.2% at 8. It is not *exactly* one, because
/// `placement::REGION_EDGE_INSET` is flat and did not scale — see the
/// 2026-09-06 follow-up in
/// `docs/measurements/2026-09-05-settlement-aid-reach.md`, which records
/// both sweeps and the predation share that fell with it.
pub const SETTLEMENT_REGION_CHUNKS: i32 = 4;

/// How likely a region is to hold a settlement at all, in percent.
///
/// Under 100 on purpose. A settlement in every region is a grid, and a grid
/// is not a place — the emptiness between them is what makes finding one
/// mean anything.
pub const SETTLEMENT_REGION_PERCENT: usize = 45;

/// How far from its derived cell a settlement will look for ground it can
/// stand on.
///
/// Bounded rather than unbounded: a region that is mostly Data Void should
/// leave its settlement unplaced rather than shove it hundreds of tiles to
/// the first standable tile anywhere, which would put it somewhere the
/// derivation never chose and, at the extreme, outside its own region.
pub const SETTLEMENT_SITE_SEARCH_TILES: i32 = 24;

/// A Server's footprint half-width — `Game::settlement_radius`'s Server
/// arm. A 3x3 square, so the one tile `move_player`'s bump ladder used to
/// admit nobody onto becomes nine.
pub const SETTLEMENT_RADIUS_SERVER: i32 = 1;

/// A Starved Mainframe's half-width — the same square as the town it may
/// have grown out of, `growth::Vitality::rows`'s reason one axis over: a
/// neglected city has nowhere worse to fall to on size than it does on
/// shelf.
pub const SETTLEMENT_RADIUS_STARVED: i32 = 1;

/// A Steady Mainframe's half-width, and what a freshly grown city reads —
/// `growth::vitality_floor` holds an untraded city here.
pub const SETTLEMENT_RADIUS_STEADY: i32 = 2;

/// A Thriving Mainframe's half-width, the largest a footprint ever gets.
pub const SETTLEMENT_RADIUS_THRIVING: i32 = 3;

/// The largest radius any settlement ever reads, over the four above — a
/// `const` expression rather than a restated 3, so a retune of
/// `SETTLEMENT_RADIUS_THRIVING` moves the outpost gate with it instead of
/// leaving it quietly wrong.
pub const SETTLEMENT_FOOTPRINT_MAX_RADIUS: i32 = {
    let radii = [
        SETTLEMENT_RADIUS_SERVER,
        SETTLEMENT_RADIUS_STARVED,
        SETTLEMENT_RADIUS_STEADY,
        SETTLEMENT_RADIUS_THRIVING,
    ];
    let mut max = radii[0];
    let mut i = 1;
    while i < radii.len() {
        if radii[i] > max {
            max = radii[i];
        }
        i += 1;
    }
    max
};

/// How often a settlement's shelf turns over — one epoch per this many
/// ticks, `Game::settlement_epoch`'s divisor.
///
/// Its own constant rather than a reused `CARAVAN_VISIT_INTERVAL_TICKS`: a
/// settlement's shelf is a property of the *world*, seeded off
/// `WorldMap::seed()`, where a caravan's visit is a property of the *base*
/// that travels. Sharing the rotation would tie two schedules nothing else
/// connects.
pub const SETTLEMENT_MARKET_ROTATION_TICKS: u64 = 900;

/// Salts a settlement's shelf seed apart from its placement (`placement.rs`
/// has its own four salts) and from everything else derived off the world
/// seed. Own constant, `CARAVAN_SALT`'s rule.
pub const SETTLEMENT_MARKET_SALT: u64 = 0x5E77_1E5E_5EED_0004;

/// How many shelf rows a Server draws — `Game::draw_shelf`'s `rows`.
///
/// A Server is a stop, not a destination (`SettlementKind`'s own doc), so
/// its shelf is deliberately thin: a `CaravanDef`'s `rows` field is the
/// same knob per shipped trader, and this is its Server-sized reading.
///
/// **Also the floor a Starved city falls to**, `growth::Vitality::rows` —
/// which is what stops a neglected city ever reading worse than the town it
/// grew out of.
pub const SETTLEMENT_SERVER_ROWS: u32 = 6;

/// A **Thriving** city's shelf rows — more than double a Server's, so a
/// city the party keeps in business reads as worth the walk rather than as
/// a bigger version of the same stop.
///
/// The ceiling of `growth::Vitality::rows` and not the flat reading of a
/// `Mainframe`: an untouched city draws `SETTLEMENT_STEADY_ROWS`, and this
/// is what commerce at `SETTLEMENT_COMMERCE_THRIVING` buys.
pub const SETTLEMENT_MAINFRAME_ROWS: u32 = 14;

/// What share of a Server's gear rows are standout stock —
/// `bonus_row_count`'s `share`, a `CaravanDef::bonus_share` reading sized
/// for the smaller settlement. Also a Starved city's share,
/// `SETTLEMENT_SERVER_ROWS`' reason.
pub const SETTLEMENT_SERVER_BONUS_SHARE: u32 = 15;

/// A **Thriving** city's standout share — a city's gear is worth digging
/// through in a way a Server's thin shelf cannot support.
/// `SETTLEMENT_MAINFRAME_ROWS`' companion, and a ceiling for its reason.
pub const SETTLEMENT_MAINFRAME_BONUS_SHARE: u32 = 35;

/// A grown-but-Steady city's shelf rows — `growth::Vitality::rows`.
///
/// Between the shipped `SETTLEMENT_SERVER_ROWS` (6) and
/// `SETTLEMENT_MAINFRAME_ROWS` (14), and it is the value an *untouched*
/// city draws: `SETTLEMENT_COMMERCE_*` bands 0 as Steady, and 0 is where
/// every authored Mainframe starts in every fresh world. So this, not 14,
/// is the number a player meets first.
pub const SETTLEMENT_STEADY_ROWS: u32 = 10;

/// A Steady city's standout share, `SETTLEMENT_STEADY_ROWS`' companion and
/// midway between the two shipped shares for its reason.
pub const SETTLEMENT_STEADY_BONUS_SHARE: u32 = 25;

/// The earliest tick a Server can grow into a Mainframe on the clock alone
/// — `growth::due_tick`'s floor.
///
/// **Measured, not guessed**: see `docs/measurements/2026-09-06-run-length.md`.
/// Shipped dev-save templates reach a mature run state at ticks 5344-6944,
/// which is a *floor* on how long a real run takes (a driven template skips
/// the ticks a player spends walking and reading). This sits below that
/// floor so the earliest-dated towns in a world grow inside a run that
/// never trades with them, and still 1.7x `SETTLEMENT_BOARD_ROTATION_TICKS`
/// — the longest *recurring* cadence shipped, and so the class growth could
/// be mistaken for — so growth cannot read as one more rotation.
/// `SETTLEMENT_GIFT_COOLDOWN_TICKS` (9000) is longer still but is a
/// one-shot gate rather than a cadence; the measurement records why that is
/// the wrong comparison and what it costs to sit below it.
pub const SETTLEMENT_GROWTH_DUE_MIN: u64 = 3000;

/// The latest tick a Server can grow on the clock alone.
///
/// Roughly twice the measured maturity floor, which is deliberate: towns
/// dated in the upper half of this span do **not** grow within a typical run
/// unless the player trades them forward. A span that every town cleared on
/// time would make `SETTLEMENT_COMMERCE_PULL_TICKS` decorative.
pub const SETTLEMENT_GROWTH_DUE_MAX: u64 = 12000;

/// Salts the growth clock apart from `placement.rs`' four questions and from
/// `SETTLEMENT_MARKET_SALT`. Own constant, `CARAVAN_SALT`'s rule: one fold,
/// salted per question, so no two questions off a region can collide.
pub const SETTLEMENT_GROWTH_SALT: u64 = 0x5E77_1E5E_5EED_0005;

/// How many Credits of trade buy one point of commerce —
/// `Game::credit_trade_volume`'s second reading of the same volume.
///
/// **Cheaper than `SETTLEMENT_TRADE_CREDITS_PER_POINT` (250) on purpose.**
/// If the two were equal, commerce and standing would move in lockstep off
/// one input and the second axis would be a second spelling of the first.
/// At 150, commerce responds visibly faster than goodwill does — which is
/// the right asymmetry: a town notices your money before it likes you.
pub const SETTLEMENT_COMMERCE_CREDITS_PER_POINT: u32 = 150;

/// How many ticks one point of commerce pulls a Server's growth date
/// earlier — `growth::pull_ticks`.
///
/// At `SETTLEMENT_COMMERCE_MAX` (100) this is 2500 ticks, which is most of
/// the way from `SETTLEMENT_GROWTH_DUE_MAX` toward `_MIN` but provably
/// never past it — see the `const _` in `growth.rs`. Trade is meant to be
/// the difference between a town that grows this run and one that does not,
/// not a button that founds a city.
pub const SETTLEMENT_COMMERCE_PULL_TICKS: u64 = 25;

/// How long one drift epoch is — `Game::settlement_growth_tick` settles
/// commerce against `now / this`, `static_epoch`'s shape, so a fast-forward
/// cannot be outrun and no per-tick arithmetic runs over every town.
pub const SETTLEMENT_COMMERCE_DECAY_TICKS: u64 = 600;

/// Points of commerce a town loses per epoch to plain neglect.
///
/// Two per 600 ticks costs a city about 20 points across a mature run — a
/// drift the player can outpace with modest trade and will not notice if
/// they are using the town at all. Neglect starves a city over roughly
/// 12000 ticks, which is a run's worth of ignoring it, not a punishment for
/// a quiet week.
pub const SETTLEMENT_COMMERCE_DECAY: i32 = 2;

/// **Extra** points lost per epoch while the town's band is `Hostile` — on
/// top of `SETTLEMENT_COMMERCE_DECAY`, not instead of it.
///
/// Triples the drift, so a city you have turned Hostile starves in about a
/// third of the time neglect alone would take. Read off the *current* band
/// and never a history: repairing standing stops the acceleration the tick
/// it lands.
pub const SETTLEMENT_COMMERCE_HOSTILE_DECAY: i32 = 4;

/// The floor every commerce writer clamps to — `growth::clamp_commerce`,
/// `relations::clamp`'s rule and the same reason: one clamp is only enough
/// because there is one door.
///
/// Mirrors `SETTLEMENT_MIN_STANDING`/`_MAX_STANDING`'s +/-100 axis so the two
/// per-town numbers read on the same scale.
pub const SETTLEMENT_COMMERCE_MIN: i32 = -100;

/// The ceiling. See `SETTLEMENT_COMMERCE_MIN`.
pub const SETTLEMENT_COMMERCE_MAX: i32 = 100;

/// At or above this, a grown city is Thriving and draws a full Mainframe
/// shelf — `growth::vitality`.
///
/// **Must be strictly above zero**, and `growth.rs` asserts it at compile
/// time. Zero is where an untouched authored Mainframe sits, and a
/// threshold at or below zero would band Tally Yard and Kernel Reach as
/// Thriving in every fresh world before anyone had traded a Credit.
pub const SETTLEMENT_COMMERCE_THRIVING: i32 = 40;

/// At or below this, a grown city is Starved and its shelf falls to the
/// Server floor.
///
/// **Must be strictly below zero**, asserted in `growth.rs`, for
/// `SETTLEMENT_COMMERCE_THRIVING`'s mirrored reason: a threshold at or above
/// zero would open every authored Mainframe Starved.
pub const SETTLEMENT_COMMERCE_STARVED: i32 = -40;

/// Every shelf bucket's weight before `Specialty` adds its own bonus —
/// `Game::specialty_weights`. Equal, so an unbiased bucket only exists in
/// the sense that no specialty favours it; the shelf still draws from all
/// four.
pub const SETTLEMENT_BASE_WEIGHT: u32 = 10;

/// What a settlement's own `Specialty` adds to its favoured bucket's
/// weight, on top of `SETTLEMENT_BASE_WEIGHT`. Large relative to the base
/// weight so a specialty reads as a lean on the shelf rather than as
/// flavour text nothing draws differently.
pub const SETTLEMENT_SPECIALTY_WEIGHT_BONUS: u32 = 25;

/// What a settlement pays per point of `Game::item_value`, before its
/// `Temperament` scales it — `Game::settlement_sell_price`. Its own
/// constant rather than a reused `STACK_MARKET_SELL_RATE`: the two are
/// unrelated vendors that only happen to share a number today.
pub const SETTLEMENT_SELL_RATE: u32 = 1;

/// The Temperament price table (shape, not fitted): how much a settlement's
/// own disposition moves what you pay and what it pays you, each scaling
/// `Game::marked_unit_cost`'s markup half or `SETTLEMENT_SELL_RATE`
/// respectively. Six constants rather than a formula over one "friendliness"
/// scalar, because Mercantile is deliberately **not the average** of Open
/// and Guarded: it competes on the buy side and takes its margin on the
/// sell side, which a single axis cannot express — see `Temperament::buy_mult`
/// and `Temperament::sell_mult`.
pub const SETTLEMENT_OPEN_BUY_MULT: f32 = 0.9;
/// See `SETTLEMENT_OPEN_BUY_MULT`.
pub const SETTLEMENT_OPEN_SELL_MULT: f32 = 1.1;
/// See `SETTLEMENT_OPEN_BUY_MULT`.
pub const SETTLEMENT_GUARDED_BUY_MULT: f32 = 1.1;
/// See `SETTLEMENT_OPEN_BUY_MULT`.
pub const SETTLEMENT_GUARDED_SELL_MULT: f32 = 0.9;
/// Roughly neutral on the buy side — Mercantile competes for your custom
/// there rather than discounting or marking up. See `SETTLEMENT_OPEN_BUY_MULT`.
pub const SETTLEMENT_MERCANTILE_BUY_MULT: f32 = 1.0;
/// Where Mercantile actually takes its margin — steeper than Guarded's sell
/// discount, because everything is business including what it pays you.
/// See `SETTLEMENT_OPEN_BUY_MULT`.
pub const SETTLEMENT_MERCANTILE_SELL_MULT: f32 = 0.85;

// ── Relations: what a town thinks of you ──
// See `settlements::relations`. The band thresholds are read on every
// lookup rather than stored, so moving one re-bands every existing save.

/// The floor and ceiling `settlements::relations::clamp` holds standing to,
/// and the only bounds in the feature — `Game::adjust_standing` is the one
/// writer, which is what makes a single clamp enough.
pub const SETTLEMENT_MIN_STANDING: i32 = -100;
/// See `SETTLEMENT_MIN_STANDING`.
pub const SETTLEMENT_MAX_STANDING: i32 = 100;

/// At or below this, a town refuses service — see
/// `settlements::relations::Standing::refuses_service`. A player must have
/// worked at it rather than drifted into it: closing a market is the
/// harshest thing standing does.
///
/// **Measured in deeds, not in points, and that is the whole of the fix
/// here.** The two negative thresholds used to mirror the positive ones
/// exactly (`-50`/`-15` against `+50`/`+15`), which reads as symmetric and
/// is not: the movers are not mirrored. The way up pays
/// `SETTLEMENT_CONTRACT_STANDING` (`+10`) a deed, so `Allied` is five deeds
/// out. The way down charges `SETTLEMENT_ABANDON_STANDING` (`-4`) — smaller
/// on purpose, see its own doc — so a mirrored `-50` put `Hostile`
/// **thirteen** abandoned contracts away, with no other negative mover in
/// the game to help and any single trade or cleared nest undoing progress.
/// Nothing reached it, so `Standing::refuses_service` and
/// `Standing::preys_on_routes` were wired, tested and unreachable.
///
/// Mirroring the *effort* instead puts both ends five deeds from `Neutral`
/// and leaves the mover asymmetry where it was designed to be.
/// `every_band_is_reachable_through_movers_that_exist` is the census that
/// holds it; `SETTLEMENT_MIN_STANDING` still leaves room below.
pub const SETTLEMENT_HOSTILE_STANDING: i32 = -20;
/// The top of the Cold band; below this a town is merely unfriendly. Two
/// deeds out, mirroring `SETTLEMENT_WARM_STANDING`'s distance in deeds
/// rather than in points — see `SETTLEMENT_HOSTILE_STANDING`.
pub const SETTLEMENT_COLD_STANDING: i32 = -8;
/// The bottom of the Warm band.
pub const SETTLEMENT_WARM_STANDING: i32 = 15;
/// The bottom of the Allied band.
pub const SETTLEMENT_ALLIED_STANDING: i32 = 50;

/// How many Credits have to move across a town's counter to buy one point
/// of standing. Counted on the whole basket — what the party spends *and*
/// what it is paid — since either direction is business the town did.
/// `Relation::trade_credits` keeps the remainder, so the figure is a volume
/// rate rather than a per-basket rounding.
pub const SETTLEMENT_TRADE_CREDITS_PER_POINT: u32 = 250;

/// How near a town something has to happen for the town to hear about it —
/// `Game::credit_nearby_settlements`. Roughly a quarter of
/// `SETTLEMENT_REGION_CHUNKS * CHUNK_SIZE`, so clearing a nest is local news
/// to at most the town whose region it stands in.
pub const SETTLEMENT_NOTICE_RADIUS: i32 = 60;

/// What bringing down a nest within that radius is worth.
pub const SETTLEMENT_NEST_CLEARED_STANDING: i32 = 4;

/// What collapsing a stack within that radius is worth — more than a nest,
/// since a stack is the larger thing and takes its guardian with it.
pub const SETTLEMENT_STACK_COLLAPSED_STANDING: i32 = 8;

/// Salts a town's job board apart from its shelf (`SETTLEMENT_MARKET_SALT`)
/// and from the Broker's board (`CONTRACT_BOARD_SALT`). All three fold the
/// world seed and an epoch, so without three salts a town's board and its
/// shelf would rotate in lockstep off one number.
pub const SETTLEMENT_BOARD_SALT: u64 = 0x005E_771E_B0A7_D001;

/// How many cycles a town's board stands before it re-derives. Slower than
/// the Broker's `CONTRACT_REFRESH_CYCLES` because a town is a place you walk
/// to: a board that had rotated by the time you arrived would make the walk
/// unplannable.
pub const SETTLEMENT_BOARD_ROTATION_TICKS: u64 = 1800;

/// How many jobs a town posts, by how much it likes you — see
/// `Standing::job_slots`. Hostile posts none (`refuses_service`), and the
/// ladder is what makes courting a town buy something other than a price.
pub const SETTLEMENT_COLD_BOARD_SLOTS: usize = 1;
pub const SETTLEMENT_NEUTRAL_BOARD_SLOTS: usize = 2;
pub const SETTLEMENT_WARM_BOARD_SLOTS: usize = 3;
pub const SETTLEMENT_ALLIED_BOARD_SLOTS: usize = 4;

/// What finishing a town's own job is worth to it — more than clearing a
/// nest on its doorstep, since the job was asked for.
pub const SETTLEMENT_CONTRACT_STANDING: i32 = 10;

/// And what handing one back costs. Smaller in magnitude than the reward:
/// taking a job you cannot finish should not be worse than never taking it,
/// or the board becomes a thing to avoid reading.
pub const SETTLEMENT_ABANDON_STANDING: i32 = -4;

/// What a town stations around your base once it likes you, added to
/// `Game::total_raid_defense` — see `Standing::garrison_defense`. One Allied
/// town is worth exactly one Shield (`assets/structures/shield.ron` grants
/// `raid_defense: 2`), which is the scale that makes courting a neighbour
/// comparable to building something.
///
/// Unmeasured: no instrument in this repo models a raid, so whether a
/// garrison is *noticeable* is a play question.
pub const SETTLEMENT_WARM_GARRISON: u32 = 1;
/// See `SETTLEMENT_WARM_GARRISON`.
pub const SETTLEMENT_ALLIED_GARRISON: u32 = 2;

/// The ceiling on the **settlement half** of raid defense — the sum over
/// towns is clamped to this before the structure sum is added, never after.
///
/// Strictly below `RAID_DAMAGE`, and that is the whole point: however many
/// Allied neighbours the party collects, a sweep still lands for something.
/// Without it, `RAID_DAMAGE.saturating_sub(total_raid_defense())` reaches
/// zero and raids stop happening in everything but the log line, which is a
/// mechanic deleted rather than softened. Structures may still zero a sweep
/// — two Shields already do — because that is a thing the player built.
pub const SETTLEMENT_GARRISON_MAX: u32 = 3;

/// How close a town has to be to the base anchor to station anyone there,
/// in Chebyshev tiles.
///
/// **Half a region, and scaled off region spacing rather than flat.** The
/// anchor does not move, so this radius is a per-run coin flip against a
/// derivation that puts at most one town in every
/// `SETTLEMENT_REGION_CHUNKS`-chunk square. At the flat 40 it shipped with,
/// 1.6% of worlds had any town near enough to garrison at all — the ladder's
/// passive half was unreachable by geometry rather than unearned. Half a
/// region reaches 39%, which is the anchor's own region and the ones
/// touching it, and is still a thing the seed decides rather than a thing
/// every base gets.
///
/// This is deliberately **outside** `SETTLEMENT_NOTICE_RADIUS`, inverting
/// what shipped. The two measure different things: noticing a deed needs
/// proximity to the *deed*, and the party roams, so a quarter-region hears
/// plenty; stationing a detachment needs only willingness, which
/// `Standing::garrison_defense` already gates at `Warm`. Distance here is a
/// sanity bound on how far a town will reach, not the price of the aid.
///
/// Measured — `docs/measurements/2026-09-05-settlement-aid-reach.md`.
pub const SETTLEMENT_GARRISON_RADIUS: i32 = crate::settlements::placement::REGION_TILES / 2;

/// How often a Hostile neighbour tries the party's stores — `Game::town_raid_check`.
///
/// **Half the ambient sweep's rate, which is the invariant and not the
/// number.** The ambient sweep is weather; this is somebody's decision, and
/// it must stay the rarer of the two.
///
/// It used to read "half `RAID_CHANCE_PER_TICK`" at `0.006` against that
/// constant's `0.012`. `RAID_CHANCE_PER_TICK` is gone — the ambient sweep is
/// `resources::RaidPressure`'s clock now — so the rate it was half *of* has
/// to be derived rather than named: `RAID_PRESSURE_THRESHOLD /
/// (RAID_PRESSURE_PER_ZONE * 2)` is 2400 ticks between sweeps in sector 2,
/// a rate of about `0.00042`, and half of that rounds to the figure below.
/// Left at `0.006` the relationship inverts to fourteen times *more* often
/// than the thing it is supposed to be rarer than, which is why this moved
/// in the same change.
///
/// Anchored to sector 2 because this roll has no depth term of its own,
/// where the clock accrues faster the deeper you are: a town's grudge is
/// not a property of the sector it is in.
pub const SETTLEMENT_RAID_CHANCE_PER_TICK: f64 = 0.0002;

/// How close a Hostile town has to be to the anchor to bother, in Chebyshev
/// tiles.
///
/// **Equal to `SETTLEMENT_GARRISON_RADIUS` today, and deliberately its own
/// constant.** The hostile half of the ladder must reach exactly as far as
/// the friendly half or one band's consequence is geometrically rarer than
/// the other's — but retuning aid must not silently retune hostility, so the
/// equality is an argued coincidence rather than a shared symbol. See
/// `docs/measurements/2026-09-05-settlement-aid-reach.md` for the 39%-of-
/// worlds figure this inherits.
pub const SETTLEMENT_RAID_RADIUS: i32 = crate::settlements::placement::REGION_TILES / 2;

/// The share of the party's banked build currency a raid takes, in percent.
pub const SETTLEMENT_RAID_HAUL_PERCENT: u32 = 10;

/// How many percentage points each point of `Game::total_raid_defense` cuts
/// off that share.
///
/// At 2, a maxed garrison (`SETTLEMENT_GARRISON_MAX`, 3) cuts 6 of the 10 —
/// real relief, never immunity, which is the claim the `const _` in
/// `settlements::relations` fails the *build* over. Two Shields plus a
/// garrison do reach zero, because that is a thing the player built.
pub const SETTLEMENT_RAID_DEFENSE_PER_POINT: u32 = 2;

/// The least a raid that lands takes.
///
/// A percentage of a small bank rounds to zero, and a raid that takes
/// nothing while still logging is the mechanic deleted rather than softened
/// — `SETTLEMENT_GARRISON_MAX`'s failure, from the other side. Applied only
/// after the share survives defense; see `Game::run_town_raid` for why that
/// order is load-bearing.
pub const SETTLEMENT_RAID_HAUL_FLOOR: u32 = 1;

/// The most a raid takes, whatever the bank holds.
///
/// A percentage of a large bank scales without limit, which would make
/// banking itself the punished behaviour. A rich base is bled, not gutted.
pub const SETTLEMENT_RAID_HAUL_CAP: u32 = 40;

/// How many members a town's patrol fields — `Game::maybe_field_patrol`.
///
/// Between `NEST_GUARDIAN_MIN` (2) and `NEST_GUARDIAN_MAX` (5): a patrol is
/// an ordinary-encounter mechanic and should read as one, not as a boss.
pub const SETTLEMENT_PATROL_SIZE: u32 = 3;

/// The Chebyshev ring a patrol member is placed in, off the town's own tile.
///
/// **Band 0 is excluded by the minimum, not by a check** — a settlement tile
/// admits nobody (`move_player`'s fourth arm), the same reason a relay
/// landing searches from band 1. Close enough that the ground reads as the
/// town's.
pub const SETTLEMENT_PATROL_RING_MIN: i32 = 2;
/// See `SETTLEMENT_PATROL_RING_MIN`.
pub const SETTLEMENT_PATROL_RING_MAX: i32 = 6;

/// Chebyshev distance from its town past which a patrol member gives up.
///
/// **Equal to `NEST_AGGRO_LEASH_RADIUS` today, and that equality is a
/// coincidence the code must not rely on.** `pursuit_tick` builds one
/// shared Dijkstra field for both tethers, sized off the *maximum* of the
/// two leashes — raise this past the nest's without that `max` and patrols
/// read as absent from the field and give up where they stand, which is a
/// mechanic disappearing with no error anywhere.
pub const SETTLEMENT_PATROL_LEASH_RADIUS: i32 = 15;

/// How near the player a patrol member has to be before it notices them.
///
/// **Inside `EXAMINE_RANGE_TILES` (12) on purpose**, so the player can see
/// and identify a patrol before it notices them. A threat only ever
/// discovered by already being in a fight is not one the player can play
/// around.
pub const SETTLEMENT_PATROL_AGGRO_RADIUS: i32 = 8;

/// How near the party a Hostile town has to be to bother fielding a patrol.
///
/// A fraction of region spacing rather than a flat number, for
/// `SETTLEMENT_GARRISON_RADIUS`' measured reason — a flat length is dead or
/// dominant depending on a pitch it cannot see.
pub const SETTLEMENT_PATROL_RANGE: i32 = crate::settlements::placement::REGION_TILES / 2;

/// How long a town waits, on average, before fielding one more member.
///
/// **A mean and not a countdown.** `Game::maybe_field_patrol` rolls its
/// reciprocal once a tick — roll first, gate after, so a miss spends one
/// draw and touches nothing else. A nest keeps its countdown on the `Nest`
/// entity; `resources::Settlements` is the record that survives a save, and
/// a timer in it would be a save field for a figure nobody can see.
///
/// Far slower than `NEST_RESPAWN_TICKS` (10): wiping a patrol should buy
/// real time, or the fight is scenery that respawns behind you.
pub const SETTLEMENT_PATROL_RESPAWN_TICKS: u64 = 60;

/// What killing one patrol member costs you with its town.
///
/// **Not a free retune.** The movers pay `+10` a contract, `+8` a Stack
/// collapse, `+4` a nest cleared, `+1` per
/// `SETTLEMENT_TRADE_CREDITS_PER_POINT` traded. At `-1` a full
/// `SETTLEMENT_PATROL_SIZE` patrol wiped costs `-3`, less than clearing one
/// nest on that town's doorstep — which is what keeps the ladder out of
/// `Hostile` climbable while patrols are actively in the way. Past that,
/// self-defence outruns every mover a Hostile player can still reach, since
/// the market and the board are already shut, and the band stops being a
/// state you can leave.
///
/// **`-1` is the only value that satisfies the inequality, which is tighter
/// than the design spec claimed.** The spec put the danger at `-4`; at
/// `SETTLEMENT_PATROL_SIZE` 3 the arithmetic is `3 * 2 = 6` against a
/// cleared nest's `4`, so **`-2` already fails**. Measured by mutation, not
/// reasoned about. Raising this needs `SETTLEMENT_PATROL_SIZE` to fall or
/// `SETTLEMENT_NEST_CLEARED_STANDING` to rise with it.
/// `wiping_a_patrol_costs_less_than_clearing_one_nest_pays` asserts the
/// inequality directly.
pub const SETTLEMENT_PATROL_KILL_STANDING: i32 = -1;

/// How long a town waits between gifts — see `Game::request_program_gift`.
///
/// The limiter, because aid is free: the price of a gift was reaching
/// Allied, so the only thing left to bound it is time. Several times
/// `SETTLEMENT_MARKET_ROTATION_TICKS`, since a gift is a larger event than a
/// shelf turning over. Unmeasured.
pub const SETTLEMENT_GIFT_COOLDOWN_TICKS: u64 = 9_000;

/// Salts a gift's species roll apart from a town's shelf
/// (`SETTLEMENT_MARKET_SALT`) and its board (`SETTLEMENT_BOARD_SALT`). All
/// three fold the world seed and the town's own region coordinates, so
/// without three salts a town's first gift would be a function of the same
/// number its shelf is.
pub const SETTLEMENT_GIFT_SALT: u64 = 0x61F7_5E77_1E5E_0001;

/// The `stat_mult` a gifted program is spawned at — **labour, not power**.
///
/// Below the 1.0 an adopted or purchased program gets, on purpose: a free
/// companion scaled to the zone is the shape closed off when scan, the
/// Terminal, free rest and the Market's fragment listing were shut, because
/// progression here is earned by fighting. A gift feeds the base's labour
/// shortage instead. **This is the number doing that work** — nothing
/// structural stops the player fielding a gift, since `ProgramRole` is
/// derived; what makes it a bad trade is this constant. Unmeasured, and the
/// first thing to revisit after a play session.
pub const SETTLEMENT_GIFT_STAT_MULT: f32 = 0.6;
/// Multiplies `SETTLEMENT_GIFT_STAT_MULT` when the town's `Specialty` is
/// `Programs` — what a town known for programs gives you over one that
/// merely likes you. Still below 1.0 after the multiply, or decision 5 is
/// undone by the specialty case. Unmeasured.
pub const SETTLEMENT_GIFT_SPECIALTY_MULT: f32 = 1.25;

/// Ticks charged per Chebyshev tile of a relay trip — see
/// `Game::travel_to_settlement`.
///
/// Exactly what the walk would have cost: `Game::move_player` spends one
/// tick per step. So travel removes the encounters and the tedium and
/// removes **nothing else** — upkeep, decay, needs and production all
/// advance exactly as far as they would have. Not a guess, which is why it
/// is the one constant in this feature with an argument instead of a shrug.
pub const SETTLEMENT_TRAVEL_TICKS_PER_TILE: u64 = 1;
