//! The shell around the storyline arcs (ADR 0042): gather a plain input
//! from the world, hand it to the pure `storyline::step`, apply its outcome.

use crate::base_grid::BaseGrid;
use crate::components::{BaseAiHost, Downed, MachineRecipe, ProgramId, Task};
use crate::game::base::hauling::{self, Step, Touch};
use crate::storyline::{
    BaseAi, HostCandidate, StationSpot, StepInput, StepOutcome, Storyline, step, trigger_count,
};
use crate::systems::{CycleModifiers, CyclePace, expected_gather_per_tick, node_cycle_shape};
use crate::tuning::BASE_AI_FUGUE_TICKS;
use crate::*;

impl Game {
    pub fn storyline(&self) -> &Storyline {
        &self.storyline
    }

    /// The entity of the program that carries `BaseAiHost`, once a claim has
    /// begun.
    pub(crate) fn base_ai_host(&self) -> Option<Entity> {
        let id = match &self.storyline.base_ai {
            BaseAi::Dormant { .. } => return None,
            BaseAi::Claiming { host, .. } | BaseAi::Fugue { host, .. } => *host,
            BaseAi::Fused(f) => f.host,
        };
        self.program_entity(id)
    }

    /// The `studies` structure standing on the arc's anchor tile.
    pub(crate) fn base_ai_station(&self) -> Option<Entity> {
        let anchor = match &self.storyline.base_ai {
            BaseAi::Dormant { .. } => return None,
            BaseAi::Claiming { station, .. } | BaseAi::Fugue { station, .. } => *station,
            BaseAi::Fused(f) => f.station,
        };
        self.station_entity_at(anchor)
    }

    /// Whether the arc has left `Dormant`: from then on the labour scheduler
    /// posts nobody to a Research Station.
    pub(crate) fn storyline_owns_research(&self) -> bool {
        !matches!(self.storyline.base_ai, BaseAi::Dormant { .. })
    }

    /// One tick of every storyline arc. After `schedule_base_labour` so a
    /// claim can release what the scheduler just posted, and before
    /// `settle_research`.
    pub(crate) fn advance_storyline(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let input = self.gather_base_ai();
        let outcome = step(&self.storyline.base_ai.clone(), &input.as_step_input());
        self.apply_base_ai(outcome);
        self.walk_claiming_host();
    }

    fn gather_base_ai(&mut self) -> BaseAiGather {
        let researched = self.world.resource::<Research>().0.len() as u32;
        let trigger = trigger_count(self.world.resource::<WorldMap>().seed());
        let mut gather = BaseAiGather {
            researched,
            trigger,
            candidates: Vec::new(),
            stations: Vec::new(),
            host_free: false,
            host_arrived: false,
            host_analysis: 0,
            host_pace: 0.0,
        };
        match self.storyline.base_ai.clone() {
            BaseAi::Dormant { .. } if researched >= trigger => {
                gather.candidates = self.host_candidates();
                let nearest_to = crate::storyline::pick_host(&gather.candidates)
                    .and_then(|id| self.program_entity(id))
                    .and_then(|e| self.world.get::<Position>(e).copied());
                gather.stations = self.station_spots(nearest_to);
            }
            BaseAi::Dormant { .. } => {}
            BaseAi::Claiming { host, station } => {
                if let Some(e) = self.program_entity(host) {
                    gather.host_free = self.is_free_for_claim(e);
                    gather.host_arrived = self.host_at_station(e, station);
                }
            }
            BaseAi::Fugue {
                host,
                station,
                ticks_left: 0,
            } => {
                if let Some(e) = self.program_entity(host) {
                    gather.host_analysis = self.analysis_of_program(e);
                    gather.host_pace = self.host_research_pace(e, station);
                }
            }
            BaseAi::Fugue { .. } | BaseAi::Fused(_) => {}
        }
        gather
    }

    /// At the base, not in the party, not downed, not jailed, not under
    /// study, not on a sortie: `ProgramRole::Staff` and standing.
    fn is_free_for_claim(&self, program: Entity) -> bool {
        self.program_role(program) == Some(ProgramRole::Staff)
            && self.world.get::<Downed>(program).is_none()
    }

