//! A program with nothing left rounds on a colleague.
//!
//! The rung past refusal. `morale.rs` decides who is on it; this file is what
//! being on it *does* — three steps sharing one resource, run once a beat
//! from `schedule_base_labour`.
//!
//! **The order of the three is load-bearing.** Advance every open fight,
//! close what has finished, and only then open new ones — so a brawl that
//! starts this beat throws its first blow on the next one. The alert
//! therefore always precedes any damage in the log, and a fight is never
//! opened and advanced in the same pass, which is what keeps `ticks_left` an
//! honest count of exchanges rather than one that is sometimes short by one.
//!
//! **The whole of "nobody dies" is one expression**, `raw.min(hp - 1)`,
//! applied to the *input* before `Game::apply_damage` — `game/throw.rs`'s
//! idiom. `apply_damage` is the one damaging path in the game, it floors HP
//! at zero, and reaching zero *is* a kill, announced. Nothing inside it
//! refuses a lethal blow. Anyone who later moves this damage calculation
//! without carrying the clamp with it turns a bad mood into a way to lose
//! companions.

use bevy_ecs::prelude::Entity;
use rand::RngExt;

use crate::Game;
use crate::components::{
    Disgruntled, Downed, Grievance, MemorySubject, Position, ProgramId, Stats,
};
use crate::resources::{Brawl, Brawls, EffectKind, GameRng, MessageKind};
use crate::tuning::{
    TANTRUM_CHANCE_PER_TICK, TANTRUM_COOLDOWN_TICKS, TANTRUM_DAMAGE_FRACTION, TANTRUM_REACH_TILES,
    TANTRUM_TICKS_MAX, TANTRUM_TICKS_MIN,
};

impl Game {
    /// One beat of every fight in the base.
    ///
    /// Sits between `update_disgruntled` — which is what puts a program on
    /// the rung this reads — and `admit_the_badly_hurt`, and **that ordering
    /// is what gets the Repair Bay for free**: a blow landed this beat is
    /// answered by the bay this beat, through the one writer that already
    /// owns that decision.
    pub(crate) fn run_tantrums(&mut self, staff: &[Entity]) {
        // The same gate `fray` consults, and asked once at the top: while a
        // base is still getting started none of this happens at all.
        if !self.base_is_established() {
            return;
        }
        self.advance_brawls();
        self.close_finished_brawls(staff);
        self.open_brawls(staff);
    }

    /// Both parties swing, and the fight is one beat shorter.
    fn advance_brawls(&mut self) {
        let mut open = std::mem::take(&mut self.world.resource_mut::<Brawls>().open);
        for brawl in &mut open {
            brawl.dealt += self.swing(brawl.aggressor, brawl.victim);
            brawl.taken += self.swing(brawl.victim, brawl.aggressor);
            brawl.ticks_left = brawl.ticks_left.saturating_sub(1);
        }
        self.world.resource_mut::<Brawls>().open = open;
    }

    /// One non-lethal blow, reporting what landed.
    ///
    /// A body already in the bay throws nothing, and a blow the clamp takes
    /// to zero is not thrown at all — no damage call, no cue. Both totals are
    /// what *landed*, which is what lets a side that never landed anything be
    /// told apart from one that landed a little.
    fn swing(&mut self, from: Entity, to: Entity) -> i32 {
        if self.world.get::<Downed>(from).is_some() {
            return 0;
        }
        let Some(stats) = self.world.get::<Stats>(to) else {
            return 0;
        };
        let raw = (TANTRUM_DAMAGE_FRACTION * stats.max_hp as f32) as i32;
        // **The clamp, and it is the whole of "a tantrum never kills."**
        let damage = raw.min(stats.hp - 1).max(0);
        if damage == 0 {
            return 0;
        }
        let landed = self.apply_damage(to, damage);
        self.push_effect(to, EffectKind::Brawl);
        landed
    }

    /// A fight ends on its last beat, on either party being downed or gone,
    /// or on either leaving base staff.
    fn close_finished_brawls(&mut self, staff: &[Entity]) {
        let open = std::mem::take(&mut self.world.resource_mut::<Brawls>().open);
        let (finished, running): (Vec<Brawl>, Vec<Brawl>) = open.into_iter().partition(|b| {
            b.ticks_left == 0
                || [b.aggressor, b.victim].iter().any(|&who| {
                    self.world.get::<Stats>(who).is_none()
                        || self.world.get::<Downed>(who).is_some()
                        || !staff.contains(&who)
                })
        });
        self.world.resource_mut::<Brawls>().open = running;
        let now = self.current_tick();
        for brawl in &finished {
            self.close_brawl(brawl);
            self.world
                .resource_mut::<Brawls>()
                .cooled_at
                .insert(brawl.aggressor, now);
        }
    }

