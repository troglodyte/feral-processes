//! Where a spent subject's downed program goes: held at the Research
//! Station, then walked away by an idle body — to a Quarantine Rack, else a
//! free stockpile cell, else the floor beside the station.
//!
//! Research never touches the player's pack. `Game::spend_subject` hands the
//! program to `Game::hold_at_station` on a finished project or a discovery;
//! this module's errand empties the station's `StudyYield` slot, and a full
//! slot is the gate on the next spend (`Game::study_yield_blocker`).
//!
//! The errand is `clearing`'s shape: **destination before pickup** (nothing
//! leaves the station until a destination is in reach), the destination
//! re-derived each beat from where the body stands rather than stored, and a
//! body is never left holding something it cannot place. A carrier is a
//! `CarryingProgram` body with no `Task` — a teardown rig's fetcher is always
//! posted — so `is_on_shift` already never frees it, and the scheduler keeps
//! it out of the posting pool the way it keeps out a `Carrying` hauler.
//!
//! **It ranks above clearing** on `drift_idle_staff`'s fall-through: a kill
//! sitting in a slot stalls the whole research line, where a machine's
//! finished output only blocks that machine. A body already *holding* one
//! ranks higher still, above the `OffShift` and respite arms, and gives up
//! after `RACK_CARRY_PATIENCE_BEATS` on one tile rather than being held out
//! of the posting pool for ever.
//!
//! **A downed program is never lost and never goes to the pack**:
//! `shelter_program` is the one chain (rack, stockpile, floor), and
//! `Game::drop_load` calls it, so any door that frees a carrier keeps the
//! kill.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{
    Carrying, CarryingProgram, Position, RackWait, Racked, Structure, StudyYield, Task, Tidying,
    UnderStudy,
};
use crate::game::base::hauling::{at_station, crew_reach, reaches};
use crate::game::base::offshift::Amenities;
use crate::items::{DownedProgram, FloorThing};
use crate::situations::chebyshev;
use crate::structures::StructureDb;

/// The beat's racking work, shared by every idle body in the pass.
pub(crate) struct RackBoard {
    /// Stations still holding a program nobody has lifted, `(x, y)`-sorted.
    stations: Vec<(Entity, Position)>,
    /// Rack room not spoken for by a body already walking one: what a new
    /// pick-up may still claim.
    spare: u32,
    /// Floor cells a carrier has chosen this beat, so a second carrier does
    /// not walk to the same one.
    claimed: Vec<Position>,
}

/// Where a carrier is taking its program.
#[derive(Clone, Copy)]
enum ProgramDrop {
    Rack(Position, Entity),
    Floor(Position),
}

impl ProgramDrop {
    fn at(self) -> Position {
        match self {
            ProgramDrop::Rack(p, _) | ProgramDrop::Floor(p) => p,
        }
    }
}

/// How many cells around the station the last rung considers, nearest first.
const BESIDE_STATION_CELLS: usize = 16;

