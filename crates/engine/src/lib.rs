pub mod battle;
pub mod components;
pub mod difficulty;
pub mod items;
pub mod progression;
pub mod resources;
pub mod save;
pub mod species;
pub mod structures;
pub mod systems;
pub mod taming;
pub mod world;

use std::collections::HashMap;
use std::path::Path;

pub use bevy_ecs::prelude::Entity;
use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use components::{
    Creature, Decompiler, Equipment, Experience, Glyph, GlyphColor, Hostile, Inventory, Needs,
    PassiveProcessor, Player, Position, ResourceNode, Stats, Structure, Tamed, Task, TaskKind,
    WanderAi,
};
use items::{EquipmentSlot, ItemId};
pub use resources::DifficultyMode;
use resources::{BattleState, GameClock, GameOver, GameRng, MessageLog, PlayerEntity};
use species::{MoveDef, SpeciesDb, SpeciesDef};
use structures::{StructureDb, StructureDef};
use world::{Biome, Tile, WorldMap};

/// How many ticks a full night's recharge cycle advances the clock by.
const REST_TICKS: u32 = 40;

/// Core Fragment cost to compile one ICE Breaker.
const ICE_BREAKER_CORE_COST: u32 = 3;

/// How much the player's `Decompiler` skill grows per level gained.
const DECOMPILER_SKILL_PER_LEVEL: i32 = 1;

pub struct PlayerStatus {
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub decompiler: i32,
    pub hunger: f32,
    pub fatigue: f32,
    pub inventory: Vec<(ItemId, u32)>,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    pub weapon: Option<ItemId>,
    pub armor: Option<ItemId>,
    pub module: Option<ItemId>,
}

#[derive(Clone)]
pub struct EntityView {
    pub entity: Entity,
    pub pos: (i32, i32),
    pub glyph: char,
    pub color: GlyphColor,
    pub label: String,
    pub is_player: bool,
    pub is_tamed: bool,
    pub is_hostile: bool,
    pub is_structure: bool,
    pub can_work: bool,
    pub hp_fraction: Option<f32>,
    pub level: Option<u32>,
}

pub struct BattleView {
    pub wild_name: String,
    pub wild_hp: i32,
    pub wild_max_hp: i32,
    pub wild_atk: i32,
    pub wild_def: i32,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub player_atk: i32,
    pub player_def: i32,
    pub player_decompiler: i32,
    pub log: Vec<String>,
    pub can_tame: bool,
    /// Estimated chance (0.0-1.0) that a decompile attempt would succeed
    /// right now, given the wild program's current HP fraction and its
    /// species' difficulty. Shown to the player even if they have no ICE
    /// Breaker yet, so they can decide whether it's worth going to compile one.
    pub decompile_chance: f32,
}

/// Full species-level detail on a single creature, shown by `Game::inspect`
/// so the player can scope a program out before bumping into it and
/// triggering an intrusion.
pub struct InspectView {
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub level: Option<u32>,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub is_hostile: bool,
    pub is_tamed: bool,
    pub taming_difficulty: f32,
    /// Estimated decompile chance if an intrusion started right now, using
    /// the creature's current HP fraction — same formula as `BattleView`.
    pub decompile_chance: f32,
    pub habitats: Vec<Biome>,
    pub moves: Vec<MoveDef>,
    pub work_resource: Option<ItemId>,
}

pub struct Game {
    world: World,
    schedule: Schedule,
}

