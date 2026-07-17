use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::species::SpeciesId;
use crate::structures::StructureId;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphColor {
    White,
    Gray,
    Green,
    DarkGreen,
    Red,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Brown,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Glyph {
    pub ch: char,
    pub color: GlyphColor,
}

/// Marks the single player-controlled entity.
#[derive(Component)]
pub struct Player;

#[derive(Component, Clone)]
pub struct Creature {
    pub species: SpeciesId,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
}

impl Stats {
    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            0.0
        } else {
            (self.hp as f32 / self.max_hp as f32).clamp(0.0, 1.0)
        }
    }
}

/// Hunger/fatigue both run 0..=100; 100 is fully satisfied, 0 is critical.
#[derive(Component, Clone, Copy, Debug)]
pub struct Needs {
    pub hunger: f32,
    pub fatigue: f32,
}

impl Default for Needs {
    fn default() -> Self {
        Self {
            hunger: 100.0,
            fatigue: 100.0,
        }
    }
}

/// A wild creature that will fight rather than flee when engaged.
#[derive(Component)]
pub struct Hostile;

/// Tracks level/XP for the player and any tamed creature. Wild (untamed)
/// creatures don't carry this — they don't level until compiled.
#[derive(Component, Clone, Copy, Debug)]
pub struct Experience {
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            xp_to_next: 20,
        }
    }
}

#[derive(Component, Default)]
pub struct WanderAi {
    pub cooldown: u32,
}

#[derive(Component)]
pub struct Tamed {
    pub owner: Entity,
}

#[derive(Component, Default, Clone)]
pub struct Inventory {
    pub items: Vec<(ItemId, u32)>,
}

impl Inventory {
    pub fn add(&mut self, item: ItemId, qty: u32) {
        if let Some(slot) = self.items.iter_mut().find(|(i, _)| *i == item) {
            slot.1 += qty;
        } else {
            self.items.push((item, qty));
        }
    }

    pub fn count(&self, item: ItemId) -> u32 {
        self.items
            .iter()
            .find(|(i, _)| *i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Removes up to `qty` of `item`, returning how many were actually removed.
    pub fn take(&mut self, item: ItemId, qty: u32) -> u32 {
        if let Some(slot) = self.items.iter_mut().find(|(i, _)| *i == item) {
            let taken = slot.1.min(qty);
            slot.1 -= taken;
            taken
        } else {
            0
        }
    }
}

#[derive(Component)]
pub struct Structure {
    pub kind: StructureId,
}

#[derive(Component)]
pub struct ResourceNode {
    pub resource: ItemId,
    pub amount: u32,
}

/// Ticks toward a structure's next passive-processing conversion (see
/// `StructureDef::passive_process`). Present only on structures whose
/// definition sets that field.
#[derive(Component, Default)]
pub struct PassiveProcessor {
    pub progress: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum TaskKind {
    GatherResource,
}

/// A generic ongoing job: `worker` progresses `target` over multiple ticks.
/// This is deliberately generic so base-building work and any future
/// colonist-style job assignment share one mechanism.
#[derive(Component)]
pub struct Task {
    pub kind: TaskKind,
    pub target: Entity,
    pub progress: u32,
    pub required: u32,
}
