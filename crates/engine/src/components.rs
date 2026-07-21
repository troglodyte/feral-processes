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
    Orange,
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

/// A player-chosen display name that overrides a creature's species name
/// wherever it's shown — currently only set via `Game::fuse_companions`.
/// Length is enforced by the caller (`MAX_CUSTOM_NAME_LEN`), not here.
#[derive(Component, Clone, Debug)]
pub struct CustomName(pub String);

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

    /// A rough "how strong is this" scalar — max HP plus Attack plus
    /// Defense, unweighted — used to gauge relative difficulty (see
    /// `difficulty_color`) without singling out any one stat.
    pub fn power(&self) -> i32 {
        self.max_hp + self.atk + self.def
    }
}

/// The player's stats at level 1, before any leveling or gear — the seed
/// value `Game::new` spawns the player with, and the baseline `balance`'s
/// projections grow from, so both stay in lockstep.
pub const PLAYER_BASE_STATS: Stats = Stats {
    hp: 90,
    max_hp: 90,
    atk: 6,
    def: 2,
};

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

/// An item sitting in an `Equipment` slot, and the gear level its stat
/// bonus was scaled for when it was equipped (see
/// `items::EquipmentStats::scaled_for_level`). The level is captured at
/// equip time — like a wild program's zone-doubled stats, it doesn't
/// retroactively change if the player breaches deeper afterward; re-equip
/// (or unequip/re-equip) to pick up a newly unlocked level.
///
/// `fusion_tier` is likewise captured at equip time — see `ItemFusions` and
/// `items::EquipmentStats::fused_for_tier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquippedItem {
    pub item: ItemId,
    pub level: u32,
    pub fusion_tier: u32,
}

/// Player-only: what's currently equipped in each slot. Each slot's
/// level-scaled stat bonus (see `EquippedItem`, `ItemId::equipment`) is
/// added directly onto `Stats`/`Decompiler` when equipped and subtracted
/// back on unequip — mirroring how leveling directly mutates `Stats`
/// elsewhere, rather than maintaining a separate "base stats" layer.
#[derive(Component, Default, Clone, Copy)]
pub struct Equipment {
    pub weapon: Option<EquippedItem>,
    pub armor: Option<EquippedItem>,
    pub module: Option<EquippedItem>,
}

