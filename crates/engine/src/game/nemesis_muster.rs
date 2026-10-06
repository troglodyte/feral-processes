//! A nemesis's band: who it gathers, how they trail it, and what ends a
//! follower's tag. The march and the siege read the band this builds.

use crate::alerts::AlertKind;
use crate::components::{Besieger, NemesisFollower, NemesisMuster};
use crate::game::spawning::SpawnEscalation;
use crate::tuning::{
    NEMESIS_BAND_MAX, NEMESIS_FOLLOW_DISTANCE, NEMESIS_RECRUIT_INTERVAL, NEMESIS_RECRUIT_RADIUS,
};
use crate::*;

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

impl Game {
    /// One world tick of every nemesis band: release followers whose leader
    /// is gone, let each idle nemesis recruit, then close followers on their
    /// leaders. A no-op during a fight, which is also when a leader's
    /// `Position` is least meaningful.
    pub(crate) fn nemesis_muster(&mut self) {
        if self.has_active_battle() {
            return;
        }
        self.release_orphaned_followers();
        let mut leaders: Vec<Entity> = self
            .world
            .query_filtered::<Entity, (With<Nemesis>, Without<Pursuing>, Without<Besieger>)>()
            .iter(&self.world)
            .collect();
        // Query order is not stable; a fixed order keeps the RNG draws of a
        // fallback spawn reproducible.
        leaders.sort();
        for leader in leaders {
            self.muster_one(leader);
        }
        self.follow_leaders();
    }

    /// The bodies tagged as following `leader`, in a fixed order.
    pub(crate) fn nemesis_band(&mut self, leader: Entity) -> Vec<Entity> {
        let mut band: Vec<Entity> = self
            .world
            .query::<(Entity, &NemesisFollower)>()
            .iter(&self.world)
            .filter(|(_, f)| f.0 == leader)
            .map(|(e, _)| e)
            .collect();
        band.sort();
        band
    }

    fn release_orphaned_followers(&mut self) {
        let orphans: Vec<Entity> = self
            .world
            .query::<(Entity, &NemesisFollower)>()
            .iter(&self.world)
            .filter(|(_, f)| self.world.get::<Nemesis>(f.0).is_none())
            .map(|(e, _)| e)
            .collect();
        for e in orphans {
            self.world.entity_mut(e).remove::<NemesisFollower>();
        }
    }

    fn muster_one(&mut self, leader: Entity) {
        let ticks = {
            let mut entity = self.world.entity_mut(leader);
            let mut muster = entity.entry::<NemesisMuster>().or_default();
            muster.get_mut().ticks += 1;
            muster.get().ticks
        };
        if ticks < NEMESIS_RECRUIT_INTERVAL || self.nemesis_band(leader).len() >= NEMESIS_BAND_MAX {
            return;
        }
        let Some(recruit) = self
            .nearest_recruit(leader)
            .or_else(|| self.spawn_recruit(leader))
        else {
            return;
        };
        self.world
            .entity_mut(recruit)
            .insert(NemesisFollower(leader));
        self.world.get_mut::<NemesisMuster>(leader).unwrap().ticks = 0;
        if self.nemesis_band(leader).len() >= NEMESIS_BAND_MAX {
            let label = self.creature_label(leader);
            self.post_alert(
                AlertKind::SiegeIncoming,
                format!("nemesis-band-{label}"),
                format!("{label} has gathered a band."),
            );
        }
    }

    /// The nearest wild hostile within `NEMESIS_RECRUIT_RADIUS` of `leader`
    /// that is not already spoken for. A nest guardian belongs to a place,
    /// a boss to itself, and a besieger is mid-siege.
    fn nearest_recruit(&mut self, leader: Entity) -> Option<Entity> {
        let here = *self.world.get::<Position>(leader)?;
        self.world
            .query_filtered::<(Entity, &Position), (
                With<Hostile>,
                Without<Nemesis>,
                Without<NemesisFollower>,
                Without<NestGuardian>,
                Without<Boss>,
                Without<Besieger>,
            )>()
            .iter(&self.world)
            .map(|(e, p)| (chebyshev(here, *p), e))
            .filter(|&(d, _)| d <= NEMESIS_RECRUIT_RADIUS)
            .min()
            .map(|(_, e)| e)
    }

