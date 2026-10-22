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
/// Mood header and their spacing. Raising this past what fits means giving
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

/// Where an opinion of another program becomes an enemy, in `bonds::band`.
///
/// Placeholders until play; `balance_sim` gates none of them. Two fresh
/// `turned_on_me` strikes (-14) plus a witness reach it.
pub const BOND_ENEMY_AT: f32 = -15.0;

/// Where an opinion becomes a rival — `MEMORY_AVOIDANCE_THRESHOLD`'s value,
/// the same "worth avoiding", stated separately so a retune of one does not
/// silently move the other.
pub const BOND_RIVAL_AT: f32 = -3.0;

/// Where an opinion becomes a friend: two strikes of `idled_with` or
/// `bonded_in_battle`.
pub const BOND_FRIEND_AT: f32 = 8.0;

/// Where an opinion becomes close: a full `bonded_in_battle` stack.
pub const BOND_CLOSE_AT: f32 = 20.0;

/// How far, Chebyshev, from a brawl's victim a staff body still sees it
/// happen and writes `saw_turn_on`.
pub const BOND_WITNESS_REACH: i32 = 2;

/// How far, Chebyshev, from a machine a sulking program must stand to spoil
/// its output (`Game::note_sabotage`).
pub const SABOTAGE_REACH: i32 = 1;

/// The chance, per `INTERACTION_PERIOD`, that a sulker with a candidate
/// machine in reach spoils a unit.
pub const SABOTAGE_CHANCE: f64 = 0.25;
const _: () = assert!(SABOTAGE_CHANCE > 0.0 && SABOTAGE_CHANCE <= 1.0);

/// Seeds `note_sabotage`'s folds. Own constant, `CARAVAN_SALT`'s rule.
pub const SABOTAGE_SALT: u64 = 0x5AB0_7A6E_5EED_0001;

/// How far one point of `Game::morale` shifts a worker's extraction
/// reliability, in `systems::mining_success_chance`.
///
/// **Signed around a baseline of zero**, exactly like Analysis's term in
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

/// How often `Game::note_interactions` pairs idle staff and rolls for a
/// word between them, in ticks.
pub const INTERACTION_PERIOD: u64 = MEMORY_POSTING_PERIOD;
// A pass faster than the posting period would let an interaction memory land
// between two posting writes and make eviction's winner depend on who
// happened to be idle, `MEMORY_POSTING_PERIOD`'s own argument.
const _: () = assert!(INTERACTION_PERIOD >= MEMORY_POSTING_PERIOD);

/// The chance a paired speaker speaks at all, before its `Sociability`.
pub const INTERACTION_CHANCE: f64 = 0.5;

/// `Sociability`'s multipliers on `INTERACTION_CHANCE`.
pub const SOCIABILITY_RESERVED: f64 = 0.5;
pub const SOCIABILITY_SOCIABLE: f64 = 1.0;
pub const SOCIABILITY_CHATTY: f64 = 1.5;

/// Folded last into `Sociability`'s seed, which domain-separates it from
/// `Disposition::seed` (the same `[id]` fold, unsalted). Trailing because with
/// no bytes after the id its contribution is one fixed multiply, so adjacent
/// ids would give structured, correlated outputs; the salt's bytes mix it.
pub const SOCIABILITY_SALT: u64 = 0x9e37_79b9_7f4a_7c15;
/// Folded last into every roll `note_interactions` reads, for the same
/// reasons.
pub const INTERACTION_SALT: u64 = 0xc2b2_ae3d_27d4_eb4f;
/// The most lines one exchange may have; the TALK page must fit whole ones.
pub const CONVERSATION_MAX_LINES: usize = 4;
/// Conversation records a program keeps; the oldest is dropped past this.
pub const CONVERSATION_RING: usize = 32;

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

/// The most the situational thoughts can add to or take from a program's
/// morale in total (`situations::sum`), either way.
///
/// **Below `-MORALE_SULKS_AT`**, so where a program stands can tip one already
/// soured by real memories into sulking but can never start a sulk alone:
/// otherwise base layout and power budget become Grievance triggers by
/// themselves, and a sulker that refuses its post changes its own situation,
/// recovers, is re-posted and sulks again. Unmeasured, a starting value to
/// revisit after play.
pub const SITUATION_MAX_TOTAL: f32 = 5.0;
const _: () = assert!(SITUATION_MAX_TOTAL < -MORALE_SULKS_AT);

