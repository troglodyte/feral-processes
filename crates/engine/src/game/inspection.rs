//! Looking at the world without changing it: the tile and entity views the
//! renderer draws, plus inspect and symlink targeting.

use crate::tuning::{DIFFICULTY_EASY_MAX, DIFFICULTY_EVEN_MAX, DIFFICULTY_TOUGH_MAX, MAX_FUSIONS};
use crate::*;

impl Game {
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

    /// Finds the nearest creature generally toward (dx, dy) from the
    /// player — the read-only "look in a direction" counterpart to
    /// `move_player`. `(dx, dy)` is one of the four cardinal unit vectors.
    /// A creature counts as "that way" if it's within the 90° cone
    /// centered on the chosen direction (i.e. leans at least as much
    /// toward that axis as away from it) and within `max_range` tiles —
    /// a strict single-tile-wide ray would almost never line up with a
    /// wandering creature's exact row/column, so this is deliberately
    /// forgiving. Ignores terrain walkability (this never moves anything,
    /// just looks), and only ever matches creatures, not structures or
    /// the player.
    pub fn find_creature_in_direction(
        &mut self,
        dx: i32,
        dy: i32,
        max_range: i32,
    ) -> Option<Entity> {
        let player = self.player_entity();
        let start = *self.world.get::<Position>(player).unwrap();
        let mut query = self.world.query::<(Entity, &Position, &Creature)>();
        query
            .iter(&self.world)
            .filter_map(|(entity, pos, _)| {
                let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
                let in_cone = if dx != 0 {
                    ddx.signum() == dx && ddx.abs() >= ddy.abs()
                } else {
                    ddy.signum() == dy && ddy.abs() >= ddx.abs()
                };
                let dist = ddx.abs().max(ddy.abs());
                (in_cone && dist >= 1 && dist <= max_range).then_some((entity, dist))
            })
            .min_by_key(|(_, dist)| *dist)
            .map(|(entity, _)| entity)
    }

