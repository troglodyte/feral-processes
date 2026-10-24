use super::*;

// ---------------------------------------------------------------------------
// Production chains
// ---------------------------------------------------------------------------

/// How many units a structure's output buffer holds when its `.ron` file
/// sets no `capacity` — see `components::Stock`. This is what paces an
/// extractor now that a node has no deposit pool: it produces until the
/// buffer is full and then clogs until someone collects, so this number is
/// how long a base runs unattended.
pub const DEFAULT_OUTPUT_CAPACITY: u32 = 20;

/// The slowest recipe, in ticks per unit, still called "quick" on a machine's
/// recipe menu. The menu shows a band, never the figure, so the player picks
/// by feel and the tick count stays out of player-facing text.
pub const RECIPE_PACE_QUICK_MAX: u32 = 8;

/// The slowest recipe still called "steady"; see `RECIPE_PACE_QUICK_MAX`.
pub const RECIPE_PACE_STEADY_MAX: u32 = 14;

/// The slowest recipe still called "slow"; anything beyond is "very slow".
pub const RECIPE_PACE_SLOW_MAX: u32 = 20;

/// How many ticks one Power Cell keeps a `power_upkeep` supplier lit — see
/// `StructureDef::power_upkeep` and `systems::power_grid_system`.
///
/// **A cell is a long window, deliberately.** A Conduit at Mk1 in zone 1
/// turns out a cell every 6 ticks — 166 per 1,000 — while a burning supplier
/// eats one every 100, which is 10 per 1,000. One posted Conduit therefore
/// covers sixteen suppliers, and the fuel loop is a thing the player sets up
/// once and then stops thinking about rather than a rate the base has to
/// keep pace with.
///
/// That is a retune away from what this number first meant. At 20 a Conduit
/// sustained three suppliers and grid capacity *was* the production rate —
/// the argument being that upkeep the base can outrun is the same as free.
/// It is now close to free by that standard, and knowingly: the cost that
/// survives is **placement**, not throughput — though it survives more
/// weakly than it did. A supplier parked away from any feeder used to go
/// dark no matter how many cells the base was sitting on, and this window
/// was sized to be long enough for the player to notice and walk over to fix
/// it. The base staff now fetch one instead (`Game::fuel_wants`), so what a
/// bad placement costs is a body's round trip rather than the grid — and the
/// window is what makes that trip comfortably long enough to make.
pub const POWER_UPKEEP_TICKS: u32 = 100;

/// How many Power Cells a `power_upkeep` supplier spends to buy one window
/// of `POWER_UPKEEP_TICKS`.
///
/// Named rather than written `1` at each site because it is read in three
/// places that must agree: `systems::burn_grid_upkeep` spends it,
/// `systems::intake_recipe` reports it as the supplier's batch size so a
/// hauler fetches the right amount, and `Game::fuel_wants` asks whether one
/// is within reach. A supplier that burnt two and was fetched one would
/// simply never stay lit.
pub const POWER_UPKEEP_CELLS_PER_WINDOW: u32 = 1;

/// Integrity one Patch Routine buys when the party rests in base. A rest
/// spends `ceil(missing / REST_INTEGRITY_PER_PATCH)` patches, so the last one
/// rounds in the party's favour.
pub const REST_INTEGRITY_PER_PATCH: i32 = 40;

/// Integrity the party regains each tick while standing in base space out of
/// battle, for free. The floor under paid rest, not a rival to it: at the
/// world's two ticks a second a hundred points takes under a minute, a
/// full recovery from near zero a few.
pub const BASE_TRICKLE_PER_TICK: i32 = 1;

/// How many units of a `StructureDef::supply` make one delivery. Its input
/// buffer then holds `2 * SUPPLY_BATCH` (`input_room`), so a defence stays
/// charged across several uses between hauls.
pub const SUPPLY_BATCH: u32 = 3;

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

/// How many labour passes a rack carrier may stand on one tile before it
/// gives its program back to the shelter chain.
///
/// A carrier is held out of the posting pool for the whole trip, so a walk
/// that only ever waits (the party in the one corridor, a body that never
/// moves) would hold a body out of work for ever. Well past any queue a
/// squeeze would clear, and short enough that a stuck body is not a
/// permanent labour loss.
pub const RACK_CARRY_PATIENCE_BEATS: u32 = 120;

/// How many `IDLE_STAFF_STEP_TICKS` beats pass between the step offers a
/// program standing at its amenity gets.
///
/// Cosmetic, like the wander's own pace. Held perfectly still for a whole
/// refill a body read as stuck beside the building; at the wander's pace it
/// read as pacing. Every other beat sits between the two, and a body offered
/// a tile out of reach or a hold still stays put for that offer.
pub const AMENITY_MILL_BEATS: u64 = 2;

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