/// The ladder climbs in order, and the gap between recovery and downing
/// tools *is* the feature.
///
/// Equal, the marker flickers every tick at the boundary, which is the whole
/// reason `Disgruntled` is stored rather than derived. Out of order, a
/// program goes straight from content to useless and the mild sulking rung
/// is unreachable. The hysteresis gap `MORALE_RECOVERED_AT >
/// MORALE_DOWNS_TOOLS_AT` follows from the chain and is not asserted twice.
const _: () = assert!(MORALE_RECOVERED_AT > MORALE_SULKS_AT);
const _: () = assert!(MORALE_SULKS_AT > MORALE_DOWNS_TOOLS_AT);

/// How often a stranded respite errand is re-asked — every this many ticks,
/// read straight off `GameClock`, `MEMORY_POSTING_PERIOD`'s shape.
///
/// **The latch exists to stop a per-beat Dijkstra**: without it, a body
/// whose amenity is unreachable would leave the posting pool, fail a step,
/// rejoin the pool and leave again on every single beat for the rest of the
/// run. Never re-trying at all was the other failure this shipped with — a
/// route that reopens (a demolished wall, a rebuilt amenity) left a body
/// stuck until its mood recovered on its own, which a real save showed
/// taking tens of thousands of ticks for most of a roster. A period is the
/// middle way: one fresh walk per stranded body per period, so the cost
/// stays bounded no matter how many bodies are stuck at once.
///
/// Set to the same order as `MEMORY_POSTING_PERIOD` (250) rather than
/// shorter: a wrong answer here costs one Dijkstra, not a lost tick of
/// production, so there is no reason to ask more often than the base's other
/// slow-period upkeep already does.
pub const RESPITE_RETRY_TICKS: u64 = 250;

// ---------------------------------------------------------------------------
// Staff tantrums
// ---------------------------------------------------------------------------
// The rung past refusal, and the grace period that keeps a young base out of
// the whole feature. **Every figure here is unmeasured**, for the reason the
// `// Acting out` comment above already gives: morale is a signed sum of
// decayed intensities with no natural scale, and nothing in `balance_sim`
// models base production. They are chosen against the shipped valences and
// against `BAY_ADMISSION_HP_FRACTION`, and they are the first thing to
// revisit after a base has been watched.

/// How many staff a base needs before a program's unmet needs are allowed to
/// count against it. One half of `Game::base_is_established`.
///
/// **Development, not time.** A tick-based grace punishes a player who founds
/// late in a run and forgives one who founds early and then neglects the
/// place for an hour; how far the base has actually been built is the honest
/// measure of "has this player had a fair chance to build amenities yet."
/// Unmeasured, and chosen as that fair-chance line rather than fitted.
pub const BASE_ESTABLISHED_STAFF: usize = 8;

/// How many structures a base needs before the same. The other half, and the
/// two are `&&`: twelve programs in a bare base has not had the chance, and a
/// sprawling base with four bodies in it is not a pressure cooker. Held by
/// `a_base_short_of_staff_earns_no_need_grudges` and its mirror, one test per
/// half, because a predicate wired `||` passes a test that only ever starves
/// both halves at once.
pub const BASE_ESTABLISHED_STRUCTURES: usize = 8;

/// Morale at or below which a program stops standing still about it and
/// rounds on a colleague.
///
/// **Past what one memory can reach, inside what a pattern of two can** —
/// `MORALE_DOWNS_TOOLS_AT`'s own rule one rung further down, and the only
/// thing keeping this number honest. A grudge is felt as valence x
/// `strike_cap` x `DISPOSITION_MEMORY_SWING`, so the worst single memory the
/// game ships (`mauled_by`, -8 at a cap of 4, felt by an `Abrasive` program)
/// reaches -44.8; -75 sits well past it and inside the pair. Held by
/// `no_single_memory_can_make_a_program_lash_out` and
/// `two_bad_memories_can_still_make_a_program_lash_out` against the real
/// `assets/memories/`.
///
/// A rung nothing can reach is a deleted feature; a rung one bad afternoon
/// reaches is a base that brawls constantly.
pub const MORALE_LASHES_OUT_AT: f32 = -75.0;

/// The ladder climbs in order — held by `the_ladder_climbs_in_order`, and
/// here as well because a retune that inverts two rungs makes the milder one
/// unreachable rather than failing to compile.
const _: () = assert!(MORALE_DOWNS_TOOLS_AT > MORALE_LASHES_OUT_AT);

