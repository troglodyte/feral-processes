//! Practised skills for tamed programs (ADR 0038). This module knows nothing
//! about structures, tasks or duties: what a level is worth belongs to the
//! code that reads it, and who practises what belongs to the work sites.

use bevy_ecs::component::Component;
use serde::{Deserialize, Serialize};

use crate::tuning::{SKILL_COST_BASE, SKILL_COST_QUAD, SKILL_LEVEL_CAP, SKILL_NEUTRAL_LEVEL};

#[cfg(test)]
mod tests;

/// `Ord` is display order. The save's string form is [`Skill::name`], never
/// the discriminant, so reordering cannot rewrite a save.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub enum Skill {
    Extract,
    Assemble,
    Research,
    Build,
    Dig,
    Farm,
    Aim,
    Force,
    Defence,
    Routines,
}

impl Skill {
    /// The skills a program practises at a work site.
    pub const WORK: [Skill; 6] = [
        Skill::Extract,
        Skill::Assemble,
        Skill::Research,
        Skill::Build,
        Skill::Dig,
        Skill::Farm,
    ];

    /// The skills a body practises by fighting.
    pub const COMBAT: [Skill; 4] = [Skill::Aim, Skill::Force, Skill::Defence, Skill::Routines];

    /// `WORK` then `COMBAT`: the save and the census read every track.
    pub const ALL: [Skill; 10] = [
        Skill::Extract,
        Skill::Assemble,
        Skill::Research,
        Skill::Build,
        Skill::Dig,
        Skill::Farm,
        Skill::Aim,
        Skill::Force,
        Skill::Defence,
        Skill::Routines,
    ];

    /// The save's string form; a name a future build drops loads inert, see
    /// [`Skill::from_name`].
    pub fn name(&self) -> &'static str {
        match self {
            Skill::Extract => "extract",
            Skill::Assemble => "assemble",
            Skill::Research => "research",
            Skill::Build => "build",
            Skill::Dig => "dig",
            Skill::Farm => "farm",
            Skill::Aim => "aim",
            Skill::Force => "force",
            Skill::Defence => "defence",
            Skill::Routines => "routines",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Skill::Extract => "Extract",
            Skill::Assemble => "Assemble",
            Skill::Research => "Research",
            Skill::Build => "Build",
            Skill::Dig => "Dig",
            Skill::Farm => "Farm",
            Skill::Aim => "Aim",
            Skill::Force => "Force",
            Skill::Defence => "Defence",
            Skill::Routines => "Routines",
        }
    }

    /// The job a level in this skill makes its holder, for the level-up line.
    pub fn role_noun(&self) -> &'static str {
        match self {
            Skill::Extract => "extractor",
            Skill::Assemble => "assembler",
            Skill::Research => "researcher",
            Skill::Build => "builder",
            Skill::Dig => "digger",
            Skill::Farm => "farmer",
            Skill::Aim => "marksman",
            Skill::Force => "bruiser",
            Skill::Defence => "guardian",
            Skill::Routines => "scripter",
        }
    }

    /// `None` for a name this build does not know, so the load path can drop
    /// it rather than refuse the save.
    pub fn from_name(name: &str) -> Option<Skill> {
        Skill::ALL.into_iter().find(|s| s.name() == name)
    }

    fn index(self) -> usize {
        Skill::ALL
            .iter()
            .position(|s| *s == self)
            .expect("ALL lists every Skill")
    }
}

