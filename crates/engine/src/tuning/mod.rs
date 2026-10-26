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
//!   each directory's `README.md`; a second copy here would drift.
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

mod base;
mod combat;
mod damage;
mod economy;
mod encounters;
mod gear_craft;
mod items;
mod meta;
mod perks;
mod player;
mod production;
mod routine_craft;
mod settlements;
mod sorties;
mod stack;
mod staff;
mod tactical;

pub use base::*;
pub use combat::*;
pub use damage::*;
pub use economy::*;
pub use encounters::*;
pub use gear_craft::*;
pub use items::*;
pub use meta::*;
pub use perks::*;
pub use player::*;
pub use production::*;
pub use routine_craft::*;
pub use settlements::*;
pub use sorties::*;
pub use stack::*;
pub use staff::*;
pub use tactical::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ZoneLevel;

    /// The reference wearer must stay a *derivation* of the two axes in
    /// `player.rs`, not three hand-written numbers that drift the first time
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

    /// The party term is gated by `balance_sim` **for free** only while a
    /// full band of levelling is worth at most one zone step: zone N one
    /// step up is arithmetically the zone N+1 fixture the sweeps already
    /// cover, which is `ENEMY_STRENGTH_BAND_STEP`'s and the distance ramp's
    /// argument both. Above one step the far field of zone N is a fixture
    /// nothing sweeps, and this needs a bound of its own before it may move.
    #[test]
    fn the_party_stat_term_stays_inside_balance_sims_reach() {
        assert!(
            PARTY_LEVEL_STAT_STEPS <= ZONE_STAT_STEP as f32,
            "PARTY_LEVEL_STAT_STEPS is {PARTY_LEVEL_STAT_STEPS}, past one zone step — \
             give it a balance_sim bound of its own before raising it"
        );
    }

    /// `zone_band_floor` must be the previous zone's ceiling, and zone 1's
    /// must be level 1. Reusing `zone_level_cap(zone - 1)` at zone 1 answers
    /// `ZONE_LEVEL_CAP_FLOOR` — the cap clamps its own line — which would
    /// open a brand-new run at full band progress.
    #[test]
    fn a_zones_level_band_starts_where_the_last_one_ended() {
        assert_eq!(zone_band_floor(1), 1, "a run starts at level 1");
        for zone in 2..=16u32 {
            assert_eq!(
                zone_band_floor(zone),
                zone_level_cap(zone - 1),
                "zone {zone}'s band must open where zone {} closed",
                zone - 1
            );
            assert!(
                zone_level_cap(zone) > zone_band_floor(zone),
                "zone {zone}'s band must have width, or progress is undefined"
            );
        }
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

    /// `a-gated-consequence-can-be-green-and-unreachable`'s rule: a top tier
    /// whose crew requirement exceeds the cap on how many programs may ever
    /// be posted would be a tier 3 nothing can reach, which is green right
    /// up until someone tries to staff it.
    #[test]
    fn the_top_outposts_tier_is_reachable_under_the_crew_cap() {
        assert!(
            *OUTPOST_TIER_CREW.last().unwrap() <= OUTPOST_CREW_CAP,
            "OUTPOST_TIER_CREW's top entry ({}) exceeds OUTPOST_CREW_CAP ({OUTPOST_CREW_CAP})",
            OUTPOST_TIER_CREW.last().unwrap(),
        );
    }
}
