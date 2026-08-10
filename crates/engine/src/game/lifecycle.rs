//! Creating, saving, and restoring a `Game`.
//!
//! `new` and `load` are the only two doors into a playable world, and both
//! go through `load_asset_dbs` so neither can produce a `Game` whose item
//! set fails the economy-role check.

use crate::abilities::AbilityId;
use crate::game::spawning;
use crate::game::zone::find_walkable_start;
use crate::tuning::{NEST_DURABILITY, STACK_LINKS_PER_ZONE, initial_wild_population};
use crate::*;

/// Splits a persisted routine list into what `db` still recognizes and what
/// it doesn't — `Game::load`'s only chance to catch an id a save carries
/// that the loaded `AbilityDb` no longer has (the ability file was removed,
/// or is now malformed and got skipped with its own warning). Left
/// unfiltered, a ghost id survives into `Routines` as an entry
/// `routine_view` can't resolve: the slot renders `(empty)`, so the install
/// picker opens over an occupied one instead of the ghost ever being
/// uninstallable.
fn recognized_routines(ids: &[AbilityId], db: &AbilityDb) -> (Vec<AbilityId>, Vec<AbilityId>) {
    ids.iter().cloned().partition(|id| db.get(id).is_some())
}

/// Present once `Game::grant_profile_rewards` has run. Not saved and not
/// inserted by either constructor: it exists only so a second grant in the
/// same session is a no-op.
#[derive(Resource)]
struct ProfileRewardsPaid;

