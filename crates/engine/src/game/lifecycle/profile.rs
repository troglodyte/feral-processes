//! Run history, score and the achievement profile on a live `Game`.

use crate::*;

/// Present once `Game::grant_profile_rewards` has run. Not saved and not
/// inserted by either constructor: it exists only so a second grant in the
/// same session is a no-op.
#[derive(Resource)]
struct ProfileRewardsPaid;

impl Game {
    pub fn history_summary(&mut self) -> Option<String> {
        let reason = self.world.resource::<GameOver>().reason.clone()?;
        let tick = self.world.resource::<GameClock>().tick;
        let mut query = self.world.query_filtered::<(), With<Tamed>>();
        let tamed_count = query.iter(&self.world).count();
        let score = self.score_card().total;
        Some(format!(
            "Session ended at cycle {tick}: {reason}. Programs compiled: {tamed_count}. \
             Score: {score}."
        ))
    }

    /// The run's score as it stands: the saved `RunTally` beside what is
    /// still live. The one API every score display reads.
    pub fn score_card(&mut self) -> crate::score::ScoreCard {
        let tally = self.world.resource::<crate::resources::RunTally>().clone();
        let keys = self
            .world
            .get::<crate::components::PhaseKeys>(self.player_entity())
            .copied()
            .unwrap_or_default();
        let structures = self
            .world
            .query_filtered::<(), With<Structure>>()
            .iter(&self.world)
            .count();
        crate::score::card(&crate::score::ScoreInputs {
            foe_levels: tally.foe_levels,
            bosses: u64::from(tally.bosses),
            deepest_depth: u64::from(tally.deepest_depth),
            keys: keys.count() as u64,
            escaped: keys.story_complete,
            structures: structures as u64,
            compiled: u64::from(tally.compiled),
            achievements: u64::from(tally.achievements),
            mode: *self.world.resource::<DifficultyMode>(),
            band: self.enemy_strength(),
        })
    }

    /// Adds the part of this run's score not yet banked to the profile's
    /// lifetime score and returns it. Idempotent: a second call with nothing
    /// new returns 0. A total that has fallen since the last bank (structures
    /// lost after an escape) banks nothing and leaves the mark where it was,
    /// so later growth is never counted twice.
    ///
    /// The caller owns the file: app-core adds the returned delta to its own
    /// profile copy and writes it, because `after_tick` is not reached on the
    /// game-over path.
    pub fn bank_run_score(&mut self) -> u64 {
        let total = self.score_card().total;
        let mut tally = self.world.resource_mut::<crate::resources::RunTally>();
        let delta = total.saturating_sub(tally.banked);
        tally.banked += delta;
        self.world
            .resource_mut::<crate::achievements::Profile>()
            .lifetime_score += delta;
        delta
    }

    pub fn write_history(&mut self, path: &Path) -> std::io::Result<()> {
        if let Some(summary) = self.history_summary() {
            save::append_run_history(path, &summary)
        } else {
            Ok(())
        }
    }

    /// The profile as it currently stands, including anything earned this
    /// run. What app-core writes to `profile.ron`.
    pub fn profile(&self) -> &crate::achievements::Profile {
        self.world.resource::<crate::achievements::Profile>()
    }

    /// Takes everything that has dirtied the profile since the last call,
    /// emptying the queue. `true` means "the profile changed, write it".
    ///
    /// The engine decides what happened; app-core owns the path and does the
    /// writing. A bool rather than the ids because no caller has ever read
    /// them — and there are two kinds now (a rung earned, a guidance
    /// setting changed), so a single typed list could only carry one of them honestly.
    pub fn take_pending_profile_writes(&mut self) -> bool {
        let mut pending = self
            .world
            .resource_mut::<crate::resources::PendingProfileWrites>();
        if pending.is_empty() {
            return false;
        }
        pending.earned.clear();
        pending.ftue = false;
        true
    }

