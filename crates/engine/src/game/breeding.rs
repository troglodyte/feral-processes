//! Breeding as the player meets it: starting one, reading its previews, and
//! the hatch that seats a child on the roster.
//!
//! The rules themselves (`child_species`, `inherit`) are pure functions in
//! `breeding.rs`; this file is the `&mut Game` half. The child is rolled when
//! the breeding **starts** and stored in the bay's `Incubator`, so nothing
//! about it can change between start and hatch.

use crate::Game;
use crate::breeding::{BreedRefusal, Incubation, ParentRolls, child_species, inherit, roll_span};
use crate::components::{
    Attributes, BreedReadyAt, Creature, Generation, Incubator, Potential, Structure, Tamed,
};
use crate::items::ItemId;
use crate::resources::{GameClock, GameRng};
use crate::species::SpeciesDb;
use crate::views::{BreedPreview, BreedSpeciesPreview, IncubatingChild, IncubationView, RollRange};
use bevy_ecs::prelude::{Entity, Mut};

/// The pack item one breeding costs.
const BREEDING_SEED: &str = "breeding_seed";

impl Game {
    fn clock_tick(&self) -> u64 {
        self.world.resource::<GameClock>().tick
    }

    /// Why `program` cannot be a parent right now, or `None`. The one answer
    /// both `breed` and the parent picker's dimmed rows read.
    pub fn breed_refusal(&self, program: Entity) -> Option<BreedRefusal> {
        let player = self.player_entity();
        if self
            .world
            .get::<Tamed>(program)
            .is_none_or(|t| t.owner != player)
        {
            return Some(BreedRefusal::NotYours);
        }
        let known = self
            .world
            .get::<Creature>(program)
            .is_some_and(|c| self.world.resource::<SpeciesDb>().get(&c.species).is_some());
        if !known {
            return Some(BreedRefusal::UnknownKind);
        }
        if self
            .world
            .get::<BreedReadyAt>(program)
            .is_some_and(|r| r.0 > self.clock_tick())
        {
            return Some(BreedRefusal::OnCooldown);
        }
        None
    }

    /// The first empty slot of `bay`, if it is a built bay with one.
    fn free_incubation_slot(&self, bay: Entity) -> Option<usize> {
        self.world.get::<Structure>(bay)?;
        self.world
            .get::<Incubator>(bay)?
            .slots
            .iter()
            .position(Option::is_none)
    }

    fn parent_rolls(&self, program: Entity) -> ParentRolls {
        ParentRolls {
            potential: self
                .world
                .get::<Potential>(program)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
            attributes: self
                .world
                .get::<Attributes>(program)
                .map(|a| a.iter().map(|(id, v)| (id.clone(), v)).collect())
                .unwrap_or_default(),
            generation: self.world.get::<Generation>(program).map_or(0, |g| g.0),
        }
    }

    fn species_of(&self, program: Entity) -> String {
        self.world
            .get::<Creature>(program)
            .map(|c| c.species.clone())
            .unwrap_or_default()
    }

