//! Read-only snapshots of engine state, shaped for the renderer.
//!
//! Every one of these is produced by a `Game` method and consumed by
//! app-core/gui. They are plain data with no back-reference into the ECS —
//! that is what keeps the renderer from reaching into the `World`.

use crate::abilities::AffinityKind;
use crate::alerts::AlertKind;
use crate::battle::ActionOption;
use crate::classes::PlayerClass;
use crate::components::{EquippedItem, GlyphColor, MachineStatus, Rarity, TaskKind};
use crate::duties::Duty;
use crate::game::party::ProgramRole;
use crate::icon::PlayerIcon;
use crate::items::{GearCopy, ItemId};
use crate::outposts::Trend;
use crate::perks::Perk;
use crate::progression::StatRow;
use crate::research::ResearchId;
use crate::resources::DifficultyMode;
use crate::rooms::RoomBand;
use crate::species::{AffinityClass, MoveDef, SpeciesId};
use crate::structures::StructureId;
use crate::tools::{ToolCategory, ToolId};
use crate::world::Biome;
use bevy_ecs::prelude::Entity;

mod base;
mod battle;
mod manifest;
mod map;
mod market;
mod outposts;
mod player;
mod programs;
mod research;
mod routines;
mod work;

pub use base::*;
pub use battle::*;
pub use manifest::*;
pub use map::*;
pub use market::*;
pub use outposts::*;
pub use player::*;
pub use programs::*;
pub use research::*;
pub use routines::*;
pub use work::*;
