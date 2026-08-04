//! The one place that decides what has been earned.
//!
//! Three of the four triggers are high-water marks on a monotone counter, so
//! this system polls them and needs no call sites at all — hooking
//! `enter_next_zone` and `enter_frame` separately would have created another
//! of this repo's two-paths-that-must-agree pairs. The fourth is event-shaped:
//! `Game::award_loot` *records* a boss kill into `RunFeats` and this system
//! still decides what that earned, so the kill site cannot drift from the
//! rules.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::achievements::{AchievementDb, Earned, Profile, Reward, Trigger, roll_main_stat};
use crate::resources::{
    DifficultyMode, GameClock, Locale, MessageKind, MessageLog, PendingProfileWrites, RunFeats,
    ZoneLevel,
};

/// Where the run currently stands, as far as a threshold is concerned.
/// Bundled so bevy's one-param-per-resource injection doesn't push the system
/// past clippy's argument-count threshold — and the grouping is real: these
/// are the four immutable readings every trigger is evaluated against, and
/// the system writes none of them.
#[derive(SystemParam)]
pub struct RunStanding<'w> {
    clock: Res<'w, GameClock>,
    zone: Res<'w, ZoneLevel>,
    /// Read instead of `Position`, always: `Position` is pinned to the
    /// surface entrance tile while the party is underground, so a depth taken
    /// from it would be a surface coordinate. That is the trap
    /// `nest_aggro_tick` needs its guard for.
    locale: Res<'w, Locale>,
    difficulty: Res<'w, DifficultyMode>,
}

/// Evaluates every authored rung against the run's current state, records
/// what is newly earned into the `Profile`, and queues it for app-core to
/// write to disk.
///
pub fn achievement_system(
    db: Res<AchievementDb>,
    mut profile: ResMut<Profile>,
    mut feats: ResMut<RunFeats>,
    standing: RunStanding,
    mut pending: ResMut<PendingProfileWrites>,
    mut log: ResMut<MessageLog>,
) {
    let RunStanding {
        clock,
        zone,
        locale,
        difficulty,
    } = standing;
    let permadeath = matches!(*difficulty, DifficultyMode::Permadeath);
    for def in db.iter() {
        if profile.contains(&def.id) {
            continue;
        }
        let met = match &def.trigger {
            Trigger::ZoneReached(n) => zone.0 >= *n,
            Trigger::StackDepthReached(n) => {
                matches!(*locale, Locale::Stack { depth, .. } if depth >= *n)
            }
            Trigger::CyclesSurvived(n) => clock.tick >= *n,
            Trigger::BossDefeated(None) => !feats.bosses_defeated.is_empty(),
            Trigger::BossDefeated(Some(species)) => feats.bosses_defeated.contains(species),
        };
        if !met {
            continue;
        }

        let rolled_stat = match def.reward {
            Reward::RandomMainStat(_) => Some(roll_main_stat(&def.id)),
            _ => None,
        };
        profile.record(Earned {
            id: def.id.clone(),
            first_tick: clock.tick,
            permadeath,
            rolled_stat,
        });
        pending.0.push(def.id.clone());
        // `Outcome` rather than `Info` deliberately: a rung can be crossed
        // mid-fight, and `MessageLog::retain_outcomes_since_battle` prunes
        // everything but four kinds when the battle ends. An `Info` line
        // would silently vanish at exactly the moment the player looked up
        // from the fight.
        log.push_kind(
            MessageKind::Outcome,
            format!("ACHIEVEMENT: {} — {}", def.name, def.description),
        );
    }

    // Unconditional, earned or not: this is a per-tick queue, and leaving a
    // kill in it re-earns from it forever once a later rung becomes reachable.
    feats.bosses_defeated.clear();
}
