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
use crate::game::spawning::SpawnPins;
use crate::taming::{TargetResistance, capture_chance};
use crate::tuning::{DEFAULT_TAMING_DIFFICULTY, JAIL_BASE_POTENCY};
use crate::*;

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
            .into_iter()
            .filter(|id| db.get(id).is_some())
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
