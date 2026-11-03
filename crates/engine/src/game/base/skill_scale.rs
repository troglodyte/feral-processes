//! What a work skill is worth at a work site (ADR 0038). `skills/` knows
//! levels; this file knows which job practises which skill and how fast a
//! level works, so every site reads the scale through one door.

use bevy_ecs::change_detection::Mut;
use bevy_ecs::entity::Entity;

use crate::Game;

use crate::components::TaskKind;
use crate::resources::{MessageKind, PendingSkillLevels};
use crate::skills::{self, Skill, Skills};
use crate::structures::Placed;
use crate::tuning::WORK_SKILL_SPREAD;

/// Which skill a posting practises, or `None` for work that has none.
///
/// Exhaustive on `TaskKind`, `Duty::admits`'s rule: a new task kind fails to
/// compile here rather than silently practising nothing. `machine` is the
/// posted structure as placed; it only matters for `GatherResource`.
///
/// A Research Station is classified by `studies` alone: it is the one rule
/// that already separates it from every other `work` block (`category()`
/// checks it first for the same reason), and its research payout follows.
pub(crate) fn work_skill_for(kind: TaskKind, machine: Option<Placed<'_>>) -> Option<Skill> {
    match kind {
        TaskKind::GatherResource => {
            let placed = machine?;
            if placed.def.studies {
                return Some(Skill::Research);
            }
            let production = placed.production()?;
            Some(if production.extracts {
                Skill::Extract
            } else {
                Skill::Assemble
            })
        }
        TaskKind::Excavate => Some(Skill::Dig),
        TaskKind::Construct => Some(Skill::Build),
        TaskKind::Tend(_) => Some(Skill::Farm),
        // A guard has no work rate to scale.
        TaskKind::Guard => None,
    }
}

/// The speed factor of `level`: half speed at 0, today's at 10, 1.5x at 20.
pub(crate) fn skill_scale(level: u32) -> f32 {
    skills::scale(level, WORK_SKILL_SPREAD)
}

/// `skill_scale` in thousandths of a step per tick, for the carry.
pub(crate) fn pace_permille(level: u32) -> u32 {
    (skill_scale(level) * 1000.0).round() as u32
}

/// `base` ticks of work at a worker of `scale`: divided, rounded to the
/// nearest tick, never below one.
pub(crate) fn scaled_ticks(base: u32, scale: f32) -> u32 {
    ((base as f32 / scale).round() as u32).max(1)
}

/// The scale a worker brings to `skill`; 1.0 without `Skills` or a skill.
pub(crate) fn scale_of(skills: Option<&Skills>, skill: Option<Skill>) -> f32 {
    match (skills, skill) {
        (Some(skills), Some(skill)) => skill_scale(skills.level(skill)),
        _ => 1.0,
    }
}

/// One working tick of `skill`: how many steps of progress it is worth, and
/// the practice it earns. A level gained is queued on `levels` for the shell
/// to announce. A worker without `Skills` (or a job without a skill) gets
/// exactly one step and earns nothing, which is today's behaviour.
///
/// Called only on a tick that advanced the worker's job, so practice is flat
/// per working tick and not scaled by speed.
pub(crate) fn work_tick(
    worker: Entity,
    skills: Option<&mut Skills>,
    skill: Option<Skill>,
    levels: &mut PendingSkillLevels,
) -> u32 {
    let (Some(skills), Some(skill)) = (skills, skill) else {
        return 1;
    };
    let steps = skills.pace_steps(pace_permille(skills.level(skill)));
    practise_tick(worker, skills, skill, levels);
    steps
}

/// The practice half of `work_tick`, for the sites (a dig swing, a farm task)
/// whose pace is baked into `Task::required` and so need no carry.
pub(crate) fn practise_tick(
    worker: Entity,
    skills: &mut Skills,
    skill: Skill,
    levels: &mut PendingSkillLevels,
) {
    let outcome = skills::practise(skills.track_mut(skill), 1);
    if outcome.levels_gained > 0 {
        levels.0.push((worker, skill, outcome.new_level));
    }
}

impl Game {
    /// Whether `worker`'s work skills count. The player carries `Skills` for
    /// combat and works at today's pace whatever their work tracks say; every
    /// `&mut Game` work door asks this. The bevy work systems keep the player
    /// out another way, by querying `With<Tamed>`.
    fn work_skills_count(&self, worker: Entity) -> bool {
        worker != self.player_entity()
    }

    /// Every work skill of `entity`, or `None` if it carries no `Skills`.
    pub(crate) fn skill_views(&self, entity: Entity) -> Option<Vec<crate::views::SkillView>> {
        self.family_views(entity, &Skill::WORK)
    }