/// What a posted walker pays, on top of the step itself, to enter a cell
/// another program is standing in.
///
/// Bodies stopped blocking a posted walk because a ring base with a
/// one-cell corridor round it was closed by whoever stood in it, and a
/// carrier with no route held its load for thousands of ticks. **Not free**,
/// though: the same cost is the walk field's step price for that cell, so a
/// route detours round a crowd whenever the detour is no longer than the
/// wait, and passing through is the answer only where it is the quicker one.
/// One tick — the smallest cost the clock can charge, and enough to make a
/// clear corridor the preferred one.
pub const SQUEEZE_EXTRA_TICKS: u64 = 1;

/// How many programs one amenity refills per tick. **A placeholder**: the
/// number is a hard-coded two until a later session decides whether it is a
/// per-structure field, a tier scaling or a room bonus. Boxed in by
/// structures and rock, an amenity serves fewer
/// (`offshift::effective_places`).
pub const AMENITY_CAPACITY: usize = 2;

/// How long a carrier stands `Stranded` holding a load before it drops the
/// load on the floor around it, one unit per tile.
///
/// A route lost mid-carry strands a carrier: structures raised across it, a
/// body standing on a face of the destination (or on a non-destination face
/// the only route runs through, which is never crossed), or a destination
/// past the walk-radius cap. A carrier is never freed while it holds a load
/// — freeing would destroy the goods — so without a timeout that is a body
/// held on shift forever. A load is only lifted for a store the walk reaches,
/// so this is the recovery from a route lost after the pickup. Long
/// enough that the transient case clears on its own first — sixteen-odd
/// `IDLE_STAFF_STEP_TICKS` beats for a wanderer on a Depot's only free face
/// to step off it — and short against the 2,291-tick streaks the `chains`
/// bench measured.
pub const STRANDED_SET_DOWN_TICKS: u64 = 100;

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
/// buffs come off a chain rooted in an Extractor, which produces Core
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

// ---------------------------------------------------------------------------
// Program extraction
// ---------------------------------------------------------------------------

/// How many downed programs `components::DownedPrograms` may hold — a flat
/// row count for phase 1 (see the spec's "Open, deliberately" section for
/// the later move to a carried-weight metric). `Mode::DownedPrograms` (a
/// later phase) is a read-only row list with no scroll, so this doubles as
/// a layout constraint the way `BASE_OUTPUT_MAX_ROWS` is: it must fit at
/// 1280x720 alongside that screen's own chrome, which a later phase's test
/// asserts by mutation. Ten rather than matching `ROUTINE_SLOT_CAP`
/// (12): a downed-program row carries a species name, level, rarity and
/// boss tag — wider than a routine slot's single name — against the same
/// vertical budget, so it is set a step under that established fit rather
/// than assumed to match it.
pub const MAX_DOWNED_PROGRAMS: usize = 10;

/// The floor every downed program's condition roll starts from, before
/// rarity, boss and fight terms are added — see `items::DownedProgram` and
/// the spec's section 1 for the full formula. Left well below 100 so those
/// terms have room to move the result rather than being clamped away on
/// every ordinary kill, and well above 0 so a plain wild kill is still
/// worth carrying on its own.
pub const CONDITION_BASE: u8 = 60;

/// Condition added per rung of `Rarity::rank` — deliberately modest next to
/// `CONDITION_BASE`. A rare kill already pays through its own loot table
/// entry; this is a secondary nudge, not a second place rarity is priced.
pub const CONDITION_PER_RARITY_STEP: u8 = 8;

/// Flat condition bonus when `DownedProgram::boss` is set. A boss fight
/// ends standing over a single, stationary target rather than whichever
/// member of a pack took the killing blow, so the program it leaves behind
/// is taken apart more cleanly on average.
pub const CONDITION_BOSS_BONUS: u8 = 10;

/// Weight on the "how clean was the kill" term of the condition roll —
/// `overkill_term` in the spec's formula, how far the killing blow went
/// past zero as a fraction of `max_hp`, negated. **Ships at `0.0`,
/// deliberately** (spec, "Open, deliberately"): the field and the roll
/// exist so the axis is tunable, but no non-zero value has a play session
/// behind it yet. Do not delete this constant for reading as unused, and
/// do not fit a non-zero value without one.
pub const FIGHT_CONDITION_WEIGHT: f32 = 0.0;

/// The floor a boss's own downed program's `condition` cannot fall below —
/// applied on top of the ordinary roll, in `Game::downed_program_for`. A
/// boss fight ends standing over a single, stationary target, so what it
/// leaves behind is never the ragged result an unlucky pack kill can be.
pub const BOSS_CONDITION_FLOOR: u8 = 80;

