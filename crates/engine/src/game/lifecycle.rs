//! Creating, saving, and restoring a `Game`.
//!
//! `new` and `load` are the only two doors into a playable world, and both
//! go through `load_asset_dbs` so neither can produce a `Game` whose item
//! set fails the economy-role check.

use crate::game::zone::find_walkable_start;
use crate::*;

impl Game {
    pub fn new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> std::io::Result<Self> {
        let AssetDbs {
            species: species_db,
            structures: structure_db,
            research: research_db,
            items: item_db,
            warnings: load_warnings,
        } = load_asset_dbs(assets_dir)?;

        let mut world_map = WorldMap::new(seed);
        let start = find_walkable_start(&mut world_map);

        let mut world = World::new();
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock::default());
        world.insert_resource(GameRng(StdRng::seed_from_u64(seed as u64)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(EffectQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(Research::default());
        world.insert_resource(ZoneLevel::default());
        world.insert_resource(Platform::default());
        world.insert_resource(ZoneSpawnPoint {
            x: start.0,
            y: start.1,
        });

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
                components::PLAYER_BASE_STATS,
                Needs::default(),
                Experience::default(),
                Decompiler::default(),
                Equipment::default(),
                Inventory {
                    items: vec![
                        (ItemId::from(ids::ICE_BREAKER), 3),
                        (ItemId::from(ids::POWER_CELL), 3),
                        (ItemId::from(ids::CORE_FRAGMENT), 5),
                    ],
                },
                ItemFusions::default(),
                StatusEffects::default(),
                CombatBuff::default(),
                Perks::default(),
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let schedule = Self::build_schedule();

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }
        game.spawn_initial_creatures(14);
        game.log("Connection established. You materialize at the edge of the Grid.");
        Ok(game)
    }

    /// The system schedule every tick runs, shared by `new` and `load` so
    /// the two can't drift — the chained pair below is exactly the kind of
    /// constraint that gets added to one copy and forgotten in the other.
    pub(crate) fn build_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            (systems::power_regen_system, systems::needs_decay_system).chain(),
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));
        schedule
    }

    pub fn load(path: &Path, assets_dir: &Path) -> std::io::Result<Self> {
        let data = save::load_from_file(path)?;
        let AssetDbs {
            species: species_db,
            structures: structure_db,
            research: research_db,
            items: item_db,
            warnings: load_warnings,
        } = load_asset_dbs(assets_dir)?;

        let mut world_map = WorldMap::new(data.seed);
        let overrides: HashMap<(i32, i32), Tile> = data.tile_overrides.into_iter().collect();
        world_map.restore_overrides(overrides);

        let mut world = World::new();
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock { tick: data.tick });
        world.insert_resource(GameRng(StdRng::seed_from_u64(data.seed as u64 ^ data.tick)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(EffectQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(data.difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(Research(data.researched.into_iter().collect()));
        world.insert_resource(ZoneLevel(data.zone));
        world.insert_resource(Platform::default());
        world.insert_resource(ZoneSpawnPoint {
            x: data.spawn_point.0,
            y: data.spawn_point.1,
        });

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
                    weapon: data.player.weapon.map(|item| EquippedItem {
                        item,
                        level: data.player.weapon_level,
                        fusion_tier: data.player.weapon_fusion_tier,
                    }),
                    armor: data.player.armor.map(|item| EquippedItem {
                        item,
                        level: data.player.armor_level,
                        fusion_tier: data.player.armor_fusion_tier,
                    }),
                    module: data.player.module.map(|item| EquippedItem {
                        item,
                        level: data.player.module_level,
                        fusion_tier: data.player.module_fusion_tier,
                    }),
                },
                Inventory {
                    items: data.player.inventory,
                },
                ItemFusions {
                    tiers: data.player.item_fusions,
                },
                StatusEffects::default(),
                CombatBuff::default(),
                Perks {
                    points: data.player.perk_points,
                    unlocked: data.player.unlocked_perks,
                },
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let schedule = Self::build_schedule();

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }

        let mut pending_cronjobs: Vec<(Entity, save::CronjobSave)> = Vec::new();
        // Collected with their slot index and sorted below: creatures come
        // back in whatever order they were written, which is no longer the
        // roster order, and roster order is now mechanically meaningful.
        let mut party_slots: Vec<(u32, Entity)> = Vec::new();
        for c in data.creatures {
            let Some(species) = game.world.resource::<SpeciesDb>().get(&c.species).cloned() else {
                continue;
            };
            let party_slot = c.party_slot;
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
                Potential {
                    hp_roll: c.hp_roll,
                    atk_roll: c.atk_roll,
                    def_roll: c.def_roll,
                    growth_roll: c.growth_roll,
                },
                ZonePortal(c.zone),
                StatusEffects::default(),
                FusionCount(c.fusions),
            ));
            if let Some(name) = c.custom_name.clone() {
                entity.insert(CustomName(name));
            }
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
                if let Some(slot) = party_slot {
                    party_slots.push((slot, creature_id));
                } else if let Some(cronjob) = c.cronjob {
                    pending_cronjobs.push((creature_id, cronjob));
                }
            } else {
                entity.insert((Hostile, WanderAi::default()));
            }
        }
        party_slots.sort_by_key(|&(slot, _)| slot);
        let mut party: Vec<Entity> = party_slots.into_iter().map(|(_, e)| e).collect();
        party.truncate(MAX_PARTY_SIZE);
        game.world.insert_resource(Party(party));

        let mut structure_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        let currency = game.currency();
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
            // A save written before `raidable` existed still records a
            // durability for what is now a non-raidable structure; the def
            // wins, so that stored value is simply dropped.
            if def.raidable {
                entity.insert(Durability {
                    hp: s.durability.unwrap_or(def.durability).min(def.durability),
                    max_hp: def.durability,
                });
            }
            let structure_id = entity.id();
            structure_positions.insert(s.position, structure_id);
            if let Some(amount) = s.resource_amount {
                let resource = def
                    .work
                    .as_ref()
                    .map(|w| w.produces.clone())
                    .unwrap_or_else(|| currency.clone());
                let capacity = def.work.as_ref().map(|w| w.capacity).unwrap_or(5);
                let level = def.work.as_ref().and_then(|w| w.level);
                entity.insert(ResourceNode {
                    resource,
                    amount,
                    capacity,
                    level,
                });
            }
            if def.passive_process.is_some() {
                entity.insert(PassiveProcessor::default());
            }
            if def.upgrade.is_some() {
                let tier = s.tier.unwrap_or(1);
                entity.insert(StructureTier(tier));
                // WorkDef::level only carries the tier-1 baseline, so a
                // restored node's reliability has to be re-derived from its
                // tier or a Mk3 would come back extracting like a Mk1.
                if let Some(mut node) = entity.get_mut::<ResourceNode>()
                    && node.level.is_some()
                {
                    node.level = Some(tier);
                }
            }
        }

        // The slab's tiles come back through SaveData::tile_overrides; only
        // its center needs rediscovering, and the Home's position is it.
        if let Some(home) = game.home_position() {
            game.world.resource_mut::<Platform>().center = Some((home.x, home.y));
        }

        // Reconnect each restored cronjob to its target structure now that
        // both sides exist. A structure is matched by position (entity ids
        // aren't stable across a save/load round trip) — if it's gone,
        // the assignment is silently dropped rather than crashing.
        for (worker, cronjob) in pending_cronjobs {
            if let Some(&target) = structure_positions.get(&cronjob.target_position) {
                game.world.entity_mut(worker).insert(Task {
                    kind: match cronjob.kind {
                        save::CronjobKind::GatherResource => TaskKind::GatherResource,
                        save::CronjobKind::Guard => TaskKind::Guard,
                    },
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
        let equipment = self.world.get::<Equipment>(player).unwrap().clone();
        let inventory = self.world.get::<Inventory>(player).unwrap().items.clone();
        let item_fusions = self
            .world
            .get::<ItemFusions>(player)
            .map(|f| f.tiers.clone())
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player).cloned().unwrap_or_default();

        let party_entities = self.world.resource::<Party>().0.clone();
        let mut creatures = Vec::new();
        let mut creature_query = self.world.query::<(
            Entity,
            &Creature,
            &Position,
            &Stats,
            Option<&Tamed>,
            Option<&Experience>,
            Option<&Task>,
            Option<&ZonePortal>,
            Option<&CustomName>,
            Option<&Potential>,
            Option<&FusionCount>,
        )>();
        for (
            entity,
            creature,
            pos,
            stats,
            tamed,
            exp,
            task,
            spawn_zone,
            custom_name,
            potential,
            fusions,
        ) in creature_query.iter(&self.world)
        {
            let potential = potential.copied().unwrap_or(Potential::NEUTRAL);
            let cronjob = task.and_then(|t| {
                self.world
                    .get::<Position>(t.target)
                    .map(|target_pos| save::CronjobSave {
                        target_position: (target_pos.x, target_pos.y),
                        progress: t.progress,
                        required: t.required,
                        kind: match t.kind {
                            TaskKind::GatherResource => save::CronjobKind::GatherResource,
                            TaskKind::Guard => save::CronjobKind::Guard,
                        },
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
                party_slot: party_entities
                    .iter()
                    .position(|&e| e == entity)
                    .map(|i| i as u32),
                zone: spawn_zone.map(|z| z.0).unwrap_or(1),
                custom_name: custom_name.map(|c| c.0.clone()),
                hp_roll: potential.hp_roll,
                atk_roll: potential.atk_roll,
                def_roll: potential.def_roll,
                growth_roll: potential.growth_roll,
                fusions: fusions.map(|f| f.0).unwrap_or(0),
            });
        }

        let mut structures = Vec::new();
        let mut structure_query = self.world.query::<(
            &Structure,
            &Position,
            Option<&ResourceNode>,
            Option<&Durability>,
            Option<&StructureTier>,
        )>();
        for (structure, pos, node, durability, tier) in structure_query.iter(&self.world) {
            structures.push(save::StructureSave {
                kind: structure.kind.clone(),
                position: (pos.x, pos.y),
                resource_amount: node.map(|n| n.amount),
                durability: durability.map(|d| d.hp),
                tier: tier.map(|t| t.0),
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
                weapon: equipment.weapon.as_ref().map(|e| e.item.clone()),
                weapon_level: equipment.weapon.as_ref().map(|e| e.level).unwrap_or(1),
                weapon_fusion_tier: equipment
                    .weapon
                    .as_ref()
                    .map(|e| e.fusion_tier)
                    .unwrap_or(0),
                armor: equipment.armor.as_ref().map(|e| e.item.clone()),
                armor_level: equipment.armor.as_ref().map(|e| e.level).unwrap_or(1),
                armor_fusion_tier: equipment.armor.as_ref().map(|e| e.fusion_tier).unwrap_or(0),
                module: equipment.module.as_ref().map(|e| e.item.clone()),
                module_level: equipment.module.as_ref().map(|e| e.level).unwrap_or(1),
                module_fusion_tier: equipment
                    .module
                    .as_ref()
                    .map(|e| e.fusion_tier)
                    .unwrap_or(0),
                item_fusions,
                perk_points: perks.points,
                unlocked_perks: perks.unlocked,
            },
            creatures,
            structures,
            tile_overrides,
            zone: self.world.resource::<ZoneLevel>().0,
            spawn_point: {
                let p = self.world.resource::<ZoneSpawnPoint>();
                (p.x, p.y)
            },
            researched: {
                let mut ids: Vec<ResearchId> = self
                    .world
                    .resource::<Research>()
                    .0
                    .iter()
                    .cloned()
                    .collect();
                ids.sort();
                ids
            },
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
}

/// Every asset database a `Game` needs, plus the per-file warnings the loads
/// accumulated for the caller to push into the message log.
struct AssetDbs {
    species: SpeciesDb,
    structures: StructureDb,
    research: ResearchDb,
    items: ItemDb,
    warnings: Vec<String>,
}

/// Loads all four asset directories and refuses an item set that leaves any
/// economy role unfilled. Both `Game::new` and `Game::load` must go through
/// here: `Game::currency`/`research_currency`/`craft_currency` each
/// `.expect("validated at startup")`, so a door into the world that skipped
/// this check would turn a modder's incomplete item set into a panic mid-play
/// instead of a startup error.
fn load_asset_dbs(assets_dir: &Path) -> std::io::Result<AssetDbs> {
    let (species, mut warnings) = SpeciesDb::load_dir(&assets_dir.join("species"))?;
    let (structures, structure_warnings) = StructureDb::load_dir(&assets_dir.join("structures"))?;
    warnings.extend(structure_warnings);
    let (research, research_warnings) =
        ResearchDb::load_dir(&assets_dir.join("research"), &structures)?;
    warnings.extend(research_warnings);
    let (items, item_warnings) = ItemDb::load_dir(&assets_dir.join("items"))?;
    warnings.extend(item_warnings);
    let missing = items.missing_roles();
    if !missing.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "item set is missing required economy role(s): {}",
                missing.join(", ")
            ),
        ));
    }
    Ok(AssetDbs {
        species,
        structures,
        research,
        items,
        warnings,
    })
}