/// The chance, per base beat, that a program already on the `LashingOut`
/// rung with somebody in reach actually starts a fight.
///
/// A rate rather than a countdown, so a base that has one miserable program
/// in it does not brawl on a metronome. Unmeasured.
pub const TANTRUM_CHANCE_PER_TICK: f64 = 0.02;

/// How far a program will look for somebody to round on, chebyshev.
///
/// **No target in reach means no tantrum**, and that is what keeps this
/// feature free of pathing: a program on this rung has already left the
/// posting half of `schedule_base_labour` and is drifting through
/// `drift_idle_staff`, so it wanders into range of somebody on its own. A
/// "walk to your enemy" arm would be a second walk with its own interruption
/// rules, bought for nothing the player could see.
pub const TANTRUM_REACH_TILES: i32 = 3;

/// The shortest and longest a fight runs, in base beats.
///
/// The short end is the one with a constraint on it — see
/// `TANTRUM_DAMAGE_FRACTION`.
pub const TANTRUM_TICKS_MIN: u32 = 4;
pub const TANTRUM_TICKS_MAX: u32 = 8;

const _: () = assert!(TANTRUM_TICKS_MIN < TANTRUM_TICKS_MAX);

/// What one blow is worth, as a fraction of the target's `max_hp`, **before**
/// mitigation and before the non-lethal clamp.
///
/// **A quarter rather than the single-digit fraction a "scuffle" suggests,
/// and the constraint binds at the *short* end of the tick range.** A
/// four-beat brawl is the shortest one that can happen, so four blows from
/// full health must already carry a body under
/// `BAY_ADMISSION_HP_FRACTION` — that is the whole of "whoever comes out
/// worst is swept into a Repair Bay", and it is what this number is sized
/// for.
///
/// Two things make the arithmetic softer than it looks and both push the same
/// way: `Game::apply_damage` runs `mitigate_incoming_damage`, so the landed
/// figure is lower than the fraction says; and a brawl rarely starts from
/// full health. So the number is pinned by
/// `a_short_brawl_still_fills_the_bay` driving a real fight against real
/// mitigation and **not** by the multiplication above — if a retune moves
/// mitigation, that test is what notices.
///
/// At the long end the `.min(hp - 1)` clamp does the rest: eight blows at a
/// quarter each would be 200% of max HP, so a long brawl simply leaves both
/// parties on 1 Integrity. Heavy is the point; the clamp is what keeps heavy
/// from being fatal.
pub const TANTRUM_DAMAGE_FRACTION: f32 = 0.25;

/// How long after coming out of a fight a program may not start another.
///
/// Bounds the *rate* while `vented` moves the meter back the other way. One
/// vent does not clear a -75 hole and is not meant to, so without this a
/// badly-run base would brawl on every roll. Unmeasured.
pub const TANTRUM_COOLDOWN_TICKS: u64 = 400;

/// How far from a brawl a bystander puts itself when it has no door to leave
/// by — the commons, or a room with no way out. Chebyshev tiles from the
/// victim. Purely for the look of the thing: nobody is ever hurt by
/// standing close. Unmeasured.
pub const TANTRUM_FLEE_TILES: i32 = 5;

/// How long bystanders hold their distance after the fight closes, before
/// the posted ones are free to walk back to their stations. Unmeasured.
pub const TANTRUM_FLEE_LINGER_TICKS: u64 = 10;

// The linger starts the moment a body is clear of the fight's room, which can
// be before the brawl closes. Outlasting the longest brawl is what keeps it
// from lapsing and walking back into a fight still going.
const _: () = assert!(TANTRUM_FLEE_LINGER_TICKS > TANTRUM_TICKS_MAX as u64);

/// How many rows the base output page shows per section.
///
/// The page has no scroll, so this is a layout constraint rather than a
/// preference: a modded item set must degrade to the most-produced rows
/// rather than overflowing the screen. Five is what two sections of it, the
/// four rows `Game::attention` can hold at once and the page's own chrome
/// leave room for in a `PopupSize::Large` at the smallest window the
/// renderer sweeps — `the_tallest_base_output_page_fits_its_popup` is the
/// gate, and raising this means giving the page a scroll first.
pub const BASE_OUTPUT_MAX_ROWS: usize = 5;

/// How many buckets a row's sparkline draws. Cells are cheap on this page;
/// rows are what is scarce, so the spark rides the row it belongs to.
pub const BASE_OUTPUT_SPARK_BUCKETS: usize = 8;
