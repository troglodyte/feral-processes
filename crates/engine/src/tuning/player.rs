use super::*;

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
/// **The pool size is the all-Analysis offense ceiling**, because a point
/// is priced 1-for-1 (`CREATION_COST_PER_ATTRIBUTE_POINT`). At 20 the
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

/// Pool points one point of any attribute costs at creation - priced
/// 1-for-1, so the pool is a count of attribute points. What a point *buys*
/// is each attribute's own `effects`, and `docs/measurements/2026-09-01-
/// creation-stat-pool-exchange-rates.md` is the record of how the four old
/// axes compared when they were priced separately.
pub const CREATION_COST_PER_ATTRIBUTE_POINT: u32 = 1;

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
/// the two `ABILITY_*_SCALE_PER_LEVEL` rates)
/// and the reciprocal of every constant denominated in *levels per*
/// something (`ROUTINE_SLOT_PER_LEVEL`, `TALENT_START_LEVEL`,
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

/// Attribute points banked per player level-up, to spend on the Points
/// screen. See `progression::canonical_spend` for the split that
/// reproduces the old automatic growth.
pub const STAT_POINTS_PER_LEVEL: u32 = 6;

/// The canonical spend per level: 4 Parity (6 max HP each) and 2 Analysis
/// (1 Atk and 1 Decompiler each). Equals `HP_PER_LEVEL`, `ATK_PER_LEVEL`
/// and the retired automatic Decompiler grant (2 a level) exactly. The
/// per-point values live in `assets/attributes/`, so that equality is held
/// by `progression::canonical_spend_is_the_old_per_level_growth` against
/// the real assets, not here; only the pool size is a build failure.
pub const CANONICAL_PARITY_PER_LEVEL: u32 = 4;
pub const CANONICAL_ANALYSIS_PER_LEVEL: u32 = 2;
const _: () =
    assert!(CANONICAL_PARITY_PER_LEVEL + CANONICAL_ANALYSIS_PER_LEVEL == STAT_POINTS_PER_LEVEL);

/// The floor on a derived max Power, so a low Bandwidth cannot leave a
/// reserve too small to cast anything.
pub const MIN_MAX_POWER: f32 = 20.0;

/// The range a derived status resist is clamped to, in percent. Negative
/// lengthens a status.
pub const STATUS_RESIST_MIN: i32 = -50;
pub const STATUS_RESIST_MAX: i32 = 75;

/// Ceiling on the player's derived Extraction term in
/// `systems::mining_success_chance`, so Analysis never out-mines the
/// sharpest species.
pub const MINING_EXTRACTION_CAP: f32 = 0.10;

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
/// **Chosen, one under the fit.** 11 is the smallest integer slope that
/// keeps `balance_sim::min_level_to_clear_zone`'s *geared* requirement
/// reachable at every zone out to 16 — a cap below that requirement is not
/// difficulty, it is a run that cannot continue. 10 was picked over it on
/// 2026-10-10 knowing it leaves zone 12 two levels short (needs 113, capped
/// at 111) and zones 11 and 13 with one and zero to spare; the sim models no
/// abilities, so that is a floor projection, not a proof. The test that
/// guards the bound names zone 12 as its one accepted exception.
///
/// The cap still sits *above* the gear-free requirement in zones 2-5, so
/// those can be cleared by levelling alone. The two clear curves both pass
/// near the origin and then diverge, so no single line can sit inside the
/// band at both ends of the range. See
/// `docs/measurements/2026-08-27-zone-level-cap.md`.
pub const ZONE_LEVEL_CAP_STEP: u32 = 10;

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
/// `perks_held` is `BoughtStats::ever_bought` — how many perk levels have
/// **ever** been bought, not how many are held now. Read off
/// `Perks::unlocked` it would reset to zero on a respec, and the loop would
/// be: buy perks, wipe them, mint the rest at the opening rate. The two are
/// equal for any run that never respecs, so this curve has not moved.
pub const OVERFLOW_XP_STEP: u32 = 120;

