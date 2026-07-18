use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use crate::items::{EquipmentSlot, ItemId};
use crate::perks::Perk;
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

/// Which zone portal's sector a creature was spawned in — set once at
/// spawn time and never changed afterward, even if the creature is later
/// tamed and carried through a portal into a deeper zone. Drives its stat
/// scale (see `ZoneLevel::stat_multiplier`) and is appended to its display
/// label (e.g. "Scrapper 2") so a deeper-zone catch reads differently from
/// a shallow one.
#[derive(Component, Clone, Copy, Debug)]
pub struct ZonePortal(pub u32);

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

/// Player-only skill at cracking a program's ICE — raises decompile odds
/// independent of the target's HP or species difficulty, and grows on
/// player level-up (see `award_player_xp`). Creatures never attempt a
/// decompile themselves, so this never appears on them.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Decompiler {
    pub skill: i32,
}

#[derive(Component)]
pub struct Tamed {
    pub owner: Entity,
}

/// Player-only: what's currently equipped in each slot. Each slot's stat
/// bonus (see `ItemId::equipment`) is added directly onto `Stats`/
/// `Decompiler` when equipped and subtracted back on unequip — mirroring
/// how leveling directly mutates `Stats` elsewhere, rather than maintaining
/// a separate "base stats" layer.
#[derive(Component, Default, Clone, Copy)]
pub struct Equipment {
    pub weapon: Option<ItemId>,
    pub armor: Option<ItemId>,
    pub module: Option<ItemId>,
}

impl Equipment {
    pub fn slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<ItemId> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Module => &mut self.module,
        }
    }

    pub fn get(&self, slot: EquipmentSlot) -> Option<ItemId> {
        match slot {
            EquipmentSlot::Weapon => self.weapon,
            EquipmentSlot::Armor => self.armor,
            EquipmentSlot::Module => self.module,
        }
    }
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

    /// Removes up to `qty` of `item`, returning how many were actually
    /// removed. Drops the slot entirely once it hits zero, rather than
    /// leaving a `(item, 0)` behind — callers that list `items` (the status
    /// panel, the inventory screen) shouldn't have to filter zero-quantity
    /// stacks themselves.
    pub fn take(&mut self, item: ItemId, qty: u32) -> u32 {
        let Some(pos) = self.items.iter().position(|(i, _)| *i == item) else {
            return 0;
        };
        let taken = self.items[pos].1.min(qty);
        self.items[pos].1 -= taken;
        if self.items[pos].1 == 0 {
            self.items.remove(pos);
        }
        taken
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

/// A status condition a battle `MoveDef::effect` can inflict on a combatant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusKind {
    /// Deals `ActiveStatus::power` damage at the end of every round it's
    /// active.
    Bleed,
    /// Causes the afflicted side to lose their next action in battle.
    Stun,
}

/// One combatant's currently active status condition, and how long it has
/// left.
#[derive(Clone, Copy, Debug)]
pub struct ActiveStatus {
    pub kind: StatusKind,
    /// Battle rounds remaining, ticked down at the end of every round.
    pub remaining: u32,
    /// Bleed damage dealt per round; unused for `Stun`.
    pub power: i32,
}

/// A creature or the player can carry at most one status condition at a
/// time — a fresh application overwrites whatever was active, mirroring a
/// classic single-status-condition model rather than a stacking one.
/// Scoped to a single intrusion: cleared whenever a battle ends, however it
/// ends (kill, tame, flee, or the player going down).
#[derive(Component, Default, Clone, Copy)]
pub struct StatusEffects {
    pub active: Option<ActiveStatus>,
}

/// A structure's remaining health against raids (see `Game::raid_check`).
/// Every deployed structure gets one, sized from its
/// `StructureDef::durability`; reaching 0 destroys the structure.
#[derive(Component, Clone, Copy, Debug)]
pub struct Durability {
    pub hp: u32,
    pub max_hp: u32,
}

/// Player-only: accumulated Perk Points (earned 1 per level-up) and which
/// perks have been unlocked with them. See `perks::Perk`.
#[derive(Component, Default, Clone)]
pub struct Perks {
    pub points: u32,
    pub unlocked: Vec<Perk>,
}

impl Perks {
    pub fn has(&self, perk: Perk) -> bool {
        self.unlocked.contains(&perk)
    }
}
