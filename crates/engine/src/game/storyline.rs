//! The shell around the storyline arcs (ADR 0042): gather a plain input
//! from the world, hand it to the pure `storyline::step`, apply its outcome.

use crate::alerts::AlertKind;
use crate::base_grid::BaseGrid;
use crate::components::{BaseAiHost, Downed, MachineRecipe, ProgramId, Task};
use crate::game::base::hauling::{self, Step, Touch};
use crate::storyline::{
    BaseAi, FusedAi, HostCandidate, StationSpot, StepInput, StepOutcome, Storyline, TrainingTarget,
    analysis_cap, pace, practise, step, training_cost, trigger_count,
};
use crate::systems::{CycleModifiers, CyclePace, expected_gather_per_tick, node_cycle_shape};
use crate::tuning::{BASE_AI_FUGUE_TICKS, BASE_AI_PRACTICE_PER_TICK};
use crate::views::BaseAiView;

const BASE_AI_ALERT_SUBJECT: &str = "base-ai";
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

    /// Re-marks the host after a load: only the arc is saved, the marker
    /// is derived from it.
    pub(crate) fn restore_base_ai_host(&mut self) {
        if let Some(host) = self.base_ai_host() {
            self.world.entity_mut(host).insert(BaseAiHost);
        }
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

    /// Whether the base AI has fused: from then on its station is the only
    /// Research Station the base may have or study at.
    pub(crate) fn base_ai_fused(&self) -> bool {
        matches!(self.storyline.base_ai, BaseAi::Fused(_))
    }

    /// Lets the storyline arcs leave `Dormant` or holds them there, for this
    /// run only: a bench switch, so a template that already has the research
    /// and a station is measured without the awakening. Not saved.
    #[doc(hidden)]
    pub fn dev_set_storyline(&mut self, on: bool) {
        self.world
            .insert_resource(crate::resources::DevStoryline(on));
    }

    fn storyline_enabled(&self) -> bool {
        self.world
            .get_resource::<crate::resources::DevStoryline>()
            .is_none_or(|dev| dev.0)
    }

    /// One tick of every storyline arc. After `schedule_base_labour` so a
    /// claim can release what the scheduler just posted, and before
    /// `settle_research`.
    pub(crate) fn advance_storyline(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if matches!(self.storyline.base_ai, BaseAi::Fused(_)) {
            self.tick_fused_ai();
            return;
        }
        // Only the opening is held: an arc a save already carries keeps going.
        if !self.storyline_enabled() && matches!(self.storyline.base_ai, BaseAi::Dormant { .. }) {
            return;
        }
        let input = self.gather_base_ai();
        let outcome = step(&self.storyline.base_ai.clone(), &input.as_step_input());
        self.apply_base_ai(outcome);
        self.walk_claiming_host();
    }

    /// One tick of the fused AI: its pace, whole units credited, and the
    /// practice that grows its Analysis. The host has no `Task`, so
    /// `task_progress_system` never credits it; this is the only researcher.
    fn tick_fused_ai(&mut self) {
        let BaseAi::Fused(mut fused) = self.storyline.base_ai.clone() else {
            return;
        };
        let Some(host) = self.program_entity(fused.host) else {
            return;
        };
        let analysis = self.analysis_of_program(host);
        fused.carry += pace(&fused, analysis);
        let units = fused.carry.floor();
        fused.carry -= units;
        let room = self.credit_ai_research(&mut fused, units as u32);
        if self.training_is_ready(&fused) {
            self.complete_training(&mut fused);
        } else if room {
            let outcome = practise(&fused, analysis, BASE_AI_PRACTICE_PER_TICK);
            fused.practice = outcome.practice;
            if outcome.analysis != analysis
                && let Some(mut attributes) = self.world.get_mut::<components::Attributes>(host)
            {
                attributes.set(
                    &crate::attributes::AttributeId::from("analysis"),
                    outcome.analysis,
                );
            }
            if outcome.stalled && analysis < analysis_cap(&fused) {
                self.post_alert(
                    AlertKind::BaseAiWaiting,
                    BASE_AI_ALERT_SUBJECT,
                    "The base AI has learned all it can on its own and is waiting to be trained.",
                );
            }
        }
        self.storyline.base_ai = BaseAi::Fused(fused);
    }

    /// Whether the selected training has its full cost and a pinned program
    /// standing in the pen to spend on it.
    fn training_is_ready(&self, fused: &FusedAi) -> bool {
        fused.training.selected
            && fused.training.progress >= training_cost(fused.level)
            && self.study_yield_blocker().is_none()
            && self.pinned_subject().is_some()
    }

    /// Spends the pinned program, adds a level and lets practice resume.
    fn complete_training(&mut self, fused: &mut FusedAi) {
        let Some(subject) = self.pinned_subject() else {
            return;
        };
        self.spend_subject(subject);
        fused.level += 1;
        fused.practice = 0;
        fused.training = TrainingTarget::default();
        self.base_ai_level_up(fused.level);
    }

    fn base_ai_level_up(&mut self, level: u32) {
        self.clear_alerts_of(&AlertKind::BaseAiWaiting);
        self.notify_filled(
            crate::notifications::NotificationKind::BaseAiLevelUp,
            &[("level", &level.to_string())],
            None,
        );
    }

    /// Chooses "Train your AI" as the one thing the AI works on. Refused
    /// until it has stalled at its threshold. Progress on a project is kept.
    pub fn select_ai_training(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(view) = self.base_ai_view() else {
            return Err("There is no AI to train.".into());
        };
        if !view.training_available {
            return Err("The AI hasn't reached the end of what it can learn alone.".into());
        }
        if view.training_selected {
            return Err("The AI is already being trained.".into());
        }
        if let BaseAi::Fused(fused) = &mut self.storyline.base_ai {
            fused.training.selected = true;
        }
        self.world
            .resource_mut::<crate::resources::ActiveResearch>()
            .id = None;
        self.refresh_research_reach();
        self.log_base("Training the base AI.");
        Ok(())
    }

    /// Fuses the first base staff program with the first Research Station at
    /// level 0, as if the arc had run, and with `stalled` leaves it at its
    /// first threshold. For a test or a hand-built state whose subject is
    /// what comes after the fusion; the arc itself never calls it.
    #[doc(hidden)]
    pub fn dev_fuse_base_ai(&mut self, stalled: bool) -> Result<(), String> {
        let host = *self
            .base_staff()
            .first()
            .ok_or("no base staff program to fuse")?;
        let station = *self
            .research_stations()
            .first()
            .ok_or("no Research Station to fuse with")?;
        let anchor = self
            .world
            .get::<Position>(station)
            .map(|p| (p.x, p.y))
            .ok_or("the station has no position")?;
        let analysis = self.analysis_of_program(host);
        let id = *self
            .world
            .get::<ProgramId>(host)
            .ok_or("the host has no program id")?;
        let pace_at_fusion = self.host_research_pace(host, anchor);
        let fused = FusedAi {
            host: id,
            station: anchor,
            level: 0,
            analysis_at_fusion: analysis,
            pace_at_fusion,
            practice: 0,
            carry: 0.0,
            training: TrainingTarget::default(),
        };
        let cap = analysis_cap(&fused);
        self.world.entity_mut(host).insert(BaseAiHost);
        self.storyline.base_ai = BaseAi::Fused(fused);
        if stalled && let Some(mut attributes) = self.world.get_mut::<components::Attributes>(host)
        {
            attributes.set(&crate::attributes::AttributeId::from("analysis"), cap);
        }
        Ok(())
    }

    /// The base AI as the research screen shows it; `None` before it fuses.
    pub fn base_ai_view(&self) -> Option<BaseAiView> {
        let BaseAi::Fused(fused) = &self.storyline.base_ai else {
            return None;
        };
        let analysis = self
            .program_entity(fused.host)
            .map_or(fused.analysis_at_fusion, |e| self.analysis_of_program(e));
        let cap = analysis_cap(fused);
        let stalled = analysis >= cap;
        Some(BaseAiView {
            level: fused.level,
            analysis,
            next_threshold: (cap != i32::MAX).then_some(cap),
            stalled,
            training_available: stalled,
            training_selected: fused.training.selected,
            training_progress: fused.training.progress,
            training_cost: training_cost(fused.level),
        })
    }

    /// Sends `units` to training, the running project or the study bank, and
    /// reports whether any of them had room to take one.
    fn credit_ai_research(&mut self, fused: &mut FusedAi, units: u32) -> bool {
        if fused.training.selected {
            let cost = training_cost(fused.level);
            let room = fused.training.progress < cost;
            fused.training.progress = (fused.training.progress + units).min(cost);
            return room;
        }
        self.world.resource_scope(
            |world, mut research: Mut<crate::resources::ActiveResearch>| {
                crate::systems::credit_research(
                    units,
                    &mut research,
                    world.resource::<crate::research::ResearchDb>(),
                )
                .room
            },
        )
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
    /// study, not on a sortie: `ProgramRole::Staff` (or the host's own
    /// `BaseAi`, once marked) and standing.
    fn is_free_for_claim(&self, program: Entity) -> bool {
        matches!(
            self.program_role(program),
            Some(ProgramRole::Staff | ProgramRole::BaseAi)
        ) && self.world.get::<Downed>(program).is_none()
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

    pub(crate) fn analysis_of_program(&self, program: Entity) -> i32 {
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
                self.release_subjects_outside_ai_station();
                self.announce_fusion();
            }
        }
    }

    /// Frees every subject pinned at a Research Station other than the AI's,
    /// in anchor order, so the research it was spending is dropped the same
    /// way every run.
    fn release_subjects_outside_ai_station(&mut self) {
        let ai = self.base_ai_station();
        let mut others: Vec<(Entity, (i32, i32))> = self
            .research_stations()
            .into_iter()
            .filter(|&e| Some(e) != ai)
            .filter_map(|e| Some((e, self.world.get::<Position>(e).map(|p| (p.x, p.y))?)))
            .collect();
        others.sort_by_key(|&(_, anchor)| anchor);
        for (station, _) in others {
            self.release_study_subject(station, "the base AI has taken the research");
        }
    }

    fn announce_awakening(&mut self, host: ProgramId) {
        let name = self
            .program_entity(host)
            .map_or_else(|| "a program".to_string(), |e| self.creature_label(e));
        self.notify_filled(
            crate::notifications::NotificationKind::BaseAiAwakening,
            &[("name", &name)],
            None,
        );
    }

    fn announce_fusion(&mut self) {
        self.notify(crate::notifications::NotificationKind::BaseAiFused);
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