/// The floor a boss's own downed program's `rarity` cannot fall below —
/// `Rarity::max` against the ordinary roll, the same way
/// `BOSS_CONDITION_FLOOR` floors condition. Matches
/// `SURFACE_BOSS_LOOT_RARITY_FLOOR` (`Silver`) rather than outbidding it:
/// the intent is parity with what a boss already guarantees on the gear it
/// pays directly, not a richer guarantee on what it leaves behind.
pub const BOSS_RARITY_FLOOR: Rarity = Rarity::Silver;

/// How much each rung of `Rarity::rank` adds to `DownedProgram::grade`'s
/// multiplier, on top of the `1.0` an `Ordinary` program contributes.
/// Modest for the reason `CONDITION_PER_RARITY_STEP` is: rarity already
/// pays out through the drop table, so grade's job is to reward condition
/// and level, with rarity as a secondary lift rather than the dominant
/// term.
pub const GRADE_PER_RARITY_RUNG: f32 = 0.15;

/// How much each level adds to `DownedProgram::grade`'s multiplier, on top
/// of the `1.0` a level-0 program would contribute. Small on purpose: a
/// level runs from 1 into the hundreds over a playthrough (see
/// `ZONE_LEVEL_CAP_STEP`), and this axis is meant to reward a kill higher
/// up the curve without letting level alone dwarf condition and rarity the
/// way a coarser step would.
pub const GRADE_PER_LEVEL: f32 = 0.01;

/// The tool forged into the player's first tool slot, written by
/// `spawn_player` into `components::Tools` on `DECOMPILE_ABILITY_ID`'s
/// terms: granted at creation only, never at `Game::load`, the profile
/// rule. Lives here rather than beside `abilities::DECOMPILE_ABILITY_ID`
/// because the spec names it `tuning::STARTER_TOOL_ID` directly (section 2).
pub const STARTER_TOOL_ID: &str = "salvage_clamp";

/// Most tools the player can hold installed at once — a silent backstop,
/// never shown on screen. `Game::install_tool` already allows one tool per
/// `ToolCategory` (a same-category install replaces rather than adds), so
/// with six categories this cannot bind today; it bounds the slot list
/// against a future category or a hand-edited save instead.
pub const MAX_INSTALLED_TOOLS: usize = 10;

/// How many rows `Game::tool_rows` may return — `MAX_DOWNED_PROGRAMS`'s own
/// reason: `Mode::Tools` is a read-only row list with no scroll, so this
/// doubles as a layout constraint that must fit at 1280x720, asserted by
/// `the_tallest_tools_list_fits_its_popup_at_1280x720` and verified by
/// mutation. Installed tools are bounded by one per `ToolCategory`; a modded
/// research tree can teach arbitrarily many tools with nothing forged yet,
/// so `tool_rows` needs its own ceiling — headroom past the six categories
/// without reopening the screen to an unbounded catalogue.
pub const MAX_TOOL_ROWS: usize = 8;

/// How many attribute rows `Game::dossier_report` may return —
/// `MAX_TOOL_ROWS`' own reason: `Mode::Dossier` is a read-only row list
/// with no scroll, so this doubles as a layout constraint, asserted by
/// `the_tallest_dossier_fits_its_popup` and verified by mutation. The
/// catalogue is trimmed against this **before** the page builds its rows,
/// so a modded twentieth attribute costs the last rows rather than pushing
/// the header off the bottom in silence. Twice the shipped five, headroom
/// for a wider catalogue without reopening the page to an unbounded one.
pub const MAX_ATTRIBUTE_ROWS: usize = 10;

/// `Game::extraction_yield`'s base unit count — the multiplier scaled by
/// `game::extraction::tier_scale(tool.tier)` and `DownedProgram::grade()`
/// (identity `1.0` at `Ordinary`, full condition, level 0). **Checked**
/// against spec decision 8, not fitted — the provisional value already
/// satisfied the gate, so it never moved: `tests::extraction::
/// the_starter_tool_is_drop_neutral_for_a_median_kill` asserts a median kill
/// (`Ordinary`, `CONDITION_BASE` condition, level 1 — grade `0.606`) through
/// the starter tool (`salvage_clamp`, tier 1, `tier_scale == 1.0`) pays
/// exactly `WORK_RESOURCE_DROP`'s own mean, `3` units: `round(3.0 * 0.606) =
/// 2` pool units plus `RICH_IN_UNITS`'s flat `1`.
///
/// That test only pins this constant to a *band*, not the digit `3.0`:
/// `round(x * 0.606)` stays `2` for any `x` in `[1.5 / 0.606, 2.5 / 0.606)`,
/// currently ≈ `[2.475, 4.125)` (verified empirically, not just algebraically
/// — the test passes unmoved at `2.5` and `4.1`, fails at `2.4` and `4.2`).
/// Every value in that band pays a median kill the identical `3` units and
/// so reads as equally drop-neutral to this gate — a starter tool at either
/// end feels roughly half or a third stronger than one at the other, and
/// nothing here would catch that; only a play session or a wider-coverage
/// test could. Say the band plainly rather than let one passing value read
/// as pinned to the digit. `salvage_clamp.ron`'s weights don't enter the
/// gate at all — `apportion` conserves the unit *total* under any
/// weighting, and `rich_in`'s addend is unconditional — so they're
/// untouched, and no combination of them could narrow this band either.
pub const TOOL_BASE_UNITS: f32 = 3.0;