/// The practised scale of `level`: 1.0 at [`SKILL_NEUTRAL_LEVEL`], `spread`
/// either side of it at level 0 and at twice neutral. What a scale is worth
/// is each reader's call; this is only the line.
pub fn scale(level: u32, spread: f32) -> f32 {
    let neutral = SKILL_NEUTRAL_LEVEL as f32;
    1.0 + spread * (level as f32 - neutral) / neutral
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SkillTrack {
    pub level: u32,
    /// Progress toward the next level; always 0 at the cap.
    pub practice: u32,
}

/// One track per [`Skill`]. Present only on tamed programs: an entity without
/// it is never practised and its work sites read a scale of 1.0.
#[derive(Component, Clone, Debug, Default)]
pub struct Skills {
    tracks: [SkillTrack; Skill::ALL.len()],
    /// Thousandths of a work step not yet paid out, so a pace below one step
    /// per tick (or above it) averages out without an RNG. Never saved: losing
    /// it costs less than one step.
    carry: u32,
}

/// Equality is the saved state: `carry` is never saved, so a save->load
/// round trip compares equal.
impl PartialEq for Skills {
    fn eq(&self, other: &Self) -> bool {
        self.tracks == other.tracks
    }
}

impl Eq for Skills {}

/// The level `Skills::seeded` adds a listed bonus to: combat skills start
/// at neutral, work skills at 0.
fn seed_base(skill: Skill) -> u32 {
    if Skill::COMBAT.contains(&skill) {
        SKILL_NEUTRAL_LEVEL
    } else {
        0
    }
}

impl Skills {
    /// How many work steps one working tick is worth at `permille` thousandths
    /// of a step per tick, carrying the remainder to the next tick.
    pub fn pace_steps(&mut self, permille: u32) -> u32 {
        let total = self.carry + permille;
        self.carry = total % 1000;
        total / 1000
    }

    pub fn level(&self, s: Skill) -> u32 {
        self.tracks[s.index()].level
    }

    pub fn track(&self, s: Skill) -> SkillTrack {
        self.tracks[s.index()]
    }

    pub fn track_mut(&mut self, s: Skill) -> &mut SkillTrack {
        &mut self.tracks[s.index()]
    }

    /// The tracks a body starts with: every combat skill at
    /// [`SKILL_NEUTRAL_LEVEL`] plus its `listed` bonus, a work skill at its
    /// `listed` level from 0 (a species' knack is absolute). Both
    /// `seed_skills` doors call this, so a class and a species cannot seed by
    /// different formulas.
    pub fn seeded(listed: &[(Skill, u32)]) -> Skills {
        let mut skills = Skills::default();
        for skill in Skill::COMBAT {
            skills.set(skill, SKILL_NEUTRAL_LEVEL, 0);
        }
        for (skill, level) in listed {
            skills.set(*skill, seed_base(*skill).saturating_add(*level), 0);
        }
        skills
    }

    /// The most a `starting_skills` row may list for `skill` before `seeded`
    /// would run past the cap: the cap less the level it seeds from.
    pub fn max_listed(skill: Skill) -> u32 {
        SKILL_LEVEL_CAP - seed_base(skill)
    }

    /// Saved rows laid over `seed`: a skill the rows do not list keeps the
    /// seed's track, so a save written before a skill existed loads it at its
    /// starting level rather than 0.
    pub fn overlay(seed: Skills, rows: impl IntoIterator<Item = (Skill, u32, u32)>) -> Skills {
        let mut skills = seed;
        for (skill, level, practice) in rows {
            skills.set(skill, level, practice);
        }
        skills
    }

    /// Installs a track from untrusted input (a species file, a save): a
    /// level past the cap is clamped, and practice is dropped at the cap.
    pub fn set(&mut self, s: Skill, level: u32, practice: u32) {
        let level = level.min(SKILL_LEVEL_CAP);
        let practice = if level >= SKILL_LEVEL_CAP {
            0
        } else {
            practice
        };
        self.tracks[s.index()] = SkillTrack { level, practice };
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PractiseOutcome {
    pub levels_gained: u32,
    pub new_level: u32,
}

/// Practice needed to leave `level`; `None` at the cap.
pub fn practice_to_next(level: u32) -> Option<u32> {
    (level < SKILL_LEVEL_CAP).then(|| SKILL_COST_BASE + SKILL_COST_QUAD * level * level)
}

/// How far `level` (with `practice` into it) is toward `SKILL_LEVEL_CAP`, in
/// `0.0..=1.0`. Counted in levels, the way the sheet's `Lv N` reads, rather
/// than in practice, whose quadratic cost would leave every early level an
/// invisible sliver.
pub fn progress_to_cap(level: u32, practice: u32) -> f32 {
    let part = practice_to_next(level).map_or(0.0, |next| practice as f32 / next as f32);
    ((level as f32 + part) / SKILL_LEVEL_CAP as f32).min(1.0)
}

/// Adds `amount`, rolling levels over until the practice runs out or the cap
/// is reached. Pure: no world, no RNG.
pub fn practise(track: &mut SkillTrack, amount: u32) -> PractiseOutcome {
    let start = track.level;
    let mut pool = track.practice.saturating_add(amount);
    while let Some(cost) = practice_to_next(track.level) {
        if pool < cost {
            break;
        }
        pool -= cost;
        track.level += 1;
    }
    track.practice = if track.level >= SKILL_LEVEL_CAP {
        0
    } else {
        pool
    };
    PractiseOutcome {
        levels_gained: track.level - start,
        new_level: track.level,
    }
}
