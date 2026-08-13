//! Looking at the world without changing it: the tile and entity views the
//! renderer draws, plus inspect and symlink targeting.

use crate::game::hauling::at_station;
use crate::tuning::{
    DIFFICULTY_EASY_MAX, DIFFICULTY_EVEN_MAX, DIFFICULTY_TOUGH_MAX, MAX_COMPANION_REFACTORS,
    MAX_FUSIONS,
};
use crate::*;
use std::collections::HashSet;

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

    /// Finds the nearest creature *or structure* generally toward (dx, dy)
    /// from the player — the read-only "look in a direction" counterpart to
    /// `move_player`. `(dx, dy)` is one of the four cardinal unit vectors.
    /// Something counts as "that way" if it's within the 90° cone centered
    /// on the chosen direction (i.e. leans at least as much toward that axis
    /// as away from it) and within `max_range` tiles — a strict
    /// single-tile-wide ray would almost never line up with a wandering
    /// creature's exact row/column, so this is deliberately forgiving.
    /// Ignores terrain walkability (this never moves anything, just looks),
    /// and never matches the player.
    ///
    /// **Both kinds are gathered in one walk, and that is what makes
    /// "nearest wins" answerable.** Two functions and a caller choosing
    /// between them would have to re-derive distance to compare, putting the
    /// cone rule in two places; the returned variant is the answer this walk
    /// already computed, so a caller never has to ask a second time what it
    /// just found.
    ///
    /// **Nothing is found underground, and that is the whole function's
    /// guard rather than one scan's.** `Position` stays pinned to the
    /// surface entrance tile while the party is in the Stack, so an
    /// unguarded scan reports the base four frames overhead as being off to
    /// your east — and, before this guard covered creatures too, opened a
    /// manifest for a wild program up there as lying "that way". The guard
    /// lives here rather than at the call site for the reason
    /// `require_surface` exists.
    ///
    /// This takes no action and moves nothing, so `require_surface` does not
    /// apply and never would have caught it. The test for whether a
    /// `Position` reader needs the guard is not "does it act" but "does it
    /// claim something about where the party is" — see `CLAUDE.md`'s
    /// load-bearing-seams entry. Underground, `x` describes the cell instead
    /// (`Game::describe_view_direction`), which is a claim about the frame
    /// the party is actually in.
    /// The structure standing on the tile one step in `(dx, dy)`, if any.
    ///
    /// One tile, deliberately, where `find_target_in_direction` above scans a
    /// cone out to `MENU_SCAN_RADIUS`. Its caller demolishes what it finds, so
    /// a cone would let a single keypress take down a structure off the far
    /// side of the screen; you have to be standing next to what you remove.
    ///
    /// An `EntityView` rather than an `Entity` because the caller has to route
    /// a Home into its confirmation screen, and `view_entities` is where
    /// `is_home` is decided — the demolish menu reads the same field from the
    /// same builder, so the two routes cannot disagree about what a Home is.
    ///
    /// Nothing is found underground, for the reason `find_target_in_direction`
    /// finds nothing there: `Position` is pinned to the surface entrance tile
    /// while the party is in the Stack, so aiming a direction key down there
    /// would pick out the base four frames overhead.
    pub fn adjacent_structure(&mut self, dx: i32, dy: i32) -> Option<EntityView> {
        if self.is_underground() {
            return None;
        }
        let center = *self.world.get::<Position>(self.player_entity())?;
        let target = (center.x + dx, center.y + dy);
        // A square box just big enough to contain the one tile asked about,
        // rather than a per-axis one: the scan is only a way to reach the
        // shared view builder, and the `pos` filter is what actually selects.
        let reach = dx.abs().max(dy.abs());
        self.view_entities(reach, reach)
            .into_iter()
            .find(|e| e.is_structure && e.pos == target)
    }

    pub fn find_target_in_direction(
        &mut self,
        dx: i32,
        dy: i32,
        max_range: i32,
    ) -> Option<InspectTarget> {
        if self.is_underground() {
            return None;
        }
        let player = self.player_entity();
        let start = *self.world.get::<Position>(player).unwrap();
        let in_cone = |pos: &Position| -> Option<i32> {
            let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
            let leans = if dx != 0 {
                ddx.signum() == dx && ddx.abs() >= ddy.abs()
            } else {
                ddy.signum() == dy && ddy.abs() >= ddx.abs()
            };
            let dist = ddx.abs().max(ddy.abs());
            (leans && dist >= 1 && dist <= max_range).then_some(dist)
        };

        let mut creatures = self.world.query::<(Entity, &Position, &Creature)>();
        let mut best: Option<(i32, InspectTarget)> = creatures
            .iter(&self.world)
            .filter_map(|(entity, pos, _)| {
                in_cone(pos).map(|d| (d, InspectTarget::Creature(entity)))
            })
            .min_by_key(|(dist, _)| *dist);

        let mut structures = self.world.query::<(Entity, &Position, &Structure)>();
        let nearest_structure = structures
            .iter(&self.world)
            .filter_map(|(entity, pos, _)| {
                in_cone(pos).map(|d| (d, InspectTarget::Structure(entity)))
            })
            .min_by_key(|(dist, _)| *dist);
        // Strictly nearer, so a creature standing *on* a structure's tile
        // keeps the tie — it is the thing that might wander off before
        // you look again, and the structure will still be there.
        best = match (best, nearest_structure) {
            (Some((cd, c)), Some((sd, s))) => Some(if sd < cd { (sd, s) } else { (cd, c) }),
            (some, None) | (None, some) => some,
        };
        best.map(|(_, target)| target)
    }

    /// The `B` roster's row for one structure, for the inspector's detail
    /// screen. Deliberately *the same call* the roster makes rather than a
    /// second builder beside it: per `CLAUDE.md` a doc comment claiming to
    /// mirror another formula has to be a call, and a detail screen that
    /// disagreed with the roster about the same machine is exactly the drift
    /// that rule exists to stop. Building every row to return one is O(n)
    /// over a base's worth of structures, once per keypress.
    pub fn structure_manifest(&mut self, entity: Entity) -> Option<StructureReport> {
        self.structure_report()
            .into_iter()
            .find(|r| r.entity == entity)
    }

    /// Puts a scan's results in the order every menu built from one shows
    /// them: by name, then by position.
    ///
    /// Both scans need it and neither may differ from the other, since the
    /// cronjob picker and the symlink picker are lists of the same base. The
    /// position tiebreak is not cosmetic — bevy's query iteration order is
    /// not stable, so two Mining Nodes with nothing else to separate them
    /// would otherwise swap rows between openings of the same menu, and a
    /// list nobody can learn the shape of is worse than an unsorted one.
    fn sort_by_label(views: &mut [EntityView]) {
        views.sort_by(|a, b| (&a.label, a.pos).cmp(&(&b.label, b.pos)));
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
        self.build_views(hits)
    }

    /// Every tamed program the player owns, wherever it happens to be
    /// standing — the roster, not a window onto the map.
    ///
    /// A companion's `Position` is the tile it was beaten on and is never
    /// written again (see `worker_away_from_post` below), so a distance
    /// filter over it hides programs by where they were *captured* rather
    /// than by where they are. `owned_pets` made this move already, for the
    /// fusion picker; the posting menus need the same list, and neither
    /// `assign_cronjob` nor `assign_guard` has a distance requirement on the
    /// program to justify one. Shares `build_views` with `view_entities`
    /// rather than repeating it: the two differ only in which entities they
    /// select, exactly as `pursuit_field` differs from `walk_field`.
    pub fn owned_program_views(&mut self) -> Vec<EntityView> {
        let player = self.player_entity();
        let mut query = self.world.query::<(Entity, &Position, &Glyph, &Tamed)>();
        let hits: Vec<(Entity, Position, Glyph)> = query
            .iter(&self.world)
            .filter(|(_, _, _, t)| t.owner == player)
            .map(|(e, p, g, _)| (e, *p, *g))
            .collect();
        self.build_views(hits)
    }

    /// The `EntityView` for each of `hits`, whatever selected them.
    fn build_views(&mut self, hits: Vec<(Entity, Position, Glyph)>) -> Vec<EntityView> {
        let worker_by_structure: HashMap<Entity, Entity> = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            tasks
                .iter(&self.world)
                .map(|(worker, task)| (task.target, worker))
                .collect()
        };
        // Structures with a posted program standing at them right now.
        //
        // Separate from the map above, which is keyed by target and so
        // collapses a machine's worker and its guard into whichever the
        // query reached last — a machine whose worker has stepped out is
        // still attended by its guard, and has to survive that pairing.
        let attended: HashSet<Entity> = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            let posted: Vec<(Entity, Entity, TaskKind)> = tasks
                .iter(&self.world)
                .map(|(holder, task)| (holder, task.target, task.kind))
                .collect();
            posted
                .into_iter()
                .filter(|&(holder, target, kind)| match kind {
                    // Nothing ever walks a guard to what it guards, so it is
                    // standing wherever it was when assigned — and is never
                    // drawn, which makes "at its post" the only useful
                    // answer for it.
                    TaskKind::Guard => true,
                    TaskKind::GatherResource => {
                        match (
                            self.world.get::<Position>(holder),
                            self.world.get::<Position>(target),
                        ) {
                            (Some(w), Some(s)) => at_station(*w, *s),
                            _ => false,
                        }
                    }
                })
                .map(|(_, target, _)| target)
                .collect()
        };

        // Whether anywhere in the base can still take a load. Base-wide, and
        // rebuilt per call for the reason `haul_step_system` rebuilds its own
        // depot list every tick: a demolished or newly-filled depot has to
        // stop counting without anything having to notice it changed.
        let anywhere_to_unload = {
            let mut stores = self.world.query::<(&Structure, &Stock)>();
            let rooms: Vec<(StructureId, u32)> = stores
                .iter(&self.world)
                .map(|(s, stock)| (s.kind.clone(), stock.output_room()))
                .collect();
            let db = self.world.resource::<StructureDb>();
            rooms
                .iter()
                .any(|(kind, room)| *room > 0 && db.get(kind).is_some_and(|d| d.stores))
        };

        let player_power = self
            .world
            .get::<Stats>(self.player_entity())
            .unwrap()
            .power();
        let mut linked_edges = self.linked_edges_by_structure();

        let mut views: Vec<EntityView> = hits
            .into_iter()
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
                let (ceiling, max_tier) = match self.entity_upgrade_ceiling(entity) {
                    Some((c, m)) => (Some(c), Some(m)),
                    None => (None, None),
                };
                let can_work = self.accepts_a_program(entity);
                let machine_status = self.world.get::<MachineStatus>(entity).copied();
                let can_trade = self.trade_options(entity).is_some();
                let structure_worker = if is_structure {
                    worker_by_structure
                        .get(&entity)
                        .map(|&worker| self.entity_label(worker))
                } else {
                    None
                };
                let worker_away_from_post = is_tamed
                    && self.world.get::<Task>(entity).is_some_and(|t| {
                        t.kind == TaskKind::GatherResource
                            && self
                                .world
                                .get::<Position>(t.target)
                                .is_some_and(|s| !at_station(pos, *s))
                    });
                let structure_attended = is_structure && attended.contains(&entity);
                let output_stranded = is_structure
                    && !anywhere_to_unload
                    && self
                        .world
                        .get::<Stock>(entity)
                        .is_some_and(|s| s.output_room() == 0);
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
                    ceiling,
                    max_tier,
                    is_boss,
                    can_work,
                    can_trade,
                    structure_worker,
                    worker_away_from_post,
                    structure_attended,
                    output_stranded,
                    hp_fraction,
                    level,
                    durability,
                    fusions: self.fusion_count(entity),
                    rarity: self.rarity_of(entity),
                    machine_status,
                    linked_edges: linked_edges.remove(&entity).unwrap_or_default(),
                }
            })
            .collect();
        Self::sort_by_label(&mut views);
        views
    }

    /// For each structure, the orthogonal offsets of the neighbours it is
    /// joined to for production — the sides the map leaves un-outlined so a
    /// chain draws as one continuous shape.
    ///
    /// **Symmetric, though the feeding relation is not.** A Refinery names
    /// the Mining Node beside it; the Mining Node names nobody, because it
    /// has no recipe to want anything. Both walls between a joined pair have
    /// to go or the single remaining line reads as a rendering fault rather
    /// than as a join, so every link found is recorded from both ends.
    ///
    /// Reads the same `assembly_recipe` and walks the same `ORTHOGONAL` as
    /// `systems::assembler_system`'s pull phase, so a join can never be drawn
    /// where the pull phase would refuse to take. The one deliberate
    /// difference is documented on `EntityView::linked_edges`: this asks what
    /// a neighbour *makes*, not what is in its buffer this instant.
    ///
    /// Computed for the whole base in one pass rather than per structure:
    /// `view_entities` runs every frame, and asking each machine to re-scan
    /// every structure in the zone would be quadratic in the size of a base
    /// for a picture that only changes when something is built.
    pub(crate) fn linked_edges_by_structure(&mut self) -> HashMap<Entity, Vec<(i32, i32)>> {
        let mut query = self.world.query::<(Entity, &Position, &Structure)>();
        let placed: Vec<(Entity, Position, StructureId)> = query
            .iter(&self.world)
            .map(|(e, p, s)| (e, *p, s.kind.clone()))
            .collect();
        let by_tile: HashMap<(i32, i32), (Entity, &StructureId)> = placed
            .iter()
            .map(|(e, p, k)| ((p.x, p.y), (*e, k)))
            .collect();

        let db = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        let mut edges: HashMap<Entity, Vec<(i32, i32)>> = HashMap::new();
        for (entity, pos, kind) in &placed {
            let Some(recipe) = db
                .get(kind)
                .and_then(|def| crate::systems::assembly_recipe(def, items))
            else {
                continue;
            };
            for (dx, dy) in crate::game::collect::ORTHOGONAL {
                let Some((neighbour, neighbour_kind)) = by_tile.get(&(pos.x + dx, pos.y + dy))
                else {
                    continue;
                };
                let feeds = db
                    .get(neighbour_kind)
                    .and_then(crate::systems::produced_item)
                    .is_some_and(|made| recipe.iter().any(|(want, _)| want == made));
                if !feeds {
                    continue;
                }
                edges.entry(*entity).or_default().push((dx, dy));
                edges.entry(*neighbour).or_default().push((-dx, -dy));
            }
        }
        for dirs in edges.values_mut() {
            dirs.sort();
            dirs.dedup();
        }
        edges
    }

    /// Every structure in the zone and every program assigned to it, for the
    /// roster screen.
    ///
    /// Deliberately unbounded where `view_entities` takes a radius: the base
    /// sits within `MAX_BUILD_DISTANCE_FROM_HOME` of its Home, but the player
    /// wanders, and a roster that thinned out as they walked away would be
    /// worse than none. It is still zone-local — structures do not travel
    /// between zones, they are repositioned around the new spawn point (see
    /// `enter_next_zone`).
    ///
    /// Ordered Home first, then grouped by def id, then nearest first inside
    /// a group. Sorting here rather than in the frontend keeps one order for
    /// every consumer.
    pub fn structure_report(&mut self) -> Vec<StructureReport> {
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut structures = self.world.query::<(Entity, &Structure, &Position)>();
        let found: Vec<(Entity, StructureId, Position)> = structures
            .iter(&self.world)
            .map(|(e, s, p)| (e, s.kind.clone(), *p))
            .collect();

        // Grouped by target rather than mapped from it: a cronjob worker and
        // a guard can be posted on the same structure, and the roster exists
        // to show both.
        let mut assignees_by_structure: HashMap<Entity, Vec<Assignee>> = HashMap::new();
        let mut tasks = self.world.query::<(Entity, &Task)>();
        let posted: Vec<(Entity, Entity, TaskKind, u32, u32)> = tasks
            .iter(&self.world)
            .map(|(worker, task)| (worker, task.target, task.kind, task.progress, task.required))
            .collect();
        for (worker, target, kind, progress, required) in posted {
            assignees_by_structure
                .entry(target)
                .or_default()
                .push(Assignee {
                    entity: worker,
                    label: self.entity_label(worker),
                    kind,
                    progress,
                    required,
                });
        }

        let mut report: Vec<StructureReport> = found
            .into_iter()
            .map(|(entity, kind, pos)| {
                let workable = self.accepts_a_program(entity);
                let named = |map: Option<&std::collections::BTreeMap<ItemId, u32>>| {
                    map.map(|m| {
                        m.iter()
                            .map(|(item, n)| (self.item_name(item).to_string(), *n))
                            .collect()
                    })
                    .unwrap_or_default()
                };
                let stock = self.world.get::<Stock>(entity);
                StructureReport {
                    input: named(stock.map(|s| &s.input)),
                    output: named(stock.map(|s| &s.output)),
                    output_capacity: stock.map(|s| s.capacity).unwrap_or(0),
                    status: self.world.get::<MachineStatus>(entity).copied(),
                    entity,
                    is_home: kind == HOME_STRUCTURE_ID,
                    kind,
                    label: self.entity_label(entity),
                    pos: (pos.x, pos.y),
                    distance: (pos.x - center.x).abs().max((pos.y - center.y).abs()),
                    tier: self.world.get::<StructureTier>(entity).map(|t| t.0),
                    durability: self
                        .world
                        .get::<Durability>(entity)
                        .map(|d| (d.hp, d.max_hp)),
                    workable,
                    assignees: assignees_by_structure.remove(&entity).unwrap_or_default(),
                }
            })
            .collect();
        report.sort_by(|a, b| {
            b.is_home
                .cmp(&a.is_home)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.distance.cmp(&b.distance))
                .then_with(|| a.pos.cmp(&b.pos))
        });
        report
    }

    /// A species' affinities, or `None` if no such species loaded.
    pub fn species_affinities(&self, id: &str) -> Option<Affinities> {
        self.world
            .resource::<SpeciesDb>()
            .get(id)
            .map(|s| s.affinities)
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
                equipment: EquipmentSlot::ALL
                    .into_iter()
                    .filter_map(|slot| self.manifest_equip_slot(slot, equipment.get(slot)?))
                    .collect(),
                perk_points: perks.map(|p| p.points).unwrap_or(0),
                perks: perks
                    .map(|p| {
                        let db = self.world.resource::<PerkDb>();
                        Perk::all()
                            .into_iter()
                            .map(|perk| (perk, p.level(perk)))
                            .filter(|(_, level)| *level > 0)
                            .filter_map(|(perk, level)| Some((db.get(perk)?.name.clone(), level)))
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
        // Through `worn_bonus` rather than scaling here, so the sheet cannot
        // quote a figure the wearer's `Stats` disagree with — this was a
        // second copy of the chain, and the copy nobody runs is the one that
        // drifts.
        let mods = self.worn_bonus(&worn)?;
        Some(ManifestEquipSlot {
            slot: slot.label().to_string(),
            item_name: self.copy_name(&worn.copy),
            gear_level: worn.level,
            fusion_tier: worn.copy.tier,
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
        let bonuses = self.player_decompiler_bonuses();
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
                rarity: self.rarity_of(entity),
                refactors: self.refactor_count(entity),
                max_refactors: MAX_COMPANION_REFACTORS,
                zone_tier: self.zone_tier(entity),
                player_zone: self.world.resource::<ZoneLevel>().0,
                habitats: species.habitats.clone(),
                moves: species.moves.clone(),
                work_resource: species.work_resource.clone(),
                taming_difficulty: species.taming_difficulty,
                decompile_chance: self
                    .taming_catalyst()
                    .zip(self.target_resistance(entity))
                    .map(|((_, potency), resistance)| {
                        taming::capture_chance(potency, resistance, bonuses)
                    }),
                growth_multiplier: species.growth_multiplier,
                base_speed: species.base_speed,
                base_int: species.base_int,
                affinities: species.affinities.non_neutral(),
                base_job: species.affinity_class(),
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
        let mut views: Vec<EntityView> = hits
            .into_iter()
            .filter(|(_, _, _, kind)| db.get(kind).is_some_and(|d| d.teleport_cost.is_some()))
            .map(|(entity, pos, glyph, kind)| {
                let bounds = self.entity_upgrade_ceiling(entity);
                EntityView {
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
                    machine_status: None,
                    linked_edges: Vec::new(),
                    is_home: kind == HOME_STRUCTURE_ID,
                    tier: self.world.get::<StructureTier>(entity).map(|t| t.0),
                    ceiling: bounds.map(|(c, _)| c),
                    max_tier: bounds.map(|(_, m)| m),
                    is_boss: false,
                    can_work: false,
                    can_trade: false,
                    structure_worker: None,
                    worker_away_from_post: false,
                    structure_attended: false,
                    output_stranded: false,
                    hp_fraction: None,
                    level: None,
                    durability: self
                        .world
                        .get::<Durability>(entity)
                        .map(|d| (d.hp, d.max_hp)),
                    fusions: 0,
                    // Structures only, so there is no creature here to have
                    // rolled a tier.
                    rarity: Rarity::Ordinary,
                }
            })
            .collect();
        Self::sort_by_label(&mut views);
        views
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
        // Only once every check above has passed: a symlink that is refused
        // must not have surfaced the party on its way to refusing. Doing it
        // here also keeps `Position` from ever being written while
        // `Locale::Stack` is live, which is what `require_surface` guards
        // the other actions against — underground it *is* the entrance tile.
        let surfaced = self.is_underground();
        if surfaced {
            self.clear_stack();
        }
        {
            let mut pos = self.world.get_mut::<Position>(player).unwrap();
            pos.x = target_pos.x;
            pos.y = target_pos.y;
        }
        self.log(if surfaced {
            format!("The symlink hauls you up out of the stack and drops you at {name}.")
        } else {
            format!("You use a symlink and teleport to {name}.")
        });
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