/// How far either side of its centre `Game::extraction_band` reaches, as a
/// fraction of the centre, rounded to whole units — a median kill's `2`
/// pool units roll `1..=3`. Symmetric so the mean stays the deterministic
/// figure `TOOL_BASE_UNITS` was checked against; the swing is the feature,
/// and the preview names items without counts because of it.
pub const EXTRACT_UNIT_SPREAD: f32 = 0.5;

/// How much each tier past 1 scales `extraction_yield`'s unit count, in
/// `game::extraction::tier_scale`. A tool's own `tier` and a later phase's
/// structure `structure_tier` (spec section 3) are meant to share this one
/// curve, so a step here would move both once that parameter exists.
///
/// **Not** part of Task 6's fit: the starter tool ships at tier 1, where
/// `tier_scale(1) == 1.0` regardless of this constant's value, so the
/// drop-neutrality test cannot see it move and it is free to set here.
pub const TOOL_TIER_SCALE_STEP: f32 = 0.5;

// How a standing extraction bench's tier enters `Game::extraction_yield`:
// as `tier - 1` steps of `TOOL_TIER_SCALE_STEP`, not as `tier`.
//
// There is no constant here on purpose — this doc is the constant. A
// freshly built bench reads as tier 1 (`Game::best_structure_tier`: a
// structure with no `StructureTier` and one never upgraded are the same
// thing to a player), so a `tier` term would pay `TOOL_TIER_SCALE_STEP`'s
// full step for a structure most bases already have, which is +50% of the
// whole material economy on the shipped `0.5`. `tier - 1` makes the
// *upgrade* the thing that sells yield — `best_structure_tier`'s own
// documented rule for the craft quality floor — and leaves phase 1's
// drop-neutrality gate (fitted with no bench standing at all) unmoved.
// The speed half, `EXTRACT_BENCH_TICK_STEP`, uses the full tier instead:
// owning the bench is worth something, it is just worth time rather than
// materials.

/// How much each tier of a standing extraction bench divides an
/// extraction's tick cost: `ticks / (1 + EXTRACT_BENCH_TICK_STEP * tier)`,
/// floored at one tick. Uses the bench's **full** tier, unlike the yield
/// term (which uses `tier - 1`) — standing the bench at all is worth time,
/// and upgrading it is worth materials. A quotient rather than a
/// subtraction so no tier can reach zero, and a floor of one so an
/// extraction is never a free action: `Game::extract_program` ticking
/// nothing would make the store a place to stand and think in, which is
/// what every other spend in the game refuses to be.
///
/// A guess, like every other number this feature ships: at `0.25` a fresh
/// bench pays `0.8x` and a tier-5 bench `0.44x`. Nothing in the repo can
/// check it — `balance_sim` models no loot and no time cost.
pub const EXTRACT_BENCH_TICK_STEP: f32 = 0.25;

/// Relative weight on the *first* candidate in `Game::routine_candidates`
/// when a `Routines` tool draws — every other candidate weighs 1. The
/// species' own declaration order is the ranking, so the earliest routine it
/// has never taught the player is the likely outcome without being the
/// certain one. At `3` against a two-candidate pool the first wins three
/// draws in four.
///
/// A guess. Deterministic (weight infinity) would make a `Routines` tool a
/// lookup table the player memorises; uniform would make the kit's own
/// ordering mean nothing.
pub const ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT: u32 = 3;

/// Extra units of a program's `rich_in` item (or its `work_resource`
/// fallback, `Game::rich_in`) that `extraction_yield` adds on top of the
/// tool's own draw — from any tool, regardless of category (spec section
/// 3). A flat floor tied to *what was killed* rather than a second roll of
/// the tool's own pool, so it stays modest next to `TOOL_BASE_UNITS`.
pub const RICH_IN_UNITS: u32 = 1;

/// What one carrier of any tool is worth in trade currency —
/// `ItemDb::synthesise_tool_carriers`'s flat price. Set below every
/// shipped tool's own `forge_cost` (cheapest today is `salvage_clamp`'s 3
/// `core_fragment`, worth 3), so forging a carrier and immediately selling
/// it back is a loss rather than a Credit loop — the carrier is buyable
/// nowhere (`game::caravan`'s `stock_pool` excludes a tool id), so this only
/// prices what selling a spare one back nets.
pub const TOOL_CARRIER_VALUE: u32 = 2;