impl Game {
    pub fn new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> std::io::Result<Self> {
        let AssetDbs {
            abilities: ability_db,
            achievements: achievement_db,
            descriptions: description_db,
            species: species_db,
            structures: structure_db,
            research: research_db,
            items: item_db,
            perks: perk_db,
            policy: enemy_policy,
            warnings: load_warnings,
        } = load_asset_dbs(assets_dir)?;

        let mut world_map = WorldMap::new(seed);
        let start = find_walkable_start(&mut world_map);

        let mut world = World::new();
        world.insert_resource(ability_db);
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(perk_db);
        world.insert_resource(enemy_policy);
        world.insert_resource(description_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock::default());
        world.insert_resource(GameRng(StdRng::seed_from_u64(seed as u64)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(BattleTimeline::default());
        world.insert_resource(EffectQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(WieldedProgram::default());
        world.insert_resource(Research::default());
        // Decompile is knowledge the player starts with, not something the
        // tree teaches — nothing grants it a second time. Without this,
        // popping it out of the one starting slot to make room would end
        // taming for the run, since re-installing checks `KnownRoutines`
        // like any other write. The disk it costs to put back is the
        // ordinary price; being unable to at all is not.
        world.insert_resource(KnownRoutines(
            [abilities::DECOMPILE_ABILITY_ID.to_string()].into(),
        ));
        world.insert_resource(BuybackLedger::default());
        world.insert_resource(ZoneLevel::default());
        world.insert_resource(Platform::default());
        world.insert_resource(Locale::default());
        world.insert_resource(CurrentStack::default());
        world.insert_resource(StackMemory::default());
        world.insert_resource(crate::resources::Trace::default());
        world.insert_resource(crate::resources::RunFeats::default());
        world.insert_resource(achievement_db);
        // Empty on purpose in *both* constructors. What has actually been
        // earned is installed afterwards by `install_profile`, which app-core
        // calls on either path; paying for it is a separate call it makes on
        // only one. See `grant_profile_rewards`.
        world.insert_resource(crate::achievements::Profile::default());
        world.insert_resource(crate::resources::PendingProfileWrites::default());
        // Both doors, deliberately: a loaded game must collect telemetry too,
        // and this is not saved (dev output, not run state) so `load` has
        // nothing to restore it from.
        world.insert_resource(crate::resources::BattleTelemetry::default());
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
                crate::tuning::PLAYER_BASE_STATS,
                Needs::default(),
                Experience::default(),
                Decompiler::default(),
                Equipment::default(),
                Inventory {
                    items: vec![
                        (ItemId::from(ids::ICE_BREAKER), 3),
                        (ItemId::from(ids::POWER_CELL), 3),
                        (ItemId::from(ids::CORE_FRAGMENT), 5),
                        (ItemId::from(ids::OUTLET), 2),
                    ],
                },
                FusedGear::default(),
                StatusEffects::default(),
                CombatBuff::default(),
                FieldBuff::default(),
                Perks::default(),
                Routines(vec![abilities::DECOMPILE_ABILITY_ID.to_string()]),
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let schedule = Self::build_schedule();

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }
        game.spawn_initial_creatures(initial_wild_population());
        game.spawn_surface_links(STACK_LINKS_PER_ZONE);
        game.log("Connection established. You materialize at the edge of the Grid.");
        Ok(game)
    }

    /// The system schedule every tick runs, shared by `new` and `load` so
    /// the two can't drift — the chained pair below is exactly the kind of
    /// constraint that gets added to one copy and forgotten in the other.
    pub(crate) fn build_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            (systems::power_regen_system, systems::needs_tick_system).chain(),
            systems::wander_ai_system,
            // Chained: `task_progress_system` and `assembler_system` both
            // write `Task::progress` (for different targets, but bevy can
            // only see the conflict, not the disjointness), and an
            // arbitrary-but-fixed order is not the same as a stated one.
            // `haul_step_system` joins the same chain for the same reason,
            // one component along — it writes `Stock` too — and runs last
            // because a load is taken off a machine the tick *after* that
            // machine reports itself clogged.
            (
                systems::task_progress_system,
                systems::player_gather_system,
                systems::assembler_system,
                crate::game::hauling::haul_step_system,
            )
                .chain(),
            difficulty::death_handling_system,
            // Unchained: it shares no mutable state with anything above it,
            // and what it reads are counters every one of those has already
            // finished writing for this tick.
            crate::game::achievements::achievement_system,
        ));
        schedule
    }

    pub fn load(path: &Path, assets_dir: &Path) -> std::io::Result<Self> {
        let data = save::load_from_file(path)?;
        let AssetDbs {
            abilities: ability_db,
            achievements: achievement_db,
            descriptions: description_db,
            species: species_db,
            structures: structure_db,
            research: research_db,
            items: item_db,
            perks: perk_db,
            policy: enemy_policy,
            warnings: load_warnings,
        } = load_asset_dbs(assets_dir)?;

        let mut world_map = WorldMap::new(data.seed);
        let overrides: HashMap<(i32, i32), Tile> = data.tile_overrides.into_iter().collect();
        world_map.restore_overrides(overrides);

        // A routine slot is persisted as a raw ability id and never
        // validated, so a save made under one mod configuration can name an
        // ability a later load's asset set doesn't have (the file was
        // removed, or is now malformed and got skipped with a warning of
        // its own). Left in place, `routine_view` can't resolve the id and
        // renders the slot `(empty)` — which then makes `install_routine`
        // refuse "no free slot" for a slot the panel just told the player
        // was free. Dropped here instead, once, at the only point both the
        // save data and the loaded `AbilityDb` are in hand.
        let (player_routines, dropped_player_routines) =
            recognized_routines(&data.player.routines, &ability_db);

        let mut world = World::new();
        world.insert_resource(ability_db);
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(perk_db);
        world.insert_resource(enemy_policy);
        world.insert_resource(description_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock { tick: data.tick });
        world.insert_resource(GameRng(StdRng::seed_from_u64(data.seed as u64 ^ data.tick)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(BattleTimeline::default());
        world.insert_resource(EffectQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(data.difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(WieldedProgram::default());
        world.insert_resource(Research(data.researched.into_iter().collect()));
        world.insert_resource(KnownRoutines(data.known_routines.into_iter().collect()));
        world.insert_resource(BuybackLedger(
            data.buyback
                .into_iter()
                .map(|(kind, tile, shelf)| ((kind, tile), shelf))
                .collect(),
        ));
        world.insert_resource(ZoneLevel(data.zone));
        world.insert_resource(Platform::default());
        world.insert_resource(Locale::default());
        world.insert_resource(CurrentStack::default());
        world.insert_resource(StackMemory::default());
        world.insert_resource(crate::resources::Trace::default());
        world.insert_resource(crate::resources::RunFeats::default());
        world.insert_resource(achievement_db);
        // Empty on purpose in *both* constructors. What has actually been
        // earned is installed afterwards by `install_profile`, which app-core
        // calls on either path; paying for it is a separate call it makes on
        // only one. See `grant_profile_rewards`.
        world.insert_resource(crate::achievements::Profile::default());
        world.insert_resource(crate::resources::PendingProfileWrites::default());
        // See `Game::new`'s copy: both doors, and nothing restores it.
        world.insert_resource(crate::resources::BattleTelemetry::default());
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
                // Gear fusion was uncapped before it shared `MAX_FUSIONS`,
                // so an older save can carry a copy above the ceiling.
                // Only the carried copies are clamped — the worn copies
                // above keep the tier their bonus was applied at, because
                // `Stats` is restored with that bonus already in it and
                // unequipping must subtract exactly what was added.
                //
                // Clamping can collapse two rows onto one tier, so this
                // goes through `add` rather than building the `Vec`
                // directly: `FusedGear` holds one row per `(item, tier)`,
                // and a duplicate row would make `count` under-report and
                // strand the copies in the row it didn't find.
                {
                    let mut fused = FusedGear::default();
                    for (item, tier, qty) in data.player.fused_gear {
                        fused.add(item, tier.min(crate::tuning::MAX_FUSIONS), qty);
                    }
                    fused
                },
                StatusEffects::default(),
                CombatBuff::default(),
                FieldBuff {
                    active: data.player.field_buffs,
                },
                Perks {
                    points: data.player.perk_points,
                    unlocked: data.player.unlocked_perks,
                },
                Routines(player_routines),
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let schedule = Self::build_schedule();

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }
        if !dropped_player_routines.is_empty() {
            game.log(format!(
                "Your installed routines included {} — no longer available, and the slot is now empty.",
                dropped_player_routines.join(", ")
            ));
        }

        // Spawned before the creature loop below so a guardian's
        // `nest_position` has a live nest to resolve to — mirrors
        // `structure_positions` further down, built for the same reason.
        //
        // The bundle itself comes from `nest_components` (`game/spawning.rs`),
        // shared with `Game::spawn_nest` — see that function's doc comment,
        // which is the other half of this note.
        let mut nest_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        for n in data.nests {
            let Some(species) = game.world.resource::<SpeciesDb>().get(&n.species).cloned() else {
                continue;
            };
            let nest = game
                .world
                .spawn(spawning::nest_components(
                    &species,
                    n.position.0,
                    n.position.1,
                    // Clamped rather than trusted outright: NEST_DURABILITY
                    // is a tuning.rs constant, not part of the save format,
                    // so lowering it must not leave an existing save's nest
                    // loading with hp above the new max — the structure
                    // path a little further down clamps the same way.
                    n.durability.min(NEST_DURABILITY),
                    n.pending_respawns,
                ))
                .id();
            nest_positions.insert(n.position, nest);
        }

        let mut pending_cronjobs: Vec<(Entity, save::CronjobSave)> = Vec::new();
        // Collected with their slot index and sorted below: creatures come
        // back in whatever order they were written, which is no longer the
        // roster order, and roster order is now mechanically meaningful.
        let mut party_slots: Vec<(u32, Entity)> = Vec::new();
        // At most one creature may claim the weapon hand. Taken defensively
        // — the first wins and any others are ignored — rather than trusting
        // the file, the same way `party_slots` is truncated below.
        let mut wielded: Option<Entity> = None;
        for c in data.creatures {
            let Some(species) = game.world.resource::<SpeciesDb>().get(&c.species).cloned() else {
                continue;
            };
            let (routines, dropped_routines) =
                recognized_routines(&c.routines, game.world.resource::<AbilityDb>());
            if !dropped_routines.is_empty() {
                game.log(format!(
                    "{} carried {} — no longer available, and the slot is now empty.",
                    species.name,
                    dropped_routines.join(", ")
                ));
            }
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
                Routines(routines),
            ));
            if let Some(name) = c.custom_name.clone() {
                entity.insert(CustomName(name));
            }
            // Only the player is spawned holding a `FieldBuff` — see that
            // component's docs — so a creature with none recorded stays
            // without one, the same as a freshly tamed program.
            if !c.field_buffs.is_empty() {
                entity.insert(FieldBuff {
                    active: c.field_buffs,
                });
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
                if c.wielded && wielded.is_none() {
                    wielded = Some(creature_id);
                }
                // Unlike a cronjob target, a load names no entity, so it
                // needs none of the deferred `pending_cronjobs` treatment.
                if let Some((item, qty)) = c.carrying.clone() {
                    entity.insert(Carrying { item, qty });
                }
                if let Some(slot) = party_slot {
                    party_slots.push((slot, creature_id));
                } else if let Some(cronjob) = c.cronjob {
                    pending_cronjobs.push((creature_id, cronjob));
                }
            } else {
                entity.insert((Hostile, WanderAi::default()));
                // A nest_position resolving to nothing (the nest's species
                // is gone, or the save predates nests) is dropped silently
                // rather than failing the load — the creature just comes
                // back as an ordinary wild program.
                if let Some(nest) = c
                    .nest_position
                    .and_then(|p| nest_positions.get(&p).copied())
                {
                    entity.insert(NestGuardian { nest });
                    if c.pursuing {
                        entity.insert(Pursuing);
                    }
                }
            }
        }
        party_slots.sort_by_key(|&(slot, _)| slot);
        let mut party: Vec<Entity> = party_slots.into_iter().map(|(_, e)| e).collect();
        party.truncate(MAX_PARTY_SIZE);
        game.world.insert_resource(Party(party));
        // Unlike `BuybackLedger` and `StackMemory`, this is not zone-local:
        // the program travels with you across a breach exactly as the party
        // does, so `enter_next_zone` must not wipe it.
        game.world.insert_resource(WieldedProgram(wielded));

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
            entity.insert(Stock {
                input: s.stock_input.iter().cloned().collect(),
                output: s.stock_output.iter().cloned().collect(),
                capacity: def.capacity,
            });
            if def.runs_a_job() {
                entity.insert(MachineStatus::default());
            }
            // Rebuilt from the def rather than from the save: with the
            // deposit pool gone, a node carries nothing per-instance that a
            // `.ron` file doesn't already say. What the node *produced* is
            // in `Stock` above, which is where the state now lives.
            if let Some(work) = &def.work {
                entity.insert(ResourceNode {
                    resource: work.produces.clone(),
                    level: work.level,
                });
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

        game.restore_surface_links(data.link_sites);
        // Before `restore_locale`, which records what the party can see from
        // where they are standing and would otherwise write into a map that
        // is about to be overwritten.
        game.world.insert_resource(data.stack_memory);
        game.world
            .insert_resource(crate::resources::Trace(data.trace));
        // Last, and after the WorldMap is in place: restoring a Stack
        // locale regenerates its frame from that map's seed.
        game.restore_locale(data.locale);

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
        let fused_gear = self
            .world
            .get::<FusedGear>(player)
            .map(|f| f.copies.clone())
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player).cloned().unwrap_or_default();
        let routines = self
            .world
            .get::<Routines>(player)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        let field_buffs = self
            .world
            .get::<FieldBuff>(player)
            .map(|f| f.active.clone())
            .unwrap_or_default();

        let party_entities = self.world.resource::<Party>().0.clone();
        let wielded = self.wielded_program();
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
            Option<&Routines>,
            Option<&FieldBuff>,
            // Nested because bevy's query tuples top out at 15 elements and
            // this one is full. Grouped by what they describe — where the
            // creature belongs and what it is holding — rather than split
            // wherever the count happened to run out.
            (Option<&NestGuardian>, Option<&Pursuing>, Option<&Carrying>),
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
            routines,
            field_buff,
            (nest_guardian, pursuing, carrying),
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
            // Same by-position resolution `cronjob` above uses: a
            // `NestGuardian`'s target entity id isn't stable across the
            // round trip, but a nest's tile is.
            let nest_position = nest_guardian.and_then(|g| {
                self.world
                    .get::<Position>(g.nest)
                    .map(|nest_pos| (nest_pos.x, nest_pos.y))
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
                wielded: wielded == Some(entity),
                zone: spawn_zone.map(|z| z.0).unwrap_or(1),
                custom_name: custom_name.map(|c| c.0.clone()),
                hp_roll: potential.hp_roll,
                atk_roll: potential.atk_roll,
                def_roll: potential.def_roll,
                growth_roll: potential.growth_roll,
                fusions: fusions.map(|f| f.0).unwrap_or(0),
                routines: routines.map(|r| r.0.clone()).unwrap_or_default(),
                field_buffs: field_buff.map(|f| f.active.clone()).unwrap_or_default(),
                nest_position,
                pursuing: pursuing.is_some(),
                carrying: carrying.map(|c| (c.item.clone(), c.qty)),
            });
        }

        let mut structures = Vec::new();
        let mut structure_query = self.world.query::<(
            &Structure,
            &Position,
            Option<&Durability>,
            Option<&StructureTier>,
            Option<&Stock>,
        )>();
        // `Stock` is optional here only because test fixtures hand-spawn
        // bare `Structure`s; `place_structure` and `load` both give every
        // real one a buffer.
        for (structure, pos, durability, tier, stock) in structure_query.iter(&self.world) {
            let encode = |map: Option<&std::collections::BTreeMap<ItemId, u32>>| {
                map.map(|m| m.iter().map(|(i, n)| (i.clone(), *n)).collect())
                    .unwrap_or_default()
            };
            structures.push(save::StructureSave {
                kind: structure.kind.clone(),
                position: (pos.x, pos.y),
                durability: durability.map(|d| d.hp),
                tier: tier.map(|t| t.0),
                stock_input: encode(stock.map(|s| &s.input)),
                stock_output: encode(stock.map(|s| &s.output)),
            });
        }

        let mut nests = Vec::new();
        let mut nest_query = self.world.query::<(&Nest, &Position, &Durability)>();
        for (nest, pos, durability) in nest_query.iter(&self.world) {
            nests.push(save::NestSave {
                species: nest.species.clone(),
                position: (pos.x, pos.y),
                durability: durability.hp,
                pending_respawns: nest.pending_respawns.clone(),
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
                fused_gear,
                perk_points: perks.points,
                unlocked_perks: perks.unlocked,
                routines,
                field_buffs,
            },
            creatures,
            structures,
            nests,
            tile_overrides,
            zone: self.world.resource::<ZoneLevel>().0,
            spawn_point: {
                let p = self.world.resource::<ZoneSpawnPoint>();
                (p.x, p.y)
            },
            buyback: self
                .world
                .resource::<BuybackLedger>()
                .0
                .iter()
                .map(|((kind, tile), shelf)| (kind.clone(), *tile, shelf.clone()))
                .collect(),
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
            known_routines: self
                .world
                .resource::<KnownRoutines>()
                .0
                .iter()
                .cloned()
                .collect(),
            link_sites: {
                let mut query = self.world.query_filtered::<&Position, With<SurfaceLink>>();
                query.iter(&self.world).map(|p| (p.x, p.y)).collect()
            },
            locale: self.locale(),
            stack_memory: self.world.resource::<StackMemory>().clone(),
            trace: self.trace(),
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

    /// The profile as it currently stands, including anything earned this
    /// run. What app-core writes to `profile.ron`.
    pub fn profile(&self) -> &crate::achievements::Profile {
        self.world.resource::<crate::achievements::Profile>()
    }

    /// Takes the ids earned since the last call, emptying the queue.
    ///
    /// The engine decides what has been earned; app-core owns the path and
    /// does the writing. Non-empty means "the profile changed, write it".
    pub fn take_pending_profile_writes(&mut self) -> Vec<crate::achievements::AchievementId> {
        std::mem::take(
            &mut self
                .world
                .resource_mut::<crate::resources::PendingProfileWrites>()
                .0,
        )
    }

    /// Replaces the empty `Profile` both constructors leave in the world with
    /// what has actually been earned across every run.
    ///
    /// **Pays nothing.** Both paths need this — `achievement_system` must not
    /// re-earn a rung on a loaded save either — while only one path wants to
    /// be paid. Splitting install from grant is what puts the never-on-load
    /// rule at a single call site instead of inside a shared constructor, and
    /// it leaves `Game::new`'s signature (667 call sites) alone.
    pub fn install_profile(&mut self, profile: crate::achievements::Profile) {
        self.world.insert_resource(profile);
    }

    /// Pays out the installed profile: stat points, Perk Points and a
    /// starting program, once each, in the order the rungs were earned.
    ///
    /// **Called after `Game::new` and never after `Game::load`.** A save
    /// already has its bonuses baked into `Stats` and `Perks::points`, so
    /// paying again on load would double them on every single reload. That is
    /// the one real trap in this feature; `installing_a_profile_pays_nothing_
    /// on_its_own` and app-core's `loading_a_save_does_not_re_apply_rewards`
    /// are what hold it.
    ///
    /// Takes no argument on purpose: it reads the installed `Profile`, so the
    /// two calls cannot disagree about which profile is in play.
    pub fn grant_profile_rewards(&mut self) {
        use crate::achievements::{AchievementDb, MainStat, Profile, Reward};

        // Nothing calls this twice, and the doubling would be invisible if
        // something started to — a stat is just a number, with no record of
        // where it came from. The marker's absence is "unpaid", so neither
        // constructor has to remember to insert it; what it does require is
        // that `install_profile` runs *before* this, which is the order
        // app-core uses.
        if self.world.contains_resource::<ProfileRewardsPaid>() {
            return;
        }
        self.world.insert_resource(ProfileRewardsPaid);

        let rewards: Vec<(Reward, Option<MainStat>)> = {
            let db = self.world.resource::<AchievementDb>();
            self.world
                .resource::<Profile>()
                .earned
                .iter()
                .filter_map(|e| db.get(&e.id).map(|def| (def.reward.clone(), e.rolled_stat)))
                .collect()
        };
        if rewards.is_empty() {
            return;
        }

        let player = self.player_entity();
        let mut stat_points = 0;
        let mut perk_points = 0;
        let mut programs = Vec::new();
        for (reward, rolled) in rewards {
            match reward {
                Reward::RandomMainStat(n) => {
                    // The profile's recorded answer, never a fresh roll: the
                    // stat was decided at earn time and written down so it
                    // could not drift.
                    let Some(stat) = rolled else { continue };
                    let n = n as i32;
                    match stat {
                        MainStat::Atk => self.world.get_mut::<Stats>(player).unwrap().atk += n,
                        MainStat::Def => self.world.get_mut::<Stats>(player).unwrap().def += n,
                        MainStat::Integrity => {
                            let mut stats = self.world.get_mut::<Stats>(player).unwrap();
                            stats.max_hp += n;
                            // Both halves, or the run starts damaged.
                            stats.hp += n;
                        }
                        MainStat::Decompiler => {
                            self.world.get_mut::<Decompiler>(player).unwrap().skill += n
                        }
                    }
                    stat_points += n;
                }
                Reward::PerkPoints(n) => {
                    self.world.get_mut::<Perks>(player).unwrap().points += n;
                    perk_points += n;
                }
                Reward::StartingProgram(species_id) => match self
                    .grant_starting_program(&species_id)
                {
                    Some(name) => programs.push(name),
                    None => self.log(format!(
                        "Profile reward skipped: no species named {species_id:?} in this install."
                    )),
                },
            }
        }

        let mut parts = Vec::new();
        if stat_points > 0 {
            parts.push(format!("{stat_points} stat point(s)"));
        }
        if perk_points > 0 {
            parts.push(format!("{perk_points} Perk Point(s)"));
        }
        for name in programs {
            parts.push(name);
        }
        if !parts.is_empty() {
            self.log_kind(
                MessageKind::Outcome,
                format!("Profile restored from your record: {}.", parts.join(", ")),
            );
        }
    }

    /// Spawns `species_id` at the player's tile, already owned, and returns
    /// what to call it — or `None` if this install has no such species, which
    /// is the cross-db check `AchievementDb::load_dir` deliberately deferred
    /// to here.
    ///
    /// Follows `adopt_orphan`'s sequence, and stops where it does:
    /// deliberately **not** pushed into `Party`. `Party` is the deployed
    /// battle line, capped at `MAX_PARTY_SIZE` and entered through an
    /// explicit `add_to_party` — the program arrives owned and the player
    /// deploys it, like every other acquisition.
    fn grant_starting_program(&mut self, species_id: &str) -> Option<String> {
        let player = self.player_entity();
        let at = *self.world.get::<Position>(player)?;
        let program = self.spawn_wild_creature_scaled(species_id, at.x, at.y, 1.0)?;
        self.world
            .entity_mut(program)
            .remove::<(Hostile, WanderAi)>();
        self.world
            .entity_mut(program)
            .insert((Tamed { owner: player }, Experience::default()));
        self.install_innate_routines(program);
        Some(self.creature_label(program))
    }
}

/// Every asset database a `Game` needs, plus the per-file warnings the loads
/// accumulated for the caller to push into the message log.
struct AssetDbs {
    abilities: AbilityDb,
    achievements: crate::achievements::AchievementDb,
    descriptions: crate::descriptions::DescriptionDb,
    species: SpeciesDb,
    structures: StructureDb,
    research: ResearchDb,
    items: ItemDb,
    perks: PerkDb,
    policy: crate::resources::EnemyPolicy,
    warnings: Vec<String>,
}

/// Loads every asset directory and refuses an item set that leaves any
/// economy role unfilled. Both `Game::new` and `Game::load` must go through
/// here: `Game::currency`/`research_currency`/`craft_currency`/`trade_currency` each
/// `.expect("validated at startup")`, so a door into the world that skipped
/// this check would turn a modder's incomplete item set into a panic mid-play
/// instead of a startup error.
fn load_asset_dbs(assets_dir: &Path) -> std::io::Result<AssetDbs> {
    let (abilities, mut warnings) = AbilityDb::load_dir(&assets_dir.join("abilities"))?;
    let (species, species_warnings) = SpeciesDb::load_dir(&assets_dir.join("species"), &abilities)?;
    warnings.extend(species_warnings);
    let (structures, structure_warnings) = StructureDb::load_dir(&assets_dir.join("structures"))?;
    warnings.extend(structure_warnings);
    let (research, research_warnings) =
        ResearchDb::load_dir(&assets_dir.join("research"), &structures, &abilities)?;
    warnings.extend(research_warnings);
    let (items, item_warnings) = ItemDb::load_dir(&assets_dir.join("items"))?;
    warnings.extend(item_warnings);
    let (perks, perk_warnings) = PerkDb::load_dir(&assets_dir.join("perks"))?;
    warnings.extend(perk_warnings);
    // A file, not a directory, and an absent one is silent — see
    // `policy::load_file`. Nothing downstream branches on whether it loaded;
    // `Game::choose_wild_action` reads the resource and falls back.
    let (policy, policy_warnings) =
        crate::policy::load_file(&assets_dir.join("policies/enemy_battle.ron"))?;
    warnings.extend(policy_warnings);
    let (achievements, achievement_warnings) =
        crate::achievements::AchievementDb::load_dir(&assets_dir.join("achievements"))?;
    warnings.extend(achievement_warnings);
    let (descriptions, description_warnings) =
        crate::descriptions::DescriptionDb::load_dir(&assets_dir.join("descriptions"))?;
    warnings.extend(description_warnings);
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
    for required in [
        abilities::FALLBACK_ABILITY_ID,
        abilities::DECOMPILE_ABILITY_ID,
    ] {
        if abilities.get(required).is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "ability set is missing the mandatory ability {required:?} — the game \
                     pre-installs it and cannot start without it"
                ),
            ));
        }
    }
    Ok(AssetDbs {
        abilities,
        achievements,
        descriptions,
        species,
        structures,
        research,
        items,
        perks,
        policy: crate::resources::EnemyPolicy(policy),
        warnings,
    })
}
