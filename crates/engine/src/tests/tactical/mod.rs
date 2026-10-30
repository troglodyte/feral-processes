//! The `Game`-facing half of the tactical battle model.

use crate::Experience;
use crate::Game;
use crate::components::{Creature, Hostile, Position, Rarity, Squad, Stats, StatusEffects};
use crate::resources::{BattleState, DifficultyMode, Party};
use crate::species::SpeciesDb;
use crate::tactical::TacticalBattle;
use crate::tactical::ai::{AiBeat, PartyTurns};
use crate::tactical::reach::{self, allowance};
use crate::tactical::turn::StepOutcome;
use crate::tests::support::{
    battle_with_a_pack_of, equip_weapon, generic_species, insert_battle, spawn_wild_on_player_tile,
    test_assets_dir,
};
use crate::tuning::{DEFAULT_BASE_SPEED, PLAYER_BASE_SPEED, TACTICAL_MOVE_MAX};
use bevy_ecs::prelude::{Entity, With};

mod auto_drive;
mod bracing_and_range;
mod cover;
mod engagement_decompile;
mod fixtures;
mod lifecycle;
mod movement;
mod routines;
mod squad_fights;
mod summons_respawn;
mod teleports;
mod turn_order;
mod view_and_toggle;

use auto_drive::*;
use bracing_and_range::*;
pub(crate) use fixtures::*;
use movement::*;
use routines::*;
use turn_order::*;
