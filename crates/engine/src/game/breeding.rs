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
    Attributes, BreedReadyAt, Creature, Generation, Glyph, Incubator, Position, Potential, Rarity,
    Routines, Stats, StatusEffects, Structure, Tamed,
};
use crate::items::ItemId;
use crate::resources::{GameClock, GameRng, MessageKind};
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
        let def = self
            .world
            .get::<Creature>(program)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species));
        let Some(def) = def else {
            return Some(BreedRefusal::UnknownKind);
        };
        if def.is_boss {
            return Some(BreedRefusal::Boss);
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

    /// Breeding Seeds in the pack — what one breeding spends.
    pub fn breeding_seeds_held(&self) -> u32 {
        self.world
            .get::<crate::components::Inventory>(self.player_entity())
            .map_or(0, |inv| inv.count(&ItemId::from(BREEDING_SEED)))
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
        if self.breeding_seeds_held() == 0 {
            return Err(BreedRefusal::NoSeed);
        }
        let seed = ItemId::from(BREEDING_SEED);
        let player = self.player_entity();

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

    /// Logs one line per child a bay was holding as it is destroyed by a
    /// raid or siege. Demolition refuses an occupied bay instead
    /// (`remove_structure`), so this is the other destruction path's half
    /// of the same rule, and like `announce_lost_shelf` it is called from
    /// `damage_structure` before the despawn.
    pub(crate) fn announce_lost_children(&mut self, bay: Entity) {
        let Some(incubator) = self.world.get::<Incubator>(bay) else {
            return;
        };
        let lost: Vec<(String, u32)> = incubator
            .slots
            .iter()
            .flatten()
            .map(|c| (c.species.clone(), c.generation))
            .collect();
        for (species, generation) in lost {
            let name = self
                .world
                .resource::<SpeciesDb>()
                .get(&species)
                .map_or(species.clone(), |d| d.name.clone());
            self.log_kind(
                MessageKind::Raid,
                format!("A {name} child (gen {generation}) is lost with the Breeding Bay."),
            );
        }
    }

    /// Hatches every due incubation, in `(x, y)` order of bay.
    ///
    /// A child is held — it stays in its slot and retries next tick — while
    /// the roster has no room, which `incubations` reports as `held`. A due
    /// child whose species has since been deleted from the install is
    /// dropped with a line rather than retried forever.
    pub(crate) fn hatch_incubations(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let now = self.clock_tick();
        let mut due: Vec<(i32, i32, Entity, usize)> = {
            let mut query = self.world.query::<(Entity, &Position, &Incubator)>();
            query
                .iter(&self.world)
                .flat_map(|(bay, at, incubator)| {
                    incubator
                        .slots
                        .iter()
                        .enumerate()
                        .filter(move |(_, slot)| slot.as_ref().is_some_and(|c| c.due <= now))
                        .map(move |(i, _)| (at.x, at.y, bay, i))
                })
                .collect()
        };
        due.sort();
        for (x, y, bay, slot) in due {
            if self.roster_room() == 0 {
                return;
            }
            let Some(child) = self
                .world
                .get_mut::<Incubator>(bay)
                .and_then(|mut inc| inc.slots[slot].take())
            else {
                continue;
            };
            match self.seat_hatchling(&child, (x, y)) {
                Some(program) => {
                    let line = format!(
                        "{} is ready in the Breeding Bay (gen {}).",
                        self.creature_label(program),
                        child.generation
                    );
                    self.log_base(line);
                }
                None => self.log_base("A child in the Breeding Bay is lost: its kind is gone."),
            }
        }
    }

    /// Seats a hatched child on the roster: the fifth door through
    /// `roster_parts`, with its stats from the species base times the rolled
    /// potential and nothing else — no zone, depth or rarity multiplier, and
    /// no `Hostile`/`WanderAi` to strip. Arrives at the anchor like a
    /// caravan purchase, falling back to the bay's own tile.
    fn seat_hatchling(&mut self, child: &Incubation, bay_at: (i32, i32)) -> Option<Entity> {
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(&child.species)
            .cloned()?;
        let (x, y) = self.anchor_position().unwrap_or(bay_at);
        let potential = child.potential;
        let scale = |base: i32, roll: f32| ((base as f32) * roll).round() as i32;
        let hp = scale(species.base_hp, potential.hp_roll);
        let mut attributes = Attributes::default();
        for (id, value) in &child.attributes {
            attributes.set(id, *value);
        }
        let program = self
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x, y },
                Glyph {
                    ch: species.glyph,
                    color: species.color,
                },
                Stats {
                    hp,
                    max_hp: hp,
                    atk: scale(species.base_atk, potential.atk_roll),
                    mitigation: species.base_mitigation,
                },
                potential,
                Rarity::Ordinary,
                StatusEffects::default(),
                Routines::default(),
                Generation(child.generation),
                attributes,
            ))
            .id();
        let parts = self.roster_parts();
        self.world.entity_mut(program).insert(parts);
        self.install_innate_routines(program);
        self.seat_derived(program);
        Some(program)
    }
}