    /// Breeds `a` with `b`, incubating the child in `bay`.
    ///
    /// Every refusal precedes the first mutation (`fuse_companions`'
    /// discipline), so a refused breeding leaves seeds, cooldowns and the bay
    /// as they were. The child's species and rolls are drawn **now** and
    /// stored, which is what stops a reload re-rolling it. Parents are
    /// otherwise untouched and stay fully usable: nothing references them
    /// once this returns.
    pub fn breed(&mut self, a: Entity, b: Entity, bay: Entity) -> Result<String, BreedRefusal> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err(BreedRefusal::Busy);
        }
        if a == b {
            return Err(BreedRefusal::SameProgram);
        }
        for parent in [a, b] {
            if let Some(refusal) = self.breed_refusal(parent) {
                return Err(refusal);
            }
        }
        let slot = self
            .free_incubation_slot(bay)
            .ok_or(BreedRefusal::NoFreeSlot)?;
        let seed = ItemId::from(BREEDING_SEED);
        let player = self.player_entity();
        if self
            .world
            .get::<crate::components::Inventory>(player)
            .is_none_or(|inv| inv.count(&seed) == 0)
        {
            return Err(BreedRefusal::NoSeed);
        }

        let (species_a, species_b) = (self.species_of(a), self.species_of(b));
        let (rolls_a, rolls_b) = (self.parent_rolls(a), self.parent_rolls(b));
        let (species, rolls) = self.world.resource_scope(|world, mut rng: Mut<GameRng>| {
            let db = world.resource::<SpeciesDb>();
            let id = child_species(&species_a, &species_b, db, &mut rng.0);
            // Both parents' kinds were found by `breed_refusal`, and a
            // hybrid is only ever authored over loaded species.
            let def = db.get(&id).expect("a child's species is loaded");
            let rolls = inherit(
                &rolls_a,
                &rolls_b,
                def,
                world.resource::<crate::attributes::AttributeDb>(),
                &mut rng.0,
            );
            (id, rolls)
        });

        let now = self.clock_tick();
        self.world
            .get_mut::<crate::components::Inventory>(player)
            .expect("checked above")
            .take(seed, 1);
        for parent in [a, b] {
            self.world
                .entity_mut(parent)
                .insert(BreedReadyAt(now + crate::tuning::BREEDING_COOLDOWN_TICKS));
        }
        self.world
            .get_mut::<Incubator>(bay)
            .expect("checked above")
            .slots[slot] = Some(Incubation {
            species,
            potential: rolls.potential,
            attributes: rolls.attributes,
            generation: rolls.generation,
            due: now + crate::tuning::INCUBATION_TICKS,
        });
        let line = format!(
            "{} and {} are bred. A child starts to incubate.",
            self.creature_label(a),
            self.creature_label(b)
        );
        self.log_base(line.clone());
        Ok(line)
    }

    /// What breeding `a` with `b` would make, before anything is spent. `None`
    /// when either is not a creature of a loaded species.
    ///
    /// The ranges are `breeding::roll_span`, the clamp `inherit` itself calls.
    pub fn breed_preview(&self, a: Entity, b: Entity) -> Option<BreedPreview> {
        let db = self.world.resource::<SpeciesDb>();
        let (species_a, species_b) = (self.species_of(a), self.species_of(b));
        let (def_a, def_b) = (db.get(&species_a)?, db.get(&species_b)?);
        let species = if species_a == species_b {
            BreedSpeciesPreview::Certain(def_a.name.clone())
        } else if let Some(hybrid) = db.hybrid_of(&species_a, &species_b) {
            BreedSpeciesPreview::Certain(hybrid.name.clone())
        } else {
            BreedSpeciesPreview::OneOf(def_a.name.clone(), def_b.name.clone())
        };
        let (ra, rb) = (self.parent_rolls(a), self.parent_rolls(b));
        let generation = ra.generation.max(rb.generation) + 1;
        let (pa, pb) = (ra.potential, rb.potential);
        let range = |label, a: f32, b: f32| {
            let (min, max) = roll_span(a.max(b), generation);
            RollRange { label, min, max }
        };
        Some(BreedPreview {
            species,
            generation,
            rolls: vec![
                range("HP", pa.hp_roll, pb.hp_roll),
                range("ATK", pa.atk_roll, pb.atk_roll),
                range("DEF", pa.def_roll, pb.def_roll),
                range("Growth", pa.growth_roll, pb.growth_roll),
                range("Assembly", pa.assembly_roll, pb.assembly_roll),
                range("Extraction", pa.extraction_roll, pb.extraction_roll),
            ],
        })
    }

    /// One row per slot of `bay`, empty slots included.
    pub fn incubations(&self, bay: Entity) -> Vec<IncubationView> {
        let Some(incubator) = self.world.get::<Incubator>(bay) else {
            return Vec::new();
        };
        let now = self.clock_tick();
        let db = self.world.resource::<SpeciesDb>();
        incubator
            .slots
            .iter()
            .map(|slot| IncubationView {
                child: slot.as_ref().map(|child| IncubatingChild {
                    species: db
                        .get(&child.species)
                        .map_or_else(|| child.species.clone(), |d| d.name.clone()),
                    generation: child.generation,
                    ticks_left: child.due.saturating_sub(now),
                    held: child.due <= now && self.roster_room() == 0,
                }),
            })
            .collect()
    }
}
