//! The Holding Cell: a downed program revived as a prisoner in a pen.
//!
//! A prisoner is a live, owned body carrying `components::Jailed`, which
//! `role_of` reads as `ProgramRole::Jailed`. It is spawned on the pen cell
//! (`study::pen_corner`, the corner a Research Station shares) and never
//! moves, so unlike a pinned subject nothing has to walk it there. A cell's
//! occupant is derived with a query (`Game::cell_prisoner`), never stored on
//! the cell.

use crate::base_grid::BaseGrid;
use crate::game::base::collect::ORTHOGONAL;
use crate::game::base::study::pen_corner;
use crate::game::extraction::extraction_band_with_bench;
use crate::game::spawning::SpawnPins;
use crate::items::DownedProgram;
use crate::resources::{GameRng, MessageKind, PowerGrid};
use crate::taming::{TargetResistance, capture_chance};
use crate::tools::{ToolCategory, ToolDb, ToolDef};
use crate::tuning::{
    DEFAULT_TAMING_DIFFICULTY, JAIL_BASE_POTENCY, JAIL_BREAKDOWN_SCALE, JAIL_MAX_ATTEMPTS,
};
use crate::*;
use rand::RngExt;

/// Why `index` can't be jailed right now — `Game::jail_blocker`'s exhaustive
/// answer. `row_fragment` is the few words an action row appends after its
/// label, `refusal` the full sentence `Game::jail_program` errs with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JailBlock {
    NotNow,
    NoSuchRecord,
    Boss,
    NoProtocol,
    RosterFull,
    UnknownSpecies,
    NoFreeCell,
}

impl JailBlock {
    pub fn row_fragment(self) -> &'static str {
        match self {
            Self::NotNow => "not now",
            Self::NoSuchRecord => "no such record",
            Self::Boss => "a boss won't reinitialize",
            Self::NoProtocol => "no protocol held",
            Self::RosterFull => "roster is full",
            Self::UnknownSpecies => "unknown species",
            Self::NoFreeCell => "no free Holding Cell beside you",
        }
    }

    pub fn refusal(self) -> &'static str {
        match self {
            Self::NotNow => "Can't do that right now.",
            Self::NoSuchRecord => "No such downed program.",
            Self::Boss => "A boss won't reinitialize.",
            Self::NoProtocol => "You need a Reinitialization Protocol.",
            Self::RosterFull => "Your roster is full.",
            Self::UnknownSpecies => "That species no longer exists.",
            Self::NoFreeCell => "Stand beside a Holding Cell with nobody in it.",
        }
    }
}

impl Game {
    /// The pen of a `holds_prisoner` structure, or `None` for anything else
    /// — `study_pen`'s counterpart, through the same `pen_corner`.
    pub(crate) fn prison_pen(&self, structure: Entity) -> Option<(i32, i32)> {
        let kind = self.world.get::<Structure>(structure)?.kind.clone();
        let def = self.world.resource::<StructureDb>().get(&kind)?;
        def.holds_prisoner.as_ref()?;
        let pos = self.world.get::<Position>(structure)?;
        Some(pen_corner(*pos, def.footprint))
    }

    /// Whoever is confined in `cell`, if anyone.
    pub fn cell_prisoner(&self, cell: Entity) -> Option<Entity> {
        self.world
            .iter_entities()
            .find(|e| {
                e.get::<components::Jailed>()
                    .is_some_and(|j| j.cell == cell)
            })
            .map(|e| e.id())
    }