    /// One habitat body at the leader's cell: wild population is sparse away
    /// from the player, so without this a distant nemesis would never gather.
    fn spawn_recruit(&mut self, leader: Entity) -> Option<Entity> {
        let Position { x, y } = *self.world.get::<Position>(leader)?;
        let (species, _) = self.pick_habitat_species(x, y, None, false)?;
        let esc: SpawnEscalation = self.field_escalation(x, y);
        self.spawn_group(&species, 1, x, y, esc, false)
            .into_iter()
            .next()
    }

    /// Steps each follower one cell toward its leader when it has fallen
    /// more than `NEMESIS_FOLLOW_DISTANCE` behind.
    fn follow_leaders(&mut self) {
        let mut followers: Vec<(Entity, Entity)> = self
            .world
            .query_filtered::<(Entity, &NemesisFollower), (Without<Pursuing>, Without<Besieger>)>()
            .iter(&self.world)
            .map(|(e, f)| (e, f.0))
            .collect();
        followers.sort();
        for (follower, leader) in followers {
            let (Some(&from), Some(&to)) = (
                self.world.get::<Position>(follower),
                self.world.get::<Position>(leader),
            ) else {
                continue;
            };
            if chebyshev(from, to) <= NEMESIS_FOLLOW_DISTANCE {
                continue;
            }
            let (dx, dy) = ((to.x - from.x).signum(), (to.y - from.y).signum());
            // Diagonal first, then each axis: a wall on the straight line
            // should bend the walk, not stop it.
            for (sx, sy) in [(dx, dy), (dx, 0), (0, dy)] {
                if (sx, sy) == (0, 0) {
                    continue;
                }
                let (nx, ny) = (from.x + sx, from.y + sy);
                if self
                    .world
                    .resource_mut::<crate::world::WorldMap>()
                    .tile(nx, ny)
                    .open_to_hostiles()
                {
                    let mut pos = self.world.get_mut::<Position>(follower).unwrap();
                    pos.x = nx;
                    pos.y = ny;
                    break;
                }
            }
        }
    }

    /// The number a save gives the band `e` is in, or `None` if it is in
    /// none: the leader's rank among the leaders that have followers, in an
    /// order built from what a body *is* (cell, species, grudge) so the same
    /// world numbers the same way after a reload. The entity index only
    /// breaks a tie between two leaders that are otherwise identical, which
    /// keeps their bands apart within one file.
    pub(crate) fn nemesis_band_number(&mut self, e: Entity) -> Option<u32> {
        let leader = match self.world.get::<NemesisFollower>(e) {
            Some(f) => f.0,
            None if self.world.get::<Nemesis>(e).is_some() => e,
            None => return None,
        };
        let mut leaders: Vec<Entity> = self
            .world
            .query::<&NemesisFollower>()
            .iter(&self.world)
            .map(|f| f.0)
            .filter(|&l| self.world.get::<Nemesis>(l).is_some())
            .collect();
        leaders.sort();
        leaders.dedup();
        let key = |game: &Game, l: Entity| {
            let pos = game.world.get::<Position>(l).map_or((0, 0), |p| (p.x, p.y));
            let species = game
                .world
                .get::<Creature>(l)
                .map(|c| c.species.clone())
                .unwrap_or_default();
            let grudge = game.world.get::<Nemesis>(l).map_or(0, |n| n.0);
            (pos, species, grudge, l)
        };
        leaders.sort_by_key(|&l| key(self, l));
        leaders.iter().position(|&l| l == leader).map(|i| i as u32)
    }

    /// Re-tags followers on their leaders after a load, from the band
    /// numbers `nemesis_band_number` wrote. A band with no leader in the file
    /// (hand-edited, or its leader's species was deleted) is dropped: the
    /// bodies load as ordinary wild programs.
    pub(crate) fn link_nemesis_bands(&mut self, pending: Vec<(u32, Entity, bool)>) {
        let leaders: std::collections::HashMap<u32, Entity> = pending
            .iter()
            .filter(|&&(_, _, is_leader)| is_leader)
            .map(|&(band, e, _)| (band, e))
            .collect();
        for (band, body, is_leader) in pending {
            if is_leader {
                continue;
            }
            if let Some(&leader) = leaders.get(&band) {
                self.world.entity_mut(body).insert(NemesisFollower(leader));
            }
        }
    }
}
