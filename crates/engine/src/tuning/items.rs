use super::*;

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

/// Fights an equipped piece of gear survives before it breaks, unless its
/// def authors `durability`. A broken copy stays worn and inert until a
/// Refit Bench restores it.
pub const GEAR_DURABILITY: u8 = 15;

/// Annealed Cores one refit takes from the bench's stock, whatever the copy
/// is: one material and one unit, so the bill never needs explaining.
pub const REFIT_CORES: u32 = 1;

/// Ticks the player must have stood on one base cell before a Refit Bench
/// starts mending: a walk past the bench spends nothing, so cores go to a
/// player who has stopped to be served.
pub const REFIT_IDLE_TICKS: u32 = 2;

/// Chebyshev distance, in base tiles, within which a standing player is
/// served by a Refit Bench.
pub const REFIT_REACH: i32 = 2;

/// A worn copy raises a `GearWorn` alert when its remaining fights fall
/// below this percentage of its durability.
pub const GEAR_WORN_ALERT_PERCENT: u32 = 20;

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

/// What hand-compiling costs, as a multiple of the cycle the machine that
/// exists to do the job would spend on one unit — see
/// `Game::hand_craft_ticks`, which is the one place the two are multiplied.
///
/// The base is a factory nobody has to visit while a recipe the Factory runs
/// in twelve ticks is also a free instant action at the player's own hands.
/// Blocking the recipe outright was the alternative and was rejected: every
/// recipe stays reachable, and what is priced is the *convenience*.
///
/// **One, because the wall clock is the other half of the price.** This was
/// ten, chosen against a compile bar that ran at thirty times the world's
/// tick rate — a Hardened Shell was three hundred game ticks and five real
/// seconds. `app_core::COMPILE_TICKS_PER_SECOND` is the world's own rate
/// now, so the ten-times price is ten times the *wait* as well: that same
/// shell is two and a half minutes of watching a bar. The two constants are
/// read together and this is the one that gave.
///
/// **What still makes the machine the obvious answer is not speed.** At
/// parity a bench beats hands on the three things the multiplier was never
/// carrying: it runs unattended while the player does something else, it
/// burns no Power out of the player's own reserve, and `QUALITY_BENCH_PER_
/// TIER` puts every copy off a developed bench above what bare hands can
/// reach. What the player buys by compiling by hand is reaching a recipe
/// before the base that makes it exists, and they pay for it in real time
/// and Power.
pub const HAND_CRAFT_TICK_MULT: u32 = 1;

/// The cycle `Game::hand_craft_ticks` prices a recipe off when **no**
/// structure produces it.
///
/// Most craftables — every piece of gear among them — have no machine at
/// all, so this is the common case rather than the fallback it reads as. It
/// sits at the shipped assemblers' median cycle so an item nobody automates
/// costs about what an automated one of middling complexity does.
pub const HAND_CRAFT_DEFAULT_CYCLE: u32 = 10;

/// The Power a hand-compile has to leave standing in the player's reserve,
/// or the whole batch is refused in `Game::begin_hand_craft` rather than
/// silently shortened — `MAX_ACTIVE_CONTRACTS` states the no-silent-caps
/// rule this follows.
///
/// Compiling by hand is work, and its ticks burn `HUNGER_DECAY_PER_TICK`
/// exactly as a tick spent anywhere else does. **That drain is the feature
/// and stays**; what is refused is only the batch that can be seen in
/// advance to end the run.
///
/// **What reaches it is the batch, not the unit.** At `HAND_CRAFT_TICK_MULT`
/// a single Hardened Shell is thirty ticks and four and a half points of a
/// hundred-point reserve, so no one item comes near this — a run of them
/// does, and `CRAFT_QUANTITY_MAX` lets the player ask for four figures of
/// one. That is what this guards, and it is why the refusal is priced off
/// `quantity × hand_craft_ticks` rather than off a per-item ceiling.
///
/// **A margin rather than exactly zero**, because a floor at `POWER_MIN`
/// would still pass a batch projected to finish at a fraction of a point:
/// the very next background tick starves it, which is the state this
/// refusal exists to prevent, reached one tick later. Ten points is about
/// sixty-six ticks at the standing drain — a crossing of base space to a
/// rest, which is free in there and refills the reserve whole, so the
/// refusal's advice is actionable rather than a dead end.
pub const HAND_CRAFT_POWER_FLOOR: f32 = 10.0;

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

/// How many downed programs of its own species a destroyed nest leaves
/// (`Game::grant_nest_cache`), replacing the flat `work_resource` roll that
/// used to pay here (`NEST_CACHE_WORK_RESOURCE_MULT`, retired with
/// `roll_work_resource_drop`). Set to match that multiplier's own reading —
/// 4x an ordinary kill's payout — rather than an unargued fraction of it:
/// `Game::leave_downed_program` leaves exactly one program per kill, so 4
/// preserves "the cache reads as several kills' worth at once" instead of
/// silently halving what a nest was worth.
pub const NEST_CACHE_PROGRAM_COUNT: usize = 4;

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