    /// Display label for any entity — species name for a creature,
    /// structure name for a structure, `"You"` otherwise. Shared by
    /// `view_entities` for both an entity's own label and cross-references
    /// (a worker's assigned structure, a structure's assigned worker).
    pub(crate) fn entity_label(&self, entity: Entity) -> String {
        if let Some(name) = self.creature_name(entity) {
            self.zone_tagged_name(entity, name)
        } else if let Some(s) = self.world.get::<Structure>(entity) {
            self.world
                .resource::<StructureDb>()
                .get(&s.kind)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| s.kind.clone())
        } else if let Some(nest) = self.world.get::<Nest>(entity) {
            let species_name = self
                .world
                .resource::<SpeciesDb>()
                .get(&nest.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| nest.species.clone());
            format!("{species_name} Nest")
        } else {
            "You".to_string()
        }
    }

    pub fn view_entities(&mut self, half_w: i32, half_h: i32) -> Vec<EntityView> {
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut query = self.world.query::<(Entity, &Position, &Glyph)>();
        let hits: Vec<(Entity, Position, Glyph)> = query
            .iter(&self.world)
            .filter(|(_, p, _)| {
                (p.x - center.x).abs() <= half_w && (p.y - center.y).abs() <= half_h
            })
            .map(|(e, p, g)| (e, *p, *g))
            .collect();

        let worker_by_structure: HashMap<Entity, Entity> = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            tasks
                .iter(&self.world)
                .map(|(worker, task)| (task.target, worker))
                .collect()
        };

        let player_power = self
            .world
            .get::<Stats>(self.player_entity())
            .unwrap()
            .power();

        hits.into_iter()
            .map(|(entity, pos, glyph)| {
                let is_player = self.world.get::<Player>(entity).is_some();
                let is_tamed = self.world.get::<Tamed>(entity).is_some();
                let is_companion = self.world.resource::<Party>().0.contains(&entity);
                let is_hostile = self.world.get::<Hostile>(entity).is_some();
                let is_structure = self.world.get::<Structure>(entity).is_some();
                let is_home = self
                    .world
                    .get::<Structure>(entity)
                    .is_some_and(|s| s.kind == HOME_STRUCTURE_ID);
                let is_boss = self.is_boss_creature(entity);
                let tier = self.world.get::<StructureTier>(entity).map(|t| t.0);
                let can_work = self.world.get::<ResourceNode>(entity).is_some();
                let can_trade = self.trade_options(entity).is_some();
                let structure_worker = if is_structure {
                    worker_by_structure
                        .get(&entity)
                        .map(|&worker| self.entity_label(worker))
                } else {
                    None
                };
                let stats = self.world.get::<Stats>(entity);
                let hp_fraction = stats.map(|s| s.hp_fraction());
                // Hostile wild programs are recolored by difficulty relative
                // to the player's current power, rather than shown in their
                // species' authored color — see `difficulty_color`. Everyone
                // and everything else (the player, tamed/companion programs,
                // structures) keeps its normal glyph color.
                let color = if is_hostile {
                    stats
                        .map(|s| difficulty_color(s.power(), player_power, is_boss))
                        .unwrap_or(glyph.color)
                } else {
                    glyph.color
                };
                let level = self.world.get::<Experience>(entity).map(|e| e.level);
                let durability = self
                    .world
                    .get::<Durability>(entity)
                    .map(|d| (d.hp, d.max_hp));
                let label = self.entity_label(entity);
                EntityView {
                    entity,
                    pos: (pos.x, pos.y),
                    glyph: glyph.ch,
                    color,
                    label,
                    is_player,
                    is_tamed,
                    is_companion,
                    is_hostile,
                    is_structure,
                    is_home,
                    tier,
                    is_boss,
                    can_work,
                    can_trade,
                    structure_worker,
                    hp_fraction,
                    level,
                    durability,
                    fusions: self.fusion_count(entity),
                }
            })
            .collect()
    }

    /// Everything known about one subject, for the manifest screen. Works on
    /// the player and on any creature — wild, owned, or in the party.
    /// Read-only: looking a program over never triggers an intrusion.
    ///
    /// `None` for anything that is neither (a structure, a nest, a despawned
    /// entity), or for a creature whose species failed to resolve.
    pub fn manifest(&self, entity: Entity) -> Option<ManifestView> {
        if self.world.get::<Player>(entity).is_some() {
            return self.player_manifest(entity);
        }
        self.program_manifest(entity)
    }

    fn player_manifest(&self, entity: Entity) -> Option<ManifestView> {
        let stats = self.world.get::<Stats>(entity)?;
        let needs = self.world.get::<Needs>(entity)?;
        let pos = self.world.get::<Position>(entity)?;
        let inv = self.world.get::<Inventory>(entity)?;
        let exp = self.world.get::<Experience>(entity)?;
        let glyph = self.world.get::<Glyph>(entity)?;
        // The same calls `player_status` makes, so the sidebar and the sheet
        // cannot show different numbers for the same player.
        let atk = self.effective_atk(entity);
        let def = self.effective_def(entity);
        let equipment = self
            .world
            .get::<Equipment>(entity)
            .cloned()
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(entity);
        Some(ManifestView {
            entity,
            name: "You".to_string(),
            glyph: glyph.ch,
            color: glyph.color,
            level: Some(exp.level),
            xp: Some((exp.xp, exp.xp_to_next)),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            def,
            power: stats.max_hp + atk + def,
            status_effect: self.status_label(entity),
            routines: self.routine_view(entity),
            subject: ManifestSubject::Player(PlayerManifest {
                hunger: needs.hunger,
                fatigue: needs.fatigue,
                decompiler: self
                    .world
                    .get::<Decompiler>(entity)
                    .map(|d| d.skill)
                    .unwrap_or(0),
                equipment: [
                    EquipmentSlot::Weapon,
                    EquipmentSlot::Armor,
                    EquipmentSlot::Module,
                ]
                .into_iter()
                .filter_map(|slot| self.manifest_equip_slot(slot, equipment.get(slot)?))
                .collect(),
                perk_points: perks.map(|p| p.points).unwrap_or(0),
                perks: perks
                    .map(|p| {
                        Perk::all()
                            .into_iter()
                            .map(|perk| (perk, p.level(perk)))
                            .filter(|(_, level)| *level > 0)
                            .map(|(perk, level)| (perk.display_name().to_string(), level))
                            .collect()
                    })
                    .unwrap_or_default(),
                position: (pos.x, pos.y),
                zone: self.world.resource::<ZoneLevel>().0,
                pet_count: self.pet_count(),
                pet_capacity: self.pet_capacity(),
                cargo_used: inv.cargo_used(self.world.resource::<ItemDb>()),
                party: self.party_info(),
            }),
        })
    }

    /// One worn item as the manifest lists it. `None` if the item's
    /// definition has gone missing (a mod removed since the save was
    /// written), which drops the row rather than failing the whole sheet.
    fn manifest_equip_slot(
        &self,
        slot: EquipmentSlot,
        worn: EquippedItem,
    ) -> Option<ManifestEquipSlot> {
        let (_, base) = self.equipment_of(&worn.item)?;
        let mods = base
            .scaled_for_level(worn.level)
            .fused_for_tier(worn.fusion_tier);
        Some(ManifestEquipSlot {
            slot: slot.label().to_string(),
            item_name: self.item_name(&worn.item).to_string(),
            gear_level: worn.level,
            fusion_tier: worn.fusion_tier,
            atk: mods.atk,
            def: mods.def,
            decompiler: mods.decompiler,
        })
    }

    fn program_manifest(&self, entity: Entity) -> Option<ManifestView> {
        let creature = self.world.get::<Creature>(entity)?;
        let species = self.world.resource::<SpeciesDb>().get(&creature.species)?;
        let stats = self.world.get::<Stats>(entity)?;
        let exp = self.world.get::<Experience>(entity);
        let is_tamed = self.world.get::<Tamed>(entity).is_some();
        let custom = self.world.get::<CustomName>(entity).map(|c| c.0.clone());
        let decompiler_skill = self.player_decompiler_skill();
        Some(ManifestView {
            entity,
            name: match &custom {
                Some(name) => name.clone(),
                None => self.zone_tagged_name(entity, species.name.clone()),
            },
            glyph: species.glyph,
            color: species.color,
            level: exp.map(|e| e.level),
            xp: exp.map(|e| (e.xp, e.xp_to_next)),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            power: stats.power(),
            status_effect: self.status_label(entity),
            routines: self.routine_view(entity),
            subject: ManifestSubject::Program(ProgramManifest {
                species_name: custom
                    .is_some()
                    .then(|| self.zone_tagged_name(entity, species.name.clone())),
                is_hostile: self.world.get::<Hostile>(entity).is_some(),
                is_tamed,
                is_companion: self.world.resource::<Party>().0.contains(&entity),
                is_boss: species.is_boss,
                activity: is_tamed.then(|| self.program_activity(entity)),
                potential: self
                    .world
                    .get::<Potential>(entity)
                    .map(|p| ManifestPotential {
                        hp_roll: p.hp_roll,
                        atk_roll: p.atk_roll,
                        def_roll: p.def_roll,
                        growth_roll: p.growth_roll,
                        percent: p.quality_percent(),
                        label: p.quality_label().to_string(),
                    }),
                fusions: self.fusion_count(entity),
                max_fusions: MAX_FUSIONS,
                habitats: species.habitats.clone(),
                moves: species.moves.clone(),
                work_resource: species.work_resource.clone(),
                taming_difficulty: species.taming_difficulty,
                decompile_chance: self.taming_catalyst().map(|(_, potency)| {
                    taming::capture_chance(
                        stats.hp_fraction(),
                        potency,
                        species.taming_difficulty,
                        decompiler_skill,
                    )
                }),
                growth_multiplier: species.growth_multiplier,
                base_speed: species.base_speed,
            }),
        })
    }

    /// Every deployed structure that's a symlink target (its def has
    /// `teleport_cost` set), anywhere on the map — unlike `view_entities`,
    /// this isn't limited to a scan radius, since the whole point of a
    /// symlink is reaching it from far away.
    pub fn symlink_targets(&mut self) -> Vec<EntityView> {
        let mut query = self
            .world
            .query::<(Entity, &Position, &Glyph, &Structure)>();
        let hits: Vec<(Entity, Position, Glyph, StructureId)> = query
            .iter(&self.world)
            .map(|(e, p, g, s)| (e, *p, *g, s.kind.clone()))
            .collect();

        let db = self.world.resource::<StructureDb>();
        hits.into_iter()
            .filter(|(_, _, _, kind)| db.get(kind).is_some_and(|d| d.teleport_cost.is_some()))
            .map(|(entity, pos, glyph, kind)| EntityView {
                entity,
                pos: (pos.x, pos.y),
                glyph: glyph.ch,
                color: glyph.color,
                label: self.entity_label(entity),
                is_player: false,
                is_tamed: false,
                is_companion: false,
                is_hostile: false,
                is_structure: true,
                is_home: kind == HOME_STRUCTURE_ID,
                tier: self.world.get::<StructureTier>(entity).map(|t| t.0),
                is_boss: false,
                can_work: false,
                can_trade: false,
                structure_worker: None,
                hp_fraction: None,
                level: None,
                durability: self
                    .world
                    .get::<Durability>(entity)
                    .map(|d| (d.hp, d.max_hp)),
                fusions: 0,
            })
            .collect()
    }

    /// The item cost to symlink to `target`, if it's a symlink-capable
    /// structure — used both by `use_symlink` itself and by the renderer to
    /// show the cost before the player commits to it.
    pub fn symlink_cost(&self, target: Entity) -> Option<Vec<(ItemId, u32)>> {
        let kind = self.world.get::<Structure>(target)?.kind.clone();
        self.world
            .resource::<StructureDb>()
            .get(&kind)
            .and_then(|d| d.teleport_cost.clone())
    }

    /// "Use symlink" — instantly teleports the player to `target` (a
    /// symlink-capable structure from `symlink_targets`), paying its
    /// `teleport_cost` from inventory.
    pub fn use_symlink(&mut self, target: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_surface()?;
        if self.world.get::<Structure>(target).is_none() {
            return Err("That's not a structure.".to_string());
        }
        let cost = self
            .symlink_cost(target)
            .ok_or_else(|| "That structure has no symlink.".to_string())?;
        let player = self.player_entity();
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item) < *qty {
                    return Err(format!("Not enough {}.", self.item_name(item)));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty);
            }
        }
        let target_pos = *self.world.get::<Position>(target).unwrap();
        let name = self.entity_label(target);
        {
            let mut pos = self.world.get_mut::<Position>(player).unwrap();
            pos.x = target_pos.x;
            pos.y = target_pos.y;
        }
        self.log(format!("You use a symlink and teleport to {name}."));
        self.tick();
        Ok(())
    }
}

/// Old-school "con"-style map coloring for a hostile wild program, relative
/// to the player's current `Stats::power`. A boss is always Magenta
/// regardless of the ratio; everything else runs Green (easy) → Yellow
/// (even) → Orange (tough) → Red (hard) as `creature_power` grows past
/// `player_power`. Pulled out of `view_entities` so the bucketing is
/// unit-testable without spinning up a `Game`.
pub(crate) fn difficulty_color(
    creature_power: i32,
    player_power: i32,
    is_boss: bool,
) -> GlyphColor {
    if is_boss {
        return GlyphColor::Magenta;
    }
    let ratio = creature_power as f64 / player_power.max(1) as f64;
    if ratio <= DIFFICULTY_EASY_MAX {
        GlyphColor::Green
    } else if ratio <= DIFFICULTY_EVEN_MAX {
        GlyphColor::Yellow
    } else if ratio <= DIFFICULTY_TOUGH_MAX {
        GlyphColor::Orange
    } else {
        GlyphColor::Red
    }
}