impl Equipment {
    pub fn slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Module => &mut self.module,
        }
    }

    pub fn get(&self, slot: EquipmentSlot) -> Option<EquippedItem> {
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

/// Player-only: how many times each equippable `ItemId` has been fused
/// (see `Game::fuse_item`) — every fusion consumes 2 copies of an item
/// from `Inventory` and permanently adds `items::ITEM_FUSION_BONUS_PER_TIER`
/// to that item type's equipped bonus (see
/// `items::EquipmentStats::fused_for_tier`). Tracked per `ItemId` rather
/// than per physical item, since inventory stacks aren't individually
/// distinguishable.
#[derive(Component, Default, Clone)]
pub struct ItemFusions {
    pub tiers: Vec<(ItemId, u32)>,
}

impl ItemFusions {
    pub fn tier(&self, item: ItemId) -> u32 {
        self.tiers
            .iter()
            .find(|(i, _)| *i == item)
            .map(|(_, t)| *t)
            .unwrap_or(0)
    }

    pub fn increment(&mut self, item: ItemId) {
        if let Some(slot) = self.tiers.iter_mut().find(|(i, _)| *i == item) {
            slot.1 += 1;
        } else {
            self.tiers.push((item, 1));
        }
    }
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
    /// The stock level `amount` refills to once mined down to 0 — see
    /// `StructureDef::work`'s `capacity` field. Nodes never run dry
    /// permanently; a worked node just cycles between empty and full.
    pub capacity: u32,
    /// Mirrors `WorkDef::level`. `None` means a completed gather cycle
    /// always yields, same as before this field existed. `Some(level)` gates
    /// each completion behind a level-based percentage chance instead (see
    /// `systems::task_progress_system`) — a harder, chancier variant that a
    /// structure opts into via its `.ron` file rather than something every
    /// worked node does by default.
    pub level: Option<u32>,
}

/// Ticks toward a structure's next passive-processing conversion (see
/// `StructureDef::passive_process`). Present only on structures whose
/// definition sets that field.
#[derive(Component, Default)]
pub struct PassiveProcessor {
    pub progress: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    GatherResource,
    /// Posted to defend a structure against raids (see `Game::raid_check`)
    /// without also working it — see `Game::assign_guard`. Unlike
    /// `GatherResource`, `task_progress_system` ignores this kind entirely;
    /// a guard doesn't produce anything even if its target happens to have
    /// a `ResourceNode`.
    Guard,
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

/// Which stat a companion's rally/shield temporarily boosts — see
/// `PlayerBuff`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuffKind {
    Atk,
    Def,
}

/// One active combat buff, and how long it has left.
#[derive(Clone, Copy, Debug)]
pub struct ActiveBuff {
    pub kind: BuffKind,
    /// Battle rounds remaining, ticked down at the end of every round.
    pub remaining: u32,
    pub power: i32,
}

/// A temporary combat buff a companion grants the player by being commanded
/// (see `Game::battle_command_companion`) instead of attacking directly.
/// Player-only. Kept separate from `StatusEffects` because that component is
/// reserved for conditions a hostile move can inflict (always unwanted) —
/// buffs are always player-directed and shouldn't be clobbered by (or
/// clobber) an unrelated bleed/stun. Like `StatusEffects`, holds at most one
/// buff at a time: commanding a companion again overwrites whatever's still
/// active. Scoped to a single intrusion, cleared with everything else when a
/// battle ends.
#[derive(Component, Default, Clone, Copy)]
pub struct PlayerBuff {
    pub active: Option<ActiveBuff>,
}

/// Uniform random-roll range applied independently to each of a newly
/// created creature's stats (baked into `Stats` at spawn) and to its
/// growth rate (`Potential::growth_roll`) — see `Game::roll_potential`.
/// The "same species, different stats" mechanic; doesn't apply to the
/// player, who has no species.
pub const MIN_INDIVIDUAL_ROLL: f32 = 0.8;
pub const MAX_INDIVIDUAL_ROLL: f32 = 1.2;

/// An individual creature's innate quality roll, set once when it's
/// created (see `Game::spawn_wild_creature` / `Game::fuse_companions`)
/// and carried for its lifetime. `hp_roll`/`atk_roll`/`def_roll` are baked
/// into its starting `Stats` at creation time — this component doesn't
/// reapply them later, it just remembers what was rolled so
/// `quality_percent`/`quality_label` can describe it. `growth_roll`
/// actively scales `progression::add_xp`'s growth on every level-up, on
/// top of `SpeciesDef::growth_multiplier`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Potential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
}

impl Potential {
    /// Fallback for an entity with no roll of its own (e.g. a legacy save,
    /// or a test helper that spawns a creature directly) — every roll at
    /// its neutral 1.0, contributing neither a bonus nor a penalty.
    pub const NEUTRAL: Potential = Potential {
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
    };

    /// A single 0-100 "how good is this individual" percentile: averages
    /// all four rolls and maps `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
    /// onto 0-100. Purely a display aggregate — each roll still applies
    /// independently to its own stat/growth.
    pub fn quality_percent(&self) -> u32 {
        let avg = (self.hp_roll + self.atk_roll + self.def_roll + self.growth_roll) / 4.0;
        let pct =
            (avg - MIN_INDIVIDUAL_ROLL) / (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * 100.0;
        pct.round().clamp(0.0, 100.0) as u32
    }

    /// A coarse, human-readable tier for `quality_percent` — shown next to
    /// a creature in the pets and inspect screens.
    pub fn quality_label(&self) -> &'static str {
        match self.quality_percent() {
            0..=19 => "Poor",
            20..=39 => "Below Average",
            40..=59 => "Average",
            60..=79 => "Above Average",
            _ => "Excellent",
        }
    }

    /// Averages two parents' rolls into one — used when fusing two
    /// companions into one (`Game::fuse_companions`), so the result's
    /// quality reflects both parents rather than an independent fresh
    /// roll.
    pub fn averaged(a: Potential, b: Potential) -> Potential {
        Potential {
            hp_roll: (a.hp_roll + b.hp_roll) / 2.0,
            atk_roll: (a.atk_roll + b.atk_roll) / 2.0,
            def_roll: (a.def_roll + b.def_roll) / 2.0,
            growth_roll: (a.growth_roll + b.growth_roll) / 2.0,
        }
    }
}

