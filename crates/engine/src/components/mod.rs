use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use crate::MAX_CUSTOM_NAME_LEN;
use crate::abilities::{AbilityId, TamperKind, TamperSlot};
use crate::classes::PlayerClass;
use crate::floors::FloorId;
use crate::icon::PlayerIcon;
use crate::items::{DownedProgram, EquipmentSlot, GearCopy, ItemId};
use crate::items_db::ItemDb;
use crate::needs::{NEED_MAX, NEED_MIN, NeedId};
use crate::perks::Perk;
use crate::species::SpeciesId;
use crate::structures::StructureId;
use crate::tools::ToolId;
use crate::tuning::{
    GOLD_STAT_MULT, MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL, PLATINUM_STAT_MULT,
    PRISMATIC_STAT_MULT, SILVER_STAT_MULT,
};

mod basics;
mod effects;
mod gear;
mod inventory;
mod memories;
mod needs;
mod staff;
mod stats;
mod structures;
mod world;

pub use basics::*;
pub use effects::*;
pub use gear::*;
pub use inventory::*;
pub use memories::*;
pub use needs::*;
pub use staff::*;
pub use stats::*;
pub use structures::*;
pub use world::*;