    /// Replaces the empty `Profile` both constructors leave in the world with
    /// what has actually been earned across every run.
    ///
    /// **Pays nothing.** Both paths need this — `achievement_system` must not
    /// re-earn a rung on a loaded save either — while only one path wants to
    /// be paid. Splitting install from grant is what puts the never-on-load
    /// rule at a single call site instead of inside a shared constructor, and
    /// it leaves `Game::new`'s signature (667 call sites) alone.
    pub fn install_profile(&mut self, profile: crate::achievements::Profile) {
        self.world.insert_resource(profile);
    }

    /// Changes the running game's tactical-battles setting, so the next
    /// surface fight of this run uses it. `install_profile` hands the engine
    /// its copy once per run; without this a mid-run toggle would wait for
    /// the next one.
    pub fn set_tactical_battles(&mut self, on: bool) {
        self.world
            .resource_mut::<crate::achievements::Profile>()
            .tactical_battles = on;
    }

    /// Pays out the installed profile: stat points, Perk Points and a
    /// starting program, once each, in the order the rungs were earned.
    ///
    /// **Called after `Game::new` and never after `Game::load`.** A save
    /// already has its bonuses baked into `Stats` and `Perks::points`, so
    /// paying again on load would double them on every single reload. That is
    /// the one real trap in this feature; `installing_a_profile_pays_nothing_
    /// on_its_own` and app-core's `loading_a_save_does_not_re_apply_rewards`
    /// are what hold it.
    ///
    /// Takes no argument on purpose: it reads the installed `Profile`, so the
    /// two calls cannot disagree about which profile is in play.
    pub fn grant_profile_rewards(&mut self) {
        use crate::achievements::{AchievementDb, Profile, Reward};

        // Nothing calls this twice, and the doubling would be invisible if
        // something started to — a stat is just a number, with no record of
        // where it came from. The marker's absence is "unpaid", so neither
        // constructor has to remember to insert it; what it does require is
        // that `install_profile` runs *before* this, which is the order
        // app-core uses.
        if self.world.contains_resource::<ProfileRewardsPaid>() {
            return;
        }
        self.world.insert_resource(ProfileRewardsPaid);

        let (rewards, score_stat_points): (Vec<Reward>, u32) = {
            let db = self.world.resource::<AchievementDb>();
            let profile = self.world.resource::<Profile>();
            (
                crate::achievements::profile_rewards(profile, db),
                crate::score::creation_bonus(profile.lifetime_score).stat_points,
            )
        };
        if rewards.is_empty() && score_stat_points == 0 {
            return;
        }

        let player = self.player_entity();
        // Paid here rather than added to the Points step's budget, so
        // `MAX_CREATION_STAT_POINTS` stays the ceiling of creation itself.
        if let Some(mut points) = self.world.get_mut::<crate::components::StatPoints>(player) {
            points.0 += score_stat_points;
        }
        let mut stat_points = score_stat_points;
        let mut perk_points = 0;
        let mut programs = Vec::new();
        for reward in rewards {
            match reward {
                Reward::RandomMainStat(n) => {
                    if let Some(mut points) =
                        self.world.get_mut::<crate::components::StatPoints>(player)
                    {
                        points.0 += n;
                    }
                    stat_points += n;
                }
                Reward::PerkPoints(n) => {
                    self.world.get_mut::<Perks>(player).unwrap().points += n;
                    perk_points += n;
                }
                Reward::None => {}
                Reward::StartingProgram(species_id) => match self
                    .grant_starting_program(&species_id)
                {
                    Some(name) => programs.push(name),
                    None => self.log(format!(
                        "Profile reward skipped: no species named {species_id:?} in this install."
                    )),
                },
            }
        }

        let mut parts = Vec::new();
        if stat_points > 0 {
            parts.push(format!("{stat_points} stat point(s)"));
        }
        if perk_points > 0 {
            parts.push(format!("{perk_points} Perk Point(s)"));
        }
        for name in programs {
            parts.push(name);
        }
        if !parts.is_empty() {
            self.log_kind(
                MessageKind::Outcome,
                format!("Profile restored from your record: {}.", parts.join(", ")),
            );
        }
    }
}
