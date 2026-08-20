//! The zone the player currently stands in: locating things on the map,
//! and stepping through a portal to the next zone.

use crate::tuning::{
    NEST_CACHE_CREDIT_ZONE_BONUS, NEST_CACHE_CREDITS, NEST_CACHE_EQUIPMENT_ROLLS,
    NEST_CACHE_WORK_RESOURCE_MULT, NEST_ORPHAN_CHANCE, STACK_LINKS_PER_ZONE, WORK_RESOURCE_DROP,
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
        // A structure has no speed and cannot dodge, so this keeps the
        // deterministic path it always had — only creature-versus-creature
        // attacks go through `battle::resolve_attack`. Identical swings have
        // to stay identical, or wearing a nest down becomes a slot machine.
        let range = self.attack_range(player, crate::tuning::PLAYER_UNARMED_DAMAGE);
        let dmg = (range.mean().round() as i32 + self.effective_atk(player)).max(1) as u32;
        let Some(mut durability) = self.world.get_mut::<Durability>(nest) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        if destroyed {
            self.log(format!("The {label} crashes and collapses!"));
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

        if let Some(resource) = &species.work_resource {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(WORK_RESOURCE_DROP) * NEST_CACHE_WORK_RESOURCE_MULT
            };
            let landed = self.grant_loot(resource.clone(), qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!(
                        "The wreckage yields {} {}.",
                        landed,
                        self.item_name(resource)
                    ),
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
        let landed = self.grant_loot(self.trade_currency(), qty);
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
        if self.pet_count() >= self.pet_capacity() {
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

    /// Every tile a deployed structure stands on — the set a hauler's walk
    /// refuses, from the `Game` side. `haul_step_system` builds the same set
    /// from its own query; both go through `hauling::structure_tiles` so the
    /// two cannot disagree about what a blocked tile is.
    pub(crate) fn structure_tiles(&mut self) -> std::collections::HashSet<(i32, i32)> {
        let mut query = self.world.query_filtered::<&Position, With<Structure>>();
        let positions: Vec<Position> = query.iter(&self.world).copied().collect();
        crate::game::base::hauling::structure_tiles(positions.into_iter())
    }

    /// The `Structure` standing at `(x, y)`, if any — and `None` outright
    /// whenever the party is not in base space, regardless of what `(x, y)`
    /// numerically is.
    ///
    /// **`Structure` is the space tag** (see `docs/seams.md`): every
    /// structure stands in `base_grid::BaseGrid`'s coordinate space, never
    /// the zone surface, so a `Structure` query only ever answers a
    /// base-space question. Gating on `in_base` closes that generally
    /// instead of at each call site — `game/stack.rs`'s `link_site_free`
    /// used to call this with **surface** coordinates while scattering
    /// Stack entrances, and with the zone spawn point and base space's
    /// origin both commonly `(0, 0)`, it silently refused valid link sites
    /// near a base. The one legitimate caller left with `(x, y)` computed
    /// while off base (`place_structure`'s founding Home) is asking about a
    /// base that cannot exist yet — no Home means no other structure either,
    /// since removing a Home cascades to every structure it stands beside —
    /// so `None` is the right answer there too, not a special case.
    pub(crate) fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        if !self.in_base() {
            return None;
        }
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Structure>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
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

    /// Any deployed structure whose def sets `enables_rest` and is within
    /// its radius of `player_pos` — gates `Game::rest`.
    pub(crate) fn nearby_rest_structure(&mut self, player_pos: Position) -> Option<Entity> {
        let mut query = self.world.query::<(Entity, &Structure, &Position)>();
        let hits: Vec<(Entity, StructureId, Position)> = query
            .iter(&self.world)
            .map(|(e, s, p)| (e, s.kind.clone(), *p))
            .collect();
        let db = self.world.resource::<StructureDb>();
        hits.into_iter().find_map(|(entity, kind, pos)| {
            let radius = db.get(&kind)?.enables_rest.as_ref()?.radius;
            if (pos.x - player_pos.x).abs() <= radius && (pos.y - player_pos.y).abs() <= radius {
                Some(entity)
            } else {
                None
            }
        })
    }

    /// The `RestDef::cost` of whichever structure `nearby_rest_structure`
    /// found, resolved to an owned `Vec` so `Game::rest` can drop this
    /// `StructureDb` borrow before taking a mutable one on the player's
    /// `Inventory` for the same call. Priced on the structure that actually
    /// granted the rest, never on Home by id, so a modded alternate rest
    /// structure can charge differently.
    pub(crate) fn rest_cost(&self, rest_structure: Entity) -> Vec<(ItemId, u32)> {
        let kind = &self.world.get::<Structure>(rest_structure).unwrap().kind;
        self.world
            .resource::<StructureDb>()
            .get(kind)
            .and_then(|def| def.enables_rest.as_ref())
            .map(|rest| rest.cost.clone())
            .unwrap_or_default()
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

    /// Breaches the player (and any tamed programs they own) forward into
    /// the next zone sector: wild programs and nests are left behind
    /// (despawned — there's no portal back down), a fresh sector is
    /// generated from a new seed, and wild programs there spawn with stats
    /// scaled by the new zone's `ZoneLevel::stat_multiplier`.
    ///
    /// The player's base travels with them. Every structure is repositioned
    /// around the new spawn point at its existing offset from the Home, and
    /// the platform slab is re-stamped beneath it, so the base arrives in
    /// exactly the layout it left in. The Portal itself is consumed by
    /// `move_player` before this runs, so it never makes the trip.
    ///
    /// Spendable currency doesn't make the trip either — see the wipe at the
    /// end of this function. Gear, supplies, fusion tiers and banked Research
    /// Data all do.
    pub(crate) fn enter_next_zone(&mut self) {
        // The base is out of phase, not on the zone surface, so a breach
        // does not touch it — no despawn, no reposition, nothing. What
        // still has to be swept is what actually is zone-local: a
        // `SurfaceLink` opens onto a frame generated for the sector it
        // stands in, and left alive it would ride the breach into a sector
        // it was never generated for.
        let stale: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, Or<(With<Hostile>, With<Nest>, With<SurfaceLink>)>>();
            query.iter(&self.world).collect()
        };
        for e in stale {
            self.world.despawn(e);
        }

        let new_level = {
            let mut zone = self.world.resource_mut::<ZoneLevel>();
            zone.0 += 1;
            zone.0
        };
        let new_seed = self
            .world
            .resource::<WorldMap>()
            .seed()
            .wrapping_add(0x9E37_79B9);
        let mut new_map = crate::sectors::map_for_zone(
            new_seed,
            new_level,
            self.world.resource::<crate::sectors::SectorDb>(),
        );
        let start = find_walkable_start(&mut new_map);
        self.world.insert_resource(new_map);
        self.world.insert_resource(ZoneSpawnPoint {
            x: start.0,
            y: start.1,
        });

        let travelers: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, Or<(With<Player>, With<Tamed>)>>();
            query.iter(&self.world).collect()
        };
        for e in travelers {
            if let Some(mut pos) = self.world.get_mut::<Position>(e) {
                pos.x = start.0;
                pos.y = start.1;
            }
        }

        // The anchor is not zone-local the way a `SurfaceLink` is — it is
        // not in the stale sweep above and so survives the breach at its
        // old coordinates — but a door back to a base that travels with the
        // party has to travel with it too. Moved rather than despawned and
        // respawned, so `resources::AnchorEntity` still names the same
        // entity on both sides of a breach.
        let anchor = self.world.resource::<AnchorEntity>().0;
        if let Some(mut pos) = self.world.get_mut::<Position>(anchor) {
            pos.x = start.0;
            pos.y = start.1;
        }

        // Build salvage and breach keys are zone-local: the next breach has
        // to be funded in the zone you leave from, so a stockpile can't chain
        // breaches past content it never engaged with. Keyed on economy role,
        // so a mod's own items reset without an engine change.
        //
        // Research Data is banked progress rather than spending money and
        // survives. So do Credits, by omission and on purpose — converting a
        // doomed stockpile into money before you breach is what a trader is
        // *for*.
        //
        // Credits used to be able to buy a Portal Fragment on the far side,
        // so a stockpile could in principle fund a breach out of a zone it
        // never worked. That was priced rather than forbidden, because the
        // Market's fragment listing was then the only route from base
        // production to progression. It has since been pulled — breaching is
        // earned by fighting — so the hole is closed at the source and
        // Credits surviving a breach no longer buys a way past content.
        // Every trader's shelf goes with it, for the same reason: a shelf
        // holding a zone's worth of salvage is precisely the stockpile that
        // wipe exists to strand. Cleared explicitly rather than left to rot
        // — a trader is base-space and permanent, so without this its
        // ledger would otherwise just keep accumulating sale history across
        // every zone the run ever passes through, rather than resetting
        // with the rest of that zone's economy.
        self.world.insert_resource(BuybackLedger::default());

        // Same reason, and the same trap: a zone's links are gone, but their
        // maps are keyed by tile and would otherwise draw the last sector's
        // walked corridors onto a fresh link that happens to land on a
        // matching coordinate.
        self.world.insert_resource(StackMemory::default());

        // And the same reason a third time, with a sharper edge: a mark left
        // behind tells the new sector that ground it has never stocked is
        // already full, so every chunk the old zone had populated would be
        // born empty here and stay that way.
        self.world
            .insert_resource(crate::resources::PopulatedChunks::default());

        let spendable = [self.currency(), self.craft_currency()];
        let player = self.player_entity();
        let lost: Vec<(ItemId, u32)> = {
            let mut inventory = self.world.get_mut::<Inventory>(player).unwrap();
            spendable
                .into_iter()
                .filter_map(|item| {
                    let qty = inventory.take(item.clone(), u32::MAX);
                    (qty > 0).then_some((item, qty))
                })
                .collect()
        };

        self.log(format!(
            "You breach the portal and materialize in a level {new_level} sector. Hostile signal strength has spiked."
        ));
        // A second line rather than a longer first one: a neutral sector has
        // nothing to say and must read exactly as it did before sectors
        // existed. Read before logging because `Game::log` wants `&mut self`
        // while `sector` is borrowing it.
        if let Some((name, description)) = self
            .sector()
            .map(|def| (def.name.clone(), def.description.clone()))
        {
            self.log(format!("{name}. {description}"));
        }
        if !lost.is_empty() {
            let manifest = lost
                .iter()
                .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                .collect::<Vec<_>>()
                .join(" and ");
            self.log(format!(
                "Your caches decohere in transit — {manifest} lost to the breach."
            ));
        }
        self.ensure_local_population();
        self.spawn_surface_links(STACK_LINKS_PER_ZONE);
    }

    /// Breaches forward until the party is standing in `zone`, for the
    /// `savetool` binary — testing zone 6 otherwise means playing to zone 6.
    ///
    /// Deliberately a loop over the real `enter_next_zone` rather than a
    /// write to `ZoneLevel`: everything that makes a breach coherent — the
    /// base travelling, the zone-local wipes, fresh spawns scaled to the new
    /// zone — lives in that function, and a shortcut would produce a save
    /// that no amount of play could have reached. `enter_next_zone` is
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
    /// see `resources::ZoneSpawnPoint`. Marked on the map so a player can
    /// navigate back toward the (comparatively) safer ground near it, per
    /// `distance_stat_multiplier`.
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