impl Game {
    pub fn new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> std::io::Result<Self> {
        let (species_db, mut load_warnings) = SpeciesDb::load_dir(&assets_dir.join("species"))?;
        let (structure_db, structure_warnings) =
            StructureDb::load_dir(&assets_dir.join("structures"))?;
        load_warnings.extend(structure_warnings);

        let mut world_map = WorldMap::new(seed);
        let start = find_walkable_start(&mut world_map);

        let mut world = World::new();
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock::default());
        world.insert_resource(GameRng(StdRng::seed_from_u64(seed as u64)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(difficulty);

        let player = world
            .spawn((
                Player,
                Position {
                    x: start.0,
                    y: start.1,
                },
                Glyph {
                    ch: '@',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: 30,
                    max_hp: 30,
                    atk: 6,
                    def: 2,
                },
                Needs::default(),
                Experience::default(),
                Decompiler::default(),
                Equipment::default(),
                Inventory {
                    items: vec![
                        (ItemId::IceBreaker, 3),
                        (ItemId::PowerCell, 3),
                        (ItemId::CoreFragment, 5),
                    ],
                },
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let mut schedule = Schedule::default();
        schedule.add_systems((
            systems::needs_decay_system,
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }
        game.spawn_initial_creatures(14);
        game.log("Connection established. You materialize at the edge of the Grid.");
        Ok(game)
    }

    pub fn load(path: &Path, assets_dir: &Path) -> std::io::Result<Self> {
        let data = save::load_from_file(path)?;
        let (species_db, mut load_warnings) = SpeciesDb::load_dir(&assets_dir.join("species"))?;
        let (structure_db, structure_warnings) =
            StructureDb::load_dir(&assets_dir.join("structures"))?;
        load_warnings.extend(structure_warnings);

        let mut world_map = WorldMap::new(data.seed);
        let overrides: HashMap<(i32, i32), Tile> = data.tile_overrides.into_iter().collect();
        world_map.restore_overrides(overrides);

        let mut world = World::new();
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock { tick: data.tick });
        world.insert_resource(GameRng(StdRng::seed_from_u64(
            data.seed as u64 ^ data.tick,
        )));
        world.insert_resource(MessageLog::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(data.difficulty);

        let player = world
            .spawn((
                Player,
                Position {
                    x: data.player.position.0,
                    y: data.player.position.1,
                },
                Glyph {
                    ch: '@',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: data.player.hp,
                    max_hp: data.player.max_hp,
                    atk: data.player.atk,
                    def: data.player.def,
                },
                Needs {
                    hunger: data.player.hunger,
                    fatigue: data.player.fatigue,
                },
                Experience {
                    level: data.player.level,
                    xp: data.player.xp,
                    xp_to_next: data.player.xp_to_next,
                },
                Decompiler {
                    skill: data.player.decompiler,
                },
                Equipment {
                    weapon: data.player.weapon,
                    armor: data.player.armor,
                    module: data.player.module,
                },
                Inventory {
                    items: data.player.inventory,
                },
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let mut schedule = Schedule::default();
        schedule.add_systems((
            systems::needs_decay_system,
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }

        let mut pending_cronjobs: Vec<(Entity, save::CronjobSave)> = Vec::new();
        for c in data.creatures {
            let Some(species) = game.world.resource::<SpeciesDb>().get(&c.species).cloned() else {
                continue;
            };
            let mut entity = game.world.spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: c.position.0,
                    y: c.position.1,
                },
                Glyph {
                    ch: species.glyph,
                    color: species.color,
                },
                Stats {
                    hp: c.hp,
                    max_hp: c.max_hp,
                    atk: c.atk,
                    def: c.def,
                },
            ));
            if c.tamed {
                let creature_id = entity.id();
                entity.insert((
                    Tamed { owner: player },
                    Experience {
                        level: c.level,
                        xp: c.xp,
                        xp_to_next: c.xp_to_next,
                    },
                ));
                if let Some(cronjob) = c.cronjob {
                    pending_cronjobs.push((creature_id, cronjob));
                }
            } else {
                entity.insert((Hostile, WanderAi::default()));
            }
        }

        let mut structure_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        for s in data.structures {
            let Some(def) = game.world.resource::<StructureDb>().get(&s.kind).cloned() else {
                continue;
            };
            let mut entity = game.world.spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position {
                    x: s.position.0,
                    y: s.position.1,
                },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ));
            let structure_id = entity.id();
            if let Some(amount) = s.resource_amount {
                let resource = def
                    .work
                    .as_ref()
                    .map(|w| w.produces)
                    .unwrap_or(ItemId::CoreFragment);
                entity.insert(ResourceNode { resource, amount });
                structure_positions.insert(s.position, structure_id);
            }
            if def.passive_process.is_some() {
                entity.insert(PassiveProcessor::default());
            }
        }

        // Reconnect each restored cronjob to its target structure now that
        // both sides exist. A structure is matched by position (entity ids
        // aren't stable across a save/load round trip) — if it's gone,
        // the assignment is silently dropped rather than crashing.
        for (worker, cronjob) in pending_cronjobs {
            if let Some(&target) = structure_positions.get(&cronjob.target_position) {
                game.world.entity_mut(worker).insert(Task {
                    kind: TaskKind::GatherResource,
                    target,
                    progress: cronjob.progress,
                    required: cronjob.required,
                });
            }
        }

        game.log("Session restored. Reconnecting to the Grid.");
        Ok(game)
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let stats = *self.world.get::<Stats>(player).unwrap();
        let needs = *self.world.get::<Needs>(player).unwrap();
        let exp = *self.world.get::<Experience>(player).unwrap();
        let decompiler = self.world.get::<Decompiler>(player).unwrap().skill;
        let equipment = *self.world.get::<Equipment>(player).unwrap();
        let inventory = self.world.get::<Inventory>(player).unwrap().items.clone();

        let mut creatures = Vec::new();
        let mut creature_query = self.world.query::<(
            &Creature,
            &Position,
            &Stats,
            Option<&Tamed>,
            Option<&Experience>,
            Option<&Task>,
        )>();
        for (creature, pos, stats, tamed, exp, task) in creature_query.iter(&self.world) {
            let cronjob = task.and_then(|t| {
                self.world.get::<Position>(t.target).map(|target_pos| save::CronjobSave {
                    target_position: (target_pos.x, target_pos.y),
                    progress: t.progress,
                    required: t.required,
                })
            });
            creatures.push(save::CreatureSave {
                species: creature.species.clone(),
                position: (pos.x, pos.y),
                hp: stats.hp,
                max_hp: stats.max_hp,
                atk: stats.atk,
                def: stats.def,
                tamed: tamed.is_some(),
                level: exp.map(|e| e.level).unwrap_or(1),
                xp: exp.map(|e| e.xp).unwrap_or(0),
                xp_to_next: exp.map(|e| e.xp_to_next).unwrap_or(20),
                cronjob,
            });
        }

        let mut structures = Vec::new();
        let mut structure_query =
            self.world
                .query::<(&Structure, &Position, Option<&ResourceNode>)>();
        for (structure, pos, node) in structure_query.iter(&self.world) {
            structures.push(save::StructureSave {
                kind: structure.kind.clone(),
                position: (pos.x, pos.y),
                resource_amount: node.map(|n| n.amount),
            });
        }

        let tile_overrides = self
            .world
            .resource::<WorldMap>()
            .overrides()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();

        let data = save::SaveData {
            seed: self.world.resource::<WorldMap>().seed(),
            tick: self.world.resource::<GameClock>().tick,
            difficulty: *self.world.resource::<DifficultyMode>(),
            player: save::PlayerSave {
                position: (pos.x, pos.y),
                hp: stats.hp,
                max_hp: stats.max_hp,
                atk: stats.atk,
                def: stats.def,
                hunger: needs.hunger,
                fatigue: needs.fatigue,
                inventory,
                level: exp.level,
                xp: exp.xp,
                xp_to_next: exp.xp_to_next,
                decompiler,
                weapon: equipment.weapon,
                armor: equipment.armor,
                module: equipment.module,
            },
            creatures,
            structures,
            tile_overrides,
        };
        save::save_to_file(path, &data)
    }

    pub fn history_summary(&mut self) -> Option<String> {
        let reason = self.world.resource::<GameOver>().reason.clone()?;
        let tick = self.world.resource::<GameClock>().tick;
        let mut query = self.world.query_filtered::<(), With<Tamed>>();
        let tamed_count = query.iter(&self.world).count();
        Some(format!(
            "Session ended at cycle {tick}: {reason}. Programs compiled: {tamed_count}."
        ))
    }

    pub fn write_history(&mut self, path: &Path) -> std::io::Result<()> {
        if let Some(summary) = self.history_summary() {
            save::append_run_history(path, &summary)
        } else {
            Ok(())
        }
    }

    fn player_entity(&self) -> Entity {
        self.world.resource::<PlayerEntity>().0
    }

    fn log(&mut self, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push(s);
    }

    pub fn message_log(&self, n: usize) -> Vec<String> {
        self.world.resource::<MessageLog>().recent(n).to_vec()
    }

    pub fn is_game_over(&self) -> Option<String> {
        self.world.resource::<GameOver>().reason.clone()
    }

    pub fn has_active_battle(&self) -> bool {
        self.world.get_resource::<BattleState>().is_some()
    }

    fn tick(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        self.maybe_spawn_wild_creature();
        self.schedule.run(&mut self.world);
        self.world.resource_mut::<GameClock>().tick += 1;
    }

    pub fn move_player(&mut self, dx: i32, dy: i32) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let (nx, ny) = (pos.x + dx, pos.y + dy);

        if let Some(target) = self.find_wild_creature_at(nx, ny) {
            self.start_battle(target);
            self.tick();
            return;
        }
        if self.find_blocking_structure_at(nx, ny).is_some() {
            return;
        }
        let walkable = self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable;
        if walkable {
            let mut p = self.world.get_mut::<Position>(player).unwrap();
            p.x = nx;
            p.y = ny;
        }
        self.tick();
    }

    pub fn eat(&mut self, item: ItemId) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if item != ItemId::PowerCell {
            self.log("You can't consume that.");
            return;
        }
        let player = self.player_entity();
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item, 1);
        if taken == 0 {
            self.log("You have no power cells.");
            return;
        }
        {
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.hunger = (needs.hunger + 25.0).min(100.0);
        }
        self.log("You drain a power cell. Reserves replenished somewhat.");
        self.tick();
    }

    /// Power down for the night: many ticks pass at once (power reserves
    /// drain accordingly, tamed programs keep processing, rogue programs
    /// keep roaming), then fatigue is restored to full. There's no separate
    /// "rest" system beyond replaying the normal tick loop plus a fatigue
    /// reset at the end.
    pub fn rest(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        self.log("You drop into low-power standby to recharge.");
        for _ in 0..REST_TICKS {
            if self.is_game_over().is_some() {
                return;
            }
            self.tick();
        }
        let player = self.player_entity();
        {
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 100.0;
        }
        self.log("You come back online, fully recharged.");
    }

    /// Scan the current sector for salvageable power cells. Chance depends
    /// on the sector's biome; this is the only way to replenish Power Cells
    /// once the starting stock runs out.
    pub fn forage(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let biome = self.world.resource_mut::<WorldMap>().tile(pos.x, pos.y).biome;
        let chance = match biome {
            Biome::Mainframe | Biome::OpenGrid => 0.6,
            Biome::NullSector => 0.3,
            Biome::StaticField => 0.15,
            Biome::DataVoid | Biome::BlackIce => 0.0,
        };
        let found = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if found {
            self.world.get_mut::<Inventory>(player).unwrap().add(ItemId::PowerCell, 1);
            self.log("You scan the sector and recover a power cell.");
        } else {
            self.log("You scan the sector but find nothing salvageable.");
        }
        self.tick();
    }

    /// Compile raw Core Fragments into an ICE Breaker. The only way to
    /// replenish breakers once the starting stock runs out.
    pub fn craft_ice_breaker(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let has_enough = self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment)
            >= ICE_BREAKER_CORE_COST;
        if !has_enough {
            self.log(format!(
                "Compiling an ICE Breaker needs {ICE_BREAKER_CORE_COST} Core Fragments."
            ));
            return;
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(ItemId::CoreFragment, ICE_BREAKER_CORE_COST);
            inv.add(ItemId::IceBreaker, 1);
        }
        self.log("You compile an ICE Breaker from salvaged core fragments.");
        self.tick();
    }

    /// Adds (`sign` = 1) or removes (`sign` = -1) an equipped item's stat
    /// bonus from the player's `Stats`/`Decompiler`. Shared by `equip` and
    /// `unequip` so the two stay symmetric.
    fn apply_equipment_delta(&mut self, player: Entity, mods: items::EquipmentStats, sign: i32) {
        if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
            stats.atk += sign * mods.atk;
            stats.def += sign * mods.def;
        }
        if mods.decompiler != 0 {
            if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player) {
                decompiler.skill += sign * mods.decompiler;
            }
        }
    }

    /// Equips `item` from inventory into its slot, swapping out (and
    /// returning to inventory) whatever was there before.
    pub fn equip(&mut self, item: ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some((slot, new_mods)) = item.equipment() else {
            return Err(format!("{} can't be equipped.", item.display_name()));
        };
        let player = self.player_entity();
        let taken = self.world.get_mut::<Inventory>(player).unwrap().take(item, 1);
        if taken == 0 {
            return Err(format!("You don't have a {}.", item.display_name()));
        }

        let old_item = {
            let mut equipment = self.world.get_mut::<Equipment>(player).unwrap();
            equipment.slot_mut(slot).replace(item)
        };
        if let Some(old_item) = old_item {
            let (_, old_mods) = old_item.equipment().unwrap();
            self.apply_equipment_delta(player, old_mods, -1);
            self.world.get_mut::<Inventory>(player).unwrap().add(old_item, 1);
        }
        self.apply_equipment_delta(player, new_mods, 1);
        self.log(format!("You equip {}.", item.display_name()));
        self.tick();
        Ok(())
    }

    /// Unequips whatever's in `slot`, returning it to inventory.
    pub fn unequip(&mut self, slot: EquipmentSlot) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let removed = {
            let mut equipment = self.world.get_mut::<Equipment>(player).unwrap();
            equipment.slot_mut(slot).take()
        };
        let Some(item) = removed else {
            return Err(format!("Nothing equipped in your {} slot.", slot.label()));
        };
        let (_, mods) = item.equipment().unwrap();
        self.apply_equipment_delta(player, mods, -1);
        self.world.get_mut::<Inventory>(player).unwrap().add(item, 1);
        self.log(format!("You unequip {}.", item.display_name()));
        self.tick();
        Ok(())
    }

    /// Removes `qty` of `item` from inventory and logs with `verb` ("drop"
    /// or "destroy") — the two are functionally identical, distinguished
    /// only by flavor text. Only ever acts on unequipped inventory stock;
    /// an equipped item must be unequipped first.
    fn discard_item(&mut self, item: ItemId, qty: u32, verb: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let taken = self.world.get_mut::<Inventory>(player).unwrap().take(item, qty);
        if taken == 0 {
            return Err(format!("You don't have any {}.", item.display_name()));
        }
        self.log(format!("You {verb} {taken} {}.", item.display_name()));
        self.tick();
        Ok(())
    }

    pub fn drop_item(&mut self, item: ItemId, qty: u32) -> Result<(), String> {
        self.discard_item(item, qty, "drop")
    }

    pub fn destroy_item(&mut self, item: ItemId, qty: u32) -> Result<(), String> {
        self.discard_item(item, qty, "destroy")
    }

    pub fn place_structure(&mut self, structure_id: &str, dx: i32, dy: i32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't deploy right now.".into());
        }
        let def = self
            .world
            .resource::<StructureDb>()
            .get(structure_id)
            .cloned()
            .ok_or_else(|| "Unknown structure".to_string())?;
        let player = self.player_entity();
        let ppos = *self.world.get::<Position>(player).unwrap();
        let (x, y) = (ppos.x + dx, ppos.y + dy);

        let walkable = self.world.resource_mut::<WorldMap>().tile(x, y).walkable;
        if !walkable {
            return Err("Can't deploy onto that terrain.".into());
        }
        if self.find_blocking_structure_at(x, y).is_some() {
            return Err("Something is already deployed there.".into());
        }
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &def.build_cost {
                if inv.count(*item) < *qty {
                    return Err(format!("Not enough {}.", item.display_name()));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &def.build_cost {
                inv.take(*item, *qty);
            }
        }

        let mut entity = self.world.spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
        ));
        if let Some(work) = &def.work {
            entity.insert(ResourceNode {
                resource: work.produces,
                amount: 20,
            });
        }
        if def.passive_process.is_some() {
            entity.insert(PassiveProcessor::default());
        }
        self.log(format!("You deploy a {}.", def.name));
        self.tick();
        Ok(())
    }

    pub fn assign_cronjob(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let owner = self
            .world
            .get::<Tamed>(worker)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        if self.world.get::<ResourceNode>(structure).is_none() {
            return Err("That structure can't be worked.".into());
        }
        let structure_kind = self.world.get::<Structure>(structure).unwrap().kind.clone();
        let ticks = self
            .world
            .resource::<StructureDb>()
            .get(&structure_kind)
            .and_then(|d| d.work.as_ref())
            .map(|w| w.ticks_per_unit)
            .unwrap_or(5);
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: ticks,
        });
        self.log("Cronjob scheduled.");
        self.tick();
        Ok(())
    }

    fn start_battle(&mut self, wild: Entity) {
        let player = self.player_entity();
        let name = self
            .world
            .get::<Creature>(wild)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "program".to_string());
        self.world.insert_resource(BattleState {
            player,
            wild_creature: wild,
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        self.log(format!("A rogue {name} intercepts your signal!"));
    }

    pub fn battle_view(&self) -> Option<BattleView> {
        let battle = self.world.get_resource::<BattleState>()?;
        let wild_stats = self.world.get::<Stats>(battle.wild_creature)?;
        let wild_creature = self.world.get::<Creature>(battle.wild_creature)?;
        let species_db = self.world.get_resource::<SpeciesDb>()?;
        let species = species_db.get(&wild_creature.species);
        let wild_name = species.map(|s| s.name.clone()).unwrap_or_default();
        let taming_difficulty = species.map(|s| s.taming_difficulty).unwrap_or(0.5);
        let player_stats = self.world.get::<Stats>(battle.player)?;
        let decompiler_skill = self.world.get::<Decompiler>(battle.player).map(|d| d.skill).unwrap_or(0);
        let can_tame = self
            .world
            .get::<Inventory>(battle.player)
            .map(|i| i.count(ItemId::IceBreaker) > 0)
            .unwrap_or(false);
        let decompile_chance = taming::capture_chance(
            wild_stats.hp_fraction(),
            taming::item_potency(ItemId::IceBreaker),
            taming_difficulty,
            decompiler_skill,
        );
        Some(BattleView {
            wild_name,
            wild_hp: wild_stats.hp,
            wild_max_hp: wild_stats.max_hp,
            wild_atk: wild_stats.atk,
            wild_def: wild_stats.def,
            player_hp: player_stats.hp,
            player_max_hp: player_stats.max_hp,
            player_atk: player_stats.atk,
            player_def: player_stats.def,
            player_decompiler: decompiler_skill,
            log: battle.log.clone(),
            can_tame,
            decompile_chance,
        })
    }

    pub fn battle_attack(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some((player, wild)) = self
            .world
            .get_resource::<BattleState>()
            .map(|b| (b.player, b.wild_creature))
        else {
            return;
        };

        let (p_atk, w_def) = {
            let p = *self.world.get::<Stats>(player).unwrap();
            let w = *self.world.get::<Stats>(wild).unwrap();
            (p.atk, w.def)
        };
        let dmg = battle::compute_damage(p_atk, w_def, 5);
        self.apply_damage(wild, dmg);
        self.log(format!("You unleash a data strike for {dmg} damage."));

        if !self.creature_alive(wild) {
            self.log("The rogue program crashes and deletes itself!");
            let wild_max_hp = self.world.get::<Stats>(wild).unwrap().max_hp;
            self.award_player_xp(player, wild_max_hp as u32);
            self.award_loot(player, wild);
            self.world.despawn(wild);
            self.world.remove_resource::<BattleState>();
            self.tick();
            return;
        }

        self.wild_retaliate(wild, player);
        if !self.creature_alive(player) {
            self.world.remove_resource::<BattleState>();
        }
        self.tick();
    }

    /// Defeated (not tamed) rogue programs drop whatever resource their
    /// species is associated with, if any — the same `work_resource` used
    /// to decide what a tamed member of that species can gather.
    fn award_loot(&mut self, player: Entity, wild: Entity) {
        let Some(species_id) = self.world.get::<Creature>(wild).map(|c| c.species.clone()) else {
            return;
        };
        let Some(species) = self.world.resource::<SpeciesDb>().get(&species_id).cloned() else {
            return;
        };

        if let Some(resource) = species.work_resource {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(1..=2)
            };
            self.world.get_mut::<Inventory>(player).unwrap().add(resource, qty);
            self.log(format!("It drops {} {}.", qty, resource.display_name()));
        }

        if let Some((item, chance)) = species.equipment_drop {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance as f64)
            };
            if roll {
                self.world.get_mut::<Inventory>(player).unwrap().add(item, 1);
                self.log(format!("It also drops a {}!", item.display_name()));
            }
        }
    }

    /// Awards `amount` XP to the player, growing stats and fully healing on
    /// any level-up gained. Silently does nothing if the player is somehow
    /// missing an `Experience` component (shouldn't happen in practice).
    fn award_player_xp(&mut self, player: Entity, amount: u32) {
        let (levels, new_level) = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, player) else {
                return;
            };
            let levels = progression::add_xp(&mut exp, &mut stats, amount);
            (levels, exp.level)
        };
        if levels > 0 {
            if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player) {
                decompiler.skill += DECOMPILER_SKILL_PER_LEVEL * levels as i32;
            }
            self.log(format!("You gain {amount} XP and reach level {new_level}!"));
        } else {
            self.log(format!("You gain {amount} XP."));
        }
    }

    pub fn battle_decompile(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some((player, wild)) = self
            .world
            .get_resource::<BattleState>()
            .map(|b| (b.player, b.wild_creature))
        else {
            return;
        };

        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ItemId::IceBreaker, 1);
        if taken == 0 {
            self.log("You have no ICE Breaker.");
            return;
        }

        let (hp_fraction, species_id) = {
            let stats = *self.world.get::<Stats>(wild).unwrap();
            let species = self.world.get::<Creature>(wild).unwrap().species.clone();
            (stats.hp_fraction(), species)
        };
        let taming_difficulty = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|s| s.taming_difficulty)
            .unwrap_or(0.5);
        let potency = taming::item_potency(ItemId::IceBreaker);
        let decompiler_skill = self.world.get::<Decompiler>(player).map(|d| d.skill).unwrap_or(0);
        let chance = taming::capture_chance(hp_fraction, potency, taming_difficulty, decompiler_skill);
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance as f64)
        };

        if roll {
            let wild_max_hp = self.world.get::<Stats>(wild).unwrap().max_hp;
            self.world.entity_mut(wild).remove::<Hostile>();
            self.world
                .entity_mut(wild)
                .insert((Tamed { owner: player }, Experience::default()));
            self.log("ICE breached! The program now runs under your control.");
            self.award_player_xp(player, wild_max_hp as u32);
            self.world.remove_resource::<BattleState>();
            self.tick();
            return;
        }

        self.log("The program's ICE holds — decompile failed!");
        self.wild_retaliate(wild, player);
        if !self.creature_alive(player) {
            self.world.remove_resource::<BattleState>();
        }
        self.tick();
    }

    pub fn battle_flee(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some((player, wild)) = self
            .world
            .get_resource::<BattleState>()
            .map(|b| (b.player, b.wild_creature))
        else {
            return;
        };
        let got_hit = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(0.5)
        };
        if got_hit {
            self.log("You jack out, but not before taking a parting counter-strike!");
            self.wild_retaliate(wild, player);
        } else {
            self.log("You jack out safely.");
        }
        self.world.remove_resource::<BattleState>();
        self.tick();
    }

    fn wild_retaliate(&mut self, wild: Entity, player: Entity) {
        let species_id = self.world.get::<Creature>(wild).unwrap().species.clone();
        let move_count = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .unwrap()
            .moves
            .len();
        let idx = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..move_count)
        };
        let mv = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .unwrap()
            .moves[idx]
            .clone();
        let (w_atk, p_def) = {
            let w = *self.world.get::<Stats>(wild).unwrap();
            let p = *self.world.get::<Stats>(player).unwrap();
            (w.atk, p.def)
        };
        let dmg = battle::compute_damage(w_atk, p_def, mv.power);
        self.apply_damage(player, dmg);
        self.log(format!(
            "The rogue program executes {} for {} damage.",
            mv.name, dmg
        ));
    }

    fn apply_damage(&mut self, target: Entity, dmg: i32) {
        if let Some(mut stats) = self.world.get_mut::<Stats>(target) {
            stats.hp = (stats.hp - dmg).max(0);
        }
    }

    fn creature_alive(&self, e: Entity) -> bool {
        self.world.get::<Stats>(e).map(|s| s.hp > 0).unwrap_or(false)
    }

    fn find_wild_creature_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query =
            self.world
                .query_filtered::<(Entity, &Position), (With<Creature>, Without<Tamed>)>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Structure>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    fn spawn_wild_creature(&mut self, species_id: &str, x: i32, y: i32) {
        let Some(species) = self.world.resource::<SpeciesDb>().get(species_id).cloned() else {
            return;
        };
        self.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp: species.base_hp,
                max_hp: species.base_hp,
                atk: species.base_atk,
                def: species.base_def,
            },
            Hostile,
            WanderAi::default(),
        ));
    }

    fn spawn_initial_creatures(&mut self, count: usize) {
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        for _ in 0..count {
            let (dx, dy) = {
                let mut rng = self.world.resource_mut::<GameRng>();
                (rng.0.random_range(-15..=15), rng.0.random_range(-15..=15))
            };
            self.try_spawn_habitat_creature(player_pos.x + dx, player_pos.y + dy);
        }
    }

    fn maybe_spawn_wild_creature(&mut self) {
        let mut count_query = self.world.query_filtered::<(), With<Creature>>();
        if count_query.iter(&self.world).count() >= 24 {
            return;
        }
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(0.05)
        };
        if !roll {
            return;
        }
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let (dx, dy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (rng.0.random_range(-12..=12), rng.0.random_range(-12..=12))
        };
        self.try_spawn_habitat_creature(player_pos.x + dx, player_pos.y + dy);
    }

    fn try_spawn_habitat_creature(&mut self, x: i32, y: i32) {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !tile.walkable {
            return;
        }
        let candidates: Vec<String> = self
            .world
            .resource::<SpeciesDb>()
            .habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        if candidates.is_empty() {
            return;
        }
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..candidates.len());
            candidates[idx].clone()
        };
        self.spawn_wild_creature(&pick, x, y);
    }

    pub fn player_status(&self) -> PlayerStatus {
        let player = self.player_entity();
        let stats = self.world.get::<Stats>(player).unwrap();
        let needs = self.world.get::<Needs>(player).unwrap();
        let pos = self.world.get::<Position>(player).unwrap();
        let inv = self.world.get::<Inventory>(player).unwrap();
        let exp = self.world.get::<Experience>(player).unwrap();
        let decompiler = self.world.get::<Decompiler>(player).map(|d| d.skill).unwrap_or(0);
        let equipment = self.world.get::<Equipment>(player).copied().unwrap_or_default();
        PlayerStatus {
            position: (pos.x, pos.y),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            decompiler,
            hunger: needs.hunger,
            fatigue: needs.fatigue,
            inventory: inv.items.clone(),
            level: exp.level,
            xp: exp.xp,
            xp_to_next: exp.xp_to_next,
            weapon: equipment.weapon,
            armor: equipment.armor,
            module: equipment.module,
        }
    }

    pub fn view_tiles(&mut self, half_w: i32, half_h: i32) -> Vec<Vec<Tile>> {
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut world_map = self.world.resource_mut::<WorldMap>();
        let mut rows = Vec::new();
        for ty in -half_h..=half_h {
            let mut row = Vec::new();
            for tx in -half_w..=half_w {
                row.push(world_map.tile(center.x + tx, center.y + ty));
            }
            rows.push(row);
        }
        rows
    }

    pub fn view_entities(&mut self, half_w: i32, half_h: i32) -> Vec<EntityView> {
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut query = self.world.query::<(Entity, &Position, &Glyph)>();
        let hits: Vec<(Entity, Position, Glyph)> = query
            .iter(&self.world)
            .filter(|(_, p, _)| (p.x - center.x).abs() <= half_w && (p.y - center.y).abs() <= half_h)
            .map(|(e, p, g)| (e, *p, *g))
            .collect();

        hits.into_iter()
            .map(|(entity, pos, glyph)| {
                let is_player = self.world.get::<Player>(entity).is_some();
                let is_tamed = self.world.get::<Tamed>(entity).is_some();
                let is_hostile = self.world.get::<Hostile>(entity).is_some();
                let is_structure = self.world.get::<Structure>(entity).is_some();
                let can_work = self.world.get::<ResourceNode>(entity).is_some();
                let hp_fraction = self.world.get::<Stats>(entity).map(|s| s.hp_fraction());
                let level = self.world.get::<Experience>(entity).map(|e| e.level);
                let label = if let Some(c) = self.world.get::<Creature>(entity) {
                    self.world
                        .resource::<SpeciesDb>()
                        .get(&c.species)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| c.species.clone())
                } else if let Some(s) = self.world.get::<Structure>(entity) {
                    self.world
                        .resource::<StructureDb>()
                        .get(&s.kind)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| s.kind.clone())
                } else {
                    "You".to_string()
                };
                EntityView {
                    entity,
                    pos: (pos.x, pos.y),
                    glyph: glyph.ch,
                    color: glyph.color,
                    label,
                    is_player,
                    is_tamed,
                    is_hostile,
                    is_structure,
                    can_work,
                    hp_fraction,
                    level,
                }
            })
            .collect()
    }

    /// Species-level detail on a creature `view_entities` reported nearby.
    /// Read-only — looking a program over never triggers an intrusion.
    /// Returns `None` for anything that isn't a creature (e.g. a structure
    /// or the player) or whose species failed to resolve.
    pub fn inspect(&self, entity: Entity) -> Option<InspectView> {
        let creature = self.world.get::<Creature>(entity)?;
        let species = self.world.resource::<SpeciesDb>().get(&creature.species)?;
        let stats = self.world.get::<Stats>(entity)?;
        let level = self.world.get::<Experience>(entity).map(|e| e.level);
        let is_hostile = self.world.get::<Hostile>(entity).is_some();
        let is_tamed = self.world.get::<Tamed>(entity).is_some();
        let decompiler_skill = self
            .world
            .get::<Decompiler>(self.player_entity())
            .map(|d| d.skill)
            .unwrap_or(0);
        let decompile_chance = taming::capture_chance(
            stats.hp_fraction(),
            taming::item_potency(ItemId::IceBreaker),
            species.taming_difficulty,
            decompiler_skill,
        );
        Some(InspectView {
            name: species.name.clone(),
            glyph: species.glyph,
            color: species.color,
            level,
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            is_hostile,
            is_tamed,
            taming_difficulty: species.taming_difficulty,
            decompile_chance,
            habitats: species.habitats.clone(),
            moves: species.moves.clone(),
            work_resource: species.work_resource,
        })
    }

    pub fn structure_defs(&self) -> Vec<StructureDef> {
        self.world.resource::<StructureDb>().all().cloned().collect()
    }

    pub fn species_defs(&self) -> Vec<SpeciesDef> {
        self.world.resource::<SpeciesDb>().all().cloned().collect()
    }
}

