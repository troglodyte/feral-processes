use super::*;

// ─────────────────────────────────────────────────────────────────────────
// Achievement profile ceilings
// ─────────────────────────────────────────────────────────────────────────

/// Most stat points a fully-cleared `assets/achievements/` ladder may
/// hand a new run to bank, summed across every `Reward::RandomMainStat`.
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
// Run score
// ---------------------------------------------------------------------------

/// Points per level of every foe defeated. A kill is worth the foe's level,
/// so a deep or strong fight outscores farming the surface.
pub const SCORE_PER_FOE_LEVEL: u64 = 10;
/// Points per boss defeated, on top of the boss's own level.
pub const SCORE_PER_BOSS: u64 = 500;
/// Points per Stack depth reached (1..frames within one Stack).
pub const SCORE_PER_DEPTH: u64 = 100;
/// Points per Phase Key held.
pub const SCORE_PER_KEY: u64 = 1000;
/// Points for escaping the Basin.
pub const SCORE_ESCAPE: u64 = 5000;
/// Points per structure standing. Small on purpose: progression is earned by
/// fighting, and `score::tests` pins that a fighter outscores a builder.
pub const SCORE_PER_STRUCTURE: u64 = 20;
/// Points per program ever decompiled this run.
pub const SCORE_PER_PROGRAM: u64 = 50;
/// Points per achievement earned this run.
pub const SCORE_PER_ACHIEVEMENT: u64 = 100;
/// Multiplier under `DifficultyMode::Permadeath`.
pub const SCORE_PERMADEATH_MULT: f32 = 1.5;
/// Multiplier added for each `EnemyStrength` band above `Standard`.
pub const SCORE_PER_BAND: f32 = 0.25;

/// Lifetime score per bonus stat point at character creation.
pub const SCORE_PER_BONUS_STAT_POINT: u64 = 2000;
/// Most bonus stat points the lifetime score may buy.
pub const BONUS_STAT_POINT_CAP: u32 = 5;
/// Lifetime score per bonus perk point at character creation.
pub const SCORE_PER_BONUS_PERK_POINT: u64 = 5000;
/// Most bonus perk points the lifetime score may buy.
pub const BONUS_PERK_POINT_CAP: u32 = 3;
/// Lifetime score per bonus creation credit.
pub const SCORE_PER_BONUS_CREDIT: u64 = 200;
/// Most bonus creation credits the lifetime score may buy.
pub const BONUS_CREDIT_CAP: u32 = 20;

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

/// Ticks between one nemesis recruit and the next, while its band is short.
///
/// **Authored in minutes of play and written here in ticks**, the way
/// `SIEGE_PRESSURE_THRESHOLD` is: the world runs at
/// `app_core::WORLD_SPEED_MULTIPLIER` = 2 ticks a second, so `600` is **5
/// minutes** per recruit. Three recruits plus `NEMESIS_MARCH_DELAY` is about
/// 23 minutes from a nemesis's last fight to its march. An unmeasured number.
pub const NEMESIS_RECRUIT_INTERVAL: u32 = 600;

/// How far, in cells (Chebyshev), a nemesis looks for an existing wild
/// program to recruit before it spawns a fresh body beside itself.
pub const NEMESIS_RECRUIT_RADIUS: i32 = 12;

/// Followers a nemesis gathers before its band is full: a siege of
/// `NEMESIS_BAND_MAX + 1` bodies, against 6 to 12 for a regular one.
pub const NEMESIS_BAND_MAX: usize = 3;

/// Ticks from a full band to its march: `SIEGE_WARN_FLOOR_TICKS`, **8
/// minutes** of play, so the player gets the same minimum notice a regular
/// siege gives.
pub const NEMESIS_MARCH_DELAY: u32 = SIEGE_WARN_FLOOR_TICKS;

/// How far a follower may drift from its leader, in cells (Chebyshev),
/// before it steps back toward it.
pub const NEMESIS_FOLLOW_DISTANCE: i32 = 2;

// ─────────────────────────────────────────────────────────────────────────
// Research: discovery by study
// ─────────────────────────────────────────────────────────────────────────
//
// See `Game::settle_study`. A Research Station with no project selected
// banks its payout into study attempts instead of landing it nowhere; each
// full bar with a subject pinned is one roll over the eligible undiscovered
// pool.
//
// **Priced against the Station, not fitted against `balance_sim`**, which
// models no base production at all. The arithmetic, at the worst rung the
// feature is ever played at — a tier-1 Station in zone 1, staffed by a
// program at the baseline aptitude with no memories:
//
//   cycle length   `research_node.ron`'s `ticks_per_unit: 14`, unscaled by
//                  tier (`systems::work_ticks_at_speed` scales on speed and
//                  build quality, not tier)
//   yield rate     `systems::mining_success_chance(level 1, …)` = 0.5
//   payout         `systems::node_payout(tier 1, zone 1)` = 1
//   ⇒ ~1 Research Data per 28 ticks, and the world runs at
//     `app_core::WORLD_SPEED_MULTIPLIER` = 2 ticks a second: **14s per unit**
//
// So one attempt is ~56 s of an idle Station, and a discovery is ~2.7
// minutes of them — "a handful of minutes at the keyboard, not an hour." By
// mid-run (tier 3, zone 2) the payout is 4 a cycle and the same discovery is
// ~40 s, which is the right shape: the Station you invested in uncovers the
// tree faster.
//
// The second anchor is the tree's own prices. `automation` costs 8 Research
// Data and `armor_bench` 24, so an attempt is half the cheapest node and a
// sixth of a bench — **finding a node is cheaper than researching it**, or
// discovery would read as a second research cost rather than as a prelude.
//
// **This is the one number in the feature that only play can confirm**, and
// nothing here can be played by an agent. What the above is blind to: how
// often a player actually leaves a Station idle, and whether twenty
// discoveries spread across ten sectors feels like uncovering a tree or like
// waiting for one.