    /// Every combat skill of `entity`, or `None` if it carries no `Skills`.
    pub(crate) fn combat_skill_views(
        &self,
        entity: Entity,
    ) -> Option<Vec<crate::views::SkillView>> {
        self.family_views(entity, &Skill::COMBAT)
    }

    fn family_views(
        &self,
        entity: Entity,
        family: &[Skill],
    ) -> Option<Vec<crate::views::SkillView>> {
        let skills = self.world.get::<Skills>(entity)?;
        Some(
            family
                .iter()
                .copied()
                .map(|s| {
                    let t = skills.track(s);
                    crate::views::SkillView {
                        skill_label: s.label(),
                        level: t.level,
                        practice: t.practice,
                        practice_to_next: skills::practice_to_next(t.level),
                    }
                })
                .collect(),
        )
    }

    /// Posts one `LevelUp` line per level the work sites queued. One entry is
    /// one level (practice arrives a point a tick), and a site that ever
    /// gained several at once queues each, so no number is skipped.
    pub(crate) fn drain_skill_levels(&mut self) {
        let pending = std::mem::take(&mut self.world.resource_mut::<PendingSkillLevels>().0);
        for (entity, skill, level) in pending {
            let line = if entity == self.player_entity() {
                format!("You are now a level {level} {}.", skill.role_noun())
            } else {
                let Some(name) = self.creature_name(entity) else {
                    continue;
                };
                format!("{name} is now a level {level} {}.", skill.role_noun())
            };
            self.log_base_kind(MessageKind::LevelUp, line);
        }
    }

    /// `scale_of` for a live roster entity.
    pub(crate) fn skill_scale_of(&self, worker: Entity, skill: Skill) -> f32 {
        let skills = self
            .work_skills_count(worker)
            .then(|| self.world.get::<Skills>(worker))
            .flatten();
        scale_of(skills, Some(skill))
    }

    /// `work_tick` for the `&mut Game` work sites: the steps one working tick
    /// of `skill` is worth for `worker`, practising as it goes.
    pub(crate) fn work_steps(&mut self, worker: Entity, skill: Skill) -> u32 {
        let counts = self.work_skills_count(worker);
        self.world
            .resource_scope(|world, mut levels: Mut<PendingSkillLevels>| {
                let mut skills = counts.then(|| world.get_mut::<Skills>(worker)).flatten();
                work_tick(worker, skills.as_deref_mut(), Some(skill), &mut levels)
            })
    }

    /// `practise_tick` for the `&mut Game` work sites; a no-op for a worker
    /// without `Skills`.
    pub(crate) fn practise_work(&mut self, worker: Entity, skill: Skill) {
        if !self.work_skills_count(worker) {
            return;
        }
        self.world
            .resource_scope(|world, mut levels: Mut<PendingSkillLevels>| {
                if let Some(mut skills) = world.get_mut::<Skills>(worker) {
                    practise_tick(worker, &mut skills, skill, &mut levels);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_half_at_zero_one_at_ten_and_rises() {
        assert_eq!(pace_permille(0), 500);
        assert_eq!(pace_permille(10), 1000);
        assert!(pace_permille(20) > 1000);
        assert!((0..20).all(|l| pace_permille(l) < pace_permille(l + 1)));
    }

    #[test]
    fn scaled_ticks_round_and_never_reach_zero() {
        assert_eq!(scaled_ticks(10, 0.5), 20);
        assert_eq!(scaled_ticks(10, 1.0), 10);
        assert_eq!(scaled_ticks(1, 1.5), 1);
        assert_eq!(scaled_ticks(0, 1.5), 1);
    }

    #[test]
    fn a_pace_pays_out_whole_steps_and_carries_the_rest() {
        let mut skills = Skills::default();
        let steps: u32 = (0..10).map(|_| skills.pace_steps(500)).sum();
        assert_eq!(steps, 5);
        let steps: u32 = (0..10).map(|_| skills.pace_steps(1500)).sum();
        assert_eq!(steps, 15);
    }

    #[test]
    fn guarding_practises_nothing_and_a_missing_machine_too() {
        assert_eq!(work_skill_for(TaskKind::Guard, None), None);
        assert_eq!(work_skill_for(TaskKind::GatherResource, None), None);
        assert_eq!(work_skill_for(TaskKind::Excavate, None), Some(Skill::Dig));
        assert_eq!(
            work_skill_for(TaskKind::Construct, None),
            Some(Skill::Build)
        );
    }
}