fn find_walkable_start(world_map: &mut WorldMap) -> (i32, i32) {
    for r in 0..64i32 {
        for dx in -r..=r {
            for dy in -r..=r {
                if r != 0 && dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if world_map.tile(dx, dy).walkable {
                    return (dx, dy);
                }
            }
        }
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn test_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn award_loot_grants_the_species_work_resource() {
        let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| s.work_resource.is_some())
            .expect("at least one species should have a work_resource for this test");
        let resource = species.work_resource.unwrap();

        let wild = game
            .world
            .spawn((
                Creature { species: species.id.clone() },
                Position { x: 0, y: 0 },
                Stats { hp: 1, max_hp: 1, atk: 1, def: 1 },
            ))
            .id();

        let before = game.world.get::<Inventory>(player).unwrap().count(resource);
        game.award_loot(player, wild);
        let after = game.world.get::<Inventory>(player).unwrap().count(resource);

        assert!(after > before, "defeating the program should have granted {resource:?}");
    }

    #[test]
    fn award_loot_grants_nothing_for_species_without_a_work_resource() {
        let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| s.work_resource.is_none() && s.equipment_drop.is_none())
            .expect("at least one species should have neither a work_resource nor an equipment_drop for this test");

        let wild = game
            .world
            .spawn((
                Creature { species: species.id.clone() },
                Position { x: 0, y: 0 },
                Stats { hp: 1, max_hp: 1, atk: 1, def: 1 },
            ))
            .id();

        let before: u32 = game.world.get::<Inventory>(player).unwrap().items.iter().map(|(_, q)| *q).sum();
        game.award_loot(player, wild);
        let after: u32 = game.world.get::<Inventory>(player).unwrap().items.iter().map(|(_, q)| *q).sum();

        assert_eq!(before, after, "no-resource species shouldn't add anything to inventory");
    }

    #[test]
    fn inspect_reports_species_detail_without_starting_a_battle() {
        let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species = game.species_defs().into_iter().next().expect("at least one species");

        let wild = game
            .world
            .spawn((
                Creature { species: species.id.clone() },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: species.base_hp,
                    max_hp: species.base_hp,
                    atk: species.base_atk,
                    def: species.base_def,
                },
            ))
            .id();

        let view = game.inspect(wild).expect("wild creature should be inspectable");
        assert_eq!(view.name, species.name);
        assert!(view.is_hostile);
        assert!(!view.is_tamed);
        assert_eq!(view.max_hp, species.base_hp);
        assert!((0.0..=1.0).contains(&view.decompile_chance));
        assert!(!game.has_active_battle(), "inspecting must not trigger an intrusion");
    }

    #[test]
    fn inspect_returns_none_for_non_creature_entities() {
        let game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        assert!(game.inspect(player).is_none());
    }

    #[test]
    fn cronjob_assignment_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(6, DifficultyMode::Forgiving, &assets).unwrap();

        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.work.is_some())
            .expect("at least one workable structure should exist");
        let structure = game
            .world
            .spawn((
                Structure { kind: structure_def.id.clone() },
                Position { x: 3, y: 3 },
                ResourceNode {
                    resource: structure_def.work.as_ref().unwrap().produces,
                    amount: 20,
                },
            ))
            .id();

        let species = game.species_defs().into_iter().next().expect("at least one species");
        let player = game.player_entity();
        game.world.spawn((
            Creature { species: species.id.clone() },
            Position { x: 3, y: 4 },
            Stats { hp: 10, max_hp: 10, atk: 1, def: 1 },
            Tamed { owner: player },
            Experience::default(),
            Task {
                kind: TaskKind::GatherResource,
                target: structure,
                progress: 3,
                required: 6,
            },
        ));

        let path = std::env::temp_dir().join(format!(
            "feral_processes_cronjob_test_{}_{}.bin",
            std::process::id(),
            6
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Task>();
        let task = query
            .iter(&loaded.world)
            .next()
            .expect("restored creature should still have its cronjob task");
        assert_eq!(task.progress, 3);
        assert_eq!(task.required, 6);
        let target_pos = loaded
            .world
            .get::<Position>(task.target)
            .expect("task target should resolve to a structure entity");
        assert_eq!((target_pos.x, target_pos.y), (3, 3));
    }

    #[test]
    fn player_decompiler_skill_grows_on_level_up_and_survives_save_load() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();

        assert_eq!(game.player_status().decompiler, 0, "should start with no decompiler skill");

        game.award_player_xp(player, 20);
        assert_eq!(game.player_status().level, 2, "20 xp should be enough to reach level 2");
        assert_eq!(
            game.player_status().decompiler,
            DECOMPILER_SKILL_PER_LEVEL,
            "one level gained should grant one point of decompiler skill"
        );

        let path = std::env::temp_dir().join(format!(
            "feral_processes_decompiler_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.player_status().decompiler,
            DECOMPILER_SKILL_PER_LEVEL,
            "decompiler skill should survive a save/load round trip"
        );
    }

    #[test]
    fn equip_grants_stat_bonus_and_removes_item_from_inventory() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::OverclockCore, 1);
        let atk_before = game.player_status().atk;

        game.equip(ItemId::OverclockCore).unwrap();

        let status = game.player_status();
        assert_eq!(status.atk, atk_before + 3, "weapon should grant its Attack bonus");
        assert_eq!(status.weapon, Some(ItemId::OverclockCore));
        assert!(
            status.inventory.iter().all(|(i, _)| *i != ItemId::OverclockCore),
            "equipped item should leave the inventory stack"
        );
    }

    #[test]
    fn equipping_the_same_slot_again_swaps_without_double_counting_the_bonus() {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::OverclockCore, 2);
        let atk_before = game.player_status().atk;

        game.equip(ItemId::OverclockCore).unwrap();
        assert_eq!(game.player_status().atk, atk_before + 3);

        // Equipping into an already-occupied slot swaps the old item back
        // to inventory and must not stack the bonus a second time.
        game.equip(ItemId::OverclockCore).unwrap();
        let status = game.player_status();
        assert_eq!(status.atk, atk_before + 3, "re-equipping must not double the bonus");
        assert_eq!(
            status.inventory.iter().find(|(i, _)| *i == ItemId::OverclockCore).map(|(_, q)| *q),
            Some(1),
            "the swapped-out copy should return to inventory"
        );
    }

    #[test]
    fn unequip_removes_bonus_and_returns_item_to_inventory() {
        let mut game = Game::new(10, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::FirewallPlating, 1);
        let def_before = game.player_status().def;
        game.equip(ItemId::FirewallPlating).unwrap();
        assert_eq!(game.player_status().def, def_before + 3);

        game.unequip(EquipmentSlot::Armor).unwrap();

        let status = game.player_status();
        assert_eq!(status.def, def_before, "unequip should remove the bonus");
        assert_eq!(status.armor, None);
        assert_eq!(
            status.inventory.iter().find(|(i, _)| *i == ItemId::FirewallPlating).map(|(_, q)| *q),
            Some(1)
        );
    }

    #[test]
    fn unequip_errors_on_an_empty_slot() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.unequip(EquipmentSlot::Weapon).is_err());
    }

    #[test]
    fn drop_and_destroy_remove_the_full_stack() {
        let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::NeuralAmplifier, 3);

        game.drop_item(ItemId::NeuralAmplifier, 3).unwrap();
        assert!(game.player_status().inventory.iter().all(|(i, _)| *i != ItemId::NeuralAmplifier));

        assert!(
            game.destroy_item(ItemId::NeuralAmplifier, 1).is_err(),
            "destroying from an empty stack should error"
        );
    }

    #[test]
    fn equipped_gear_and_its_bonus_survive_save_and_load() {
        let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::NeuralAmplifier, 1);
        game.equip(ItemId::NeuralAmplifier).unwrap();
        let decompiler_after_equip = game.player_status().decompiler;

        let path = std::env::temp_dir().join(format!(
            "feral_processes_equipment_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert_eq!(status.module, Some(ItemId::NeuralAmplifier));
        assert_eq!(status.decompiler, decompiler_after_equip);
    }
}