/// Research Data banked per study attempt — `ActiveResearch::study`'s cap,
/// and the bar `Game::settle_study` spends. See the section note above.
pub const STUDY_ATTEMPT_DATA: u32 = 4;

/// The roll a spent study attempt makes. A miss logs and costs the attempt;
/// there is no pity counter, because the bar itself is already the pacing.
/// See the section note above.
pub const STUDY_DISCOVERY_CHANCE: f64 = 0.35;

/// The fraction of a program's `max_hp` a Power Siphon takes when it lets go,
/// however it lets go (manual release, deconstruction, a raid, a Home
/// demolished). The price of the power is the program, so the hold is never
/// free to leave; it never kills, because `release_siphoned` floors the
/// result at 1 Integrity.
pub const SIPHON_RELEASE_INTEGRITY_LOSS: f32 = 0.75;
const _: () = assert!(SIPHON_RELEASE_INTEGRITY_LOSS > 0.0 && SIPHON_RELEASE_INTEGRITY_LOSS < 1.0);

/// Ticks between strikes of the `siphoned` memory on a held program
/// (`Game::note_siphoned`). A stretch memory like `MEMORY_POSTING_PERIOD`'s,
/// but four times slower: the grudge is meant to build over a long hold, and
/// at the strike cap it takes nine of these (9000 ticks) to arrive. At the
/// posting period a hold would be a tantrum within one afternoon.
pub const SIPHON_GRUDGE_PERIOD: u64 = 4 * MEMORY_POSTING_PERIOD;

// ─────────────────────────────────────────────────────────────────────────
// Player emulation
// ─────────────────────────────────────────────────────────────────────────
//
// See `progression::emulated_stats` (todo #100). An emulation grows the
// image's species to the player's level and then multiplies attack and
// mitigation by the same figure, so a wild program of the same species and
// level is always the weaker of the two.

/// The flat multiplier `progression::emulated_stats` applies to both attack
/// and mitigation, on top of the species' own growth. Kept above 1.0 so an
/// emulation always beats a wild program of the same species at the same
/// level — the strength the feature exists to sell. `balance_sim` models no
/// abilities, so this was fitted against the arena instead (todo #100 Task
/// 7): kept at its guessed value, `docs/measurements/
/// 2026-09-18-emulation-edge.md`. Against a coinflip fight an ordinary
/// species roughly doubles the win rate at 1.25 and barely more than a
/// tenth of that at 1.05 — the constant this task tunes is doing real work
/// for the ordinary roster. The strongest species (an apex boss) already
/// outclasses the player's own kit at 1.25 and is nearly as strong at 1.05,
/// because a boss's `growth_multiplier` (2.0) alone doubles the player's own
/// fixed growth rate before this multiplier is ever applied — a species-level
/// gap this one global constant cannot close either way, recorded as an open
/// risk rather than something a lower number here would fix.
pub const EMULATION_EDGE: f32 = 1.25;

/// Added to `EMULATION_EDGE` per level of `Perk::EmulationFidelity`. Only
/// ever pushes every species further in the same direction, so it cannot
/// narrow the boss-species gap `EMULATION_EDGE`'s own doc records — left at
/// its guessed value, untested by the 2026-09-18 measurement for that
/// reason.
pub const EMULATION_EDGE_PER_PERK_LEVEL: f32 = 0.1;

// ---------------------------------------------------------------------------
// Traps
// ---------------------------------------------------------------------------

/// How many traps may stand in a zone at once.
///
/// Hidden from the player and not something to be played around: it bounds
/// the per-tick cost of `Game::run_traps` and the carpet a player could
/// otherwise lay across every tile they walk. A run that reaches it is a run
/// that has stopped playing the feature and started farming it.
pub const TRAP_PLACEMENT_CAP: usize = 50;

/// Ticks between one trap's capture attempts — four minutes at the world's
/// two ticks a second.
///
/// A countdown rather than a per-tick roll, and that is what keeps fifty
/// traps off the seeded RNG stream: a trap whose period has not elapsed
/// spends no `GameRng` draw at all.
pub const TRAP_PERIOD_TICKS: u32 = 480;

/// The chance one elapsed period catches anything — about one catch every
/// twenty-seven minutes per trap, which is the figure that decides whether
/// walking back to a trap is worth the trip.
pub const TRAP_CAPTURE_CHANCE: f64 = 0.15;

/// Points off a caught program's condition, after `roll_condition`, on a
/// 0..=100 scale.
///
/// A trap is meant to be *worse* than beating the program yourself: this,
/// the authored rarity ceiling and the caught program never carrying a
/// routine are the three things that hold that, and none of them is a gate
/// on the feature working.
pub const TRAP_CONDITION_PENALTY: u8 = 15;

// ---------------------------------------------------------------------------
// Alert board
// ---------------------------------------------------------------------------

/// How many rows the alert board holds before the oldest is dropped — see
/// `alerts::post`.
pub const ALERT_BOARD_CAP: usize = 50;