    /// Every Holding Cell the party is orthogonally beside — beside any of
    /// its footprint cells, not only the anchor — in `(x, y)` order.
    fn adjacent_holding_cells(&self) -> Vec<Entity> {
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter(|e| self.prison_pen(e.id()).is_some())
            .filter_map(|e| {
                let anchor = *e.get::<Position>()?;
                let cells = crate::tactical::footprint_cells_at(
                    (anchor.x, anchor.y),
                    self.structure_footprint_of(e.id()),
                );
                ORTHOGONAL
                    .iter()
                    .any(|(dx, dy)| cells.contains(&(px + dx, py + dy)))
                    .then_some((anchor.x, anchor.y, e.id()))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// Whether anything is standing on `pen` or could not be: no floor under
    /// it, a body that walks the base on it, or the party itself.
    fn pen_is_taken(&self, pen: (i32, i32)) -> bool {
        if !self.world.resource::<BaseGrid>().is_floor(pen.0, pen.1) {
            return true;
        }
        if self.base_pos() == Some(pen) {
            return true;
        }
        self.world.iter_entities().any(|e| {
            let Some(pos) = e.get::<Position>() else {
                return false;
            };
            (pos.x, pos.y) == pen
                && e.get::<Tamed>().is_some()
                && crate::game::party::walks_the_base(
                    self.program_role(e.id()),
                    e.get::<Task>().map(|t| t.kind),
                )
        })
    }

    /// The first adjacent cell with nobody in it and a clear pen.
    fn free_holding_cell(&self) -> Option<Entity> {
        self.adjacent_holding_cells().into_iter().find(|&cell| {
            self.cell_prisoner(cell).is_none()
                && self
                    .prison_pen(cell)
                    .is_some_and(|pen| !self.pen_is_taken(pen))
        })
    }

    /// The one gate `jail_blocker` and `jail_program` share: the free cell
    /// the record would be pinned in, or the first refusal. In order: the
    /// run itself, the record, what it would cost, the roster, the species,
    /// then somewhere to put it.
    fn jail_gate(&self, index: usize) -> Result<Entity, JailBlock> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err(JailBlock::NotNow);
        }
        let player = self.player_entity();
        let record = self
            .world
            .get::<DownedPrograms>(player)
            .and_then(|held| held.0.get(index))
            .ok_or(JailBlock::NoSuchRecord)?;
        if record.boss {
            return Err(JailBlock::Boss);
        }
        let protocols = self.world.get::<Inventory>(player).map_or(0, |inv| {
            inv.count(&ItemId::from(crate::items::ids::REINITIALIZATION_PROTOCOL))
        });
        if protocols == 0 {
            return Err(JailBlock::NoProtocol);
        }
        if self.roster_room() == 0 {
            return Err(JailBlock::RosterFull);
        }
        if self
            .world
            .resource::<SpeciesDb>()
            .get(record.species.as_str())
            .is_none()
        {
            return Err(JailBlock::UnknownSpecies);
        }
        self.free_holding_cell().ok_or(JailBlock::NoFreeCell)
    }

    /// Why `index` can't be jailed right now, or `None` once every refusal
    /// clears — `jail_program`'s own gate, shared so an action row can grey
    /// on the check it spends against.
    pub fn jail_blocker(&self, index: usize) -> Option<JailBlock> {
        self.jail_gate(index).err()
    }

    /// Boots the downed program at `index` as a prisoner in the free Holding
    /// Cell beside the party, spending one Reinitialization Protocol and the
    /// record.
    ///
    /// **Refused whole through `jail_gate` before anything is written.** The
    /// body keeps its record's rarity and carried routine (`SpawnPins`, never
    /// re-rolled) and starts at level 1: `record.level` is the kill's zone, not
    /// a level worth restoring. It counts against roster room from here, so a
    /// later successful decompile can never fail on a full roster.
    pub fn jail_program(&mut self, index: usize) -> Result<(), String> {
        let cell = self.jail_gate(index).map_err(|b| b.refusal().to_string())?;
        let pen = self
            .prison_pen(cell)
            .ok_or_else(|| JailBlock::NoFreeCell.refusal().to_string())?;
        let player = self.player_entity();
        let record = self.world.get::<DownedPrograms>(player).unwrap().0[index].clone();
        // Filtered against `AbilityDb` before pinning: a mod that pulled the
        // ability out from under an old kill must not hand the body a
        // `Routines` entry every reader assumes resolves.
        let db = self.world.resource::<AbilityDb>();
        let carried: Vec<crate::abilities::AbilityId> = record
            .carried
            .iter()
            .filter(|id| db.get(id).is_some())
            .cloned()
            .collect();
        let pins = SpawnPins {
            rarity: Some(record.rarity),
            routines: Some(carried),
        };
        // The spawn is the last thing that can fail, so it precedes the spend.
        let Some(prisoner) =
            self.adopt_program_pinned(record.species.as_str(), pen.0, pen.1, 1.0, pins)
        else {
            return Err(JailBlock::UnknownSpecies.refusal().to_string());
        };
        self.world.get_mut::<Inventory>(player).unwrap().take(
            ItemId::from(crate::items::ids::REINITIALIZATION_PROTOCOL),
            1,
        );
        self.world
            .get_mut::<DownedPrograms>(player)
            .unwrap()
            .0
            .remove(index);
        self.world.entity_mut(prisoner).insert(components::Jailed {
            cell,
            attempts: 0,
            progress: 0,
            record: Some(record),
        });
        let label = self.creature_label(prisoner);
        self.log_kind(
            MessageKind::Outcome,
            format!("{label} boots up behind the bars of a Holding Cell."),
        );
        Ok(())
    }

    /// The chance of `prisoner`'s next decompile roll, or `None` if it isn't
    /// confined. **The only place a prisoner's `TargetResistance` is
    /// assembled**: the screen quotes this and the cell rolls against it.
    ///
    /// A full-Integrity, even-match target at `JAIL_BASE_POTENCY` — no
    /// catalyst is spent — priced with the player's own decompiler bonuses and
    /// `attempts` failures already behind it.
    pub fn jail_odds(&self, prisoner: Entity) -> Option<f32> {
        let jailed = self.world.get::<components::Jailed>(prisoner)?;
        let species = self.world.get::<Creature>(prisoner)?;
        let taming_difficulty = self
            .world
            .resource::<SpeciesDb>()
            .get(&species.species)
            .map_or(DEFAULT_TAMING_DIFFICULTY, |s| s.taming_difficulty);
        Some(capture_chance(
            JAIL_BASE_POTENCY,
            TargetResistance {
                hp_fraction: 1.0,
                taming_difficulty,
                prior_attempts: jailed.attempts,
                threat_ratio: 1.0,
            },
            self.player_decompiler_bonuses(),
        ))
    }
}

impl Game {
    /// Every standing Holding Cell, in tile order — `teardown_rigs`' reason:
    /// bevy's iteration order is not stable and two cells finishing in a
    /// different order between runs would reorder their log lines.
    fn holding_cells(&self) -> Vec<Entity> {
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter(|e| self.prison_pen(e.id()).is_some())
            .filter_map(|e| {
                let p = e.get::<Position>()?;
                Some((p.x, p.y, e.id()))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// Whether a worker holds the warden's post on `cell` — the same
    /// predicate `assembler_system` and `step_teardown_rig` staff on, a
    /// `TaskKind::GatherResource` aimed at the structure.
    pub(crate) fn cell_has_warden(&self, cell: Entity) -> bool {
        self.world.iter_entities().any(|e| {
            e.get::<Task>()
                .is_some_and(|t| t.target == cell && matches!(t.kind, TaskKind::GatherResource))
        })
    }

    /// Whether `cell` is making progress this beat: warded and lit. The brackets
    /// on the map read this, so a prisoner rattles exactly while the warden's
    /// clock is running.
    pub(crate) fn cell_is_working(&self, cell: Entity) -> bool {
        self.cell_has_warden(cell) && !self.world.resource::<PowerGrid>().is_dark(cell)
    }

    /// The `attempt_ticks` of `cell`'s def, or `None` for a structure that
    /// holds nobody.
    fn attempt_ticks_of(&self, cell: Entity) -> Option<u32> {
        let kind = &self.world.get::<Structure>(cell)?.kind;
        let def = self.world.resource::<StructureDb>().get(kind)?;
        Some(def.holds_prisoner.as_ref()?.attempt_ticks)
    }

    /// What the roster shows for `cell`, or `None` for a structure that is
    /// not a Holding Cell.
    pub fn prison_state(&self, cell: Entity) -> Option<crate::views::PrisonState> {
        let attempt_ticks = self.attempt_ticks_of(cell)?;
        let prisoner = self.cell_prisoner(cell).and_then(|body| {
            let jailed = self.world.get::<components::Jailed>(body)?;
            Some(crate::views::PrisonerState {
                name: self.creature_label(body),
                attempts: jailed.attempts,
                max_attempts: JAIL_MAX_ATTEMPTS,
                progress: jailed.progress,
                attempt_ticks,
                odds_percent: (self.jail_odds(body)? * 100.0).round() as u32,
            })
        });
        Some(crate::views::PrisonState {
            warded: self.cell_has_warden(cell),
            prisoner,
        })
    }

    /// Whether any Holding Cell stands in the base, beside the party or not.
    pub fn has_holding_cell(&self) -> bool {
        !self.holding_cells().is_empty()
    }

    /// One beat of every Holding Cell — `run_teardown_rigs`' shape and its
    /// place in the tick, after the schedule that writes `PowerGrid`.
    pub(crate) fn run_holding_cells(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        for cell in self.holding_cells() {
            self.step_holding_cell(cell);
        }
    }

    /// The gates mirror `step_teardown_rig`: a dark cell and an unstaffed one
    /// are left to `power_grid_system` and `idle_machine_system`, which write
    /// `Unpowered` and `Idle` (read as "No warden") over anything written
    /// here. The warden is the same predicate the rig's staffing uses — a
    /// worker holding `TaskKind::GatherResource` on the cell.
    ///
    /// **A cell with nobody inside draws no `GameRng`**, so a save with an
    /// empty cell keeps its stream.
    fn step_holding_cell(&mut self, cell: Entity) {
        if self.world.resource::<PowerGrid>().is_dark(cell) {
            return;
        }
        let warded = self.cell_has_warden(cell);
        if !warded {
            return;
        }
        // `Starved` and not `Idle` for an empty cell, `step_teardown_rig`'s
        // reason: `idle_machine_system` never reaches a staffed machine.
        let Some(prisoner) = self.cell_prisoner(cell) else {
            self.set_rig_status(cell, MachineStatus::Starved);
            return;
        };
        // A breakdown the cell had no room to pay is retried every beat,
        // not re-rolled: the last attempt already failed.
        if self.prisoner_is_spent(prisoner) {
            let status = if self.break_down_prisoner(cell, prisoner) {
                MachineStatus::Running
            } else {
                MachineStatus::Clogged
            };
            self.set_rig_status(cell, status);
            return;
        }
        self.set_rig_status(cell, MachineStatus::Running);
        let due = self.attempt_ticks_of(cell).unwrap_or(u32::MAX);
        let progress = {
            let mut jailed = self.world.get_mut::<components::Jailed>(prisoner).unwrap();
            jailed.progress += 1;
            jailed.progress
        };
        if progress < due {
            return;
        }
        self.world
            .get_mut::<components::Jailed>(prisoner)
            .unwrap()
            .progress = 0;
        let chance = self.jail_odds(prisoner).unwrap_or(0.0);
        let landed = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random::<f32>() < chance
        };
        self.settle_jail_attempt(cell, prisoner, landed);
        // A final failure the cell had no room to pay stays put: say so on
        // this beat rather than showing Running until the next retry.
        if self.prisoner_is_spent(prisoner) {
            self.set_rig_status(cell, MachineStatus::Clogged);
        }
    }

    /// Whether `prisoner` has used every attempt and is still in its cell —
    /// a breakdown the cell had no room to pay.
    fn prisoner_is_spent(&self, prisoner: Entity) -> bool {
        self.world
            .get::<components::Jailed>(prisoner)
            .is_some_and(|j| j.attempts >= JAIL_MAX_ATTEMPTS)
    }

    /// Writes one decompile attempt's outcome. Split from the roll so the
    /// outcome can be driven without a lucky seed.
    ///
    /// A success removes `Jailed` — the body is already a roster member from
    /// the pin, so nothing can fail on a full roster. A failure raises the
    /// next attempt's odds (`Jailed::attempts` feeds
    /// `TargetResistance::prior_attempts`), and the `JAIL_MAX_ATTEMPTS`th one
    /// breaks the program down.
    pub(crate) fn settle_jail_attempt(&mut self, cell: Entity, prisoner: Entity, landed: bool) {
        let label = self.creature_label(prisoner);
        if landed {
            self.world
                .entity_mut(prisoner)
                .remove::<components::Jailed>();
            self.log_kind(
                MessageKind::Outcome,
                format!("{label} accepts its new parameters and joins your roster."),
            );
            self.notify_filled(
                crate::notifications::NotificationKind::ProgramDecompiled,
                &[("name", &label)],
                None,
            );
            self.note_compiled();
            return;
        }
        let attempts = {
            let mut jailed = self.world.get_mut::<components::Jailed>(prisoner).unwrap();
            jailed.attempts += 1;
            jailed.attempts
        };
        if attempts >= JAIL_MAX_ATTEMPTS {
            self.break_down_prisoner(cell, prisoner);
        } else {
            self.log_base(format!(
                "The decompile of {label} slips ({attempts}/{JAIL_MAX_ATTEMPTS})."
            ));
        }
    }

    /// What a broken-down prisoner pays: `extraction_yield` at
    /// `JAIL_BREAKDOWN_SCALE` of `rolled` units (never fewer than one). Pure, and a *call* — the
    /// player's extraction, the rig and a failed cell price one program with
    /// one formula.
    pub fn breakdown_yield(
        &self,
        program: &DownedProgram,
        tool: &ToolDef,
        rolled: i32,
    ) -> Vec<(ItemId, u32)> {
        // At least one unit: a level-1 body's band is a handful of units, and
        // rounding 0.4 of it down would make most breakdowns pay nothing.
        let scaled = ((rolled as f32 * JAIL_BREAKDOWN_SCALE).round() as i32).max(1);
        self.extraction_yield(program, tool, scaled)
    }

    /// The tools a breakdown may be priced with, sorted by id (`ToolDb::all`),
    /// at tier 1. Image and Routine tools are skipped: they teach rather
    /// than pay.
    fn breakdown_pool(&self) -> Vec<ToolDef> {
        self.world
            .resource::<ToolDb>()
            .all()
            .filter(|t| {
                !t.yields.is_empty()
                    && !matches!(t.category, ToolCategory::Routines | ToolCategory::Image)
            })
            .cloned()
            .map(|mut t| {
                t.tier = 1;
                t
            })
            .collect()
    }

    /// A breakdown's band: no bench term, because a cell is not a bench's
    /// work (plan Decision 4).
    fn breakdown_band(program: &DownedProgram, tool: &ToolDef) -> crate::battle::DamageRange {
        extraction_band_with_bench(program, tool, 0)
    }

    /// The most a breakdown of `program` could pay, over every tool it might
    /// draw — the room a cell must have before the tool is drawn at all.
    fn breakdown_worst_case(&self, program: &DownedProgram) -> u32 {
        self.breakdown_pool()
            .iter()
            .map(|tool| {
                let band = Self::breakdown_band(program, tool);
                self.breakdown_yield(program, tool, band.max)
                    .iter()
                    .map(|(_, qty)| *qty)
                    .sum()
            })
            .max()
            .unwrap_or(0)
    }

    /// The record `prisoner` was booted from. A prisoner from a save that
    /// predates `Jailed::record` has none, so one is rebuilt from the body
    /// (`downed_program_for`, which rolls a condition) and **stored back**,
    /// so a deferred breakdown retried every beat rolls it once.
    fn prisoner_record(&mut self, prisoner: Entity) -> Option<DownedProgram> {
        if let Some(record) = self
            .world
            .get::<components::Jailed>(prisoner)
            .and_then(|j| j.record.clone())
        {
            return Some(record);
        }
        let rebuilt = self.downed_program_for(prisoner)?;
        self.world.get_mut::<components::Jailed>(prisoner)?.record = Some(rebuilt.clone());
        Some(rebuilt)
    }

    /// The prisoner's last failed attempt: the body despawns and a reduced
    /// extraction-style yield lands in the cell's `Stock::output`.
    ///
    /// **Defers, drawing nothing, until the cell has room for the worst
    /// payout** (the teardown rig's rule: a full output holds the program
    /// rather than eating the yield); `false` means deferred and the
    /// prisoner stays. Two `GameRng` draws when it goes ahead: the tool,
    /// then the band.
    fn break_down_prisoner(&mut self, cell: Entity, prisoner: Entity) -> bool {
        let program = self.prisoner_record(prisoner);
        if let Some(program) = program.as_ref() {
            let worst = self.breakdown_worst_case(program);
            if !self
                .world
                .get::<Stock>(cell)
                .is_some_and(|s| s.fits_payout(worst))
            {
                return false;
            }
        }
        let label = self.creature_label(prisoner);
        let granted = match (program.as_ref(), self.breakdown_tool()) {
            (Some(program), Some(tool)) => {
                let rolled = {
                    let band = Self::breakdown_band(program, &tool);
                    let mut rng = self.world.resource_mut::<GameRng>();
                    band.roll(&mut rng.0)
                };
                self.breakdown_yield(program, &tool, rolled)
            }
            _ => Vec::new(),
        };
        self.world.despawn(prisoner);
        if let Some(mut stock) = self.world.get_mut::<Stock>(cell) {
            for (item, qty) in &granted {
                *stock.output.entry(item.clone()).or_default() += qty;
            }
        }
        let parts: Vec<String> = granted
            .iter()
            .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
            .collect();
        let line = if parts.is_empty() {
            format!("{label} breaks down in the cell and leaves nothing usable.")
        } else {
            format!("{label} breaks down in the cell: {}.", parts.join(", "))
        };
        self.log_base_kind(MessageKind::Loot, line);
        true
    }

    /// The tool a breakdown is priced with, drawn uniformly from
    /// `breakdown_pool`.
    fn breakdown_tool(&mut self) -> Option<ToolDef> {
        let pool = self.breakdown_pool();
        if pool.is_empty() {
            return None;
        }
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..pool.len())
        };
        pool.into_iter().nth(pick)
    }

    /// A destroyed or demolished cell lets its prisoner go: the body
    /// despawns and its record goes back to the player's list, the protocol
    /// lost. `CarryingProgram`'s rule (`return_carried_program`): destruction
    /// returns the program rather than eating it, and the store being full
    /// logs the loss through `push_downed_program`. A no-op for a structure
    /// that holds nobody, so every destruction door can call it.
    pub(crate) fn release_prisoner(&mut self, structure: Entity) {
        let Some(prisoner) = self.cell_prisoner(structure) else {
            return;
        };
        let label = self.creature_label(prisoner);
        if let Some(record) = self.prisoner_record(prisoner) {
            self.push_downed_program(record);
        }
        self.world.despawn(prisoner);
        self.log_base(format!(
            "{label} goes dark with the Holding Cell; its record returns to your pack."
        ));
    }
}