/// A structure's remaining health against raids (see `Game::raid_check`).
/// Every deployed structure gets one, sized from its
/// `StructureDef::durability`; reaching 0 destroys the structure.
#[derive(Component, Clone, Copy, Debug)]
pub struct Durability {
    pub hp: u32,
    pub max_hp: u32,
}

/// A stationary spawner for a wild species — see the nests feature (spec:
/// `docs/superpowers/specs/2026-07-20-nests-design.md`). Present on the
/// nest entity itself, which also carries `Position`, `Glyph`, and
/// `Durability` (all reused as-is — a nest is destroyed the same way a
/// structure is, just via a direct bump-attack instead of a raid).
#[derive(Component, Clone, Debug)]
pub struct Nest {
    pub species: SpeciesId,
    /// Ticks remaining until each queued replacement guardian spawns —
    /// one entry per guardian currently missing from the nest's original
    /// count (see `systems`-adjacent `Game::nest_respawn_tick`). Emptied
    /// naturally once every missing guardian is back.
    pub pending_respawns: Vec<u32>,
}

/// Tags a wild creature as tethered to a `Nest` — see
/// `systems::wander_ai_system`'s radius check. Removed (not the
/// creature) when its nest is destroyed (`Game::attack_nest`) or when the
/// creature itself is killed/tamed, at which point it either despawns or
/// resumes ordinary untethered behavior.
#[derive(Component, Clone, Copy, Debug)]
pub struct NestGuardian {
    pub nest: Entity,
}

/// Present on a structure whose `StructureDef::temporary` is set —
/// counts down by one on every ordinary game tick (see
/// `Game::tick_inner`) until it hits 0, at which point the structure
/// collapses. Ticks spent inside a `Game::rest` cycle deliberately don't
/// decrement this.
#[derive(Component, Clone, Copy, Debug)]
pub struct Temporary {
    pub ticks_remaining: u32,
}

/// Player-only: accumulated Perk Points (earned 1 per level-up) and which
/// perks have been bought with them. See `perks::Perk` — a perk can be
/// bought more than once, so `unlocked` holds one entry per level bought
/// (duplicates allowed) rather than a unique set.
#[derive(Component, Default, Clone)]
pub struct Perks {
    pub points: u32,
    pub unlocked: Vec<Perk>,
}

impl Perks {
    /// How many levels of `perk` have been bought — 0 if none.
    pub fn level(&self, perk: Perk) -> u32 {
        self.unlocked.iter().filter(|&&p| p == perk).count() as u32
    }
}

#[cfg(test)]
mod potential_tests {
    use super::Potential;

    #[test]
    fn quality_percent_maps_the_roll_range_onto_0_to_100() {
        let worst = Potential {
            hp_roll: 0.8,
            atk_roll: 0.8,
            def_roll: 0.8,
            growth_roll: 0.8,
        };
        let neutral = Potential::NEUTRAL;
        let best = Potential {
            hp_roll: 1.2,
            atk_roll: 1.2,
            def_roll: 1.2,
            growth_roll: 1.2,
        };
        assert_eq!(worst.quality_percent(), 0);
        assert_eq!(neutral.quality_percent(), 50);
        assert_eq!(best.quality_percent(), 100);
    }

    #[test]
    fn quality_label_buckets_the_percent_into_a_coarse_tier() {
        assert_eq!(
            Potential {
                hp_roll: 0.8,
                atk_roll: 0.8,
                def_roll: 0.8,
                growth_roll: 0.8,
            }
            .quality_label(),
            "Poor"
        );
        assert_eq!(Potential::NEUTRAL.quality_label(), "Average");
        assert_eq!(
            Potential {
                hp_roll: 1.2,
                atk_roll: 1.2,
                def_roll: 1.2,
                growth_roll: 1.2,
            }
            .quality_label(),
            "Excellent"
        );
    }

    #[test]
    fn averaged_splits_the_difference_between_two_parents() {
        let a = Potential {
            hp_roll: 0.8,
            atk_roll: 1.0,
            def_roll: 1.2,
            growth_roll: 0.9,
        };
        let b = Potential {
            hp_roll: 1.2,
            atk_roll: 1.0,
            def_roll: 0.8,
            growth_roll: 1.1,
        };
        let fused = Potential::averaged(a, b);
        assert_eq!(fused.hp_roll, 1.0);
        assert_eq!(fused.atk_roll, 1.0);
        assert_eq!(fused.def_roll, 1.0);
        assert_eq!(fused.growth_roll, 1.0);
    }
}