impl Game {
    /// Every Quarantine Rack, `(x, y)`-sorted. Sorted because callers pick
    /// "the first with room" and query order is not stable.
    pub(crate) fn quarantine_racks(&self) -> Vec<(i32, i32, Entity)> {
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let p = e.get::<Position>()?;
                db.get(kind)?.racks.as_ref()?;
                e.contains::<Racked>().then_some((p.x, p.y, e.id()))
            })
            .collect();
        found.sort();
        found
    }

    /// Whether this body is walking a program to a rack: a `CarryingProgram`
    /// with no `Task`. A rig's fetcher carries one too, but only while posted.
    pub(crate) fn is_rack_carrier(&self, who: Entity) -> bool {
        self.world.get::<CarryingProgram>(who).is_some() && self.world.get::<Task>(who).is_none()
    }

    fn programs_in_transit(&self) -> u32 {
        self.world
            .iter_entities()
            .filter(|e| self.is_rack_carrier(e.id()))
            .count() as u32
    }

    fn programs_held_at_stations(&self) -> u32 {
        self.world
            .iter_entities()
            .filter(|e| e.get::<StudyYield>().is_some_and(|held| held.0.is_some()))
            .count() as u32
    }

    fn total_rack_room(&self) -> u32 {
        self.quarantine_racks()
            .into_iter()
            .map(|(_, _, rack)| self.rack_room(rack))
            .sum()
    }

    /// Rack slots free once every program already promised a place is
    /// counted: those held at a station and those on their way.
    ///
    /// **Total room minus claims, not a walk.** Rack contents change by the
    /// player's own hand too, so this is read fresh by the gate each tick.
    pub(crate) fn free_rack_room(&self) -> u32 {
        let claimed = self.programs_in_transit() + self.programs_held_at_stations();
        self.total_rack_room().saturating_sub(claimed)
    }

    /// Whether the station's slot still holds the last spent subject. Nothing
    /// more is spent into it until a body carries that one away.
    pub(crate) fn study_slot_full(&self) -> bool {
        self.study_station()
            .and_then(|s| self.world.get::<StudyYield>(s))
            .is_some_and(|held| held.0.is_some())
    }

    /// Why a subject-gated project may not complete right now, in the text
    /// the HUD shows after "needs". One door for `settle_research`'s gate and
    /// `research_material_shortfall`. Rack room is no reason: the floor
    /// beside the station is always somewhere to go.
    pub(crate) fn study_yield_blocker(&self) -> Option<&'static str> {
        self.study_slot_full()
            .then_some("crew to carry the last program away")
    }

    /// Leaves `program` standing at `station`.
    pub(crate) fn hold_at_station(&mut self, station: Entity, program: DownedProgram) {
        self.world
            .entity_mut(station)
            .insert(StudyYield(Some(program)));
    }

    /// Puts a program somewhere it survives, and never in the player's pack
    /// (research does not use it): the first rack with room, else the first
    /// free stockpile cell, else the floor around `at`.
    ///
    /// **The last rung is the floor, not a log line saying it is lost.** A
    /// claimed rack's room can shrink under the claim (a demolition, a rig
    /// fetcher's kill sheltered onto a rack, `stow_program`'s overflow), so
    /// "no rack has room" is reachable and the kill must still outlast it.
    /// `Game::set_down` is the door that cannot fail; a floor program is
    /// picked up by hand like any other.
    ///
    /// **The one exception to "never the pack":** with no free floor cell
    /// `set_down` falls back to `stow_program`, which fills the player's
    /// downed store (or a rack, or the store past its cap). Very hard to
    /// reach, and the log says which it was.
    pub(crate) fn shelter_program(&mut self, held: DownedProgram, at: Position) {
        let label = self.downed_program_label(&held);
        if let Some(rack) = self
            .quarantine_racks()
            .into_iter()
            .map(|(_, _, e)| e)
            .find(|rack| self.rack_room(*rack) > 0)
            && let Some(mut shelf) = self.world.get_mut::<Racked>(rack)
        {
            shelf.0.push(held);
            return;
        }
        if let Some(cell) = self
            .free_stockpile_cells()
            .and_then(|cells| cells.first().copied())
        {
            self.log_base(format!(
                "No rack has room — {label} is set down on a stockpile."
            ));
            self.set_down(cell, vec![FloorThing::Program(held)]);
            return;
        }
        // With no free floor cell `set_down` falls back to `stow_program`,
        // which says where the program really went; claiming the floor first
        // would be a lie.
        if !self.free_floor_cells(at, 1).is_empty() {
            self.log_base(format!(
                "No rack has room — {label} is set down on the floor."
            ));
        }
        self.set_down(at, vec![FloorThing::Program(held)]);
    }

    /// A destroyed station's held program, put somewhere it survives. Called
    /// at both destruction paths, beside `return_rig_tool`.
    pub(crate) fn return_study_yield(&mut self, station: Entity) {
        // Read before the take: a station without a tile keeps its slot
        // rather than losing what it cannot place.
        let Some(at) = self.world.get::<Position>(station).copied() else {
            return;
        };
        let Some(held) = self
            .world
            .get_mut::<StudyYield>(station)
            .and_then(|mut slot| slot.0.take())
        else {
            return;
        };
        self.shelter_program(held, at);
    }

    /// `None` when nothing is held at a station and nobody is walking one:
    /// no body is asked.
    pub(crate) fn rack_board(&mut self) -> Option<RackBoard> {
        let mut stations: Vec<(Entity, Position)> = self
            .world
            .query::<(Entity, &Position, &StudyYield)>()
            .iter(&self.world)
            .filter(|(_, _, held)| held.0.is_some())
            .map(|(e, p, _)| (e, *p))
            .collect();
        let in_transit = self.programs_in_transit();
        if stations.is_empty() && in_transit == 0 {
            return None;
        }
        stations.sort_by_key(|(_, p)| (p.x, p.y));
        let spare = self.total_rack_room().saturating_sub(in_transit);
        Some(RackBoard {
            stations,
            spare,
            claimed: Vec::new(),
        })
    }

    /// The gates the other idle errands share, less their `CarryingProgram`
    /// exclusion: this errand is the one that body belongs to. A hauler's or
    /// clearer's `Carrying` and a tidier's item are somebody else's trip.
    pub(crate) fn may_rack(&self, worker: Entity, amenities: &Amenities) -> bool {
        let Ok(w) = self.world.get_entity(worker) else {
            return false;
        };
        let on_floor = w
            .get::<Position>()
            .is_some_and(|p| self.world.resource::<BaseGrid>().is_floor(p.x, p.y));
        on_floor
            && w.get::<Task>().is_none()
            && w.get::<UnderStudy>().is_none()
            && w.get::<Tidying>().is_none()
            && w.get::<Carrying>().is_none()
            && self.is_on_shift(worker, amenities)
    }

    /// The racking arm of `drift_idle_staff`: the gate and the beat in one
    /// call, so the carrier arm above the need arms and the pick-up arm
    /// below them cannot gate differently. `board` is `None` when nothing is
    /// held or in transit.
    pub(crate) fn try_rack(
        &mut self,
        worker: Entity,
        amenities: &Amenities,
        board: &mut Option<RackBoard>,
    ) -> bool {
        board.is_some()
            && self.may_rack(worker, amenities)
            && board.as_mut().is_some_and(|b| self.rack_step(worker, b))
    }

    /// Counts the labour passes a rack carrier has stood on one tile, and
    /// past `tuning::RACK_CARRY_PATIENCE_BEATS` hands the program back.
    ///
    /// A carrier is held out of the posting pool for the whole trip, and a
    /// walk is allowed to *wait* (`walk_idle_toward` returns `Ok` for a body
    /// or the party in the way), so without a bound a stuck body would be
    /// lost to labour for ever. It gives the kill back the way a carrier with
    /// no reachable rack does, and is free again.
    pub(crate) fn rack_carrier_patience(&mut self, worker: Entity) {
        let Some(here) = self.world.get::<Position>(worker).copied() else {
            return;
        };
        if !self.is_rack_carrier(worker) {
            self.world.entity_mut(worker).remove::<RackWait>();
            return;
        }
        let beats = match self.world.get::<RackWait>(worker) {
            Some(wait) if wait.at == here => wait.beats + 1,
            _ => 0,
        };
        if beats >= crate::tuning::RACK_CARRY_PATIENCE_BEATS {
            self.world.entity_mut(worker).remove::<RackWait>();
            self.put_carried_program_back(worker);
            return;
        }
        self.world
            .entity_mut(worker)
            .insert(RackWait { at: here, beats });
    }

    /// One beat of the errand. Returns whether the body spent it.
    pub(crate) fn rack_step(&mut self, worker: Entity, board: &mut RackBoard) -> bool {
        let Some(here) = self.world.get::<Position>(worker).copied() else {
            return false;
        };
        let holding = self.world.get::<CarryingProgram>(worker).is_some();
        if !holding && board.stations.is_empty() {
            return false;
        }
        // Gathered before the reach field borrows the grid.
        let stockpile = self.free_stockpile_cells().unwrap_or_default();
        let beside = self
            .study_station()
            .and_then(|s| self.world.get::<Position>(s).copied())
            .map(|at| self.free_floor_cells(at, BESIDE_STATION_CELLS))
            .unwrap_or_default();
        let blocked = self.blocked_tiles().rigid();
        let grid = self.world.resource::<BaseGrid>();
        let radius = grid.radius();
        let reach = crew_reach(grid, here, &blocked, radius);
        let reachable = |p: Position| reaches(grid, &reach, here, p, 1, &blocked, radius);
        let open = |p: Position| reachable(p) && !board.claimed.contains(&p);
        // A pick-up may only count rack room nobody walking is promised.
        let rack = (holding || board.spare > 0)
            .then(|| {
                self.quarantine_racks()
                    .into_iter()
                    .filter(|&(_, _, rack)| self.rack_room(rack) > 0)
                    .map(|(x, y, rack)| (Position { x, y }, rack))
                    .filter(|&(p, _)| reachable(p))
                    .min_by_key(|&(p, _)| (chebyshev(here, p), p.x, p.y))
            })
            .flatten()
            .map(|(p, rack)| ProgramDrop::Rack(p, rack));
        let destination = rack
            .or_else(|| {
                stockpile
                    .into_iter()
                    .filter(|&p| open(p))
                    .min_by_key(|&p| (chebyshev(here, p), p.x, p.y))
                    .map(ProgramDrop::Floor)
            })
            .or_else(|| {
                beside
                    .into_iter()
                    .map(|(x, y)| Position { x, y })
                    .find(|&p| open(p))
                    .map(ProgramDrop::Floor)
            });
        if let Some(ProgramDrop::Floor(p)) = destination {
            board.claimed.push(p);
        }
        if holding {
            return self.rack_deliver(worker, here, destination);
        }
        // Destination before pickup.
        let Some(destination) = destination else {
            return false;
        };
        let Some((index, target)) = board
            .stations
            .iter()
            .enumerate()
            .map(|(i, &(_, p))| (i, p))
            .filter(|&(_, p)| reachable(p))
            .min_by_key(|&(_, p)| (chebyshev(here, p), p.x, p.y))
        else {
            return false;
        };
        if !at_station(here, target, 1) {
            return self.walk_idle_toward(worker, here, target).is_ok();
        }
        let (station, _) = board.stations.remove(index);
        let taken = self
            .world
            .get_mut::<StudyYield>(station)
            .and_then(|mut slot| slot.0.take());
        if let Some(program) = taken {
            self.world
                .entity_mut(worker)
                .insert(CarryingProgram(program));
            if matches!(destination, ProgramDrop::Rack(..)) {
                board.spare -= 1;
            }
        }
        true
    }

    /// The held half: walk to the destination and leave the program there —
    /// shelved on a rack or set down on its cell — or set it back when there
    /// is none.
    fn rack_deliver(
        &mut self,
        worker: Entity,
        here: Position,
        destination: Option<ProgramDrop>,
    ) -> bool {
        let Some(destination) = destination else {
            self.put_carried_program_back(worker);
            return true;
        };
        let pos = destination.at();
        if !at_station(here, pos, 1) {
            if self.walk_idle_toward(worker, here, pos).is_err() {
                self.put_carried_program_back(worker);
            }
            return true;
        }
        let Some(held) = self
            .world
            .entity_mut(worker)
            .take::<CarryingProgram>()
            .map(|c| c.0)
        else {
            return true;
        };
        let label = self.downed_program_label(&held);
        match destination {
            ProgramDrop::Rack(_, rack) => {
                if let Some(mut shelf) = self.world.get_mut::<Racked>(rack) {
                    shelf.0.push(held);
                }
                self.log_base(format!("{label} is racked."));
            }
            ProgramDrop::Floor(cell) => {
                self.set_down(cell, vec![FloorThing::Program(held)]);
                self.log_base(format!("{label} is set down."));
            }
        }
        true
    }

    /// A carrier with nowhere reachable to deliver sets its load back on the
    /// station slot, where the gate reports it; failing that, anywhere it
    /// survives. A body is never left holding what it cannot
    /// place, and the kill is never dropped.
    fn put_carried_program_back(&mut self, worker: Entity) {
        let Some(at) = self.world.get::<Position>(worker).copied() else {
            return;
        };
        let Some(held) = self
            .world
            .entity_mut(worker)
            .take::<CarryingProgram>()
            .map(|c| c.0)
        else {
            return;
        };
        // The slot is a holding place for an errand that reached nothing,
        // not one of `shelter_program`'s destinations.
        let slot = self.study_station().filter(|&s| {
            self.world
                .get::<StudyYield>(s)
                .is_none_or(|held| held.0.is_none())
        });
        match slot {
            Some(station) => self.hold_at_station(station, held),
            None => self.shelter_program(held, at),
        }
    }
}