/// What one respec costs in Credits — `Game::respec_perks` wipes every perk
/// level, `Game::respec_talents` every talent on one companion, and both
/// charge this.
///
/// Flat, and the same number on both ledgers, by decision. A price that
/// scales with what is being undone reads as a tax on experimenting, and an
/// escalating one needs a stored counter per subject for no gain the player
/// can see. Priced in Credits because they are the one currency that
/// survives a breach, so a respec competes with building and gear rather
/// than with the Stack.
pub const RESPEC_CREDIT_COST: u32 = 250;

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

/// The **floor** of `zone`'s level band — the level a party arrives in it
/// at, where `zone_level_cap` is the level they leave it at.
///
/// The pair is what `Game::party_band_progress` measures across, and zone 1
/// is the case that needs saying: `zone_level_cap(0)` answers
/// `ZONE_LEVEL_CAP_FLOOR` rather than 1, because the cap's own floor clamps
/// it — so reusing it here would open a brand-new run at *full* band
/// progress and hand a level-1 player a zone step of extra difficulty on
/// their first fight. Level 1 is the literal because a run starts there,
/// not because the formula degenerates to it.
pub const fn zone_band_floor(zone: u32) -> u32 {
    if zone <= 1 {
        1
    } else {
        zone_level_cap(zone - 1)
    }
}

/// How many zone steps a **full** band of levelling is worth to the ground's
/// stats — `Game::party_stat_steps`, scaled by
/// `Game::party_band_progress`.
///
/// A fractional zone step, `ENEMY_STRENGTH_BAND_STEP`'s rule: the party term
/// is the fifth thing that buys a step, after zone, distance, depth and the
/// enemy-strength band, and it adds at the same seam rather than compounding
/// on top of one. Expressed that way it means one thing at every zone — "the
/// ground fights like it is N zones deeper" — and `balance_sim` gates it for
/// free, since zone N one step up **is** the zone N+1 fixture the sweeps
/// already cover.
///
/// **At 1.0 or below that free gating holds and above it does not**, which
/// is what `the_party_stat_term_stays_inside_balance_sims_reach` refuses.
/// The other bound is not here at all: the term is scaled by band progress
/// and `Game::level_cap` is the only ceiling on a level, so the party's own
/// cap is what stops this running away. There is deliberately no clamp of
/// its own to find and "simplify" out.
pub const PARTY_LEVEL_STAT_STEPS: f32 = 1.0;

/// How many **fielded companions** buy one escalation step on the two group
/// curves — `Game::party_count_steps`.
///
/// The spawner was blind to party size until this landed: a solo player and
/// a five-strong party met the same pack. `MAX_PARTY_SIZE` is 5, so this
/// tops out at two steps.
pub const PARTY_SIZE_STEP_MEMBERS: u32 = 2;

/// How many escalation steps a **full** band of levelling buys on the two
/// group curves, floored to a whole step — the count half of what
/// `PARTY_LEVEL_STAT_STEPS` does to stats.
///
/// Separate from the stat term rather than shared with it, because the two
/// axes reach different code: stats fold in caller-side at
/// `Game::field_stat_mult` and `Game::stack_depth_multiplier`, while count
/// folds in at `Game::group_steps` — which must stay clear of
/// `Game::danger_steps`, whose third reader is the species danger-band
/// window.
pub const PARTY_LEVEL_COUNT_STEPS: u32 = 1;

/// Fraction of in-level XP knocked back by a "setback" penalty (a flatline,
/// a Forgiving-mode reboot, or a forced jack-out mid-battle) — see
/// `progression::apply_setback_xp_penalty`. Deliberately mild: it erodes
/// progress toward the next level, never the level or stats themselves.
pub const SETBACK_XP_PENALTY_FRACTION: f64 = 0.2;

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
/// `progression::kill_xp`. The scale is `battle::threat_ratio` (the formula
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