    fn host_candidates(&mut self) -> Vec<HostCandidate> {
        let mut candidates: Vec<HostCandidate> = self
            .base_staff()
            .into_iter()
            .filter_map(|e| {
                Some(HostCandidate {
                    id: *self.world.get::<ProgramId>(e)?,
                    analysis: self.analysis_of_program(e),
                    free: self.is_free_for_claim(e),
                })
            })
            .collect();
        candidates.sort_by_key(|c| c.id);
        candidates
    }

    fn analysis_of_program(&self, program: Entity) -> i32 {
        let species = self
            .world
            .get::<Creature>(program)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species));
        crate::species::analysis_of(
            self.world.get::<components::Attributes>(program),
            species,
            self.world.resource::<crate::attributes::AttributeDb>(),
        )
    }

    /// Every Research Station by anchor tile, with its walking distance from
    /// `from` (zero when there is nobody to measure from).
    fn station_spots(&self, from: Option<Position>) -> Vec<StationSpot> {
        let mut spots: Vec<StationSpot> = self
            .research_stations()
            .into_iter()
            .filter_map(|e| self.world.get::<Position>(e))
            .map(|p| StationSpot {
                anchor: (p.x, p.y),
                distance: from.map_or(0, |f| (f.x - p.x).abs().max((f.y - p.y).abs()) as u32),
            })
            .collect();
        spots.sort_by_key(|s| s.anchor);
        spots
    }

    fn station_entity_at(&self, anchor: (i32, i32)) -> Option<Entity> {
        self.research_stations()
            .into_iter()
            .find(|&e| self.world.get::<Position>(e).map(|p| (p.x, p.y)) == Some(anchor))
    }

    /// Arrival is derived from position, as `UnderStudy`'s is.
    fn host_at_station(&self, host: Entity, anchor: (i32, i32)) -> bool {
        let (Some(station), Some(here)) = (
            self.station_entity_at(anchor),
            self.world.get::<Position>(host).copied(),
        ) else {
            return false;
        };
        let at = *self.world.get::<Position>(station).unwrap();
        hauling::at_station(here, at, self.structure_footprint_of(station), Touch::Sides)
    }

    /// Research units per tick `host` would make at the station on the
    /// ordinary formula: the same pieces a posted program's cycle uses, over
    /// the same cycle length.
    pub(crate) fn host_research_pace(&mut self, host: Entity, anchor: (i32, i32)) -> f64 {
        let Some(station) = self.station_entity_at(anchor) else {
            return 0.0;
        };
        let speed = self.species_base_speed(host);
        let ticks = self.work_ticks_for(station, speed);
        let species = self
            .world
            .get::<Creature>(host)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species));
        let catalogue = self.world.resource::<crate::attributes::AttributeDb>();
        let keen_scavenger_level = self
            .world
            .get::<Perks>(self.player_entity())
            .map_or(0, |p| p.level(Perk::KeenScavenger));
        let worker = CycleModifiers {
            keen_scavenger_level,
            analysis_over_base: self.analysis_of_program(host) - catalogue.analysis_base(),
            class: species.and_then(|d| d.affinity_class()),
            morale: self.morale(host),
            need_strain: self.need_strain(host),
            extraction: 0.0,
        };
        let Some(node) = self.world.get::<ResourceNode>(station) else {
            return 0.0;
        };
        let structure_db = self.world.resource::<StructureDb>();
        let (flat_payout, tierless) = node_cycle_shape(
            self.world.get::<Structure>(station),
            self.world.get::<MachineRecipe>(station),
            structure_db,
        );
        let tier = self
            .world
            .get::<components::StructureTier>(station)
            .filter(|_| !tierless);
        let steps_per_tick = f64::from(crate::game::base::skill_scale::scale_of(
            self.world.get::<crate::skills::Skills>(host),
            Some(crate::skills::Skill::Research),
        ));
        expected_gather_per_tick(
            node,
            tier,
            *self.world.resource::<ZoneLevel>(),
            flat_payout,
            worker,
            self.world.resource::<ItemDb>(),
            CyclePace {
                ticks,
                steps_per_tick,
            },
        )
    }

    fn apply_base_ai(&mut self, outcome: StepOutcome) {
        match outcome {
            StepOutcome::None => {}
            StepOutcome::BeginClaim {
                host,
                station,
                announce,
            } => {
                self.begin_claim(host, station);
                self.storyline.base_ai = BaseAi::Claiming { host, station };
                if announce {
                    self.announce_awakening(host);
                }
            }
            StepOutcome::AbandonClaim => {
                if let Some(host) = self.base_ai_host() {
                    self.world.entity_mut(host).remove::<BaseAiHost>();
                }
                self.storyline.base_ai = BaseAi::Dormant { announced: true };
            }
            StepOutcome::StartFugue => {
                if let BaseAi::Claiming { host, station } = self.storyline.base_ai {
                    self.storyline.base_ai = BaseAi::Fugue {
                        host,
                        station,
                        ticks_left: BASE_AI_FUGUE_TICKS,
                    };
                }
            }
            StepOutcome::TickFugue => {
                if let BaseAi::Fugue { ticks_left, .. } = &mut self.storyline.base_ai {
                    *ticks_left -= 1;
                }
            }
            StepOutcome::Fuse(fused) => {
                self.storyline.base_ai = BaseAi::Fused(fused);
                self.announce_fusion();
            }
        }
    }

    /// Stub: Task 3 replaces both announcements with Storyline
    /// notifications.
    fn announce_awakening(&mut self, host: ProgramId) {
        let name = self
            .program_entity(host)
            .map_or_else(|| "a program".to_string(), |e| self.creature_label(e));
        self.log_base(format!("Something is going wrong with {name}."));
    }

    fn announce_fusion(&mut self) {
        self.log_base("The program seems to have fused with the Research Station.");
    }

    /// Marks `host`, drops what it was doing, and frees every program posted
    /// at a Research Station: nothing researches until the fusion.
    fn begin_claim(&mut self, host: ProgramId, _station: (i32, i32)) {
        let stations = self.research_stations();
        let posted: Vec<Entity> = self
            .world
            .query::<(Entity, &Task)>()
            .iter(&self.world)
            .filter(|(_, t)| stations.contains(&t.target))
            .map(|(e, _)| e)
            .collect();
        for worker in posted {
            self.drop_load(worker);
            self.world.entity_mut(worker).remove::<Task>();
        }
        if let Some(e) = self.program_entity(host) {
            self.drop_load(e);
            self.world.entity_mut(e).insert(BaseAiHost).remove::<Task>();
        }
    }

    /// One step of the claimed program toward its station, by the walk a
    /// posted worker takes. A route that does not exist holds it still.
    fn walk_claiming_host(&mut self) {
        let BaseAi::Claiming { host, .. } = self.storyline.base_ai else {
            return;
        };
        let (Some(host), Some(station)) = (self.program_entity(host), self.base_ai_station())
        else {
            return;
        };
        let (Some(here), Some(at)) = (
            self.world.get::<Position>(host).copied(),
            self.world.get::<Position>(station).copied(),
        ) else {
            return;
        };
        let side = self.structure_footprint_of(station);
        if hauling::at_station(here, at, side, Touch::Sides) {
            return;
        }
        let blocked = self.blocked_tiles().rigid();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let Ok(Some(Step { to, .. })) = hauling::step_to_post(
            self.world.resource::<BaseGrid>(),
            here,
            at,
            side,
            &blocked,
            pocket_radius,
            Touch::Sides,
        ) else {
            return;
        };
        if self.field_pos() == Some((to.x, to.y)) {
            return;
        }
        if let Some(mut pos) = self.world.get_mut::<Position>(host) {
            *pos = to;
        }
    }
}

/// The plain input one tick gathers, owning what `StepInput` borrows.
struct BaseAiGather {
    researched: u32,
    trigger: u32,
    candidates: Vec<HostCandidate>,
    stations: Vec<StationSpot>,
    host_free: bool,
    host_arrived: bool,
    host_analysis: i32,
    host_pace: f64,
}

impl BaseAiGather {
    fn as_step_input(&self) -> StepInput<'_> {
        StepInput {
            researched: self.researched,
            trigger: self.trigger,
            candidates: &self.candidates,
            stations: &self.stations,
            host_free: self.host_free,
            host_arrived: self.host_arrived,
            host_analysis: self.host_analysis,
            host_pace: self.host_pace,
        }
    }
}
