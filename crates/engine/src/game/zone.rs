//! The zone the player currently stands in: locating things on the map,
//! and stepping through a portal to the next zone.

use crate::items::DownedProgram;
use crate::tuning::{
    NEST_CACHE_CREDIT_ZONE_BONUS, NEST_CACHE_CREDITS, NEST_CACHE_EQUIPMENT_ROLLS,
    NEST_CACHE_PROGRAM_COUNT, NEST_ORPHAN_CHANCE,
};
use crate::*;

impl Game {
    pub(crate) fn find_wild_creature_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), (With<Creature>, Without<Tamed>)>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Finds a `Nest` at `(x, y)`, if any — checked in `move_player`
    /// before the ordinary blocking-structure check, so walking into a
    /// nest tile attacks it instead of just being blocked.
    pub(crate) fn find_nest_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Nest>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Finds a `Trap` at `(x, y)`, if any — checked in `move_player`'s
    /// ladder, so walking onto one collects or bumps instead of stepping.
    ///
    /// Answers the `Entity` and nothing else: every caller either despawns
    /// it or reads its component, and a second return shape would be a
    /// second place the read of `Trap` is spelled. `find_nest_at` is the
    /// precedent.
    pub(crate) fn find_trap_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<crate::components::Trap>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// How many traps stand anywhere in this zone — `TRAP_PLACEMENT_CAP`'s
    /// half of `place_trap`'s refusal ladder, and the one door a frontend
    /// has onto the question, since `Trap` is a component and the `World` is
    /// the engine's alone.
    pub fn trap_count(&mut self) -> usize {
        self.world
            .query_filtered::<(), With<crate::components::Trap>>()
            .iter(&self.world)
            .count()
    }

    /// Drops one unit of `item` on the tile `(dx, dy)` from the party, where
    /// it stands as a trap until it catches something or is destroyed.
    ///
    /// **Every refusal lands before anything is spent** —
    /// `commit_caravan_basket`'s rule — and the occupancy question is asked
    /// through the same finders `move_player`'s ladder uses, never a second
    /// set: a tile that is placeable to one function and occupied to another
    /// is how a trap ends up under a nest.
    ///
    /// **No `after_tick()` obligation.** This is reached through
    /// `App::handle_key`, whose tail already calls it — one of the three
    /// paths that do. A second call here would spend the tick twice.
    pub fn place_trap(&mut self, item: &ItemId, dx: i32, dy: i32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't place that right now.".into());
        }
        self.require_surface()?;
        let player = self.player_entity();
        let name = self.item_name(item).to_string();
        if self
            .world
            .get::<Inventory>(player)
            .map_or(0, |inv| inv.count(item))
            == 0
        {
            return Err(format!("You have no {name}."));
        }
        if !self.is_placeable(item) {
            return Err(format!("A {name} isn't something you can place."));
        }
        // Checked ahead of walkability deliberately: the cap is about the
        // per-tick cost and the carpet, so a player at it should be told
        // they are at it wherever they point.
        if self.trap_count() >= crate::tuning::TRAP_PLACEMENT_CAP {
            return Err(format!(
                "You already have {} out. Collect one first.",
                crate::tuning::TRAP_PLACEMENT_CAP
            ));
        }
        let pos = *self
            .world
            .get::<Position>(player)
            .ok_or_else(|| "You aren't anywhere you can place from.".to_string())?;
        let (nx, ny) = (pos.x + dx, pos.y + dy);
        if !self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable {
            return Err("Nothing would hold it there.".into());
        }
        if self.find_wild_creature_at(nx, ny).is_some()
            || self.find_nest_at(nx, ny).is_some()
            || self.find_surface_link_at(nx, ny).is_some()
            || self.find_settlement_at(nx, ny).is_some()
            || self.find_trap_at(nx, ny).is_some()
        {
            return Err("Something is already there.".into());
        }

        self.world
            .get_mut::<Inventory>(player)
            .expect("the count above read the player's own inventory")
            .take(item.clone(), 1);
        self.world.spawn((
            crate::components::Trap {
                item: item.clone(),
                next_roll: crate::tuning::TRAP_PERIOD_TICKS,
                caught: None,
            },
            Position { x: nx, y: ny },
            Glyph {
                ch: crate::components::TRAP_GLYPH_ARMED,
                color: GlyphColor::Yellow,
            },
        ));
        self.log(format!("You set a {name} down. Now it waits."));
        self.tick();
        Ok(())
    }

    /// Destroys the trap on the tile `(dx, dy)` from the party.
    ///
    /// **Hands back nothing** — no material refund, and a sprung one loses
    /// what it caught. Reusing the demolish gesture with no confirmation is
    /// a taken decision: the trap is on the ground in front of the player
    /// and the only way to be holding one again is to compile it.
    pub fn destroy_trap(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        self.require_surface()?;
        let player = self.player_entity();
        let pos = *self
            .world
            .get::<Position>(player)
            .ok_or_else(|| "You aren't anywhere you can reach from.".to_string())?;
        let trap = self
            .find_trap_at(pos.x + dx, pos.y + dy)
            .ok_or_else(|| "Nothing of yours to demolish there.".to_string())?;
        let label = self.entity_label(trap);
        self.world.despawn(trap);
        self.log(format!("You break the {label} down. Nothing comes back."));
        self.tick();
        Ok(())
    }

    /// Deals one hit of the player's `effective_atk` (against no defense
    /// — a nest has none, only a `Durability` pool) to `nest`. A nest
    /// never retaliates, unlike an ordinary wild-creature encounter — see
    /// the nests design doc for why this deliberately isn't routed
    /// through `BattleState`. Destroying it strips `NestGuardian` from
    /// every creature tethered to it (they resume ordinary wandering) and
    /// despawns the nest, which implicitly cancels anything left in its
    /// `Nest::pending_respawns`.
    pub(crate) fn attack_nest(&mut self, nest: Entity) {
        // On every hit, not just the first: a guardian that wandered
        // outside its tether and walked home (see `wander_ai_system`)
        // would otherwise go unprovoked by the next swing.
        self.provoke_nest(nest);
        let player = self.player_entity();
        let label = self.entity_label(nest);
        // Deterministic, and shared with the base's rock — see
        // `Game::swing_damage` for why neither goes through
        // `battle::resolve_attack`.
        let dmg = self.swing_damage(player);
        let Some(mut durability) = self.world.get_mut::<Durability>(nest) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        if destroyed {
            self.log(format!("The {label} crashes and collapses!"));
            // Read before `despawn_nest` deletes the entity carrying it —
            // `grant_nest_cache`'s own ordering constraint, one line over.
            if let Some(pos) = self.world.get::<Position>(nest) {
                let tile = (pos.x, pos.y);
                self.credit_nearby_settlements(
                    tile,
                    crate::tuning::SETTLEMENT_NEST_CLEARED_STANDING,
                );
            }
            self.note_deed(crate::contracts::Deed::ClearedNest);
            // Reads Nest::species, so this has to run before despawn_nest
            // deletes the component it's reading.
            self.grant_nest_cache(nest);
            self.despawn_nest(nest);
        } else {
            self.log(format!(
                "You unleash a data strike into the {label} for {dmg} damage."
            ));
        }
    }

    /// Pays the loot a destroyed `nest` owes, drawn entirely from its
    /// species' existing `SpeciesDef` fields — content stays data, only the
    /// `NEST_CACHE_*` magnitudes in `tuning.rs` are code. Mirrors
    /// `award_loot`'s clone-then-drop-the-borrow shape (`combat_rewards.rs`)
    /// so the `SpeciesDb` resource isn't held live across a `grant_loot`
    /// call that itself needs the world.
    ///
    /// No XP: the guardians already paid that on the way down, and this is
    /// the structure itself coming down.
    pub(crate) fn grant_nest_cache(&mut self, nest: Entity) {
        let Some(species_id) = self.world.get::<Nest>(nest).map(|n| n.species.clone()) else {
            return;
        };
        let Some(species) = self.world.resource::<SpeciesDb>().get(&species_id).cloned() else {
            return;
        };

        // `NEST_CACHE_PROGRAM_COUNT` programs of the nest's own species
        // rather than the guardians' own kills paying twice — each
        // guardian already left its own downed program on the way down
        // (`Game::leave_downed_program`, from the ordinary kill path); this
        // is the wreckage itself, on top of that.
        //
        // `ability_user_level(nest)` rather than a second hand-rolled
        // derivation: a `Nest` carries no `Experience` either, so this is
        // the same "no `Experience`, read `ZoneLevel`" answer
        // `downed_program_for` gives a wild `Creature` — one function
        // both sites call, hoisted out of the loop since it doesn't change
        // between programs.
        let level = self.ability_user_level(nest);
        for _ in 0..NEST_CACHE_PROGRAM_COUNT {
            let condition = DownedProgram::roll_condition(Rarity::Ordinary, false, 0.0);
            let landed = self.push_downed_program(DownedProgram {
                species: species_id.clone(),
                level,
                rarity: Rarity::Ordinary,
                boss: false,
                condition,
                // Wreckage, not a kill: no individual was running anything,
                // so there is no carrier's prize in here.
                carried: None,
            });
            if landed {
                self.log_kind(
                    MessageKind::Loot,
                    format!("The wreckage yields a downed {}.", species.name),
                );
            }
        }

        for _ in 0..NEST_CACHE_EQUIPMENT_ROLLS {
            for (item, chance) in self.equipment_drops_for(&species) {
                let roll = {
                    let mut rng = self.world.resource_mut::<GameRng>();
                    rng.0.random_bool(chance.clamp(0.0, 1.0) as f64)
                };
                if roll {
                    let copy = self.grant_gear_drop(item, Rarity::Ordinary);
                    self.log_kind(
                        MessageKind::Loot,
                        format!("The wreckage also yields a {}!", self.drop_label(&copy)),
                    );
                }
            }
        }

        let zone_bonus = {
            let zone = self.world.resource::<ZoneLevel>().0;
            NEST_CACHE_CREDIT_ZONE_BONUS * zone.saturating_sub(1)
        };
        let qty = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(NEST_CACHE_CREDITS) + zone_bonus
        };
        let landed = self.grant_loot(self.trade_currency(), qty, LootSource::Cache);
        if landed > 0 {
            self.log_kind(
                MessageKind::Loot,
                format!("The cache holds {landed} credits!"),
            );
        }

        self.leave_nest_orphan(nest, &species_id);
    }

    /// Rolls `NEST_ORPHAN_CHANCE` for the thing a nest is actually cleared
    /// for: a program of the nest's **own** species, left running in the
    /// wreckage and joining the roster free.
    ///
    /// Its own species rather than a habitat draw so which nest the player
    /// walks up to is a real choice — you hunt the nest of the program you
    /// want. Free where `Game::adopt_orphan` charges a taming catalyst,
    /// because the Stack's orphan is an opportunity walked past and a nest
    /// is a fight already paid for.
    ///
    /// A full roster loses it, and says so. The alternative — refusing to
    /// destroy the nest at all — would make a structure's destruction
    /// conditional on unrelated state, and by the time this runs the caller
    /// has already committed to the `despawn_nest` on the next line.
    ///
    /// That ordering is also why the nest's `Position` is still readable
    /// here: it is the last thing to read anything off the entity, and it
    /// shares the reason `grant_nest_cache` itself runs before the despawn.
    fn leave_nest_orphan(&mut self, nest: Entity, species_id: &str) {
        let left_behind = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(NEST_ORPHAN_CHANCE)
        };
        if !left_behind {
            return;
        }
        let Some(at) = self.world.get::<Position>(nest).copied() else {
            return;
        };
        if self.roster_room() == 0 {
            self.log_kind(
                MessageKind::Loot,
                "Something small was still running in the wreckage. You have no room for it.",
            );
            return;
        }
        // Scaled like any other spawn in this zone; the depth multiplier a
        // Stack orphan carries has no meaning on the surface.
        let Some(program) = self.adopt_program(species_id, at.x, at.y, 1.0) else {
            return;
        };
        let name = self.creature_label(program);
        self.log_kind(
            MessageKind::Loot,
            format!("{name} was still running in the wreckage. It comes with you."),
        );
    }

    /// Despawns `nest`, first stripping `NestGuardian` and `Pursuing` from
    /// every creature tethered to it so none is left pointing at a dead
    /// entity or chasing on its behalf — they resume ordinary wandering.
    /// Despawning implicitly cancels anything left in
    /// `Nest::pending_respawns`.
    pub(crate) fn despawn_nest(&mut self, nest: Entity) {
        let guardians: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&self.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        for guardian in guardians {
            self.world
                .entity_mut(guardian)
                .remove::<(NestGuardian, Pursuing)>();
        }
        self.world.despawn(nest);
    }

    /// Sets `Pursuing` on every living guardian tethered to `nest` — the
    /// whole effect of an attack landing. Collected in an inner scope
    /// first so the query's borrow of `self.world` ends before the
    /// `entity_mut` loop, the same shape `despawn_nest` above uses.
    pub(crate) fn provoke_nest(&mut self, nest: Entity) {
        let guardians: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&self.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        for guardian in guardians {
            self.world.entity_mut(guardian).insert(Pursuing);
        }
    }

    /// Whether `nest` currently has at least one living guardian marked
    /// `Pursuing` — `nest_respawn_tick` uses this so a replacement spawned
    /// while the nest is under siege arrives already provoked, instead of
    /// standing there calm until the player's next hit reaches it.
    pub(crate) fn nest_has_pursuers(&mut self, nest: Entity) -> bool {
        let mut query = self.world.query::<(&NestGuardian, Option<&Pursuing>)>();
        query
            .iter(&self.world)
            .any(|(g, pursuing)| g.nest == nest && pursuing.is_some())
    }

    /// Every cell a deployed structure's **footprint** covers, anchor and
    /// floor alike — every cell of a 2x2 Research Station, not just the one
    /// that blocks.
    ///
    /// **The wider of the two sets, and the pair is not interchangeable.**
    /// `blocked_tiles` below answers "may a body step onto that cell", and a
    /// structure's own floor cells are exactly the ones it answers yes for;
    /// this one answers "is that cell spoken for by a structure at all",
    /// which is a question about the ground rather than about where a body
    /// may walk. The one caller is `hauling::has_station`, whose own doc has
    /// why that distinction — and not merely counting bodies — is what
    /// deadlocks the dig crew: a marked cell whose only free walkable face
    /// is a Station's own floor cell is refused here even though
    /// `blocked_tiles` would let a body stand on it, because this set counts
    /// that floor cell taken.
    pub(crate) fn structure_tiles(&mut self) -> std::collections::HashSet<(i32, i32)> {
        let rows = self.structure_footprints();
        crate::game::base::hauling::footprint_tiles(rows.into_iter().map(|(_, p, side)| (p, side)))
    }

    /// Every cell a walk in base space refuses — every deployed structure's
    /// **anchor** and every body standing in one — from the `Game` side.
    /// `haul_step_system` builds the same set from its own queries; both go
    /// through `hauling::blocked_tiles` so the two cannot disagree about what
    /// a blocked cell is.
    ///
    /// **The narrower of the two sets** — see `structure_tiles`' doc for why
    /// a structure's own floor cells are walkable here and taken there.
    pub(crate) fn blocked_tiles(&mut self) -> std::collections::HashSet<(i32, i32)> {
        let rows = self.structure_footprints();
        let bodies: Vec<Position> = self.base_bodies().into_iter().map(|(_, p)| p).collect();
        crate::game::base::hauling::blocked_tiles(
            rows.into_iter().map(|(_, p, side)| (p, side)),
            bodies.into_iter(),
        )
    }

    /// Every body standing in base space, with the cell it is standing in.
    ///
    /// **The `Game`-side half of `party::walks_the_base`**, which is where the
    /// rule itself lives and why this is a list rather than a predicate: the
    /// three readers want different things out of it. `blocked_tiles` takes
    /// the cells, `Game::place_structure` looks one up, and
    /// `drift_idle_staff` folds them into a tally to find out who is sharing.
    ///
    /// A program whose `Position` is the surface tile it was beaten on is held
    /// out by the rule, not by a coordinate test: base space and the zone
    /// surface alias onto each other freely — both origins are usually
    /// `(0, 0)` — so a stale tile is indistinguishable from a real one by
    /// looking at it.
    pub(crate) fn base_bodies(&mut self) -> Vec<(Entity, Position)> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position, Option<&Task>), With<Tamed>>();
        let candidates: Vec<(Entity, Position, Option<TaskKind>)> = query
            .iter(&self.world)
            .map(|(e, p, task)| (e, *p, task.map(|t| t.kind)))
            .collect();
        candidates
            .into_iter()
            .filter(|&(entity, _, task)| {
                crate::game::party::walks_the_base(self.program_role(entity), task)
            })
            .map(|(entity, pos, _)| (entity, pos))
            .collect()
    }

    /// The `Structure` standing at `(x, y)`, if any — and `None` outright
    /// whenever the party is not in base space, regardless of what `(x, y)`
    /// numerically is.
    ///
    /// **`Structure` is the space tag** (see
    /// `seam:structure-is-the-space-tag`): every structure stands in
    /// `base_grid::BaseGrid`'s coordinate space, never the zone surface, so a
    /// `Structure` query only ever answers a base-space question. Gating on
    /// `in_base` closes that generally instead of at each call site —
    /// `game/stack.rs`'s `link_site_free` used to call this with **surface**
    /// coordinates while scattering Stack entrances, and with the zone spawn
    /// point and base space's origin both commonly `(0, 0)`, it silently
    /// refused valid link sites near a base. The one legitimate caller left
    /// with `(x, y)` computed while off base (`place_structure`'s founding
    /// Home) is asking about a base that cannot exist yet — no Home means no
    /// other structure either, since removing a Home cascades to every
    /// structure it stands beside — so `None` is the right answer there too,
    /// not a special case.
    /// **Point-in-footprint, not point-equality.** `(x, y)` need not be a
    /// structure's own anchor — this answers for any cell any standing
    /// structure's footprint covers, so a build, a walk or an examine
    /// pointed at a Research Station's floor cell is answered for the
    /// Station rather than reading as open ground.
    pub(crate) fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        if !self.in_base() {
            return None;
        }
        self.structure_footprints()
            .into_iter()
            .find(|(_, p, side)| {
                crate::tactical::footprint_cells_at((p.x, p.y), *side).contains(&(x, y))
            })
            .map(|(e, _, _)| e)
    }

    /// A structure kind's footprint side — `1` for anything that fails to
    /// resolve, the same lenient answer an absent `footprint` field parses
    /// to.
    pub(crate) fn structure_footprint(&self, kind: &StructureId) -> u8 {
        self.world
            .resource::<StructureDb>()
            .get(kind)
            .map(|def| def.footprint)
            .unwrap_or(1)
    }

    /// A footprint's side, resolved off an entity's own `Structure` — `1`
    /// for anything that isn't one, a `DigSite` or `BuildSite` included, so
    /// a single call answers correctly whichever `TaskKind::Excavate` or
    /// `GatherResource` hands it.
    pub(crate) fn structure_footprint_of(&self, entity: Entity) -> u8 {
        match self.world.get::<Structure>(entity) {
            Some(s) => self.structure_footprint(&s.kind),
            None => 1,
        }
    }

    /// Every standing structure, with its footprint's side resolved —
    /// `find_blocking_structure_at`, `structure_tiles` and `blocked_tiles`
    /// all read these same rows rather than each building their own.
    pub(crate) fn structure_footprints(&mut self) -> Vec<(Entity, Position, u8)> {
        let mut query = self.world.query::<(Entity, &Position, &Structure)>();
        let rows: Vec<(Entity, Position, StructureId)> = query
            .iter(&self.world)
            .map(|(e, p, s)| (e, *p, s.kind.clone()))
            .collect();
        rows.into_iter()
            .map(|(e, p, kind)| {
                let side = self.structure_footprint(&kind);
                (e, p, side)
            })
            .collect()
    }

    /// Whether `entity` is a structure whose def is a barrier
    /// (`StructureDef::barrier`) — false for anything else, a def that no
    /// longer resolves included.
    pub(crate) fn is_barrier(&self, entity: Entity) -> bool {
        self.world
            .get::<Structure>(entity)
            .and_then(|s| self.world.resource::<StructureDb>().get(s.kind.as_str()))
            .is_some_and(|def| def.barrier)
    }

    /// The Home structure's position, if one is deployed anywhere right
    /// now — the anchor `place_structure` measures the build radius from.
    pub(crate) fn home_position(&mut self) -> Option<Position> {
        let mut query = self.world.query::<(&Structure, &Position)>();
        query
            .iter(&self.world)
            .find(|(s, _)| s.kind == HOME_STRUCTURE_ID)
            .map(|(_, p)| *p)
    }

    /// Finds a zone-portal structure (`StructureDef::zone_portal`) at
    /// `(x, y)`, if any — checked from `Game::move_in_base` so walking onto
    /// one breaches the zone. `(x, y)` is a base-space coordinate: a Portal
    /// is a `Structure`, and every `Structure` stands in base space now, so
    /// this refuses to answer at all outside it — see
    /// `find_blocking_structure_at`'s doc for why that guard belongs here
    /// rather than at each caller.
    pub(crate) fn find_zone_portal_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        if !self.in_base() {
            return None;
        }
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position, &Structure), ()>();
        let (entity, kind) = query
            .iter(&self.world)
            .find(|(_, p, _)| p.x == x && p.y == y)
            .map(|(e, _, s)| (e, s.kind.clone()))?;
        self.world
            .resource::<StructureDb>()
            .get(&kind)
            .is_some_and(|d| d.zone_portal)
            .then_some(entity)
    }

    /// A barrier structure (`StructureDef::barrier`) standing on base-space
    /// `(x, y)`, if any — `find_zone_portal_at`'s shape, asked by
    /// `Game::move_in_base` before the step.
    pub(crate) fn barrier_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Structure>>();
        let at: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
            .collect();
        at.into_iter().find(|&e| self.is_barrier(e))
    }

    /// This run's enemy-strength band — `resources::EnemyStrength`.
    pub fn enemy_strength(&self) -> crate::resources::EnemyStrength {
        *self.world.resource::<crate::resources::EnemyStrength>()
    }

    /// The multiplier a spawn takes for sitting `steps` **zone steps** up
    /// its own zone's curve.
    ///
    /// The one derivation of that ratio, shared by the two things that buy
    /// steps — the distance ramp in `Game::field_stat_mult` and the
    /// enemy-strength band — so the two cannot disagree about what a step is
    /// worth. Expressed as a ratio on the zone ladder rather than a curve of
    /// its own, which is what leaves `balance_sim`'s per-zone sweeps gating
    /// both of them: `steps` of 1 at zone N is arithmetically zone N+1.
    pub(crate) fn zone_curve_ratio(&self, steps: f32) -> f32 {
        let zone = self.world.resource::<ZoneLevel>().stat_multiplier() as f32;
        (zone + crate::tuning::ZONE_STAT_STEP as f32 * steps) / zone
    }

    /// Moves the run to `band` and re-stocks the ground already walked at
    /// it — the same three calls `enter_next_zone` makes below, and for the
    /// same reason: a difficulty term is baked into a body's `Stats` at
    /// spawn, so without the re-stock the knob would only be felt on ground
    /// the party had not reached yet.
    ///
    /// **Refused mid-fight.** `clear_local_wild` despawns bodies, and a body
    /// sitting in `BattleState::groups` is exactly what `end_battle` exists
    /// to stop anyone deleting — `BattleState::planned` indexes `Party`
    /// positionally and the wild side is named by identity.
    ///
    /// Two things the re-stock deliberately does not reach, both of them
    /// `clear_local_wild`'s own exclusions: a `NestGuardian` is tethered to
    /// its nest and a `Nemesis` is a named individual, so each keeps the
    /// stats it spawned with until it dies. And only chunks within
    /// `POPULATION_CHUNK_MARGIN` are touched — ground walked into later has
    /// never been populated and stocks at the new band anyway.
    pub fn set_enemy_strength(
        &mut self,
        band: crate::resources::EnemyStrength,
    ) -> Result<(), String> {
        if self.has_active_battle() {
            return Err("Not in the middle of a fight.".to_string());
        }
        self.world.insert_resource(band);
        self.world
            .insert_resource(crate::resources::PopulatedChunks::default());
        self.clear_local_wild();
        self.ensure_local_population();
        Ok(())
    }

    /// The band without the re-stock or the refusal — a fixture, and
    /// `#[cfg(test)]` because it is one. `spawn_wild_creature`'s reason:
    /// ungated it reads like the plain way to set the band, which is how a
    /// caller ends up quietly skipping the re-stock.
    #[cfg(test)]
    pub(crate) fn force_enemy_strength(&mut self, band: crate::resources::EnemyStrength) {
        self.world.insert_resource(band);
    }

    /// Raises the world's tier.
    ///
    /// A breach used to be a migration: every hostile, nest and Stack
    /// entrance despawned, a fresh `WorldMap` carved from a fresh seed,
    /// the party and the base anchor teleported onto it, and a zone's
    /// worth of economy — the buyback shelves, the caravan, the spendable
    /// currencies — destroyed on the way through. Nothing of the sector
    /// you left survived, which is why nothing in it was ever worth
    /// knowing.
    ///
    /// The world is persistent now. There is one map for the run, minted
    /// at `Game::new`, and a breach raises the tier that everything
    /// spawned into it is scaled against. The party does not move; the
    /// ground under them does not change; what changes is what walks on
    /// it. That is the infrastructure settlements need — a place can only
    /// be worth returning to if it is still there.
    ///
    /// Two lines below look like the wipe code that was deleted around
    /// them and are the opposite — they are the mechanism:
    ///
    /// - Clearing `PopulatedChunks` is what makes the world visibly harden.
    ///   It marks which chunks have been stocked, so emptying it sends
    ///   `Game::ensure_local_population` back over ground it has already
    ///   covered to re-stock it at the new tier. It is paired with
    ///   `Game::clear_local_wild` and is inert without it: `populate_chunk`
    ///   counts the survivors against `WILD_LOCAL_DENSITY_TARGET`, so on
    ///   ground already worked an unpaired re-stock fills only the gaps and
    ///   leaves the old tier standing.
    /// - Clearing `StackMemory` is what makes an entrance re-tier. A
    ///   surviving link keys a `FrameSpec` that now folds in the tier, so
    ///   the frame behind it is re-carved and the memory of the old one —
    ///   which cells were seen, which caches were emptied, which lair was
    ///   cleared — describes a frame that no longer exists.
    pub(crate) fn enter_next_zone(&mut self) {
        self.notify(crate::notifications::NotificationKind::Breach);

        let new_level = {
            let mut zone = self.world.resource_mut::<ZoneLevel>();
            zone.0 += 1;
            zone.0
        };

        // Equality, not `>=`: the warning is about sweeps *starting*, and a
        // `>=` gate would repeat it at every breach for the rest of the run.
        // Read off the constant `Game::raid_check` gates on, so the screen
        // and the gate cannot come to disagree about which sector it is.
        if new_level == crate::tuning::RAID_MIN_ZONE {
            self.notify(crate::notifications::NotificationKind::SweepsBegin);
        }

        self.world.insert_resource(StackMemory::default());
        self.world
            .insert_resource(crate::resources::PopulatedChunks::default());

        self.log(format!(
            "You breach the portal and materialize in a level {new_level} sector. Hostile signal strength has spiked."
        ));

        self.clear_local_wild();
        self.ensure_local_population();
        self.ensure_local_settlements();
    }

    /// Breaches forward until the party is standing in `zone`, for the
    /// `savetool` binary — testing zone 6 otherwise means playing to zone 6.
    ///
    /// Deliberately a loop over the real `enter_next_zone` rather than a
    /// write to `ZoneLevel`: everything that makes a breach coherent — the
    /// two resets, and the ground re-stocking at each tier on the way past
    /// — lives in that function, and a shortcut would produce a save that no
    /// amount of play could have reached. That matters more now, not less:
    /// a written tier is a world that never hardened. `enter_next_zone` is
    /// `pub(crate)` and a `src/bin/` target is a separate crate, so this is
    /// also the seam that lets the tool reach it at all.
    ///
    /// Only runs forward: a breach consumes the portal and there is no way
    /// back, so a backwards warp is refused rather than silently ignored.
    pub fn warp_to_zone(&mut self, zone: u32) -> Result<(), String> {
        let current = self.world.resource::<ZoneLevel>().0;
        if zone <= current {
            return Err(format!(
                "already in zone {current}; a breach only runs forward, so zone {zone} is unreachable"
            ));
        }
        for _ in current..zone {
            self.enter_next_zone();
        }
        Ok(())
    }

    /// Where the player materialized on breaching into the current zone —
    /// see `resources::ZoneSpawnPoint`. Not drawn on the map: the outline
    /// that used to mark it was removed, and what the point still decides is
    /// the centre `in_opening_ring` and `frames_at` measure from.
    pub fn zone_spawn_point(&self) -> (i32, i32) {
        let p = self.world.resource::<ZoneSpawnPoint>();
        (p.x, p.y)
    }
}

/// The first walkable tile found spiralling out from the origin — where the
/// player is dropped when a zone is generated.
pub(crate) fn find_walkable_start(world_map: &mut WorldMap) -> (i32, i32) {
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