    /// Says what the fight was and writes what each side is left holding.
    ///
    /// The reply line is **omitted** rather than printed as `0 damage` when
    /// the other side never landed one — reachable whenever a party is down
    /// to its last point of Integrity, where the non-lethal clamp takes every
    /// blow aimed at it to nothing.
    pub(crate) fn close_brawl(&mut self, brawl: &Brawl) {
        let who = self.creature_label(brawl.aggressor);
        let them = self.creature_label(brawl.victim);
        if brawl.dealt > 0 {
            self.log_base_kind(
                MessageKind::Tantrum,
                format!(
                    "{who} hurt {them} in a tantrum, causing {} damage.",
                    brawl.dealt
                ),
            );
        }
        if brawl.taken > 0 {
            self.log_base_kind(
                MessageKind::Tantrum,
                format!("{them} fought back, causing {} damage.", brawl.taken),
            );
        }
        self.log_base_kind(
            MessageKind::Tantrum,
            format!("{who} came out of it calmer. {them} will not forget it."),
        );
        self.remember(brawl.aggressor, "vented", MemorySubject::Nothing);
        if let Some(id) = self.world.get::<ProgramId>(brawl.aggressor).copied() {
            self.remember(brawl.victim, "turned_on_me", MemorySubject::Program(id));
        }
    }

    /// At most one new fight per candidate.
    ///
    /// **The roll is inside the per-candidate loop**, so a base with nobody
    /// on the rung draws nothing at all from `GameRng` — this feature cannot
    /// shift the seeded stream, which would read later as unrelated tests
    /// flaking.
    fn open_brawls(&mut self, staff: &[Entity]) {
        let now = self.current_tick();
        let busy: Vec<Entity> = self
            .world
            .resource::<Brawls>()
            .open
            .iter()
            .flat_map(|b| [b.aggressor, b.victim])
            .collect();
        let candidates: Vec<Entity> = staff
            .iter()
            .copied()
            .filter(|&who| {
                self.world
                    .get::<Disgruntled>(who)
                    .is_some_and(|d| d.grievance == Grievance::LashingOut)
            })
            .filter(|&who| self.world.get::<Downed>(who).is_none())
            .filter(|&who| !busy.contains(&who))
            .filter(|&who| {
                self.world
                    .resource::<Brawls>()
                    .cooled_at
                    .get(&who)
                    .is_none_or(|&then| now.saturating_sub(then) >= TANTRUM_COOLDOWN_TICKS)
            })
            .collect();
        for aggressor in candidates {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(TANTRUM_CHANCE_PER_TICK)
            };
            if !roll {
                continue;
            }
            let Some(victim) = self.pick_victim(aggressor, staff, &busy) else {
                continue;
            };
            let ticks = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(TANTRUM_TICKS_MIN..=TANTRUM_TICKS_MAX)
            };
            let who = self.creature_label(aggressor);
            let them = self.creature_label(victim);
            self.log_base_kind(MessageKind::Tantrum, format!("{who} rounds on {them}."));
            self.world.resource_mut::<Brawls>().open.push(Brawl {
                aggressor,
                victim,
                ticks_left: ticks,
                dealt: 0,
                taken: 0,
            });
        }
    }

    /// The one it likes least, of the staff standing within reach.
    ///
    /// Reach decides who is eligible at all; among those the pick is the
    /// lowest `opinion_of`, falling through to nearest and then to `Entity`
    /// order so an all-neutral field still resolves the same way every run.
    fn pick_victim(&self, aggressor: Entity, staff: &[Entity], busy: &[Entity]) -> Option<Entity> {
        let here = self.world.get::<Position>(aggressor).copied()?;
        let mut ranked: Vec<(Entity, f32, i32)> = staff
            .iter()
            .copied()
            .filter(|&who| who != aggressor)
            .filter(|&who| self.world.get::<Downed>(who).is_none())
            .filter(|&who| !busy.contains(&who))
            .filter_map(|who| {
                let there = self.world.get::<Position>(who).copied()?;
                let away = (there.x - here.x).abs().max((there.y - here.y).abs());
                (away <= TANTRUM_REACH_TILES).then_some((
                    who,
                    self.regard_for(aggressor, who),
                    away,
                ))
            })
            .collect();
        ranked.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.2.cmp(&b.2))
                .then(a.0.cmp(&b.0))
        });
        ranked.first().map(|&(who, _, _)| who)
    }

    /// What `who` thinks of `other` as a colleague — zero for a body with no
    /// `ProgramId` to be the subject of a memory, which is what the player's
    /// own entity would be.
    fn regard_for(&self, who: Entity, other: Entity) -> f32 {
        self.world
            .get::<ProgramId>(other)
            .map(|&id| self.opinion_of(who, &MemorySubject::Program(id)))
            .unwrap_or(0.0)
    }
}
